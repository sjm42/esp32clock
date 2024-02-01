// clock.rs

use crate::*;

use esp_idf_hal::{gpio::*, prelude::*, spi};
use max7219::MAX7219;
use tokio::time::{sleep, Duration};

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
        led_mat.set_intensity(i, 4).ok();
    });

    loop {
        (0..8).for_each(|i| {
            led_mat
                .write_raw(
                    i,
                    &[
                        0b01010101, 0b10101010, 0b01010101, 0b10101010, 0b01010101, 0b10101010,
                        0b01010101, 0b10101010,
                    ],
                )
                .ok();
        });
        sleep(Duration::from_secs(1)).await;

        (0..8).for_each(|i| {
            led_mat
                .write_raw(
                    i,
                    &[
                        0b10101010, 0b01010101, 0b10101010, 0b01010101, 0b10101010, 0b01010101,
                        0b10101010, 0b01010101,
                    ],
                )
                .ok();
        });

        sleep(Duration::from_secs(1)).await;
    }
    // Ok(())
}

// EOF
