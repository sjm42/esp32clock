# ESP32 Clock

A WiFi-connected ESP32-C3 clock firmware that can drive either:

- an 8-module 8x8 monochrome MAX7219 LED matrix chain, or
- a 64x8 WS2812 RGB matrix made from two chained 8x32 panels

## Features

- runtime configuration is stored on flash, serialized with CRC32 checksum
- default build-time WiFi credentials can be overridden with env variables (`WIFI_SSID`, `WIFI_PASS`)
- WPA2-Personal and WPA2-Enterprise WiFi authentication are supported
- WiFi modem power saving is disabled after association to improve connection stability
- DHCP or static IPv4 configuration with custom DNS servers
- language can be set to Eng/Fin and it affects weekday and month abbreviations on screen
- IANA timezone support (filtered to Europe at build time, configurable via `CHRONO_TZ_TIMEZONE_FILTER`)
- templated web UI for configuration, messaging, firmware OTA updates, and uptime display
- static web assets live under `static/` and are gzip-compressed at build time before being embedded into the firmware
- HTTP API for reading/saving config, sending instant messages, reading uptime, resetting config, and form-based OTA firmware updates
- MQTT support for receiving outdoor temperature, instant messages, and display on/off control
- DS18B20 1-wire temperature sensor support with MQTT publishing
- sunrise/sunset-based automatic day/night LED brightness adjustment
- animated date and temperature displays :grin:
- OTA firmware updates with dual partition slots
- gateway health check with automatic reboot on connectivity loss
- short-press button entry to WiFi AP configuration mode
- long-press button factory reset with LED feedback
- activity LED indication for ping, sensor reads, MQTT receives, and AP mode
- AP mode uses daytime display brightness and a slower marquee for readability
- selectable display backend: MAX7219 or WS2812

## Project layout

- `src/bin/esp32clock.rs` - firmware entrypoint and task orchestration
- `src/*.rs` - runtime modules for clock, display backends, WiFi, MQTT, config, API server, sensor handling, shared state, and font data
- `templates/index.html.ask` - Askama template for the web UI
- `static/` - frontend assets served by the API server (`form.js`, `index.css`, `favicon.ico`)
- `pics/` - hardware/demo images
- `build.rs`, `.cargo/config.toml`, `partitions.csv`, `sdkconfig.defaults` - build and platform configuration

## Build and flash

Toolchain and target are configured in `rust-toolchain.toml` and `.cargo/config.toml`.
The supported target is ESP32-C3 (`riscv32imc-esp-espidf`).

During the build, `build.rs` compresses the files in `static/` and embeds the
gzipped assets into the firmware image.

```bash
# build release firmware
cargo build -r

# build + flash + serial monitor
./flash

# build OTA image (firmware.bin)
./make_ota_image

# build + flash + serial monitor for WS2812
./flash_ws2812

# build WS2812 OTA image (firmware.bin)
./make_ota_image_ws2812

# lint and formatting checks
cargo clippy --all-targets
cargo clippy --all-targets --no-default-features --features esp32c3,ws2812
cargo fmt --check
```

## Cargo features

Exactly one display backend must be enabled at build time.

Default build enables `esp32c3` and `max7219`.

- `max7219` - monochrome MAX7219 8x8 matrix backend
- `ws2812` - RGB WS2812 matrix backend
- `reset_settings` - reset config at boot

Examples:

```bash
# default build: ESP32-C3 + MAX7219
cargo build -r

# WS2812 build
cargo build -r --no-default-features --features esp32c3,ws2812

# WS2812 helper scripts
./flash_ws2812
./make_ota_image_ws2812
```

## Hardware

- ESP32-C3 module by WeAct studio with RISC-V cpu
- the partition table uses two OTA slots of 1984K each, so 4 MB flash is the minimum
- if using a different C3 module with different pinout, the pin config must be adjusted
- purchase link: <https://www.aliexpress.com/item/1005004960064227.html>
- display backend is selected at build time
- MAX7219 wiring in the reference design:
  - GPIO 0 = CLK
  - GPIO 1 = CS
  - GPIO 2 = DIN
- WS2812 wiring in the current reference design:
  - GPIO 7 = data output
  - tested with two chained 8x32 WS2812 panels, mapped as a 64x8 display
  - the current panel mapper assumes the common "8-pixel vertical snake" layout
- GPIO 8 is used for the ESP32-C3 status LED
- GPIO 10 is used for the optional DS18B20 1-wire temperature sensor
- GPIO 9 is used for the setup/reset button
- MAX7219 option:
  - 8 pieces of 8x8 LED matrix displays driven by MAX7219
    - they can be made by soldering two 4-unit modules in chain, or just use one 1x8 readymade module.
    - search for "MAX7219 8x8 dot matrix module" and use either two 4-unit modules or one 8-unit module.
    - Examples: <https://www.aliexpress.com/item/1005006222492232.html>
- WS2812 option:
  - two chained 8x32 RGB WS2812B panels
  - current firmware uses a dim red default color for the clock text
  - one and exactly one of `max7219` or `ws2812` must be selected for any build

## Button and AP mode

- short press stores a one-shot AP-mode boot flag in NVS and reboots into setup mode
- long press counts down to a factory reset, restores default config, and reboots
- while the button is held, the status LED blinks; in AP mode it stays on continuously
- AP mode starts an open WiFi network named `esp32clock` at `10.42.42.1`
- AP mode keeps only the setup UI and display active; MQTT, sensor polling, sensor scanning, and ping watchdog logic are disabled
- AP mode forces `led_intensity_day` and scrolls the IP marquee more slowly for readability

## Sample pictures

