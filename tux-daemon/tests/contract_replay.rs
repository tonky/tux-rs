//! Deterministic contract replay tests based on captured fixture data.

mod common;

use serial_test::serial;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tux_core::backend::fan::FanBackend;
use tux_core::dbus_types::{FanData, FanHealthResponse};
use tux_core::device::DeviceDescriptor;
use tux_core::device_table;
use tux_core::dmi::{DetectedDevice, DmiInfo};
use tux_core::platform::Platform;
use tux_core::profile::ChargingSettings;
use zbus::proxy;

use common::{MockChargingBackend, TestDaemonBuilder};

const FIXTURE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, serde::Deserialize)]
struct Fixture {
    schema_version: u32,
    meta: FixtureMeta,
    raw: FixtureRaw,
    normalized: FixtureNormalized,
}

#[derive(Debug, serde::Deserialize)]
struct FixtureMeta {
    product_sku: String,
}

#[derive(Debug, serde::Deserialize)]
struct FixtureRaw {
    sysfs: std::collections::BTreeMap<String, String>,
    dbus: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, serde::Deserialize)]
struct FixtureNormalized {
    fans: Vec<FixtureFan>,
    charging: Option<FixtureCharging>,
    health: Option<FixtureHealth>,
}

#[derive(Debug, serde::Deserialize)]
struct FixtureFan {
    index: u32,
    temp_celsius: f32,
    duty_percent: u8,
    rpm: u32,
    rpm_available: bool,
}

#[derive(Debug, serde::Deserialize)]
struct FixtureCharging {
    profile: Option<String>,
    priority: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct FixtureHealth {
    status: String,
    consecutive_failures: u32,
}

#[proxy(
    interface = "com.tuxedocomputers.tccd.Fan",
    default_service = "com.tuxedocomputers.tccd",
    default_path = "/com/tuxedocomputers/tccd"
)]
trait Fan {
    fn get_fan_data(&self, fan_index: u32) -> zbus::Result<String>;
    fn get_fan_health(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "com.tuxedocomputers.tccd.Charging",
    default_service = "com.tuxedocomputers.tccd",
    default_path = "/com/tuxedocomputers/tccd"
)]
trait Charging {
    fn get_charging_settings(&self) -> zbus::Result<String>;
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    fixture_dir()
        .join("tests")
        .join("fixtures")
        .join("driver_contract")
        .join("uniwill")
        .join(name)
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_files() -> Vec<PathBuf> {
    let root = fixture_path("");
    let mut files = std::fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("failed to read fixture dir {}: {e}", root.display()))
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|e| e == "toml"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn load_fixture(path: &Path) -> Fixture {
    let data = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));
    let fixture: Fixture = toml::from_str(&data)
        .unwrap_or_else(|e| panic!("failed to parse fixture {}: {e}", path.display()));
    assert_eq!(
        fixture.schema_version,
        FIXTURE_SCHEMA_VERSION,
        "unsupported schema_version in {}",
        path.display()
    );
    fixture
}

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

