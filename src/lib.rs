// lib.rs
#![warn(clippy::large_futures)]
#![feature(round_char_boundary)]

pub use std::{pin::Pin, sync::Arc};

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