![pic1](https://raw.githubusercontent.com/sjm42/esp32clock/master/pics/pic1.png)

![pic2](https://raw.githubusercontent.com/sjm42/esp32clock/master/pics/pic2.png)

![pic3](https://raw.githubusercontent.com/sjm42/esp32clock/master/pics/pic3.png)

![pic4](https://raw.githubusercontent.com/sjm42/esp32clock/master/pics/pic4.png)

## Web UI

A configuration web UI is served at the root URL (`/`). It provides a form for editing
all settings, sending instant messages, and triggering OTA firmware updates.

Recent UI changes:

- the landing page now shows the firmware version, active OTA slot, and live uptime
- forms are submitted asynchronously from `static/form.js` and show inline status messages
- `static/index.css` provides the current responsive layout used by the built-in UI

## Testing and validation

This is an embedded firmware project and the binary uses `harness = false`, so
there is no standard unit-test flow yet.

Recommended validation. The display backends are mutually exclusive, so do not
use `--all-features`; check the default MAX7219 build and WS2812 separately.

- static checks: `cargo clippy --all-targets`,
  `cargo clippy --all-targets --no-default-features --features esp32c3,ws2812`,
  and `cargo fmt --check`
- on-device smoke test via `./flash`:
  - device boots and syncs time
  - short button press reboots into AP mode and serves config UI at `http://10.42.42.1/`
  - long button hold factory-resets config and reboots
  - `/config` GET/POST works
  - MQTT message/display controls work (if enabled)
  - optional DS18B20 reading/publishing works (if enabled)

## Build-time environment variables

These can be set before build to override defaults:

- `WIFI_SSID`, `WIFI_PASS` - default WiFi credentials written when default config is created
- `API_PORT` - default HTTP API port written when default config is created
- `MCU` - MCU selection helper used by ESP build tooling (defaults to `esp32c3`)
- `ESP_IDF_VERSION` - ESP-IDF version used by the build (currently `v5.5.4` in `.cargo/config.toml`)
- `CRATE_CC_NO_DEFAULTS` - set to `1` in `.cargo/config.toml` for ESP C/C++ build flags
- `CHRONO_TZ_TIMEZONE_FILTER` - timezone list filter (default `Europe/.*`)

## API and configuration

Read the current runtime config with a GET request:

```text
curl -so- http://10.6.66.183/config | jq
{
  "port": 80,
  "wifi_ssid": "mywifi",
  "wifi_pass": "mypass",
  "wifi_wpa2ent": false,
  "wifi_username": "",
  "v4dhcp": true,
  "v4addr": "0.0.0.0",
  "v4mask": 0,
  "v4gw": "0.0.0.0",
  "dns1": "0.0.0.0",
  "dns2": "0.0.0.0",
  "mqtt_enable": false,
  "mqtt_url": "mqtt://127.0.0.1:1883",
  "mqtt_topic": "out_temperature",
  "lang": "Eng",
  "tz": "Europe/Helsinki",
  "lat": 61.5,
  "lon": 23.8,
  "sensor_enable": false,
  "sensor_topic": "",
  "led_intensity_day": 4,
  "led_intensity_night": 0,
  "display_shutoff_enable": false
}
```

Write back a modified config with a POST request:

```text
curl -so- -H 'Content-Type: application/json' \
http://10.6.66.183/config -d '{
  "port": 80,
  "wifi_ssid": "mywifi",
  "wifi_pass": "mypass",
  "wifi_wpa2ent": false,
  "wifi_username": "",
  "v4dhcp": true,
  "v4addr": "0.0.0.0",
  "v4mask": 0,
  "v4gw": "0.0.0.0",
  "dns1": "0.0.0.0",
  "dns2": "0.0.0.0",
  "mqtt_enable": true,
  "mqtt_url": "mqtt://10.6.66.1:1883",
  "mqtt_topic": "local_airport_temp",
  "lang": "Eng",
  "tz": "Europe/Helsinki",
  "lat": 61.5,
  "lon": 23.8,
  "sensor_enable": false,
  "sensor_topic": "",
  "led_intensity_day": 4,
  "led_intensity_night": 0,
  "display_shutoff_enable": false
}'
```

Send an instant message:

```text
curl -so- -H 'Content-Type: application/json' \
http://10.6.66.183/msg -d '{"msg": "Hello world!"}'
```

Read device uptime:

```text
curl -so- http://10.6.66.183/uptime | jq
{
  "uptime": 12345
}
```

Reset config to factory defaults:

```text
curl -so- http://10.6.66.183/reset_config
```

Trigger OTA firmware update from a URL. The OTA endpoint accepts form data and
currently requires a plain `http://` URL:

```text
curl -so- -X POST http://10.6.66.183/fw \
  -d 'url=http://myserver/firmware.bin'
```

List supported timezones (filtered to Europe at build time). The endpoint now
returns JSON:

```text
curl -so- http://10.6.66.183/tz | jq '.timezones[:5]'
[
  "Europe/Amsterdam",
  "Europe/Andorra",
  "Europe/Astrakhan",
  "Europe/Athens",
  "Europe/Belfast"
]
```

## MQTT

When MQTT is enabled, the clock subscribes to:

- `esp32clock-all-msg` - broadcast messages to all clocks
- `esp32clock-all-displays` - broadcast display on/off control to all clocks
- `esp32clock-<MAC>` - device-specific topic (MAC address based)
- the configured `mqtt_topic` - for receiving outdoor temperature

Temperature messages are expected as JSON: `{"temperature": 23.5}`

Display on/off messages, honored when `display_shutoff_enable` is true:
`{"state": true}` or `{"state": false}`

If a DS18B20 sensor is enabled, the clock publishes its readings to `sensor_topic` in
the same JSON format.
