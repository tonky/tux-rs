use std::io;
#[cfg(test)]
use std::path::PathBuf;

use tux_core::backend::fan::FanBackend;

use super::sysfs::{SysfsReader, PWM_ENABLE_AUTO, PWM_ENABLE_MANUAL, check_fan_index, discover_hwmon, fan_attr};

/// sysfs base for tuxedo_nb05_fan_control platform device.
const FANCTL_SYSFS: &str = "/sys/devices/platform/tuxedo_nb05_fan_control";

/// hwmon parent directory for tuxedo_nb05_sensors (registered under the platform device).
const SENSORS_HWMON_BASE: &str = "/sys/devices/platform/tuxedo_nb05_sensors/hwmon";

/// FanBackend for NB05 platforms using tuxedo-drivers interfaces.
///
/// Requires:
/// - `tuxedo_nb05_fan_control` kernel module: exposes `fan{N}_pwm` and
///   `fan{N}_pwm_enable` (1-indexed) directly on the platform device.
/// - `tuxedo_nb05_sensors` kernel module: exposes hwmon with `temp1_input`
///   (millicelsius) and `fan{N}_input` (RPM) under the platform device's hwmon dir.
///
/// `pwm_enable` semantics (driver-specific, not standard Linux hwmon):
/// - `1` = manual: EC enforces the user-written PWM value
/// - `2` = auto: EC decides speed itself
#[derive(Debug)]
pub struct TdNb05FanBackend {
    fanctl: SysfsReader,
    hwmon: SysfsReader,
    num_fans: u8,
}

impl TdNb05FanBackend {
    /// Create a new backend. Returns `None` if tuxedo-drivers NB05 sysfs paths
    /// are not present (i.e. the kernel modules are not loaded).
    pub fn new(num_fans: u8) -> Option<Self> {
        let fanctl = SysfsReader::new(FANCTL_SYSFS);
        if !fanctl.available() || !fanctl.exists("fan1_pwm") {
            return None;
        }
        let hwmon_path = discover_hwmon(SENSORS_HWMON_BASE)?;
        Some(Self {
            fanctl,
            hwmon: SysfsReader::new(hwmon_path),
            num_fans,
        })
    }

    /// Create a backend with explicit paths (for unit tests).
    #[cfg(test)]
    pub fn with_paths(
        fanctl_path: impl Into<PathBuf>,
        hwmon_path: impl Into<PathBuf>,
        num_fans: u8,
    ) -> Self {
        Self {
            fanctl: SysfsReader::new(fanctl_path),
            hwmon: SysfsReader::new(hwmon_path),
            num_fans,
        }
    }

}

impl FanBackend for TdNb05FanBackend {
    fn read_temp(&self) -> io::Result<u8> {
        // temp1_input is in millicelsius; clamp to u8.
        let milli = self.hwmon.read_u32("temp1_input")?;
        Ok((milli / 1000).min(255) as u8)
    }

    fn write_pwm(&self, fan_index: u8, pwm: u8) -> io::Result<()> {
        check_fan_index(fan_index, self.num_fans)?;
        self.fanctl
            .write_u8(&fan_attr(fan_index, "pwm"), pwm)?;
        self.fanctl
            .write_u8(&fan_attr(fan_index, "pwm_enable"), PWM_ENABLE_MANUAL)
    }

    fn read_pwm(&self, fan_index: u8) -> io::Result<u8> {
        check_fan_index(fan_index, self.num_fans)?;
        self.fanctl.read_u8(&fan_attr(fan_index, "pwm"))
    }

    fn set_auto(&self, fan_index: u8) -> io::Result<()> {
        check_fan_index(fan_index, self.num_fans)?;
        self.fanctl
            .write_u8(&fan_attr(fan_index, "pwm_enable"), PWM_ENABLE_AUTO)
    }

