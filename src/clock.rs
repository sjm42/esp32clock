// clock.rs

use crate::*;

use chrono::*;
use esp_idf_hal::{gpio::*, prelude::*, spi};
use esp_idf_svc::sntp;
use max7219::MAX7219;
use tokio::time::{sleep, Duration};

const DEFAULT_VSCROLLD: u8 = 20;

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
        disp.print(&format!("WiFi ({})", SPIN[cnt % 4]));
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

    Box::pin(disp.vscroll(DEFAULT_VSCROLLD, false, &mut led_mat, "NTP.....")).await;
    sleep(Duration::from_millis(500)).await;

    let _ntp = sntp::EspSntp::new_default()?;
    Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut led_mat, "NTP OK! ")).await;
    sleep(Duration::from_millis(500)).await;

    // set up language and timezone

    let lang = &state.config.read().await.lang;
    let tz = &*state.tz.read().await;

    // finally, move to the main clock display loop

    let mut time_vscroll = Some(true);
    loop {
        if time_vscroll.is_none() {
            sleep(Duration::from_millis(100)).await;
        }

        {
            // is reset requested?
            let mut reset = state.reset.write().await;
            if *reset {
                *reset = false;
                esp_idf_hal::reset::restart();
            }
        }

        let local = Utc::now().with_timezone(tz);
        let hour = local.hour();
        let min = local.minute();
        let sec = local.second();
        let ts = local.format(" %H%M%S ").to_string();

        // Right after 04:42 local time, we are resetting
        if hour == 4 && min == 42 && (0..10).contains(&sec) {
            *state.reset.write().await = true;
        }

        if let Some(dir) = time_vscroll {
            let intensity = if (0..=7).contains(&hour) { 1 } else { 8 };
            (0..8).for_each(|i| {
                led_mat.set_intensity(i, intensity).ok();
            });

            Box::pin(disp.vscroll(DEFAULT_VSCROLLD, dir, &mut led_mat, &ts)).await;
        } else {
            disp.print(&ts);
            disp.show(&mut led_mat);
        }

        time_vscroll = match sec {
            11 | 41 => {
                // show date

                let wday_s = match lang {
                    MyLang::Eng => WEEKDAY_EN[local.weekday() as usize],
                    MyLang::Fin => WEEKDAY_FI[local.weekday() as usize],
                };
                let mon_s = match lang {
                    MyLang::Eng => MONTH_EN[local.month0() as usize],
                    MyLang::Fin => MONTH_FI[local.month0() as usize],
                };
                let day = local.day();
                let year = local.year();

                let date_s1 = format!(" {wday_s} {day}. ");
                let date_s2 = format!("{mon_s} {year:04}");

                Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut led_mat, &date_s1)).await;
                sleep(Duration::from_millis(1500)).await;

                Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut led_mat, &date_s2)).await;
                sleep(Duration::from_millis(1500)).await;

                Box::pin(disp.vscroll(DEFAULT_VSCROLLD, false, &mut led_mat, &date_s1)).await;

                Some(false)
            }

            21 | 51 => {
                // show temperature

                let t = *state.temp.read().await;

                // don't show temperature if it was not updated ever, or more than 1 hour ago
                if t > -1000.0 && *state.temp_t.read().await > local.timestamp() - 3600 {
                    let temp_s = format!(" {t:+.1}°C");
                    Box::pin(disp.vscroll(DEFAULT_VSCROLLD, false, &mut led_mat, &temp_s)).await;
                    sleep(Duration::from_millis(1500)).await;

                    Some(true)
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
