//! Fixture schema validation for driver-daemon contract captures.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use tux_core::dbus_types::{FanData, FanHealthResponse};

#[derive(Debug, Deserialize)]
struct Fixture {
    schema_version: u32,
    meta: Meta,
    raw: Raw,
    normalized: Normalized,
}

#[derive(Debug, Deserialize)]
struct Meta {
    fixture_id: String,
    platform: String,
    product_sku: String,
    captured_at: String,
    capture_tool_version: String,
    capture_source: String,
    kernel_release: String,
    daemon_version: String,
    driver_stack: String,
}

#[derive(Debug, Deserialize)]
struct Raw {
    sysfs: BTreeMap<String, String>,
    dbus: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct Normalized {
    fans: Vec<NormalizedFan>,
    charging: Option<NormalizedCharging>,
    health: Option<NormalizedHealth>,
}

#[derive(Debug, Deserialize)]
struct NormalizedFan {
    index: u32,
    temp_celsius: f32,
    duty_percent: u8,
    rpm: u32,
    rpm_available: bool,
}

#[derive(Debug, Deserialize)]
struct NormalizedCharging {
    profile: Option<String>,
    priority: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NormalizedHealth {
    status: String,
    consecutive_failures: u32,
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("driver_contract")
        .join("uniwill")
}

fn fixture_files() -> Vec<PathBuf> {
    let dir = fixture_dir();
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).expect("failed to read fixture directory") {
        let path = entry.expect("failed to read directory entry").path();
        if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            files.push(path);
        }
    }
    files.sort();
    files
}

fn parse_fixture(path: &PathBuf) -> Fixture {
    let data = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));
    toml::from_str::<Fixture>(&data)
        .unwrap_or_else(|e| panic!("failed to parse fixture {}: {e}", path.display()))
}

fn require_non_empty(value: &str, field: &str, path: &PathBuf) {
    assert!(
        !value.trim().is_empty(),
        "{} must be non-empty in {}",
        field,
        path.display()
    );
}

fn parse_required_i32(raw: &BTreeMap<String, String>, key: &str, path: &PathBuf) -> i32 {
    let value = raw
        .get(key)
        .unwrap_or_else(|| panic!("missing raw.sysfs key '{}' in {}", key, path.display()));
    assert!(
        !value.trim().is_empty(),
        "raw.sysfs.{} must be non-empty in {}",
        key,
        path.display()
    );
    value
        .parse::<i32>()
        .unwrap_or_else(|_| panic!("raw.sysfs.{} must be integer in {}", key, path.display()))
}

#[test]
fn has_at_least_one_fixture() {
    let files = fixture_files();
    assert!(
        !files.is_empty(),
        "expected at least one fixture under {}",
        fixture_dir().display()
    );
}

