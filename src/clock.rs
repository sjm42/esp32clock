// clock.rs

use crate::*;

use chrono::*;
use esp_idf_hal::{gpio::*, prelude::*, spi};
use esp_idf_svc::sntp;
use max7219::MAX7219;
use tokio::time::{sleep, Duration};

const DEFAULT_VSCROLLD: u8 = 20;

#[allow(unused_variables)]
pub async fn run_clock(state: Arc<std::pin::Pin<Box<MyState>>>) -> anyhow::Result<()> {
    // set up SPI bus and MAX7219 driver

    let led_spi = state.spi.write().await.take().unwrap();
    let spi_driver = spi::SpiDriver::new::<spi::SPI2>(
        led_spi.spi,
        led_spi.sclk,
        led_spi.sdo,
        None::<AnyInputPin>,
        &spi::SpiDriverConfig::new(),
    )?;
    let spiconfig = spi::config::Config::new().baudrate(10.MHz().into());
    let spi_dev = spi::SpiDeviceDriver::new(spi_driver, Some(led_spi.cs), &spiconfig)?;
    let mut led_mat = MAX7219::from_spi(8, spi_dev).unwrap();

    // set up led matrix display

    led_mat.power_on().ok();
    (0..8).for_each(|i| {
        led_mat.clear_display(i).ok();
        led_mat.set_intensity(i, 1).ok();
    });
    let mut disp = MyDisplay::new_upside_down();

    // wait for WiFi connection to complete
    let mut cnt = 0;
    loop {
        if *state.wifi_up.read().await {
            break;
        }
        disp.print(&format!("WiFi ({})", SPIN[cnt % 4]), false);
        disp.show(&mut led_mat);

        cnt += 1;
        sleep(Duration::from_millis(200)).await;
    }

    Box::pin(disp.vscroll(DEFAULT_VSCROLLD, false, &mut led_mat, "Connect!")).await;
    sleep(Duration::from_millis(1000)).await;

    // show our IP address briefly

    let ip_info = format!("IP: {}", state.ip_addr.read().await);
    Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut led_mat, &ip_info)).await;
    sleep(Duration::from_millis(500)).await;
    Box::pin(disp.marquee(15, &mut led_mat, &ip_info)).await;

    // start up NTP
    let ntp = sntp::EspSntp::new_default()?;
    cnt = 0;
    loop {
        if Utc::now().year() > 2020 && ntp.get_sync_status() == sntp::SyncStatus::Completed {
            // we probably have NTP time by now...
            break;
        }

        disp.print(&format!("NTP..({})", SPIN[cnt % 4]), false);
        disp.show(&mut led_mat);

        cnt += 1;
        sleep(Duration::from_millis(200)).await;
    }

    Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut led_mat, "NTP OK! ")).await;
    sleep(Duration::from_millis(500)).await;

    // set up language and timezone

    let lang = &state.config.read().await.lang;
    let tz = &*state.tz.read().await;

    // finally, move to the main clock display loop

    let mut last_sec = 0;
    let mut dsec = 0u8;
    let mut time_vscroll = Some(true);
    loop {
        {
            // is reset requested?
            let mut reset = state.reset.write().await;
            if *reset {
                *reset = false;
                esp_idf_hal::reset::restart();
            }
        }

        if time_vscroll.is_none() {
            sleep(Duration::from_millis(100)).await;
        }

        let local = Utc::now().with_timezone(tz);
        let sec = local.second();
        if sec != last_sec {
            dsec = 0;
            last_sec = sec;
        } else {
            dsec += 1;
        }

        let min = local.minute();
        let hour = local.hour();
        let wday_index = local.weekday() as usize;
        let wday_s = match lang {
            MyLang::Eng => WEEKDAY_EN[wday_index],
            MyLang::Fin => WEEKDAY_FI[wday_index],
        };

        // Right after 04:42 local time, we are resetting
        if hour == 4 && min == 42 && (0..10).contains(&sec) {
            *state.reset.write().await = true;
        }

        let ts = format!("{hour:02}{min:02}:{sec:02}");
        if let Some(dir) = time_vscroll {
            let intensity = if (0..=7).contains(&hour) { 1 } else { 8 };
            (0..8).for_each(|i| {
                led_mat.set_intensity(i, intensity).ok();
            });

            Box::pin(disp.vscroll(DEFAULT_VSCROLLD, dir, &mut led_mat, &ts)).await;
        } else {
            disp.print(&ts, false);
            disp.show(&mut led_mat);
        }

        time_vscroll = match sec {
            11 | 41 => {
                // show date

                let mon_index = local.month0() as usize;
                let mon_s = match lang {
                    MyLang::Eng => MONTH_EN[mon_index],
                    MyLang::Fin => MONTH_FI[mon_index],
                };
                let day = local.day();
                // let year = local.year() - 2000;
                let year = local.year();

                let date_s1 = format!("{wday_s} {day}. ");
                let date_s2 = format!("{mon_s} {year}  ");

                Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut led_mat, &date_s1)).await;
                sleep(Duration::from_millis(1500)).await;

                Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut led_mat, &date_s2)).await;
                sleep(Duration::from_millis(1500)).await;

                Box::pin(disp.vscroll(DEFAULT_VSCROLLD, false, &mut led_mat, &date_s1)).await;
                Some(false)
            }

            21 | 51 => {
                // show temperature?

                let t = *state.temp.read().await;
                if t > NO_TEMP && state.config.read().await.enable_mqtt {
                    if *state.temp_t.read().await < local.timestamp() - 3600 {
                        // Well, MQTT is enabled, we have had earlier temp reading and now it's expired.
                        // Thus, it's better to reboot because we have some kind of a network problem.

                        *state.reset.write().await = true;
                        None
                    } else {
                        // OK we have the temp reading, show it.

                        let temp_s = format!("{t:+.1}°C");
                        Box::pin(disp.vscroll(DEFAULT_VSCROLLD, false, &mut led_mat, &temp_s))
                            .await;
                        sleep(Duration::from_millis(1500)).await;

                        Some(true)
                    }
                } else {
                    None
                }
            }
            _ => None,
        };

        // Whoa, we have an incoming message to display!
        if let Some(msg) = state.msg.write().await.take() {
            Box::pin(disp.message(DEFAULT_VSCROLLD, &mut led_mat, &msg, lang)).await;
            time_vscroll = Some(true);
        }
    }
}

// EOF
