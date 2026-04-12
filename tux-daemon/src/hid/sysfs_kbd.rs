//! Sysfs-based keyboard backlight controllers.
//!
//! Drives keyboard backlights exposed via the Linux LED class at
//! `/sys/class/leds/rgb:kbd_backlight` (RGB) or
//! `/sys/class/leds/white:kbd_backlight` (white-only).
//! Controls brightness through `brightness` sysfs file, and color
//! through `multi_intensity` for RGB keyboards.
//! No animation modes — static color only.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{KeyboardLed, Rgb};

/// Sysfs-based single-zone RGB keyboard backlight.
pub struct SysfsRgbKeyboard {
    /// e.g. `/sys/class/leds/rgb:kbd_backlight`
    base_path: PathBuf,
    /// Hardware maximum from `max_brightness`
    max_brightness: u16,
}

impl SysfsRgbKeyboard {
    /// Open a sysfs RGB LED at the given path.
    ///
    /// Reads `max_brightness` to calibrate the brightness scale.
    pub fn open(base_path: &Path) -> io::Result<Self> {
        let max_str = fs::read_to_string(base_path.join("max_brightness"))?;
        let max_brightness: u16 = max_str
            .trim()
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if max_brightness == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "max_brightness is 0",
            ));
        }
        Ok(Self {
            base_path: base_path.to_owned(),
            max_brightness,
        })
    }

    fn write_sysfs(&self, file: &str, value: &str) -> io::Result<()> {
        fs::write(self.base_path.join(file), value)
    }
}

impl KeyboardLed for SysfsRgbKeyboard {
    fn set_brightness(&mut self, brightness: u8) -> io::Result<()> {
        // Scale 0–255 input to 0–max_brightness.
        let scaled = (brightness as u32 * self.max_brightness as u32) / 255;
        self.write_sysfs("brightness", &scaled.to_string())
    }

    fn set_color(&mut self, _zone: u8, color: Rgb) -> io::Result<()> {
        let value = format!("{} {} {}", color.r, color.g, color.b);
        self.write_sysfs("multi_intensity", &value)
    }

    fn set_mode(&mut self, mode: &str) -> io::Result<()> {
        if mode != "static" {
            tracing::warn!("sysfs keyboard does not support mode '{mode}', using static");
        }
        Ok(())
    }

    fn zone_count(&self) -> u8 {
        1
    }

    fn turn_off(&mut self) -> io::Result<()> {
        self.write_sysfs("brightness", "0")
    }

    fn turn_on(&mut self) -> io::Result<()> {
        // Restore to max brightness.
        self.write_sysfs("brightness", &self.max_brightness.to_string())
    }

    fn flush(&mut self) -> io::Result<()> {
        // Sysfs writes are immediate — no buffering needed.
        Ok(())
    }

    fn device_type(&self) -> &str {
        "sysfs_rgb"
    }

    fn available_modes(&self) -> Vec<String> {
        vec!["static".into()]
    }
}

/// Sysfs-based white-only keyboard backlight.
///
/// Drives single-color (white) keyboards exposed at
/// `/sys/class/leds/white:kbd_backlight`. Brightness only — no color or mode
/// control. Typically has very few levels (e.g. max_brightness=2).
pub struct SysfsWhiteKeyboard {
    /// e.g. `/sys/class/leds/white:kbd_backlight`
    base_path: PathBuf,
    /// Hardware maximum from `max_brightness` (e.g. 2)
    max_brightness: u16,
}

impl SysfsWhiteKeyboard {
    /// Open a sysfs white LED at the given path.
    pub fn open(base_path: &Path) -> io::Result<Self> {
        let max_str = fs::read_to_string(base_path.join("max_brightness"))?;
        let max_brightness: u16 = max_str
            .trim()
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if max_brightness == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "max_brightness is 0",
            ));
        }
        Ok(Self {
            base_path: base_path.to_owned(),
            max_brightness,
        })
    }

    fn write_sysfs(&self, file: &str, value: &str) -> io::Result<()> {
        fs::write(self.base_path.join(file), value)
    }
}

