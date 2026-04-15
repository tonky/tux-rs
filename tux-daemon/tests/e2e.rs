#![cfg(target_os = "linux")]
//! End-to-end tests: full-stack device-driven D-Bus roundtrips.
//!
//! Each test starts a daemon configured for a specific device archetype,
//! exercises all capabilities that the device supports via D-Bus, and
//! verifies backend state matches expectations.

mod common;

use serial_test::serial;
use tux_core::backend::fan::FanBackend;
use tux_core::dbus_types::{CapabilitiesResponse, ProfileList, SystemInfoResponse};
use tux_core::device::DeviceDescriptor;
use tux_core::device_table;
use tux_core::dmi::{DetectedDevice, DmiInfo};
use tux_core::fan_curve::FanConfig;
use tux_core::platform::Platform;
use tux_core::profile::TuxProfile;
use zbus::proxy;

use common::{MockChargingBackend, TestDaemonBuilder};

// ── D-Bus Proxies ──────────────────────────────────────────────────────

#[proxy(
    interface = "com.tuxedocomputers.tccd.Fan",
    default_service = "com.tuxedocomputers.tccd",
    default_path = "/com/tuxedocomputers/tccd"
)]
trait Fan {
    fn set_fan_speed(&self, fan_index: u32, pwm: u8) -> zbus::Result<()>;
    fn set_auto_mode(&self, fan_index: u32) -> zbus::Result<()>;
    fn get_fan_speed(&self, fan_index: u32) -> zbus::Result<u32>;
    fn get_temperature(&self, sensor_index: u32) -> zbus::Result<i32>;
    fn get_fan_info(&self) -> zbus::Result<(u32, u32, bool, u8)>;
    fn get_active_fan_curve(&self) -> zbus::Result<String>;
    fn set_fan_curve(&self, toml_str: &str) -> zbus::Result<()>;
    fn set_fan_mode(&self, mode: &str) -> zbus::Result<()>;
}

#[proxy(
    interface = "com.tuxedocomputers.tccd.Profile",
    default_service = "com.tuxedocomputers.tccd",
    default_path = "/com/tuxedocomputers/tccd"
)]
trait Profile {
    fn list_profiles(&self) -> zbus::Result<String>;
    fn get_profile(&self, id: &str) -> zbus::Result<String>;
    fn create_profile(&self, toml_str: &str) -> zbus::Result<String>;
    fn delete_profile(&self, id: &str) -> zbus::Result<()>;
    fn copy_profile(&self, id: &str) -> zbus::Result<String>;
    fn update_profile(&self, id: &str, toml_str: &str) -> zbus::Result<()>;
    fn set_active_profile(&self, id: &str, state: &str) -> zbus::Result<()>;
    fn get_profile_assignments(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "com.tuxedocomputers.tccd.Device",
    default_service = "com.tuxedocomputers.tccd",
    default_path = "/com/tuxedocomputers/tccd"
)]
trait Device {
    #[zbus(property)]
    fn device_name(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn platform(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn daemon_version(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "com.tuxedocomputers.tccd.System",
    default_service = "com.tuxedocomputers.tccd",
    default_path = "/com/tuxedocomputers/tccd"
)]
trait System {
    fn get_system_info(&self) -> zbus::Result<String>;
    fn get_power_state(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "com.tuxedocomputers.tccd.Settings",
    default_service = "com.tuxedocomputers.tccd",
    default_path = "/com/tuxedocomputers/tccd"
)]
trait Settings {
    fn get_capabilities(&self) -> zbus::Result<String>;
    fn get_global_settings(&self) -> zbus::Result<String>;
    fn set_global_settings(&self, toml_str: &str) -> zbus::Result<()>;
}

#[proxy(
    interface = "com.tuxedocomputers.tccd.Charging",
    default_service = "com.tuxedocomputers.tccd",
    default_path = "/com/tuxedocomputers/tccd"
)]
trait Charging {
    fn get_charging_settings(&self) -> zbus::Result<String>;
    fn set_charging_settings(&self, toml_str: &str) -> zbus::Result<()>;
    fn get_start_threshold(&self) -> zbus::Result<u8>;
    fn set_start_threshold(&self, pct: u8) -> zbus::Result<()>;
    fn get_end_threshold(&self) -> zbus::Result<u8>;
    fn set_end_threshold(&self, pct: u8) -> zbus::Result<()>;
}

// ── Test Device Factories ──────────────────────────────────────────────

