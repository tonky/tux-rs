//! ITE USB HID keyboard backlight control.
//!
//! Operates entirely in userspace via `/dev/hidraw*` — no kernel module needed.
//! Supports four ITE chip families: 8291 (per-key), 8291-LB (lightbar),
//! 8297 (RGB lightbar), and 829x (per-key 6×20).

use std::sync::{Arc, Mutex};

pub mod color_scaling;
pub mod discover;
pub mod hidraw;
pub mod ite8291;
pub mod ite8291_lb;
pub mod ite8297;
pub mod ite829x;
pub mod sysfs_kbd;

/// Shared reference to a keyboard LED controller, safe to clone across interfaces.
pub type SharedKeyboard = Arc<Mutex<Box<dyn KeyboardLed>>>;

/// Wrap discovered keyboards into shared references.
pub fn wrap_keyboards(keyboards: Vec<Box<dyn KeyboardLed>>) -> Vec<SharedKeyboard> {
    keyboards
        .into_iter()
        .map(|kb| Arc::new(Mutex::new(kb)))
        .collect()
}

/// Scale a 0–255 brightness value to a device-specific range [0, device_max].
pub fn scale_brightness(brightness: u8, device_max: u8) -> u8 {
    ((brightness as u16 * device_max as u16) / 255) as u8
}

/// RGB color value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const WHITE: Self = Self::new(255, 255, 255);
    pub const OFF: Self = Self::new(0, 0, 0);
}

/// Common trait for all ITE keyboard LED controllers.
pub trait KeyboardLed: Send + Sync {
    /// Set overall brightness (0–255, scaled to device range internally).
    fn set_brightness(&mut self, brightness: u8) -> std::io::Result<()>;

    /// Set color for a given zone (0-indexed).
    fn set_color(&mut self, zone: u8, color: Rgb) -> std::io::Result<()>;

    /// Set animation mode by name.
    fn set_mode(&mut self, mode: &str) -> std::io::Result<()>;

    /// Number of independently-addressable color zones.
    fn zone_count(&self) -> u8;

    /// Turn off LEDs (preserves settings).
    fn turn_off(&mut self) -> std::io::Result<()>;

    /// Turn on LEDs (restores previous state).
    fn turn_on(&mut self) -> std::io::Result<()>;

    /// Flush buffered state to hardware.
    fn flush(&mut self) -> std::io::Result<()>;

    /// Human-readable device type identifier.
    fn device_type(&self) -> &str;

    /// List of supported animation modes.
    fn available_modes(&self) -> Vec<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_default_is_black() {
        assert_eq!(Rgb::default(), Rgb::OFF);
    }

    #[test]
    fn rgb_white_constant() {
        assert_eq!(Rgb::WHITE, Rgb::new(255, 255, 255));
    }

    #[test]
    fn scale_brightness_full_range() {
        // 255 → device_max (identity at max)
        assert_eq!(scale_brightness(255, 50), 50);
        assert_eq!(scale_brightness(255, 100), 100);
        assert_eq!(scale_brightness(255, 10), 10);
    }

    #[test]
    fn scale_brightness_zero() {
        assert_eq!(scale_brightness(0, 50), 0);
        assert_eq!(scale_brightness(0, 100), 0);
        assert_eq!(scale_brightness(0, 10), 0);
    }

    #[test]
    fn scale_brightness_midpoint() {
        // 128/255 ≈ 0.502 → 50 * 0.502 = 25.1 → 25
        assert_eq!(scale_brightness(128, 50), 25);
        // 128/255 ≈ 0.502 → 100 * 0.502 = 50.2 → 50
        assert_eq!(scale_brightness(128, 100), 50);
    }

    #[test]
    fn scale_brightness_matches_original_drivers() {
        // Verify the shared function produces the same results as the
        // original per-driver implementations for each device_max.
        let ite8291_max: u8 = 0x32; // 50
        let ite8291_lb_max: u8 = 0x64; // 100
        let ite829x_max: u8 = 0x0a; // 10

        for b in 0..=255u8 {
            let expected_8291 = ((b as u16 * ite8291_max as u16) / 255) as u8;
            assert_eq!(scale_brightness(b, ite8291_max), expected_8291);

            let expected_8291_lb = ((b as u16 * ite8291_lb_max as u16) / 255) as u8;
            assert_eq!(scale_brightness(b, ite8291_lb_max), expected_8291_lb);

            let expected_829x = ((b as u16 * ite829x_max as u16) / 255) as u8;
            assert_eq!(scale_brightness(b, ite829x_max), expected_829x);
        }
    }
}
