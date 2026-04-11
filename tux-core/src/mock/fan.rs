use std::io;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU16, Ordering};

use crate::backend::fan::FanBackend;

/// In-memory fan backend for testing. Thread-safe via atomics.
pub struct MockFanBackend {
    temp: AtomicU8,
    pwm: Vec<AtomicU8>,
    rpm: Vec<AtomicU16>,
    auto_mode: Vec<AtomicBool>,
    fail_temp: AtomicBool,
    num_fans: u8,
}

impl MockFanBackend {
    pub fn new(num_fans: u8) -> Self {
        let n = num_fans as usize;
        Self {
            temp: AtomicU8::new(40),
            pwm: (0..n).map(|_| AtomicU8::new(0)).collect(),
            rpm: (0..n).map(|_| AtomicU16::new(0)).collect(),
            auto_mode: (0..n).map(|_| AtomicBool::new(true)).collect(),
            fail_temp: AtomicBool::new(false),
            num_fans,
        }
    }

    pub fn set_temp(&self, temp: u8) {
        self.temp.store(temp, Ordering::Relaxed);
    }

    /// When set to true, `read_temp()` will return an error.
    pub fn set_fail_temp(&self, fail: bool) {
        self.fail_temp.store(fail, Ordering::Relaxed);
    }

    pub fn set_rpm(&self, fan: u8, rpm: u16) {
        if let Some(slot) = self.rpm.get(fan as usize) {
            slot.store(rpm, Ordering::Relaxed);
        }
    }

    pub fn is_auto(&self, fan: u8) -> bool {
        self.auto_mode
            .get(fan as usize)
            .map(|a| a.load(Ordering::Relaxed))
            .unwrap_or(false)
    }
}

impl FanBackend for MockFanBackend {
    fn read_temp(&self) -> io::Result<u8> {
        if self.fail_temp.load(Ordering::Relaxed) {
            return Err(io::Error::other("simulated temp read failure"));
        }
        Ok(self.temp.load(Ordering::Relaxed))
    }

    fn write_pwm(&self, fan_index: u8, pwm: u8) -> io::Result<()> {
        let idx = fan_index as usize;
        if idx >= self.num_fans as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "fan index {} out of range (max {})",
                    fan_index, self.num_fans
                ),
            ));
        }
        self.pwm[idx].store(pwm, Ordering::Relaxed);
        self.auto_mode[idx].store(false, Ordering::Relaxed);
        Ok(())
    }

    fn read_pwm(&self, fan_index: u8) -> io::Result<u8> {
        let idx = fan_index as usize;
        if idx >= self.num_fans as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "fan index {} out of range (max {})",
                    fan_index, self.num_fans
                ),
            ));
        }
        Ok(self.pwm[idx].load(Ordering::Relaxed))
    }

    fn set_auto(&self, fan_index: u8) -> io::Result<()> {
        let idx = fan_index as usize;
        if idx >= self.num_fans as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "fan index {} out of range (max {})",
                    fan_index, self.num_fans
                ),
            ));
        }
        self.auto_mode[idx].store(true, Ordering::Relaxed);
        self.pwm[idx].store(0, Ordering::Relaxed);
        Ok(())
    }

    fn read_fan_rpm(&self, fan_index: u8) -> io::Result<u16> {
        let idx = fan_index as usize;
        if idx >= self.num_fans as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "fan index {} out of range (max {})",
                    fan_index, self.num_fans
                ),
            ));
        }
        Ok(self.rpm[idx].load(Ordering::Relaxed))
    }

    fn num_fans(&self) -> u8 {
        self.num_fans
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_pwm_read_back() {
        let mock = MockFanBackend::new(2);
        mock.write_pwm(0, 128).unwrap();
        assert_eq!(mock.read_pwm(0).unwrap(), 128);
        assert_eq!(mock.read_pwm(1).unwrap(), 0);
    }

    #[test]
    fn set_auto_clears_manual() {
        let mock = MockFanBackend::new(1);
        mock.write_pwm(0, 200).unwrap();
        assert!(!mock.is_auto(0));
        mock.set_auto(0).unwrap();
        assert!(mock.is_auto(0));
        assert_eq!(mock.read_pwm(0).unwrap(), 0);
    }

    #[test]
    fn read_temp_returns_configured() {
        let mock = MockFanBackend::new(1);
        assert_eq!(mock.read_temp().unwrap(), 40);
        mock.set_temp(85);
        assert_eq!(mock.read_temp().unwrap(), 85);
    }

    #[test]
    fn fan_index_out_of_range() {
        let mock = MockFanBackend::new(1);
        assert!(mock.write_pwm(1, 100).is_err());
        assert!(mock.read_pwm(2).is_err());
        assert!(mock.set_auto(1).is_err());
        assert!(mock.read_fan_rpm(1).is_err());
    }

    #[test]
    fn num_fans_correct() {
        assert_eq!(MockFanBackend::new(0).num_fans(), 0);
        assert_eq!(MockFanBackend::new(1).num_fans(), 1);
        assert_eq!(MockFanBackend::new(3).num_fans(), 3);
    }

    #[test]
    fn set_rpm_and_read() {
        let mock = MockFanBackend::new(2);
        mock.set_rpm(0, 2500);
        mock.set_rpm(1, 3100);
        assert_eq!(mock.read_fan_rpm(0).unwrap(), 2500);
        assert_eq!(mock.read_fan_rpm(1).unwrap(), 3100);
    }

    #[test]
    fn write_pwm_disables_auto() {
        let mock = MockFanBackend::new(2);
        assert!(mock.is_auto(0));
        assert!(mock.is_auto(1));
        mock.write_pwm(0, 50).unwrap();
        assert!(!mock.is_auto(0));
        assert!(mock.is_auto(1));
    }
}
