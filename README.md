# esp32clock

Make a nice clock with ESP32 and MAX7219 8x8x4 led matrix displays

## Features

- while source code has default WiFi credentials, they can be overridden with env variables
- runtime configuration including WiFi credentials is stored on flash, serialized and with crc32 checksum
- static ipv4 configuration is supported
- language can be set to Eng/Fin and it affects weekday and month abbreviations on screen
- all known timezones are supported
- HTTP JSON API is provided for reading and saving config, and sending instant messages
- supported timezones can be listed with an API call
- MQTT is supported for getting (outdoors) temperature reading and for IM

## Hardware

- ESP32-C3 module by WeAct studio is recommended, but the firmware should work on almost any ESP32 supporting WiFi
- if using a different module with different pinout and/or cpu, the pin config and build parameters must be adjusted
- purchase link: <https://www.aliexpress.com/item/1005004960064227.html>
- in the "reference" design, ESP32-C3 is soldered on the CLK/CS/DIN pins of display module, corresponding to GPIO 0/1/2 pins.
- 8 pieces of 8x8 LED matrix displays driven by MAX7219 is used:

- they can be made by soldering two 4-unit modules in chain, or just use one 1x8 ready made module.
- search for "MAX7219 8x8 dot matrix module" and use either two 4-unit modules or one 8-unit module.
- Examples: <https://www.aliexpress.com/item/1005006222492232.html>

## Sample pictures

![pic1](https://raw.githubusercontent.com/sjm42/esp32clock/master/pics/pic1.png)

![pic2](https://raw.githubusercontent.com/sjm42/esp32clock/master/pics/pic2.png)

![pic3](https://raw.githubusercontent.com/sjm42/esp32clock/master/pics/pic3.png)

![pic4](https://raw.githubusercontent.com/sjm42/esp32clock/master/pics/pic4.png)

## API and configuration

Read the current runtime config with a GET request:

```text
curl -so- http://10.6.66.183/conf |jq
{
  "bfc": 0,
  "port": 80,
  "wifi_ssid": "mywifi",
  "wifi_pass": "mypass",
  "v4dhcp": true,
  "v4addr": "0.0.0.0",
  "v4mask": 0,
  "v4gw": "0.0.0.0",
  "enable_mqtt": false,
  "mqtt_url": "mqtt://mqtt.local:1883",
  "temp_topic": "outdoor_temperature",
  "lang": "Fin",
  "tz": "Europe/Helsinki"
}
```

Write back a modified config with a POST request:

```text
curl -so- -H 'Content-Type: application/json' \
http://10.6.66.183/conf -d \
"{\"bfc\":0,\"port\":80,\
\"wifi_ssid\":\"mywifi\",\"wifi_pass\":\"mypass\",\
\"v4dhcp\":true,\"v4addr\":\"0.0.0.0\",\
\"v4mask\":0,\"v4gw\":\"0.0.0.0\",\
\"enable_mqtt\":true,\
\"mqtt_url\":\"mqtt://10.6.66.1:1883\",\
\"temp_topic\":\"local_airport_temp\",\
\"lang\":\"Eng\",\"tz\":\"Europe/Helsinki\"}"

```

List supported timezones (the whole list is 500+ lines!):

```text
curl -so- http://10.28.5.182/tz | grep Europe
Europe/Amsterdam
Europe/Andorra
Europe/Astrakhan
Europe/Athens
Europe/Belfast
Europe/Belgrade
Europe/Berlin
Europe/Bratislava
Europe/Brussels
Europe/Bucharest
Europe/Budapest
Europe/Busingen
Europe/Chisinau
Europe/Copenhagen
Europe/Dublin
Europe/Gibraltar
Europe/Guernsey
Europe/Helsinki
Europe/Isle_of_Man
Europe/Istanbul
Europe/Jersey
Europe/Kaliningrad
Europe/Kiev
Europe/Kirov
Europe/Kyiv
Europe/Lisbon
Europe/Ljubljana
Europe/London
Europe/Luxembourg
Europe/Madrid
Europe/Malta
Europe/Mariehamn
Europe/Minsk
Europe/Monaco
Europe/Moscow
Europe/Nicosia
Europe/Oslo
Europe/Paris
Europe/Podgorica
Europe/Prague
Europe/Riga
Europe/Rome
Europe/Samara
Europe/San_Marino
Europe/Sarajevo
Europe/Saratov
Europe/Simferopol
Europe/Skopje
Europe/Sofia
Europe/Stockholm
Europe/Tallinn
Europe/Tirane
Europe/Tiraspol
Europe/Ulyanovsk
Europe/Uzhgorod
Europe/Vaduz
Europe/Vatican
Europe/Vienna
Europe/Vilnius
Europe/Volgograd
Europe/Warsaw
Europe/Zagreb
Europe/Zaporozhye
Europe/Zurich

```
