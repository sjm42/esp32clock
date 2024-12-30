// lib.rs

#![feature(round_char_boundary)]
#![warn(clippy::large_futures)]

// #![allow(unused_imports)]
// #![allow(unused_variables)]
// #![allow(unused_mut)]
// #![allow(dead_code)]

pub use std::{
    fmt, net,
    net::{Ipv4Addr, SocketAddr},
};
pub use std::{pin::Pin, sync::Arc};

#[cfg(feature = "max7219")]
use max7219::{connectors::SpiConnector, MAX7219};

pub use anyhow::bail;
pub use chrono::*;
use chrono_tz::{Tz, TZ_VARIANTS};
pub use esp_idf_hal::{
    gpio::{self, *},
    prelude::*,
    spi,
};
pub use esp_idf_svc::hal::spi::SpiDeviceDriver;
pub use serde::Deserialize;
pub use tokio::sync::RwLock;
pub use tokio::time::{sleep, Duration};
pub use tracing::*;

pub use apiserver::*;
pub use clock::*;
pub use config::*;
pub use display::*;
pub use font::*;
pub use mqtt::*;
pub use onewire::*;
pub use state::*;
pub use wifi::*;

#[cfg(feature = "max7219")]
pub type LedMatrix<'a> = MAX7219<SpiConnector<SpiDeviceDriver<'a, spi::SpiDriver<'a>>>>;

#[derive(Debug, Deserialize)]
pub struct Temperature {
    temperature: f32,
}

#[derive(Debug, Deserialize)]
pub struct MyMessage {
    msg: String,
}

pub const FW_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const SPIN: [char; 4] = ['|', '/', '-', '\\'];

pub const WEEKDAY_EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
pub const WEEKDAY_FI: [&str; 7] = ["Ma", "Ti", "Ke", "To", "Pe", "La", "Su"];

#[rustfmt::skip]
pub const MONTH_EN: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun",
    "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[rustfmt::skip]
pub const MONTH_FI: [&str; 12] = [
    "Tam", "Hel", "Maa", "Huh", "Tou", "Kes",
    "Hei", "Elo", "Syy", "Lok", "Mar", "Jou",
];

/*
#[rustfmt::skip]
pub const MONTH_FI: [&str; 12] = [
    "Tammi", "Helmi", "Maals", "Huhti", "Touko", "Kesä",
    "Heinä", "Elo", "Syys", "Loka", "Marrs", "Joulu",
];
*/

pub const NO_TEMP: f32 = -1000.0;

mod apiserver;
mod clock;
mod config;
mod display;
mod font;
mod mqtt;
mod onewire;
mod state;
mod wifi;

// mod ws2812;
// pub use ws2812::*;

// EOF
