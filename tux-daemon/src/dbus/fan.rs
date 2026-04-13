//! D-Bus Fan interface: `com.tuxedocomputers.tccd.Fan`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use tokio::sync::watch;
use zbus::interface;

use tux_core::backend::fan::FanBackend;
use tux_core::fan_curve::{FanConfig, FanMode};

use crate::config::ProfileAssignments;
use crate::power_monitor::PowerState;
use crate::profile_store::ProfileStore;

/// D-Bus object implementing the Fan interface.
pub struct FanInterface {
    backend: Arc<dyn FanBackend>,
    config_tx: watch::Sender<FanConfig>,
    config_rx: watch::Receiver<FanConfig>,
    store: Arc<std::sync::RwLock<ProfileStore>>,
    assignments_rx: watch::Receiver<ProfileAssignments>,
    power_rx: watch::Receiver<PowerState>,
    /// Shared counter of consecutive fan-engine temp-read failures.
    failure_counter: Arc<AtomicU32>,
    /// Sends per-fan manual PWM setpoints to the fan engine for re-application
    /// on backends where the EC overrides one-shot writes (e.g. Inwill).
    manual_pwms_tx: watch::Sender<Vec<u8>>,
}

pub struct FanInterfaceDeps {
    pub backend: Arc<dyn FanBackend>,
    pub config_tx: watch::Sender<FanConfig>,
    pub config_rx: watch::Receiver<FanConfig>,
    pub store: Arc<std::sync::RwLock<ProfileStore>>,
    pub assignments_rx: watch::Receiver<ProfileAssignments>,
    pub power_rx: watch::Receiver<PowerState>,
    pub failure_counter: Arc<AtomicU32>,
    pub manual_pwms_tx: watch::Sender<Vec<u8>>,
}

impl FanInterface {
    pub fn new(deps: FanInterfaceDeps) -> Self {
        Self {
            backend: deps.backend,
            config_tx: deps.config_tx,
            config_rx: deps.config_rx,
            store: deps.store,
            assignments_rx: deps.assignments_rx,
            power_rx: deps.power_rx,
            failure_counter: deps.failure_counter,
            manual_pwms_tx: deps.manual_pwms_tx,
        }
    }

    /// Persist the current fan config to the active profile on disk.
    fn persist_to_active_profile(&self, config: &FanConfig) {
        let assignments = self.assignments_rx.borrow();
        let power = *self.power_rx.borrow();
        let active_id = match power {
            PowerState::Ac => &assignments.ac_profile,
            PowerState::Battery => &assignments.battery_profile,
        };
        if let Ok(mut store) = self.store.write() {
            if let Err(e) = store.update_fan_settings(active_id, config) {
                tracing::warn!("failed to persist fan curve to profile '{active_id}': {e}");
            } else {
                tracing::debug!("persisted fan curve to active profile '{active_id}'");
            }
        }
    }
}

impl FanInterface {
    /// Validate fan_index fits in u8 range.
    fn check_fan_index(fan_index: u32) -> zbus::fdo::Result<u8> {
        u8::try_from(fan_index)
            .map_err(|_| zbus::fdo::Error::InvalidArgs("fan_index out of range".into()))
    }
}