impl KeyboardLed for SysfsWhiteKeyboard {
    fn set_brightness(&mut self, brightness: u8) -> io::Result<()> {
        if brightness == 0 {
            return self.write_sysfs("brightness", "0");
        }

        // Two-step white keyboards (max=2): preserve distinct low/high stages.
        // 1..127 -> 1 (dim), 128..255 -> 2 (bright).
        if self.max_brightness == 2 {
            let level = if brightness <= 127 { 1 } else { 2 };
            return self.write_sysfs("brightness", &level.to_string());
        }

        // Scale 0–255 input to 0–max_brightness with rounding.
        // Rounding is important for low max_brightness values (e.g. 2)
        // to avoid truncation eating levels.
        let scaled = ((brightness as u32 * self.max_brightness as u32) + 127) / 255;
        self.write_sysfs("brightness", &scaled.to_string())
    }

    fn set_color(&mut self, _zone: u8, _color: Rgb) -> io::Result<()> {
        // White-only keyboard — color is not supported, silently ignore.
        Ok(())
    }

    fn set_mode(&mut self, _mode: &str) -> io::Result<()> {
        // Only static mode supported.
        Ok(())
    }

    fn zone_count(&self) -> u8 {
        1
    }

    fn turn_off(&mut self) -> io::Result<()> {
        self.write_sysfs("brightness", "0")
    }

