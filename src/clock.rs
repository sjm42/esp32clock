// clock.rs

use crate::*;

use chrono::*;
use esp_idf_svc::sntp;
use std::rc::Rc;
use tokio::time::{sleep, Duration};

const DEFAULT_VSCROLLD: u8 = 20;
const CONFIG_RESET_COUNT: i32 = 9;

// #[allow(unused_variables)]
pub async fn run_clock(state: Arc<std::pin::Pin<Box<MyState>>>) -> anyhow::Result<()> {
    // set up SPI bus and MAX7219 driver

    let pins = state.pins.write().await.take().unwrap();
    let button = gpio::PinDriver::input(pins.button)?;

    let spi_driver = spi::SpiDriver::new::<spi::SPI2>(
        pins.spi,
        pins.sclk,
        pins.sdo,
        None::<AnyInputPin>,
        &spi::SpiDriverConfig::new(),
    )?;
    let spiconfig = spi::config::Config::new().baudrate(10.MHz().into());
    let spi_dev = spi::SpiDeviceDriver::new(spi_driver, Some(pins.cs), &spiconfig)?;
    let led_mat = Rc::new(RwLock::new(MAX7219::from_spi(8, spi_dev).unwrap()));

    // set up led matrix display

    {
        let mut mat = led_mat.write().await;
        mat.power_on().ok();
        for i in 0..8 {
            mat.clear_display(i).ok();
            mat.set_intensity(i, 1).ok();
        }
    }
    let mut disp = MyDisplay::new_upside_down();

    // wait for WiFi connection to complete
    let mut cnt = 0;
    loop {
        if *state.wifi_up.read().await {
            break;
        }
        disp.print(&format!("WiFi ({})", SPIN[cnt % 4]), false);
        disp.show(&mut *led_mat.write().await);

        if button.is_low() {
            Box::pin(reset_button(state.clone(), &button, led_mat.clone())).await?;
        }

        cnt += 1;
        sleep(Duration::from_millis(200)).await;
    }

    Box::pin(disp.vscroll(
        DEFAULT_VSCROLLD,
        false,
        &mut *led_mat.write().await,
        "Connect!",
    ))
    .await;
    sleep(Duration::from_millis(1000)).await;

    // show our IP address briefly

    let ip_info = format!("IP: {}", state.ip_addr.read().await);
    {
        let mut mat = led_mat.write().await;
        Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut mat, &ip_info)).await;
        sleep(Duration::from_millis(500)).await;
        Box::pin(disp.marquee(15, &mut mat, &ip_info)).await;
    }

    // start up NTP
    let ntp = sntp::EspSntp::new_default()?;
    cnt = 0;
    loop {
        if Utc::now().year() > 2020 && ntp.get_sync_status() == sntp::SyncStatus::Completed {
            // we probably have NTP time by now...
            break;
        }

        disp.print(&format!("NTP..({})", SPIN[cnt % 4]), false);
        disp.show(&mut *led_mat.write().await);

        if button.is_low() {
            Box::pin(reset_button(state.clone(), &button, led_mat.clone())).await?;
        }

        cnt += 1;
        sleep(Duration::from_millis(200)).await;
    }

    Box::pin(disp.vscroll(
        DEFAULT_VSCROLLD,
        true,
        &mut *led_mat.write().await,
        "NTP OK! ",
    ))
    .await;
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

        if button.is_low() {
            Box::pin(reset_button(state.clone(), &button, led_mat.clone())).await?;
        }

        let local = Utc::now().with_timezone(tz);
        let sec = local.second();
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
            for i in 0..8 {
                led_mat.write().await.set_intensity(i, intensity).ok();
            }

            Box::pin(disp.vscroll(DEFAULT_VSCROLLD, dir, &mut *led_mat.write().await, &ts)).await;
        } else {
            disp.print(&ts, false);
            disp.show(&mut *led_mat.write().await);
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

                Box::pin(disp.vscroll(
                    DEFAULT_VSCROLLD,
                    true,
                    &mut *led_mat.write().await,
                    &date_s1,
                ))
                .await;
                sleep(Duration::from_millis(1500)).await;

                Box::pin(disp.vscroll(
                    DEFAULT_VSCROLLD,
                    true,
                    &mut *led_mat.write().await,
                    &date_s2,
                ))
                .await;
                sleep(Duration::from_millis(1500)).await;

                Box::pin(disp.vscroll(
                    DEFAULT_VSCROLLD,
                    false,
                    &mut *led_mat.write().await,
                    &date_s1,
                ))
                .await;
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
                        Box::pin(disp.vscroll(
                            DEFAULT_VSCROLLD,
                            false,
                            &mut *led_mat.write().await,
                            &temp_s,
                        ))
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
            Box::pin(disp.message(DEFAULT_VSCROLLD, &mut *led_mat.write().await, &msg, lang)).await;
            time_vscroll = Some(true);
        }
    }
}

async fn reset_button<'a, 'b>(
    state: Arc<std::pin::Pin<Box<MyState>>>,
    button: &PinDriver<'a, AnyInputPin, Input>,
    led_mat: Rc<RwLock<MAX7219<SpiConnector<SpiDeviceDriver<'b, SpiDriver<'b>>>>>>,
) -> anyhow::Result<()> {
    let mut reset_cnt = CONFIG_RESET_COUNT;
    let mut disp = MyDisplay::new_upside_down();

    while button.is_low() {
        // button is pressed and kept down, countdown and factory reset if reach zero
        let msg = format!("Reset? {reset_cnt}");
        error!("{msg}");
        disp.print(&msg, false);
        disp.show(&mut *led_mat.write().await);

        if reset_cnt == 0 {
            // okay do factory reset now
            error!("Factory resetting...");
            disp.print("Reset...", false);
            disp.show(&mut *led_mat.write().await);

            let new_config = MyConfig::default();
            new_config.to_nvs(&mut *state.nvs.write().await)?;
            sleep(Duration::from_millis(2000)).await;
            esp_idf_hal::reset::restart();
        }

        reset_cnt -= 1;
        sleep(Duration::from_millis(500)).await;
        continue;
    }
    Ok(())
}
// EOF
