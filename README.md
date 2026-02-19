# ESP32 Clock

A clock with ESP32 and MAX7219 8x8 led matrix displays

## Features

- runtime configuration is stored on flash, serialized with CRC32 checksum
- default build-time WiFi credentials can be overridden with env variables (`WIFI_SSID`, `WIFI_PASS`)
- WPA2-Personal and WPA2-Enterprise WiFi authentication are supported
- DHCP or static IPv4 configuration with custom DNS servers
- language can be set to Eng/Fin and it affects weekday and month abbreviations on screen
- IANA timezone support (filtered to Europe at build time, configurable via `CHRONO_TZ_TIMEZONE_FILTER`)
- templated web UI for configuration, messaging, and firmware OTA updates
- HTTP JSON API for reading/saving config, sending instant messages, and OTA firmware updates
- MQTT support for receiving outdoor temperature, instant messages, and display on/off control
- DS18B20 1-wire temperature sensor support with MQTT publishing
- sunrise/sunset-based automatic day/night LED brightness adjustment
- animated date and temperature displays :grin:
- OTA firmware updates with dual partition slots
- gateway health check with automatic reboot on connectivity loss
- hardware button for factory reset

## Build and flash

Toolchain and target are configured in `rust-toolchain.toml` and `.cargo/config.toml`.
Default target is ESP32-C3 (`riscv32imc-esp-espidf`). For ESP32, switch to the
commented `xtensa-esp32-espidf` target in `.cargo/config.toml`.

```bash
# build release firmware
cargo build -r

# build + flash + serial monitor
./flash

# build OTA image (firmware.bin)
./makeimage

# lint and formatting checks
cargo clippy --all-targets --all-features
cargo fmt --check
```

## Cargo features

Default build enables `esp32c3` and `max7219`.

- `esp32s` - ESP32-S variant support
- `ws2812` - WS2812 LED support (alternative display backend) (NOT WORKING YET)
- `reset_settings` - reset config at boot

Examples:

```bash
# default features
cargo build -r

# ws2812 build
cargo build -r --no-default-features --features esp32c3,ws2812
```

## Hardware

- ESP32-C3 module by WeAct studio with RISC-V cpu is the default target; ESP32
  (Xtensa) is also supported by switching target/build settings
- the partition table uses two OTA slots of ~2 MB each, so 4 MB flash is the minimum
- if using a different module with different pinout and/or cpu type, the pin config and build parameters must be
  adjusted
- purchase link: <https://www.aliexpress.com/item/1005004960064227.html>
- in the "reference" design, ESP32-C3 is soldered on the CLK/CS/DIN pins of display module, corresponding to GPIO 0/1/2
  pins
- GPIO 8 is used for the optional DS18B20 1-wire temperature sensor
- GPIO 9 is used for the factory reset button
- 8 pieces of 8x8 LED matrix displays driven by MAX7219 is used:
    - they can be made by soldering two 4-unit modules in chain, or just use one 1x8 readymade module.
    - search for "MAX7219 8x8 dot matrix module" and use either two 4-unit modules or one 8-unit module.
    - Examples: <https://www.aliexpress.com/item/1005006222492232.html>

## Sample pictures

![pic1](https://raw.githubusercontent.com/sjm42/esp32clock/master/pics/pic1.png)

![pic2](https://raw.githubusercontent.com/sjm42/esp32clock/master/pics/pic2.png)

![pic3](https://raw.githubusercontent.com/sjm42/esp32clock/master/pics/pic3.png)

![pic4](https://raw.githubusercontent.com/sjm42/esp32clock/master/pics/pic4.png)

## Web UI

A configuration web UI is served at the root URL (`/`). It provides a form for editing
all settings, sending instant messages, and triggering OTA firmware updates.

## Testing and validation

This is an embedded firmware project and the binary uses `harness = false`, so
there is no standard unit-test flow yet.

Recommended validation:

- static checks: `cargo clippy --all-targets --all-features` and `cargo fmt --check`
- on-device smoke test via `./flash`:
  - device boots and syncs time
  - `/config` GET/POST works
  - MQTT message/display controls work (if enabled)
  - optional DS18B20 reading/publishing works (if enabled)

## Build-time environment variables

These can be set before build to override defaults:

- `WIFI_SSID`, `WIFI_PASS` - default WiFi credentials
- `API_PORT` - default HTTP API port
- `MCU` - MCU selection helper used by ESP build tooling
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

Reset config to factory defaults:

```text
curl -so- http://10.6.66.183/reset_config
```

Trigger OTA firmware update from a URL:

```text
curl -so- -X POST http://10.6.66.183/fw -d 'url=http://myserver/firmware.bin'
```

List supported timezones (filtered to Europe at build time):

```text
curl -so- http://10.6.66.183/tz | grep Europe
Europe/Amsterdam
Europe/Andorra
Europe/Astrakhan
Europe/Athens
Europe/Belfast
Europe/Belgrade
Europe/Berlin
Europe/Bratislava
Europe/Brussels
... etc.
```

## MQTT

When MQTT is enabled, the clock subscribes to:

- `esp32clock-all-msg` - broadcast messages to all clocks
- `esp32clock-all-displays` - broadcast display on/off control to all clocks
- `esp32clock-<MAC>` - device-specific topic (MAC address based)
- the configured `mqtt_topic` - for receiving outdoor temperature

Temperature messages are expected as JSON: `{"temperature": 23.5}`

Display on/off messages: `{"state": true}` or `{"state": false}`

If a DS18B20 sensor is enabled, the clock publishes its readings to `sensor_topic` in
the same JSON format.
