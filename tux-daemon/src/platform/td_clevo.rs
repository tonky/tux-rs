use std::io;
use std::sync::{Arc, Mutex};

use tux_core::backend::fan::FanBackend;

use super::tuxedo_io::{
    R_CL_FANINFO1, R_CL_FANINFO2, R_CL_FANINFO3, TuxedoIo, W_CL_FANAUTO, W_CL_FANSPEED,
};

/// Maximum number of Clevo fans supported by the tuxedo_io interface.
const CLEVO_MAX_FANS: u8 = 3;

/// FanBackend for Clevo platforms using the `tuxedo_io` ioctl interface.
///
/// Fan info is delivered as a packed `i32` (treated as `u32`):
///   - bits  7:0  — duty (0–255, same scale as Linux PWM)
///   - bits 15:8  — temperature in °C
///   - bits 31:16 — RPM
///
/// Fan speed is written as a packed `i32`:
///   - bits  7:0  — fan 0 speed
///   - bits 15:8  — fan 1 speed
///   - bits 23:16 — fan 2 speed
pub struct TdClevoFanBackend {
    io: Arc<dyn TuxedoIo>,
    max_fans: u8,
    /// Prevents concurrent read-modify-write on fan speed.
    write_lock: Mutex<()>,
}

impl std::fmt::Debug for TdClevoFanBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TdClevoFanBackend")
            .field("max_fans", &self.max_fans)
            .finish()
    }
}

impl TdClevoFanBackend {
    /// Create a backend using the provided ioctl device.
    ///
    /// `max_fans` should come from the device table (typically 2 or 3 for Clevo).
    pub fn new(io: Arc<dyn TuxedoIo>, max_fans: u8) -> Self {
        Self {
            io,
            max_fans: max_fans.min(CLEVO_MAX_FANS),
            write_lock: Mutex::new(()),
        }
    }

    /// Ioctl command for fan info by 0-based index.
    fn faninfo_cmd(fan_index: u8) -> io::Result<u64> {
        match fan_index {
            0 => Ok(R_CL_FANINFO1),
            1 => Ok(R_CL_FANINFO2),
            2 => Ok(R_CL_FANINFO3),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "fan index {fan_index} out of range (max {})",
                    CLEVO_MAX_FANS - 1
                ),
            )),
        }
    }

    fn read_fan_info_raw(&self, fan_index: u8) -> io::Result<u32> {
        let cmd = Self::faninfo_cmd(fan_index)?;
        Ok(self.io.read_i32(cmd)? as u32)
    }

    fn parse_duty(info: u32) -> u8 {
        (info & 0xFF) as u8
    }

    fn parse_temp(info: u32) -> u8 {
        ((info >> 8) & 0xFF) as u8
    }

    fn parse_rpm(info: u32) -> u16 {
        ((info >> 16) & 0xFFFF) as u16
    }
}

impl FanBackend for TdClevoFanBackend {
    fn read_temp(&self) -> io::Result<u8> {
        let info = self.read_fan_info_raw(0)?;
        Ok(Self::parse_temp(info))
    }

