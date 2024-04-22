// clock.rs

use crate::*;

use chrono::*;
use embedded_hal::spi::*;
use esp_idf_svc::sntp;
use tokio::time::{sleep, Duration};

#[cfg(feature = "ws2812")]
use smart_leds::{
    brightness, gamma,
    hsv::{hsv2rgb, Hsv},
    SmartLedsWrite, RGB8,
};
#[cfg(feature = "ws2812")]
use ws2812::Ws2812;
#[cfg(feature = "ws2812")]
use ws2812_spi as ws2812;

const DEFAULT_VSCROLLD: u8 = 20;
const CONFIG_RESET_COUNT: i32 = 9;

const N_LEDS: usize = 8 * 8;
const INTENSITY_NIGHT: u8 = 1;
const INTENSITY_DAY: u8 = 4;
const INTENSITY_BOOST_N: u8 = 3;
const INTENSITY_BOOST_D: u8 = 4;

// #[allow(unused_variables)]
pub async fn run_clock(mut state: Arc<std::pin::Pin<Box<MyState>>>) -> anyhow::Result<()> {
    // set up SPI bus and MAX7219 driver

    let pins = state.pins.write().await.take().unwrap();
    let button = gpio::PinDriver::input(pins.button)?;

    #[cfg(feature = "ws2812")]
    let mut ws = {
        let spi_driver = spi::SpiDriver::new::<spi::SPI2>(
            pins.spi,
            pins.sclk,
            pins.sdo,
            None::<AnyInputPin>,
            &spi::SpiDriverConfig::new(),
        )?;
        let spiconfig = spi::config::Config::new()
            .baudrate(3500.kHz().into())
            .data_mode(spi::config::Mode {
                polarity: spi::config::Polarity::IdleLow,
                phase: spi::config::Phase::CaptureOnFirstTransition,
            })
            .write_only(true);
        let spi_dev = spi::SpiDeviceDriver::new(spi_driver, None::<AnyOutputPin>, &spiconfig)?;
        Ws2812::new(spi_dev)
    };

    #[cfg(feature = "ws2812")]
    let mut data = [RGB8::default(); N_LEDS];
    #[cfg(feature = "ws2812")]
    loop {
        for j in 0..256 {
            for i in 0..N_LEDS {
                // rainbow cycle using HSV, where hue goes through all colors in circle
                // value sets the brightness
                let hsv = Hsv {
                    hue: ((i * 3 + j) % 256) as u8,
                    sat: 255,
                    val: 100,
                };

                data[i] = hsv2rgb(hsv);
            }
            // before writing, apply gamma correction for nicer rainbow
            ws.write(gamma(data.iter().cloned()))?;
            sleep(Duration::from_millis(1000)).await;
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
            let intensity = {
                #[cfg(not(feature = "special"))]
                {
                    INTENSITY_NIGHT
                }
                #[cfg(feature = "special")]
                {
                    if i > 3 {
                        INTENSITY_NIGHT + INTENSITY_BOOST_N
                    } else {
                        INTENSITY_NIGHT
                    }
                }
            };
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

        disp.print(&format!("WiFi ({})", SPIN[cnt % 4]), false);
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
            disp.print(&format!("NTP..({})", SPIN[cnt % 4]), false);
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

    let lang = state.config.read().await.lang.to_owned();
    let tz = state.tz.read().await.to_owned();

    // finally, move to the main clock display loop

    let mut time_vscroll = Some(true);
    loop {
        if time_vscroll.is_none() {
            sleep(Duration::from_millis(100)).await;
        }

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

        let ts = format!("{hour:02}{min:02}:{sec:02}");

        #[cfg(feature = "max7219")]
        if let Some(dir) = time_vscroll {
            for i in 0..8 {
                let intensity = if (0..=7).contains(&hour) {
                    #[cfg(not(feature = "special"))]
                    {
                        INTENSITY_NIGHT
                    }
                    #[cfg(feature = "special")]
                    {
                        if i > 3 {
                            INTENSITY_NIGHT + INTENSITY_BOOST_N
                        } else {
                            INTENSITY_NIGHT
                        }
                    }
                } else {
                    #[cfg(not(feature = "special"))]
                    {
                        INTENSITY_DAY
                    }
                    #[cfg(feature = "special")]
                    {
                        if i > 3 {
                            INTENSITY_DAY + INTENSITY_BOOST_D
                        } else {
                            INTENSITY_DAY
                        }
                    }
                };
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
                if t > NO_TEMP && state.config.read().await.enable_mqtt {
                    if *state.temp_t.read().await < local.timestamp() - 3600 {
                        // Well, MQTT is enabled, we have had earlier temp reading and now it's expired.
                        // Thus, it's better to reboot because we have some kind of a network problem.

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
