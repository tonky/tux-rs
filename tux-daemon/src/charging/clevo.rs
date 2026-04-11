//! Clevo flexicharger backend — start/end threshold control via sysfs.
//!
//! The tuxedo-clevo kernel shim exposes:
//!   - `charge_start_threshold` (RW, 0–100)
//!   - `charge_end_threshold`   (RW, 0–100)

use std::io;

use super::ChargingBackend;
use crate::platform::sysfs::SysfsReader;

const SYSFS_BASE: &str = "/sys/devices/platform/tuxedo-clevo";

/// Clevo ACPI flexicharger backend.
///
/// Controls when charging starts and stops via battery percentage thresholds.
#[derive(Debug)]
pub struct ClevoCharging {
    sysfs: SysfsReader,
}

impl ClevoCharging {
    /// Create a new backend, returning `None` if the sysfs directory doesn't exist.
    pub fn new() -> Option<Self> {
        let sysfs = SysfsReader::new(SYSFS_BASE);
        sysfs.available().then_some(Self { sysfs })
    }

    /// Create a backend with a custom sysfs path (for testing).
    #[cfg(test)]
    pub fn with_path(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            sysfs: SysfsReader::new(path),
        }
    }

    fn clamp_pct(pct: u8) -> u8 {
        pct.min(100)
    }
}

impl ChargingBackend for ClevoCharging {
    fn get_start_threshold(&self) -> io::Result<u8> {
        self.sysfs.read_u8("charge_start_threshold")
    }

    fn set_start_threshold(&self, pct: u8) -> io::Result<()> {
        self.sysfs
            .write_u8("charge_start_threshold", Self::clamp_pct(pct))
    }

    fn get_end_threshold(&self) -> io::Result<u8> {
        self.sysfs.read_u8("charge_end_threshold")
    }

    fn set_end_threshold(&self, pct: u8) -> io::Result<()> {
        self.sysfs
            .write_u8("charge_end_threshold", Self::clamp_pct(pct))
    }

    fn get_profile(&self) -> io::Result<Option<String>> {
        // Clevo doesn't use named profiles.
        Ok(None)
    }

    fn set_profile(&self, _profile: &str) -> io::Result<()> {
        // No-op for Clevo.
        Ok(())
    }

    fn get_priority(&self) -> io::Result<Option<String>> {
        // Clevo doesn't have priority control.
        Ok(None)
    }

    fn set_priority(&self, _priority: &str) -> io::Result<()> {
        // No-op for Clevo.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tux_core::mock::sysfs::MockSysfs;

    fn setup() -> (MockSysfs, ClevoCharging) {
        let mock = MockSysfs::new();
        let base = mock.platform_dir("tuxedo-clevo");
        mock.create_attr("devices/platform/tuxedo-clevo/charge_start_threshold", "40");
        mock.create_attr("devices/platform/tuxedo-clevo/charge_end_threshold", "80");
        let backend = ClevoCharging::with_path(base);
        (mock, backend)
    }

    #[test]
    fn get_start_threshold() {
        let (_mock, backend) = setup();
        assert_eq!(backend.get_start_threshold().unwrap(), 40);
    }

    #[test]
    fn get_end_threshold() {
        let (_mock, backend) = setup();
        assert_eq!(backend.get_end_threshold().unwrap(), 80);
    }

    #[test]
    fn set_start_threshold_roundtrip() {
        let (_mock, backend) = setup();
        backend.set_start_threshold(25).unwrap();
        assert_eq!(backend.get_start_threshold().unwrap(), 25);
    }

    #[test]
    fn set_end_threshold_roundtrip() {
        let (_mock, backend) = setup();
        backend.set_end_threshold(90).unwrap();
        assert_eq!(backend.get_end_threshold().unwrap(), 90);
    }

    #[test]
    fn threshold_clamped_to_100() {
        let (_mock, backend) = setup();
        backend.set_end_threshold(150).unwrap();
        assert_eq!(backend.get_end_threshold().unwrap(), 100);
    }

    #[test]
    fn profile_returns_none() {
        let (_mock, backend) = setup();
        assert!(backend.get_profile().unwrap().is_none());
    }

    #[test]
    fn priority_returns_none() {
        let (_mock, backend) = setup();
        assert!(backend.get_priority().unwrap().is_none());
    }
}
