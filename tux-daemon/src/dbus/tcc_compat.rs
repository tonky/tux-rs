//! Legacy TCC compatibility shim: flat `com.tuxedocomputers.tccd` interface.
//!
//! Exposes the same 58-method D-Bus interface that the original TUXEDO Control
//! Center Angular/Electron GUI expects. All complex data is serialised as JSON
//! strings (D-Bus type `s`), matching the original TypeScript implementation.

#[cfg(feature = "tcc-compat")]
mod inner {
    use std::path::Path;
    use std::sync::{Arc, Mutex, RwLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde::{Deserialize, Serialize};
    use tokio::sync::watch;
    use tracing::warn;
    use zbus::interface;

    use tux_core::backend::fan::FanBackend;
    use tux_core::dmi::DetectedDevice;
    use tux_core::fan_curve::FanConfig;
    use tux_core::profile::TuxProfile;

    use crate::charging::ChargingBackend;
    use crate::config::ProfileAssignments;
    use crate::cpu::governor::CpuGovernor;
    use crate::cpu::tdp::TdpBackend;
    use crate::gpu::GpuPowerBackend;
    use crate::hid::{KeyboardLed, SharedKeyboard};
    use crate::power_monitor::PowerState;
    use crate::profile_store::ProfileStore;

    // ── TCC JSON schema types ──────────────────────────────────────────

    /// Matches TCC's `IDBusFanData`.
    #[derive(Serialize, Deserialize)]
    pub struct TccFanData {
        pub cpu: TccFanSensorData,
        pub gpu1: TccFanSensorData,
        pub gpu2: TccFanSensorData,
    }

    #[derive(Serialize, Deserialize)]
    pub struct TccFanSensorData {
        pub speed: TccTimeData,
        pub temp: TccTimeData,
    }

    #[derive(Serialize, Deserialize)]
    pub struct TccTimeData {
        pub timestamp: TccDbusVariant,
        pub data: f64,
    }

    #[derive(Serialize, Deserialize)]
    pub struct TccDbusVariant {
        pub signature: String,
        pub value: i64,
    }

    /// Matches TCC's `ITccProfile`.
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TccProfile {
        pub id: String,
        pub name: String,
        pub description: String,
        pub display: TccDisplay,
        pub cpu: TccCpu,
        pub webcam: TccWebcam,
        pub fan: TccFanControl,
        pub odm_profile: TccOdmProfile,
        pub odm_power_limits: TccOdmPowerLimits,
        #[serde(rename = "nvidiaPowerCTRLProfile")]
        pub nvidia_power_ctrl_profile: TccNvidiaPowerCtrl,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TccDisplay {
        pub brightness: i32,
        pub use_brightness: bool,
        pub refresh_rate: i32,
        pub use_ref_rate: bool,
        pub x_resolution: i32,
        pub y_resolution: i32,
        pub use_resolution: bool,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TccCpu {
        pub online_cores: i32,
        pub use_max_perf_gov: bool,
        pub scaling_min_frequency: i32,
        pub scaling_max_frequency: i32,
        pub governor: String,
        pub energy_performance_preference: String,
        pub no_turbo: bool,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TccWebcam {
        pub status: bool,
        pub use_status: bool,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TccFanControl {
        pub use_control: bool,
        pub fan_profile: String,
        pub minimum_fanspeed: i32,
        pub maximum_fanspeed: i32,
        pub offset_fanspeed: i32,
        pub custom_fan_curve: TccFanProfile,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TccFanProfile {
        #[serde(default)]
        pub name: Option<String>,
        #[serde(default, rename = "tableCPU")]
        pub table_cpu: Option<Vec<TccFanTableEntry>>,
        #[serde(default, rename = "tableGPU")]
        pub table_gpu: Option<Vec<TccFanTableEntry>>,
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct TccFanTableEntry {
        pub temp: i32,
        pub speed: i32,
    }

    #[derive(Serialize, Deserialize)]
    pub struct TccOdmProfile {
        pub name: String,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TccOdmPowerLimits {
        pub tdp_values: Vec<i32>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TccNvidiaPowerCtrl {
        #[serde(rename = "cTGPOffset")]
        pub ctgp_offset: i32,
    }

    /// Matches TCC's `ITccSettings`.
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TccSettings {
        pub fahrenheit: bool,
        pub state_map: TccStateMap,
        pub shutdown_time: Option<String>,
        pub cpu_settings_enabled: bool,
        pub fan_control_enabled: bool,
        pub keyboard_backlight_control_enabled: bool,
        pub ycbcr420_workaround: Vec<serde_json::Value>,
        pub charging_profile: Option<String>,
        pub charging_priority: Option<String>,
        pub keyboard_backlight_states: Vec<TccKeyboardBacklightState>,
    }

    #[derive(Serialize, Deserialize)]
    pub struct TccStateMap {
        pub power_ac: String,
        pub power_bat: String,
    }

    /// Matches TCC's `KeyboardBacklightStateInterface`.
    #[derive(Serialize, Deserialize)]
    pub struct TccKeyboardBacklightState {
        pub mode: i32,
        pub brightness: i32,
        pub red: i32,
        pub green: i32,
        pub blue: i32,
    }

    /// Matches TCC's `KeyboardBacklightCapabilitiesInterface`.
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TccKeyboardCapabilities {
        pub modes: Vec<i32>,
        pub zones: i32,
        pub max_brightness: i32,
        pub max_red: i32,
        pub max_green: i32,
        pub max_blue: i32,
    }

    // ── Conversion helpers ─────────────────────────────────────────────

    fn now_micros() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as i64)
            .unwrap_or(0)
    }

    fn profile_to_tcc(p: &TuxProfile) -> TccProfile {
        let curve_points: Vec<TccFanTableEntry> = p
            .fan
            .curve
            .iter()
            .map(|pt| TccFanTableEntry {
                temp: pt.temp as i32,
                speed: pt.speed as i32,
            })
            .collect();

        TccProfile {
            id: p.id.clone(),
            name: p.name.clone(),
            description: p.description.clone(),
            display: TccDisplay {
                brightness: p.display.brightness.map(|b| b as i32).unwrap_or(100),
                use_brightness: p.display.brightness.is_some(),
                refresh_rate: -1,
                use_ref_rate: false,
                x_resolution: -1,
                y_resolution: -1,
                use_resolution: false,
            },
            cpu: TccCpu {
                online_cores: -1,
                use_max_perf_gov: p.cpu.governor == "performance",
                scaling_min_frequency: -1,
                scaling_max_frequency: -1,
                governor: p.cpu.governor.clone(),
                energy_performance_preference: p
                    .cpu
                    .energy_performance_preference
                    .clone()
                    .unwrap_or_default(),
                no_turbo: p.cpu.no_turbo,
            },
            webcam: TccWebcam {
                status: true,
                use_status: false,
            },
            fan: TccFanControl {
                use_control: p.fan.enabled,
                fan_profile: format!("{:?}", p.fan.mode),
                minimum_fanspeed: p.fan.min_speed_percent as i32,
                maximum_fanspeed: p.fan.max_speed_percent as i32,
                offset_fanspeed: 0,
                custom_fan_curve: TccFanProfile {
                    name: Some("custom".to_string()),
                    table_cpu: Some(curve_points.clone()),
                    table_gpu: Some(curve_points),
                },
            },
            odm_profile: TccOdmProfile {
                name: p.odm_profile.clone().unwrap_or_default(),
            },
            odm_power_limits: TccOdmPowerLimits {
                tdp_values: match &p.tdp {
                    Some(tdp) => {
                        let mut v = Vec::new();
                        if let Some(pl1) = tdp.pl1 {
                            v.push(pl1 as i32);
                        }
                        if let Some(pl2) = tdp.pl2 {
                            v.push(pl2 as i32);
                        }
                        v
                    }
                    None => vec![],
                },
            },
            nvidia_power_ctrl_profile: TccNvidiaPowerCtrl {
                ctgp_offset: p
                    .gpu
                    .as_ref()
                    .and_then(|g| g.ctgp_offset)
                    .map(|v| v as i32)
                    .unwrap_or(0),
            },
        }
    }

    // ── The compat interface ───────────────────────────────────────────

    /// Legacy TCC compatibility interface.
    ///
    /// Implements the flat `com.tuxedocomputers.tccd` interface that the
    /// original Angular/Electron TCC GUI expects.
    pub struct TccCompatInterface {
        device_name: String,
        fan_backend: Option<Arc<dyn FanBackend>>,
        config_rx: watch::Receiver<FanConfig>,
        store: Arc<RwLock<ProfileStore>>,
        assignments_tx: Arc<watch::Sender<ProfileAssignments>>,
        assignments_rx: watch::Receiver<ProfileAssignments>,
        power_rx: watch::Receiver<PowerState>,
        charging: Option<Arc<dyn ChargingBackend>>,
        keyboards: Vec<Arc<Mutex<Box<dyn KeyboardLed>>>>,
        gpu_backend: Option<Arc<dyn GpuPowerBackend>>,
        #[allow(dead_code)]
        cpu_governor: Option<Arc<CpuGovernor>>,
        #[allow(dead_code)]
        tdp_backend: Option<Arc<dyn TdpBackend>>,
        mode_reapply_pending: std::sync::Mutex<bool>,
        sensor_data_collection: std::sync::Mutex<bool>,
    }

    impl TccCompatInterface {
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            device: &DetectedDevice,
            fan_backend: Option<Arc<dyn FanBackend>>,
            config_rx: watch::Receiver<FanConfig>,
            store: Arc<RwLock<ProfileStore>>,
            assignments_tx: Arc<watch::Sender<ProfileAssignments>>,
            assignments_rx: watch::Receiver<ProfileAssignments>,
            power_rx: watch::Receiver<PowerState>,
            charging: Option<Arc<dyn ChargingBackend>>,
            keyboards: Vec<SharedKeyboard>,
            gpu_backend: Option<Arc<dyn GpuPowerBackend>>,
            cpu_governor: Option<Arc<CpuGovernor>>,
            tdp_backend: Option<Arc<dyn TdpBackend>>,
        ) -> Self {
            Self {
                device_name: device.descriptor.name.to_string(),
                fan_backend,
                config_rx,
                store,
                assignments_tx,
                assignments_rx,
                power_rx,
                charging,
                keyboards,
                gpu_backend,
                cpu_governor,
                tdp_backend,
                mode_reapply_pending: std::sync::Mutex::new(false),
                sensor_data_collection: std::sync::Mutex::new(false),
            }
        }

        fn list_profiles_inner(&self) -> Vec<TuxProfile> {
            self.store
                .read()
                .map(|s| s.list().into_iter().cloned().collect())
                .unwrap_or_default()
        }

        fn active_profile_inner(&self) -> Option<TuxProfile> {
            let assignments = self.assignments_rx.borrow().clone();
            let power = *self.power_rx.borrow();
            let id = match power {
                PowerState::Ac => &assignments.ac_profile,
                PowerState::Battery => &assignments.battery_profile,
            };
            self.store.read().ok()?.get(id).cloned()
        }
    }

    #[interface(name = "com.tuxedocomputers.tccd")]
    impl TccCompatInterface {
        // ── Health / Info ───────────────────────────────────────────

        #[zbus(name = "dbusAvailable")]
        fn dbus_available(&self) -> bool {
            true
        }

        fn get_device_name(&self) -> String {
            self.device_name.clone()
        }

        fn tccd_version(&self) -> String {
            tux_core::version().to_string()
        }

        fn tuxedo_wmi_available(&self) -> bool {
            Path::new("/sys/devices/platform/tuxedo_keyboard").exists()
        }

        fn fan_hwmon_available(&self) -> bool {
            self.fan_backend.is_some()
        }

        // ── Fan ────────────────────────────────────────────────────

        #[zbus(name = "GetFanDataJSON")]
        fn get_fan_data_json(&self) -> String {
            let ts = now_micros();
            let (cpu_speed, cpu_temp) = self
                .fan_backend
                .as_ref()
                .map(|b| {
                    let speed = b.read_fan_rpm(0).unwrap_or(0) as f64;
                    let temp = b.read_temp().unwrap_or(0) as f64;
                    (speed, temp)
                })
                .unwrap_or((-1.0, -1.0));

            let gpu1_speed = self
                .fan_backend
                .as_ref()
                .and_then(|b| {
                    if b.num_fans() > 1 {
                        Some(b.read_fan_rpm(1).unwrap_or(0) as f64)
                    } else {
                        None
                    }
                })
                .unwrap_or(-1.0);

            let data = TccFanData {
                cpu: TccFanSensorData {
                    speed: TccTimeData {
                        timestamp: TccDbusVariant {
                            signature: "x".to_string(),
                            value: ts,
                        },
                        data: cpu_speed,
                    },
                    temp: TccTimeData {
                        timestamp: TccDbusVariant {
                            signature: "x".to_string(),
                            value: ts,
                        },
                        data: cpu_temp,
                    },
                },
                gpu1: TccFanSensorData {
                    speed: TccTimeData {
                        timestamp: TccDbusVariant {
                            signature: "x".to_string(),
                            value: ts,
                        },
                        data: gpu1_speed,
                    },
                    temp: TccTimeData {
                        timestamp: TccDbusVariant {
                            signature: "x".to_string(),
                            value: ts,
                        },
                        data: -1.0,
                    },
                },
                gpu2: TccFanSensorData {
                    speed: TccTimeData {
                        timestamp: TccDbusVariant {
                            signature: "x".to_string(),
                            value: ts,
                        },
                        data: -1.0,
                    },
                    temp: TccTimeData {
                        timestamp: TccDbusVariant {
                            signature: "x".to_string(),
                            value: ts,
                        },
                        data: -1.0,
                    },
                },
            };
            serde_json::to_string(&data).unwrap_or_default()
        }

        fn get_fans_min_speed(&self) -> i32 {
            let config = self.config_rx.borrow();
            config.min_speed_percent as i32
        }

        fn get_fans_off_available(&self) -> bool {
            false
        }

        // ── Profiles ───────────────────────────────────────────────

        #[zbus(name = "GetActiveProfileJSON")]
        fn get_active_profile_json(&self) -> String {
            match self.active_profile_inner() {
                Some(p) => serde_json::to_string(&profile_to_tcc(&p)).unwrap_or_default(),
                None => "{}".to_string(),
            }
        }

        fn set_temp_profile(&self, name: &str) -> bool {
            let profiles = self.list_profiles_inner();
            if let Some(p) = profiles.iter().find(|p| p.name == name) {
                let power = *self.power_rx.borrow();
                let id = p.id.clone();
                let state = match power {
                    PowerState::Ac => "ac",
                    PowerState::Battery => "battery",
                };
                self.assignments_tx.send_modify(|a| match state {
                    "ac" => a.ac_profile = id.clone(),
                    _ => a.battery_profile = id.clone(),
                });
                true
            } else {
                warn!("TCC compat: profile not found by name: {name}");
                false
            }
        }

        fn set_temp_profile_by_id(&self, id: &str) -> bool {
            let store = match self.store.read() {
                Ok(s) => s,
                Err(_) => return false,
            };
            if store.get(id).is_none() {
                warn!("TCC compat: profile not found by id: {id}");
                return false;
            }
            let power = *self.power_rx.borrow();
            let id_str = id.to_string();
            match power {
                PowerState::Ac => {
                    self.assignments_tx.send_modify(|a| a.ac_profile = id_str);
                }
                PowerState::Battery => {
                    self.assignments_tx
                        .send_modify(|a| a.battery_profile = id_str);
                }
            }
            true
        }

        #[zbus(name = "GetProfilesJSON")]
        fn get_profiles_json(&self) -> String {
            let profiles = self.list_profiles_inner();
            let tcc: Vec<TccProfile> = profiles.iter().map(profile_to_tcc).collect();
            serde_json::to_string(&tcc).unwrap_or_default()
        }

        #[zbus(name = "GetCustomProfilesJSON")]
        fn get_custom_profiles_json(&self) -> String {
            let profiles = self.list_profiles_inner();
            let custom: Vec<TccProfile> = profiles
                .iter()
                .filter(|p| !p.is_default)
                .map(profile_to_tcc)
                .collect();
            serde_json::to_string(&custom).unwrap_or_default()
        }

        #[zbus(name = "GetDefaultProfilesJSON")]
        fn get_default_profiles_json(&self) -> String {
            let profiles = self.list_profiles_inner();
            let defaults: Vec<TccProfile> = profiles
                .iter()
                .filter(|p| p.is_default)
                .map(profile_to_tcc)
                .collect();
            serde_json::to_string(&defaults).unwrap_or_default()
        }

        #[zbus(name = "GetDefaultValuesProfileJSON")]
        fn get_default_values_profile_json(&self) -> String {
            let profiles = self.list_profiles_inner();
            match profiles.iter().find(|p| p.is_default) {
                Some(p) => serde_json::to_string(&profile_to_tcc(p)).unwrap_or_default(),
                None => "{}".to_string(),
            }
        }

        #[zbus(name = "GetSettingsJSON")]
        fn get_settings_json(&self) -> String {
            let assignments = self.assignments_rx.borrow().clone();
            let charging_profile = self
                .charging
                .as_ref()
                .and_then(|c| c.get_profile().ok().flatten());
            let charging_priority = self
                .charging
                .as_ref()
                .and_then(|c| c.get_priority().ok().flatten());

            let settings = TccSettings {
                fahrenheit: false,
                state_map: TccStateMap {
                    power_ac: assignments.ac_profile,
                    power_bat: assignments.battery_profile,
                },
                shutdown_time: None,
                cpu_settings_enabled: true,
                fan_control_enabled: self.fan_backend.is_some(),
                keyboard_backlight_control_enabled: !self.keyboards.is_empty(),
                ycbcr420_workaround: vec![],
                charging_profile,
                charging_priority,
                keyboard_backlight_states: vec![],
            };
            serde_json::to_string(&settings).unwrap_or_default()
        }

        // ── GPU ────────────────────────────────────────────────────

        #[zbus(name = "GetIGpuInfoValuesJSON")]
        fn get_i_gpu_info_values_json(&self) -> String {
            "{}".to_string()
        }

        #[zbus(name = "GetDGpuInfoValuesJSON")]
        fn get_d_gpu_info_values_json(&self) -> String {
            "{}".to_string()
        }

        #[zbus(name = "GetIGpuAvailable")]
        fn get_i_gpu_available(&self) -> i32 {
            -1
        }

        #[zbus(name = "GetDGpuAvailable")]
        fn get_d_gpu_available(&self) -> i32 {
            -1
        }

        #[zbus(name = "GetCpuPowerValuesJSON")]
        fn get_cpu_power_values_json(&self) -> String {
            "[]".to_string()
        }

        fn get_prime_state(&self) -> String {
            "-1".to_string()
        }

        // ── Keyboard ──────────────────────────────────────────────

        #[zbus(name = "GetKeyboardBacklightCapabilitiesJSON")]
        fn get_keyboard_backlight_capabilities_json(&self) -> String {
            if self.keyboards.is_empty() {
                return "{}".to_string();
            }
            let kb = match self.keyboards[0].lock() {
                Ok(g) => g,
                Err(_) => return "{}".to_string(),
            };
            let caps = TccKeyboardCapabilities {
                modes: vec![0, 1], // static, breathing
                zones: kb.zone_count() as i32,
                max_brightness: 255,
                max_red: 255,
                max_green: 255,
                max_blue: 255,
            };
            serde_json::to_string(&caps).unwrap_or_default()
        }

        #[zbus(name = "GetKeyboardBacklightStatesJSON")]
        fn get_keyboard_backlight_states_json(&self) -> String {
            // Return empty array — state is managed by the native interface.
            "[]".to_string()
        }

        #[zbus(name = "SetKeyboardBacklightStatesJSON")]
        fn set_keyboard_backlight_states_json(&self, json: &str) -> bool {
            let states: Vec<TccKeyboardBacklightState> = match serde_json::from_str(json) {
                Ok(s) => s,
                Err(e) => {
                    warn!("TCC compat: invalid keyboard JSON: {e}");
                    return false;
                }
            };
            // Apply to first keyboard if available.
            if let Some(kb_lock) = self.keyboards.first()
                && let Ok(mut kb) = kb_lock.lock()
            {
                for state in &states {
                    let _ = kb.set_brightness(state.brightness.clamp(0, 255) as u8);
                    if kb.zone_count() > 0 {
                        let _ = kb.set_color(
                            0,
                            crate::hid::Rgb::new(
                                state.red.clamp(0, 255) as u8,
                                state.green.clamp(0, 255) as u8,
                                state.blue.clamp(0, 255) as u8,
                            ),
                        );
                    }
                }
                let _ = kb.flush();
            }
            true
        }

        // ── Charging ──────────────────────────────────────────────

        fn get_charging_profiles_available(&self) -> String {
            "[]".to_string()
        }

        fn get_current_charging_profile(&self) -> String {
            self.charging
                .as_ref()
                .and_then(|c| c.get_profile().ok().flatten())
                .unwrap_or_default()
        }

        fn set_charging_profile(&self, descriptor: &str) -> bool {
            self.charging
                .as_ref()
                .map(|c| c.set_profile(descriptor).is_ok())
                .unwrap_or(false)
        }

        fn get_charging_priorities_available(&self) -> String {
            "[]".to_string()
        }

        fn get_current_charging_priority(&self) -> String {
            self.charging
                .as_ref()
                .and_then(|c| c.get_priority().ok().flatten())
                .unwrap_or_default()
        }

        fn set_charging_priority(&self, descriptor: &str) -> bool {
            self.charging
                .as_ref()
                .map(|c| c.set_priority(descriptor).is_ok())
                .unwrap_or(false)
        }

        fn get_charge_start_available_thresholds(&self) -> String {
            // TCC returns JSON array of numbers, e.g. [40, 50, 60, 70, 80, 90, 95, 100]
            let thresholds: Vec<i32> = (1..=100).collect();
            serde_json::to_string(&thresholds).unwrap_or_default()
        }

        fn get_charge_end_available_thresholds(&self) -> String {
            let thresholds: Vec<i32> = (1..=100).collect();
            serde_json::to_string(&thresholds).unwrap_or_default()
        }

        fn get_charge_start_threshold(&self) -> i32 {
            self.charging
                .as_ref()
                .and_then(|c| c.get_start_threshold().ok())
                .map(|v| v as i32)
                .unwrap_or(0)
        }

        fn get_charge_end_threshold(&self) -> i32 {
            self.charging
                .as_ref()
                .and_then(|c| c.get_end_threshold().ok())
                .map(|v| v as i32)
                .unwrap_or(100)
        }

        fn set_charge_start_threshold(&self, value: i32) -> bool {
            self.charging
                .as_ref()
                .map(|c| c.set_start_threshold(value.clamp(0, 100) as u8).is_ok())
                .unwrap_or(false)
        }

        fn set_charge_end_threshold(&self, value: i32) -> bool {
            self.charging
                .as_ref()
                .map(|c| c.set_end_threshold(value.clamp(0, 100) as u8).is_ok())
                .unwrap_or(false)
        }

        fn get_charge_type(&self) -> String {
            String::new()
        }

        fn set_charge_type(&self, _charge_type: &str) -> bool {
            false
        }

        // ── Misc ──────────────────────────────────────────────────

        fn device_has_aquaris(&self) -> bool {
            false
        }

        #[zbus(name = "GetDisplayModesJSON")]
        fn get_display_modes_json(&self) -> String {
            "[]".to_string()
        }

        fn get_is_x11(&self) -> i32 {
            -1
        }

        #[zbus(name = "WebcamSWAvailable")]
        fn webcam_sw_available(&self) -> bool {
            false
        }

        #[zbus(name = "GetWebcamSWStatus")]
        fn get_webcam_sw_status(&self) -> bool {
            false
        }

        #[zbus(name = "GetForceYUV420OutputSwitchAvailable")]
        fn get_force_yuv420_output_switch_available(&self) -> bool {
            false
        }

        fn consume_mode_reapply_pending(&self) -> bool {
            let mut pending = self
                .mode_reapply_pending
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let was_pending = *pending;
            *pending = false;
            was_pending
        }

        #[zbus(name = "ODMProfilesAvailable")]
        fn odm_profiles_available(&self) -> Vec<String> {
            vec![]
        }

        #[zbus(name = "ODMPowerLimitsJSON")]
        fn odm_power_limits_json(&self) -> String {
            "{}".to_string()
        }

        fn get_fn_lock_supported(&self) -> bool {
            Path::new("/sys/devices/platform/tuxedo_keyboard/fn_lock").exists()
        }

        fn get_fn_lock_status(&self) -> bool {
            std::fs::read_to_string("/sys/devices/platform/tuxedo_keyboard/fn_lock")
                .map(|v| v.trim() == "1")
                .unwrap_or(false)
        }

        fn set_fn_lock_status(&self, status: bool) {
            let _ = std::fs::write(
                "/sys/devices/platform/tuxedo_keyboard/fn_lock",
                if status { "1" } else { "0" },
            );
        }

        fn set_sensor_data_collection_status(&self, status: bool) {
            if let Ok(mut s) = self.sensor_data_collection.lock() {
                *s = status;
            }
        }

        fn get_sensor_data_collection_status(&self) -> bool {
            self.sensor_data_collection
                .lock()
                .map(|s| *s)
                .unwrap_or(false)
        }

        #[zbus(name = "SetDGpuD0Metrics")]
        fn set_d_gpu_d0_metrics(&self, _status: bool) {
            // No-op: not implemented in tux-rs.
        }

        #[zbus(name = "GetNVIDIAPowerCTRLDefaultPowerLimit")]
        fn get_nvidia_power_ctrl_default_power_limit(&self) -> i32 {
            0
        }

        #[zbus(name = "GetNVIDIAPowerCTRLMaxPowerLimit")]
        fn get_nvidia_power_ctrl_max_power_limit(&self) -> i32 {
            1000
        }

        #[zbus(name = "GetNVIDIAPowerCTRLAvailable")]
        fn get_nvidia_power_ctrl_available(&self) -> bool {
            self.gpu_backend.is_some()
        }

        #[zbus(name = "GetIsUnsupportedConfigurableTGPDevice")]
        fn get_is_unsupported_configurable_tgp_device(&self) -> bool {
            true
        }

        // ── Signal ────────────────────────────────────────────────

        #[zbus(signal)]
        async fn mode_reapply_pending_changed(
            emitter: &zbus::object_server::SignalEmitter<'_>,
            pending: bool,
        ) -> zbus::Result<()>;
    }

    // ── Tests ──────────────────────────────────────────────────────────

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::profile_store::ProfileStore;
        use tux_core::fan_curve::{FanConfig, FanCurvePoint, FanMode};
        use tux_core::mock::dmi::MockDmiSource;
        use tux_core::profile::{
            ChargingSettings, CpuSettings, DisplaySettings, FanProfileSettings, KeyboardSettings,
            TuxProfile,
        };

        fn make_test_profile(id: &str, name: &str, is_default: bool) -> TuxProfile {
            TuxProfile {
                id: id.to_string(),
                name: name.to_string(),
                description: "test".to_string(),
                is_default,
                fan: FanProfileSettings {
                    enabled: true,
                    mode: FanMode::Auto,
                    min_speed_percent: 20,
                    max_speed_percent: 100,
                    offset_speed_percent: 0,
                    curve: vec![
                        FanCurvePoint {
                            temp: 40,
                            speed: 30,
                        },
                        FanCurvePoint {
                            temp: 80,
                            speed: 90,
                        },
                    ],
                    tcc_fan_profile: None,
                },
                cpu: CpuSettings {
                    governor: "powersave".to_string(),
                    energy_performance_preference: Some("balance_performance".to_string()),
                    no_turbo: false,
                    online_cores: None,
                    scaling_min_frequency: None,
                    scaling_max_frequency: None,
                    use_max_perf_gov: None,
                },
                keyboard: KeyboardSettings::default(),
                display: DisplaySettings::default(),
                charging: ChargingSettings::default(),
                odm_profile: None,
                tdp: None,
                gpu: None,
            }
        }

        fn make_iface() -> TccCompatInterface {
            let dir = tempfile::tempdir().unwrap();
            let mut store = ProfileStore::new(dir.path()).unwrap();
            // Add test profiles.
            let p1 = make_test_profile("default-1", "Office", true);
            let p2 = make_test_profile("custom-1", "MyProfile", false);
            store.create(p1).unwrap();
            store.create(p2).unwrap();

            let store = Arc::new(RwLock::new(store));
            let config = FanConfig::default();
            let (_, config_rx) = watch::channel(config);
            let assignments = crate::config::ProfileAssignments {
                ac_profile: "default-1".to_string(),
                battery_profile: "default-1".to_string(),
            };
            let (assignments_tx, assignments_rx) = watch::channel(assignments);
            let (_, power_rx) = watch::channel(PowerState::Ac);

            // Use a mock DMI source.
            let source = MockDmiSource::new().tuxedo_base("STELLARIS1XI05");
            let device = tux_core::dmi::detect_device(&source).unwrap();

            TccCompatInterface::new(
                &device,
                None, // no fan backend in tests
                config_rx,
                store,
                Arc::new(assignments_tx),
                assignments_rx,
                power_rx,
                None, // no charging
                vec![],
                None, // no GPU
                None, // no CPU governor
                None, // no TDP
            )
        }

        #[test]
        fn dbus_available_returns_true() {
            let iface = make_iface();
            assert!(iface.dbus_available());
        }

        #[test]
        fn get_profiles_json_valid() {
            let iface = make_iface();
            let json = iface.get_profiles_json();
            let profiles: Vec<TccProfile> = serde_json::from_str(&json).unwrap();
            // 4 builtins + 2 custom = at least 6
            assert!(profiles.len() >= 6);
            // Check our custom profiles are present.
            let office = profiles
                .iter()
                .find(|p| p.name == "Office" && p.id == "default-1")
                .unwrap();
            assert!(office.fan.use_control);
        }

        #[test]
        fn get_custom_profiles_filters_defaults() {
            let iface = make_iface();
            let json = iface.get_custom_profiles_json();
            let profiles: Vec<TccProfile> = serde_json::from_str(&json).unwrap();
            assert_eq!(profiles.len(), 1);
            assert_eq!(profiles[0].name, "MyProfile");
        }

        #[test]
        fn get_default_profiles_filters_custom() {
            let iface = make_iface();
            let json = iface.get_default_profiles_json();
            let profiles: Vec<TccProfile> = serde_json::from_str(&json).unwrap();
            // Our custom profile "MyProfile" should not appear.
            assert!(profiles.iter().all(|p| p.name != "MyProfile"));
            // At least the builtins + our "Office" default are present.
            assert!(!profiles.is_empty());
        }

        #[test]
        fn set_temp_profile_by_id_activates() {
            let iface = make_iface();
            assert!(iface.set_temp_profile_by_id("custom-1"));
            // Verify assignment changed.
            let a = iface.assignments_rx.borrow();
            assert_eq!(a.ac_profile, "custom-1");
        }

        #[test]
        fn set_temp_profile_by_id_missing_returns_false() {
            let iface = make_iface();
            assert!(!iface.set_temp_profile_by_id("nonexistent"));
        }

        #[test]
        fn get_fan_data_json_structure() {
            let iface = make_iface();
            let json = iface.get_fan_data_json();
            let data: TccFanData = serde_json::from_str(&json).unwrap();
            // No fan backend → speeds should be -1.
            assert_eq!(data.cpu.speed.data, -1.0);
            assert_eq!(data.cpu.temp.data, -1.0);
            assert_eq!(data.gpu1.speed.data, -1.0);
        }

        #[test]
        fn get_settings_json_valid() {
            let iface = make_iface();
            let json = iface.get_settings_json();
            let settings: TccSettings = serde_json::from_str(&json).unwrap();
            assert_eq!(settings.state_map.power_ac, "default-1");
            assert!(!settings.fahrenheit);
            assert!(!settings.fan_control_enabled); // No fan backend
        }

        #[test]
        fn unsupported_methods_return_safe_defaults() {
            let iface = make_iface();
            assert!(!iface.device_has_aquaris());
            assert_eq!(iface.get_display_modes_json(), "[]");
            assert_eq!(iface.get_is_x11(), -1);
            assert_eq!(iface.get_prime_state(), "-1");
            assert!(iface.get_is_unsupported_configurable_tgp_device());
            assert!(!iface.webcam_sw_available());
            assert!(!iface.get_webcam_sw_status());
            assert!(!iface.get_force_yuv420_output_switch_available());
            assert_eq!(iface.get_i_gpu_info_values_json(), "{}");
            assert_eq!(iface.get_d_gpu_info_values_json(), "{}");
            assert_eq!(iface.get_i_gpu_available(), -1);
            assert_eq!(iface.get_d_gpu_available(), -1);
            assert!(iface.odm_profiles_available().is_empty());
        }

        #[test]
        fn consume_mode_reapply_pending_roundtrip() {
            let iface = make_iface();
            // Initially not pending.
            assert!(!iface.consume_mode_reapply_pending());
            // Set pending.
            *iface.mode_reapply_pending.lock().unwrap() = true;
            assert!(iface.consume_mode_reapply_pending());
            // Consumed — should be false again.
            assert!(!iface.consume_mode_reapply_pending());
        }

        #[test]
        fn sensor_data_collection_roundtrip() {
            let iface = make_iface();
            assert!(!iface.get_sensor_data_collection_status());
            iface.set_sensor_data_collection_status(true);
            assert!(iface.get_sensor_data_collection_status());
            iface.set_sensor_data_collection_status(false);
            assert!(!iface.get_sensor_data_collection_status());
        }

        #[test]
        fn charge_thresholds_without_backend() {
            let iface = make_iface();
            assert_eq!(iface.get_charge_start_threshold(), 0);
            assert_eq!(iface.get_charge_end_threshold(), 100);
            assert!(!iface.set_charge_start_threshold(50));
            assert!(!iface.set_charge_end_threshold(80));
        }

        #[test]
        fn get_active_profile_json_valid() {
            let iface = make_iface();
            let json = iface.get_active_profile_json();
            let profile: TccProfile = serde_json::from_str(&json).unwrap();
            assert_eq!(profile.id, "default-1");
            assert_eq!(profile.name, "Office");
        }

        #[test]
        fn keyboard_set_returns_true_with_empty_json() {
            let iface = make_iface();
            assert!(iface.set_keyboard_backlight_states_json("[]"));
        }

        #[test]
        fn keyboard_set_returns_false_on_invalid_json() {
            let iface = make_iface();
            assert!(!iface.set_keyboard_backlight_states_json("not json"));
        }

        #[test]
        fn get_keyboard_capabilities_without_keyboards() {
            let iface = make_iface();
            assert_eq!(iface.get_keyboard_backlight_capabilities_json(), "{}");
        }

        #[test]
        fn nvidia_power_without_backend() {
            let iface = make_iface();
            assert!(!iface.get_nvidia_power_ctrl_available());
            assert_eq!(iface.get_nvidia_power_ctrl_default_power_limit(), 0);
            assert_eq!(iface.get_nvidia_power_ctrl_max_power_limit(), 1000);
        }
    }
}

#[cfg(feature = "tcc-compat")]
pub use inner::TccCompatInterface;