    fn read_fan_rpm(&self, fan_index: u8) -> io::Result<u16> {
        check_fan_index(fan_index, self.num_fans)?;
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

    /// Build a mock filesystem layout mimicking the two tuxedo_nb05 sysfs dirs.
    fn setup_mock(num_fans: u8) -> (TempDir, TdNb05FanBackend) {
        let dir = TempDir::new().unwrap();
        let fanctl = dir.path().join("fan_control");
        let hwmon = dir.path().join("hwmon");
        fs::create_dir_all(&fanctl).unwrap();
        fs::create_dir_all(&hwmon).unwrap();

        for fan in 0..num_fans {
            let n = fan + 1;
            fs::write(fanctl.join(format!("fan{n}_pwm")), "0\n").unwrap();
            fs::write(fanctl.join(format!("fan{n}_pwm_enable")), "2\n").unwrap();
            fs::write(hwmon.join(format!("fan{n}_input")), "1500\n").unwrap();
        }
        fs::write(hwmon.join("temp1_input"), "45000\n").unwrap();

        let backend = TdNb05FanBackend::with_paths(&fanctl, &hwmon, num_fans);
        (dir, backend)
    }

    #[test]
    fn read_temp_converts_millicelsius() {
        let (_dir, backend) = setup_mock(2);
        assert_eq!(backend.read_temp().unwrap(), 45);
    }

    #[test]
    fn write_and_read_pwm() {
        let (_dir, backend) = setup_mock(2);
        backend.write_pwm(0, 128).unwrap();
        assert_eq!(backend.read_pwm(0).unwrap(), 128);
    }

    #[test]
    fn write_pwm_sets_manual_enable() {
        let (_dir, backend) = setup_mock(2);
        backend.write_pwm(0, 200).unwrap();
        let enable = backend
            .fanctl
            .read_u8("fan1_pwm_enable")
            .unwrap();
        assert_eq!(enable, PWM_ENABLE_MANUAL);
    }

    #[test]
    fn set_auto_writes_auto_enable() {
        let (_dir, backend) = setup_mock(2);
        // First go manual
        backend.write_pwm(0, 200).unwrap();
        // Then release to auto
        backend.set_auto(0).unwrap();
        let enable = backend
            .fanctl
            .read_u8("fan1_pwm_enable")
            .unwrap();
        assert_eq!(enable, PWM_ENABLE_AUTO);
    }

    #[test]
    fn read_fan_rpm_returns_value() {
        let (_dir, backend) = setup_mock(2);
        assert_eq!(backend.read_fan_rpm(0).unwrap(), 1500);
        assert_eq!(backend.read_fan_rpm(1).unwrap(), 1500);
    }

    #[test]
    fn out_of_range_fan_index_errors() {
        let (_dir, backend) = setup_mock(1);
        assert!(backend.write_pwm(1, 128).is_err());
        assert!(backend.read_pwm(1).is_err());
        assert!(backend.set_auto(1).is_err());
        assert!(backend.read_fan_rpm(1).is_err());
    }

    #[test]
    fn num_fans_reflects_init() {
        let (_dir, b1) = setup_mock(1);
        let (_dir, b2) = setup_mock(2);
        assert_eq!(b1.num_fans(), 1);
        assert_eq!(b2.num_fans(), 2);
    }

    #[test]
    fn discover_hwmon_finds_first_directory() {
        let dir = TempDir::new().unwrap();
        let hwmon_dir = dir.path().join("hwmon");
        let hwmon0 = hwmon_dir.join("hwmon3");
        fs::create_dir_all(&hwmon0).unwrap();

        let discovered = discover_hwmon(hwmon_dir.to_str().unwrap());
        assert_eq!(discovered.unwrap(), hwmon0);
    }

    #[test]
    fn discover_hwmon_returns_none_when_missing() {
        let dir = TempDir::new().unwrap();
        let hwmon_path = dir.path().join("hwmon_nonexistent");
        let result = discover_hwmon(hwmon_path.to_str().unwrap());
        assert!(result.is_none());
    }
}
