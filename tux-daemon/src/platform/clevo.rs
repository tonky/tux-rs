use std::io;
use std::sync::Mutex;

use tux_core::backend::fan::FanBackend;

use super::sysfs::SysfsReader;

const SYSFS_BASE: &str = "/sys/devices/platform/tuxedo-clevo";

/// FanBackend for Clevo platforms (Polaris, Stellaris).
///
/// The tuxedo-clevo kernel shim packs per-fan data into u32 values:
///   - `fanN_info` (RO): bits[7:0]=duty, [15:8]=temp_°C, [31:16]=RPM
///   - `fan_speed` (WO): packed u32 with fan0=[7:0], fan1=[15:8], fan2=[23:16]
///   - `fan_auto` (WO): trigger to restore firmware auto mode
pub struct ClevoFanBackend {
    sysfs: SysfsReader,
    max_fans: u8,
    /// Guards the read-modify-write in write_pwm to prevent races.
    write_lock: Mutex<()>,
}

impl std::fmt::Debug for ClevoFanBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClevoFanBackend")
            .field("max_fans", &self.max_fans)
            .finish()
    }
}

impl ClevoFanBackend {
    /// Create a new backend, returning `None` if the sysfs directory doesn't exist.
    pub fn new(max_fans: u8) -> Option<Self> {
        let sysfs = SysfsReader::new(SYSFS_BASE);
        sysfs.available().then_some(Self {
            sysfs,
            max_fans,
            write_lock: Mutex::new(()),
        })
    }

    /// Create a backend with a custom sysfs path (for testing).
    #[cfg(test)]
    pub fn with_path(path: impl Into<std::path::PathBuf>, max_fans: u8) -> Self {
        Self {
            sysfs: SysfsReader::new(path),
            max_fans,
            write_lock: Mutex::new(()),
        }
    }

    /// Parse duty from fan_info u32: bits[7:0].
    fn parse_duty(info: u32) -> u8 {
        (info & 0xFF) as u8
    }

    /// Parse temperature from fan_info u32: bits[15:8].
    fn parse_temp(info: u32) -> u8 {
        ((info >> 8) & 0xFF) as u8
    }

    /// Parse RPM from fan_info u32: bits[31:16].
    fn parse_rpm(info: u32) -> u16 {
        ((info >> 16) & 0xFFFF) as u16
    }

    fn read_fan_info(&self, fan_index: u8) -> io::Result<u32> {
        let attr = format!("fan{fan_index}_info");
        self.sysfs.read_u32(&attr)
    }
}

impl FanBackend for ClevoFanBackend {
    fn read_temp(&self) -> io::Result<u8> {
        // Use fan0's embedded temperature.
        let info = self.read_fan_info(0)?;
        Ok(Self::parse_temp(info))
    }

    fn write_pwm(&self, fan_index: u8, pwm: u8) -> io::Result<()> {
        if fan_index >= self.max_fans {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("fan index {fan_index} out of range (max {})", self.max_fans),
            ));
        }
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| io::Error::other("write lock poisoned"))?;
        // Read current duties for all fans, then set the target one.
        let mut duties = [0u8; 3];
        for i in 0..self.max_fans {
            if i == fan_index {
                duties[i as usize] = pwm;
            } else {
                let info = self.read_fan_info(i)?;
                duties[i as usize] = Self::parse_duty(info);
            }
        }
        let packed: u32 = duties[0] as u32 | (duties[1] as u32) << 8 | (duties[2] as u32) << 16;
        self.sysfs.write_u32("fan_speed", packed)
    }

    fn read_pwm(&self, fan_index: u8) -> io::Result<u8> {
        let info = self.read_fan_info(fan_index)?;
        Ok(Self::parse_duty(info))
    }

    fn set_auto(&self, _fan_index: u8) -> io::Result<()> {
        // Writing any value to fan_auto triggers auto mode restore.
        self.sysfs.write_u8("fan_auto", 1)
    }

    fn read_fan_rpm(&self, fan_index: u8) -> io::Result<u16> {
        let info = self.read_fan_info(fan_index)?;
        Ok(Self::parse_rpm(info))
    }

    fn num_fans(&self) -> u8 {
        self.max_fans
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> (TempDir, ClevoFanBackend) {
        let dir = TempDir::new().unwrap();
        // fan0_info: duty=128, temp=72, rpm=3200 → (3200<<16)|(72<<8)|128 = 209_733_760 + 18_432 + 128
        let info0: u32 = 128 | (72 << 8) | (3200 << 16);
        let info1: u32 = 100 | (65 << 8) | (2800 << 16);
        let info2: u32 = 0;
        fs::write(dir.path().join("fan0_info"), format!("{info0}\n")).unwrap();
        fs::write(dir.path().join("fan1_info"), format!("{info1}\n")).unwrap();
        fs::write(dir.path().join("fan2_info"), format!("{info2}\n")).unwrap();
        fs::write(dir.path().join("fan_speed"), "0\n").unwrap();
        fs::write(dir.path().join("fan_auto"), "0\n").unwrap();
        let backend = ClevoFanBackend::with_path(dir.path(), 3);
        (dir, backend)
    }

    #[test]
    fn parse_fan_info() {
        let info: u32 = 128 | (72 << 8) | (3200 << 16);
        assert_eq!(ClevoFanBackend::parse_duty(info), 128);
        assert_eq!(ClevoFanBackend::parse_temp(info), 72);
        assert_eq!(ClevoFanBackend::parse_rpm(info), 3200);
    }

    #[test]
    fn read_temp_from_fan0() {
        let (_dir, backend) = setup();
        assert_eq!(backend.read_temp().unwrap(), 72);
    }

    #[test]
    fn read_pwm_from_fan_info() {
        let (_dir, backend) = setup();
        assert_eq!(backend.read_pwm(0).unwrap(), 128);
        assert_eq!(backend.read_pwm(1).unwrap(), 100);
    }

    #[test]
    fn read_rpm() {
        let (_dir, backend) = setup();
        assert_eq!(backend.read_fan_rpm(0).unwrap(), 3200);
        assert_eq!(backend.read_fan_rpm(1).unwrap(), 2800);
    }

    #[test]
    fn write_pwm_packs_correctly() {
        let (dir, backend) = setup();
        backend.write_pwm(0, 200).unwrap();
        let packed: u32 = fs::read_to_string(dir.path().join("fan_speed"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        // fan0=200, fan1=100 (preserved from info), fan2=0
        assert_eq!(packed & 0xFF, 200);
        assert_eq!((packed >> 8) & 0xFF, 100);
        assert_eq!((packed >> 16) & 0xFF, 0);
    }

    #[test]
    fn write_pwm_out_of_range_errors() {
        let (_dir, backend) = setup();
        assert!(backend.write_pwm(3, 100).is_err());
    }

    #[test]
    fn set_auto_triggers() {
        let (dir, backend) = setup();
        backend.set_auto(0).unwrap();
        let val = fs::read_to_string(dir.path().join("fan_auto")).unwrap();
        assert_eq!(val, "1");
    }

    #[test]
    fn num_fans() {
        let (_dir, backend) = setup();
        assert_eq!(backend.num_fans(), 3);
    }
}
