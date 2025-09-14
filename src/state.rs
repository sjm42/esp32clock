// state.rs

use esp_idf_hal::{
    gpio::{AnyIOPin, AnyInputPin, AnyOutputPin},
    rmt,
    spi::SPI2,
};
use esp_idf_svc::nvs;
use one_wire_bus::Address;

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

pub struct MyOnewire {
    pub onewire: AnyIOPin,
}
unsafe impl Sync for MyOnewire {}

pub struct MyState {
    pub config: MyConfig,
    pub onewire_addr: Address,
    pub tz: Tz,

    pub nvs: RwLock<nvs::EspNvs<nvs::NvsDefault>>,
    pub pins: RwLock<Option<MyPins>>,
    pub onewire_pin: RwLock<Option<MyOnewire>>,
    pub wifi_up: RwLock<bool>,
    pub if_index: RwLock<u32>,
    pub ip_addr: RwLock<Ipv4Addr>,
    pub ping_ip: RwLock<Option<Ipv4Addr>>,
    pub myid: RwLock<String>,
    pub api_cnt: RwLock<u64>,
    pub display_enabled: RwLock<bool>,
    pub temp: RwLock<f32>,
    pub temp_t: RwLock<i64>,
    pub meas: RwLock<f32>,
    pub meas_updated: RwLock<bool>,
    pub msg: RwLock<Option<String>>,

    pub reset: RwLock<bool>,
}

impl MyState {
    pub fn new(
        config: MyConfig,
        nvs: nvs::EspNvs<nvs::NvsDefault>,
        onewire_addr: Address,
        tz: Tz,
        pins: MyPins,
        onewire_pin: MyOnewire,
    ) -> Self {
        MyState {
            config,
            onewire_addr,
            tz,

            nvs: RwLock::new(nvs),
            pins: RwLock::new(Some(pins)),
            onewire_pin: RwLock::new(Some(onewire_pin)),
            wifi_up: RwLock::new(false),
            if_index: RwLock::new(0),
            ip_addr: RwLock::new(net::Ipv4Addr::new(0, 0, 0, 0)),
            ping_ip: RwLock::new(None),
            myid: RwLock::new("esp32clock".into()),
            api_cnt: RwLock::new(0),
            display_enabled: RwLock::new(true),
            temp: RwLock::new(NO_TEMP),
            temp_t: RwLock::new(0),
            meas: RwLock::new(NO_TEMP),
            meas_updated: RwLock::new(false),
            msg: RwLock::new(None),
            reset: RwLock::new(false),
        }
    }
}
// EOF
