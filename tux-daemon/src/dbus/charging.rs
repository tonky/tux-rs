//! D-Bus Charging interface for battery threshold and profile control.

use std::sync::Arc;
use std::sync::Mutex;

use zbus::interface;

use crate::charging::ChargingBackend;

/// D-Bus object implementing the Charging interface.
pub struct ChargingInterface {
    backend: Option<Arc<dyn ChargingBackend>>,
    daemon_config: std::sync::Arc<std::sync::RwLock<crate::config::DaemonConfig>>,
    last_known: Arc<Mutex<Option<tux_core::profile::ChargingSettings>>>,
}

impl ChargingInterface {
    pub fn new(
        backend: Option<Arc<dyn ChargingBackend>>,
        daemon_config: std::sync::Arc<std::sync::RwLock<crate::config::DaemonConfig>>,
    ) -> Self {
        let last_known = daemon_config.read().ok().and_then(|c| c.charging.clone());
        Self {
            backend,
            daemon_config,
            last_known: Arc::new(Mutex::new(last_known)),
        }
    }

    fn backend(&self) -> zbus::fdo::Result<&Arc<dyn ChargingBackend>> {
        self.backend
            .as_ref()
            .ok_or_else(|| zbus::fdo::Error::Failed("charging hardware not available".into()))
    }

    fn read_settings_once(
        backend: &Arc<dyn ChargingBackend>,
    ) -> Result<tux_core::profile::ChargingSettings, std::io::Error> {
        let start = backend.get_start_threshold()?;
        let end = backend.get_end_threshold()?;
        let profile = backend.get_profile()?;
        let priority = backend.get_priority()?;

        Ok(tux_core::profile::ChargingSettings {
            profile,
            priority,
            start_threshold: if start > 0 { Some(start) } else { None },
            end_threshold: if end > 0 { Some(end) } else { None },
        })
    }

    fn is_transient_io_error(e: &std::io::Error) -> bool {
        e.kind() == std::io::ErrorKind::Other || e.raw_os_error() == Some(5)
    }
}

