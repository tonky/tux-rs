//! ITE 829x per-key RGB controller (6×20 matrix, 120 LEDs).
//!
//! Protocol: 6-byte SET_FEATURE reports with report ID 0xCC.
//! Uses `keyb_send_data(cmd, d0, d1, d2, d3)` pattern.

use std::io;

use super::color_scaling::ColorScaling;
use super::hidraw::HidrawOps;
use super::{KeyboardLed, Rgb};

const ROWS: usize = 6;
const COLS: usize = 20;
const MAX_BRIGHTNESS: u8 = 0x0a; // 10

/// Build an LED ID from row and column: `(row & 0x07) << 5 | (col & 0x1f)`.
const fn led_id(row: u8, col: u8) -> u8 {
    ((row & 0x07) << 5) | (col & 0x1f)
}

const MODES: &[&str] = &["static", "random"];

pub struct Ite829x<H: HidrawOps = super::hidraw::HidrawDevice> {
    hid: H,
    scaling: ColorScaling,
    brightness: u8,
    color: Rgb,
    on: bool,
    current_mode: String,
}

impl<H: HidrawOps> Ite829x<H> {
    pub fn new(hid: H) -> Self {
        Self::with_scaling(hid, ColorScaling::IDENTITY)
    }

    pub fn with_scaling(hid: H, scaling: ColorScaling) -> Self {
        Self {
            hid,
            scaling,
            brightness: MAX_BRIGHTNESS,
            color: Rgb::WHITE,
            on: true,
            current_mode: "static".to_string(),
        }
    }

    fn scale_brightness(brightness: u8) -> u8 {
        super::scale_brightness(brightness, MAX_BRIGHTNESS)
    }

    /// Send a 6-byte command: `0xcc cmd d0 d1 d2 d3`.
    fn send_cmd(&self, cmd: u8, d0: u8, d1: u8, d2: u8, d3: u8) -> io::Result<()> {
        let buf = [0xcc, cmd, d0, d1, d2, d3];
        self.hid.set_feature(&buf)
    }

    /// Set brightness via command: `cc 09 [brightness] 02 00 00`.
    fn write_brightness(&self) -> io::Result<()> {
        self.send_cmd(0x09, self.brightness.min(MAX_BRIGHTNESS), 0x02, 0x00, 0x00)
    }

    /// Set a single key color: `cc 01 [led_id] [r] [g] [b]`.
    fn write_key(&self, row: u8, col: u8, r: u8, g: u8, b: u8) -> io::Result<()> {
        self.send_cmd(0x01, led_id(row, col), r, g, b)
    }

    /// Fill all keys with one color.
    fn write_all_color(&self, r: u8, g: u8, b: u8) -> io::Result<()> {
        for row in 0..ROWS as u8 {
            for col in 0..COLS as u8 {
                self.write_key(row, col, r, g, b)?;
            }
        }
        Ok(())
    }

    /// Set random animation: `cc 00 09 00 00 00`.
    fn write_random(&self) -> io::Result<()> {
        self.send_cmd(0x00, 0x09, 0x00, 0x00, 0x00)
    }

    fn write_off(&self) -> io::Result<()> {
        self.write_brightness()?;
        self.write_all_color(0, 0, 0)
    }
}

impl<H: HidrawOps> KeyboardLed for Ite829x<H> {
    fn set_brightness(&mut self, brightness: u8) -> io::Result<()> {
        self.brightness = Self::scale_brightness(brightness);
        if self.on {
            self.write_brightness()
        } else {
            Ok(())
        }
    }

    fn set_color(&mut self, _zone: u8, color: Rgb) -> io::Result<()> {
        self.color = color;
        Ok(())
    }

    fn set_mode(&mut self, mode: &str) -> io::Result<()> {
        if !MODES.contains(&mode) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown ite829x mode: {mode}"),
            ));
        }
        self.current_mode = mode.to_string();
        if self.on { self.flush() } else { Ok(()) }
    }

    fn zone_count(&self) -> u8 {
        1 // treated as single zone for simplicity
    }

    fn turn_off(&mut self) -> io::Result<()> {
        self.on = false;
        self.write_off()
    }

    fn turn_on(&mut self) -> io::Result<()> {
        self.on = true;
        self.flush()
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.on {
            return self.write_off();
        }
        self.write_brightness()?;
        match self.current_mode.as_str() {
            "random" => self.write_random(),
            _ => {
                let scaled = self.scaling.apply(self.color);
                self.write_all_color(scaled.r, scaled.g, scaled.b)
            }
        }
    }

    fn device_type(&self) -> &str {
        "ite829x"
    }

    fn available_modes(&self) -> Vec<String> {
        MODES.iter().map(|s| (*s).to_string()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::hidraw::MockHidraw;

    fn make_driver() -> Ite829x<MockHidraw> {
        Ite829x::new(MockHidraw::new(0x8910))
    }

    #[test]
    fn led_id_encoding() {
        assert_eq!(led_id(0, 0), 0x00);
        assert_eq!(led_id(1, 0), 0x20);
        assert_eq!(led_id(0, 1), 0x01);
        assert_eq!(led_id(5, 19), 0xb3); // (5<<5) | 19 = 160 + 19 = 179 = 0xb3
    }

    #[test]
    fn flush_sends_brightness_then_all_keys() {
        let mut drv = make_driver();
        drv.color = Rgb::new(0xFF, 0x00, 0x00);
        drv.flush().unwrap();
        let reports = drv.hid.sent_reports();
        // 1 brightness + ROWS*COLS key writes = 1 + 120 = 121
        assert_eq!(reports.len(), 121);
        // First report is brightness
        assert_eq!(reports[0][1], 0x09);
        // Second report onwards are key colors
        assert_eq!(reports[1][1], 0x01); // set key command
    }

    #[test]
    fn random_mode_sends_special_command() {
        let mut drv = make_driver();
        drv.set_mode("random").unwrap();
        let reports = drv.hid.sent_reports();
        // brightness + random command
        assert_eq!(reports.last().unwrap()[1], 0x00); // random cmd
        assert_eq!(reports.last().unwrap()[2], 0x09);
    }

    #[test]
    fn device_type_string() {
        let drv = make_driver();
        assert_eq!(drv.device_type(), "ite829x");
    }

    #[test]
    fn brightness_scales_correctly() {
        assert_eq!(Ite829x::<MockHidraw>::scale_brightness(0), 0);
        assert_eq!(Ite829x::<MockHidraw>::scale_brightness(255), MAX_BRIGHTNESS);
    }

    #[test]
    fn invalid_mode_error() {
        let mut drv = make_driver();
        assert!(drv.set_mode("wave").is_err());
    }

    #[test]
    fn report_format_6_bytes() {
        let mut drv = make_driver();
        drv.flush().unwrap();
        let reports = drv.hid.sent_reports();
        for r in &reports {
            assert_eq!(r.len(), 6);
            assert_eq!(r[0], 0xcc); // report ID
        }
    }
}
