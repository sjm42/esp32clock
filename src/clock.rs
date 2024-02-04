// clock.rs

use crate::*;

use chrono::*;
use chrono_tz::Europe::Helsinki;
use esp_idf_hal::{gpio::*, prelude::*, spi};
use esp_idf_svc::sntp;
use max7219::MAX7219;
use tokio::time::{sleep, Duration};

const SPIN: [char; 4] = ['|', '/', '-', '\\'];

const WEEKDAY_EN: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
const WEEKDAY_FI: [&str; 7] = ["Ma", "Ti", "Ke", "To", "Pe", "La", "Su"];

#[rustfmt::skip]
const MONTH_EN: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun",
    "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[rustfmt::skip]
const MONTH_FI: [&str; 12] = [
    "Tam", "Hel", "Maa", "Huh", "Tou", "Kes",
    "Hei", "Elo", "Syy", "Lok", "Mar", "Jou",
];

pub async fn run_clock(state: Arc<std::pin::Pin<Box<MyState>>>) -> anyhow::Result<()> {
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

    led_mat.power_on().ok();
    (0..8).for_each(|i| {
        led_mat.clear_display(i).ok();
        led_mat.set_intensity(i, 1).ok();
    });
    let mut disp = MyDisplay::new_upside_down();

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

    Box::pin(disp.drop(10, &mut led_mat, "Connect!")).await;
    sleep(Duration::from_millis(1000)).await;

    let ip_info = format!("IP: {}", state.ip_addr.read().await);
    Box::pin(disp.drop(10, &mut led_mat, &ip_info)).await;
    sleep(Duration::from_millis(500)).await;
    Box::pin(disp.marquee(10, &mut led_mat, &ip_info)).await;

    Box::pin(disp.drop(10, &mut led_mat, "NTP.....")).await;
    sleep(Duration::from_millis(500)).await;

    let _ntp = sntp::EspSntp::new_default()?;
    Box::pin(disp.drop(10, &mut led_mat, "NTP OK! ")).await;
    sleep(Duration::from_millis(500)).await;

    let lang = &state.config.read().await.lang;

    let mut time_drop = true;
    loop {
        sleep(Duration::from_millis(200)).await;

        {
            // is reset requested?
            let mut reset = state.reset.write().await;
            if *reset {
                *reset = false;
                esp_idf_hal::reset::restart();
            }
        }

        let local = Utc::now().with_timezone(&Helsinki);
        let hour = local.hour();
        let min = local.minute();
        let sec = local.second();
        let ts = local.format("%H:%M:%S").to_string();

        // Right after 04:42 local time, we are resetting
        if hour == 4 && min == 42 && (0..10).contains(&sec) {
            *state.reset.write().await = true;
        }

        if time_drop {
            Box::pin(disp.drop(10, &mut led_mat, &ts)).await;
        } else {
            disp.print(&ts);
            disp.show(&mut led_mat);
        }

        time_drop = match sec {
            11 | 41 => {
                let intensity = if (8..=23).contains(&hour) { 4 } else { 1 };
                (0..8).for_each(|i| {
                    led_mat.set_intensity(i, intensity).ok();
                });

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
                Box::pin(disp.drop(10, &mut led_mat, &date_s1)).await;
                sleep(Duration::from_millis(1000)).await;

                let date_s2 = format!("{mon_s} {year:04}");
                Box::pin(disp.drop(10, &mut led_mat, &date_s2)).await;
                sleep(Duration::from_millis(1000)).await;

                true
            }

            1 | 21 | 31 | 51 => {
                let t = *state.temp.read().await;
                if t > -1000.0 {
                    let temp_s = format!(" {t:+.1}°C");
                    Box::pin(disp.drop(10, &mut led_mat, &temp_s)).await;
                    sleep(Duration::from_millis(1500)).await;

                    true
                } else {
                    false
                }
            }
            _ => false,
        };

        // Whoa we have an incoming message to display!
        if let Some(msg) = state.msg.write().await.take() {
            Box::pin(disp.message(50, &mut led_mat, &msg)).await;
        }
    }
}

// EOF
