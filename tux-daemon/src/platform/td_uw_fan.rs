use std::io;
use std::path::{Path, PathBuf};

use tux_core::backend::fan::FanBackend;

/// Path to the `tuxedo_uw_fan` platform device sysfs directory.
const PLATFORM_PATH: &str = "/sys/devices/platform/tuxedo-uw-fan";

/// EC fan speed scale (0–200) and standard PWM scale (0–255).
const EC_PWM_MAX: u16 = 200;
const PWM_MAX: u16 = 255;

/// `fan_mode` value for automatic (firmware) fan control.
const FAN_MODE_AUTO: &str = "0";
/// `fan_mode` value for manual (software) fan control.
const FAN_MODE_MANUAL: &str = "1";

/// FanBackend for Inwill platforms using the `tuxedo_uw_fan` sysfs interface.
///
/// Exposes `/sys/devices/platform/tuxedo-uw-fan/{fan0_pwm,fan1_pwm,fan_mode,cpu_temp,fan_count}`.
/// Fan speed is in EC native scale (0–200); this backend converts to/from Linux PWM (0–255).
pub struct TdUwFanBackend {
    base: PathBuf,
    num_fans: u8,
}

impl TdUwFanBackend {
    /// Discover the `tuxedo-uw-fan` platform device. Returns `None` if not present.
    pub fn new() -> Option<Self> {
        Self::with_base(Path::new(PLATFORM_PATH))
    }

    /// Constructor with explicit base path (for tests).
    pub fn with_base(base: &Path) -> Option<Self> {
        if !base.exists() {
            return None;
        }
        let fan_count_path = base.join("fan_count");
        let num_fans = if fan_count_path.exists() {
            std::fs::read_to_string(&fan_count_path)
                .ok()
                .and_then(|s| s.trim().parse::<u8>().ok())
                .unwrap_or(2)
        } else {
            2
        };
        Some(Self { base: base.to_path_buf(), num_fans })
    }

    fn fan_pwm_attr(&self, fan_index: u8) -> io::Result<PathBuf> {
        self.check_fan_index(fan_index)?;
        let name = format!("fan{fan_index}_pwm");
        Ok(self.base.join(name))
    }

    fn check_fan_index(&self, fan_index: u8) -> io::Result<()> {
        if fan_index >= self.num_fans {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("fan index {fan_index} out of range (max {})", self.num_fans - 1),
            ))
        } else {
            Ok(())
        }
    }

    fn read_attr(&self, name: &str) -> io::Result<String> {
        let path = self.base.join(name);
        std::fs::read_to_string(&path)
            .map(|s| s.trim().to_string())
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))
    }

    fn write_attr(&self, name: &str, value: &str) -> io::Result<()> {
        let path = self.base.join(name);
        std::fs::write(&path, value)
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))
    }

    /// EC scale (0–200) → Linux PWM (0–255).
    fn ec_to_pwm(ec: u16) -> u8 {
        ((ec.min(EC_PWM_MAX) as u32 * PWM_MAX as u32 + EC_PWM_MAX as u32 / 2)
            / EC_PWM_MAX as u32) as u8
    }

    /// Linux PWM (0–255) → EC scale (0–200).
    fn pwm_to_ec(pwm: u8) -> u16 {
        ((pwm as u32 * EC_PWM_MAX as u32 + PWM_MAX as u32 / 2) / PWM_MAX as u32) as u16
    }
}

