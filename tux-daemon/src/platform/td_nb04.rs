use std::io;
use std::path::PathBuf;

use tux_core::backend::fan::FanBackend;

use super::sysfs::{SysfsReader, check_fan_index, discover_hwmon, fan_attr};

/// hwmon parent directory for tuxedo_nb04_sensors.
const SENSORS_HWMON_BASE: &str = "/sys/devices/platform/tuxedo_nb04_sensors/hwmon";

/// sysfs base for tuxedo_nb04_power_profiles platform device.
const POWER_PROFILES_SYSFS: &str = "/sys/devices/platform/tuxedo_nb04_power_profiles";

/// NB04 firmware power profiles, exposed via `platform_profile` sysfs attribute.
///
/// Writing one of these strings selects the corresponding firmware thermal policy,
/// which indirectly controls fan speed. There is no direct PWM control on NB04.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nb04Profile {
    LowPower,
    Balanced,
    Performance,
}

impl Nb04Profile {
    fn as_str(self) -> &'static str {
        match self {
            Nb04Profile::LowPower => "low-power",
            Nb04Profile::Balanced => "balanced",
            Nb04Profile::Performance => "performance",
        }
    }
}

/// FanBackend for NB04 platforms using tuxedo-drivers interfaces.
///
/// NB04 (Sirius series) does **not** expose raw PWM fan control via tuxedo-drivers.
/// Fan speed is managed exclusively through firmware power profiles.
///
/// This backend provides:
/// - Temperature and RPM reading via `tuxedo_nb04_sensors` hwmon.
/// - Power profile selection via `tuxedo_nb04_power_profiles` sysfs.
///
/// `write_pwm` / `set_auto` return `Unsupported` — the fan engine must not be
/// started for NB04 (return `None` from `init_fan_backend`). This backend exists
/// to support future temperature/RPM telemetry paths and power profile control.
#[derive(Debug)]
pub struct TdNb04FanBackend {
    hwmon: SysfsReader,
    power_profiles: SysfsReader,
    num_fans: u8,
}

impl TdNb04FanBackend {
    /// Create a new backend. Returns `None` if the hwmon directory is absent
    /// (tuxedo_nb04_sensors not loaded).
    pub fn new(num_fans: u8) -> Option<Self> {
        let hwmon_path = discover_hwmon(SENSORS_HWMON_BASE)?;
        let power_profiles = SysfsReader::new(POWER_PROFILES_SYSFS);
        Some(Self {
            hwmon: SysfsReader::new(hwmon_path),
            power_profiles,
            num_fans,
        })
    }

    /// Create a backend with explicit paths (for unit tests).
    #[cfg(test)]
    pub fn with_paths(
        hwmon_path: impl Into<PathBuf>,
        power_profiles_path: impl Into<PathBuf>,
        num_fans: u8,
    ) -> Self {
        Self {
            hwmon: SysfsReader::new(hwmon_path),
            power_profiles: SysfsReader::new(power_profiles_path),
            num_fans,
        }
    }

    fn check_fan_index(&self, fan_index: u8) -> io::Result<()> {
        check_fan_index(fan_index, self.num_fans)
    }

    /// Set the NB04 power profile.
    pub fn set_profile(&self, profile: Nb04Profile) -> io::Result<()> {
        self.power_profiles
            .write_str("platform_profile", profile.as_str())
    }

    /// Read the current NB04 power profile.
    pub fn get_profile(&self) -> io::Result<Nb04Profile> {
        let s = self.power_profiles.read_str("platform_profile")?;
        match s.as_str() {
            "low-power" => Ok(Nb04Profile::LowPower),
            "balanced" => Ok(Nb04Profile::Balanced),
            "performance" => Ok(Nb04Profile::Performance),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown platform_profile value: {other}"),
            )),
        }
    }
}

impl FanBackend for TdNb04FanBackend {
    fn read_temp(&self) -> io::Result<u8> {
        // temp1_input is CPU temperature in millicelsius.
        let milli = self.hwmon.read_u32("temp1_input")?;
        Ok((milli / 1000).min(255) as u8)
    }

