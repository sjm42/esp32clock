// display.rs

use crate::*;

use bit_reverse::LookupReverse;
use encoding_rs::*;
use esp_idf_hal::spi::*;
use max7219::{connectors::SpiConnector, MAX7219};
use std::borrow::Cow;
use tokio::time::{sleep, Duration};

const ELEMS: usize = 8;
const MAX_TEXT_SIZE: usize = 256;

type LedMatrix<'a> = MAX7219<SpiConnector<SpiDeviceDriver<'a, SpiDriver<'a>>>>;

pub struct MyDisplay {
    pub fbuf: [[u8; 8]; ELEMS],
    pub upside_down: bool,
}

impl<'a> MyDisplay {
    pub fn new(upside_down: bool) -> Self {
        Self {
            fbuf: [[0; 8]; ELEMS],
            upside_down,
        }
    }

    pub fn new_upright() -> Self {
        Self::new(false)
    }

    pub fn new_upside_down() -> Self {
        Self::new(true)
    }

    pub fn print<S>(&mut self, s: S)
    where
        S: AsRef<str>,
    {
        let c_count = s.as_ref().chars().count();
        let msg = if c_count <= ELEMS {
            s.as_ref()
        } else {
            let cut = s.as_ref().floor_char_boundary(ELEMS);
            let (s1, _s2) = s.as_ref().split_at(cut);
            s1
        };

        let (buf, _c, _e) = ISO_8859_15.encode(msg);

        self.clear();
        for (d, c) in buf.iter().enumerate().take(ELEMS) {
            let offset = (*c as usize) * 8;

            (0..8).for_each(|r| {
                self.fbuf[d][r] = FONT[offset + r];
            })
        }
    }

    pub fn show_buf(&self, buf: &[[u8; 8]], led_mat: &mut LedMatrix) {
        if buf.len() != ELEMS {
            // our slice size does not match!
            return;
        }

        if self.upside_down {
            // Our display is rotated 180 degrees!
            // Thus, we have to turn everything around.
            let mut revbuf = [[0u8; 8]; ELEMS];
            (0..ELEMS).for_each(|d| {
                (0..8).for_each(|r| {
                    revbuf[ELEMS - 1 - d][7 - r] = buf[d][r].swap_bits();
                });
            });

            (0..ELEMS).for_each(|d| {
                led_mat.write_raw(d, &revbuf[d]).ok();
            });
        } else {
            (0..ELEMS).for_each(|d| {
                led_mat.write_raw(d, &buf[d]).ok();
            });
        }
    }

    pub fn clear(&mut self) {
        (0..ELEMS).for_each(|d| (0..8).for_each(|r| self.fbuf[d][r] = 0));
    }

    pub fn show(&self, led_mat: &mut LedMatrix) {
        self.show_buf(&self.fbuf, led_mat);
    }

    pub async fn marquee<S>(&mut self, delay: u8, led_mat: &mut LedMatrix<'_>, s: S)
    where
        S: AsRef<str>,
    {
        let delay = std::cmp::max(1, delay as u64);

        let c_count = s.as_ref().chars().count();
        let msg = if c_count < MAX_TEXT_SIZE {
            s.as_ref()
        } else {
            let cut = s.as_ref().floor_char_boundary(MAX_TEXT_SIZE);
            let (s1, _s2) = s.as_ref().split_at(cut);
            s1
        };

        // We render a large enough framebuffer for the text
        let (mut buf, _c, _e) = ISO_8859_15.encode(msg);

        // pad buf to have at least ELEMS items
        if buf.len() < ELEMS {
            let mut s = buf.to_vec();
            (0..(ELEMS - buf.len())).for_each(|_| s.push(b' '));
            buf = Cow::Owned(s);
        }

        let dlen = buf.len();
        let mut dbuf = Vec::with_capacity(dlen);
        for (d, c) in buf.iter().enumerate() {
            let offset = (*c as usize) * 8;
            let rv = [0u8; 8];
            dbuf.push(rv);
            (0..8).for_each(|r| {
                dbuf[d][r] = FONT[offset + r];
            })
        }

        for _ in 0..dlen * 8 {
            for d in 0..dlen {
                for r in 0..8 {
                    dbuf[d][r] <<= 1;
                    if d < dlen - 1 && dbuf[d + 1][r] & 0x80 != 0 {
                        dbuf[d][r] |= 1;
                    }
                }
            }

            self.show_buf(&dbuf[0..ELEMS], led_mat);
            sleep(Duration::from_millis(delay)).await;
        }

        // cleanup
        self.clear();
        self.show(led_mat);
    }

    pub async fn drop<S>(&mut self, delay: u8, led_mat: &mut LedMatrix<'_>, s: S)
    where
        S: AsRef<str>,
    {
        let delay = std::cmp::max(1, delay as u64);

        let c_count = s.as_ref().chars().count();
        let msg = if c_count < ELEMS {
            s.as_ref()
        } else {
            let cut = s.as_ref().floor_char_boundary(ELEMS);
            let (s1, _s2) = s.as_ref().split_at(cut);
            s1
        };

        // We render a large enough framebuffer for the text
        let (mut buf, _c, _e) = ISO_8859_15.encode(msg);

        // pad buf to have at least ELEMS items
        if buf.len() < ELEMS {
            let mut s = buf.to_vec();
            (0..(ELEMS - buf.len())).for_each(|_| s.push(b' '));
            buf = Cow::Owned(s);
        }

        // render our new text to dbuf
        let mut dbuf = [[0u8; 8]; ELEMS];
        for (d, c) in buf.iter().enumerate() {
            let offset = (*c as usize) * 8;
            (0..8).for_each(|r| {
                dbuf[d][r] = FONT[offset + r];
            })
        }

        for p in 0..8 {
            for (d, c) in dbuf.iter().enumerate().take(ELEMS) {
                for r in (1..8).rev() {
                    self.fbuf[d][r] = self.fbuf[d][r - 1];
                }
                self.fbuf[d][0] = c[7 - p];
            }

            self.show(led_mat);
            sleep(Duration::from_millis(delay)).await;
        }
    }
}

impl Default for MyDisplay {
    fn default() -> Self {
        Self::new(false)
    }
}
// EOF
