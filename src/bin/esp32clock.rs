// bin/esp32clock.rs

#![warn(clippy::large_futures)]

use chrono_tz::Etc::UTC;
use esp32clock::*;
use esp_idf_hal::{
    delay::FreeRtos,
    gpio::{Input, PinDriver, Pull},
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop, nvs, ota::EspOta, ping, timer::EspTaskTimerService, wifi::WifiDriver,
};
use esp_idf_sys::esp;
use one_wire_bus::OneWire;

// use esp_idf_sys::esp_app_desc;
// esp_app_desc!();

const CONFIG_RESET_COUNT: i32 = 9;
const BUTTON_POLL_MS: u64 = 500;
const BUTTON_BLINK_MS: u64 = 500;
const BUTTON_COUNTDOWN_STEP_MS: u64 = 500;

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
    info!("Starting up, firmware version {}", FW_VERSION);
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

    let ap_mode = matches!(nvs.get_u8(AP_MODE_NVS_KEY)?, Some(1));
    if ap_mode {
        info!("One-shot AP mode requested for this boot.");
        let _ = nvs.remove(AP_MODE_NVS_KEY)?;
    }

    info!("Setting timezone...");
    let tz_s = &config.tz;
    let tz = tz_s.parse().unwrap_or_else(|e| {
        error!("Cannot parse timezone {tz_s:?}: {e:?}");
        error!("Defaulting to UTC.");
        UTC
    });

    let peripherals = Peripherals::take()?;
    let pins = peripherals.pins;

    let spi = peripherals.spi2;
    let sclk = pins.gpio0.degrade_output();
    let cs = pins.gpio1.degrade_output();
    let sdo = pins.gpio2.degrade_output();
    let button = PinDriver::input(pins.gpio9.degrade_input(), Pull::Up)?;
    let mypins = MyPins { spi, sclk, sdo, cs };

    let activity_led = gpio::PinDriver::output(pins.gpio8.degrade_output())?;
    let onewire_pin = gpio::PinDriver::input_output_od(pins.gpio10.degrade_input_output(), Pull::Up)?;
    let mut one_wire_bus = OneWire::new(onewire_pin).unwrap();
    let onewire_addr = if ap_mode {
        info!("Sensor is disabled in AP mode.");
        one_wire_bus::Address(0)
    } else if config.sensor_enable {
        info!("Sensor: scanning 1-wire devices...");

        if let Ok(a) = scan_1wire(&mut one_wire_bus) {
            info!("Onewire response: {a:#?}");
            a
        } else {
            one_wire_bus::Address(0)
        }
    } else {
        info!("Sensor is disabled.");
        one_wire_bus::Address(0)
    };

    let state = Box::pin(MyState::new(
        ap_mode,
        config,
        nvs,
        onewire_addr,
        tz,
        ota_slot,
        activity_led,
    ));
    let shared_state = Arc::new(state);

    let wifidriver = WifiDriver::new(peripherals.modem, sysloop.clone(), Some(nvs_default_partition))?;

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(Box::pin(async move {
            shared_state.led_off().await.ok();
            let wifi_loop = WifiLoop {
                state: shared_state.clone(),
                wifi: None,
            };

            info!("Entering main loop...");
            tokio::select! {
                _ = Box::pin(poll_reset(shared_state.clone(), button)) => { error!("poll_reset() ended."); }
                _ = Box::pin(run_clock(shared_state.clone(), mypins)) => { error!("run_clock() ended."); }
                _ = Box::pin(poll_sensor(shared_state.clone(), one_wire_bus)) => { error!("poll_sensor() ended."); }
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
    if state.ap_mode {
        info!("Ping watchdog is disabled in AP mode.");
        loop {
            sleep(Duration::from_secs(3600)).await;
        }
    }

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
                state.led_on().await.ok();
                let res = ping.ping(ping_ip, &conf)?;
                state.led_off().await.ok();
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

async fn poll_reset(mut state: Arc<std::pin::Pin<Box<MyState>>>, button: PinDriver<'_, Input>) -> anyhow::Result<()> {
    loop {
        sleep(Duration::from_millis(BUTTON_POLL_MS)).await;

        if *state.reset.read().await {
            esp_idf_hal::reset::restart();
        }

        if button.is_low() {
            Box::pin(reset_button(&mut state, &button)).await?;
        }
    }
}

async fn reset_button<'a>(
    state: &mut Arc<std::pin::Pin<Box<MyState>>>,
    button: &PinDriver<'a, Input>,
) -> anyhow::Result<()> {
    let mut reset_cnt = CONFIG_RESET_COUNT;
    let mut blink_on = true;
    let mut blink_elapsed_ms = 0;
    let mut countdown_elapsed_ms = 0;

    while button.is_low() {
        if countdown_elapsed_ms == 0 {
            error!("Reset? {reset_cnt}");
            *state.reset_display.write().await = ResetDisplayState::Countdown(reset_cnt);

            if reset_cnt == 0 {
                error!("Factory resetting...");
                *state.reset_display.write().await = ResetDisplayState::FactoryResetting;
                state.led_on().await?;

                {
                    let new_config = MyConfig::default();
                    let mut nvs = state.nvs.write().await;
                    new_config.to_nvs(&mut nvs)?;
                    let _ = nvs.remove(AP_MODE_NVS_KEY)?;
                }
                sleep(Duration::from_secs(5)).await;
                esp_idf_hal::reset::restart();
            }

            reset_cnt -= 1;
        }

        if blink_elapsed_ms == 0 {
            state.set_led(blink_on).await?;
            blink_on = !blink_on;
        }

        sleep(Duration::from_millis(BUTTON_POLL_MS)).await;
        blink_elapsed_ms = (blink_elapsed_ms + BUTTON_POLL_MS) % BUTTON_BLINK_MS;
        countdown_elapsed_ms = (countdown_elapsed_ms + BUTTON_POLL_MS) % BUTTON_COUNTDOWN_STEP_MS;
    }

    *state.reset_display.write().await = ResetDisplayState::None;
    state.led_off().await?;

    if !state.ap_mode {
        info!("Short button press, rebooting into AP mode for manual configuration.");
        state.request_ap_mode_on_next_boot().await?;
        sleep(Duration::from_millis(250)).await;
        esp_idf_hal::reset::restart();
    }

    Ok(())
}
// EOF
