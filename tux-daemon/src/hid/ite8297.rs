//! ITE 8297 RGB lightbar controller.
//!
//! Simple protocol: 64-byte SET_FEATURE report with color bytes at offsets 4–6.
//! Report ID 0xCC, sub-command 0xB0.

use std::io;

use super::color_scaling::ColorScaling;
use super::hidraw::HidrawOps;
use super::{KeyboardLed, Rgb};

/// Feature report size for ITE 8297.
const REPORT_SIZE: usize = 64;

pub struct Ite8297<H: HidrawOps = super::hidraw::HidrawDevice> {
    hid: H,
    scaling: ColorScaling,
    color: Rgb,
    on: bool,
}

impl<H: HidrawOps> Ite8297<H> {
    pub fn new(hid: H) -> Self {
        Self::with_scaling(hid, ColorScaling::IDENTITY)
    }

    pub fn with_scaling(hid: H, scaling: ColorScaling) -> Self {
        Self {
            hid,
            scaling,
            color: Rgb::WHITE,
            on: true,
        }
    }

    fn write_color(&self, r: u8, g: u8, b: u8) -> io::Result<()> {
        let mut buf = [0u8; REPORT_SIZE];
        buf[0] = 0xcc;
        buf[1] = 0xb0;
        buf[2] = 0x01;
        buf[3] = 0x01;
        buf[4] = r;
        buf[5] = g;
        buf[6] = b;
        self.hid.set_feature(&buf)
    }

    fn write_off(&self) -> io::Result<()> {
        self.write_color(0, 0, 0)
    }
}

impl<H: HidrawOps> KeyboardLed for Ite8297<H> {
    fn set_brightness(&mut self, brightness: u8) -> io::Result<()> {
        // ITE 8297 doesn't have separate brightness control —
        // scale the color values directly.
        if self.on {
            let scale = brightness as u16;
            let r = ((self.color.r as u16 * scale) / 255) as u8;
            let g = ((self.color.g as u16 * scale) / 255) as u8;
            let b = ((self.color.b as u16 * scale) / 255) as u8;
            self.write_color(r, g, b)
        } else {
            Ok(())
        }
    }

    fn set_color(&mut self, _zone: u8, color: Rgb) -> io::Result<()> {
        self.color = color;
        Ok(())
    }

    fn set_mode(&mut self, mode: &str) -> io::Result<()> {
        // ITE 8297 only supports static color
        if mode != "static" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("ite8297 only supports 'static' mode, got: {mode}"),
            ));
        }
        Ok(())
    }

    fn zone_count(&self) -> u8 {
        1
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
        let scaled = self.scaling.apply(self.color);
        self.write_color(scaled.r, scaled.g, scaled.b)
    }

    fn device_type(&self) -> &str {
        "ite8297"
    }

    fn available_modes(&self) -> Vec<String> {
        vec!["static".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::hidraw::MockHidraw;

    fn make_driver() -> Ite8297<MockHidraw> {
        Ite8297::new(MockHidraw::new(0x8297))
    }

    #[test]
    fn flush_writes_64_byte_feature_report() {
        let mut drv = make_driver();
        drv.color = Rgb::new(0xFF, 0x80, 0x40);
        drv.flush().unwrap();
        let reports = drv.hid.sent_reports();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].len(), REPORT_SIZE);
        assert_eq!(reports[0][0], 0xcc);
        assert_eq!(reports[0][1], 0xb0);
        assert_eq!(reports[0][4], 0xFF); // red
        assert_eq!(reports[0][5], 0x80); // green
        assert_eq!(reports[0][6], 0x40); // blue
    }

    #[test]
    fn turn_off_writes_zeros() {
        let mut drv = make_driver();
        drv.turn_off().unwrap();
        let reports = drv.hid.sent_reports();
        assert_eq!(reports[0][4], 0);
        assert_eq!(reports[0][5], 0);
        assert_eq!(reports[0][6], 0);
    }

    #[test]
    fn only_static_mode_supported() {
        let mut drv = make_driver();
        assert!(drv.set_mode("static").is_ok());
        assert!(drv.set_mode("breathing").is_err());
    }

    #[test]
    fn device_type_string() {
        let drv = make_driver();
        assert_eq!(drv.device_type(), "ite8297");
    }

    #[test]
    fn brightness_scales_color_output() {
        let mut drv = make_driver();
        drv.color = Rgb::new(255, 255, 255);
        drv.set_brightness(128).unwrap();
        let reports = drv.hid.sent_reports();
        // ~50% brightness
        assert!(reports[0][4] > 120 && reports[0][4] < 132);
    }
}
