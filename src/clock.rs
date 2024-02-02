// clock.rs

use crate::*;

use chrono::*;
use chrono_tz::Europe::Helsinki;
use esp_idf_hal::{gpio::*, prelude::*, spi};
use esp_idf_svc::sntp;
use max7219::MAX7219;
use tokio::time::{sleep, Duration};

const SPIN: [char; 4] = ['|', '/', '-', '\\'];

const WEEKDAY_FI: [&str; 7] = ["Ma", "Ti", "Ke", "To", "Pe", "La", "Su"];

#[rustfmt::skip]
const MONTH_FI: [&str; 12] = [
    "tammikuu", "helmikuu", "maaliskuu", "huhtikuu", "toukokuu",  "kesäkuu",
    "heinäkuu", "elokuu",   "syyskuu",   "lokakuu",  "marraskuu", "joulukuu",
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
        led_mat.set_intensity(i, 8).ok();
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
    sleep(Duration::from_millis(500)).await;
    Box::pin(disp.marquee(2, &mut led_mat, "Connect!")).await;
    sleep(Duration::from_millis(500)).await;

    Box::pin(disp.drop(10, &mut led_mat, "NTP.....")).await;
    sleep(Duration::from_millis(500)).await;
    let _ntp = sntp::EspSntp::new_default()?;
    disp.print("NTP OK!");
    disp.show(&mut led_mat);
    sleep(Duration::from_millis(500)).await;
    Box::pin(disp.marquee(2, &mut led_mat, "NTP OK!")).await;

    loop {
        let mut local = Utc::now().with_timezone(&Helsinki);
        let s = local.second();
        if s == 11 || s == 41 {
            let wday_s = WEEKDAY_FI[local.weekday() as usize];
            let mon_s = MONTH_FI[local.month0() as usize];
            let date_fmt = format!("{wday_s}  %d. {mon_s} %Y");
            let date = local.format(&date_fmt).to_string();

            Box::pin(disp.drop(10, &mut led_mat, &date)).await;
            sleep(Duration::from_millis(1000)).await;
            Box::pin(disp.marquee(25, &mut led_mat, &date)).await;

            local = Utc::now().with_timezone(&Helsinki);
            let ts = local.format("%H:%M   ").to_string();
            Box::pin(disp.drop(10, &mut led_mat, &ts)).await;

            continue;
        }

        let ts = local.format("%H:%M:%S").to_string();
        disp.print(&ts);
        disp.show(&mut led_mat);

        sleep(Duration::from_millis(200)).await;
    }
}

// EOF
