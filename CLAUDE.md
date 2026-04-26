# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

ESP32-C3-based clock firmware written in embedded Rust. The firmware can drive either an 8-module MAX7219 8x8 matrix chain or a WS2812 64x8 RGB matrix, and includes WiFi, web UI, MQTT, DS18B20 temperature sensor, AP setup mode, status LED feedback, and OTA firmware updates.

## Build Commands

```bash
# Build release firmware
cargo build -r

# Build and flash to device with serial monitor
./flash                  # equivalent to: cargo run -r -- --baud 921600
./flash_ws2812           # WS2812 backend

# Build firmware.bin for OTA deployment
./make_ota_image
./make_ota_image_ws2812  # WS2812 backend

# Check/lint
cargo clippy --all-targets
cargo clippy --all-targets --no-default-features --features esp32c3,ws2812

# Format
cargo fmt
```

There are no unit tests (`harness = false` in Cargo.toml) — this is an embedded firmware project.

## Build Environment

- **Toolchain**: Nightly Rust with `rust-src` component (see `rust-toolchain.toml`)
- **Target**: `riscv32imc-esp-espidf` (ESP32-C3 RISC-V)
- **ESP-IDF version**: v5.5.4 (pinned in `.cargo/config.toml` — do not update without verifying esp-idf-svc compatibility)
- **Environment variables**: `WIFI_SSID`, `WIFI_PASS`, `API_PORT`, `MCU`, `ESP_IDF_VERSION`, `CRATE_CC_NO_DEFAULTS`, and `CHRONO_TZ_TIMEZONE_FILTER` can be set for local overrides
- **Timezone filter**: `CHRONO_TZ_TIMEZONE_FILTER=Europe/.*` set in `.cargo/config.toml`
- **C/C++ build flags**: `CRATE_CC_NO_DEFAULTS=1` set in `.cargo/config.toml`
- Uses `build-std = ["std", "panic_abort"]` — builds the standard library from source

## Code Style

- `rustfmt.toml`: max_width=120, imports grouped by std/external/crate
- `clippy.toml`: future-size-threshold=128 (warns on large futures via `#![warn(clippy::large_futures)]`)

## Architecture

### Concurrency Model

The firmware runs on Tokio async runtime. The main binary (`src/bin/esp32clock.rs`) launches concurrent tasks via `tokio::select!`:

1. **poll_reset()** — setup/factory-reset button handling and reboot requests
2. **run_clock()** — main display loop: NTP time, date/temp animations, AP-mode status display
3. **poll_sensor()** — DS18B20 temperature polling (60s interval, disabled in AP mode)
4. **run_mqtt()** — MQTT client for messages, temperature, display control (disabled in AP mode)
5. **run_api_server()** — Axum HTTP server (web UI + JSON API)
6. **WifiLoop::run()** — WiFi STA or AP-mode network setup
7. **pinger()** — gateway health check (5 min interval, disabled in AP mode)

### Shared State

`MyState` (in `src/state.rs`) is the central shared state, wrapped in `Arc<Pin<Box<...>>>` and protected by `tokio::sync` primitives. All async tasks share this state for config, AP-mode flag, network status, temperature data, display control, NVS access, and the activity LED driver.

### Module Responsibilities

| Module | Purpose |
|---|---|
| `config.rs` | `MyConfig` struct with serde serialization; stored in NVS via postcard+CRC32 |
| `display.rs` | `MyDisplay` — shared logical 8-character display/framebuffer, ISO-8859-15 encoding, rotation support |
| `clock.rs` | Main display loop: time/date/temp rendering, sunrise/sunset brightness, AP-mode status display |
| `apiserver.rs` | Axum routes: GET/POST `/config`, POST `/msg`, GET `/uptime`, GET `/reset_config`, POST `/fw` (OTA), GET `/tz`, and embedded static assets |
| `mqtt.rs` | MQTT subscribe/publish: temperature JSON, display control, messages; disabled in AP mode |
| `wifi.rs` | `WifiLoop` — async WiFi driver with DHCP/static IP, WPA2-Personal/Enterprise, or AP mode |
| `onewire.rs` | DS18B20 1-wire sensor: 12-bit reads with CRC verification and retries; disabled in AP mode |
| `font.rs` | Embedded 36KB font lookup table for the LED matrix |
| `ws2812.rs` | WS2812 RMT backend and panel pixel mapper for the RGB display option |
| `lib.rs` | Re-exports, shared types (`Temperature`, `MyMessage`, `DisplayEnabled`), constants |

### Configuration Flow

`MyConfig` is loaded from ESP32 NVS (non-volatile storage) at startup. It's serialized with postcard and verified with CRC32. The web UI and JSON API (`/config`) allow runtime changes which are persisted back to NVS.

### Cargo Features

- `default = ["esp32c3", "max7219"]` — standard build
- `max7219` — MAX7219 monochrome display backend
- `ws2812` — WS2812 RGB display backend
- `reset_settings` — factory reset on boot
- exactly one display backend must be enabled; `lib.rs` enforces this with `compile_error!`
- do not use `--all-features`; it enables both display backends and intentionally fails

### Key Hardware Pins (ESP32-C3)

- MAX7219 build: GPIO 0/1/2 = SPI CLK/CS/DIN for the display chain
- WS2812 build: GPIO 7 = data output for the RGB panel chain
- GPIO 8: status LED (active low)
- GPIO 9: setup/factory-reset button
- GPIO 10: DS18B20 1-wire temperature sensor

### Display Backends

- `max7219` is the default backend and targets the classic 8-module 8x8 monochrome chain.
- `ws2812` targets two chained 8x32 WS2812 panels presented as one 64x8 display.
- The current WS2812 panel mapper assumes the common 8-pixel vertical snake layout.
- The WS2812 backend currently renders the clock text in dim red by default.

### AP Mode

- Entered via short button press; the request is stored as a one-shot NVS boot flag
- Starts an open AP named `esp32clock` on `10.42.42.1/24`
- Keeps the web UI and display active, forces daytime display brightness, and slows the AP-mode marquee for readability
- Disables MQTT, sensor scanning/polling, and ping watchdog logic
- LED stays on continuously in AP mode

### LED Behavior

- LED blinks while the button is held
- LED stays on after factory reset trigger until reboot
- LED lights during ping, successful DS18B20 reads, and inbound MQTT messages
- LED stays on continuously in AP mode

### Error Recovery Strategy

Outside AP mode, the firmware reboots on: WiFi connection failure at startup, NTP sync timeout (1 min), stale MQTT temperature data (>1 hour when MQTT temp display is enabled), gateway ping failure, daily at 04:42 local time for stability, or explicit reset/factory-reset requests. In AP mode, the normal network watchdog paths are disabled and reboots should only come from explicit reset/factory-reset flows or unexpected top-level task exit.

### Templates

HTML templates use Askama (type-safe Rust templates) in `templates/`. Frontend JS and CSS live under `static/` and are served as embedded compressed assets.

### Flash Partitions

Dual OTA partition scheme (`partitions.csv`): two 1984K app slots for seamless OTA updates, plus NVS, OTA metadata, and PHY init partitions. Minimum 4MB flash required.
