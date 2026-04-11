use std::io;

use tux_core::backend::fan::FanBackend;

use super::sysfs::SysfsReader;

const SYSFS_BASE: &str = "/sys/devices/platform/tuxedo-uniwill";

/// Uniwill EC scale maximum (0–200).
const EC_PWM_MAX: u16 = 200;
/// Standard PWM maximum (0–255).
const PWM_MAX: u16 = 255;
/// Rounding offset for PWM→EC conversion (PWM_MAX / 2).
const PWM_TO_EC_ROUNDING: u16 = 127;
/// Rounding offset for EC→PWM conversion (EC_PWM_MAX / 2).
const EC_TO_PWM_ROUNDING: u16 = 100;

/// FanBackend for Uniwill platforms (InfinityBook).
///
/// Reads temperatures and controls fans via the tuxedo-uniwill kernel shim.
/// Converts standard 0–255 PWM to the Uniwill EC's 0–200 scale.
#[derive(Debug)]
pub struct UniwillFanBackend {
    sysfs: SysfsReader,
}

impl UniwillFanBackend {
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

    /// Convert standard PWM (0–255) to Uniwill EC scale (0–200).
    fn pwm_to_ec(pwm: u8) -> u8 {
        ((pwm as u16 * EC_PWM_MAX + PWM_TO_EC_ROUNDING) / PWM_MAX) as u8
    }

    /// Convert Uniwill EC scale (0–200) to standard PWM (0–255).
    fn ec_to_pwm(ec: u8) -> u8 {
        ((ec as u16 * PWM_MAX + EC_TO_PWM_ROUNDING) / EC_PWM_MAX).min(PWM_MAX) as u8
    }
}

impl FanBackend for UniwillFanBackend {
    fn read_temp(&self) -> io::Result<u8> {
        self.sysfs.read_u8("cpu_temp")
    }

    fn write_pwm(&self, fan_index: u8, pwm: u8) -> io::Result<()> {
        if fan_index >= self.num_fans() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "fan index {fan_index} out of range (max {})",
                    self.num_fans()
                ),
            ));
        }
        // The kernel module atomically handles manual mode entry + burst
        // writes to suppress EC ramp-up, so we just write the duty value.
        let ec_value = Self::pwm_to_ec(pwm);
        let attr = format!("fan{fan_index}_pwm");
        self.sysfs.write_u8(&attr, ec_value)
    }

    fn read_pwm(&self, fan_index: u8) -> io::Result<u8> {
        if fan_index >= self.num_fans() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "fan index {fan_index} out of range (max {})",
                    self.num_fans()
                ),
            ));
        }
        let attr = format!("fan{fan_index}_pwm");
        let ec_value = self.sysfs.read_u8(&attr)?;
        Ok(Self::ec_to_pwm(ec_value))
    }

    fn set_auto(&self, _fan_index: u8) -> io::Result<()> {
        self.sysfs.write_u8("fan_mode", 0)
    }

    fn read_fan_rpm(&self, _fan_index: u8) -> io::Result<u16> {
        // Uniwill shim doesn't expose RPM; return 0.
        Ok(0)
    }

    fn num_fans(&self) -> u8 {
        2
    }

    fn supports_fan_table(&self) -> bool {
        self.sysfs.exists("fan_table")
    }

    fn write_fan_table(&self, zones: &[(u8, u8)]) -> io::Result<()> {
        // Format: pairs of "end_temp speed" values, space-separated.
        let mut s = String::new();
        for (end_temp, speed) in zones {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(&format!("{end_temp} {speed}"));
        }
        self.sysfs.write_str("fan_table", &s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> (TempDir, UniwillFanBackend) {
        let dir = TempDir::new().unwrap();
        // Pre-populate sysfs attributes
        fs::write(dir.path().join("cpu_temp"), "65\n").unwrap();
        fs::write(dir.path().join("fan0_pwm"), "100\n").unwrap();
        fs::write(dir.path().join("fan1_pwm"), "80\n").unwrap();
        fs::write(dir.path().join("fan_mode"), "0\n").unwrap();
        let backend = UniwillFanBackend::with_path(dir.path());
        (dir, backend)
    }

    #[test]
    fn pwm_scale_conversion_max() {
        // PWM 255 → EC 200
        assert_eq!(UniwillFanBackend::pwm_to_ec(255), 200);
    }

    #[test]
    fn pwm_scale_conversion_mid() {
        // PWM 127 → EC 99 (rounded: 127*200+127=25527, /255=100.1 → 100)
        let ec = UniwillFanBackend::pwm_to_ec(127);
        assert!(ec == 99 || ec == 100, "expected ~100, got {ec}");
    }

    #[test]
    fn pwm_scale_conversion_zero() {
        assert_eq!(UniwillFanBackend::pwm_to_ec(0), 0);
    }

    #[test]
    fn ec_to_pwm_roundtrip() {
        // 200 → 255
        assert_eq!(UniwillFanBackend::ec_to_pwm(200), 255);
        // 0 → 0
        assert_eq!(UniwillFanBackend::ec_to_pwm(0), 0);
    }

    #[test]
    fn ec_to_pwm_clamps_out_of_range() {
        // EC value above 200 should still produce a valid PWM (≤255), not wrap.
        let pwm = UniwillFanBackend::ec_to_pwm(201);
        assert!(pwm >= 253, "expected close to 255 for ec=201, got {pwm}");
    }

    #[test]
    fn read_temp() {
        let (_dir, backend) = setup();
        assert_eq!(backend.read_temp().unwrap(), 65);
    }

    #[test]
    fn write_pwm_sets_duty_value() {
        let (dir, backend) = setup();
        backend.write_pwm(0, 255).unwrap();
        let pwm = fs::read_to_string(dir.path().join("fan0_pwm")).unwrap();
        assert_eq!(pwm, "200"); // 255 → 200 EC scale
    }

    #[test]
    fn set_auto_restores_fan_mode() {
        let (dir, backend) = setup();
        fs::write(dir.path().join("fan_mode"), "1\n").unwrap();
        backend.set_auto(0).unwrap();
        let mode = fs::read_to_string(dir.path().join("fan_mode")).unwrap();
        assert_eq!(mode, "0");
    }

    #[test]
    fn read_pwm_converts_ec_to_standard() {
        let (dir, backend) = setup();
        fs::write(dir.path().join("fan0_pwm"), "200\n").unwrap();
        assert_eq!(backend.read_pwm(0).unwrap(), 255);
    }

    #[test]
    fn num_fans_is_two() {
        let (_dir, backend) = setup();
        assert_eq!(backend.num_fans(), 2);
    }

    #[test]
    fn supports_fan_table_when_attr_exists() {
        let (dir, backend) = setup();
        // No fan_table file → not supported.
        assert!(!backend.supports_fan_table());

        // Create the file → supported.
        fs::write(dir.path().join("fan_table"), "").unwrap();
        assert!(backend.supports_fan_table());
    }

    #[test]
    fn write_fan_table_formats_correctly() {
        let (dir, backend) = setup();
        fs::write(dir.path().join("fan_table"), "").unwrap();

        let zones = vec![(45, 40), (65, 80), (80, 120), (100, 180)];
        backend.write_fan_table(&zones).unwrap();

        let written = fs::read_to_string(dir.path().join("fan_table")).unwrap();
        assert_eq!(written, "45 40 65 80 80 120 100 180");
    }
}
