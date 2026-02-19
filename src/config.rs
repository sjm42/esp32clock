// config.rs

use askama::Template;
use crc::{CRC_32_ISCSI, Crc};
use esp_idf_svc::nvs;
use serde::{Deserialize, Serialize};

use crate::*;

const CONFIG_NAME: &str = "cfg";
pub const NVS_BUF_SIZE: usize = 256;
pub const BOOT_FAIL_MAX: u8 = 4;

const DEFAULT_API_PORT: u16 = 80;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MyLang {
    Eng,
    Fin,
}

impl fmt::Display for MyLang {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Template)]
#[template(path = "index.html.ask", escape = "html")]
pub struct MyConfig {
    pub port: u16,

    pub wifi_ssid: String,
    pub wifi_pass: String,
    pub wifi_wpa2ent: bool,
    pub wifi_username: String,

    pub v4dhcp: bool,
    pub v4addr: net::Ipv4Addr,
    pub v4mask: u8,
    pub v4gw: net::Ipv4Addr,
    pub dns1: net::Ipv4Addr,
    pub dns2: net::Ipv4Addr,

    pub mqtt_enable: bool,
    pub mqtt_url: String,
    pub mqtt_topic: String,

    pub lang: MyLang,
    pub tz: String,
    pub lat: f32,
    pub lon: f32,

    pub sensor_enable: bool,
    pub sensor_topic: String,

    pub led_intensity_day: u8,
    pub led_intensity_night: u8,
    pub display_shutoff_enable: bool,
}

impl Default for MyConfig {
    fn default() -> Self {
        Self {
            port: option_env!("API_PORT")
                .unwrap_or("-")
                .parse()
                .unwrap_or(DEFAULT_API_PORT),

            wifi_ssid: option_env!("WIFI_SSID").unwrap_or("internet").into(),
            wifi_pass: option_env!("WIFI_PASS").unwrap_or("").into(),
            wifi_wpa2ent: false,
            wifi_username: String::new(),

            v4dhcp: true,
            v4addr: net::Ipv4Addr::new(0, 0, 0, 0),
            v4mask: 0,
            v4gw: net::Ipv4Addr::new(0, 0, 0, 0),
            dns1: net::Ipv4Addr::new(0, 0, 0, 0),
            dns2: net::Ipv4Addr::new(0, 0, 0, 0),

            mqtt_enable: false,
            mqtt_url: "mqtt://127.0.0.1:1883".into(),
            mqtt_topic: "out_temperature".into(),

            lang: MyLang::Eng,
            tz: "Europe/Helsinki".into(),
            lat: 61.5, // Tampere
            lon: 23.8, //

            sensor_enable: false,
            sensor_topic: String::new(),

            led_intensity_day: 4,
            led_intensity_night: 0,
            display_shutoff_enable: false,
        }
    }
}

impl MyConfig {
    pub fn from_nvs(nvs: &mut nvs::EspNvs<nvs::NvsDefault>) -> Option<Self> {
        let mut nvsbuf = [0u8; NVS_BUF_SIZE];
        info!("Reading up to {sz} bytes from nvs...", sz = NVS_BUF_SIZE);
        let b = match nvs.get_raw(CONFIG_NAME, &mut nvsbuf) {
            Err(e) => {
                error!("Nvs read error {e:?}");
                return None;
            }
            Ok(Some(b)) => b,
            _ => {
                error!("Nvs key not found");
                return None;
            }
        };
        info!("Got {sz} bytes from nvs. Parsing config...", sz = b.len());

        let crc = Crc::<u32>::new(&CRC_32_ISCSI);
        let digest = crc.digest();
        match postcard::from_bytes_crc32::<MyConfig>(b, digest) {
            Ok(c) => {
                info!("Successfully parsed config from nvs.");
                Some(c)
            }
            Err(e) => {
                error!("Cannot parse config from nvs: {e:?}");
                None
            }
        }
    }

    pub fn to_nvs(&self, nvs: &mut nvs::EspNvs<nvs::NvsDefault>) -> anyhow::Result<()> {
        let mut nvsbuf = [0u8; NVS_BUF_SIZE];
        let crc = Crc::<u32>::new(&CRC_32_ISCSI);
        let digest = crc.digest();
        let nvsdata = match postcard::to_slice_crc32(self, &mut nvsbuf, digest) {
            Ok(d) => d,
            Err(e) => {
                let estr = format!("Cannot encode config to buffer {e:?}");
                bail!("{estr}");
            }
        };
        info!("Encoded config to {sz} bytes. Saving to nvs...", sz = nvsdata.len());

        match nvs.set_raw(CONFIG_NAME, nvsdata) {
            Ok(_) => {
                info!("Config saved.");
                Ok(())
            }
            Err(e) => {
                let estr = format!("Cannot save to nvs: {e:?}");
                bail!("{estr}");
            }
        }
    }
}
// EOF
