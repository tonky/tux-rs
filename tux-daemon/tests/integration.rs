//! Integration tests: full daemon D-Bus roundtrips on the session bus.
//!
//! These tests start a real D-Bus service with mock backends and exercise
//! the D-Bus API. They require a running D-Bus session bus (available on
//! any Linux desktop; in CI use `dbus-run-session`).

mod common;

use serial_test::serial;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use tux_core::backend::fan::FanBackend;
use tux_core::dbus_types::{FanData, FanHealthResponse};
use tux_core::device_table;
use tux_core::dmi::{DetectedDevice, DmiInfo};
use tux_core::fan_curve::FanConfig;
use tux_core::platform::Platform;
use tux_core::profile::ChargingSettings;
use tux_daemon::charging::ChargingBackend;
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
    fn get_fan_data(&self, fan_index: u32) -> zbus::Result<String>;
    fn get_fan_health(&self) -> zbus::Result<String>;
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

/// D-Bus proxy for the Charging interface.
#[proxy(
    interface = "com.tuxedocomputers.tccd.Charging",
    default_service = "com.tuxedocomputers.tccd",
    default_path = "/com/tuxedocomputers/tccd"
)]
trait Charging {
    fn get_charging_settings(&self) -> zbus::Result<String>;
}

fn parse_fan_health(toml_str: &str) -> FanHealthResponse {
    toml::from_str(toml_str).expect("fan health TOML must deserialize")
}

