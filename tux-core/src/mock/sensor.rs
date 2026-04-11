use std::io;
use std::sync::Mutex;

use crate::backend::sensor::SensorBackend;

/// In-memory mock sensor backend for testing.
pub struct MockSensorBackend {
    temperatures: Mutex<Vec<(String, f32)>>,
    rpms: Mutex<Vec<(String, u16)>>,
}

impl MockSensorBackend {
    pub fn new() -> Self {
        Self {
            temperatures: Mutex::new(Vec::new()),
            rpms: Mutex::new(Vec::new()),
        }
    }

    pub fn set_temperatures(&self, temps: Vec<(String, f32)>) {
        *self.temperatures.lock().unwrap() = temps;
    }

    pub fn set_rpms(&self, rpms: Vec<(String, u16)>) {
        *self.rpms.lock().unwrap() = rpms;
    }
}

impl Default for MockSensorBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl SensorBackend for MockSensorBackend {
    fn read_temperatures(&self) -> io::Result<Vec<(String, f32)>> {
        Ok(self.temperatures.lock().unwrap().clone())
    }

    fn read_fan_rpms(&self) -> io::Result<Vec<(String, u16)>> {
        Ok(self.rpms.lock().unwrap().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_configured_temps_and_rpms() {
        let mock = MockSensorBackend::new();
        mock.set_temperatures(vec![("CPU".into(), 65.5), ("GPU".into(), 72.0)]);
        mock.set_rpms(vec![("Fan 1".into(), 2400), ("Fan 2".into(), 3100)]);

        let temps = mock.read_temperatures().unwrap();
        assert_eq!(temps.len(), 2);
        assert_eq!(temps[0].0, "CPU");
        assert!((temps[0].1 - 65.5).abs() < f32::EPSILON);

        let rpms = mock.read_fan_rpms().unwrap();
        assert_eq!(rpms.len(), 2);
        assert_eq!(rpms[1].1, 3100);
    }

    #[test]
    fn empty_by_default() {
        let mock = MockSensorBackend::default();
        assert!(mock.read_temperatures().unwrap().is_empty());
        assert!(mock.read_fan_rpms().unwrap().is_empty());
    }
}
