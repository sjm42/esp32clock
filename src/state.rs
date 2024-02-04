// state.rs

use std::net::Ipv4Addr;

use crate::*;

use chrono_tz::Tz;
use esp_idf_hal::{gpio::AnyOutputPin, spi::SPI2};
use esp_idf_svc::nvs;
use tokio::sync::RwLock;

pub struct LedSpi {
    pub spi: SPI2,
    pub sclk: AnyOutputPin,
    pub sdo: AnyOutputPin,
    pub cs: AnyOutputPin,
}

// unsafe impl Send for LedSpi {}
unsafe impl Sync for LedSpi {}

pub struct MyState {
    pub config: RwLock<MyConfig>,
    pub cnt: RwLock<u64>,
    pub nvs: RwLock<nvs::EspNvs<nvs::NvsDefault>>,
    pub spi: RwLock<Option<LedSpi>>,
    pub wifi_up: RwLock<bool>,
    pub ip_addr: RwLock<Ipv4Addr>,
    pub myid: RwLock<String>,
    pub temp: RwLock<f32>,
    pub msg: RwLock<Option<String>>,
    pub tz: RwLock<Tz>,
    pub reset: RwLock<bool>,
}

// EOF