#[interface(name = "com.tuxedocomputers.tccd.Charging")]
impl ChargingInterface {
    /// Get current charging settings as a TOML string.
    #[zbus(name = "GetChargingSettings")]
    fn get_charging_settings(&self) -> zbus::fdo::Result<String> {
        let backend = self.backend()?;
        // Retry whole settings read to tolerate transient EC/sysfs EIO bursts.
        let mut last_err: Option<std::io::Error> = None;
        let mut settings: Option<tux_core::profile::ChargingSettings> = None;
        for _ in 0..5 {
            match Self::read_settings_once(backend) {
                Ok(s) => {
                    settings = Some(s);
                    break;
                }
                Err(e) if Self::is_transient_io_error(&e) => {
                    last_err = Some(e);
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                Err(e) => {
                    return Err(zbus::fdo::Error::Failed(e.to_string()));
                }
            }
        }
        let settings = match settings {
            Some(s) => s,
            None => {
                let cached = self.last_known.lock().ok().and_then(|g| g.clone());
                if let Some(cached) = cached {
                    tracing::warn!(
                        "charging read failed, returning cached settings: {}",
                        last_err
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_else(|| "unknown error".to_string())
                    );
                    cached
                } else {
                    let config_fallback = self
                        .daemon_config
                        .read()
                        .ok()
                        .and_then(|cfg| cfg.charging.clone())
                        .unwrap_or_default();
                    tracing::warn!(
                        "charging read failed without cache, returning config/default fallback: {}",
                        last_err
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_else(|| "unknown error".to_string())
                    );
                    config_fallback
                }
            }
        };

        if let Ok(mut cache) = self.last_known.lock() {
            *cache = Some(settings.clone());
        }

        toml::to_string(&settings)
            .map_err(|e| zbus::fdo::Error::Failed(format!("serialization error: {e}")))
    }

    /// Apply charging settings from a TOML string.
    #[zbus(name = "SetChargingSettings")]
    fn set_charging_settings(&self, toml_str: &str) -> zbus::fdo::Result<()> {
        let backend = self.backend()?;
        let settings: tux_core::profile::ChargingSettings = toml::from_str(toml_str)
            .map_err(|e| zbus::fdo::Error::Failed(format!("invalid TOML: {e}")))?;

        // Cross-field validation: start threshold must be less than end threshold.
        if let (Some(start), Some(end)) = (settings.start_threshold, settings.end_threshold)
            && start >= end
        {
            return Err(zbus::fdo::Error::Failed(
                "start threshold must be less than end threshold".into(),
            ));
        }

        if let Some(start) = settings.start_threshold {
            backend
                .set_start_threshold(start)
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        }
        if let Some(end) = settings.end_threshold {
            backend
                .set_end_threshold(end)
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        }
        if let Some(ref profile) = settings.profile {
            backend
                .set_profile(profile)
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        }
        if let Some(ref priority) = settings.priority {
            backend
                .set_priority(priority)
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        }

        if let Ok(mut cache) = self.last_known.lock() {
            *cache = Some(settings.clone());
        }

        // Persist globally
        if let Ok(mut config) = self.daemon_config.write() {
            config.charging = Some(settings);
            if let Err(e) = config.save(std::path::Path::new(crate::config::DEFAULT_CONFIG_PATH)) {
                tracing::warn!("failed to save charging settings: {e}");
            }
        }

        Ok(())
    }

    /// Get the start threshold (0–100%), or 0 if unsupported.
    fn get_start_threshold(&self) -> zbus::fdo::Result<u8> {
        self.backend()?
            .get_start_threshold()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Set the start threshold (0–100%).
    fn set_start_threshold(&self, pct: u8) -> zbus::fdo::Result<()> {
        self.backend()?
            .set_start_threshold(pct)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get the end threshold (0–100%), or 0 if unsupported.
    fn get_end_threshold(&self) -> zbus::fdo::Result<u8> {
        self.backend()?
            .get_end_threshold()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Set the end threshold (0–100%).
    fn set_end_threshold(&self, pct: u8) -> zbus::fdo::Result<()> {
        self.backend()?
            .set_end_threshold(pct)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get the charge profile name, or empty string if unsupported.
    fn get_charge_profile(&self) -> zbus::fdo::Result<String> {
        self.backend()?
            .get_profile()
            .map(|opt| opt.unwrap_or_default())
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Set the charge profile name.
    fn set_charge_profile(&self, profile: &str) -> zbus::fdo::Result<()> {
        self.backend()?
            .set_profile(profile)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        if let Ok(mut cache) = self.last_known.lock() {
            let mut s = cache.clone().unwrap_or_default();
            s.profile = Some(profile.to_string());
            *cache = Some(s);
        }
        Ok(())
    }

    /// Get the charge priority, or empty string if unsupported.
    fn get_charge_priority(&self) -> zbus::fdo::Result<String> {
        self.backend()?
            .get_priority()
            .map(|opt| opt.unwrap_or_default())
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Set the charge priority.
    fn set_charge_priority(&self, priority: &str) -> zbus::fdo::Result<()> {
        self.backend()?
            .set_priority(priority)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        if let Ok(mut cache) = self.last_known.lock() {
            let mut s = cache.clone().unwrap_or_default();
            s.priority = Some(priority.to_string());
            *cache = Some(s);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charging::clevo::ClevoCharging;
    use tux_core::mock::sysfs::MockSysfs;

    fn setup_clevo() -> (MockSysfs, ChargingInterface) {
        let mock = MockSysfs::new();
        let base = mock.platform_dir("tuxedo_keyboard");
        mock.create_attr(
            "devices/platform/tuxedo_keyboard/charge_control_start_threshold",
            "40",
        );
        mock.create_attr(
            "devices/platform/tuxedo_keyboard/charge_control_end_threshold",
            "80",
        );
        let backend = ClevoCharging::with_path(base);
        let daemon_config = Arc::new(std::sync::RwLock::new(
            crate::config::DaemonConfig::default(),
        ));
        let iface = ChargingInterface::new(Some(Arc::new(backend)), daemon_config);
        (mock, iface)
    }

    #[test]
    fn get_settings_returns_toml() {
        let (_mock, iface) = setup_clevo();
        let toml_str = iface.get_charging_settings().unwrap();
        let settings: tux_core::profile::ChargingSettings = toml::from_str(&toml_str).unwrap();
        assert_eq!(settings.start_threshold, Some(40));
        assert_eq!(settings.end_threshold, Some(80));
        assert!(settings.profile.is_none());
        assert!(settings.priority.is_none());
    }

    #[test]
    fn set_settings_from_toml() {
        let (_mock, iface) = setup_clevo();
        let toml_str = r#"
start_threshold = 20
end_threshold = 90
"#;
        iface.set_charging_settings(toml_str).unwrap();
        assert_eq!(iface.get_start_threshold().unwrap(), 20);
        assert_eq!(iface.get_end_threshold().unwrap(), 90);
    }

    #[test]
    fn individual_threshold_methods() {
        let (_mock, iface) = setup_clevo();
        assert_eq!(iface.get_start_threshold().unwrap(), 40);
        assert_eq!(iface.get_end_threshold().unwrap(), 80);

        iface.set_start_threshold(30).unwrap();
        iface.set_end_threshold(95).unwrap();
        assert_eq!(iface.get_start_threshold().unwrap(), 30);
        assert_eq!(iface.get_end_threshold().unwrap(), 95);
    }

    #[test]
    fn profile_empty_for_clevo() {
        let (_mock, iface) = setup_clevo();
        assert_eq!(iface.get_charge_profile().unwrap(), "");
        assert_eq!(iface.get_charge_priority().unwrap(), "");
    }

    // --- Uniwill D-Bus tests ---

    fn setup_uniwill() -> (MockSysfs, ChargingInterface) {
        use crate::charging::uniwill::UniwillCharging;
        let mock = MockSysfs::new();
        let base = mock.platform_dir("tuxedo_keyboard");
        mock.create_attr(
            "devices/platform/tuxedo_keyboard/charging_profile/charging_profile",
            "balanced",
        );
        mock.create_attr(
            "devices/platform/tuxedo_keyboard/charging_priority/charging_prio",
            "charge_battery",
        );
        let backend = UniwillCharging::with_path(base);
        let daemon_config = Arc::new(std::sync::RwLock::new(
            crate::config::DaemonConfig::default(),
        ));
        let iface = ChargingInterface::new(Some(Arc::new(backend)), daemon_config);
        (mock, iface)
    }

    #[test]
    fn get_settings_uniwill_profiles() {
        let (_mock, iface) = setup_uniwill();
        let toml_str = iface.get_charging_settings().unwrap();
        let settings: tux_core::profile::ChargingSettings = toml::from_str(&toml_str).unwrap();
        assert_eq!(settings.profile, Some("balanced".to_string()));
        assert_eq!(settings.priority, Some("charge_battery".to_string()));
        // Uniwill returns 0 for thresholds → omitted from TOML.
        assert!(settings.start_threshold.is_none());
        assert!(settings.end_threshold.is_none());
    }

    #[test]
    fn set_settings_uniwill_profile_and_priority() {
        let (_mock, iface) = setup_uniwill();
        let toml_str = r#"
profile = "high_capacity"
priority = "performance"
"#;
        iface.set_charging_settings(toml_str).unwrap();
        assert_eq!(iface.get_charge_profile().unwrap(), "high_capacity");
        assert_eq!(iface.get_charge_priority().unwrap(), "performance");
    }

    #[test]
    fn set_invalid_profile_errors() {
        let (_mock, iface) = setup_uniwill();
        let toml_str = r#"profile = "turbo""#;
        assert!(iface.set_charging_settings(toml_str).is_err());
    }

    #[test]
    fn set_invalid_priority_errors() {
        let (_mock, iface) = setup_uniwill();
        let toml_str = r#"priority = "max_speed""#;
        assert!(iface.set_charging_settings(toml_str).is_err());
    }

    #[test]
    fn uniwill_thresholds_zero() {
        let (_mock, iface) = setup_uniwill();
        assert_eq!(iface.get_start_threshold().unwrap(), 0);
        assert_eq!(iface.get_end_threshold().unwrap(), 0);
    }

    #[test]
    fn set_settings_rejects_start_ge_end() {
        let (_mock, iface) = setup_clevo();
        // start == end
        let toml_str = "start_threshold = 50\nend_threshold = 50\n";
        let err = iface.set_charging_settings(toml_str).unwrap_err();
        assert!(
            err.to_string()
                .contains("start threshold must be less than end threshold")
        );
    }

    #[test]
    fn set_settings_rejects_start_gt_end() {
        let (_mock, iface) = setup_clevo();
        // start > end
        let toml_str = "start_threshold = 80\nend_threshold = 20\n";
        assert!(iface.set_charging_settings(toml_str).is_err());
    }

    #[test]
    fn set_settings_allows_start_lt_end() {
        let (_mock, iface) = setup_clevo();
        let toml_str = "start_threshold = 20\nend_threshold = 80\n";
        iface.set_charging_settings(toml_str).unwrap();
        assert_eq!(iface.get_start_threshold().unwrap(), 20);
        assert_eq!(iface.get_end_threshold().unwrap(), 80);
    }

    #[test]
    fn set_settings_single_threshold_skips_validation() {
        let (_mock, iface) = setup_clevo();
        // Only setting one threshold should not trigger cross-validation.
        let toml_str = "start_threshold = 90\n";
        iface.set_charging_settings(toml_str).unwrap();
        assert_eq!(iface.get_start_threshold().unwrap(), 90);
    }
}
