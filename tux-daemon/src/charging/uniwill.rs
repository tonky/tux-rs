//! Uniwill EC charging backend — charge profile and priority control via sysfs.
//!
//! The `tuxedo_keyboard` platform device (from tuxedo-drivers) exposes two
//! attribute groups under `/sys/devices/platform/tuxedo_keyboard`:
//!
//!   `charging_profile/charging_profile`  (RW, "high_capacity" | "balanced" | "stationary")
//!   `charging_profile/charging_profiles_available` (RO)
//!   `charging_priority/charging_prio`    (RW, "charge_battery" | "performance")
//!   `charging_priority/charging_prios_available`   (RO)
//!
//! Both attribute groups are conditionally created by the driver; not all
//! Uniwill hardware supports both features.

use std::io;

use super::ChargingBackend;
use crate::platform::sysfs::SysfsReader;
use tracing::info;

const SYSFS_BASE: &str = "/sys/devices/platform/tuxedo_keyboard";

/// Attribute path for charge profile (inside the `charging_profile` subgroup).
const ATTR_PROFILE: &str = "charging_profile/charging_profile";
/// Attribute path for charge priority (inside the `charging_priority` subgroup).
const ATTR_PRIO: &str = "charging_priority/charging_prio";

/// Valid charge profile names (as reported by `charging_profiles_available`).
const VALID_PROFILES: &[&str] = &["high_capacity", "balanced", "stationary"];

/// Valid priority values (as reported by `charging_prios_available`).
const VALID_PRIORITIES: &[&str] = &["charge_battery", "performance"];

/// Uniwill EC charging backend.
///
/// Controls battery charging via named profiles (capacity strategy) and
/// priority (charge-first vs performance-first TDP allocation).
#[derive(Debug)]
pub struct UniwillCharging {
    sysfs: SysfsReader,
}

impl UniwillCharging {
    const IO_RETRY_ATTEMPTS: usize = 10;
    const IO_RETRY_DELAY_MS: u64 = 100;

    /// Create a new backend, returning `None` if the charging sysfs files don't exist.
    pub fn new() -> Option<Self> {
        let sysfs = SysfsReader::new(SYSFS_BASE);
        let has_profile = sysfs.exists(ATTR_PROFILE);
        let has_prio = sysfs.exists(ATTR_PRIO);
        if !has_profile && !has_prio {
            None
        } else {
            // Require at least one readable charging attribute. Some systems expose
            // paths that still return EIO at runtime; treat those as unavailable.
            let profile_ok = has_profile && sysfs.read_str(ATTR_PROFILE).is_ok();
            let prio_ok = has_prio && sysfs.read_str(ATTR_PRIO).is_ok();
            if profile_ok || prio_ok {
                Some(Self { sysfs })
            } else {
                info!(
                    "Uniwill charging attributes present but not readable; disabling charging backend"
                );
                None
            }
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

    fn read_str_retry(&self, attr: &str) -> io::Result<String> {
        let mut last_err: Option<io::Error> = None;
        for _ in 0..Self::IO_RETRY_ATTEMPTS {
            match self.sysfs.read_str(attr) {
                Ok(v) => return Ok(v),
                Err(e) if Self::is_transient_io_error(&e) => {
                    // Transient EIO has been observed on some Uniwill nodes.
                    last_err = Some(e);
                    std::thread::sleep(std::time::Duration::from_millis(Self::IO_RETRY_DELAY_MS));
                }
                Err(e) => return Err(e),
            }
        }
        Err(last_err.unwrap_or_else(|| io::Error::other("charging read failed")))
    }

    fn write_str_retry(&self, attr: &str, value: &str) -> io::Result<()> {
        let mut last_err: Option<io::Error> = None;
        for _ in 0..Self::IO_RETRY_ATTEMPTS {
            match self.sysfs.write_str(attr, value) {
                Ok(()) => return Ok(()),
                Err(e) if Self::is_transient_io_error(&e) => {
                    last_err = Some(e);
                    std::thread::sleep(std::time::Duration::from_millis(Self::IO_RETRY_DELAY_MS));
                }
                Err(e) => return Err(e),
            }
        }
        Err(last_err.unwrap_or_else(|| io::Error::other("charging write failed")))
    }

    fn is_transient_io_error(e: &io::Error) -> bool {
        e.kind() == io::ErrorKind::Other || e.raw_os_error() == Some(5)
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
        self.read_str_retry(ATTR_PROFILE).map(Some)
    }

    fn set_profile(&self, profile: &str) -> io::Result<()> {
        Self::validate_profile(profile)?;
        self.write_str_retry(ATTR_PROFILE, profile)
    }

    fn get_priority(&self) -> io::Result<Option<String>> {
        self.read_str_retry(ATTR_PRIO).map(Some)
    }

    fn set_priority(&self, priority: &str) -> io::Result<()> {
        Self::validate_priority(priority)?;
        self.write_str_retry(ATTR_PRIO, priority)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tux_core::mock::sysfs::MockSysfs;

    fn setup() -> (MockSysfs, UniwillCharging) {
        let mock = MockSysfs::new();
        let base = mock.platform_dir("tuxedo_keyboard");
        mock.create_attr(
            "devices/platform/tuxedo_keyboard/charging_profile/charging_profile",
            "balanced",
        );
        mock.create_attr(
            "devices/platform/tuxedo_keyboard/charging_priority/charging_prio",
            "charge_battery",
        );
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
        assert_eq!(
            backend.get_priority().unwrap(),
            Some("charge_battery".to_string())
        );
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
    fn set_priority_charge_battery() {
        let (_mock, backend) = setup();
        backend.set_priority("charge_battery").unwrap();
        assert_eq!(
            backend.get_priority().unwrap(),
            Some("charge_battery".to_string())
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
