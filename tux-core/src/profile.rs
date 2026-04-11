//! Profile data model: groups per-scenario settings that can be applied together.

use serde::{Deserialize, Serialize};

use crate::fan_curve::{FanCurvePoint, FanMode};

/// A complete profile grouping all per-scenario hardware settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TuxProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_default: bool,
    pub fan: FanProfileSettings,
    pub cpu: CpuSettings,
    #[serde(default)]
    pub keyboard: KeyboardSettings,
    #[serde(default)]
    pub display: DisplaySettings,
    #[serde(default)]
    pub charging: ChargingSettings,
    #[serde(default)]
    pub odm_profile: Option<String>,
    #[serde(default)]
    pub tdp: Option<TdpSettings>,
    #[serde(default)]
    pub gpu: Option<GpuSettings>,
}

/// Fan control settings within a profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FanProfileSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub mode: FanMode,
    #[serde(default)]
    pub min_speed_percent: u8,
    #[serde(default = "default_max_speed")]
    pub max_speed_percent: u8,
    #[serde(default)]
    pub offset_speed_percent: i8,
    #[serde(default)]
    pub curve: Vec<FanCurvePoint>,
    /// TCC fan profile preset name (e.g. "Quiet", "Silent", "Custom").
    /// Preserved for TCC GUI round-trip; `None` means "Custom".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcc_fan_profile: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_max_speed() -> u8 {
    100
}

impl Default for FanProfileSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: FanMode::Auto,
            min_speed_percent: 0,
            max_speed_percent: 100,
            offset_speed_percent: 0,
            curve: Vec::new(),
            tcc_fan_profile: None,
        }
    }
}

/// CPU governor and turbo settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CpuSettings {
    #[serde(default = "default_governor")]
    pub governor: String,
    #[serde(default)]
    pub energy_performance_preference: Option<String>,
    #[serde(default)]
    pub no_turbo: bool,
    #[serde(default)]
    pub online_cores: Option<i32>,
    #[serde(default)]
    pub use_max_perf_gov: Option<bool>,
    #[serde(default)]
    pub scaling_min_frequency: Option<i32>,
    #[serde(default)]
    pub scaling_max_frequency: Option<i32>,
}

fn default_governor() -> String {
    "powersave".to_string()
}

impl Default for CpuSettings {
    fn default() -> Self {
        Self {
            governor: "powersave".to_string(),
            energy_performance_preference: None,
            no_turbo: false,
            online_cores: None,
            use_max_perf_gov: None,
            scaling_min_frequency: None,
            scaling_max_frequency: None,
        }
    }
}

/// Keyboard backlight settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyboardSettings {
    #[serde(default = "default_kb_brightness")]
    pub brightness: u8,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_kb_mode")]
    pub mode: String,
}

fn default_kb_brightness() -> u8 {
    50
}

fn default_color() -> String {
    "#ffffff".to_string()
}

fn default_kb_mode() -> String {
    "static".to_string()
}

impl Default for KeyboardSettings {
    fn default() -> Self {
        Self {
            brightness: 50,
            color: "#ffffff".to_string(),
            mode: "static".to_string(),
        }
    }
}

/// Display brightness settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DisplaySettings {
    #[serde(default)]
    pub brightness: Option<u8>,
}

/// Battery/charging threshold settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ChargingSettings {
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub start_threshold: Option<u8>,
    #[serde(default)]
    pub end_threshold: Option<u8>,
}

/// TDP (Thermal Design Power) settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TdpSettings {
    pub pl1: Option<u32>,
    pub pl2: Option<u32>,
}

/// GPU power settings (NB02 NVIDIA cTGP control).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GpuSettings {
    #[serde(default)]
    pub ctgp_offset: Option<u8>,
}

