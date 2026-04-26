# Repository Guidelines

## Project Structure & Module Organization
- `src/bin/esp32clock.rs`: firmware entrypoint and task orchestration.
- `src/*.rs`: core modules (`clock`, `display`, `wifi`, `mqtt`, `apiserver`, `config`, `state`, `onewire`, `font`, `ws2812`).
- `templates/`: Askama HTML templates for the web UI.
- `static/`: static assets served by the API server (`form.js`, `index.css`, `favicon.ico`).
- `pics/`: hardware/demo images for docs.
- Build and platform config: `Cargo.toml`, `.cargo/config.toml`, `build.rs`, `partitions.csv`, `sdkconfig.defaults`.

## Build, Test, and Development Commands
- `cargo build -r`: build release firmware for the configured ESP target.
- `./flash`: build, flash, and open serial monitor (`cargo run -r -- --baud 921600`).
- `./flash_ws2812`: build, flash, and open serial monitor for the WS2812 backend.
- `./make_ota_image`: build and generate OTA artifact `firmware.bin`.
- `./make_ota_image_ws2812`: build and generate WS2812 OTA artifact `firmware.bin`.
- `cargo clippy --all-targets`: lint the default MAX7219 build.
- `cargo clippy --all-targets --no-default-features --features esp32c3,ws2812`: lint the WS2812 build.
- `cargo fmt`: apply Rust formatting (`rustfmt.toml` enforces width/import grouping).

## Coding Style & Naming Conventions
- Rust 2024 edition; use idiomatic Rust (`snake_case` for functions/variables, `CamelCase` for types, `SCREAMING_SNAKE_CASE` for constants).
- Keep lines readable under the configured `max_width = 120`.
- Prefer small modules with clear responsibility; current hardware target is ESP32-C3 with MAX7219 by default, but the firmware can also target a WS2812 matrix backend.
- Exactly one display backend feature must be enabled: `max7219` or `ws2812`.
- Run `cargo fmt` and both backend-specific `cargo clippy` commands before committing.

## Testing Guidelines
- This repository currently has no unit-test harness (`harness = false` for the binary).
- Treat validation as:
  - static checks: `cargo clippy --all-targets`, `cargo clippy --all-targets --no-default-features --features esp32c3,ws2812`, and `cargo fmt --check`
  - device smoke tests: `./flash` or `./flash_ws2812`, normal WiFi bring-up, short-press AP-mode entry, long-press factory reset, `/config` read/write, MQTT message handling, and display behavior.
- If adding tests later, place module tests near implementation (`mod tests`) and keep test names behavior-focused (for example, `loads_default_config_when_nvs_empty`).

## Commit & Pull Request Guidelines
- Follow the existing history style: short, imperative subjects (for example, `Update esp-idf`, `cargo update`, `Fix MQTT reconnect loop`).
- Keep commits focused (one concern per commit).
- PRs should include:
  - what changed and why
  - hardware/feature flags used for validation
  - manual test evidence (serial logs, API calls, or screenshots for UI changes)
  - linked issue(s) when applicable

## Security & Configuration Tips
- Do not commit real WiFi/MQTT credentials.
- Use environment variables (`WIFI_SSID`, `WIFI_PASS`, `API_PORT`, `MCU`, `ESP_IDF_VERSION`, `CRATE_CC_NO_DEFAULTS`, `CHRONO_TZ_TIMEZONE_FILTER`) for local overrides.
- Keep ESP-IDF version changes deliberate; verify compatibility before upgrading pinned versions.
- Current ESP32-C3 pin mapping:
  - MAX7219 build: GPIO0/1/2 = SPI CLK/CS/DIN
  - WS2812 build: GPIO7 = data output
  - GPIO8 = status LED
  - GPIO9 = setup/reset button
  - GPIO10 = DS18B20
- In AP mode the firmware should keep only setup-relevant behavior active: web UI, AP networking, display status, and explicit reset flows. MQTT, sensor polling, sensor scanning, and ping watchdog logic should remain disabled there.