    fn write_pwm(&self, fan_index: u8, pwm: u8) -> io::Result<()> {
        if fan_index >= self.max_fans {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("fan index {fan_index} out of range (max {})", self.max_fans),
            ));
        }
        // Guard to prevent concurrent read-modify-write from racing.
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| io::Error::other("write lock poisoned"))?;

        // Read current duties for ALL three fan slots to build the packed argument.
        // We must read all CLEVO_MAX_FANS slots (not just max_fans) to avoid
        // accidentally commanding an unmanaged third fan to minimum speed.
        let mut duties = [0u8; CLEVO_MAX_FANS as usize];
        for i in 0..CLEVO_MAX_FANS {
            if i == fan_index {
                duties[i as usize] = pwm;
            } else {
                // Use the EC's current value; if reading fails (e.g. no third
                // fan), leave slot as 0 — firmware ignores it on 2-fan hardware.
                duties[i as usize] = self.read_fan_info_raw(i).map(Self::parse_duty).unwrap_or(0);
            }
        }
        let packed: i32 = duties[0] as i32 | ((duties[1] as i32) << 8) | ((duties[2] as i32) << 16);
        self.io.write_i32(W_CL_FANSPEED, packed)
    }

    fn read_pwm(&self, fan_index: u8) -> io::Result<u8> {
        if fan_index >= self.max_fans {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("fan index {fan_index} out of range (max {})", self.max_fans),
            ));
        }
        let info = self.read_fan_info_raw(fan_index)?;
        Ok(Self::parse_duty(info))
    }

    fn set_auto(&self, _fan_index: u8) -> io::Result<()> {
        // W_CL_FANAUTO (0x4008_EE11) is a _IOW ioctl that takes an i32 argument.
        // Unlike Uniwill's W_UW_FANAUTO (_IO, no-arg), Clevo requires a value write.
        // Restores all fans to auto regardless of fan_index.
        self.io.write_i32(W_CL_FANAUTO, 0)
    }

    fn read_fan_rpm(&self, fan_index: u8) -> io::Result<u16> {
        if fan_index >= self.max_fans {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("fan index {fan_index} out of range (max {})", self.max_fans),
            ));
        }
        let info = self.read_fan_info_raw(fan_index)?;
        Ok(Self::parse_rpm(info))
    }

    fn num_fans(&self) -> u8 {
        self.max_fans
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::tuxedo_io::MockTuxedoIo;

    /// Fan info packed as: duty=0x32 (50), temp=0x1E (30°C), RPM=0x05DC (1500).
    const FAN0_INFO: i32 = 0x05DC_1E32u32 as i32;
    /// Fan 1 info: duty=0x50 (80), temp=0x20 (32°C), RPM=0x0640 (1600).
    const FAN1_INFO: i32 = 0x0640_2050u32 as i32;

    fn setup_mock(max_fans: u8) -> (Arc<MockTuxedoIo>, TdClevoFanBackend) {
        let mut mock = MockTuxedoIo::new();
        mock.set_read(R_CL_FANINFO1, FAN0_INFO);
        mock.set_read(R_CL_FANINFO2, FAN1_INFO);
        mock.set_read(R_CL_FANINFO3, 0);
        let arc = Arc::new(mock);
        let backend = TdClevoFanBackend::new(arc.clone() as Arc<dyn TuxedoIo>, max_fans);
        (arc, backend)
    }

    #[test]
    fn read_temp_from_fan0() {
        let (_, backend) = setup_mock(2);
        assert_eq!(backend.read_temp().unwrap(), 0x1E); // 30°C
    }

    #[test]
    fn read_pwm_returns_duty() {
        let (_, backend) = setup_mock(2);
        assert_eq!(backend.read_pwm(0).unwrap(), 0x32); // duty=50
        assert_eq!(backend.read_pwm(1).unwrap(), 0x50); // duty=80
    }

    #[test]
    fn read_fan_rpm() {
        let (_, backend) = setup_mock(2);
        assert_eq!(backend.read_fan_rpm(0).unwrap(), 0x05DC); // 1500
        assert_eq!(backend.read_fan_rpm(1).unwrap(), 0x0640); // 1600
    }

    #[test]
    fn write_pwm_sends_packed_fanspeed() {
        let (arc, backend) = setup_mock(2);
        // Set fan1 to 200 (0xC8). Fan0 current duty = 0x32, fan2 = 0.
        backend.write_pwm(1, 0xC8).unwrap();
        let writes = arc.writes.lock().unwrap();
        assert_eq!(writes.len(), 1);
        let (cmd, val) = writes[0];
        assert_eq!(cmd, W_CL_FANSPEED);
        // packed = fan0=0x32 | fan1=0xC8<<8 | fan2=0x00<<16
        let expected = 0x32 | (0xC8 << 8);
        assert_eq!(val, expected);
    }

    #[test]
    fn set_auto_sends_fanauto() {
        let (arc, backend) = setup_mock(2);
        backend.set_auto(0).unwrap();
        let writes = arc.writes.lock().unwrap();
        assert_eq!(writes[0], (W_CL_FANAUTO, 0));
    }

    #[test]
    fn out_of_range_fan_index_errors() {
        let (_, backend) = setup_mock(2);
        assert!(backend.write_pwm(2, 100).is_err());
        assert!(backend.read_pwm(2).is_err());
        assert!(backend.read_fan_rpm(2).is_err());
    }

    #[test]
    fn num_fans_reflects_init() {
        let (_, b2) = setup_mock(2);
        let (_, b3) = setup_mock(3);
        assert_eq!(b2.num_fans(), 2);
        assert_eq!(b3.num_fans(), 3);
    }

    #[test]
    fn read_temp_fails_on_ioctl_error() {
        let arc = Arc::new(MockTuxedoIo::new());
        arc.set_fail_reads(true);
        let backend = TdClevoFanBackend::new(arc as Arc<dyn TuxedoIo>, 2);
        assert!(
            backend.read_temp().is_err(),
            "read_temp should propagate ioctl error"
        );
    }

    #[test]
    fn write_pwm_fails_on_ioctl_write_error() {
        let (arc, backend) = setup_mock(2);
        arc.set_fail_writes(true);
        assert!(
            backend.write_pwm(0, 128).is_err(),
            "write_pwm should propagate write failure"
        );
    }

    #[test]
    fn write_pwm_partial_failure_write_fails_after_read_succeeds() {
        // Read succeeds (fan info available), but write is injected to fail.
        let (arc, backend) = setup_mock(2);
        // Reads are fine, only writes fail.
        arc.set_fail_writes(true);
        // write_pwm does read-modify-write: reads duty from fan info, then writes packed speed.
        let result = backend.write_pwm(1, 0xAA);
        assert!(
            result.is_err(),
            "write_pwm should fail when the write ioctl fails"
        );
    }

    #[test]
    fn set_auto_fails_on_ioctl_write_error() {
        let (arc, backend) = setup_mock(2);
        arc.set_fail_writes(true);
        assert!(
            backend.set_auto(0).is_err(),
            "set_auto should propagate write failure"
        );
    }
}
