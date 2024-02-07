// lib.rs
#![warn(clippy::large_futures)]
#![feature(round_char_boundary)]

pub use serde::Deserialize;
pub use std::{pin::Pin, sync::Arc};

#[derive(Debug, Deserialize)]
pub struct Temperature {
    temperature: f32,
}

#[derive(Debug, Deserialize)]
pub struct MyMessage {
    msg: String,
}

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
    "Tammi", "Helmi", "Maals", "Huhti", "Touko", "Kesä",
    "Heinä", "Elo", "Syys", "Loka", "Marrs", "Joulu",
];

mod config;
pub use config::*;

mod state;
pub use state::*;

mod font;
pub use font::*;

mod apiserver;
pub use apiserver::*;

mod clock;
pub use clock::*;

mod display;
pub use display::*;

mod mqtt;
pub use mqtt::*;

mod wifi;
pub use wifi::*;

// EOF
