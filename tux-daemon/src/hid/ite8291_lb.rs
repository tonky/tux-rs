//! ITE 8291 Lightbar controller (PIDs: 0x6010, 0x7000, 0x7001).
//!
//! Three sub-variants with slightly different control byte layouts.
//! Single-zone mono color + animation modes.

use std::io;

use super::color_scaling::ColorScaling;
use super::hidraw::HidrawOps;
use super::{KeyboardLed, Rgb};

const CTRL_SIZE: usize = 8;
const MAX_BRIGHTNESS: u8 = 0x64; // 100

const MODES: &[&str] = &["static", "breathing"];

fn mode_to_code(mode: &str) -> Option<u8> {
    match mode {
        "static" => Some(0x01),
        "breathing" => Some(0x02),
        _ => None,
    }
}

/// Sub-variant determined by USB product ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LbVariant {
    /// PID 0x6010
    V6010,
    /// PID 0x7000
    V7000,
    /// PID 0x7001
    V7001,
}

impl LbVariant {
    fn from_pid(pid: u16) -> Self {
        match pid {
            0x7000 => Self::V7000,
            0x7001 => Self::V7001,
            _ => Self::V6010,
        }
    }
}

pub struct Ite8291Lb<H: HidrawOps = super::hidraw::HidrawDevice> {
    hid: H,
    variant: LbVariant,
    scaling: ColorScaling,
    brightness: u8,
    color: Rgb,
    on: bool,
    current_mode: String,
}

impl<H: HidrawOps> Ite8291Lb<H> {
    pub fn new(hid: H) -> Self {
        Self::with_scaling(hid, ColorScaling::IDENTITY)
    }

