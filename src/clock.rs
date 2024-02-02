// clock.rs

use crate::*;

use esp_idf_hal::{gpio::*, prelude::*, spi};
use max7219::MAX7219;
use tokio::time::{sleep, Duration};

#[allow(unreachable_code)]
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

    /*
    // just raw chars, don't care about orientation
    for i in 0..8 {
        let offset = (0x30 + i) * 8;
        let char: &[u8; 8] = &FONT[offset..offset + 8].try_into().unwrap();
        led_mat.write_raw(i, char).ok();
    }
    sleep(Duration::from_secs(2)).await;
    */

    let mut disp = MyDisplay::new_upside_down();
    loop {
        disp.print("Pällit!");
        disp.show(&mut led_mat);
        sleep(Duration::from_secs(1)).await;

        disp.clear();
        disp.show(&mut led_mat);
        sleep(Duration::from_millis(500)).await;

        for _ in 0..3 {
            disp.clear();
            disp.show(&mut led_mat);
            sleep(Duration::from_millis(200)).await;

            disp.print("Pällit!");
            disp.show(&mut led_mat);
            sleep(Duration::from_millis(200)).await;
        }

        disp.drop(&mut led_mat, "Ojennus!").await;

        sleep(Duration::from_secs(1)).await;
        disp.marquee(&mut led_mat, "Ojennus!").await;
    }

    loop {
        sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}

// EOF
