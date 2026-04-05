// clock.rs

use esp_idf_svc::sntp;

use crate::*;

const DEFAULT_VSCROLLD: u16 = 20;

struct RunningState {
    lang: MyLang,
    tz: Tz,
    sunrise_t: Option<DateTime<Tz>>,
    sunset_t: Option<DateTime<Tz>>,
    time_vscroll: Option<bool>,
    display_is_turned_off: bool,
}

enum ClockMode {
    WaitWifi { cnt: usize },
    ApMode { ip_info: String },
    WaitNtp { cnt: usize },
    Running(RunningState),
}

// #[allow(unused_variables)]
pub async fn run_clock(state: Arc<std::pin::Pin<Box<MyState>>>, pins: MyPins) -> anyhow::Result<()> {
    #[cfg(feature = "max7219")]
    let mut led_mat = {
        let spi_driver = spi::SpiDriver::new::<spi::SPI2>(
            pins.spi,
            pins.sclk,
            pins.sdo,
            None::<gpio::AnyInputPin<'static>>,
            &spi::SpiDriverConfig::new(),
        )?;
        let spiconfig = spi::config::Config::new().baudrate(10_u32.MHz().into());
        let spi_dev = spi::SpiDeviceDriver::new(spi_driver, Some(pins.cs), &spiconfig)?;
        MAX7219::from_spi(8, spi_dev).unwrap()
    };
    #[cfg(feature = "ws2812")]
    let mut led_mat = LedMatrix::new(pins.sdo)?;

    // set up led matrix display
    led_mat.power_on().ok();
    for i in 0..8 {
        let intensity = state.config.led_intensity_night;
        led_mat.clear_display(i).ok();
        led_mat.set_intensity(i, intensity).ok();
    }
    let mut disp = MyDisplay::new_upside_down();
    let mut mode = ClockMode::WaitWifi { cnt: 0 };
    let mut ntp: Option<sntp::EspSntp<'static>> = None;

    loop {
        if Box::pin(show_reset_status(&state, &mut disp, &mut led_mat)).await {
            sleep(Duration::from_millis(100)).await;
            continue;
        }

        {
            let mut reset = state.reset.write().await;
            if *reset {
                *reset = false;
                esp_idf_hal::reset::restart();
            }
        }

        match &mut mode {
            ClockMode::WaitWifi { cnt } => {
                if *state.wifi_up.read().await {
                    Box::pin(disp.vscroll(DEFAULT_VSCROLLD, false, &mut led_mat, "Connect!")).await;
                    sleep(Duration::from_millis(1000)).await;

                    if state.ap_mode {
                        let ip_info = format!("    Go to url: http://{}/", state.ip_addr.read().await);
                        mode = ClockMode::ApMode { ip_info };
                    } else {
                        let ip_info = format!("My ip: {}", state.ip_addr.read().await);
                        Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut led_mat, &ip_info)).await;
                        sleep(Duration::from_millis(500)).await;
                        Box::pin(disp.marquee(15, &mut led_mat, &ip_info)).await;

                        ntp = Some(sntp::EspSntp::new_default()?);
                        mode = ClockMode::WaitNtp { cnt: 0 };
                    }
                    continue;
                }

                if *cnt > 300 && !state.ap_mode {
                    // we did not get connected in one minute, reset
                    esp_idf_hal::reset::restart();
                }

                disp.print(format!("WiFi ({})", SPIN[*cnt % 4]), false);
                disp.show(&mut led_mat);

                *cnt += 1;
                sleep(Duration::from_millis(200)).await;
            }

            ClockMode::ApMode { ip_info } => {
                for i in 0..8 {
                    led_mat.power_on().ok();
                    led_mat.set_intensity(i, state.config.led_intensity_day).ok();
                }
                Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut led_mat, "AP mode!")).await;
                sleep(Duration::from_millis(1000)).await;
                Box::pin(disp.marquee(25, &mut led_mat, "    Waiting for configuration...")).await;
                sleep(Duration::from_millis(1000)).await;
                Box::pin(disp.marquee(25, &mut led_mat, ip_info)).await;
                sleep(Duration::from_millis(1000)).await;
            }

            ClockMode::WaitNtp { cnt } => {
                if Utc::now().year() > 2020 && ntp.as_ref().unwrap().get_sync_status() == sntp::SyncStatus::Completed {
                    Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut led_mat, "NTP OK! ")).await;
                    sleep(Duration::from_millis(500)).await;

                    mode = ClockMode::Running(build_running_state(&state));
                    continue;
                }

                if *cnt > 300 {
                    // we did not get NTP time in one minute, reset
                    esp_idf_hal::reset::restart();
                }

                disp.print(format!("NTP..({})", SPIN[*cnt % 4]), false);
                disp.show(&mut led_mat);

                *cnt += 1;
                sleep(Duration::from_millis(200)).await;
            }

            ClockMode::Running(running) => {
                if running.time_vscroll.is_none() {
                    sleep(Duration::from_millis(500)).await;
                }

                if !*state.display_enabled.read().await {
                    running.time_vscroll = None;
                    if !running.display_is_turned_off {
                        Box::pin(disp.turn_off(500, &mut led_mat)).await;
                        running.display_is_turned_off = true;
                    }
                    continue;
                }
                running.display_is_turned_off = false;

                let local = Utc::now().with_timezone(&running.tz);
                let sec = local.second();
                let min = local.minute();
                let hour = local.hour();
                let wday_index = local.weekday() as usize;
                let wday_s = match running.lang {
                    MyLang::Eng => WEEKDAY_EN[wday_index],
                    MyLang::Fin => WEEKDAY_FI[wday_index],
                };

                if hour == 4 && min == 42 && (0..10).contains(&sec) {
                    *state.reset.write().await = true;
                }

                let ts = format!(" {hour:02}{min:02}:{sec:02}");

                if let Some(dir) = running.time_vscroll {
                    let daylight = match (&running.sunrise_t, &running.sunset_t) {
                        (Some(sunrise_t), Some(sunset_t)) => local > *sunrise_t && local < *sunset_t,
                        _ => true,
                    };
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

                running.time_vscroll = match sec {
                    11 | 41 => {
                        let mon_index = local.month0() as usize;
                        let mon_s = match running.lang {
                            MyLang::Eng => MONTH_EN[mon_index],
                            MyLang::Fin => MONTH_FI[mon_index],
                        };
                        let day = local.day();
                        let year = local.year();

                        let date_s1 = format!("{wday_s} {day}. ");
                        let date_s2 = format!("{mon_s} {year}  ");
                        Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut led_mat, &date_s1)).await;
                        sleep(Duration::from_millis(1500)).await;

                        Box::pin(disp.vscroll(DEFAULT_VSCROLLD, true, &mut led_mat, &date_s2)).await;
                        sleep(Duration::from_millis(1500)).await;

                        Box::pin(disp.vscroll(DEFAULT_VSCROLLD, false, &mut led_mat, &date_s1)).await;
                        Some(false)
                    }

                    21 | 51 => {
                        let t = *state.temp.read().await;
                        if t > NO_TEMP && state.config.mqtt_enable {
                            if *state.temp_t.read().await < local.timestamp() - 3600 {
                                *state.reset.write().await = true;
                                None
                            } else {
                                let temp_s = format!("{t:+.1}°C");
                                Box::pin(disp.vscroll(DEFAULT_VSCROLLD, false, &mut led_mat, &temp_s)).await;
                                sleep(Duration::from_millis(1500)).await;

                                Some(true)
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                if let Some(msg) = state.msg.write().await.take() {
                    Box::pin(disp.message(DEFAULT_VSCROLLD, &mut led_mat, &msg, &running.lang)).await;
                    running.time_vscroll = Some(true);
                }
            }
        }
    }
}