#[interface(name = "com.tuxedocomputers.tccd.Fan")]
impl FanInterface {
    /// Write a PWM value (0–255) to a specific fan.
    /// Automatically switches the fan engine to Manual mode so the
    /// engine doesn't override the value on its next tick.
    fn set_fan_speed(&self, fan_index: u32, pwm: u8) -> zbus::fdo::Result<()> {
        let idx = Self::check_fan_index(fan_index)?;
        self.config_tx
            .send_modify(|config| config.mode = FanMode::Manual);
        // Record the per-fan setpoint so the engine can re-apply it on backends
        // that require tick-level re-application (e.g. Inwill EC override workaround).
        self.manual_pwms_tx.send_modify(|pwms| {
            let needed = idx as usize + 1;
            if pwms.len() < needed {
                pwms.resize(needed, 128);
            }
            pwms[idx as usize] = pwm;
        });
        self.backend
            .write_pwm(idx, pwm)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Restore hardware automatic fan control for a fan.
    fn set_auto_mode(&self, fan_index: u32) -> zbus::fdo::Result<()> {
        let idx = Self::check_fan_index(fan_index)?;
        self.backend
            .set_auto(idx)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Read fan speed. Returns RPM if available, otherwise falls back to
    /// PWM percentage (scaled to max_rpm range) for platforms without RPM sensors.
    fn get_fan_speed(&self, fan_index: u32) -> zbus::fdo::Result<u32> {
        let idx = Self::check_fan_index(fan_index)?;
        let rpm = match self.backend.read_fan_rpm(idx) {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::Unsupported => 0,
            Err(e) => return Err(zbus::fdo::Error::Failed(e.to_string())),
        };
        if rpm > 0 {
            return Ok(rpm as u32);
        }
        // Fall back to PWM → synthetic speed so TUI/dashboard isn't stuck at 0.
        let pwm = self
            .backend
            .read_pwm(idx)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        // Map 0–255 PWM to 0–max_rpm range. max_rpm is 6000 in get_fan_info().
        let max_rpm: u32 = 6000;
        Ok((pwm as u32 * max_rpm + 127) / 255)
    }

    /// Read temperature in millidegrees Celsius.
    /// Currently only sensor_index 0 (CPU) is supported.
    fn get_temperature(&self, sensor_index: u32) -> zbus::fdo::Result<i32> {
        if sensor_index > 0 {
            return Err(zbus::fdo::Error::InvalidArgs(
                "only sensor_index 0 (CPU) is supported".into(),
            ));
        }
        self.backend
            .read_temp()
            .map(|t| t as i32 * 1000)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get fan hardware info: (max_rpm, min_rpm, multi_fan, num_fans).
    fn get_fan_info(&self) -> (u32, u32, bool, u8) {
        let n = self.backend.num_fans();
        // Approximate RPM bounds — real values depend on platform firmware.
        (6000, 0, n > 1, n)
    }

    /// Set the fan operating mode: "auto", "manual", or "custom".
    fn set_fan_mode(&self, mode: &str) -> zbus::fdo::Result<()> {
        let fan_mode = match mode {
            "auto" => FanMode::Auto,
            "manual" => FanMode::Manual,
            "custom" | "custom-curve" => FanMode::CustomCurve,
            _ => {
                return Err(zbus::fdo::Error::InvalidArgs(format!(
                    "unknown mode: {mode}"
                )));
            }
        };
        self.config_tx.send_modify(|config| {
            config.mode = fan_mode;
        });
        // Persist mode change to active profile
        let current = self.config_rx.borrow().clone();
        self.persist_to_active_profile(&current);
        Ok(())
    }

    /// Set the fan curve from a TOML-encoded string.
    fn set_fan_curve(&self, toml_str: &str) -> zbus::fdo::Result<()> {
        let new_config: FanConfig =
            toml::from_str(toml_str).map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
        new_config
            .validate()
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
        self.config_tx.send_replace(new_config.clone());
        self.persist_to_active_profile(&new_config);
        Ok(())
    }

    /// Get the current active fan curve as a TOML string.
    fn get_active_fan_curve(&self) -> zbus::fdo::Result<String> {
        let config = self.config_rx.borrow().clone();
        toml::to_string_pretty(&config).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get full telemetry for a single fan as a TOML-encoded `FanData`.
    ///
    /// Returns `duty_percent` (PWM, 0–255) and `rpm_available` so the TUI can
    /// use duty as the authoritative speed indicator on platforms without RPM
    /// sensors (where `read_fan_rpm` always returns 0).
    fn get_fan_data(&self, fan_index: u32) -> zbus::fdo::Result<String> {
        let idx = Self::check_fan_index(fan_index)?;
        let rpm = match self.backend.read_fan_rpm(idx) {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::Unsupported => 0,
            Err(e) => return Err(zbus::fdo::Error::Failed(e.to_string())),
        };
        let duty = self
            .backend
            .read_pwm(idx)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        let temp = self.backend.read_temp().map(|t| t as f32).unwrap_or(0.0);
        let data = tux_core::dbus_types::FanData {
            rpm: rpm as u32,
            temp_celsius: temp,
            duty_percent: duty,
            rpm_available: rpm > 0,
        };
        toml::to_string_pretty(&data).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Fan engine health: status string and consecutive failure count.
    ///
    /// Returns a TOML-encoded `FanHealthResponse`:
    /// - `status = "ok"` when `consecutive_failures < 5`
    /// - `status = "degraded"` when `consecutive_failures` is 5–29
    /// - `status = "failed"` when `consecutive_failures >= 30`
    fn get_fan_health(&self) -> zbus::fdo::Result<String> {
        let failures = self.failure_counter.load(Ordering::Relaxed);
        let status = if failures >= 30 {
            "failed"
        } else if failures >= 5 {
            "degraded"
        } else {
            "ok"
        };
        let health = tux_core::dbus_types::FanHealthResponse {
            status: status.to_string(),
            consecutive_failures: failures,
        };
        toml::to_string_pretty(&health).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Number of fans on this platform.
    #[zbus(property)]
    fn fan_count(&self) -> u32 {
        self.backend.num_fans() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tux_core::mock::fan::MockFanBackend;

    fn make_fan_iface(num_fans: u8) -> (Arc<MockFanBackend>, FanInterface) {
        let backend = Arc::new(MockFanBackend::new(num_fans));
        let (config_tx, config_rx) = watch::channel(FanConfig::default());
        let tmp = tempfile::tempdir().unwrap();
        let store = Arc::new(std::sync::RwLock::new(
            ProfileStore::new(tmp.path()).unwrap(),
        ));
        let (_atx, arx) = watch::channel(ProfileAssignments::default());
        let (_ptx, prx) = watch::channel(PowerState::Ac);
        let failure_counter = Arc::new(AtomicU32::new(0));
        let (manual_pwms_tx, _manual_pwms_rx) = watch::channel(Vec::<u8>::new());
        let iface = FanInterface::new(FanInterfaceDeps {
            backend: backend.clone() as Arc<dyn FanBackend>,
            config_tx,
            config_rx,
            store,
            assignments_rx: arx,
            power_rx: prx,
            failure_counter,
            manual_pwms_tx,
        });
        (backend, iface)
    }

    #[test]
    fn get_fan_speed_returns_rpm_when_nonzero() {
        let (backend, iface) = make_fan_iface(1);
        backend.set_rpm(0, 2500);
        let speed = iface.get_fan_speed(0).unwrap();
        assert_eq!(speed, 2500);
    }

    #[test]
    fn get_fan_speed_falls_back_to_pwm_when_rpm_zero() {
        let (backend, iface) = make_fan_iface(1);
        // RPM is 0 by default, set PWM to simulate active fan.
        backend.write_pwm(0, 128).unwrap();
        let speed = iface.get_fan_speed(0).unwrap();
        // 128 * 6000 / 255 ≈ 3012 (with rounding: (128*6000+127)/255 = 3012)
        assert_eq!(speed, 3012);
    }

    #[test]
    fn get_fan_speed_pwm_zero_returns_zero() {
        let (_backend, iface) = make_fan_iface(1);
        // Both RPM and PWM are 0.
        let speed = iface.get_fan_speed(0).unwrap();
        assert_eq!(speed, 0);
    }

    #[test]
    fn get_fan_speed_pwm_max_returns_max_rpm() {
        let (backend, iface) = make_fan_iface(1);
        backend.write_pwm(0, 255).unwrap();
        let speed = iface.get_fan_speed(0).unwrap();
        // (255 * 6000 + 127) / 255 = 6000
        assert_eq!(speed, 6000);
    }

    #[test]
    fn get_fan_speed_prefers_rpm_over_pwm() {
        let (backend, iface) = make_fan_iface(1);
        backend.set_rpm(0, 3000);
        backend.write_pwm(0, 128).unwrap();
        // Should return RPM, not the PWM fallback.
        let speed = iface.get_fan_speed(0).unwrap();
        assert_eq!(speed, 3000);
    }

    #[test]
    fn get_fan_speed_invalid_index() {
        let (_backend, iface) = make_fan_iface(1);
        assert!(iface.get_fan_speed(1).is_err());
    }

    #[test]
    fn get_fan_info_multi_fan() {
        let (_backend, iface) = make_fan_iface(2);
        let (max_rpm, min_rpm, multi_fan, num_fans) = iface.get_fan_info();
        assert_eq!(max_rpm, 6000);
        assert_eq!(min_rpm, 0);
        assert!(multi_fan);
        assert_eq!(num_fans, 2);
    }

    #[test]
    fn get_fan_info_single_fan() {
        let (_backend, iface) = make_fan_iface(1);
        let (_, _, multi_fan, num_fans) = iface.get_fan_info();
        assert!(!multi_fan);
        assert_eq!(num_fans, 1);
    }

    #[test]
    fn set_fan_speed_writes_pwm() {
        let (backend, iface) = make_fan_iface(2);
        iface.set_fan_speed(0, 200).unwrap();
        assert_eq!(backend.read_pwm(0).unwrap(), 200);
        assert_eq!(backend.read_pwm(1).unwrap(), 0); // untouched
    }

    #[test]
    fn set_auto_mode_restores_auto() {
        let (backend, iface) = make_fan_iface(1);
        iface.set_fan_speed(0, 128).unwrap();
        assert!(!backend.is_auto(0));
        iface.set_auto_mode(0).unwrap();
        assert!(backend.is_auto(0));
    }

    #[test]
    fn get_temperature_sensor_0() {
        let (backend, iface) = make_fan_iface(1);
        backend.set_temp(65);
        let temp = iface.get_temperature(0).unwrap();
        // millidegrees = 65 * 1000
        assert_eq!(temp, 65000);
    }

    #[test]
    fn get_temperature_invalid_sensor() {
        let (_backend, iface) = make_fan_iface(1);
        assert!(iface.get_temperature(1).is_err());
    }

    #[test]
    fn get_fan_data_returns_duty_and_rpm_available() {
        let (backend, iface) = make_fan_iface(1);
        backend.set_rpm(0, 2400);
        backend.write_pwm(0, 128).unwrap();
        backend.set_temp(65);
        let toml_str = iface.get_fan_data(0).unwrap();
        let data: tux_core::dbus_types::FanData = toml::from_str(&toml_str).unwrap();
        assert_eq!(data.rpm, 2400);
        assert_eq!(data.duty_percent, 128);
        assert!(
            data.rpm_available,
            "rpm_available should be true when rpm > 0"
        );
    }

    #[test]
    fn get_fan_data_rpm_not_available_when_zero() {
        let (backend, iface) = make_fan_iface(1);
        // RPM stays 0, duty set.
        backend.write_pwm(0, 100).unwrap();
        let toml_str = iface.get_fan_data(0).unwrap();
        let data: tux_core::dbus_types::FanData = toml::from_str(&toml_str).unwrap();
        assert_eq!(data.rpm, 0);
        assert_eq!(data.duty_percent, 100);
        assert!(
            !data.rpm_available,
            "rpm_available should be false when rpm == 0"
        );
    }

    #[test]
    fn get_fan_data_succeeds_when_rpm_unsupported() {
        // Regression: backends may return Unsupported for read_fan_rpm.
        // get_fan_data must not propagate that as a D-Bus error.
        let (backend, iface) = make_fan_iface(1);
        backend.set_rpm_unsupported(true);
        backend.write_pwm(0, 180).unwrap();
        let toml_str = iface.get_fan_data(0).unwrap();
        let data: tux_core::dbus_types::FanData = toml::from_str(&toml_str).unwrap();
        assert_eq!(data.rpm, 0, "rpm should be 0 when unsupported");
        assert_eq!(data.duty_percent, 180, "duty must still be reported");
        assert!(
            !data.rpm_available,
            "rpm_available must be false when unsupported"
        );
    }

    #[test]
    fn get_fan_health_ok_when_no_failures() {
        let (_backend, iface) = make_fan_iface(1);
        let toml_str = iface.get_fan_health().unwrap();
        let health: tux_core::dbus_types::FanHealthResponse = toml::from_str(&toml_str).unwrap();
        assert_eq!(health.status, "ok");
        assert_eq!(health.consecutive_failures, 0);
    }

    #[test]
    fn get_fan_health_degraded_at_five_failures() {
        let (_backend, iface) = make_fan_iface(1);
        iface.failure_counter.store(5, Ordering::Relaxed);
        let toml_str = iface.get_fan_health().unwrap();
        let health: tux_core::dbus_types::FanHealthResponse = toml::from_str(&toml_str).unwrap();
        assert_eq!(health.status, "degraded");
    }

    #[test]
    fn get_fan_health_failed_at_thirty_failures() {
        let (_backend, iface) = make_fan_iface(1);
        iface.failure_counter.store(30, Ordering::Relaxed);
        let toml_str = iface.get_fan_health().unwrap();
        let health: tux_core::dbus_types::FanHealthResponse = toml::from_str(&toml_str).unwrap();
        assert_eq!(health.status, "failed");
    }

    #[test]
    fn set_fan_mode_custom() {
        let (_backend, iface) = make_fan_iface(1);
        iface.set_fan_mode("custom").unwrap();
        let config = iface.config_rx.borrow();
        assert_eq!(config.mode, FanMode::CustomCurve);
    }

    #[test]
    fn set_fan_mode_auto() {
        let (_backend, iface) = make_fan_iface(1);
        iface.set_fan_mode("auto").unwrap();
        let config = iface.config_rx.borrow();
        assert_eq!(config.mode, FanMode::Auto);
    }

    #[test]
    fn set_fan_mode_invalid() {
        let (_backend, iface) = make_fan_iface(1);
        assert!(iface.set_fan_mode("turbo").is_err());
    }
}