fn make_detected_device(descriptor: &'static DeviceDescriptor) -> DetectedDevice {
    DetectedDevice {
        descriptor,
        dmi: DmiInfo {
            board_vendor: "TUXEDO".to_string(),
            board_name: "TEST".to_string(),
            product_sku: descriptor.product_sku.to_string(),
            sys_vendor: "TUXEDO".to_string(),
            product_name: descriptor.name.to_string(),
            product_version: "1.0".to_string(),
        },
        exact_match: true,
    }
}

/// A Uniwill device with 2 fans, ITE keyboard, charging, and GPU power.
fn uniwill_stellaris_device() -> DetectedDevice {
    let desc = device_table::lookup_by_sku("STELLARIS1XI03")
        .expect("STELLARIS1XI03 must be in device_table for e2e tests");
    make_detected_device(desc)
}

/// A Clevo device with 2 fans and Flexicharger.
fn clevo_aura_device() -> DetectedDevice {
    let desc = device_table::lookup_by_sku("AURA14GEN3")
        .expect("AURA14GEN3 must be in device_table for e2e tests");
    make_detected_device(desc)
}

/// A minimal Uniwill device (fallback — single fan, no extras).
fn minimal_uniwill_device() -> DetectedDevice {
    let desc = device_table::fallback_for_platform(Platform::Uniwill);
    make_detected_device(desc)
}

// ── E2E Test: Uniwill Stellaris — fan + charging + capabilities ────────

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn e2e_uniwill_stellaris_full() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = uniwill_stellaris_device();

    // Set up mock charging for this Uniwill.
    let charging =
        std::sync::Arc::new(MockChargingBackend::new(20, 80).with_profile("balanced", "charge"));

    let daemon = TestDaemonBuilder::new(&device, profile_dir.path())
        .with_charging(charging)
        .build()
        .await;

    // ── Device info ──
    let device_proxy = DeviceProxy::new(&daemon.connection).await.unwrap();
    let name = device_proxy.device_name().await.unwrap();
    assert_eq!(name, "TUXEDO Stellaris 15 Gen3 Intel");

    let platform = device_proxy.platform().await.unwrap();
    assert_eq!(platform, "Uniwill");

    // ── Capabilities reflect device ──
    let settings = SettingsProxy::new(&daemon.connection).await.unwrap();
    let caps_toml = settings.get_capabilities().await.unwrap();
    let caps: CapabilitiesResponse = toml::from_str(&caps_toml).unwrap();
    assert!(caps.fan_control);
    assert_eq!(caps.fan_count, 2);
    assert!(caps.keyboard_backlight);
    assert_eq!(caps.keyboard_type, "rgb");
    assert!(!caps.charging_thresholds); // EcProfilePriority has profiles, not thresholds
    assert!(caps.charging_profiles);

    // ── Fan control ──
    let fan = FanProxy::new(&daemon.connection).await.unwrap();
    let info = fan.get_fan_info().await.unwrap();
    assert_eq!(info.3, 2); // 2 fans

    // Read initial state.
    let temp = fan.get_temperature(0).await.unwrap();
    assert_eq!(temp, 45_000);
    let rpm0 = fan.get_fan_speed(0).await.unwrap();
    assert_eq!(rpm0, 2400);
    let rpm1 = fan.get_fan_speed(1).await.unwrap();
    assert_eq!(rpm1, 2400);

    // Switch to manual, write PWM.
    fan.set_fan_mode("manual").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    fan.set_fan_speed(0, 200).await.unwrap();
    assert_eq!(daemon.fan_backend.read_pwm(0).unwrap(), 200);
    fan.set_fan_speed(1, 100).await.unwrap();
    assert_eq!(daemon.fan_backend.read_pwm(1).unwrap(), 100);

    // Fan index out of range.
    assert!(fan.set_fan_speed(2, 128).await.is_err());

    // Restore auto.
    fan.set_auto_mode(0).await.unwrap();
    fan.set_auto_mode(1).await.unwrap();
    assert!(daemon.fan_backend.is_auto(0));
    assert!(daemon.fan_backend.is_auto(1));

    // ── Fan curve ──
    let curve_toml = r#"
mode = "CustomCurve"
min_speed_percent = 25
active_poll_ms = 1000
idle_poll_ms = 5000
hysteresis_degrees = 3

[[curve]]
temp = 30
speed = 15

[[curve]]
temp = 50
speed = 30

[[curve]]
temp = 70
speed = 60