async fn wait_for_fan_health<F>(
    proxy: &FanProxy<'_>,
    timeout: std::time::Duration,
    predicate: F,
) -> FanHealthResponse
where
    F: Fn(&FanHealthResponse) -> bool,
{
    let deadline = tokio::time::Instant::now() + timeout;
    let mut last = parse_fan_health(
        &proxy
            .get_fan_health()
            .await
            .expect("fan health call failed"),
    );
    if predicate(&last) {
        return last;
    }

    while tokio::time::Instant::now() < deadline {
        let current = parse_fan_health(
            &proxy
                .get_fan_health()
                .await
                .expect("fan health call failed"),
        );
        if predicate(&current) {
            return current;
        }
        last = current;
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    panic!(
        "timed out waiting for fan health condition; last status={} failures={}",
        last.status, last.consecutive_failures
    );
}

struct FlakyChargingBackend {
    start: std::sync::Mutex<u8>,
    end: std::sync::Mutex<u8>,
    profile: std::sync::Mutex<Option<String>>,
    priority: std::sync::Mutex<Option<String>>,
    remaining_read_failures: AtomicU8,
}

impl FlakyChargingBackend {
    fn new(start: u8, end: u8, profile: &str, priority: &str, read_failures: u8) -> Self {
        Self {
            start: std::sync::Mutex::new(start),
            end: std::sync::Mutex::new(end),
            profile: std::sync::Mutex::new(Some(profile.to_string())),
            priority: std::sync::Mutex::new(Some(priority.to_string())),
            remaining_read_failures: AtomicU8::new(read_failures),
        }
    }

    fn maybe_fail_read(&self) -> io::Result<()> {
        let remaining = self.remaining_read_failures.load(Ordering::Relaxed);
        if remaining > 0 {
            self.remaining_read_failures.fetch_sub(1, Ordering::Relaxed);
            return Err(io::Error::other(
                "simulated transient charging read failure",
            ));
        }
        Ok(())
    }
}

impl ChargingBackend for FlakyChargingBackend {
    fn get_start_threshold(&self) -> io::Result<u8> {
        self.maybe_fail_read()?;
        Ok(*self.start.lock().expect("start threshold lock poisoned"))
    }

    fn set_start_threshold(&self, pct: u8) -> io::Result<()> {
        *self.start.lock().expect("start threshold lock poisoned") = pct;
        Ok(())
    }

    fn get_end_threshold(&self) -> io::Result<u8> {
        self.maybe_fail_read()?;
        Ok(*self.end.lock().expect("end threshold lock poisoned"))
    }

    fn set_end_threshold(&self, pct: u8) -> io::Result<()> {
        *self.end.lock().expect("end threshold lock poisoned") = pct;
        Ok(())
    }

    fn get_profile(&self) -> io::Result<Option<String>> {
        self.maybe_fail_read()?;
        Ok(self.profile.lock().expect("profile lock poisoned").clone())
    }

    fn set_profile(&self, profile: &str) -> io::Result<()> {
        *self.profile.lock().expect("profile lock poisoned") = Some(profile.to_string());
        Ok(())
    }

    fn get_priority(&self) -> io::Result<Option<String>> {
        self.maybe_fail_read()?;
        Ok(self
            .priority
            .lock()
            .expect("priority lock poisoned")
            .clone())
    }

    fn set_priority(&self, priority: &str) -> io::Result<()> {
        *self.priority.lock().expect("priority lock poisoned") = Some(priority.to_string());
        Ok(())
    }
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

fn charging_test_device() -> DetectedDevice {
    if let Some(desc) = device_table::lookup_by_sku("STELLARIS1XI03") {
        return DetectedDevice {
            descriptor: desc,
            dmi: DmiInfo {
                board_vendor: "TUXEDO".to_string(),
                board_name: "TEST".to_string(),
                product_sku: desc.product_sku.to_string(),
                sys_vendor: "TUXEDO".to_string(),
                product_name: desc.name.to_string(),
                product_version: "1.0".to_string(),
            },
            exact_match: true,
        };
    }

    test_device()
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

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn fan_health_transitions_and_recovers_under_temp_failures() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = test_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;
    let proxy = FanProxy::new(&daemon.connection).await.unwrap();

    // Increase tick frequency to make failure-state transitions deterministic and fast.
    let fast_curve = r#"
mode = "CustomCurve"
min_speed_percent = 25
active_poll_ms = 10
idle_poll_ms = 10
hysteresis_degrees = 0

[[curve]]
temp = 0
speed = 20

[[curve]]
temp = 100
speed = 100
"#;
    proxy.set_fan_curve(fast_curve).await.unwrap();

    daemon.fan_backend.set_fail_temp(true);

    let degraded = wait_for_fan_health(&proxy, std::time::Duration::from_secs(2), |h| {
        h.status == "degraded" && h.consecutive_failures >= 5
    })
    .await;
    let failed = wait_for_fan_health(&proxy, std::time::Duration::from_secs(3), |h| {
        h.status == "failed" && h.consecutive_failures >= 30
    })
    .await;
    assert!(
        failed.consecutive_failures >= degraded.consecutive_failures,
        "failures should keep increasing while temp reads fail"
    );

    daemon.fan_backend.set_fail_temp(false);

    let recovered = wait_for_fan_health(&proxy, std::time::Duration::from_secs(2), |h| {
        h.status == "ok" && h.consecutive_failures == 0
    })
    .await;
    assert_eq!(recovered.status, "ok");
    assert_eq!(recovered.consecutive_failures, 0);

    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn fan_data_temp_gracefully_degrades_on_temp_read_failure() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = test_device();
    let daemon = common::TestDaemon::start(&device, profile_dir.path()).await;
    let proxy = FanProxy::new(&daemon.connection).await.unwrap();

    daemon.fan_backend.set_fail_temp(true);
    let data_toml = proxy.get_fan_data(0).await.unwrap();
    let data: FanData = toml::from_str(&data_toml).unwrap();

    assert_eq!(data.temp_celsius, 0.0);
    assert_eq!(data.rpm, 2400);

    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn charging_get_settings_retries_transient_read_failures() {
    let profile_dir = tempfile::tempdir().unwrap();
    let device = charging_test_device();
    let backend = Arc::new(FlakyChargingBackend::new(
        25,
        80,
        "balanced",
        "charge_battery",
        3,
    ));

    let daemon = common::TestDaemonBuilder::new(&device, profile_dir.path())
        .with_charging(backend.clone())
        .build()
        .await;
    let proxy = ChargingProxy::new(&daemon.connection).await.unwrap();

    let settings_toml = proxy.get_charging_settings().await.unwrap();
    let settings: ChargingSettings = toml::from_str(&settings_toml).unwrap();

    assert_eq!(settings.start_threshold, Some(25));
    assert_eq!(settings.end_threshold, Some(80));
    assert_eq!(settings.profile.as_deref(), Some("balanced"));
    assert_eq!(settings.priority.as_deref(), Some("charge_battery"));
    assert_eq!(backend.remaining_read_failures.load(Ordering::Relaxed), 0);

    daemon.stop().await;
}
