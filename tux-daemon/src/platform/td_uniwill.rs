use std::io;
use std::sync::Arc;

use tux_core::backend::fan::FanBackend;

use super::tuxedo_io::{
    TuxedoIo, R_UW_FAN_TEMP, R_UW_FANSPEED, R_UW_FANSPEED2, W_UW_FANAUTO, W_UW_FANSPEED,
    W_UW_FANSPEED2,
};

/// Number of fans on Uniwill platforms.
const NUM_FANS: u8 = 2;

/// Uniwill EC fan speed scale maximum (0–200).
const EC_PWM_MAX: u16 = 200;
/// Standard Linux PWM maximum (0–255).
const PWM_MAX: u16 = 255;

/// FanBackend for Uniwill platforms using the `tuxedo_io` ioctl interface.
///
/// The tuxedo_io driver reads/writes fan speed in the EC's native scale (0–200).
/// This backend converts to/from the standard Linux PWM scale (0–255) used by
/// the rest of the daemon, consistent with the legacy sysfs-based `UniwillFanBackend`.
///
/// `W_UW_FANAUTO` is a no-argument ioctl that undo all previously written fan
/// speeds, restoring firmware automatic control for both fans simultaneously.
pub struct TdUniwillFanBackend {
    io: Arc<dyn TuxedoIo>,
}

impl TdUniwillFanBackend {
    /// Create a backend using the provided ioctl device.
    pub fn new(io: Arc<dyn TuxedoIo>) -> Self {
        Self { io }
    }

    /// Convert standard PWM (0–255) to Uniwill EC scale (0–200), rounding to nearest.
    fn pwm_to_ec(pwm: u8) -> i32 {
        ((pwm as u16 * EC_PWM_MAX + PWM_MAX / 2) / PWM_MAX) as i32
    }

    /// Convert Uniwill EC scale (0–200) to standard PWM (0–255), clamped.
    ///
    /// Clamps the incoming EC value to [0, EC_PWM_MAX] before conversion to
    /// defend against hardware glitches or driver bugs returning negative values
    /// (negative i32 → u16 silently wraps, producing wildly wrong PWM).
    fn ec_to_pwm(ec: i32) -> u8 {
        let ec = ec.clamp(0, EC_PWM_MAX as i32) as u16;
        ((ec * PWM_MAX + EC_PWM_MAX / 2) / EC_PWM_MAX) as u8
    }

    fn check_fan_index(fan_index: u8) -> io::Result<()> {
        if fan_index >= NUM_FANS {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("fan index {fan_index} out of range (max {})", NUM_FANS - 1),
            ))
        } else {
            Ok(())
        }
    }
}

impl FanBackend for TdUniwillFanBackend {
    fn read_temp(&self) -> io::Result<u8> {
        // R_UW_FAN_TEMP returns raw °C from EC RAM 0x043e.
        self.io.read_i32(R_UW_FAN_TEMP).map(|v| v.clamp(0, 255) as u8)
    }

    fn write_pwm(&self, fan_index: u8, pwm: u8) -> io::Result<()> {
        Self::check_fan_index(fan_index)?;
        let ec = Self::pwm_to_ec(pwm);
        let cmd = if fan_index == 0 { W_UW_FANSPEED } else { W_UW_FANSPEED2 };
        self.io.write_i32(cmd, ec)
    }

    fn read_pwm(&self, fan_index: u8) -> io::Result<u8> {
        Self::check_fan_index(fan_index)?;
        let cmd = if fan_index == 0 { R_UW_FANSPEED } else { R_UW_FANSPEED2 };
        let ec = self.io.read_i32(cmd)?;
        Ok(Self::ec_to_pwm(ec))
    }

    fn set_auto(&self, _fan_index: u8) -> io::Result<()> {
        // W_UW_FANAUTO restores auto for both fans simultaneously.
        self.io.ioctl_noarg(W_UW_FANAUTO)
    }