[[curve]]
temp = 90
speed = 100
"#;
    fan.set_fan_curve(curve_toml).await.unwrap();
    let active = fan.get_active_fan_curve().await.unwrap();
    let config: FanConfig = toml::from_str(&active).unwrap();
    assert_eq!(config.curve.len(), 4);
    assert_eq!(config.min_speed_percent, 25);

    // ── Charging ──
    let charging_proxy = ChargingProxy::new(&daemon.connection).await.unwrap();
    let start = charging_proxy.get_start_threshold().await.unwrap();
    assert_eq!(start, 20);
    let end = charging_proxy.get_end_threshold().await.unwrap();
    assert_eq!(end, 80);

    // Update thresholds.
    charging_proxy.set_start_threshold(30).await.unwrap();
    charging_proxy.set_end_threshold(90).await.unwrap();
    assert_eq!(charging_proxy.get_start_threshold().await.unwrap(), 30);
    assert_eq!(charging_proxy.get_end_threshold().await.unwrap(), 90);

    // TOML-based settings roundtrip.
    let settings_toml = charging_proxy.get_charging_settings().await.unwrap();
    assert!(settings_toml.contains("start_threshold"));

    // ── System info ──
    let system = SystemProxy::new(&daemon.connection).await.unwrap();
    let info_toml = system.get_system_info().await.unwrap();
    let info: SystemInfoResponse = toml::from_str(&info_toml).unwrap();
    assert!(!info.version.is_empty());
    assert!(!info.hostname.is_empty());

    let power_state = system.get_power_state().await.unwrap();
    assert_eq!(power_state, "ac");

    // ── Profile CRUD ──
    let profile = ProfileProxy::new(&daemon.connection).await.unwrap();
    let list = profile.list_profiles().await.unwrap();
    assert!(list.contains("__office__"));
    assert!(list.contains("__quiet__"));

    // Create profile.
    let new_profile = r#"
id = "e2e-test"
name = "E2E Test Profile"

[fan]
mode = "Auto"
min_speed_percent = 20

[cpu]
governor = "powersave"
"#;
    let id = profile.create_profile(new_profile).await.unwrap();
    assert_eq!(id, "e2e-test");

    // Update.
    let updated = r#"
id = "e2e-test"
name = "E2E Updated"

[fan]
mode = "Manual"
min_speed_percent = 50

[cpu]
governor = "performance"
"#;
    profile.update_profile("e2e-test", updated).await.unwrap();
    let fetched = profile.get_profile("e2e-test").await.unwrap();
    assert!(fetched.contains("E2E Updated"));

    // Set active.
    profile.set_active_profile("e2e-test", "ac").await.unwrap();
    let assignments = profile.get_profile_assignments().await.unwrap();
    assert!(assignments.contains("e2e-test"));

    // Delete.
    profile.delete_profile("e2e-test").await.unwrap();
    assert!(profile.get_profile("e2e-test").await.is_err());

    daemon.stop().await;
}

// ── E2E Test: Clevo — fan + Flexicharger ───────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn e2e_clevo_with_charging() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = clevo_aura_device();

    let charging = std::sync::Arc::new(MockChargingBackend::new(40, 80));

    let daemon = TestDaemonBuilder::new(&device, profile_dir.path())
        .with_charging(charging)
        .build()
        .await;

    // ── Device identity ──
    let device_proxy = DeviceProxy::new(&daemon.connection).await.unwrap();
    assert_eq!(
        device_proxy.device_name().await.unwrap(),
        "TUXEDO Aura 14 Gen3"
    );
    assert_eq!(device_proxy.platform().await.unwrap(), "Clevo");

    // ── Capabilities match Clevo features ──
    let settings = SettingsProxy::new(&daemon.connection).await.unwrap();
    let caps: CapabilitiesResponse =
        toml::from_str(&settings.get_capabilities().await.unwrap()).unwrap();
    assert!(caps.fan_control);
    assert_eq!(caps.fan_count, 2);
    assert!(caps.keyboard_backlight); // Rgb3Zone
    assert_eq!(caps.keyboard_type, "rgb");
    assert!(caps.charging_thresholds); // Flexicharger
    assert!(!caps.charging_profiles); // Flexicharger, not EcProfilePriority

    // ── Fan control ──
    let fan = FanProxy::new(&daemon.connection).await.unwrap();
    fan.set_fan_mode("manual").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    fan.set_fan_speed(0, 128).await.unwrap();
    assert_eq!(daemon.fan_backend.read_pwm(0).unwrap(), 128);

    // ── Charging (Flexicharger) ──
    let charging_proxy = ChargingProxy::new(&daemon.connection).await.unwrap();
    assert_eq!(charging_proxy.get_start_threshold().await.unwrap(), 40);
    assert_eq!(charging_proxy.get_end_threshold().await.unwrap(), 80);

    charging_proxy.set_start_threshold(50).await.unwrap();
    charging_proxy.set_end_threshold(95).await.unwrap();
    assert_eq!(charging_proxy.get_start_threshold().await.unwrap(), 50);
    assert_eq!(charging_proxy.get_end_threshold().await.unwrap(), 95);

    daemon.stop().await;
}

