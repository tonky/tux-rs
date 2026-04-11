//! Integration tests: full daemon D-Bus roundtrips on the session bus.
//!
//! These tests start a real D-Bus service with mock backends and exercise
//! the D-Bus API. They require a running D-Bus session bus (available on
//! any Linux desktop; in CI use `dbus-run-session`).

mod common;

use serial_test::serial;
use tux_core::backend::fan::FanBackend;
use tux_core::device_table;
use tux_core::dmi::{DetectedDevice, DmiInfo};
use tux_core::fan_curve::FanConfig;
use tux_core::platform::Platform;
use zbus::proxy;

/// D-Bus proxy for the Fan interface.
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
    fn get_active_fan_curve(&self) -> zbus::Result<String>;
    fn set_fan_curve(&self, toml_str: &str) -> zbus::Result<()>;
    fn set_fan_mode(&self, mode: &str) -> zbus::Result<()>;
}

/// D-Bus proxy for the Profile interface.
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
    fn set_active_profile(&self, id: &str, state: &str) -> zbus::Result<()>;
}

/// D-Bus proxy for the Device interface (properties only).
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

/// D-Bus proxy for the System interface.
#[proxy(
    interface = "com.tuxedocomputers.tccd.System",
    default_service = "com.tuxedocomputers.tccd",
    default_path = "/com/tuxedocomputers/tccd"
)]
trait System {
    fn get_power_state(&self) -> zbus::Result<String>;
}

/// D-Bus proxy for the Settings interface.
#[proxy(
    interface = "com.tuxedocomputers.tccd.Settings",
    default_service = "com.tuxedocomputers.tccd",
    default_path = "/com/tuxedocomputers/tccd"
)]
trait Settings {
    fn get_capabilities(&self) -> zbus::Result<String>;
}

fn test_device() -> DetectedDevice {
    DetectedDevice {
        descriptor: device_table::fallback_for_platform(Platform::Uniwill),
        dmi: DmiInfo {
            board_vendor: "TUXEDO".to_string(),
            board_name: "TEST".to_string(),
            product_sku: "TEST_SKU".to_string(),
            sys_vendor: "TUXEDO".to_string(),
            product_name: "Test Laptop".to_string(),
            product_version: "1.0".to_string(),
        },
        exact_match: false,
    }
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn device_info_roundtrip() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = test_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    let proxy = DeviceProxy::new(&daemon.connection).await.unwrap();
    let name = proxy.device_name().await.unwrap();
    assert_eq!(name, "Unknown Uniwill Device");

    let platform = proxy.platform().await.unwrap();
    assert!(!platform.is_empty());

    let version = proxy.daemon_version().await.unwrap();
    assert!(!version.is_empty());

    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn fan_read_write_roundtrip() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = test_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    let proxy = FanProxy::new(&daemon.connection).await.unwrap();

    // Read initial temperature (set to 45 in TestDaemon::start).
    let temp = proxy.get_temperature(0).await.unwrap();
    assert_eq!(temp, 45_000); // millidegrees

    // Read initial RPM.
    let rpm = proxy.get_fan_speed(0).await.unwrap();
    assert_eq!(rpm, 2400);

    // Switch to manual mode so the engine stops overwriting PWM.
    proxy.set_fan_mode("manual").await.unwrap();
    // Give the engine time to observe the mode change.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Write PWM then verify via mock backend.
    proxy.set_fan_speed(0, 128).await.unwrap();
    assert_eq!(daemon.fan_backend.read_pwm(0).unwrap(), 128);
    assert!(!daemon.fan_backend.is_auto(0));

    // Restore auto mode.
    proxy.set_auto_mode(0).await.unwrap();
    assert!(daemon.fan_backend.is_auto(0));

    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn profile_crud_roundtrip() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = test_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    let proxy = ProfileProxy::new(&daemon.connection).await.unwrap();

    // List initial profiles (should have builtins).
    let list = proxy.list_profiles().await.unwrap();
    assert!(list.contains("__office__"));

    // Create a new profile.
    let new_profile_toml = r#"
id = "test-profile"
name = "Test Profile"

[fan]
mode = "CustomCurve"
min_speed_percent = 30

[[fan.curve]]
temp = 30
speed = 0

[[fan.curve]]
temp = 50
speed = 30

[[fan.curve]]
temp = 70
speed = 60

[[fan.curve]]
temp = 90
speed = 100

[cpu]
governor = "powersave"
"#;

    let id = proxy.create_profile(new_profile_toml).await.unwrap();
    assert_eq!(id, "test-profile");

    // Get the profile back.
    let profile_toml = proxy.get_profile("test-profile").await.unwrap();
    assert!(profile_toml.contains("Test Profile"));
    assert!(profile_toml.contains("min_speed_percent = 30"));

    // Delete the profile.
    proxy.delete_profile("test-profile").await.unwrap();

    // Verify it's gone.
    let result = proxy.get_profile("test-profile").await;
    assert!(result.is_err());

    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn profile_copy_roundtrip() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = test_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    let proxy = ProfileProxy::new(&daemon.connection).await.unwrap();

    // Copy one of the builtin profiles.
    let new_id = proxy.copy_profile("__office__").await.unwrap();
    assert!(!new_id.is_empty());
    assert_ne!(new_id, "__office__");

    // The copy should exist.
    let copy_toml = proxy.get_profile(&new_id).await.unwrap();
    assert!(copy_toml.contains("Office"));

    // Clean up.
    proxy.delete_profile(&new_id).await.unwrap();

    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn system_power_state() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = test_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    let proxy = SystemProxy::new(&daemon.connection).await.unwrap();
    let state = proxy.get_power_state().await.unwrap();
    // TestDaemon sets PowerState::Ac.
    assert_eq!(state, "ac");

    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn settings_capabilities() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = test_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    let proxy = SettingsProxy::new(&daemon.connection).await.unwrap();
    let caps = proxy.get_capabilities().await.unwrap();
    // Should be a TOML string with capabilities.
    assert!(caps.contains("fan") || caps.contains("keyboard") || caps.contains("charging"));

    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn fan_curve_roundtrip() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = test_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    let proxy = FanProxy::new(&daemon.connection).await.unwrap();

    // Get current curve.
    let curve = proxy.get_active_fan_curve().await.unwrap();
    assert!(curve.contains("mode"));

    // Set a new curve (must include all required FanConfig fields).
    let new_curve = r#"
mode = "Manual"
min_speed_percent = 50
active_poll_ms = 2000
idle_poll_ms = 10000
hysteresis_degrees = 3

[[curve]]
temp = 30
speed = 20

[[curve]]
temp = 60
speed = 50

[[curve]]
temp = 80
speed = 80

[[curve]]
temp = 95
speed = 100
"#;
    proxy.set_fan_curve(new_curve).await.unwrap();

    // Verify it changed.
    let updated = proxy.get_active_fan_curve().await.unwrap();
    assert!(updated.contains("Manual"));

    daemon.stop().await;
}

