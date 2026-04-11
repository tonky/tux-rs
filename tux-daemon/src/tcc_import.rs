//! TCC profile/settings import: handles `--new_profiles` and `--new_settings`
//! CLI flags used by the TCC GUI's pkexec-based config save flow.
//!
//! When called with these flags, the binary reads TCC JSON temp files,
//! converts them to TuxProfile TOML, writes to our profiles directory,
//! and exits.

use std::env;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use tux_core::fan_curve::{FanCurvePoint, FanMode};
use tux_core::profile::{
    ChargingSettings, CpuSettings, DisplaySettings, FanProfileSettings, GpuSettings,
    KeyboardSettings, TdpSettings, TuxProfile,
};

const PROFILES_DIR: &str = "/etc/tux-daemon/profiles";
const SETTINGS_PATH: &str = "/etc/tux-daemon/config.toml";

/// Check if we were invoked in import mode.
pub fn is_import_mode() -> bool {
    let args: Vec<String> = env::args().collect();
    args.iter()
        .any(|a| a == "--new_profiles" || a == "--new_settings")
}

/// Run the import: read TCC JSON, convert, write TOML, exit.
pub fn run_import() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    let profiles_path = get_arg_value(&args, "--new_profiles");
    let settings_path = get_arg_value(&args, "--new_settings");

    if let Some(path) = &profiles_path {
        import_profiles(path)?;
    }

    if let Some(path) = &settings_path {
        import_settings(path)?;
    }

    Ok(())
}

fn get_arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(|s| s.trim().replace('\'', ""))
}