// ── E2E Test: Minimal device — no extras ───────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn e2e_minimal_device() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = minimal_uniwill_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    // ── Capabilities should report minimal features ──
    let settings = SettingsProxy::new(&daemon.connection).await.unwrap();
    let caps: CapabilitiesResponse =
        toml::from_str(&settings.get_capabilities().await.unwrap()).unwrap();
    assert!(caps.fan_control);
    assert_eq!(caps.fan_count, 1);
    assert!(!caps.keyboard_backlight);
    assert_eq!(caps.keyboard_type, "none");
    assert!(!caps.charging_thresholds);

    // ── Fan control still works (single fan) ──
    let fan = FanProxy::new(&daemon.connection).await.unwrap();
    let info = fan.get_fan_info().await.unwrap();
    assert_eq!(info.3, 1);

    fan.set_fan_mode("manual").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    fan.set_fan_speed(0, 64).await.unwrap();
    assert_eq!(daemon.fan_backend.read_pwm(0).unwrap(), 64);

    // Fan index 1 is out of range.
    assert!(fan.set_fan_speed(1, 128).await.is_err());

    // ── Charging interface should not be registered ──
    let charging_result = ChargingProxy::builder(&daemon.connection)
        .build()
        .await
        .unwrap()
        .get_start_threshold()
        .await;
    assert!(charging_result.is_err());

    // ── Profile CRUD still works ──
    let profile = ProfileProxy::new(&daemon.connection).await.unwrap();
    let list = profile.list_profiles().await.unwrap();
    assert!(list.contains("__office__"));

    daemon.stop().await;
}

// ── E2E Test: TUI/Dashboard state via D-Bus ────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn e2e_dashboard_state_matches_daemon() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = uniwill_stellaris_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    let fan = FanProxy::new(&daemon.connection).await.unwrap();

    // Verify dashboard-like polling returns consistent data.
    let temp = fan.get_temperature(0).await.unwrap();
    assert_eq!(temp, 45_000);

    let rpm0 = fan.get_fan_speed(0).await.unwrap();
    let rpm1 = fan.get_fan_speed(1).await.unwrap();
    assert_eq!(rpm0, 2400);
    assert_eq!(rpm1, 2400);

    // Update mock backend state (simulates sensor change).
    daemon.fan_backend.set_temp(72);
    daemon.fan_backend.set_rpm(0, 3500);
    daemon.fan_backend.set_rpm(1, 3200);

    // Dashboard poll now reads updated state.
    let temp2 = fan.get_temperature(0).await.unwrap();
    assert_eq!(temp2, 72_000);
    let rpm0_new = fan.get_fan_speed(0).await.unwrap();
    let rpm1_new = fan.get_fan_speed(1).await.unwrap();
    assert_eq!(rpm0_new, 3500);
    assert_eq!(rpm1_new, 3200);

    // System info is well-formed.
    let system = SystemProxy::new(&daemon.connection).await.unwrap();
    let info_toml = system.get_system_info().await.unwrap();
    let info: SystemInfoResponse = toml::from_str(&info_toml).unwrap();
    assert!(!info.version.is_empty());
    assert!(!info.hostname.is_empty());
    assert!(!info.kernel.is_empty());

    daemon.stop().await;
}