    fn turn_on(&mut self) -> io::Result<()> {
        self.write_sysfs("brightness", &self.max_brightness.to_string())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn device_type(&self) -> &str {
        "sysfs_white"
    }

    fn available_modes(&self) -> Vec<String> {
        vec!["static".into()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_mock_sysfs(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("max_brightness"), "200\n").unwrap();
        fs::write(dir.join("brightness"), "0\n").unwrap();
        fs::write(dir.join("multi_intensity"), "0 0 0\n").unwrap();
    }

    #[test]
    fn open_reads_max_brightness() {
        let tmp = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tmp/test_sysfs_open");
        let _ = fs::remove_dir_all(&tmp);
        create_mock_sysfs(&tmp);

        let kb = SysfsRgbKeyboard::open(&tmp).unwrap();
        assert_eq!(kb.max_brightness, 200);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn open_fails_on_missing_max() {
        let tmp = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tmp/test_sysfs_no_max");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        // No max_brightness file.
        assert!(SysfsRgbKeyboard::open(&tmp).is_err());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn set_brightness_scales_to_max() {
        let tmp = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tmp/test_sysfs_brightness");
        let _ = fs::remove_dir_all(&tmp);
        create_mock_sysfs(&tmp);

        let mut kb = SysfsRgbKeyboard::open(&tmp).unwrap();
        // 255 → max_brightness (200)
        kb.set_brightness(255).unwrap();
        assert_eq!(fs::read_to_string(tmp.join("brightness")).unwrap(), "200");

        // 128 → ~100
        kb.set_brightness(128).unwrap();
        let val: u32 = fs::read_to_string(tmp.join("brightness"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(val, 100); // 128*200/255 = 100

        // 0 → 0
        kb.set_brightness(0).unwrap();
        assert_eq!(fs::read_to_string(tmp.join("brightness")).unwrap(), "0");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn set_color_writes_multi_intensity() {
        let tmp = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tmp/test_sysfs_color");
        let _ = fs::remove_dir_all(&tmp);
        create_mock_sysfs(&tmp);

        let mut kb = SysfsRgbKeyboard::open(&tmp).unwrap();
        kb.set_color(0, Rgb::new(255, 128, 0)).unwrap();
        assert_eq!(
            fs::read_to_string(tmp.join("multi_intensity")).unwrap(),
            "255 128 0"
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn available_modes_is_static_only() {
        let tmp = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tmp/test_sysfs_modes");
        let _ = fs::remove_dir_all(&tmp);
        create_mock_sysfs(&tmp);

        let kb = SysfsRgbKeyboard::open(&tmp).unwrap();
        assert_eq!(kb.available_modes(), vec!["static"]);
        assert_eq!(kb.device_type(), "sysfs_rgb");
        assert_eq!(kb.zone_count(), 1);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn turn_off_sets_brightness_zero() {
        let tmp = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tmp/test_sysfs_off");
        let _ = fs::remove_dir_all(&tmp);
        create_mock_sysfs(&tmp);

        let mut kb = SysfsRgbKeyboard::open(&tmp).unwrap();
        kb.set_brightness(200).unwrap();
        kb.turn_off().unwrap();
        assert_eq!(fs::read_to_string(tmp.join("brightness")).unwrap(), "0");

        kb.turn_on().unwrap();
        assert_eq!(fs::read_to_string(tmp.join("brightness")).unwrap(), "200");

        let _ = fs::remove_dir_all(&tmp);
    }

    // ── SysfsWhiteKeyboard tests ──

    fn create_mock_white_sysfs(dir: &Path, max_brightness: u16) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("max_brightness"), format!("{max_brightness}\n")).unwrap();
        fs::write(dir.join("brightness"), "0\n").unwrap();
    }

    #[test]
    fn white_open_reads_max_brightness() {
        let tmp = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tmp/test_sysfs_white_open");
        let _ = fs::remove_dir_all(&tmp);
        create_mock_white_sysfs(&tmp, 2);

        let kb = SysfsWhiteKeyboard::open(&tmp).unwrap();
        assert_eq!(kb.max_brightness, 2);
        assert_eq!(kb.device_type(), "sysfs_white");
        assert_eq!(kb.zone_count(), 1);
        assert_eq!(kb.available_modes(), vec!["static"]);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn white_brightness_scales_with_rounding() {
        let tmp = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tmp/test_sysfs_white_brightness");
        let _ = fs::remove_dir_all(&tmp);
        create_mock_white_sysfs(&tmp, 2);

        let mut kb = SysfsWhiteKeyboard::open(&tmp).unwrap();

        // 0 → 0 (off)
        kb.set_brightness(0).unwrap();
        assert_eq!(fs::read_to_string(tmp.join("brightness")).unwrap(), "0");

        // 255 → 2 (max)
        kb.set_brightness(255).unwrap();
        assert_eq!(fs::read_to_string(tmp.join("brightness")).unwrap(), "2");

        // 128 -> 2 (upper stage)
        kb.set_brightness(128).unwrap();
        assert_eq!(
            fs::read_to_string(tmp.join("brightness")).unwrap().trim(),
            "2"
        );

        // 64 -> 1 (lower stage)
        kb.set_brightness(64).unwrap();
        assert_eq!(
            fs::read_to_string(tmp.join("brightness")).unwrap().trim(),
            "1"
        );

        // 63 -> 1 (lower stage)
        kb.set_brightness(63).unwrap();
        assert_eq!(
            fs::read_to_string(tmp.join("brightness")).unwrap().trim(),
            "1"
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn white_color_is_noop() {
        let tmp = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tmp/test_sysfs_white_color");
        let _ = fs::remove_dir_all(&tmp);
        create_mock_white_sysfs(&tmp, 2);

        let mut kb = SysfsWhiteKeyboard::open(&tmp).unwrap();
        // set_color should succeed silently without creating multi_intensity file.
        kb.set_color(0, Rgb::new(255, 0, 0)).unwrap();
        assert!(!tmp.join("multi_intensity").exists());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn white_turn_off_on() {
        let tmp = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tmp/test_sysfs_white_off_on");
        let _ = fs::remove_dir_all(&tmp);
        create_mock_white_sysfs(&tmp, 2);

        let mut kb = SysfsWhiteKeyboard::open(&tmp).unwrap();
        kb.set_brightness(255).unwrap();
        kb.turn_off().unwrap();
        assert_eq!(fs::read_to_string(tmp.join("brightness")).unwrap(), "0");

        kb.turn_on().unwrap();
        assert_eq!(fs::read_to_string(tmp.join("brightness")).unwrap(), "2");

        let _ = fs::remove_dir_all(&tmp);
    }
}