#[test]
fn fixture_schema_and_constraints_are_valid() {
    let files = fixture_files();

    for path in &files {
        let fixture = parse_fixture(path);

        assert_eq!(
            fixture.schema_version,
            1,
            "schema_version must be 1 in {}",
            path.display()
        );

        require_non_empty(&fixture.meta.fixture_id, "meta.fixture_id", path);
        assert_eq!(
            fixture.meta.platform,
            "Uniwill",
            "meta.platform must be Uniwill in {}",
            path.display()
        );
        require_non_empty(&fixture.meta.product_sku, "meta.product_sku", path);
        require_non_empty(&fixture.meta.captured_at, "meta.captured_at", path);
        require_non_empty(
            &fixture.meta.capture_tool_version,
            "meta.capture_tool_version",
            path,
        );
        assert!(
            matches!(
                fixture.meta.capture_source.as_str(),
                "manual-hardware" | "manual-sample" | "ci-synthetic"
            ),
            "meta.capture_source invalid in {}",
            path.display()
        );
        require_non_empty(&fixture.meta.kernel_release, "meta.kernel_release", path);
        require_non_empty(&fixture.meta.daemon_version, "meta.daemon_version", path);
        assert_eq!(
            fixture.meta.driver_stack,
            "tuxedo-drivers",
            "meta.driver_stack must be tuxedo-drivers in {}",
            path.display()
        );

        // Required raw keys for baseline Uniwill fixture.
        for key in [
            "cpu_temp",
            "fan1_pwm",
            "charging_profile",
            "charging_priority",
        ] {
            assert!(
                fixture.raw.sysfs.contains_key(key),
                "raw.sysfs missing key '{}' in {}",
                key,
                path.display()
            );
            assert!(
                !fixture.raw.sysfs[key].trim().is_empty(),
                "raw.sysfs.{} must be non-empty in {}",
                key,
                path.display()
            );
        }

        let cpu_temp = parse_required_i32(&fixture.raw.sysfs, "cpu_temp", path);
        assert!(
            (0..=120).contains(&cpu_temp),
            "cpu_temp out of range in {}",
            path.display()
        );

        let fan1_pwm = parse_required_i32(&fixture.raw.sysfs, "fan1_pwm", path);
        assert!(
            (0..=200).contains(&fan1_pwm),
            "fan1_pwm out of 0..200 in {}",
            path.display()
        );

        let mut fan2_pwm: Option<i32> = None;
        for pwm_key in ["fan2_pwm"] {
            if let Some(pwm_str) = fixture.raw.sysfs.get(pwm_key)
                && !pwm_str.is_empty()
            {
                let pwm: i32 = pwm_str
                    .parse()
                    .unwrap_or_else(|_| panic!("{} not integer in {}", pwm_key, path.display()));
                assert!(
                    (0..=200).contains(&pwm),
                    "{} out of 0..200 in {}",
                    pwm_key,
                    path.display()
                );
                fan2_pwm = Some(pwm);
            }
        }

        assert!(
            !fixture.normalized.fans.is_empty(),
            "normalized.fans must be non-empty in {}",
            path.display()
        );

        let mut seen = BTreeSet::new();
        for fan in &fixture.normalized.fans {
            assert!(
                seen.insert(fan.index),
                "duplicate normalized fan index {} in {}",
                fan.index,
                path.display()
            );
            assert!(
                (0.0..=120.0).contains(&fan.temp_celsius),
                "normalized temp_celsius out of range in {}",
                path.display()
            );
            assert!(
                fan.duty_percent <= 100,
                "normalized duty_percent out of range in {}",
                path.display()
            );
            if !fan.rpm_available {
                assert_eq!(
                    fan.rpm,
                    0,
                    "rpm must be 0 when rpm_available is false in {}",
                    path.display()
                );
            }
        }

        let raw_fan0 = fixture
            .raw
            .dbus
            .get("fan_data_0")
            .filter(|s| !s.trim().is_empty())
            .map(|raw| {
                toml::from_str::<FanData>(raw).unwrap_or_else(|e| {
                    panic!("raw.dbus.fan_data_0 invalid in {}: {e}", path.display())
                })
            });

        if let Some(fan0) = fixture.normalized.fans.iter().find(|f| f.index == 0) {
            if let Some(raw0) = &raw_fan0 {
                assert_eq!(
                    fan0.duty_percent,
                    raw0.duty_percent,
                    "fan0 duty_percent must match raw.dbus.fan_data_0 in {}",
                    path.display()
                );
                assert_eq!(
                    fan0.rpm,
                    raw0.rpm,
                    "fan0 rpm must match raw.dbus.fan_data_0 in {}",
                    path.display()
                );
                assert_eq!(
                    fan0.rpm_available,
                    raw0.rpm_available,
                    "fan0 rpm_available must match raw.dbus.fan_data_0 in {}",
                    path.display()
                );
                assert!(
                    (fan0.temp_celsius - raw0.temp_celsius).abs() < 0.001,
                    "fan0 temp_celsius must match raw.dbus.fan_data_0 in {}",
                    path.display()
                );
            } else {
                let expected = (fan1_pwm * 100 / 200) as u8;
                assert_eq!(
                    fan0.duty_percent,
                    expected,
                    "fan0 duty_percent must match fan1_pwm scaling when raw.dbus.fan_data_0 is absent in {}",
                    path.display()
                );
            }
        }

        let raw_fan1 = fixture
            .raw
            .dbus
            .get("fan_data_1")
            .filter(|s| !s.trim().is_empty())
            .map(|raw| {
                toml::from_str::<FanData>(raw).unwrap_or_else(|e| {
                    panic!("raw.dbus.fan_data_1 invalid in {}: {e}", path.display())
                })
            });

        if let Some(fan1) = fixture.normalized.fans.iter().find(|f| f.index == 1) {
            if let Some(raw1) = &raw_fan1 {
                assert_eq!(
                    fan1.duty_percent,
                    raw1.duty_percent,
                    "fan1 duty_percent must match raw.dbus.fan_data_1 in {}",
                    path.display()
                );
                assert_eq!(
                    fan1.rpm,
                    raw1.rpm,
                    "fan1 rpm must match raw.dbus.fan_data_1 in {}",
                    path.display()
                );
                assert_eq!(
                    fan1.rpm_available,
                    raw1.rpm_available,
                    "fan1 rpm_available must match raw.dbus.fan_data_1 in {}",
                    path.display()
                );
                assert!(
                    (fan1.temp_celsius - raw1.temp_celsius).abs() < 0.001,
                    "fan1 temp_celsius must match raw.dbus.fan_data_1 in {}",
                    path.display()
                );
            } else if let Some(raw_pwm) = fan2_pwm {
                let expected = (raw_pwm * 100 / 200) as u8;
                assert_eq!(
                    fan1.duty_percent,
                    expected,
                    "fan1 duty_percent must match fan2_pwm scaling when raw.dbus.fan_data_1 is absent in {}",
                    path.display()
                );
            }
        }

        if let Some(ch) = &fixture.normalized.charging {
            if let Some(profile) = &ch.profile {
                assert!(
                    matches!(
                        profile.as_str(),
                        "high_capacity" | "balanced" | "stationary"
                    ),
                    "invalid normalized.charging.profile in {}",
                    path.display()
                );
            }
            if let Some(priority) = &ch.priority {
                assert!(
                    matches!(priority.as_str(), "charge_battery" | "performance"),
                    "invalid normalized.charging.priority in {}",
                    path.display()
                );
            }
        }

        if let Some(health) = &fixture.normalized.health {
            assert!(
                matches!(health.status.as_str(), "ok" | "degraded" | "failed"),
                "invalid normalized.health.status in {}",
                path.display()
            );
            if health.status == "ok" {
                assert_eq!(
                    health.consecutive_failures,
                    0,
                    "ok status should have zero consecutive_failures in {}",
                    path.display()
                );
            }
        }

        if let Some(raw_fan_data) = fixture.raw.dbus.get("fan_data_0")
            && !raw_fan_data.trim().is_empty()
        {
            let parsed: FanData = toml::from_str(raw_fan_data).unwrap_or_else(|e| {
                panic!("raw.dbus.fan_data_0 invalid in {}: {e}", path.display())
            });
            assert!(parsed.duty_percent <= 100);
        } else {
            panic!(
                "raw.dbus.fan_data_0 must be non-empty in {}",
                path.display()
            );
        }

        if let Some(raw_fan_data) = fixture.raw.dbus.get("fan_data_1")
            && !raw_fan_data.trim().is_empty()
        {
            let parsed: FanData = toml::from_str(raw_fan_data).unwrap_or_else(|e| {
                panic!("raw.dbus.fan_data_1 invalid in {}: {e}", path.display())
            });
            assert!(parsed.duty_percent <= 100);
        }
        if let Some(raw_health) = fixture.raw.dbus.get("fan_health")
            && !raw_health.trim().is_empty()
        {
            let _parsed: FanHealthResponse = toml::from_str(raw_health).unwrap_or_else(|e| {
                panic!("raw.dbus.fan_health invalid in {}: {e}", path.display())
            });
        } else {
            panic!(
                "raw.dbus.fan_health must be non-empty in {}",
                path.display()
            );
        }

        if let Some(raw_charging) = fixture.raw.dbus.get("charging_settings")
            && !raw_charging.trim().is_empty()
        {
            let parsed = toml::from_str::<toml::Table>(raw_charging).unwrap_or_else(|e| {
                panic!(
                    "raw.dbus.charging_settings invalid table in {}: {e}",
                    path.display()
                )
            });
            assert!(
                parsed.contains_key("profile") || parsed.contains_key("priority"),
                "raw.dbus.charging_settings should include profile or priority in {}",
                path.display()
            );
        }
    }
}
