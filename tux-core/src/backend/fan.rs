use std::io;

/// Hardware abstraction for fan control.
///
/// Platform backends (Uniwill, Clevo, NB04, NB05, Tuxi) implement this trait
/// to provide fan speed control and temperature/RPM reading via their respective
/// sysfs interfaces.
pub trait FanBackend: Send + Sync {
    /// Read the primary CPU temperature in °C.
    fn read_temp(&self) -> io::Result<u8>;

    /// Write a PWM duty value (0–255) to a specific fan.
    fn write_pwm(&self, fan_index: u8, pwm: u8) -> io::Result<()>;

    /// Read the current PWM duty value for a specific fan.
    fn read_pwm(&self, fan_index: u8) -> io::Result<u8>;

    /// Restore automatic (firmware-controlled) fan mode for a specific fan.
    fn set_auto(&self, fan_index: u8) -> io::Result<()>;

    /// Read RPM for a specific fan.
    fn read_fan_rpm(&self, fan_index: u8) -> io::Result<u16>;

    /// Number of fans on this platform.
    fn num_fans(&self) -> u8;

    /// Whether the backend requires the engine to re-write manual PWM setpoints
    /// on every idle tick.
    ///
    /// Some EC firmware (notably the Inwill universal fan table controller)
    /// periodically restores its own stored fan table, overriding any one-shot
    /// PWM write within seconds. Backends that experience this should return
    /// `true` so the engine's Manual mode loop re-applies the user's setpoint
    /// on every `idle_poll_ms` tick.
    ///
    /// All other backends (NB05, Tuxi, Clevo, …) should leave this as `false`.
    fn requires_manual_reapply(&self) -> bool {
        false
    }

    /// Whether the backend supports programming a native EC fan table.
    ///
    /// When true, the fan engine can program the curve once and let the
    /// EC enforce it, avoiding per-tick PWM writes.
    fn supports_fan_table(&self) -> bool {
        false
    }

    /// Program the EC's native fan table with zone entries.
    ///
    /// Each entry is `(end_temp_celsius, speed_ec_scale)`. The EC maps
    /// temperature ranges to fan speeds internally — no polling needed.
    fn write_fan_table(&self, zones: &[(u8, u8)]) -> io::Result<()> {
        let _ = zones;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "fan table not supported by this backend",
        ))
    }
}
