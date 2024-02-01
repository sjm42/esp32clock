// state.rs

use crate::*;

use esp_idf_hal::{gpio::AnyIOPin, spi::SPI2};
use esp_idf_svc::nvs;
use tokio::sync::RwLock;

pub struct LedSpi {
    pub spi: SPI2,
    pub sclk: AnyIOPin,
    pub sdo: AnyIOPin,
    pub cs: AnyIOPin,
}

// unsafe impl Send for LedSpi {}
unsafe impl Sync for LedSpi {}

pub struct MyState {
    pub config: RwLock<MyConfig>,
    pub cnt: RwLock<u64>,
    pub nvs: RwLock<nvs::EspNvs<nvs::NvsDefault>>,
    pub spi: RwLock<Option<LedSpi>>,
    pub reset: RwLock<bool>,
}

// EOF
