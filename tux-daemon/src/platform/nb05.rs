use std::io;

use tux_core::backend::fan::FanBackend;
use tux_core::dmi::DetectedDevice;
use tux_core::registers::PlatformRegisters;

use super::sysfs::SysfsReader;

const SYSFS_BASE: &str = "/sys/devices/platform/tuxedo-ec";
const EC_RAM_ATTR: &str = "ec_ram";

// EC register addresses — temperature
const EC_CPU_TEMP: u64 = 0x0470;

// EC register addresses — RPM (big-endian u16)
const EC_FAN0_RPM_HI: u64 = 0x0298;
const EC_FAN0_RPM_LO: u64 = 0x0299;
const EC_FAN1_RPM_HI: u64 = 0x0218;
const EC_FAN1_RPM_LO: u64 = 0x0219;

// --- "ranges" mode (Pulse 2-fan) ---
// Fan 1 (CPU) enable: bit 0 at 0x2c0
const RANGES_FAN0_ENABLE: u64 = 0x02C0;
// Fan 1 duty curve: 7 breakpoints at 0x02C1..0x02C7 + high-temp at 0x02C8..0x02C9
const RANGES_FAN0_DUTY_BASE: u64 = 0x02C1;
// Fan 2 (System) enable: bit 0 at 0x240
const RANGES_FAN1_ENABLE: u64 = 0x0240;
// Fan 2 duty curve base
const RANGES_FAN1_DUTY_BASE: u64 = 0x0241;

// --- "onereg" mode (InfinityFlex 1-fan) ---
const ONEREG_ENABLE: u64 = 0x02F1;
const ONEREG_ENABLE_ON: u8 = 0xAA;
const ONEREG_ENABLE_OFF: u8 = 0x00;
const ONEREG_DUTY: u64 = 0x1809;

/// Maximum EC duty value.
const EC_DUTY_MAX: u16 = 0xB8; // 184

/// Number of duty curve breakpoints in "ranges" mode.
const RANGES_CURVE_POINTS: usize = 7;

/// FanBackend for NB05 platforms (Pulse, InfinityFlex).
///
/// Accesses the EC directly via binary pread/pwrite on the tuxedo-ec `ec_ram` attribute.
/// Two sub-variants:
///   - **ranges** (Pulse, 2-fan): multi-register duty curves per fan
///   - **onereg** (InfinityFlex, 1-fan): single duty register
#[derive(Debug)]
pub struct Nb05FanBackend {
    sysfs: SysfsReader,
    num_fans: u8,
    onereg: bool,
}

impl Nb05FanBackend {
    /// Create a new backend from a detected device, returning `None` if the
    /// sysfs directory doesn't exist.
    pub fn new(device: &DetectedDevice) -> Option<Self> {
        let sysfs = SysfsReader::new(SYSFS_BASE);
        if !sysfs.available() || !sysfs.exists(EC_RAM_ATTR) {
            return None;
        }
        let (num_fans, onereg) = match &device.descriptor.registers {
            PlatformRegisters::Nb05(regs) => (regs.num_fans, regs.fanctl_onereg),
            _ => (1, false),
        };
        Some(Self {
            sysfs,
            num_fans,
            onereg,
        })
    }

    /// Create a backend with a custom sysfs path (for testing).
    #[cfg(test)]
    pub fn with_path(path: impl Into<std::path::PathBuf>, num_fans: u8, onereg: bool) -> Self {
        Self {
            sysfs: SysfsReader::new(path),
            num_fans,
            onereg,
        }
    }

    /// Read a single byte from the EC RAM at the given address.
    fn ec_read(&self, addr: u64) -> io::Result<u8> {
        let buf = self.sysfs.pread(EC_RAM_ATTR, addr, 1)?;
        Ok(buf[0])
    }

    /// Write a single byte to the EC RAM at the given address.
    fn ec_write(&self, addr: u64, val: u8) -> io::Result<()> {
        self.sysfs.pwrite(EC_RAM_ATTR, addr, &[val])
    }

    /// Convert standard PWM (0–255) to EC duty (0–184).
    fn pwm_to_duty(pwm: u8) -> u8 {
        ((pwm as u16 * EC_DUTY_MAX + 127) / 255) as u8
    }

    /// Convert EC duty (0–184) to standard PWM (0–255).
    fn duty_to_pwm(duty: u8) -> u8 {
        ((duty as u16 * 255 + 92) / EC_DUTY_MAX).min(255) as u8
    }

