//! Shared D-Bus response types for daemon ↔ TUI communication.
//!
//! Both `tux-daemon` and `tux-tui` use these types for serializing/deserializing
//! D-Bus TOML payloads, ensuring wire format agreement.

use serde::{Deserialize, Serialize};

/// Fan telemetry as returned by D-Bus `GetFanInfo` (structured version).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FanInfoResponse {
    pub max_rpm: u32,
    pub min_rpm: u32,
    pub multi_fan: bool,
    pub num_fans: u8,
}

/// Per-fan telemetry data point.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FanData {
    pub rpm: u32,
    pub temp_celsius: f32,
    pub duty_percent: u8,
    /// `true` if the RPM reading is from a real hardware sensor;
    /// `false` if the platform does not expose an RPM counter.
    #[serde(default)]
    pub rpm_available: bool,
}

/// Fan engine health status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FanHealthResponse {
    /// "ok", "degraded" (≥5 consecutive failures), or "failed" (≥30).
    pub status: String,
    pub consecutive_failures: u32,
}

/// Keyboard info as returned by D-Bus `GetKeyboardInfo`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyboardInfoResponse {
    pub keyboards: Vec<KeyboardData>,
}

/// Single keyboard controller info.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyboardData {
    pub index: u32,
    pub device_type: String,
    pub zone_count: u8,
    pub available_modes: Vec<String>,
}

/// GPU info as returned by D-Bus `GetGpuInfo`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpuInfoResponse {
    pub gpus: Vec<GpuData>,
}

/// Single GPU telemetry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpuData {
    pub name: String,
    pub temperature: Option<f32>,
    pub power_draw_w: Option<f32>,
    pub usage_percent: Option<u8>,
    pub gpu_type: String,
}

/// System info as returned by D-Bus `GetSystemInfo`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SystemInfoResponse {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub kernel: String,
}

/// Hardware capabilities derived from detected device.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CapabilitiesResponse {
    #[serde(default)]
    pub fan_control: bool,
    #[serde(default)]
    pub fan_count: u8,
    #[serde(default)]
    pub keyboard_backlight: bool,
    #[serde(default)]
    pub keyboard_type: String,
    #[serde(default)]
    pub keyboard_modes: Vec<String>,
    #[serde(default)]
    pub charging_thresholds: bool,
    #[serde(default)]
    pub charging_profiles: bool,
    #[serde(default)]
    pub tdp_control: bool,
    #[serde(default)]
    pub power_profiles: bool,
    #[serde(default)]
    pub gpu_control: bool,
    #[serde(default)]
    pub display_brightness: bool,
}

/// Charging settings as returned by D-Bus `GetChargingSettings`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChargingSettingsResponse {
    pub start_threshold: u8,
    pub end_threshold: u8,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub priority: String,
}

/// Wrapper for deserializing the profile list from the daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileList {
    pub profiles: Vec<crate::profile::TuxProfile>,
}

/// Profile assignments for AC and battery power states.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileAssignmentsResponse {
    pub ac_profile: String,
    pub battery_profile: String,
}

/// Dashboard telemetry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DashboardSnapshot {
    pub cpu_temp: Option<f32>,
    pub fan_speeds: Vec<u32>,
    pub power_state: String,
}

/// CPU load snapshot: overall + per-core utilization percentages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CpuLoadResponse {
    /// Overall CPU utilization (0–100%).
    pub overall: f32,
    /// Per-core utilization (0–100%), indexed by core number.
    pub per_core: Vec<f32>,
}

/// Per-core CPU frequencies in MHz.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CpuFreqResponse {
    /// Per-core frequency in MHz, indexed by core number.
    pub per_core: Vec<u32>,
}

/// Battery information as returned by D-Bus `GetBatteryInfo`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BatteryInfoResponse {
    /// Whether a battery was detected.
    #[serde(default)]
    pub present: bool,
    /// Charge level 0–100%.
    #[serde(default)]
    pub capacity_percent: u32,
    /// Charging status: "Charging", "Discharging", "Full", "Not charging", or "Unknown".
    #[serde(default)]
    pub status: String,
    /// Lifetime cycle count.
    #[serde(default)]
    pub cycle_count: u32,
    /// Current charge in mAh.
    #[serde(default)]
    pub charge_now_mah: u32,
    /// Last full charge in mAh.
    #[serde(default)]
    pub charge_full_mah: u32,
    /// Design (factory) capacity in mAh.
    #[serde(default)]
    pub charge_full_design_mah: u32,
    /// Current draw in mA (positive = charging, negative = discharging).
    #[serde(default)]
    pub current_now_ma: i32,
    /// Current voltage in mV.
    #[serde(default)]
    pub voltage_now_mv: u32,
    /// Design voltage in mV.
    #[serde(default)]
    pub voltage_design_mv: u32,
    /// Battery chemistry (e.g. "Li-ion").
    #[serde(default)]
    pub technology: String,
    /// Manufacturer name.
    #[serde(default)]
    pub manufacturer: String,
    /// Model name.
    #[serde(default)]
    pub model_name: String,
    /// Battery health: charge_full / charge_full_design as percentage.
    #[serde(default)]
    pub health_percent: u32,
}