    pub fn with_scaling(hid: H, scaling: ColorScaling) -> Self {
        let variant = LbVariant::from_pid(hid.product_id());
        Self {
            hid,
            variant,
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

    fn write_control(&self, data: &[u8; CTRL_SIZE]) -> io::Result<()> {
        self.hid.set_feature(data)
    }

    /// Write mono color + brightness in static mode.
    fn write_mono(&self) -> io::Result<()> {
        let scaled = self.scaling.apply(self.color);
        let Rgb { r, g, b } = scaled;
        let br = self.brightness.min(MAX_BRIGHTNESS);

        match self.variant {
            LbVariant::V6010 => {
                self.write_control(&[0x14, 0x00, 0x01, r, g, b, 0x00, 0x00])?;
                self.write_control(&[0x08, 0x02, 0x01, 0x01, br, 0x08, 0x00, 0x00])?;
            }
            LbVariant::V7000 => {
                self.write_control(&[0x14, 0x01, 0x01, r, g, b, 0x00, 0x00])?;
                self.write_control(&[0x08, 0x21, 0x01, 0x01, br, 0x01, 0x00, 0x00])?;
            }
            LbVariant::V7001 => {
                self.write_control(&[0x14, 0x00, 0x01, r, g, b, 0x00, 0x00])?;
                self.write_control(&[0x08, 0x22, 0x01, 0x01, br, 0x01, 0x00, 0x00])?;
            }
        }
        Ok(())
    }

    fn write_breathing(&self) -> io::Result<()> {
        let br = self.brightness.min(MAX_BRIGHTNESS);
        let speed = 0x05u8;

        match self.variant {
            LbVariant::V6010 => {
                self.write_control(&[0x08, 0x02, 0x02, speed, br, 0x08, 0x00, 0x00])?;
            }
            LbVariant::V7000 => {
                self.write_control(&[0x08, 0x21, 0x02, speed, br, 0x08, 0x00, 0x00])?;
            }
            LbVariant::V7001 => {
                self.write_control(&[0x08, 0x22, 0x02, speed, br, 0x08, 0x00, 0x00])?;
            }
        }
        Ok(())
    }

    fn write_off(&self) -> io::Result<()> {
        self.write_control(&[0x08, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
    }
}

impl<H: HidrawOps> KeyboardLed for Ite8291Lb<H> {
    fn set_brightness(&mut self, brightness: u8) -> io::Result<()> {
        self.brightness = Self::scale_brightness(brightness);
        if self.on { self.flush() } else { Ok(()) }
    }

    fn set_color(&mut self, _zone: u8, color: Rgb) -> io::Result<()> {
        self.color = color;
        Ok(())
    }

    fn set_mode(&mut self, mode: &str) -> io::Result<()> {
        if mode_to_code(mode).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown lightbar mode: {mode}"),
            ));
        }
        self.current_mode = mode.to_string();
        if self.on { self.flush() } else { Ok(()) }
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
        match self.current_mode.as_str() {
            "breathing" => self.write_breathing(),
            _ => self.write_mono(),
        }
    }

    fn device_type(&self) -> &str {
        "ite8291_lb"
    }

    fn available_modes(&self) -> Vec<String> {
        MODES.iter().map(|s| (*s).to_string()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::hidraw::MockHidraw;

    fn make_driver(pid: u16) -> Ite8291Lb<MockHidraw> {
        Ite8291Lb::new(MockHidraw::new(pid))
    }

    #[test]
    fn variant_detection_from_pid() {
        assert_eq!(LbVariant::from_pid(0x6010), LbVariant::V6010);
        assert_eq!(LbVariant::from_pid(0x7000), LbVariant::V7000);
        assert_eq!(LbVariant::from_pid(0x7001), LbVariant::V7001);
    }

    #[test]
    fn write_mono_v6010() {
        let mut drv = make_driver(0x6010);
        drv.color = Rgb::new(0xFF, 0x00, 0x80);
        drv.flush().unwrap();
        let reports = drv.hid.sent_reports();
        assert_eq!(reports.len(), 2);
        // Color define
        assert_eq!(reports[0][0], 0x14);
        assert_eq!(reports[0][1], 0x00); // V6010
        assert_eq!(reports[0][3], 0xFF); // red
        assert_eq!(reports[0][4], 0x00); // green
        assert_eq!(reports[0][5], 0x80); // blue
        // Params
        assert_eq!(reports[1][0], 0x08);
        assert_eq!(reports[1][1], 0x02); // V6010
    }

    #[test]
    fn write_mono_v7000() {
        let mut drv = make_driver(0x7000);
        drv.flush().unwrap();
        let reports = drv.hid.sent_reports();
        assert_eq!(reports[0][1], 0x01); // V7000 color byte
        assert_eq!(reports[1][1], 0x21); // V7000 params byte
    }

    #[test]
    fn write_mono_v7001() {
        let mut drv = make_driver(0x7001);
        drv.flush().unwrap();
        let reports = drv.hid.sent_reports();
        assert_eq!(reports[0][1], 0x00); // V7001 has 0x00 like V6010
        assert_eq!(reports[1][1], 0x22); // V7001 params byte
    }

    #[test]
    fn breathing_mode_v6010() {
        let mut drv = make_driver(0x6010);
        drv.set_mode("breathing").unwrap();
        let reports = drv.hid.sent_reports();
        assert_eq!(reports.last().unwrap()[2], 0x02); // breathing
    }

    #[test]
    fn turn_off_sends_off_bytes() {
        let mut drv = make_driver(0x6010);
        drv.turn_off().unwrap();
        let reports = drv.hid.sent_reports();
        assert_eq!(reports[0][0], 0x08);
        assert_eq!(reports[0][1], 0x01); // off
    }

    #[test]
    fn invalid_mode_error() {
        let mut drv = make_driver(0x6010);
        assert!(drv.set_mode("nonexistent").is_err());
    }

    #[test]
    fn device_type_string() {
        let drv = make_driver(0x6010);
        assert_eq!(drv.device_type(), "ite8291_lb");
    }

    #[test]
    fn zone_count_is_one() {
        let drv = make_driver(0x6010);
        assert_eq!(drv.zone_count(), 1);
    }
}
