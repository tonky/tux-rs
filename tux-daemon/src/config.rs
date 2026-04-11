//! TOML configuration for tux-daemon.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::info;

use tux_core::fan_curve::FanConfig;

/// Default config file location.
pub const DEFAULT_CONFIG_PATH: &str = "/etc/tux-daemon/config.toml";

/// Top-level daemon configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonConfig {
    pub fan: FanConfig,
    pub daemon: DaemonSection,
    pub profiles: ProfileAssignments,
    pub charging: Option<tux_core::profile::ChargingSettings>,
}

/// Profile assignment for AC/battery auto-switching.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileAssignments {
    /// Profile ID to apply when on AC power.
    pub ac_profile: String,
    /// Profile ID to apply when on battery.
    pub battery_profile: String,
}

impl Default for ProfileAssignments {
    fn default() -> Self {
        Self {
            ac_profile: "__office__".to_string(),
            battery_profile: "__quiet__".to_string(),
        }
    }
}

/// General daemon settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonSection {
    /// Log level filter (e.g. "info", "debug", "warn").
    pub log_level: String,
    /// Seconds after last D-Bus client disconnects before switching to idle polling.
    pub idle_timeout_s: u64,
}

impl Default for DaemonSection {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            idle_timeout_s: 30,
        }
    }
}

impl DaemonConfig {
    /// Load configuration from a TOML file. Falls back to defaults on any error.
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => match toml::from_str::<DaemonConfig>(&contents) {
                Ok(config) => {
                    if let Err(e) = config.fan.validate() {
                        tracing::warn!(
                            "invalid fan config in {}: {e}, using defaults",
                            path.display()
                        );
                        return Self::default();
                    }
                    info!("loaded config from {}", path.display());
                    config
                }
                Err(e) => {
                    tracing::warn!(
                        "failed to parse config {}: {e}, using defaults",
                        path.display()
                    );
                    Self::default()
                }
            },
            Err(_) => {
                info!("no config at {}, using defaults", path.display());
                Self::default()
            }
        }
    }

    /// Parse configuration from a TOML string.
    #[cfg(test)]
    fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    /// Save configuration to the specified path.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let serialized = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, serialized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tux_core::fan_curve::FanMode;

    #[test]
    fn default_config_is_valid() {
        let config = DaemonConfig::default();
        assert!(config.fan.validate().is_ok());
        assert_eq!(config.daemon.log_level, "info");
    }

    #[test]
    fn parse_full_toml() {
        let toml = r#"
[fan]
mode = "CustomCurve"
min_speed_percent = 30
active_poll_ms = 1500
idle_poll_ms = 8000
hysteresis_degrees = 2

[[fan.curve]]
temp = 35
speed = 0

[[fan.curve]]
temp = 55
speed = 25

[[fan.curve]]
temp = 75
speed = 70

[[fan.curve]]
temp = 95
speed = 100

[daemon]
log_level = "debug"
"#;
        let config = DaemonConfig::from_toml(toml).unwrap();
        assert_eq!(config.fan.mode, FanMode::CustomCurve);
        assert_eq!(config.fan.min_speed_percent, 30);
        assert_eq!(config.fan.curve.len(), 4);
        assert_eq!(config.fan.curve[0].temp, 35);
        assert_eq!(config.fan.active_poll_ms, 1500);
        assert_eq!(config.daemon.log_level, "debug");
    }

    #[test]
    fn parse_partial_toml_uses_defaults() {
        let toml = r#"
[daemon]
log_level = "warn"
"#;
        let config = DaemonConfig::from_toml(toml).unwrap();
        // Fan section should use defaults
        assert_eq!(config.fan.mode, FanMode::CustomCurve);
        assert_eq!(config.fan.min_speed_percent, 25);
        assert!(!config.fan.curve.is_empty());
        assert_eq!(config.daemon.log_level, "warn");
    }

    #[test]
    fn parse_empty_toml_uses_defaults() {
        let config = DaemonConfig::from_toml("").unwrap();
        assert_eq!(config.fan.mode, FanMode::CustomCurve);
        assert_eq!(config.daemon.log_level, "info");
    }

    #[test]
    fn toml_roundtrip() {
        let original = DaemonConfig::default();
        let serialized = toml::to_string_pretty(&original).unwrap();
        let deserialized = DaemonConfig::from_toml(&serialized).unwrap();
        assert_eq!(original.fan.mode, deserialized.fan.mode);
        assert_eq!(
            original.fan.min_speed_percent,
            deserialized.fan.min_speed_percent
        );
        assert_eq!(original.fan.curve.len(), deserialized.fan.curve.len());
        assert_eq!(original.daemon.log_level, deserialized.daemon.log_level);
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let config = DaemonConfig::load(Path::new("/nonexistent/path/config.toml"));
        assert_eq!(config.fan.mode, FanMode::CustomCurve);
    }

    #[test]
    fn profile_assignments_toml_roundtrip() {
        let original = ProfileAssignments {
            ac_profile: "custom_ac".to_string(),
            battery_profile: "custom_battery".to_string(),
        };
        let toml = format!("[profiles]\n{}", toml::to_string(&original).unwrap());
        let config: DaemonConfig = toml::from_str(&toml).unwrap();
        assert_eq!(config.profiles.ac_profile, "custom_ac");
        assert_eq!(config.profiles.battery_profile, "custom_battery");
    }

    #[test]
    fn profile_assignments_defaults() {
        let config = DaemonConfig::default();
        assert_eq!(config.profiles.ac_profile, "__office__");
        assert_eq!(config.profiles.battery_profile, "__quiet__");
    }
}