async fn show_reset_status(
    state: &Arc<std::pin::Pin<Box<MyState>>>,
    disp: &mut MyDisplay,
    led_mat: &mut LedMatrix<'_>,
) -> bool {
    match *state.reset_display.read().await {
        ResetDisplayState::None => false,
        ResetDisplayState::Countdown(reset_cnt) => {
            let msg = format!("Reset {reset_cnt}");
            disp.print(&msg, false);
            disp.show(led_mat);
            true
        }
        ResetDisplayState::FactoryResetting => {
            Box::pin(disp.marquee(25, led_mat, "   FACTORY RESETTING!")).await;
            true
        }
    }
}

fn build_running_state(state: &Arc<std::pin::Pin<Box<MyState>>>) -> RunningState {
    let lang = state.config.lang.to_owned();
    let tz = state.tz.to_owned();

    let (lat, lon) = (state.config.lat, state.config.lon);
    let local_t = Utc::now().with_timezone(&tz);

    let coords = sunrise::Coordinates::new(lat as f64, lon as f64).unwrap();
    let solarday = sunrise::SolarDay::new(coords, local_t.date_naive());
    let sunrise_t = solarday
        .event_time(sunrise::SolarEvent::Sunrise)
        .map(|time| time.with_timezone(&tz));
    let sunset_t = solarday
        .event_time(sunrise::SolarEvent::Sunset)
        .map(|time| time.with_timezone(&tz));

    RunningState {
        lang,
        tz,
        sunrise_t,
        sunset_t,
        time_vscroll: Some(true),
        display_is_turned_off: false,
    }
}
// EOF