/// Display brightness state as exchanged over D-Bus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplayState {
    /// Current brightness as a percentage (0–100).
    pub brightness: u32,
    /// Maximum raw brightness value reported by the driver.
    pub max_brightness: u32,
    /// Name of the backlight driver (e.g. "intel_backlight").
    pub driver: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_info_response_roundtrip() {
        let resp = FanInfoResponse {
            max_rpm: 6000,
            min_rpm: 0,
            multi_fan: true,
            num_fans: 2,
        };
        let toml_str = toml::to_string(&resp).unwrap();
        let decoded: FanInfoResponse = toml::from_str(&toml_str).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn fan_data_roundtrip() {
        let data = FanData {
            rpm: 2400,
            temp_celsius: 45.5,
            duty_percent: 60,
            rpm_available: true,
        };
        let toml_str = toml::to_string(&data).unwrap();
        let decoded: FanData = toml::from_str(&toml_str).unwrap();
        assert_eq!(data, decoded);
    }

    #[test]
    fn keyboard_info_roundtrip() {
        let resp = KeyboardInfoResponse {
            keyboards: vec![KeyboardData {
                index: 0,
                device_type: "ite8291".into(),
                zone_count: 4,
                available_modes: vec!["static".into(), "breathe".into()],
            }],
        };
        let toml_str = toml::to_string(&resp).unwrap();
        let decoded: KeyboardInfoResponse = toml::from_str(&toml_str).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn gpu_info_roundtrip() {
        let resp = GpuInfoResponse {
            gpus: vec![GpuData {
                name: "nvidia".into(),
                temperature: Some(65.0),
                power_draw_w: Some(80.5),
                usage_percent: Some(45),
                gpu_type: "Discrete".into(),
            }],
        };
        let toml_str = toml::to_string(&resp).unwrap();
        let decoded: GpuInfoResponse = toml::from_str(&toml_str).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn system_info_roundtrip() {
        let resp = SystemInfoResponse {
            version: "0.1.0".into(),
            hostname: "testhost".into(),
            kernel: "6.17.0".into(),
        };
        let toml_str = toml::to_string(&resp).unwrap();
        let decoded: SystemInfoResponse = toml::from_str(&toml_str).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn capabilities_roundtrip() {
        let resp = CapabilitiesResponse {
            fan_control: true,
            fan_count: 2,
            keyboard_backlight: true,
            keyboard_type: "rgb".into(),
            charging_thresholds: true,
            charging_profiles: false,
            tdp_control: true,
            keyboard_modes: vec!["static".into(), "breathe".into()],
            power_profiles: true,
            gpu_control: false,
            display_brightness: true,
        };
        let toml_str = toml::to_string(&resp).unwrap();
        let decoded: CapabilitiesResponse = toml::from_str(&toml_str).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn profile_assignments_roundtrip() {
        let resp = ProfileAssignmentsResponse {
            ac_profile: "__office__".into(),
            battery_profile: "__quiet__".into(),
        };
        let toml_str = toml::to_string(&resp).unwrap();
        let decoded: ProfileAssignmentsResponse = toml::from_str(&toml_str).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn dashboard_snapshot_roundtrip() {
        let snap = DashboardSnapshot {
            cpu_temp: Some(55.3),
            fan_speeds: vec![2400, 2200],
            power_state: "ac".into(),
        };
        let toml_str = toml::to_string(&snap).unwrap();
        let decoded: DashboardSnapshot = toml::from_str(&toml_str).unwrap();
        assert_eq!(snap, decoded);
    }

    #[test]
    fn charging_settings_roundtrip() {
        let resp = ChargingSettingsResponse {
            start_threshold: 20,
            end_threshold: 80,
            profile: "balanced".into(),
            priority: "charge".into(),
        };
        let toml_str = toml::to_string(&resp).unwrap();
        let decoded: ChargingSettingsResponse = toml::from_str(&toml_str).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn battery_info_roundtrip() {
        let resp = BatteryInfoResponse {
            present: true,
            capacity_percent: 85,
            status: "Discharging".into(),
            cycle_count: 142,
            charge_now_mah: 4200,
            charge_full_mah: 5000,
            charge_full_design_mah: 5300,
            current_now_ma: -1500,
            voltage_now_mv: 15800,
            voltage_design_mv: 15480,
            technology: "Li-ion".into(),
            manufacturer: "OEM".into(),
            model_name: "standard".into(),
            health_percent: 94,
        };
        let toml_str = toml::to_string(&resp).unwrap();
        let decoded: BatteryInfoResponse = toml::from_str(&toml_str).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn display_state_roundtrip() {
        let state = DisplayState {
            brightness: 75,
            max_brightness: 96000,
            driver: "intel_backlight".into(),
        };
        let toml_str = toml::to_string(&state).unwrap();
        let decoded: DisplayState = toml::from_str(&toml_str).unwrap();
        assert_eq!(state, decoded);
    }
}
