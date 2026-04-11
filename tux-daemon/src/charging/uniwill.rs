//! Uniwill EC charging backend — charge profile and priority control via sysfs.
//!
//! The tuxedo-uniwill kernel shim exposes:
//!   - `charge_profile`  (RW, "high_capacity" | "balanced" | "stationary")
//!   - `charge_priority` (RW, "charge" | "performance")

use std::io;

use super::ChargingBackend;
use crate::platform::sysfs::SysfsReader;

const SYSFS_BASE: &str = "/sys/devices/platform/tuxedo-uniwill";

/// Valid charge profile names.
const VALID_PROFILES: &[&str] = &["high_capacity", "balanced", "stationary"];

/// Valid priority values.
const VALID_PRIORITIES: &[&str] = &["charge", "performance"];

/// Uniwill EC charging backend.
///
/// Controls battery charging via named profiles (capacity strategy) and
/// priority (charge-first vs performance-first TDP allocation).
#[derive(Debug)]
pub struct UniwillCharging {
    sysfs: SysfsReader,
}

impl UniwillCharging {
    /// Create a new backend, returning `None` if the charging sysfs files don't exist.
    pub fn new() -> Option<Self> {
        let sysfs = SysfsReader::new(SYSFS_BASE);
        if sysfs.exists("charge_profile") || sysfs.exists("charge_priority") {
            Some(Self { sysfs })
        } else {
            None
        }
    }

    /// Create a backend with a custom sysfs path (for testing).
    #[cfg(test)]
    pub fn with_path(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            sysfs: SysfsReader::new(path),
        }
    }

    fn validate_profile(profile: &str) -> io::Result<()> {
        if VALID_PROFILES.contains(&profile) {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "invalid charge profile '{profile}', expected one of: {}",
                    VALID_PROFILES.join(", ")
                ),
            ))
        }
    }

    fn validate_priority(priority: &str) -> io::Result<()> {
        if VALID_PRIORITIES.contains(&priority) {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "invalid charge priority '{priority}', expected one of: {}",
                    VALID_PRIORITIES.join(", ")
                ),
            ))
        }
    }
}

impl ChargingBackend for UniwillCharging {
    fn get_start_threshold(&self) -> io::Result<u8> {
        // Uniwill doesn't expose numeric thresholds — return 0 (not applicable).
        Ok(0)
    }

    fn set_start_threshold(&self, _pct: u8) -> io::Result<()> {
        // No-op for Uniwill.
        Ok(())
    }

    fn get_end_threshold(&self) -> io::Result<u8> {
        // Uniwill doesn't expose numeric thresholds — return 0 (not applicable).
        Ok(0)
    }

    fn set_end_threshold(&self, _pct: u8) -> io::Result<()> {
        // No-op for Uniwill.
        Ok(())
    }

    fn get_profile(&self) -> io::Result<Option<String>> {
        self.sysfs.read_str("charge_profile").map(Some)
    }

    fn set_profile(&self, profile: &str) -> io::Result<()> {
        Self::validate_profile(profile)?;
        self.sysfs.write_str("charge_profile", profile)
    }

    fn get_priority(&self) -> io::Result<Option<String>> {
        self.sysfs.read_str("charge_priority").map(Some)
    }

    fn set_priority(&self, priority: &str) -> io::Result<()> {
        Self::validate_priority(priority)?;
        self.sysfs.write_str("charge_priority", priority)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tux_core::mock::sysfs::MockSysfs;

    fn setup() -> (MockSysfs, UniwillCharging) {
        let mock = MockSysfs::new();
        let base = mock.platform_dir("tuxedo-uniwill");
        mock.create_attr("devices/platform/tuxedo-uniwill/charge_profile", "balanced");
        mock.create_attr("devices/platform/tuxedo-uniwill/charge_priority", "charge");
        let backend = UniwillCharging::with_path(base);
        (mock, backend)
    }

    #[test]
    fn get_profile() {
        let (_mock, backend) = setup();
        assert_eq!(backend.get_profile().unwrap(), Some("balanced".to_string()));
    }

    #[test]
    fn set_profile_roundtrip() {
        let (_mock, backend) = setup();
        backend.set_profile("high_capacity").unwrap();
        assert_eq!(
            backend.get_profile().unwrap(),
            Some("high_capacity".to_string())
        );
    }

    #[test]
    fn set_profile_stationary() {
        let (_mock, backend) = setup();
        backend.set_profile("stationary").unwrap();
        assert_eq!(
            backend.get_profile().unwrap(),
            Some("stationary".to_string())
        );
    }

    #[test]
    fn invalid_profile_error() {
        let (_mock, backend) = setup();
        let err = backend.set_profile("turbo").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("turbo"));
    }

    #[test]
    fn get_priority() {
        let (_mock, backend) = setup();
        assert_eq!(backend.get_priority().unwrap(), Some("charge".to_string()));
    }

    #[test]
    fn set_priority_roundtrip() {
        let (_mock, backend) = setup();
        backend.set_priority("performance").unwrap();
        assert_eq!(
            backend.get_priority().unwrap(),
            Some("performance".to_string())
        );
    }

    #[test]
    fn invalid_priority_error() {
        let (_mock, backend) = setup();
        let err = backend.set_priority("max_speed").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("max_speed"));
    }

    #[test]
    fn numeric_thresholds_noop() {
        let (_mock, backend) = setup();
        // Uniwill doesn't support numeric thresholds — returns defaults without error.
        assert_eq!(backend.get_start_threshold().unwrap(), 0);
        assert_eq!(backend.get_end_threshold().unwrap(), 0);
        // Set is no-op.
        backend.set_start_threshold(50).unwrap();
        backend.set_end_threshold(80).unwrap();
    }
}