    /// Write the duty curve for a fan in "ranges" mode.
    /// Sets all 7 breakpoints + 2 high-temp values to the same duty.
    fn write_ranges_fan(&self, fan_index: u8, duty: u8) -> io::Result<()> {
        let (enable_reg, duty_base) = match fan_index {
            0 => (RANGES_FAN0_ENABLE, RANGES_FAN0_DUTY_BASE),
            1 => (RANGES_FAN1_ENABLE, RANGES_FAN1_DUTY_BASE),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("fan index {fan_index} out of range"),
                ));
            }
        };

        // Write all 7 breakpoints to the same duty
        for i in 0..RANGES_CURVE_POINTS {
            self.ec_write(duty_base + i as u64, duty)?;
        }
        // Write high-temp duty (2 registers after the 7 breakpoints)
        self.ec_write(duty_base + RANGES_CURVE_POINTS as u64, duty)?;
        self.ec_write(duty_base + RANGES_CURVE_POINTS as u64 + 1, duty)?;

        // Enable the fan
        self.ec_write(enable_reg, 0x01)?;
        Ok(())
    }

    /// Read the first breakpoint duty for a fan in "ranges" mode.
    fn read_ranges_fan_duty(&self, fan_index: u8) -> io::Result<u8> {
        let duty_base = match fan_index {
            0 => RANGES_FAN0_DUTY_BASE,
            1 => RANGES_FAN1_DUTY_BASE,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("fan index {fan_index} out of range"),
                ));
            }
        };
        self.ec_read(duty_base)
    }

    /// Read RPM for a specific fan (big-endian u16).
    fn read_rpm_registers(&self, fan_index: u8) -> io::Result<u16> {
        let (hi_reg, lo_reg) = match fan_index {
            0 => (EC_FAN0_RPM_HI, EC_FAN0_RPM_LO),
            1 => (EC_FAN1_RPM_HI, EC_FAN1_RPM_LO),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("fan index {fan_index} out of range"),
                ));
            }
        };
        let hi = self.ec_read(hi_reg)? as u16;
        let lo = self.ec_read(lo_reg)? as u16;
        Ok((hi << 8) | lo)
    }
}

impl FanBackend for Nb05FanBackend {
    fn read_temp(&self) -> io::Result<u8> {
        self.ec_read(EC_CPU_TEMP)
    }