impl FanBackend for TdUwFanBackend {
    fn read_temp(&self) -> io::Result<u8> {
        self.read_attr("cpu_temp")?
            .parse::<u8>()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    fn read_pwm(&self, fan_index: u8) -> io::Result<u8> {
        let attr = self.fan_pwm_attr(fan_index)?;
        let name = attr.file_name().unwrap().to_string_lossy().to_string();
        let ec: u16 = self
            .read_attr(&name)?
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(Self::ec_to_pwm(ec))
    }

    fn write_pwm(&self, fan_index: u8, pwm: u8) -> io::Result<()> {
        let attr = self.fan_pwm_attr(fan_index)?;
        let name = attr.file_name().unwrap().to_string_lossy().to_string();
        let ec = Self::pwm_to_ec(pwm);
        self.write_attr("fan_mode", FAN_MODE_MANUAL)?;
        self.write_attr(&name, &ec.to_string())
    }

    fn set_auto(&self, _fan_index: u8) -> io::Result<()> {
        // fan_mode=0 restores auto for all fans simultaneously.
        self.write_attr("fan_mode", FAN_MODE_AUTO)
    }

    fn read_fan_rpm(&self, fan_index: u8) -> io::Result<u16> {
        self.check_fan_index(fan_index)?;
        // tuxedo_uw_fan does not expose RPM.
        Err(io::Error::new(io::ErrorKind::Unsupported, "RPM not available via tuxedo_uw_fan"))
    }

    fn num_fans(&self) -> u8 {
        self.num_fans
    }

    /// The Inwill EC periodically restores its own fan table, overriding any
    /// static PWM write within seconds. Manual mode must re-apply on every tick.
    fn requires_manual_reapply(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_base(fan_count: u8, fan0: u16, fan1: u16, mode: &str, cpu_temp: u8) -> TempDir {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        std::fs::write(p.join("fan_count"), format!("{fan_count}\n")).unwrap();
        std::fs::write(p.join("fan0_pwm"), format!("{fan0}\n")).unwrap();
        std::fs::write(p.join("fan1_pwm"), format!("{fan1}\n")).unwrap();
        std::fs::write(p.join("fan_mode"), format!("{mode}\n")).unwrap();
        std::fs::write(p.join("cpu_temp"), format!("{cpu_temp}\n")).unwrap();
        dir
    }

    #[test]
    fn new_returns_none_when_path_missing() {
        assert!(TdUwFanBackend::with_base(Path::new("/nonexistent/tuxedo-uw-fan")).is_none());
    }

    #[test]
    fn read_temp_returns_cpu_temp() {
        let dir = make_base(2, 60, 80, "0", 52);
        let b = TdUwFanBackend::with_base(dir.path()).unwrap();
        assert_eq!(b.read_temp().unwrap(), 52);
    }

    #[test]
    fn read_pwm_converts_ec_to_linux_scale() {
        // EC 200 → Linux 255
        let dir = make_base(2, 200, 100, "0", 50);
        let b = TdUwFanBackend::with_base(dir.path()).unwrap();
        assert_eq!(b.read_pwm(0).unwrap(), 255);
        // EC 100 → 128 (rounded)
        assert_eq!(b.read_pwm(1).unwrap(), 128);
    }

    #[test]
    fn read_pwm_ec_zero_is_zero() {
        let dir = make_base(2, 0, 0, "0", 45);
        let b = TdUwFanBackend::with_base(dir.path()).unwrap();
        assert_eq!(b.read_pwm(0).unwrap(), 0);
    }

    #[test]
    fn write_pwm_sets_manual_mode_and_ec_value() {
        let dir = make_base(2, 0, 0, "0", 45);
        let b = TdUwFanBackend::with_base(dir.path()).unwrap();
        b.write_pwm(0, 128).unwrap();
        let mode = std::fs::read_to_string(dir.path().join("fan_mode")).unwrap();
        assert_eq!(mode.trim(), "1");
        let val: u16 = std::fs::read_to_string(dir.path().join("fan0_pwm"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        // PWM 128 → EC ≈ 100
        assert_eq!(val, 100);
    }

    #[test]
    fn write_pwm_max_gives_ec_200() {
        let dir = make_base(2, 0, 0, "0", 45);
        let b = TdUwFanBackend::with_base(dir.path()).unwrap();
        b.write_pwm(0, 255).unwrap();
        let val: u16 = std::fs::read_to_string(dir.path().join("fan0_pwm"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(val, 200);
    }

    #[test]
    fn set_auto_writes_mode_zero() {
        let dir = make_base(2, 100, 100, "1", 55);
        let b = TdUwFanBackend::with_base(dir.path()).unwrap();
        b.set_auto(0).unwrap();
        let mode = std::fs::read_to_string(dir.path().join("fan_mode")).unwrap();
        assert_eq!(mode.trim(), "0");
    }

    #[test]
    fn read_fan_rpm_returns_unsupported() {
        let dir = make_base(2, 60, 80, "0", 50);
        let b = TdUwFanBackend::with_base(dir.path()).unwrap();
        assert_eq!(b.read_fan_rpm(0).unwrap_err().kind(), io::ErrorKind::Unsupported);
    }

    #[test]
    fn out_of_range_fan_index_errors() {
        let dir = make_base(2, 60, 80, "0", 50);
        let b = TdUwFanBackend::with_base(dir.path()).unwrap();
        assert_eq!(b.read_pwm(2).unwrap_err().kind(), io::ErrorKind::InvalidInput);
        assert_eq!(b.write_pwm(2, 128).unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn num_fans_reads_from_fan_count() {
        let dir = make_base(2, 0, 0, "0", 40);
        let b = TdUwFanBackend::with_base(dir.path()).unwrap();
        assert_eq!(b.num_fans(), 2);
    }

    #[test]
    fn pwm_to_ec_roundtrip() {
        for pwm in [0u8, 64, 128, 192, 255] {
            let ec = TdUwFanBackend::pwm_to_ec(pwm);
            let back = TdUwFanBackend::ec_to_pwm(ec);
            assert!((back as i16 - pwm as i16).abs() <= 1, "pwm={pwm} ec={ec} back={back}");
        }
    }
}