/// The four built-in profiles that are always available.
pub fn builtin_profiles() -> Vec<TuxProfile> {
    vec![
        TuxProfile {
            id: "__max_energy_save__".to_string(),
            name: "Max Energy Save".to_string(),
            description: "Minimal power consumption, silent operation".to_string(),
            is_default: true,
            fan: FanProfileSettings {
                enabled: true,
                mode: FanMode::CustomCurve,
                min_speed_percent: 0,
                max_speed_percent: 100,
                curve: vec![
                    FanCurvePoint {
                        temp: 50,
                        speed: 10,
                    },
                    FanCurvePoint {
                        temp: 70,
                        speed: 40,
                    },
                    FanCurvePoint {
                        temp: 85,
                        speed: 70,
                    },
                    FanCurvePoint {
                        temp: 95,
                        speed: 100,
                    },
                ],
                ..Default::default()
            },
            cpu: CpuSettings {
                governor: "powersave".to_string(),
                energy_performance_preference: Some("power".to_string()),
                no_turbo: true,
                ..Default::default()
            },
            keyboard: KeyboardSettings {
                brightness: 30,
                ..KeyboardSettings::default()
            },
            display: DisplaySettings::default(),
            charging: ChargingSettings::default(),
            odm_profile: Some("powersave".to_string()),
            tdp: None,
            gpu: None,
        },
        TuxProfile {
            id: "__quiet__".to_string(),
            name: "Quiet".to_string(),
            description: "Low fan noise, moderate performance".to_string(),
            is_default: true,
            fan: FanProfileSettings {
                enabled: true,
                mode: FanMode::CustomCurve,
                min_speed_percent: 0,
                max_speed_percent: 80,
                curve: vec![
                    FanCurvePoint {
                        temp: 55,
                        speed: 15,
                    },
                    FanCurvePoint {
                        temp: 70,
                        speed: 35,
                    },
                    FanCurvePoint {
                        temp: 85,
                        speed: 60,
                    },
                    FanCurvePoint {
                        temp: 95,
                        speed: 80,
                    },
                ],
                ..Default::default()
            },
            cpu: CpuSettings {
                governor: "powersave".to_string(),
                energy_performance_preference: Some("balance_power".to_string()),
                no_turbo: false,
                ..Default::default()
            },
            keyboard: KeyboardSettings::default(),
            display: DisplaySettings::default(),
            charging: ChargingSettings::default(),
            odm_profile: Some("powersave".to_string()),
            tdp: None,
            gpu: None,
        },
        TuxProfile {
            id: "__office__".to_string(),
            name: "Office".to_string(),
            description: "Balanced performance and power consumption".to_string(),
            is_default: true,
            fan: FanProfileSettings {
                enabled: true,
                mode: FanMode::CustomCurve,
                min_speed_percent: 20,
                max_speed_percent: 100,
                curve: vec![
                    FanCurvePoint {
                        temp: 45,
                        speed: 20,
                    },
                    FanCurvePoint {
                        temp: 65,
                        speed: 45,
                    },
                    FanCurvePoint {
                        temp: 80,
                        speed: 75,
                    },
                    FanCurvePoint {
                        temp: 90,
                        speed: 100,
                    },
                ],
                ..Default::default()
            },
            cpu: CpuSettings {
                governor: "schedutil".to_string(),
                energy_performance_preference: Some("balance_performance".to_string()),
                no_turbo: false,
                ..Default::default()
            },
            keyboard: KeyboardSettings::default(),
            display: DisplaySettings::default(),
            charging: ChargingSettings::default(),
            odm_profile: Some("balanced".to_string()),
            tdp: None,
            gpu: None,
        },
        TuxProfile {
            id: "__high_performance__".to_string(),
            name: "High Performance".to_string(),
            description: "Maximum performance, higher fan noise".to_string(),
            is_default: true,
            fan: FanProfileSettings {
                enabled: true,
                mode: FanMode::CustomCurve,
                min_speed_percent: 30,
                max_speed_percent: 100,
                curve: vec![
                    FanCurvePoint {
                        temp: 40,
                        speed: 30,
                    },
                    FanCurvePoint {
                        temp: 60,
                        speed: 60,
                    },
                    FanCurvePoint {
                        temp: 75,
                        speed: 85,
                    },
                    FanCurvePoint {
                        temp: 85,
                        speed: 100,
                    },
                ],
                ..Default::default()
            },
            cpu: CpuSettings {
                governor: "performance".to_string(),
                energy_performance_preference: Some("performance".to_string()),
                no_turbo: false,
                ..Default::default()
            },
            keyboard: KeyboardSettings {
                brightness: 80,
                ..KeyboardSettings::default()
            },
            display: DisplaySettings::default(),
            charging: ChargingSettings::default(),
            odm_profile: Some("performance".to_string()),
            tdp: None,
            gpu: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_profiles_are_valid() {
        let profiles = builtin_profiles();
        assert_eq!(profiles.len(), 4);
        for p in &profiles {
            assert!(p.is_default, "builtin profile {} should be default", p.id);
            assert!(p.id.starts_with("__"), "builtin IDs should start with __");
            assert!(p.id.ends_with("__"), "builtin IDs should end with __");
        }
    }

    #[test]
    fn toml_roundtrip() {
        for profile in builtin_profiles() {
            let serialized = toml::to_string_pretty(&profile).unwrap();
            let deserialized: TuxProfile = toml::from_str(&serialized).unwrap();
            assert_eq!(profile, deserialized, "roundtrip failed for {}", profile.id);
        }
    }

    #[test]
    fn deserialize_minimal_profile() {
        let toml = r#"
id = "test"
name = "Test Profile"

[fan]
mode = "Auto"

[cpu]
governor = "powersave"
"#;
        let profile: TuxProfile = toml::from_str(toml).unwrap();
        assert_eq!(profile.id, "test");
        assert_eq!(profile.fan.mode, FanMode::Auto);
        assert!(profile.fan.enabled); // default true
        assert_eq!(profile.fan.max_speed_percent, 100); // default
        assert!(!profile.is_default); // default false
    }

    #[test]
    fn builtin_profile_ids() {
        let profiles = builtin_profiles();
        let ids: Vec<&str> = profiles.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"__max_energy_save__"));
        assert!(ids.contains(&"__quiet__"));
        assert!(ids.contains(&"__office__"));
        assert!(ids.contains(&"__high_performance__"));
    }
}