    fn write_pwm(&self, _fan_index: u8, _pwm: u8) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "NB04 does not support direct PWM fan control; use power profiles",
        ))
    }

    fn read_pwm(&self, _fan_index: u8) -> io::Result<u8> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "NB04 does not expose fan PWM; use power profiles",
        ))
    }

    fn set_auto(&self, fan_index: u8) -> io::Result<()> {
        // Validate fan_index for API consistency even though the operation
        // switches the global power profile and affects all fans.
        self.check_fan_index(fan_index)?;
        // Best effort: switch to balanced profile, which is the firmware default.
        self.set_profile(Nb04Profile::Balanced)
    }

    fn read_fan_rpm(&self, fan_index: u8) -> io::Result<u16> {
        self.check_fan_index(fan_index)?;
        // hwmon fan attributes: fan1_input, fan2_input (1-indexed, plain RPM).
        self.hwmon.read_u16(&fan_attr(fan_index, "input"))
    }

    fn num_fans(&self) -> u8 {
        self.num_fans
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_mock(num_fans: u8) -> (TempDir, TdNb04FanBackend) {
        let dir = TempDir::new().unwrap();
        let hwmon = dir.path().join("hwmon");
        let profiles = dir.path().join("power_profiles");
        fs::create_dir_all(&hwmon).unwrap();
        fs::create_dir_all(&profiles).unwrap();

        fs::write(hwmon.join("temp1_input"), "62000\n").unwrap();
        for fan in 0..num_fans {
            let n = fan + 1;
            fs::write(hwmon.join(format!("fan{n}_input")), "3000\n").unwrap();
        }
        fs::write(profiles.join("platform_profile"), "balanced\n").unwrap();

        let backend = TdNb04FanBackend::with_paths(&hwmon, &profiles, num_fans);
        (dir, backend)
    }

    #[test]
    fn read_temp_converts_millicelsius() {
        let (_dir, backend) = setup_mock(2);
        assert_eq!(backend.read_temp().unwrap(), 62);
    }

    #[test]
    fn read_fan_rpm_returns_value() {
        let (_dir, backend) = setup_mock(2);
        assert_eq!(backend.read_fan_rpm(0).unwrap(), 3000);
        assert_eq!(backend.read_fan_rpm(1).unwrap(), 3000);
    }

    #[test]
    fn write_pwm_returns_unsupported() {
        let (_dir, backend) = setup_mock(2);
        let err = backend.write_pwm(0, 128).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
    }

    #[test]
    fn read_pwm_returns_unsupported() {
        let (_dir, backend) = setup_mock(2);
        let err = backend.read_pwm(0).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
    }

    #[test]
    fn set_auto_writes_balanced_profile() {
        let (_dir, backend) = setup_mock(2);
        backend.set_auto(0).unwrap();
        assert_eq!(backend.get_profile().unwrap(), Nb04Profile::Balanced);
    }

    #[test]
    fn set_and_get_profile_roundtrip() {
        let (_dir, backend) = setup_mock(2);
        backend.set_profile(Nb04Profile::Performance).unwrap();
        assert_eq!(backend.get_profile().unwrap(), Nb04Profile::Performance);
        backend.set_profile(Nb04Profile::LowPower).unwrap();
        assert_eq!(backend.get_profile().unwrap(), Nb04Profile::LowPower);
    }

    #[test]
    fn out_of_range_fan_index_errors() {
        let (_dir, backend) = setup_mock(2);
        assert!(backend.read_fan_rpm(2).is_err());
        // set_auto validates fan_index even though it's a global operation.
        assert!(backend.set_auto(2).is_err());
    }

    #[test]
    fn num_fans_reflects_init() {
        let (_dir, backend) = setup_mock(2);
        assert_eq!(backend.num_fans(), 2);
    }

    #[test]
    fn unknown_profile_returns_error() {
        let (_dir, backend) = setup_mock(2);
        // Write an invalid value directly
        backend
            .power_profiles
            .write_str("platform_profile", "turbo")
            .unwrap();
        assert!(backend.get_profile().is_err());
    }
}