// ── Error path tests ───────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn fan_invalid_index_returns_error() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = test_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    let proxy = FanProxy::new(&daemon.connection).await.unwrap();

    // Fan index 99 is out of range (device has 1 fan).
    let result = proxy.set_fan_speed(99, 128).await;
    assert!(result.is_err());

    let result = proxy.get_fan_speed(99).await;
    assert!(result.is_err());

    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn profile_delete_builtin_returns_error() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = test_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    let proxy = ProfileProxy::new(&daemon.connection).await.unwrap();

    // Builtin profiles cannot be deleted.
    let result = proxy.delete_profile("__office__").await;
    assert!(result.is_err());

    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn profile_create_invalid_toml_returns_error() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = test_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    let proxy = ProfileProxy::new(&daemon.connection).await.unwrap();

    // Malformed TOML should fail.
    let result = proxy.create_profile("not valid toml {{{").await;
    assert!(result.is_err());

    daemon.stop().await;
}

/// Regression test for fan curve loss on profile switch.
///
/// Repro: create custom profile → edit fan curve → save → activate builtin
/// → re-activate custom → fan curve should retain the edited curve, not default.
#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn fan_curve_persists_across_profile_switch() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = test_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;

    let profile_proxy = ProfileProxy::new(&daemon.connection).await.unwrap();
    let fan_proxy = FanProxy::new(&daemon.connection).await.unwrap();

    // 1. Create a custom profile with a default fan curve.
    let profile_toml = r#"
id = "curve-test-profile"
name = "curve-test"
description = "test fan curve persistence"

[fan]
mode = "CustomCurve"
min_speed_percent = 20

[[fan.curve]]
temp = 30
speed = 20

[[fan.curve]]
temp = 60
speed = 50

[[fan.curve]]
temp = 80
speed = 80

[[fan.curve]]
temp = 95
speed = 100

[cpu]
governor = "powersave"
"#;
    let profile_id = profile_proxy.create_profile(profile_toml).await.unwrap();

    // 2. Activate the custom profile (for AC power state).
    profile_proxy
        .set_active_profile(&profile_id, "ac")
        .await
        .unwrap();

    // 3. Set a custom 3-point fan curve via the Fan D-Bus interface.
    let custom_curve = r#"
mode = "CustomCurve"
min_speed_percent = 30
active_poll_ms = 2000
idle_poll_ms = 10000
hysteresis_degrees = 3

[[curve]]
temp = 40
speed = 25

[[curve]]
temp = 65
speed = 60

[[curve]]
temp = 85
speed = 100
"#;
    fan_proxy.set_fan_curve(custom_curve).await.unwrap();

    // 4. Verify the curve was applied at runtime.
    let active = fan_proxy.get_active_fan_curve().await.unwrap();
    let active_config: FanConfig = toml::from_str(&active).unwrap();
    assert_eq!(active_config.curve.len(), 3);
    assert_eq!(active_config.curve[0].temp, 40);
    assert_eq!(active_config.curve[1].speed, 60);

    // 5. Switch to a builtin profile.
    profile_proxy
        .set_active_profile("__quiet__", "ac")
        .await
        .unwrap();

    // 6. Switch back to the custom profile.
    profile_proxy
        .set_active_profile(&profile_id, "ac")
        .await
        .unwrap();

    // 7. The custom fan curve should be restored from disk, not reset to default.
    let restored = fan_proxy.get_active_fan_curve().await.unwrap();
    let restored_config: FanConfig = toml::from_str(&restored).unwrap();
    assert_eq!(
        restored_config.curve.len(),
        3,
        "curve should have 3 points after profile re-activation"
    );
    assert_eq!(restored_config.curve[0].temp, 40);
    assert_eq!(restored_config.curve[0].speed, 25);
    assert_eq!(restored_config.curve[1].temp, 65);
    assert_eq!(restored_config.curve[1].speed, 60);
    assert_eq!(restored_config.curve[2].temp, 85);
    assert_eq!(restored_config.curve[2].speed, 100);
    assert_eq!(restored_config.min_speed_percent, 30);

    // Clean up.
    profile_proxy.delete_profile(&profile_id).await.unwrap();
    daemon.stop().await;
}
