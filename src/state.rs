// state.rs

use esp_idf_hal::{gpio::AnyOutputPin, spi::SPI2};
use esp_idf_svc::nvs;
use one_wire_bus::Address;

use crate::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResetDisplayState {
    None,
    Countdown(i32),
    FactoryResetting,
}

pub struct MyPins {
    pub spi: SPI2<'static>,
    pub sclk: AnyOutputPin<'static>,
    pub sdo: AnyOutputPin<'static>,
    pub cs: AnyOutputPin<'static>,
}

pub struct MyState {
    pub ap_mode: bool,
    pub config: MyConfig,
    pub onewire_addr: Address,
    pub tz: Tz,
    pub ota_slot: String,
    pub boot_instant: std::time::Instant,

    pub api_cnt: AtomicU32,
    pub nvs: RwLock<nvs::EspNvs<nvs::NvsDefault>>,
    pub activity_led: Mutex<gpio::PinDriver<'static, gpio::Output>>,
    pub wifi_up: RwLock<bool>,
    pub if_index: RwLock<u32>,
    pub ip_addr: RwLock<Ipv4Addr>,
    pub ping_ip: RwLock<Option<Ipv4Addr>>,
    pub myid: RwLock<String>,
    pub display_enabled: RwLock<bool>,
    pub temp: RwLock<f32>,
    pub temp_t: RwLock<i64>,
    pub meas: RwLock<f32>,
    pub meas_updated: RwLock<bool>,
    pub msg: RwLock<Option<String>>,
    pub reset_display: RwLock<ResetDisplayState>,

    pub reset: RwLock<bool>,
}

impl MyState {
    pub fn new(
        ap_mode: bool,
        config: MyConfig,
        nvs: nvs::EspNvs<nvs::NvsDefault>,
        onewire_addr: Address,
        tz: Tz,
        ota_slot: String,
        activity_led: gpio::PinDriver<'static, gpio::Output>,
    ) -> Self {
        MyState {
            ap_mode,
            config,
            onewire_addr,
            tz,
            ota_slot,
            boot_instant: std::time::Instant::now(),

            api_cnt: 0.into(),
            nvs: RwLock::new(nvs),
            activity_led: Mutex::new(activity_led),
            wifi_up: RwLock::new(false),
            if_index: RwLock::new(0),
            ip_addr: RwLock::new(net::Ipv4Addr::new(0, 0, 0, 0)),
            ping_ip: RwLock::new(None),
            myid: RwLock::new("esp32clock".into()),
            display_enabled: RwLock::new(true),
            temp: RwLock::new(NO_TEMP),
            temp_t: RwLock::new(0),
            meas: RwLock::new(NO_TEMP),
            meas_updated: RwLock::new(false),
            msg: RwLock::new(None),
            reset_display: RwLock::new(ResetDisplayState::None),
            reset: RwLock::new(false),
        }
    }

    pub async fn set_led(&self, enabled: bool) -> anyhow::Result<()> {
        let mut led = self.activity_led.lock().await;
        if enabled {
            led.set_low()?;
        } else {
            led.set_high()?;
        }
        Ok(())
    }

    pub async fn led_on(&self) -> anyhow::Result<()> {
        self.set_led(true).await
    }

    pub async fn led_off(&self) -> anyhow::Result<()> {
        self.set_led(false).await
    }

    pub async fn pulse_led(&self, duration: Duration) -> anyhow::Result<()> {
        self.led_on().await?;
        sleep(duration).await;
        self.led_off().await
    }

    pub async fn request_ap_mode_on_next_boot(&self) -> anyhow::Result<()> {
        self.nvs.write().await.set_u8(AP_MODE_NVS_KEY, 1)?;
        Ok(())
    }
}
// EOF