    fn write_pwm(&self, fan_index: u8, pwm: u8) -> io::Result<()> {
        let duty = Self::pwm_to_duty(pwm);
        if self.onereg {
            if fan_index != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "onereg mode only supports fan 0",
                ));
            }
            self.ec_write(ONEREG_DUTY, duty)?;
            self.ec_write(ONEREG_ENABLE, ONEREG_ENABLE_ON)
        } else {
            self.write_ranges_fan(fan_index, duty)
        }
    }

    fn read_pwm(&self, fan_index: u8) -> io::Result<u8> {
        let duty = if self.onereg {
            if fan_index != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "onereg mode only supports fan 0",
                ));
            }
            self.ec_read(ONEREG_DUTY)?
        } else {
            self.read_ranges_fan_duty(fan_index)?
        };
        Ok(Self::duty_to_pwm(duty))
    }

    fn set_auto(&self, fan_index: u8) -> io::Result<()> {
        if self.onereg {
            self.ec_write(ONEREG_ENABLE, ONEREG_ENABLE_OFF)
        } else {
            let enable_reg = match fan_index {
                0 => RANGES_FAN0_ENABLE,
                1 => RANGES_FAN1_ENABLE,
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("fan index {fan_index} out of range"),
                    ));
                }
            };
            self.ec_write(enable_reg, 0x00)
        }
    }

    fn read_fan_rpm(&self, fan_index: u8) -> io::Result<u16> {
        self.read_rpm_registers(fan_index)
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

    /// Create a mock ec_ram binary file of 64K.
    fn create_ec_ram(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join(EC_RAM_ATTR);
        let data = vec![0u8; 0x10000];
        fs::write(&path, &data).unwrap();
        path
    }

    fn set_ec_byte(dir: &std::path::Path, addr: u64, val: u8) {
        use std::io::{Seek, SeekFrom, Write};
        let path = dir.join(EC_RAM_ATTR);
        let mut f = fs::OpenOptions::new().write(true).open(&path).unwrap();
        f.seek(SeekFrom::Start(addr)).unwrap();
        f.write_all(&[val]).unwrap();
    }

    fn get_ec_byte(dir: &std::path::Path, addr: u64) -> u8 {
        use std::io::{Read, Seek, SeekFrom};
        let path = dir.join(EC_RAM_ATTR);
        let mut f = fs::File::open(&path).unwrap();
        f.seek(SeekFrom::Start(addr)).unwrap();
        let mut buf = [0u8; 1];
        f.read_exact(&mut buf).unwrap();
        buf[0]
    }

    fn setup_ranges() -> (TempDir, Nb05FanBackend) {
        let dir = TempDir::new().unwrap();
        create_ec_ram(dir.path());
        // Set CPU temp to 65°C
        set_ec_byte(dir.path(), EC_CPU_TEMP, 65);
        // Set Fan 0 RPM to 3200 (big-endian: 0x0C80)
        set_ec_byte(dir.path(), EC_FAN0_RPM_HI, 0x0C);
        set_ec_byte(dir.path(), EC_FAN0_RPM_LO, 0x80);
        let backend = Nb05FanBackend::with_path(dir.path(), 2, false);
        (dir, backend)
    }

    fn setup_onereg() -> (TempDir, Nb05FanBackend) {
        let dir = TempDir::new().unwrap();
        create_ec_ram(dir.path());
        set_ec_byte(dir.path(), EC_CPU_TEMP, 50);
        let backend = Nb05FanBackend::with_path(dir.path(), 1, true);
        (dir, backend)
    }

    #[test]
    fn pwm_to_duty_conversion() {
        assert_eq!(Nb05FanBackend::pwm_to_duty(255), 184); // max
        assert_eq!(Nb05FanBackend::pwm_to_duty(0), 0); // min
        // Mid-range: 127 → ~92
        let mid = Nb05FanBackend::pwm_to_duty(127);
        assert!((91..=92).contains(&mid), "expected ~92, got {mid}");
    }

    #[test]
    fn duty_to_pwm_conversion() {
        assert_eq!(Nb05FanBackend::duty_to_pwm(184), 255); // max
        assert_eq!(Nb05FanBackend::duty_to_pwm(0), 0); // min
    }

    #[test]
    fn duty_to_pwm_clamps_out_of_range() {
        // EC duty above 184 should clamp to 255, not wrap to 0.
        let pwm = Nb05FanBackend::duty_to_pwm(185);
        assert!(pwm >= 253, "expected close to 255 for duty=185, got {pwm}");
        // Extreme case
        let pwm = Nb05FanBackend::duty_to_pwm(255);
        assert_eq!(pwm, 255);
    }

    #[test]
    fn read_temp_ranges() {
        let (_dir, backend) = setup_ranges();
        assert_eq!(backend.read_temp().unwrap(), 65);
    }

    #[test]
    fn read_rpm_ranges() {
        let (_dir, backend) = setup_ranges();
        assert_eq!(backend.read_fan_rpm(0).unwrap(), 3200);
    }

    #[test]
    fn write_pwm_ranges_mode() {
        let (dir, backend) = setup_ranges();
        backend.write_pwm(0, 255).unwrap();

        // All 7 breakpoints + 2 high-temp should be 184
        for i in 0..9 {
            let val = get_ec_byte(dir.path(), RANGES_FAN0_DUTY_BASE + i);
            assert_eq!(val, 184, "breakpoint {i} should be 184");
        }
        // Enable register should be 1
        assert_eq!(get_ec_byte(dir.path(), RANGES_FAN0_ENABLE), 1);
    }

    #[test]
    fn read_pwm_ranges_mode() {
        let (dir, backend) = setup_ranges();
        // Set first breakpoint to 92 (EC duty)
        set_ec_byte(dir.path(), RANGES_FAN0_DUTY_BASE, 92);
        let pwm = backend.read_pwm(0).unwrap();
        // 92 EC → ~127 PWM
        assert!((126..=128).contains(&pwm), "expected ~127, got {pwm}");
    }

    #[test]
    fn set_auto_ranges_disables() {
        let (dir, backend) = setup_ranges();
        set_ec_byte(dir.path(), RANGES_FAN0_ENABLE, 1);
        backend.set_auto(0).unwrap();
        assert_eq!(get_ec_byte(dir.path(), RANGES_FAN0_ENABLE), 0);
    }

    #[test]
    fn write_pwm_onereg_mode() {
        let (dir, backend) = setup_onereg();
        backend.write_pwm(0, 255).unwrap();
        assert_eq!(get_ec_byte(dir.path(), ONEREG_DUTY), 184);
        assert_eq!(get_ec_byte(dir.path(), ONEREG_ENABLE), ONEREG_ENABLE_ON);
    }

    #[test]
    fn read_pwm_onereg_mode() {
        let (dir, backend) = setup_onereg();
        set_ec_byte(dir.path(), ONEREG_DUTY, 184);
        assert_eq!(backend.read_pwm(0).unwrap(), 255);
    }

    #[test]
    fn set_auto_onereg_disables() {
        let (dir, backend) = setup_onereg();
        set_ec_byte(dir.path(), ONEREG_ENABLE, ONEREG_ENABLE_ON);
        backend.set_auto(0).unwrap();
        assert_eq!(get_ec_byte(dir.path(), ONEREG_ENABLE), ONEREG_ENABLE_OFF);
    }

    #[test]
    fn onereg_rejects_fan1() {
        let (_dir, backend) = setup_onereg();
        assert!(backend.write_pwm(1, 100).is_err());
        assert!(backend.read_pwm(1).is_err());
    }

    #[test]
    fn num_fans_ranges() {
        let (_dir, backend) = setup_ranges();
        assert_eq!(backend.num_fans(), 2);
    }

    #[test]
    fn num_fans_onereg() {
        let (_dir, backend) = setup_onereg();
        assert_eq!(backend.num_fans(), 1);
    }
}
