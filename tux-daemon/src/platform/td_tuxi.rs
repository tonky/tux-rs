use std::io;
#[cfg(test)]
use std::path::PathBuf;

use tux_core::backend::fan::FanBackend;

use super::sysfs::{
    PWM_ENABLE_AUTO, PWM_ENABLE_MANUAL, SysfsReader, check_fan_index, discover_hwmon, fan_attr,
};

/// sysfs base for tuxedo_tuxi_fan_control platform device.
///
/// Note: the driver registers the platform device with `driver.name = "tuxedo_fan_control"`,
/// not "tuxedo_tuxi_fan_control" — so the path reflects that name.
const FANCTL_SYSFS: &str = "/sys/devices/platform/tuxedo_fan_control";

/// Parent of the hwmon device registered by tuxedo_tuxi_fan_control.
const SENSORS_HWMON_BASE: &str = "/sys/devices/platform/tuxedo_fan_control/hwmon";

/// FanBackend for Tuxi platforms using tuxedo-drivers interfaces.
///
/// Requires:
/// - `tuxedo_tuxi` kernel module: registers a `tuxedo_fan_control` platform device
///   with `fan{N}_pwm` and `fan{N}_pwm_enable` sysfs attributes (1-indexed).
///   PWM is already in Linux standard 0–255 scale (no conversion needed).
/// - hwmon device under the platform device exposes `temp1_input` (millicelsius)
///   and `fan{N}_input` (RPM), conditionally registered when the firmware supports
///   temperature and RPM readback.
///
/// `pwm_enable` semantics (consistent with NB05):
/// - `1` = manual: EC enforces the user-written PWM value.
/// - `2` = auto: EC decides speed itself.
#[derive(Debug)]
pub struct TdTuxiFanBackend {
    fanctl: SysfsReader,
    hwmon: Option<SysfsReader>,
    num_fans: u8,
}

impl TdTuxiFanBackend {
    /// Create a new backend. Returns `None` if the `tuxedo_fan_control` sysfs
    /// directory does not exist (module not loaded).
    pub fn new(num_fans: u8) -> Option<Self> {
        let fanctl = SysfsReader::new(FANCTL_SYSFS);
        if !fanctl.available() || !fanctl.exists("fan1_pwm") {
            return None;
        }
        // hwmon is optional — older Tuxi firmware may not support temp/RPM readback.
        let hwmon = discover_hwmon(SENSORS_HWMON_BASE).map(SysfsReader::new);
        Some(Self {
            fanctl,
            hwmon,
            num_fans,
        })
    }

    /// Create a backend with explicit paths (for unit tests).
    #[cfg(test)]
    pub fn with_paths(
        fanctl_path: impl Into<PathBuf>,
        hwmon_path: Option<impl Into<PathBuf>>,
        num_fans: u8,
    ) -> Self {
        Self {
            fanctl: SysfsReader::new(fanctl_path),
            hwmon: hwmon_path.map(SysfsReader::new),
            num_fans,
        }
    }
}

impl FanBackend for TdTuxiFanBackend {
    fn read_temp(&self) -> io::Result<u8> {
        match &self.hwmon {
            Some(hwmon) => {
                let milli = hwmon.read_u32("temp1_input")?;
                Ok((milli / 1000).min(255) as u8)
            }
            None => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "hwmon not available on this Tuxi firmware",
            )),
        }
    }

    fn write_pwm(&self, fan_index: u8, pwm: u8) -> io::Result<()> {
        check_fan_index(fan_index, self.num_fans)?;
        self.fanctl.write_u8(&fan_attr(fan_index, "pwm"), pwm)?;
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
        match &self.hwmon {
            Some(hwmon) => hwmon.read_u16(&fan_attr(fan_index, "input")),
            // Consistent with read_temp: Unsupported when hwmon is absent.
            None => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "hwmon not available on this Tuxi firmware",
            )),
        }
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

    fn setup_mock(num_fans: u8, with_hwmon: bool) -> (TempDir, TdTuxiFanBackend) {
        let dir = TempDir::new().unwrap();
        let fanctl = dir.path().join("fan_control");
        fs::create_dir_all(&fanctl).unwrap();

        for fan in 0..num_fans {
            let n = fan + 1;
            fs::write(fanctl.join(format!("fan{n}_pwm")), "0\n").unwrap();
            fs::write(fanctl.join(format!("fan{n}_pwm_enable")), "2\n").unwrap();
        }

        let hwmon_path = if with_hwmon {
            let hwmon = dir.path().join("hwmon");
            fs::create_dir_all(&hwmon).unwrap();
            fs::write(hwmon.join("temp1_input"), "52000\n").unwrap();
            for fan in 0..num_fans {
                let n = fan + 1;
                fs::write(hwmon.join(format!("fan{n}_input")), "2000\n").unwrap();
            }
            Some(hwmon)
        } else {
            None
        };

        let backend = TdTuxiFanBackend::with_paths(&fanctl, hwmon_path, num_fans);
        (dir, backend)
    }

    #[test]
    fn read_temp_millicelsius_to_celsius() {
        let (_dir, backend) = setup_mock(2, true);
        assert_eq!(backend.read_temp().unwrap(), 52);
    }

    #[test]
    fn read_temp_returns_error_without_hwmon() {
        let (_dir, backend) = setup_mock(2, false);
        assert!(backend.read_temp().is_err());
    }

    #[test]
    fn write_and_read_pwm() {
        let (_dir, backend) = setup_mock(2, false);
        backend.write_pwm(0, 150).unwrap();
        assert_eq!(backend.read_pwm(0).unwrap(), 150);
    }

    #[test]
    fn write_pwm_sets_manual_enable() {
        let (_dir, backend) = setup_mock(2, false);
        backend.write_pwm(1, 100).unwrap();
        let enable = backend.fanctl.read_u8("fan2_pwm_enable").unwrap();
        assert_eq!(enable, PWM_ENABLE_MANUAL);
    }

    #[test]
    fn set_auto_writes_auto_enable() {
        let (_dir, backend) = setup_mock(2, false);
        backend.write_pwm(0, 200).unwrap();
        backend.set_auto(0).unwrap();
        let enable = backend.fanctl.read_u8("fan1_pwm_enable").unwrap();
        assert_eq!(enable, PWM_ENABLE_AUTO);
    }

    #[test]
    fn read_fan_rpm_with_hwmon() {
        let (_dir, backend) = setup_mock(2, true);
        assert_eq!(backend.read_fan_rpm(0).unwrap(), 2000);
        assert_eq!(backend.read_fan_rpm(1).unwrap(), 2000);
    }

    #[test]
    fn read_fan_rpm_without_hwmon_returns_unsupported() {
        let (_dir, backend) = setup_mock(2, false);
        let err = backend.read_fan_rpm(0).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
    }

    #[test]
    fn out_of_range_fan_index_errors() {
        let (_dir, backend) = setup_mock(2, false);
        assert!(backend.write_pwm(2, 100).is_err());
        assert!(backend.read_pwm(2).is_err());
        assert!(backend.set_auto(2).is_err());
        assert!(backend.read_fan_rpm(2).is_err());
    }

    #[test]
    fn num_fans_reflects_init() {
        let (_dir, b1) = setup_mock(1, false);
        let (_dir, b2) = setup_mock(2, false);
        assert_eq!(b1.num_fans(), 1);
        assert_eq!(b2.num_fans(), 2);
    }
}