// ── E2E Test: Profile lifecycle end-to-end ─────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn e2e_profile_lifecycle() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = clevo_aura_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    let profile = ProfileProxy::new(&daemon.connection).await.unwrap();

    // State 1: only builtins.
    let list_toml = profile.list_profiles().await.unwrap();
    let profile_list: ProfileList = toml::from_str(&list_toml).unwrap();
    let initial_count = profile_list.profiles.len();
    assert!(initial_count >= 4); // 4 builtins

    // State 2: copy a builtin.
    let copied_id = profile.copy_profile("__office__").await.unwrap();
    let list_toml = profile.list_profiles().await.unwrap();
    let profile_list: ProfileList = toml::from_str(&list_toml).unwrap();
    assert_eq!(profile_list.profiles.len(), initial_count + 1);

    // State 3: fetch the copy.
    let copied = profile.get_profile(&copied_id).await.unwrap();
    let p: TuxProfile = toml::from_str(&copied).unwrap();
    assert!(!p.is_default);
    assert!(p.name.contains("Office"));

    // State 4: update the copy.
    let mut updated_p = p.clone();
    updated_p.name = "Custom Gaming".to_string();
    updated_p.fan.min_speed_percent = 40;
    let updated_toml = toml::to_string(&updated_p).unwrap();
    profile
        .update_profile(&copied_id, &updated_toml)
        .await
        .unwrap();

    let fetched = profile.get_profile(&copied_id).await.unwrap();
    assert!(fetched.contains("Custom Gaming"));

    // State 5: set as active for AC.
    profile.set_active_profile(&copied_id, "ac").await.unwrap();
    let assignments = profile.get_profile_assignments().await.unwrap();
    assert!(assignments.contains(&copied_id));

    // State 6: delete.
    profile.delete_profile(&copied_id).await.unwrap();
    assert!(profile.get_profile(&copied_id).await.is_err());

    // Cannot delete builtins.
    assert!(profile.delete_profile("__office__").await.is_err());

    daemon.stop().await;
}

// ── E2E Test: Settings roundtrip ───────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn e2e_settings_roundtrip() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = clevo_aura_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    let settings = SettingsProxy::new(&daemon.connection).await.unwrap();

    // Read defaults.
    let defaults = settings.get_global_settings().await.unwrap();
    assert!(defaults.contains("celsius"));
    assert!(defaults.contains("fan_control_enabled"));

    // Update settings.
    let new_settings = r#"
temperature_unit = "fahrenheit"
fan_control_enabled = false
cpu_settings_enabled = true
"#;
    settings.set_global_settings(new_settings).await.unwrap();

    // Verify change persisted.
    let updated = settings.get_global_settings().await.unwrap();
    assert!(updated.contains("fahrenheit"));
    assert!(updated.contains("fan_control_enabled = false"));

    daemon.stop().await;
}

// ── E2E Tests: TUI CLI dumps ──────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn e2e_cli_dump_dashboard() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = uniwill_stellaris_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    // Use the D-Bus client directly to verify the same data the CLI would read.
    let fan = FanProxy::new(&daemon.connection).await.unwrap();
    let temp = fan.get_temperature(0).await.unwrap();
    let rpm0 = fan.get_fan_speed(0).await.unwrap();
    let rpm1 = fan.get_fan_speed(1).await.unwrap();
    let system = SystemProxy::new(&daemon.connection).await.unwrap();
    let power = system.get_power_state().await.unwrap();

    // Verify the data matches what DumpDashboard would produce.
    let snapshot = tux_core::dbus_types::DashboardSnapshot {
        cpu_temp: Some(temp as f32 / 1000.0),
        fan_speeds: vec![rpm0, rpm1],
        power_state: power,
    };
    let json = serde_json::to_string_pretty(&snapshot).unwrap();
    assert!(json.contains("\"cpu_temp\""));
    assert!(json.contains("\"fan_speeds\""));
    assert!(json.contains("\"power_state\""));
    assert!(json.contains("45.0"));
    assert!(json.contains("2400"));
    assert!(json.contains("\"ac\""));

    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn e2e_cli_dump_profiles() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = clevo_aura_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    // Fetch profile list via D-Bus (same as CLI DumpProfiles).
    let profile = ProfileProxy::new(&daemon.connection).await.unwrap();
    let list_toml = profile.list_profiles().await.unwrap();
    let profile_list: ProfileList = toml::from_str(&list_toml).unwrap();

    // Serialize as JSON (same as CLI output).
    let json = serde_json::to_string_pretty(&profile_list.profiles).unwrap();
    assert!(json.contains("__office__"));
    assert!(json.contains("__quiet__"));
    assert!(json.contains("__high_performance__"));
    assert!(json.contains("__max_energy_save__"));

    // Verify it's valid JSON containing profile structure.
    let parsed: Vec<tux_core::profile::TuxProfile> = serde_json::from_str(&json).unwrap();
    assert!(parsed.len() >= 4);

    daemon.stop().await;
}