// ── TCC JSON types (subset for deserialization) ─────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TccProfile {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    display: Option<TccDisplay>,
    #[serde(default)]
    cpu: Option<TccCpu>,
    #[serde(default)]
    fan: Option<TccFanControl>,
    #[serde(default)]
    odm_profile: Option<TccOdmProfile>,
    #[serde(default)]
    odm_power_limits: Option<TccOdmPowerLimits>,
    #[serde(default, rename = "nvidiaPowerCTRLProfile")]
    nvidia_power_ctrl_profile: Option<TccNvidiaPowerCtrl>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TccDisplay {
    brightness: i32,
    use_brightness: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TccCpu {
    governor: String,
    #[serde(default)]
    energy_performance_preference: Option<String>,
    #[serde(default)]
    no_turbo: bool,
    #[serde(default)]
    online_cores: Option<i32>,
    #[serde(default)]
    use_max_perf_gov: Option<bool>,
    #[serde(default)]
    scaling_min_frequency: Option<i32>,
    #[serde(default)]
    scaling_max_frequency: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct TccFanControl {
    use_control: bool,
    #[serde(default)]
    fan_profile: Option<String>,
    #[serde(default)]
    minimum_fanspeed: i32,
    #[serde(default)]
    maximum_fanspeed: i32,
    #[serde(default)]
    offset_fanspeed: i32,
    #[serde(default)]
    custom_fan_curve: Option<TccFanProfile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TccFanProfile {
    #[serde(default, rename = "tableCPU")]
    table_cpu: Option<Vec<TccFanTableEntry>>,
}

#[derive(Deserialize)]
struct TccFanTableEntry {
    temp: i32,
    speed: i32,
}

#[derive(Deserialize)]
struct TccOdmProfile {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TccOdmPowerLimits {
    tdp_values: Vec<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TccNvidiaPowerCtrl {
    #[serde(rename = "cTGPOffset")]
    ctgp_offset: i32,
}

// ── TCC settings type ──────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct TccSettings {
    #[serde(default)]
    state_map: Option<TccStateMap>,
    #[serde(default)]
    charging_profile: Option<String>,
    #[serde(default)]
    charging_priority: Option<String>,
}

#[derive(Deserialize)]
struct TccStateMap {
    #[serde(default)]
    power_ac: Option<String>,
    #[serde(default)]
    power_bat: Option<String>,
}

// ── Conversion ─────────────────────────────────────────────────────

fn tcc_to_profile(tcc: TccProfile) -> TuxProfile {
    let fan = tcc.fan.as_ref();
    let cpu = tcc.cpu.as_ref();
    let display = tcc.display.as_ref();

    let curve = fan
        .and_then(|f| f.custom_fan_curve.as_ref())
        .and_then(|c| c.table_cpu.as_ref())
        .map(|table| {
            table
                .iter()
                .map(|e| FanCurvePoint {
                    temp: e.temp.clamp(0, 100) as u8,
                    speed: e.speed.clamp(0, 100) as u8,
                })
                .collect()
        })
        .unwrap_or_default();

    let fan_mode = if fan.is_some_and(|f| f.use_control) {
        FanMode::CustomCurve
    } else {
        FanMode::Auto
    };

    let tdp = tcc.odm_power_limits.and_then(|l| {
        if l.tdp_values.is_empty() {
            None
        } else {
            Some(TdpSettings {
                pl1: l.tdp_values.first().map(|v| *v as u32),
                pl2: l.tdp_values.get(1).map(|v| *v as u32),
            })
        }
    });

    let gpu = tcc.nvidia_power_ctrl_profile.and_then(|g| {
        if g.ctgp_offset == 0 {
            None
        } else {
            Some(GpuSettings {
                ctgp_offset: Some(g.ctgp_offset.clamp(0, 255) as u8),
            })
        }
    });

    TuxProfile {
        id: tcc.id,
        name: tcc.name,
        description: tcc.description,
        is_default: false,
        fan: FanProfileSettings {
            enabled: fan.is_some_and(|f| f.use_control),
            mode: fan_mode,
            min_speed_percent: fan
                .map(|f| f.minimum_fanspeed.clamp(0, 100) as u8)
                .unwrap_or(0),
            max_speed_percent: fan
                .map(|f| f.maximum_fanspeed.clamp(0, 100) as u8)
                .unwrap_or(100),
            offset_speed_percent: fan
                .map(|f| f.offset_fanspeed.clamp(-100, 100) as i8)
                .unwrap_or(0),
            curve,
            tcc_fan_profile: fan.and_then(|f| f.fan_profile.clone()),
        },
        cpu: cpu
            .map(|c| CpuSettings {
                governor: c.governor.clone(),
                energy_performance_preference: c.energy_performance_preference.clone(),
                no_turbo: c.no_turbo,
                online_cores: c.online_cores,
                use_max_perf_gov: c.use_max_perf_gov,
                scaling_min_frequency: c.scaling_min_frequency,
                scaling_max_frequency: c.scaling_max_frequency,
            })
            .unwrap_or_default(),
        keyboard: KeyboardSettings::default(),
        display: display
            .map(|d| DisplaySettings {
                brightness: if d.use_brightness {
                    Some(d.brightness.clamp(0, 100) as u8)
                } else {
                    None
                },
            })
            .unwrap_or_default(),
        charging: ChargingSettings::default(),
        odm_profile: tcc.odm_profile.map(|o| o.name).filter(|n| !n.is_empty()),
        tdp,
        gpu,
    }
}

// ── Import logic ───────────────────────────────────────────────────

fn import_profiles(json_path: &str) -> anyhow::Result<()> {
    let json = fs::read_to_string(json_path)?;
    let tcc_profiles: Vec<TccProfile> = serde_json::from_str(&json)?;

    fs::create_dir_all(PROFILES_DIR)?;

    // Get builtin profile IDs to skip them.
    let builtins: std::collections::HashSet<String> = tux_core::profile::builtin_profiles()
        .into_iter()
        .map(|p| p.id)
        .collect();

    for tcc in tcc_profiles {
        if builtins.contains(&tcc.id) {
            continue; // Don't overwrite built-in profiles.
        }
        let profile = tcc_to_profile(tcc);
        let toml_str = toml::to_string_pretty(&profile)?;
        let file_path = Path::new(PROFILES_DIR).join(format!("{}.toml", profile.id));
        fs::write(&file_path, toml_str)?;
        eprintln!(
            "tcc-import: wrote profile '{}' to {}",
            profile.name,
            file_path.display()
        );
    }

    // Clean up temp file.
    let _ = fs::remove_file(json_path);

    // Signal the running daemon to reload profiles.
    signal_daemon_reload();
    Ok(())
}

fn import_settings(json_path: &str) -> anyhow::Result<()> {
    let json = fs::read_to_string(json_path)?;
    let settings: TccSettings = serde_json::from_str(&json)?;

    // Update profile assignments in daemon config.
    if let Some(state_map) = settings.state_map {
        let config_path = Path::new(SETTINGS_PATH);
        let mut config_str = fs::read_to_string(config_path).unwrap_or_default();

        if let Some(ac) = state_map.power_ac {
            config_str = update_toml_field(&config_str, "profiles", "ac_profile", &ac);
        }
        if let Some(bat) = state_map.power_bat {
            config_str = update_toml_field(&config_str, "profiles", "battery_profile", &bat);
        }
        fs::write(config_path, config_str)?;
        eprintln!("tcc-import: updated settings at {}", config_path.display());
    }

    // Clean up temp file.
    let _ = fs::remove_file(json_path);
    Ok(())
}

fn update_toml_field(content: &str, section: &str, key: &str, value: &str) -> String {
    // Parse existing TOML, update field, re-serialize.
    if let Ok(mut doc) = content.parse::<toml::Value>() {
        if let Some(table) = doc.as_table_mut() {
            let sec = table
                .entry(section)
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
            if let Some(sec_table) = sec.as_table_mut() {
                sec_table.insert(key.to_string(), toml::Value::String(value.to_string()));
            }
        }
        return toml::to_string_pretty(&doc).unwrap_or_else(|_| content.to_string());
    }
    content.to_string()
}

/// Send SIGHUP to the running tux-daemon to trigger profile reload.
fn signal_daemon_reload() {
    use std::process::Command;
    // Find the running daemon PID from systemd.
    if let Ok(output) = Command::new("systemctl")
        .args(["show", "-p", "MainPID", "--value", "tux-daemon.service"])
        .output()
        && let Ok(pid_str) = String::from_utf8(output.stdout)
    {
        let pid_str = pid_str.trim();
        if pid_str != "0" && !pid_str.is_empty() {
            let _ = Command::new("kill").args(["-HUP", pid_str]).status();
            eprintln!("tcc-import: sent SIGHUP to daemon (pid {pid_str})");
            return;
        }
    }
    eprintln!("tcc-import: could not signal daemon, restart may be needed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tux_core::fan_curve::FanCurvePoint;
    use tux_core::profile::{GpuSettings, TdpSettings};

    // ── get_arg_value ──────────────────────────────────────────────

    #[test]
    fn get_arg_value_finds_flag() {
        let args = vec![
            "tux-daemon".into(),
            "--new_profiles".into(),
            "/tmp/profiles.json".into(),
        ];
        assert_eq!(
            get_arg_value(&args, "--new_profiles"),
            Some("/tmp/profiles.json".to_string())
        );
    }

    #[test]
    fn get_arg_value_strips_quotes() {
        let args = vec![
            "tux-daemon".into(),
            "--new_profiles".into(),
            "'/tmp/profiles.json'".into(),
        ];
        assert_eq!(
            get_arg_value(&args, "--new_profiles"),
            Some("/tmp/profiles.json".to_string())
        );
    }

    #[test]
    fn get_arg_value_missing_flag() {
        let args = vec!["tux-daemon".into()];
        assert_eq!(get_arg_value(&args, "--new_profiles"), None);
    }

    #[test]
    fn get_arg_value_flag_at_end_no_value() {
        let args = vec!["tux-daemon".into(), "--new_profiles".into()];
        assert_eq!(get_arg_value(&args, "--new_profiles"), None);
    }

    // ── update_toml_field ──────────────────────────────────────────

    #[test]
    fn update_toml_field_creates_section_and_key() {
        let result = update_toml_field("", "profiles", "ac_profile", "my-id");
        let doc: toml::Value = result.parse().unwrap();
        let val = doc["profiles"]["ac_profile"].as_str().unwrap();
        assert_eq!(val, "my-id");
    }

    #[test]
    fn update_toml_field_overwrites_existing() {
        let input = "[profiles]\nac_profile = \"old-id\"\n";
        let result = update_toml_field(input, "profiles", "ac_profile", "new-id");
        let doc: toml::Value = result.parse().unwrap();
        assert_eq!(doc["profiles"]["ac_profile"].as_str().unwrap(), "new-id");
    }

    #[test]
    fn update_toml_field_preserves_other_keys() {
        let input = "[profiles]\nac_profile = \"old\"\nbattery_profile = \"bat\"\n";
        let result = update_toml_field(input, "profiles", "ac_profile", "new");
        let doc: toml::Value = result.parse().unwrap();
        assert_eq!(doc["profiles"]["ac_profile"].as_str().unwrap(), "new");
        assert_eq!(doc["profiles"]["battery_profile"].as_str().unwrap(), "bat");
    }

    // ── tcc_to_profile: fan ────────────────────────────────────────

    fn make_tcc_profile() -> TccProfile {
        TccProfile {
            id: "test-id".to_string(),
            name: "Test Profile".to_string(),
            description: "A test".to_string(),
            display: None,
            cpu: None,
            fan: Some(TccFanControl {
                use_control: true,
                fan_profile: Some("Quiet".to_string()),
                minimum_fanspeed: 10,
                maximum_fanspeed: 90,
                offset_fanspeed: 5,
                custom_fan_curve: Some(TccFanProfile {
                    table_cpu: Some(vec![
                        TccFanTableEntry {
                            temp: 40,
                            speed: 20,
                        },
                        TccFanTableEntry {
                            temp: 70,
                            speed: 60,
                        },
                        TccFanTableEntry {
                            temp: 90,
                            speed: 100,
                        },
                    ]),
                }),
            }),
            odm_profile: None,
            odm_power_limits: None,
            nvidia_power_ctrl_profile: None,
        }
    }

    #[test]
    fn tcc_to_profile_preserves_fan_profile_name() {
        let tcc = make_tcc_profile();
        let profile = tcc_to_profile(tcc);
        assert_eq!(profile.fan.tcc_fan_profile, Some("Quiet".to_string()));
    }

    #[test]
    fn tcc_to_profile_fan_curve_extracted() {
        let tcc = make_tcc_profile();
        let profile = tcc_to_profile(tcc);
        assert_eq!(profile.fan.curve.len(), 3);
        assert_eq!(
            profile.fan.curve[0],
            FanCurvePoint {
                temp: 40,
                speed: 20
            }
        );
        assert_eq!(
            profile.fan.curve[2],
            FanCurvePoint {
                temp: 90,
                speed: 100
            }
        );
    }

    #[test]
    fn tcc_to_profile_fan_speeds() {
        let tcc = make_tcc_profile();
        let profile = tcc_to_profile(tcc);
        assert_eq!(profile.fan.min_speed_percent, 10);
        assert_eq!(profile.fan.max_speed_percent, 90);
        assert_eq!(profile.fan.offset_speed_percent, 5);
    }

    #[test]
    fn tcc_to_profile_use_control_true_sets_custom_curve() {
        let tcc = make_tcc_profile();
        let profile = tcc_to_profile(tcc);
        assert!(profile.fan.enabled);
        assert_eq!(profile.fan.mode, FanMode::CustomCurve);
    }

    #[test]
    fn tcc_to_profile_use_control_false_sets_auto() {
        let mut tcc = make_tcc_profile();
        tcc.fan.as_mut().unwrap().use_control = false;
        let profile = tcc_to_profile(tcc);
        assert!(!profile.fan.enabled);
        assert_eq!(profile.fan.mode, FanMode::Auto);
    }

    #[test]
    fn tcc_to_profile_no_fan_section_defaults() {
        let tcc = TccProfile {
            id: "nofan".to_string(),
            name: "No Fan".to_string(),
            description: String::new(),
            display: None,
            cpu: None,
            fan: None,
            odm_profile: None,
            odm_power_limits: None,
            nvidia_power_ctrl_profile: None,
        };
        let profile = tcc_to_profile(tcc);
        assert!(!profile.fan.enabled);
        assert_eq!(profile.fan.mode, FanMode::Auto);
        assert!(profile.fan.curve.is_empty());
        assert_eq!(profile.fan.min_speed_percent, 0);
        assert_eq!(profile.fan.max_speed_percent, 100);
        assert_eq!(profile.fan.tcc_fan_profile, None);
    }

    // ── tcc_to_profile: CPU ────────────────────────────────────────

    #[test]
    fn tcc_to_profile_cpu_fields() {
        let mut tcc = make_tcc_profile();
        tcc.cpu = Some(TccCpu {
            governor: "performance".to_string(),
            energy_performance_preference: Some("performance".to_string()),
            no_turbo: true,
            online_cores: Some(8),
            use_max_perf_gov: Some(true),
            scaling_min_frequency: Some(800000),
            scaling_max_frequency: Some(4500000),
        });
        let profile = tcc_to_profile(tcc);
        assert_eq!(profile.cpu.governor, "performance");
        assert_eq!(
            profile.cpu.energy_performance_preference,
            Some("performance".to_string())
        );
        assert!(profile.cpu.no_turbo);
        assert_eq!(profile.cpu.online_cores, Some(8));
        assert_eq!(profile.cpu.use_max_perf_gov, Some(true));
        assert_eq!(profile.cpu.scaling_min_frequency, Some(800000));
        assert_eq!(profile.cpu.scaling_max_frequency, Some(4500000));
    }

    #[test]
    fn tcc_to_profile_no_cpu_defaults() {
        let tcc = make_tcc_profile(); // cpu: None
        let profile = tcc_to_profile(tcc);
        assert_eq!(profile.cpu.governor, "powersave");
        assert!(!profile.cpu.no_turbo);
    }

    // ── tcc_to_profile: display ────────────────────────────────────

    #[test]
    fn tcc_to_profile_display_brightness_used() {
        let mut tcc = make_tcc_profile();
        tcc.display = Some(TccDisplay {
            brightness: 75,
            use_brightness: true,
        });
        let profile = tcc_to_profile(tcc);
        assert_eq!(profile.display.brightness, Some(75));
    }

    #[test]
    fn tcc_to_profile_display_brightness_not_used() {
        let mut tcc = make_tcc_profile();
        tcc.display = Some(TccDisplay {
            brightness: 75,
            use_brightness: false,
        });
        let profile = tcc_to_profile(tcc);
        assert_eq!(profile.display.brightness, None);
    }

    // ── tcc_to_profile: TDP ────────────────────────────────────────

    #[test]
    fn tcc_to_profile_tdp_values() {
        let mut tcc = make_tcc_profile();
        tcc.odm_power_limits = Some(TccOdmPowerLimits {
            tdp_values: vec![45, 65],
        });
        let profile = tcc_to_profile(tcc);
        assert_eq!(
            profile.tdp,
            Some(TdpSettings {
                pl1: Some(45),
                pl2: Some(65),
            })
        );
    }

    #[test]
    fn tcc_to_profile_tdp_empty_values() {
        let mut tcc = make_tcc_profile();
        tcc.odm_power_limits = Some(TccOdmPowerLimits { tdp_values: vec![] });
        let profile = tcc_to_profile(tcc);
        assert_eq!(profile.tdp, None);
    }

    // ── tcc_to_profile: GPU ────────────────────────────────────────

    #[test]
    fn tcc_to_profile_gpu_offset() {
        let mut tcc = make_tcc_profile();
        tcc.nvidia_power_ctrl_profile = Some(TccNvidiaPowerCtrl { ctgp_offset: 42 });
        let profile = tcc_to_profile(tcc);
        assert_eq!(
            profile.gpu,
            Some(GpuSettings {
                ctgp_offset: Some(42),
            })
        );
    }

    #[test]
    fn tcc_to_profile_gpu_zero_offset_is_none() {
        let mut tcc = make_tcc_profile();
        tcc.nvidia_power_ctrl_profile = Some(TccNvidiaPowerCtrl { ctgp_offset: 0 });
        let profile = tcc_to_profile(tcc);
        assert_eq!(profile.gpu, None);
    }

    // ── tcc_to_profile: ODM profile ────────────────────────────────

    #[test]
    fn tcc_to_profile_odm_profile() {
        let mut tcc = make_tcc_profile();
        tcc.odm_profile = Some(TccOdmProfile {
            name: "enthusiast".to_string(),
        });
        let profile = tcc_to_profile(tcc);
        assert_eq!(profile.odm_profile, Some("enthusiast".to_string()));
    }

    #[test]
    fn tcc_to_profile_odm_empty_name_is_none() {
        let mut tcc = make_tcc_profile();
        tcc.odm_profile = Some(TccOdmProfile {
            name: String::new(),
        });
        let profile = tcc_to_profile(tcc);
        assert_eq!(profile.odm_profile, None);
    }

    // ── tcc_to_profile: clamping ───────────────────────────────────

    #[test]
    fn tcc_to_profile_clamps_fan_speed_values() {
        let tcc = TccProfile {
            id: "clamp".to_string(),
            name: "Clamp".to_string(),
            description: String::new(),
            display: None,
            cpu: None,
            fan: Some(TccFanControl {
                use_control: true,
                fan_profile: None,
                minimum_fanspeed: -10, // should clamp to 0
                maximum_fanspeed: 200, // should clamp to 100
                offset_fanspeed: -200, // should clamp to -100
                custom_fan_curve: Some(TccFanProfile {
                    table_cpu: Some(vec![
                        TccFanTableEntry {
                            temp: -5,
                            speed: -10,
                        }, // clamp to 0, 0
                        TccFanTableEntry {
                            temp: 200,
                            speed: 150,
                        }, // clamp to 100, 100
                    ]),
                }),
            }),
            odm_profile: None,
            odm_power_limits: None,
            nvidia_power_ctrl_profile: None,
        };
        let profile = tcc_to_profile(tcc);
        assert_eq!(profile.fan.min_speed_percent, 0);
        assert_eq!(profile.fan.max_speed_percent, 100);
        assert_eq!(profile.fan.offset_speed_percent, -100);
        assert_eq!(profile.fan.curve[0], FanCurvePoint { temp: 0, speed: 0 });
        assert_eq!(
            profile.fan.curve[1],
            FanCurvePoint {
                temp: 100,
                speed: 100
            }
        );
    }

    // ── import_profiles (file I/O) ─────────────────────────────────

    #[test]
    fn tcc_json_deserializes_correctly() {
        let json = r#"[{
            "id": "custom-123",
            "name": "My Custom",
            "description": "A custom profile",
            "fan": {
                "useControl": true,
                "fanProfile": "Silent",
                "minimumFanspeed": 15,
                "maximumFanspeed": 95,
                "offsetFanspeed": 0,
                "customFanCurve": {
                    "tableCPU": [
                        {"temp": 30, "speed": 10},
                        {"temp": 60, "speed": 50},
                        {"temp": 85, "speed": 100}
                    ]
                }
            },
            "cpu": {
                "governor": "performance",
                "energyPerformancePreference": "performance",
                "noTurbo": false,
                "onlineCores": 14,
                "useMaxPerfGov": true,
                "scalingMinFrequency": 800000,
                "scalingMaxFrequency": 5000000
            },
            "display": {
                "brightness": 80,
                "useBrightness": true
            },
            "odmProfile": {"name": "enthusiast"},
            "odmPowerLimits": {"tdpValues": [45, 115]},
            "nvidiaPowerCTRLProfile": {"cTGPOffset": 25}
        }]"#;

        let tcc_profiles: Vec<TccProfile> = serde_json::from_str(json).unwrap();
        assert_eq!(tcc_profiles.len(), 1);

        let profile = tcc_to_profile(tcc_profiles.into_iter().next().unwrap());
        assert_eq!(profile.id, "custom-123");
        assert_eq!(profile.name, "My Custom");
        assert_eq!(profile.fan.tcc_fan_profile, Some("Silent".to_string()));
        assert_eq!(profile.fan.curve.len(), 3);
        assert_eq!(profile.cpu.governor, "performance");
        assert_eq!(profile.cpu.online_cores, Some(14));
        assert_eq!(profile.display.brightness, Some(80));
        assert_eq!(profile.odm_profile, Some("enthusiast".to_string()));
        assert_eq!(
            profile.tdp,
            Some(TdpSettings {
                pl1: Some(45),
                pl2: Some(115)
            })
        );
        assert_eq!(
            profile.gpu,
            Some(GpuSettings {
                ctgp_offset: Some(25)
            })
        );
    }

    // ── import_settings (file I/O) ─────────────────────────────────

    #[test]
    fn tcc_settings_json_deserializes() {
        let json = r#"{
            "stateMap": {
                "power_ac": "profile-ac",
                "power_bat": "profile-bat"
            },
            "chargingProfile": "balanced",
            "chargingPriority": "charge_battery"
        }"#;
        let settings: TccSettings = serde_json::from_str(json).unwrap();
        let state_map = settings.state_map.unwrap();
        assert_eq!(state_map.power_ac, Some("profile-ac".to_string()));
        assert_eq!(state_map.power_bat, Some("profile-bat".to_string()));
        assert_eq!(settings.charging_profile, Some("balanced".to_string()));
        assert_eq!(
            settings.charging_priority,
            Some("charge_battery".to_string())
        );
    }
}
