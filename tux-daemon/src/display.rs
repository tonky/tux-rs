//! Sysfs backlight display brightness backend.
//!
//! Scans `/sys/class/backlight/` for available drivers and provides
//! read/write access to display brightness using the standard Linux
//! backlight interface.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tracing::{debug, info, warn};

const BACKLIGHT_BASE: &str = "/sys/class/backlight";

/// A discovered sysfs backlight controller.
#[derive(Debug)]
pub struct BacklightController {
    /// Driver name (directory name under /sys/class/backlight/).
    pub driver: String,
    /// Full path to the driver directory.
    path: PathBuf,
    /// Cached max_brightness value.
    max_brightness: u32,
}

impl BacklightController {
    /// Try to open a backlight controller at the given sysfs path.
    fn open(path: &Path) -> Option<Self> {
        let driver = path.file_name()?.to_str()?.to_string();
        let max_brightness = read_sysfs_u32(&path.join("max_brightness"))?;
        if max_brightness == 0 {
            warn!("backlight driver {driver} has max_brightness=0, skipping");
            return None;
        }
        debug!("discovered backlight driver: {driver} (max={max_brightness})");
        Some(Self {
            driver,
            path: path.to_path_buf(),
            max_brightness,
        })
    }

    /// Read current brightness as a percentage (0–100).
    pub fn brightness_percent(&self) -> Option<u32> {
        let raw = read_sysfs_u32(&self.path.join("brightness"))?;
        Some(((raw as u64 * 100) / self.max_brightness as u64) as u32)
    }

    /// Read current raw brightness value.
    pub fn brightness_raw(&self) -> Option<u32> {
        read_sysfs_u32(&self.path.join("brightness"))
    }

    /// Write brightness as a percentage (0–100).
    pub fn set_brightness_percent(&self, percent: u32) -> std::io::Result<()> {
        let percent = percent.min(100);
        let raw = ((percent as u64 * self.max_brightness as u64) / 100) as u32;
        self.set_brightness_raw(raw)
    }

    /// Write a raw brightness value.
    pub fn set_brightness_raw(&self, raw: u32) -> std::io::Result<()> {
        let raw = raw.min(self.max_brightness);
        fs::write(self.path.join("brightness"), raw.to_string())
    }

    /// Maximum raw brightness value.
    pub fn max_brightness(&self) -> u32 {
        self.max_brightness
    }
}

/// Display brightness manager: holds discovered backlight controllers.
pub struct DisplayBacklight {
    controllers: Vec<BacklightController>,
}

impl DisplayBacklight {
    /// Scan sysfs for available backlight controllers.
    pub fn discover() -> Self {
        let mut controllers = Vec::new();
        if let Ok(entries) = fs::read_dir(BACKLIGHT_BASE) {
            for entry in entries.flatten() {
                if let Some(ctrl) = BacklightController::open(&entry.path()) {
                    controllers.push(ctrl);
                }
            }
        }
        if controllers.is_empty() {
            info!("no backlight controllers found");
        } else {
            info!(
                "found {} backlight controller(s): {}",
                controllers.len(),
                controllers
                    .iter()
                    .map(|c| c.driver.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        Self { controllers }
    }

    /// Whether any backlight controller was found.
    pub fn is_available(&self) -> bool {
        !self.controllers.is_empty()
    }

    /// Get the primary (first) controller, if any.
    pub fn primary(&self) -> Option<&BacklightController> {
        self.controllers.first()
    }

    /// Read current brightness as percentage from the primary controller.
    pub fn brightness_percent(&self) -> Option<u32> {
        self.primary()?.brightness_percent()
    }

    /// Set brightness as percentage on all controllers (best-effort).
    pub fn set_brightness_percent(&self, percent: u32) -> std::io::Result<()> {
        let mut last_err = None;
        for ctrl in &self.controllers {
            if let Err(e) = ctrl.set_brightness_percent(percent) {
                warn!("failed to set brightness on {}: {e}", ctrl.driver);
                last_err = Some(e);
            }
        }
        match last_err {
            Some(e) if self.controllers.len() == 1 => Err(e),
            _ => Ok(()),
        }
    }
}

/// Shared display backlight handle.
pub type SharedDisplay = Arc<DisplayBacklight>;

/// Read a u32 from a sysfs file.
fn read_sysfs_u32(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_mock_backlight(dir: &Path, driver: &str, max: u32, current: u32) -> PathBuf {
        let driver_path = dir.join(driver);
        fs::create_dir_all(&driver_path).unwrap();
        fs::write(driver_path.join("max_brightness"), max.to_string()).unwrap();
        fs::write(driver_path.join("brightness"), current.to_string()).unwrap();
        driver_path
    }

    #[test]
    fn backlight_controller_reads_percentage() {
        let tmp = PathBuf::from("tmp/test_backlight_read");
        let _ = fs::remove_dir_all(&tmp);
        let drv = setup_mock_backlight(&tmp, "test_bl", 1000, 500);

        let ctrl = BacklightController::open(&drv).unwrap();
        assert_eq!(ctrl.max_brightness(), 1000);
        assert_eq!(ctrl.brightness_percent(), Some(50));
        assert_eq!(ctrl.brightness_raw(), Some(500));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn backlight_controller_writes_percentage() {
        let tmp = PathBuf::from("tmp/test_backlight_write");
        let _ = fs::remove_dir_all(&tmp);
        let drv = setup_mock_backlight(&tmp, "test_bl", 1000, 500);

        let ctrl = BacklightController::open(&drv).unwrap();
        ctrl.set_brightness_percent(75).unwrap();
        assert_eq!(ctrl.brightness_percent(), Some(75));
        assert_eq!(ctrl.brightness_raw(), Some(750));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn backlight_controller_clamps_to_max() {
        let tmp = PathBuf::from("tmp/test_backlight_clamp");
        let _ = fs::remove_dir_all(&tmp);
        let drv = setup_mock_backlight(&tmp, "test_bl", 1000, 500);

        let ctrl = BacklightController::open(&drv).unwrap();
        ctrl.set_brightness_percent(200).unwrap();
        // Should be clamped to 100% = 1000
        assert_eq!(ctrl.brightness_raw(), Some(1000));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn skips_driver_with_zero_max() {
        let tmp = PathBuf::from("tmp/test_backlight_zero");
        let _ = fs::remove_dir_all(&tmp);
        let drv = setup_mock_backlight(&tmp, "zero_bl", 0, 0);

        let ctrl = BacklightController::open(&drv);
        assert!(ctrl.is_none());

        let _ = fs::remove_dir_all(&tmp);
    }
}