fn replay_device_from_fixture(fixture: &Fixture) -> DetectedDevice {
    let required_fans = fixture
        .normalized
        .fans
        .iter()
        .map(|f| f.index as usize)
        .max()
        .map(|max_idx| max_idx + 1)
        .unwrap_or(1);

    if let Some(desc) = device_table::lookup_by_sku(&fixture.meta.product_sku)
        && (desc.fans.count as usize) >= required_fans
    {
        return make_detected_device(desc);
    }

    if let Some(desc) = device_table::devices_for_platform(Platform::Uniwill)
        .into_iter()
        .find(|d| (d.fans.count as usize) >= required_fans)
    {
        return make_detected_device(desc);
    }

    let fallback = device_table::fallback_for_platform(Platform::Uniwill);
    assert!(
        (fallback.fans.count as usize) >= required_fans,
        "fixture requires {required_fans} fan channels but fallback descriptor only has {}",
        fallback.fans.count
    );
    make_detected_device(fallback)
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn fixture_raw_payloads_match_normalized_sections() {
    for path in fixture_files() {
        let fixture = load_fixture(&path);

        if let Some(raw0) = fixture.raw.dbus.get("fan_data_0") {
            let parsed: FanData = toml::from_str(raw0)
                .unwrap_or_else(|e| panic!("fan_data_0 parse failed for {}: {e}", path.display()));
            let expected = fixture
                .normalized
                .fans
                .iter()
                .find(|f| f.index == 0)
                .unwrap_or_else(|| panic!("missing normalized fan index 0 in {}", path.display()));
            assert_eq!(parsed.rpm, expected.rpm, "fixture: {}", path.display());
            assert_eq!(
                parsed.duty_percent,
                expected.duty_percent,
                "fixture: {}",
                path.display()
            );
            assert_eq!(
                parsed.rpm_available,
                expected.rpm_available,
                "fixture: {}",
                path.display()
            );
            assert!(
                (parsed.temp_celsius - expected.temp_celsius).abs() < 0.001,
                "fixture: {}",
                path.display()
            );
        }

        if let Some(raw1) = fixture.raw.dbus.get("fan_data_1") {
            let parsed: FanData = toml::from_str(raw1)
                .unwrap_or_else(|e| panic!("fan_data_1 parse failed for {}: {e}", path.display()));
            let expected = fixture
                .normalized
                .fans
                .iter()
                .find(|f| f.index == 1)
                .unwrap_or_else(|| panic!("missing normalized fan index 1 in {}", path.display()));
            assert_eq!(parsed.rpm, expected.rpm, "fixture: {}", path.display());
            assert_eq!(
                parsed.duty_percent,
                expected.duty_percent,
                "fixture: {}",
                path.display()
            );
            assert_eq!(
                parsed.rpm_available,
                expected.rpm_available,
                "fixture: {}",
                path.display()
            );
            assert!(
                (parsed.temp_celsius - expected.temp_celsius).abs() < 0.001,
                "fixture: {}",
                path.display()
            );
        }

        if let Some(raw_health) = fixture.raw.dbus.get("fan_health") {
            let parsed: FanHealthResponse = toml::from_str(raw_health)
                .unwrap_or_else(|e| panic!("fan_health parse failed for {}: {e}", path.display()));
            let expected = fixture
                .normalized
                .health
                .as_ref()
                .unwrap_or_else(|| panic!("missing normalized health in {}", path.display()));
            assert_eq!(parsed.status, expected.status, "fixture: {}", path.display());
            assert_eq!(
                parsed.consecutive_failures,
                expected.consecutive_failures,
                "fixture: {}",
                path.display()
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn replay_fixture_matches_dbus_outputs() {
    for path in fixture_files() {
        let fixture = load_fixture(&path);
        let profile = fixture
            .normalized
            .charging
            .as_ref()
            .and_then(|c| c.profile.clone())
            .unwrap_or_else(|| "balanced".to_string());
        let priority = fixture
            .normalized
            .charging
            .as_ref()
            .and_then(|c| c.priority.clone())
            .unwrap_or_else(|| "charge_battery".to_string());

        let charging = Arc::new(MockChargingBackend::new(0, 0).with_profile(&profile, &priority));
        let profile_dir = tempfile::tempdir().expect("failed to create temporary profile dir");
        let device = replay_device_from_fixture(&fixture);

        let daemon = TestDaemonBuilder::new(&device, profile_dir.path())
            .with_charging(charging)
            .build()
            .await;

        // Replay fixture values into the mock backend.
        let temp = fixture
            .raw
            .sysfs
            .get("cpu_temp")
            .expect("cpu_temp is required")
            .parse::<u8>()
            .expect("cpu_temp must be an integer");
        daemon.fan_backend.set_temp(temp);

        let all_rpm_unavailable = fixture.normalized.fans.iter().all(|f| !f.rpm_available);
        daemon.fan_backend.set_rpm_unsupported(all_rpm_unavailable);

        for fan in &fixture.normalized.fans {
            daemon
                .fan_backend
                .write_pwm(fan.index as u8, fan.duty_percent)
                .unwrap_or_else(|e| panic!("failed writing pwm for fixture {}: {e}", path.display()));
            daemon.fan_backend.set_rpm(fan.index as u8, fan.rpm as u16);
        }

        let fan_proxy = FanProxy::new(&daemon.connection)
            .await
            .unwrap_or_else(|e| panic!("failed to connect fan proxy for {}: {e}", path.display()));
        for expected in &fixture.normalized.fans {
            let toml_str = fan_proxy
                .get_fan_data(expected.index)
                .await
                .unwrap_or_else(|e| {
                    panic!(
                        "failed to get fan_data_{} for {}: {e}",
                        expected.index,
                        path.display()
                    )
                });
            let actual: FanData = toml::from_str(&toml_str).unwrap_or_else(|e| {
                panic!(
                    "failed to parse fan_data_{} TOML for {}: {e}",
                    expected.index,
                    path.display()
                )
            });

            assert_eq!(actual.rpm, expected.rpm, "fixture: {}", path.display());
            assert_eq!(
                actual.duty_percent,
                expected.duty_percent,
                "fixture: {}",
                path.display()
            );
            assert_eq!(
                actual.rpm_available,
                expected.rpm_available,
                "fixture: {}",
                path.display()
            );
            // Mock backend has one shared temp value for all fan indices.
            assert!(
                (actual.temp_celsius - expected.temp_celsius).abs() < 0.001,
                "fixture: {}",
                path.display()
            );
        }

        let health_toml = fan_proxy
            .get_fan_health()
            .await
            .unwrap_or_else(|e| panic!("failed to get fan health for {}: {e}", path.display()));
        let health: FanHealthResponse = toml::from_str(&health_toml)
            .unwrap_or_else(|e| panic!("failed to parse fan health for {}: {e}", path.display()));
        let expected_health = fixture
            .normalized
            .health
            .as_ref()
            .unwrap_or_else(|| panic!("missing normalized.health in {}", path.display()));
        assert_eq!(health.status, expected_health.status, "fixture: {}", path.display());
        assert_eq!(
            health.consecutive_failures,
            expected_health.consecutive_failures,
            "fixture: {}",
            path.display()
        );

        let charging_proxy = ChargingProxy::new(&daemon.connection)
            .await
            .unwrap_or_else(|e| {
                panic!("failed to connect charging proxy for {}: {e}", path.display())
            });
        let charging_toml = charging_proxy
            .get_charging_settings()
            .await
            .unwrap_or_else(|e| panic!("failed to get charging settings for {}: {e}", path.display()));
        let charging: ChargingSettings = toml::from_str(&charging_toml)
            .unwrap_or_else(|e| panic!("failed to parse charging settings for {}: {e}", path.display()));
        let expected_charging = fixture
            .normalized
            .charging
            .as_ref()
            .unwrap_or_else(|| panic!("missing normalized.charging in {}", path.display()));
        assert_eq!(
            charging.profile,
            expected_charging.profile,
            "fixture: {}",
            path.display()
        );
        assert_eq!(
            charging.priority,
            expected_charging.priority,
            "fixture: {}",
            path.display()
        );

        daemon.stop().await;
    }
}
