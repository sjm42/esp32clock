// clock.rs

use esp_idf_svc::sntp;

#[cfg(feature = "ws2812")]
use smart_leds::{
    brightness, gamma, hsv::{hsv2rgb, Hsv}, SmartLedsWrite,
    RGB8,
};
#[cfg(feature = "ws2812")]
use smart_leds_trait::SmartLedsWrite;

use crate::*;

const DEFAULT_VSCROLLD: u16 = 20;
const CONFIG_RESET_COUNT: i32 = 9;

// #[allow(unused_variables)]
pub async fn run_clock(mut state: Arc<std::pin::Pin<Box<MyState>>>) -> anyhow::Result<()> {
    // set up SPI bus and MAX7219 driver

    let pins = state.pins.write().await.take().unwrap();
    let button = gpio::PinDriver::input(pins.button)?;

    #[cfg(feature = "ws2812")]
    {
        let mut ws2812 = Ws2812Esp32Rmt::new(pins.rmt, pins.sdo)?;

        let mut data = [RGB8::default(); N_LEDS];

        loop {
            for j in 0..256 {
                for i in 0..N_LEDS {
                    // rainbow cycle using HSV, where hue goes through all colors in circle
                    // value sets the brightness
                    let hsv = Hsv {
                        hue: ((i * 3 + j) % 256) as u8,
                        sat: 255,
                        val: 16,
                    };

                    data[i] = hsv2rgb(hsv);
                }

                // ws2812.write(gamma(data.iter().cloned()))?;

                let pixels = std::iter::repeat(hsv2rgb(Hsv {
                    hue: 128,
                    sat: 255,
                    val: 16,
                }))
                    .take(25);

                // ws2812.write(pixels)?;

                sleep(Duration::from_millis(1000)).await;
            }
        }
    }

    #[cfg(feature = "max7219")]
    let mut led_mat = {
        let spi_driver = spi::SpiDriver::new::<spi::SPI2>(
            pins.spi,
            pins.sclk,
            pins.sdo,
            None::<AnyInputPin>,
            &spi::SpiDriverConfig::new(),
        )?;
        let spiconfig = spi::config::Config::new().baudrate(10.MHz().into());
        let spi_dev = spi::SpiDeviceDriver::new(spi_driver, Some(pins.cs), &spiconfig)?;
        MAX7219::from_spi(8, spi_dev).unwrap()
    };

    // set up led matrix display
    #[cfg(feature = "max7219")]
    {
        led_mat.power_on().ok();
        for i in 0..8 {
            let intensity = state.config.led_intensity_night;
            led_mat.clear_display(i).ok();
            led_mat.set_intensity(i, intensity).ok();
        }
    }
    let mut disp = MyDisplay::new_upside_down();

    // wait for WiFi connection to complete
    let mut cnt = 0;
    loop {
        if *state.wifi_up.read().await {
            break;
        }

        if cnt > 300 {
            // we did not get connected in one minute, reset
            esp_idf_hal::reset::restart();
        }

        disp.print(format!("WiFi ({})", SPIN[cnt % 4]), false);
        #[cfg(feature = "max7219")]
        disp.show(&mut led_mat);

        #[cfg(feature = "max7219")]
        if button.is_low() {
            Box::pin(reset_button(&mut state, &button, &mut led_mat)).await?;
        }

        cnt += 1;
        sleep(Duration::from_millis(200)).await;
    }

    #[cfg(feature = "max7219")]
    {
        Box::pin(disp.vscroll(DEFAULT_VSCROLLD, false, &mut led_mat, "Connect!")).await;
        sleep(Duration::from_millis(1000)).await;
    }

    // show our IP address briefly
    let ip_info = format!("IP: {}", state.ip_addr.read().await);
    #[cfg(feature = "max7219")]
    {
        Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut led_mat, &ip_info)).await;
        sleep(Duration::from_millis(500)).await;
        Box::pin(disp.marquee(15, &mut led_mat, &ip_info)).await;
    }

    // start up NTP
    let ntp = sntp::EspSntp::new_default()?;
    cnt = 0;
    loop {
        if Utc::now().year() > 2020 && ntp.get_sync_status() == sntp::SyncStatus::Completed {
            // we probably have NTP time by now...
            break;
        }

        if cnt > 300 {
            // we did not get NTP time in one minute, reset
            esp_idf_hal::reset::restart();
        }

        #[cfg(feature = "max7219")]
        {
            disp.print(format!("NTP..({})", SPIN[cnt % 4]), false);
            disp.show(&mut led_mat);
        }

        #[cfg(feature = "max7219")]
        if button.is_low() {
            Box::pin(reset_button(&mut state, &button, &mut led_mat)).await?;
        }

        cnt += 1;
        sleep(Duration::from_millis(200)).await;
    }

    #[cfg(feature = "max7219")]
    {
        Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut led_mat, "NTP OK! ")).await;
        sleep(Duration::from_millis(500)).await;
    }

    // set up language and timezone
    let lang = state.config.lang.to_owned();
    let tz = state.tz.to_owned();

    // Only determine local sunrise/sunset times once because we can rely
    // on the fact that we are rebooting every night!
    let (lat, lon) = (state.config.lat, state.config.lon);
    let local_t = Utc::now().with_timezone(&tz);

    let coords = sunrise::Coordinates::new(lat as f64, lon as f64).unwrap();
    let solarday = sunrise::SolarDay::new(coords, local_t.date_naive());
    let sunrise_t = solarday
        .event_time(sunrise::SolarEvent::Sunrise)
        .with_timezone(&tz);
    let sunset_t = solarday
        .event_time(sunrise::SolarEvent::Sunset)
        .with_timezone(&tz);

    // finally, move to the main clock display loop
    let mut time_vscroll = Some(true);
    let mut display_is_turned_off = false;
    loop {
        if time_vscroll.is_none() {
            sleep(Duration::from_millis(500)).await;
        }

        #[cfg(feature = "max7219")]
        if !*state.display_enabled.read().await {
            // our display is disabled, shut it down
            time_vscroll = None;
            if !display_is_turned_off {
                Box::pin(disp.turn_off(500, &mut led_mat)).await;
                display_is_turned_off = true;
            }

            // we short-circuit the loop here until display is turned on again
            continue;
        }
        display_is_turned_off = false;

        {
            // is reset requested?
            let mut reset = state.reset.write().await;
            if *reset {
                *reset = false;
                esp_idf_hal::reset::restart();
            }
        }

        #[cfg(feature = "max7219")]
        if button.is_low() {
            Box::pin(reset_button(&mut state, &button, &mut led_mat)).await?;
        }

        let local = Utc::now().with_timezone(&tz);
        // let ms = ((local.timestamp_subsec_millis() % 1000) / 500) * 5;
        // let sp = SPIN[((local.timestamp_subsec_millis() % 1000) / 250) as usize];
        let sec = local.second();
        let min = local.minute();
        let hour = local.hour();
        let wday_index = local.weekday() as usize;
        let wday_s = match lang {
            MyLang::Eng => WEEKDAY_EN[wday_index],
            MyLang::Fin => WEEKDAY_FI[wday_index],
        };

        // Right after 04:42 local time, we are resetting
        if hour == 4 && min == 42 && (0..10).contains(&sec) {
            *state.reset.write().await = true;
        }

        let ts = format!(" {hour:02}{min:02}:{sec:02}");

        #[cfg(feature = "max7219")]
        if let Some(dir) = time_vscroll {
            // adjust display brightness for time of day

            let daylight = local > sunrise_t && local < sunset_t;
            info!("Daylight: {daylight}");

            for i in 0..8 {
                let intensity = if daylight {
                    state.config.led_intensity_day
                } else {
                    state.config.led_intensity_night
                };
                led_mat.power_on().ok();
                led_mat.set_intensity(i, intensity).ok();
            }

            Box::pin(disp.vscroll(DEFAULT_VSCROLLD, dir, &mut led_mat, &ts)).await;
        } else {
            disp.print(&ts, false);
            disp.show(&mut led_mat);
        }

        time_vscroll = match sec {
            11 | 41 => {
                // show date

                let mon_index = local.month0() as usize;
                let mon_s = match lang {
                    MyLang::Eng => MONTH_EN[mon_index],
                    MyLang::Fin => MONTH_FI[mon_index],
                };
                let day = local.day();
                // let year = local.year() - 2000;
                let year = local.year();

                let date_s1 = format!("{wday_s} {day}. ");
                let date_s2 = format!("{mon_s} {year}  ");
                #[cfg(feature = "max7219")]
                {
                    Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut led_mat, &date_s1)).await;
                    sleep(Duration::from_millis(1500)).await;

                    Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut led_mat, &date_s2)).await;
                    sleep(Duration::from_millis(1500)).await;

                    Box::pin(disp.vscroll(DEFAULT_VSCROLLD, false, &mut led_mat, &date_s1)).await;
                }
                Some(false)
            }

            21 | 51 => {
                // show temperature?
                let t = *state.temp.read().await;
                if t > NO_TEMP && state.config.mqtt_enable {
                    if *state.temp_t.read().await < local.timestamp() - 3600 {
                        // Well, MQTT is enabled, we have had earlier temp reading, and now it's expired.
                        // Thus, it's better to reboot because we have some kind of network problem.

                        *state.reset.write().await = true;
                        None
                    } else {
                        // OK we have the temp reading, show it.

                        #[cfg(feature = "max7219")]
                        {
                            let temp_s = format!("{t:+.1}°C");
                            Box::pin(disp.vscroll(DEFAULT_VSCROLLD, false, &mut led_mat, &temp_s))
                                .await;
                            sleep(Duration::from_millis(1500)).await;
                        }

                        Some(true)
                    }
                } else {
                    None
                }
            }
            _ => None,
        };

        // Whoa, we have an incoming message to display!
        #[cfg(feature = "max7219")]
        if let Some(msg) = state.msg.write().await.take() {
            Box::pin(disp.message(DEFAULT_VSCROLLD, &mut led_mat, &msg, &lang)).await;
            time_vscroll = Some(true);
        }
    }
}

#[cfg(feature = "max7219")]
async fn reset_button<'a, 'b>(
    state: &mut Arc<std::pin::Pin<Box<MyState>>>,
    button: &PinDriver<'a, AnyInputPin, Input>,
    led_mat: &mut MAX7219<SpiConnector<SpiDeviceDriver<'b, spi::SpiDriver<'b>>>>,
) -> anyhow::Result<()> {
    let mut reset_cnt = CONFIG_RESET_COUNT;
    let mut disp = MyDisplay::new_upside_down();

    while button.is_low() {
        // button is pressed and kept down, countdown and factory reset if reach zero
        let msg = format!("Reset? {reset_cnt}");
        error!("{msg}");
        disp.print(&msg, false);
        disp.show(led_mat);

        if reset_cnt == 0 {
            // okay do factory reset now
            error!("Factory resetting...");
            disp.print("Reset...", false);
            disp.show(led_mat);

            let new_config = MyConfig::default();
            new_config.to_nvs(&mut *state.nvs.write().await)?;
            sleep(Duration::from_millis(2000)).await;
            esp_idf_hal::reset::restart();
        }

        reset_cnt -= 1;
        sleep(Duration::from_millis(500)).await;
        continue;
    }
    Ok(())
}
// EOF
