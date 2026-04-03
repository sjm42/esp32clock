# Repository Guidelines

## Project Structure & Module Organization
- `src/bin/esp32clock.rs`: firmware entrypoint and task orchestration.
- `src/*.rs`: core modules (`clock`, `display`, `wifi`, `mqtt`, `apiserver`, `config`, `state`, `onewire`).
- `templates/`: Askama HTML templates for the web UI.
- `static/`: static assets served by the API server (`form.js`, `index.css`, `favicon.ico`).
- `pics/`: hardware/demo images for docs.
- Build and platform config: `Cargo.toml`, `.cargo/config.toml`, `build.rs`, `partitions.csv`, `sdkconfig.defaults`.

## Build, Test, and Development Commands
- `cargo build -r`: build release firmware for the configured ESP target.
- `./flash`: build, flash, and open serial monitor (`cargo run -r -- --baud 921600`).
- `./makeimage`: build and generate OTA artifact `firmware.bin`.
- `cargo clippy --all-targets --all-features`: lint code before opening a PR.
- `cargo fmt`: apply Rust formatting (`rustfmt.toml` enforces width/import grouping).

## Coding Style & Naming Conventions
- Rust 2024 edition; use idiomatic Rust (`snake_case` for functions/variables, `CamelCase` for types, `SCREAMING_SNAKE_CASE` for constants).
- Keep lines readable under the configured `max_width = 120`.
- Prefer small modules with clear responsibility; current hardware target is ESP32-C3 with MAX7219 by default. The `ws2812` feature exists as a placeholder and is not implemented.
- Run `cargo fmt` and `cargo clippy` before committing.

## Testing Guidelines
- This repository currently has no unit-test harness (`harness = false` for the binary).
- Treat validation as:
  - static checks: `cargo clippy` + `cargo fmt --check`
  - device smoke tests: `./flash`, normal WiFi bring-up, short-press AP-mode entry, long-press factory reset, `/config` read/write, MQTT message handling, and display behavior.
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
- Use environment variables (`WIFI_SSID`, `WIFI_PASS`, `API_PORT`, `MCU`) for local overrides.
- Keep ESP-IDF version changes deliberate; verify compatibility before upgrading pinned versions.
- Current ESP32-C3 pin mapping: GPIO0/1/2 = MAX7219 SPI, GPIO8 = status LED, GPIO9 = setup/reset button, GPIO10 = DS18B20.
- In AP mode the firmware should keep only setup-relevant behavior active: web UI, AP networking, display status, and explicit reset flows. MQTT, sensor polling, sensor scanning, and ping watchdog logic should remain disabled there.