    fn read_fan_rpm(&self, _fan_index: u8) -> io::Result<u16> {
        // tuxedo_io does not expose Uniwill fan RPM; return 0 as sentinel.
        Ok(0)
    }

    fn num_fans(&self) -> u8 {
        NUM_FANS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::tuxedo_io::MockTuxedoIo;

    fn setup_mock() -> (Arc<MockTuxedoIo>, TdUniwillFanBackend) {
        let mut mock = MockTuxedoIo::new();
        // Temperature: 55 °C
        mock.set_read(R_UW_FAN_TEMP, 55);
        // Fan 0 speed: EC 100 → PWM ≈ 127
        mock.set_read(R_UW_FANSPEED, 100);
        // Fan 1 speed: EC 50 → PWM ≈ 63
        mock.set_read(R_UW_FANSPEED2, 50);
        let arc = Arc::new(mock);
        let backend = TdUniwillFanBackend::new(arc.clone() as Arc<dyn TuxedoIo>);
        (arc, backend)
    }

    #[test]
    fn read_temp_returns_degrees() {
        let (_, backend) = setup_mock();
        assert_eq!(backend.read_temp().unwrap(), 55);
    }

    #[test]
    fn read_pwm_converts_ec_to_linux_scale() {
        let (_, backend) = setup_mock();
        // EC 100/200 * 255 = 127.5 → rounds to 127 or 128 (within ±1)
        let pwm0 = backend.read_pwm(0).unwrap();
        assert!((127..=128).contains(&pwm0), "expected ~127, got {pwm0}");
        // EC 50/200 * 255 = 63.75 → ~63 or 64
        let pwm1 = backend.read_pwm(1).unwrap();
        assert!((63..=64).contains(&pwm1), "expected ~63, got {pwm1}");
    }

    #[test]
    fn write_pwm_sends_correct_ec_value() {
        let (arc, backend) = setup_mock();
        // PWM 255 → EC 200
        backend.write_pwm(0, 255).unwrap();
        // PWM 0 → EC 0
        backend.write_pwm(1, 0).unwrap();
        let writes = arc.writes.lock().unwrap();
        assert_eq!(writes[0], (W_UW_FANSPEED, 200));
        assert_eq!(writes[1], (W_UW_FANSPEED2, 0));
    }

    #[test]
    fn write_pwm_midpoint_roundtrip() {
        let (arc, backend) = setup_mock();
        backend.write_pwm(0, 128).unwrap();
        let writes = arc.writes.lock().unwrap();
        let ec = writes[0].1;
        // EC ≈ 128 * 200 / 255 = 100 (rounded)
        assert!((99..=101).contains(&ec), "expected ~100, got {ec}");
    }

    #[test]
    fn set_auto_sends_noarg_ioctl() {
        let (arc, backend) = setup_mock();
        backend.set_auto(0).unwrap();
        let calls = arc.noarg_calls.lock().unwrap();
        assert_eq!(calls[0], W_UW_FANAUTO);
    }

    #[test]
    fn read_fan_rpm_returns_zero() {
        let (_, backend) = setup_mock();
        assert_eq!(backend.read_fan_rpm(0).unwrap(), 0);
    }

    #[test]
    fn out_of_range_fan_index_errors() {
        let (_, backend) = setup_mock();
        assert!(backend.write_pwm(2, 100).is_err());
        assert!(backend.read_pwm(2).is_err());
    }

    #[test]
    fn num_fans_is_two() {
        let (_, backend) = setup_mock();
        assert_eq!(backend.num_fans(), 2);
    }

    #[test]
    fn pwm_ec_conversion_boundaries() {
        assert_eq!(TdUniwillFanBackend::pwm_to_ec(0), 0);
        assert_eq!(TdUniwillFanBackend::pwm_to_ec(255), 200);
        assert_eq!(TdUniwillFanBackend::ec_to_pwm(0), 0);
        assert_eq!(TdUniwillFanBackend::ec_to_pwm(200), 255);
    }
}
