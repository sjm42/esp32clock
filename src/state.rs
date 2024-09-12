// state.rs

use std::net::Ipv4Addr;

use chrono_tz::Tz;
use esp_idf_hal::{
    gpio::{AnyInputPin, AnyOutputPin},
    rmt,
    spi::SPI2,
};
use esp_idf_svc::nvs;

use crate::*;


pub struct MyPins {
    pub rmt: rmt::CHANNEL0,
    pub spi: SPI2,
    pub sclk: AnyOutputPin,
    pub sdo: AnyOutputPin,
    pub cs: AnyOutputPin,
    pub button: AnyInputPin,
}

unsafe impl Sync for MyPins {}

pub struct MyState {
    pub config: RwLock<MyConfig>,
    pub api_cnt: RwLock<u64>,
    pub nvs: RwLock<nvs::EspNvs<nvs::NvsDefault>>,
    pub pins: RwLock<Option<MyPins>>,
    pub wifi_up: RwLock<bool>,
    pub ip_addr: RwLock<Ipv4Addr>,
    pub myid: RwLock<String>,
    pub temp: RwLock<f32>,
    pub temp_t: RwLock<i64>,
    pub msg: RwLock<Option<String>>,
    pub tz: RwLock<Tz>,
    pub reset: RwLock<bool>,
}
// EOF
