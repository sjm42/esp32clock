// bin/esp32clock.rs

#![warn(clippy::large_futures)]

use chrono_tz::Etc::UTC;
use esp_idf_hal::{delay::FreeRtos, gpio::Pull};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop, nvs, ota::EspOta, ping, timer::EspTaskTimerService,
    wifi::WifiDriver,
};
use esp_idf_sys::esp;
use one_wire_bus::OneWire;

use esp32clock::*;

// DANGER! DO NOT USE THIS until esp-idf-svc supports newer versions of ESP-IDF
// - until then, only up to esp-idf 5.3.2 is supported with esp_app_desc!()
// Without the macro usage up to esp-idf v5.4.2 is supported.
// ESP-IDF version 5.5 requires updated esp-idf-svc crate to be released.

// use esp_idf_sys::esp_app_desc;
// esp_app_desc!();

fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    // eventfd is needed by our mio poll implementation.  Note you should set max_fds
    // higher if you have other code that may need eventfd.

    #[allow(clippy::needless_update)]
    let config = esp_idf_sys::esp_vfs_eventfd_config_t {
        max_fds: 1,
        ..Default::default()
    };
    esp! { unsafe { esp_idf_sys::esp_vfs_eventfd_register(&config) } }?;

    // comment or uncomment these, if you encounter this boot error:
    // E (439) esp_image: invalid segment length 0xXXXX
    // this means that the code size is not 32bit aligned
    // and any small change to the code will likely fix it.
    info!("Hello.");
    info!("Starting up, firmare version {}", FW_VERSION);
    let ota_slot = {
        let mut ota = EspOta::new()?;
        let running_slot = ota.get_running_slot()?;
        ota.mark_running_slot_valid()?;
        let ota_slot = format!("{} ({:?})", &running_slot.label, running_slot.state);
        info!("OTA slot: {ota_slot}");
        ota_slot
    };

    let sysloop = EspSystemEventLoop::take()?;
    let timer = EspTaskTimerService::new()?;
    let nvs_default_partition = nvs::EspDefaultNvsPartition::take()?;

    let ns = env!("CARGO_BIN_NAME");
    let mut nvs = match nvs::EspNvs::new(nvs_default_partition.clone(), ns, true) {
        Ok(nvs) => {
            info!("Got namespace {ns:?} from default partition");
            nvs
        }
        Err(e) => panic!("Could not get namespace {ns}: {e:?}"),
    };

    #[cfg(feature = "reset_settings")]
    let config = {
        let c = MyConfig::default();
        c.to_nvs(&mut nvs)?;
        c
    };

    #[cfg(not(feature = "reset_settings"))]
    let config = match MyConfig::from_nvs(&mut nvs) {
        None => {
            error!("Could not read nvs config, using defaults");
            let c = MyConfig::default();
            c.to_nvs(&mut nvs)?;
            info!("Successfully saved default config to nvs.");
            c
        }

        // using settings saved on nvs if we could find them
        Some(c) => c,
    };
    info!("My config:\n{config:#?}");

    info!("Setting timezone...");
    let tz_s = &config.tz;
    let tz = tz_s.parse().unwrap_or_else(|e| {
        error!("Cannot parse timezone {tz_s:?}: {e:?}");
        error!("Defaulting to UTC.");
        UTC
    });

    let peripherals = Peripherals::take()?;
    let pins = peripherals.pins;
    let rmt = peripherals.rmt.channel0;

    let spi = peripherals.spi2;
    let sclk = pins.gpio0.downgrade_output();
    let cs = pins.gpio1.downgrade_output();
    let sdo = pins.gpio2.downgrade_output();
    let button = pins.gpio9.downgrade_input();
    let mypins = MyPins {
        rmt,
        spi,
        sclk,
        sdo,
        cs,
        button,
    };

    let mut onewire = pins.gpio8.downgrade();
    let onewire_addr = if config.sensor_enable {
        info!("Sensor: scanning 1-wire devices...");

        let mut pin_drv = gpio::PinDriver::input_output_od(&mut onewire)?;
        pin_drv.set_pull(Pull::Up)?;
        let mut w = OneWire::new(pin_drv).unwrap();

        if let Ok(a) = scan_1wire(&mut w) {
            info!("Onewire response: {a:#?}");
            a
        } else {
            one_wire_bus::Address(0)
        }
    } else {
        info!("Sensor is disabled.");
        one_wire_bus::Address(0)
    };
    let onewire_pin = MyOnewire { onewire };

    let state = Box::pin(MyState::new(
        config,
        nvs,
        onewire_addr,
        tz,
        ota_slot,
        mypins,
        onewire_pin,
    ));
    let shared_state = Arc::new(state);

    let wifidriver = WifiDriver::new(
        peripherals.modem,
        sysloop.clone(),
        Some(nvs_default_partition),
    )?;

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(Box::pin(async move {
            let wifi_loop = WifiLoop {
                state: shared_state.clone(),
                wifi: None,
            };

            info!("Entering main loop...");
            tokio::select! {
                _ = Box::pin(run_clock(shared_state.clone())) => { error!("run_clock() ended."); }
                _ = Box::pin(poll_sensor(shared_state.clone())) => { error!("poll_sensor() ended."); }
                _ = Box::pin(run_mqtt(shared_state.clone())) => { error!("run_mqtt() ended."); }
                _ = Box::pin(run_api_server(shared_state.clone())) => { error!("run_api_server() ended."); }
                _ = Box::pin(wifi_loop.run(wifidriver, sysloop, timer)) => { error!("wifi_loop.run() ended."); }
                _ = Box::pin(pinger(shared_state.clone())) => { error!("pinger() ended."); }
            };
        }));

    // not actually returing from main() but we reboot instead
    info!("main() finished, reboot.");
    FreeRtos::delay_ms(3000);
    esp_idf_hal::reset::restart();
}

async fn pinger(state: Arc<std::pin::Pin<Box<MyState>>>) -> anyhow::Result<()> {
    loop {
        sleep(Duration::from_secs(300)).await;

        if let Some(ping_ip) = *state.ping_ip.read().await {
            let if_idx = *state.if_index.read().await;
            if if_idx > 0 {
                tracing::log::info!("Starting ping {ping_ip} (if_idx {if_idx})");
                let conf = ping::Configuration {
                    count: 2,
                    interval: Duration::from_millis(500),
                    timeout: Duration::from_millis(200),
                    data_size: 64,
                    tos: 0,
                };
                let mut ping = ping::EspPing::new(if_idx);
                let res = ping.ping(ping_ip, &conf)?;
                tracing::log::info!("Pinger result: {res:?}");
                if res.received == 0 {
                    tracing::log::error!("Ping failed, rebooting.");
                    sleep(Duration::from_millis(2000)).await;
                    esp_idf_hal::reset::restart();
                }
            } else {
                tracing::log::error!("No if_index. wat?");
            }
        }
    }
}
// EOF
