use std::io;

use tux_core::backend::fan::FanBackend;

use super::sysfs::SysfsReader;

const SYSFS_BASE: &str = "/sys/devices/platform/tuxedo-tuxi";

/// FanBackend for Tuxi platforms (Aura).
///
/// Controls fans via the tuxedo-tuxi kernel shim. PWM is native 0–255.
/// Temperature is reported in tenth-Kelvin by the shim; we convert to °C.
#[derive(Debug)]
pub struct TuxiFanBackend {
    sysfs: SysfsReader,
    num_fans: u8,
}

impl TuxiFanBackend {
    /// Create a new backend, returning `None` if the sysfs directory doesn't exist.
    pub fn new() -> Option<Self> {
        let sysfs = SysfsReader::new(SYSFS_BASE);
        if !sysfs.available() {
            return None;
        }
        let num_fans = sysfs.read_u8("fan_count").unwrap_or(2);
        Some(Self { sysfs, num_fans })
    }

    /// Create a backend with a custom sysfs path (for testing).
    #[cfg(test)]
    pub fn with_path(path: impl Into<std::path::PathBuf>, num_fans: u8) -> Self {
        Self {
            sysfs: SysfsReader::new(path),
            num_fans,
        }
    }

    /// Convert tenth-Kelvin to °C: (val - 2730) / 10.
    fn tenth_kelvin_to_celsius(tk: u16) -> u8 {
        let celsius = (tk.saturating_sub(2730)) / 10;
        celsius.min(255) as u8
    }
}

impl FanBackend for TuxiFanBackend {
    fn read_temp(&self) -> io::Result<u8> {
        // Use fan0's temperature sensor as primary CPU temp.
        let tk = self.sysfs.read_u16("fan0_temp")?;
        Ok(Self::tenth_kelvin_to_celsius(tk))
    }

    fn write_pwm(&self, fan_index: u8, pwm: u8) -> io::Result<()> {
        if fan_index >= self.num_fans {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("fan index {fan_index} out of range (max {})", self.num_fans),
            ));
        }
        let attr = format!("fan{fan_index}_pwm");
        self.sysfs.write_u8(&attr, pwm)?;
        // Ensure manual mode
        self.sysfs.write_u8("fan_mode", 1)
    }

    fn read_pwm(&self, fan_index: u8) -> io::Result<u8> {
        if fan_index >= self.num_fans {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("fan index {fan_index} out of range (max {})", self.num_fans),
            ));
        }
        let attr = format!("fan{fan_index}_pwm");
        self.sysfs.read_u8(&attr)
    }

    fn set_auto(&self, _fan_index: u8) -> io::Result<()> {
        self.sysfs.write_u8("fan_mode", 0)
    }

    fn read_fan_rpm(&self, fan_index: u8) -> io::Result<u16> {
        let attr = format!("fan{fan_index}_rpm");
        self.sysfs.read_u16(&attr)
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

    fn setup() -> (TempDir, TuxiFanBackend) {
        let dir = TempDir::new().unwrap();
        // 25°C = 2980 tenth-Kelvin
        fs::write(dir.path().join("fan0_temp"), "2980\n").unwrap();
        fs::write(dir.path().join("fan1_temp"), "3030\n").unwrap();
        fs::write(dir.path().join("fan0_pwm"), "128\n").unwrap();
        fs::write(dir.path().join("fan1_pwm"), "64\n").unwrap();
        fs::write(dir.path().join("fan0_rpm"), "2400\n").unwrap();
        fs::write(dir.path().join("fan1_rpm"), "2200\n").unwrap();
        fs::write(dir.path().join("fan_count"), "2\n").unwrap();
        fs::write(dir.path().join("fan_mode"), "0\n").unwrap();
        let backend = TuxiFanBackend::with_path(dir.path(), 2);
        (dir, backend)
    }

    #[test]
    fn tenth_kelvin_conversion() {
        // 2730 tenth-K = 0°C
        assert_eq!(TuxiFanBackend::tenth_kelvin_to_celsius(2730), 0);
        // 2980 tenth-K = 25°C
        assert_eq!(TuxiFanBackend::tenth_kelvin_to_celsius(2980), 25);
        // 3730 tenth-K = 100°C
        assert_eq!(TuxiFanBackend::tenth_kelvin_to_celsius(3730), 100);
        // Below absolute zero (saturating)
        assert_eq!(TuxiFanBackend::tenth_kelvin_to_celsius(0), 0);
    }

    #[test]
    fn read_temp_converts() {
        let (_dir, backend) = setup();
        // 2980 tenth-K → 25°C
        assert_eq!(backend.read_temp().unwrap(), 25);
    }

    #[test]
    fn write_pwm_native_scale() {
        let (dir, backend) = setup();
        backend.write_pwm(0, 200).unwrap();
        let pwm = fs::read_to_string(dir.path().join("fan0_pwm")).unwrap();
        assert_eq!(pwm, "200"); // Native 0–255, no conversion
    }

    #[test]
    fn write_pwm_sets_manual_mode() {
        let (dir, backend) = setup();
        backend.write_pwm(0, 128).unwrap();
        let mode = fs::read_to_string(dir.path().join("fan_mode")).unwrap();
        assert_eq!(mode, "1");
    }

    #[test]
    fn set_auto_restores() {
        let (dir, backend) = setup();
        fs::write(dir.path().join("fan_mode"), "1\n").unwrap();
        backend.set_auto(0).unwrap();
        let mode = fs::read_to_string(dir.path().join("fan_mode")).unwrap();
        assert_eq!(mode, "0");
    }

    #[test]
    fn read_rpm() {
        let (_dir, backend) = setup();
        assert_eq!(backend.read_fan_rpm(0).unwrap(), 2400);
        assert_eq!(backend.read_fan_rpm(1).unwrap(), 2200);
    }

    #[test]
    fn num_fans_from_sysfs() {
        let (_dir, backend) = setup();
        assert_eq!(backend.num_fans(), 2);
    }
}
