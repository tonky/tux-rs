use std::io;

/// Hardware abstraction for temperature and fan RPM sensors.
///
/// Provides labeled readings for all available sensors on the platform.
pub trait SensorBackend: Send + Sync {
    /// Read all available temperature sensors. Returns (label, °C) pairs.
    fn read_temperatures(&self) -> io::Result<Vec<(String, f32)>>;

    /// Read all available fan RPM sensors. Returns (label, RPM) pairs.
    fn read_fan_rpms(&self) -> io::Result<Vec<(String, u16)>>;
}
