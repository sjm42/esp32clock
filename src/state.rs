// state.rs

use std::net;
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
    pub if_index: RwLock<u32>,
    pub ip_addr: RwLock<Ipv4Addr>,
    pub ping_ip: RwLock<Option<Ipv4Addr>>,
    pub myid: RwLock<String>,
    pub temp: RwLock<f32>,
    pub temp_t: RwLock<i64>,
    pub msg: RwLock<Option<String>>,
    pub tz: RwLock<Tz>,
    pub reset: RwLock<bool>,
}

impl MyState {
    pub fn new(config: MyConfig, nvs: nvs::EspNvs<nvs::NvsDefault>, pins: MyPins, tz: Tz) -> Self {
        MyState {
            config: RwLock::new(config),
            api_cnt: RwLock::new(0),
            nvs: RwLock::new(nvs),
            pins: RwLock::new(Some(pins)),
            wifi_up: RwLock::new(false),
            if_index: RwLock::new(0),
            ip_addr: RwLock::new(net::Ipv4Addr::new(0, 0, 0, 0)),
            ping_ip: RwLock::new(None),
            myid: RwLock::new("esp32clock".into()),
            temp: RwLock::new(NO_TEMP),
            temp_t: RwLock::new(0),
            msg: RwLock::new(None),
            tz: RwLock::new(tz),
            reset: RwLock::new(false),
        }
    }
}

// EOF
