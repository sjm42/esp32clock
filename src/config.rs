// config.rs

use anyhow::bail;
use askama::Template;
use crc::{Crc, CRC_32_ISCSI};
use esp_idf_svc::nvs;
use log::*;
use serde::{Deserialize, Serialize};
use std::{fmt, net};

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
#[template(path = "index.html")]
pub struct MyConfig {
    pub port: u16,

    pub wifi_ssid: String,
    pub wifi_pass: String,

    pub v4dhcp: bool,
    pub v4addr: net::Ipv4Addr,
    pub v4mask: u8,
    pub v4gw: net::Ipv4Addr,

    pub enable_mqtt: bool,
    pub mqtt_url: String,
    pub temp_topic: String,

    pub lang: MyLang,
    pub tz: String,
}

impl Default for MyConfig {
    fn default() -> Self {
        Self {
            port: option_env!("API_PORT")
                .unwrap_or("-")
                .parse()
                .unwrap_or(DEFAULT_API_PORT),

            wifi_ssid: option_env!("WIFI_SSID").unwrap_or("internet").into(),
            wifi_pass: option_env!("WIFI_PASS").unwrap_or("password").into(),

            v4dhcp: true,
            v4addr: net::Ipv4Addr::new(0, 0, 0, 0),
            v4mask: 0,
            v4gw: net::Ipv4Addr::new(0, 0, 0, 0),

            enable_mqtt: false,
            mqtt_url: "mqtt://127.0.0.1:1883".into(),
            temp_topic: "out_temperature".into(),

            lang: MyLang::Eng,
            tz: "Europe/Helsinki".into(),
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
        info!(
            "Encoded config to {sz} bytes. Saving to nvs...",
            sz = nvsdata.len()
        );

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
