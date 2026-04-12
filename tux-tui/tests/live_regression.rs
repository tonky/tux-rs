//! Live regression test for TUXEDO InfinityBook Pro Gen8 (Uniwill).
//!
//! This test exercises every D-Bus API that the IBP Gen8 supports, using
//! the same `DaemonClient` library that the TUI uses.  It is `#[ignore]`d
//! by default; run with:
//!
//!     cargo test -p tux-tui --test live_regression -- --ignored --nocapture
//!
//! Or via Justfile:
//!
//!     just live-test
//!
//! Known IBP Gen8 MK1 capabilities:
//!   - Uniwill platform, 2 fans (CPU + GPU), max ~6000 RPM
//!   - RGB keyboard backlight (ITE)
//!   - Power profiles (governor + EPP)
//!   - Charging thresholds (declared but sysfs often absent)
//!   - NO TDP control, NO dedicated GPU power control
//!
//! All write operations use save/restore so the system is left unchanged.

use tux_core::dbus_types::{
    CapabilitiesResponse, DisplayState, ProfileAssignmentsResponse, ProfileList, SystemInfoResponse,
};
use tux_core::fan_curve::{FanConfig, FanCurvePoint, FanMode};
use tux_core::profile::ChargingSettings;
use tux_tui::dbus_client::DaemonClient;

/// Keyboard state as stored by the daemon's Settings interface.
/// Mirrors `tux_daemon::dbus::settings::KeyboardState`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
struct KeyboardState {
    #[serde(default)]
    brightness: i64,
    #[serde(default)]
    color: String,
    #[serde(default)]
    mode: String,
}

/// Connect to whichever bus has the daemon running.
async fn connect() -> DaemonClient {
    if let Ok(c) = DaemonClient::connect(false).await
        && c.get_power_state().await.is_ok()
    {
        return c;
    }
    DaemonClient::connect(true)
        .await
        .expect("daemon not reachable on system or session bus")
}

fn section(name: &str) {
    println!("\n{}", "=".repeat(60));
    println!("  {name}");
    println!("{}", "=".repeat(60));
}

async fn get_charging_settings_retry(client: &DaemonClient) -> Result<String, zbus::Error> {
    let mut last_err: Option<zbus::Error> = None;
    for _ in 0..10 {
        match client.get_charging_settings().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }
    }
    Err(last_err.expect("retry loop must set last_err"))
}

// ── The main test ──────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn ibp_gen8_live_regression() {
    let client = connect().await;

    // ── 1. Device identity (must be IBP Gen8) ──────────────────
    section("Device Identity");

    let device_name: String = client
        .get_device_property("DeviceName")
        .await
        .and_then(|v| String::try_from(v).map_err(|e| zbus::Error::Failure(e.to_string())))
        .expect("DeviceName property");
    assert!(
        device_name.contains("InfinityBook Pro") && device_name.contains("Gen8"),
        "expected InfinityBook Pro Gen8, got: {device_name}"
    );
    println!("  device_name={device_name}");

    let platform: String = client
        .get_device_property("Platform")
        .await
        .and_then(|v| String::try_from(v).map_err(|e| zbus::Error::Failure(e.to_string())))
        .expect("Platform property");
    assert_eq!(
        platform, "Uniwill",
        "expected Uniwill platform, got: {platform}"
    );
    println!("  platform={platform}");

    let daemon_ver: String = client
        .get_device_property("DaemonVersion")
        .await
        .and_then(|v| String::try_from(v).map_err(|e| zbus::Error::Failure(e.to_string())))
        .expect("DaemonVersion property");
    assert!(!daemon_ver.is_empty());
    println!("  daemon_version={daemon_ver}");

    // ── 2. System info ─────────────────────────────────────────
    section("System Info");

    let sys_toml = client
        .get_system_info()
        .await
        .expect("get_system_info failed");
    let sys: SystemInfoResponse = toml::from_str(&sys_toml).expect("bad SystemInfoResponse TOML");
    assert!(!sys.version.is_empty());
    assert!(!sys.hostname.is_empty());
    assert!(!sys.kernel.is_empty());
    println!(
        "  version={} hostname={} kernel={}",
        sys.version, sys.hostname, sys.kernel
    );

    let power = client
        .get_power_state()
        .await
        .expect("get_power_state failed");
    assert!(
        power == "ac" || power == "battery",
        "unexpected power state: {power}"
    );
    println!("  power_state={power}");

    let cpu_freq = client
        .get_cpu_frequency()
        .await
        .expect("get_cpu_frequency failed");
    assert!(cpu_freq > 0, "cpu_frequency is 0");
    println!("  cpu_frequency={cpu_freq} MHz");

    let cpu_count = client.get_cpu_count().await.expect("get_cpu_count failed");
    assert!(
        cpu_count >= 14,
        "expected ≥14 cores for IBP Gen8, got {cpu_count}"
    );
    println!("  cpu_count={cpu_count}");

    let profile_name = client
        .get_active_profile_name()
        .await
        .expect("get_active_profile_name failed");
    assert!(!profile_name.is_empty());
    println!("  active_profile={profile_name}");

    // ── 2b. CPU load & per-core frequencies ────────────────────
    section("CPU Load & Per-Core Frequencies");

    let cpu_load = client.get_cpu_load().await.expect("get_cpu_load failed");
    assert!(!cpu_load.is_empty(), "cpu load should be non-empty");
    println!("  cpu_load: {}", cpu_load.trim().replace('\n', " | "));

    let per_core = client
        .get_per_core_frequencies()
        .await
        .expect("get_per_core_frequencies failed");
    assert!(
        !per_core.is_empty(),
        "per-core frequencies should be non-empty"
    );
    println!(
        "  per_core_frequencies: {} entries",
        per_core.lines().count()
    );

    // ── 3. Capabilities — assert IBP Gen8 expectations ─────────
    section("Capabilities");

    let caps_toml = client
        .get_capabilities()
        .await
        .expect("get_capabilities failed");
    let caps: CapabilitiesResponse =
        toml::from_str(&caps_toml).expect("bad CapabilitiesResponse TOML");

    assert!(caps.fan_control, "IBP Gen8 must have fan_control");
    assert_eq!(caps.fan_count, 2, "IBP Gen8 has 2 fans");
    assert!(caps.power_profiles, "IBP Gen8 must have power profiles");
    // TDP and GPU control are NOT available on this model.
    assert!(!caps.tdp_control, "IBP Gen8 should not have TDP control");
    assert!(!caps.gpu_control, "IBP Gen8 should not have GPU control");

    println!(
        "  fan_control={} fan_count={}",
        caps.fan_control, caps.fan_count
    );
    println!(
        "  keyboard_backlight={} keyboard_type={}",
        caps.keyboard_backlight, caps.keyboard_type
    );
    if !caps.keyboard_backlight {
        println!("  keyboard backlight backend unavailable; keyboard illumination tests will be skipped");
    }
    println!(
        "  charging_thresholds={} charging_profiles={}",
        caps.charging_thresholds, caps.charging_profiles
    );
    println!(
        "  tdp_control={} power_profiles={} gpu_control={}",
        caps.tdp_control, caps.power_profiles, caps.gpu_control
    );

    // ── 4. Fan control — full exercise ─────────────────────────
    section("Fan Control — Sensors & Speeds");

    let fan_info = client.get_fan_info().await.expect("get_fan_info failed");
    let num_fans = fan_info.3;
    assert_eq!(num_fans, 2, "IBP Gen8 has 2 fans");
    println!(
        "  num_fans={} max_rpm={} min_rpm={} multi_fan={}",
        num_fans, fan_info.0, fan_info.1, fan_info.2
    );

    // Temperature.
    let temp = client
        .get_temperature(0)
        .await
        .expect("get_temperature failed");
    assert!(
        (20_000..=100_000).contains(&temp),
        "temperature {temp} millidegrees out of range for normal operation"
    );
    println!("  cpu_temp={:.1}°C", temp as f32 / 1000.0);

    // Fan speeds for both fans.
    for i in 0..2u32 {
        let rpm = client.get_fan_speed(i).await.expect("get_fan_speed failed");
        println!("  fan[{i}] rpm={rpm}");
    }

    // ── 4a. Fan curve save/restore ─────────────────────────────
    section("Fan Control — Curve Save/Restore");

    let orig_curve_toml = client
        .get_active_fan_curve()
        .await
        .expect("get_active_fan_curve failed");
    let orig_config: FanConfig = toml::from_str(&orig_curve_toml).expect("bad FanConfig TOML");
    println!(
        "  original: {} points, mode={:?}, min_speed={}%",
        orig_config.curve.len(),
        orig_config.mode,
        orig_config.min_speed_percent
    );

    // Set a constant 60% curve across all temperatures.
    // This should make both fans spin at 60% regardless of CPU temp.
    let constant_60 = FanConfig {
        mode: FanMode::CustomCurve,
        min_speed_percent: 0, // let curve value dominate
        curve: vec![
            FanCurvePoint { temp: 0, speed: 60 },
            FanCurvePoint {
                temp: 100,
                speed: 60,
            },
        ],
        active_poll_ms: 500, // poll fast for quicker convergence
        idle_poll_ms: 500,
        hysteresis_degrees: 0, // no hysteresis — apply every tick
    };
    let constant_60_toml = toml::to_string_pretty(&constant_60).unwrap();
    client
        .set_fan_curve(&constant_60_toml)
        .await
        .expect("set_fan_curve(constant 60%) failed");

    // Readback the curve to verify it was stored correctly.
    let readback_toml = client
        .get_active_fan_curve()
        .await
        .expect("readback get_active_fan_curve failed");
    let readback: FanConfig = toml::from_str(&readback_toml).unwrap();
    assert_eq!(readback.curve.len(), 2);
    assert_eq!(readback.curve[0].speed, 60);
    assert_eq!(readback.curve[1].speed, 60);
    assert_eq!(readback.mode, FanMode::CustomCurve);
    println!("  constant 60% curve set: OK");

    // Wait for the fan engine to poll and apply the new curve.
    // The engine polls every active_poll_ms (500ms), give it 3 cycles + ramp time.
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;

    // Read back fan speeds. On Uniwill, GetFanSpeed returns synthetic RPM
    // derived from PWM: (pwm * 6000) / 255.
    // 60% → PWM 153 → synthetic RPM ≈ 3600.
    // Allow a wide tolerance (2400–4800, i.e. 40-80%) to account for EC
    // rounding and the pwm→ec→pwm conversion chain.
    let expected_min_rpm: u32 = 2400; // 40% of 6000
    let expected_max_rpm: u32 = 4800; // 80% of 6000
    for i in 0..2u32 {
        let rpm = client.get_fan_speed(i).await.expect("get_fan_speed failed");
        assert!(
            (expected_min_rpm..=expected_max_rpm).contains(&rpm),
            "fan[{i}] rpm={rpm} — expected {expected_min_rpm}..={expected_max_rpm} \
             for constant 60% curve"
        );
        println!("  fan[{i}] rpm={rpm} (expected ~3600) — OK");
    }

    // Restore original curve.
    client
        .set_fan_curve(&orig_curve_toml)
        .await
        .expect("restore fan curve failed");
    println!("  original curve restored");

    // ── 4b. Direct fan speed control ───────────────────────────
    section("Fan Control — Manual PWM + Auto Restore");

    // Switch to manual mode.
    client
        .set_fan_mode("manual")
        .await
        .expect("set_fan_mode(manual) failed");
    println!("  set_fan_mode(manual): OK");

    // Drive fan 0 to ~50% PWM (128/255).
    client
        .set_fan_speed(0, 128)
        .await
        .expect("set_fan_speed(0, 128) failed");
    println!("  set_fan_speed(0, 128): OK");

    // Wait for EC to reflect the new PWM.
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Read back — should report speed corresponding to ~50% PWM.
    // PWM 128 → synthetic RPM = (128 * 6000) / 255 ≈ 3012.
    let rpm_fan0 = client
        .get_fan_speed(0)
        .await
        .expect("get_fan_speed(0) failed after manual set");
    assert!(
        rpm_fan0 >= 1500,
        "fan[0] rpm={rpm_fan0} too low after PWM 128 — expected ≥1500"
    );
    println!("  fan[0] rpm={rpm_fan0} after PWM 128 — OK");

    // Drive fan 1 to ~75% PWM (191/255).
    // NOTE: On InfinityBook Pro Gen8 the EC links both fans, so fan 1 may not
    // reach 191 independently — just verify we can set it and it responds.
    client
        .set_fan_speed(1, 191)
        .await
        .expect("set_fan_speed(1, 191) failed");
    println!("  set_fan_speed(1, 191): OK");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let rpm_fan1 = client
        .get_fan_speed(1)
        .await
        .expect("get_fan_speed(1) failed after manual set");
    // EC may link fans, so just verify they're spinning (≥1500 RPM).
    assert!(
        rpm_fan1 >= 1500,
        "fan[1] rpm={rpm_fan1} too low after PWM 191 — expected ≥1500"
    );
    println!("  fan[1] rpm={rpm_fan1} after PWM 191 — OK");

    // Restore auto mode for both fans.
    client
        .set_auto_mode(0)
        .await
        .expect("set_auto_mode(0) failed");
    client
        .set_auto_mode(1)
        .await
        .expect("set_auto_mode(1) failed");
    println!("  set_auto_mode(0) + set_auto_mode(1): OK");

    // Go back to auto fan mode.
    client
        .set_fan_mode("auto")
        .await
        .expect("set_fan_mode(auto) failed");
    println!("  set_fan_mode(auto): OK — fans restored");

    // ── 5. Profiles — CRUD + activate ──────────────────────────
    section("Profiles — List & Assignments");

    let profiles_toml = client.list_profiles().await.expect("list_profiles failed");
    let profile_list: ProfileList = toml::from_str(&profiles_toml).expect("bad ProfileList TOML");
    assert!(!profile_list.profiles.is_empty(), "profile list is empty");
    println!("  {} profiles:", profile_list.profiles.len());
    for p in &profile_list.profiles {
        println!("    {} — {}", p.id, p.name);
    }

    // Save original assignments.
    let orig_assignments_toml = client
        .get_profile_assignments()
        .await
        .expect("get_profile_assignments failed");
    let orig_assignments: ProfileAssignmentsResponse =
        toml::from_str(&orig_assignments_toml).expect("bad assignments TOML");
    println!(
        "  assignments: ac={} battery={}",
        orig_assignments.ac_profile, orig_assignments.battery_profile
    );

    // ── 5a. Copy + update profile ──────────────────────────────
    section("Profiles — Copy + Update");

    // Copy a builtin profile.
    let copy_id = client
        .copy_profile("__office__")
        .await
        .expect("copy_profile failed");
    println!("  copied __office__ → {copy_id}");

    // Build a profile update that includes charging settings appropriate
    // for the detected platform (Uniwill profiles vs Clevo thresholds).
    let charging_section = if caps.charging_profiles {
        r#"
[charging]
profile = "balanced"
priority = "performance"
"#
        .to_string()
    } else if caps.charging_thresholds {
        r#"
[charging]
start_threshold = 40
end_threshold = 80
"#
        .to_string()
    } else {
        String::new()
    };

    // Update the copy (including charging settings when available).
    let update_toml = format!(
        r#"
id = "{copy_id}"
name = "IBP Gen8 Live Test"

[fan]
mode = "Auto"
min_speed_percent = 15

[cpu]
governor = "powersave"
{charging_section}"#
    );
    client
        .update_profile(&copy_id, &update_toml)
        .await
        .expect("update_profile failed");
    println!("  updated {copy_id}");

    // Verify the update.
    let updated_list_toml = client.list_profiles().await.unwrap();
    assert!(
        updated_list_toml.contains("IBP Gen8 Live Test"),
        "updated profile not found in list"
    );

    // ── 5b. Activate profile for AC ────────────────────────────
    section("Profiles — Activate AC");

    // Set as active AC profile.
    client
        .set_active_profile(&copy_id, "ac")
        .await
        .expect("set_active_profile(ac) failed");
    let new_assignments_toml = client.get_profile_assignments().await.unwrap();
    let new_assignments: ProfileAssignmentsResponse =
        toml::from_str(&new_assignments_toml).unwrap();
    assert_eq!(new_assignments.ac_profile, copy_id);
    println!("  set {copy_id} as ac profile");

    // Verify that activating the profile applied its charging settings.
    // Charging settings are applied for the currently active power source.
    if (caps.charging_profiles || caps.charging_thresholds) && power == "ac" {
        let mut applied: Option<ChargingSettings> = None;
        for _ in 0..10 {
            if let Ok(applied_toml) = client.get_charging_settings().await
                && let Ok(parsed) = toml::from_str::<ChargingSettings>(&applied_toml)
            {
                let matches = if caps.charging_profiles {
                    parsed.profile.as_deref() == Some("balanced")
                        && parsed.priority.as_deref() == Some("performance")
                } else {
                    parsed.start_threshold == Some(40) && parsed.end_threshold == Some(80)
                };
                applied = Some(parsed);
                if matches {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
        let applied = applied.expect("charging readback after profile activation failed");
        if caps.charging_profiles {
            assert_eq!(
                applied.profile.as_deref(),
                Some("balanced"),
                "profile activation should have set charging profile to 'balanced'"
            );
            assert_eq!(
                applied.priority.as_deref(),
                Some("performance"),
                "profile activation should have set charging priority to 'performance'"
            );
            println!(
                "  profile charging applied: profile=balanced, priority=performance — OK"
            );
        } else {
            assert_eq!(
                applied.start_threshold,
                Some(40),
                "profile activation should have set start_threshold=40"
            );
            assert_eq!(
                applied.end_threshold,
                Some(80),
                "profile activation should have set end_threshold=80"
            );
            println!("  profile charging applied: start=40%, end=80% — OK");
        }
    } else if caps.charging_profiles || caps.charging_thresholds {
        println!("  skipping AC charging-apply verification (current power_state={power})");
    }

    // ── 5c. Activate profile for battery ───────────────────────
    section("Profiles — Activate Battery");

    client
        .set_active_profile(&copy_id, "battery")
        .await
        .expect("set_active_profile(battery) failed");
    let bat_assignments_toml = client.get_profile_assignments().await.unwrap();
    let bat_assignments: ProfileAssignmentsResponse =
        toml::from_str(&bat_assignments_toml).unwrap();
    assert_eq!(bat_assignments.battery_profile, copy_id);
    println!("  set {copy_id} as battery profile: OK");

    // If we're currently on battery, verify charging settings apply now.
    if (caps.charging_profiles || caps.charging_thresholds) && power == "battery" {
        let mut applied: Option<ChargingSettings> = None;
        for _ in 0..10 {
            if let Ok(applied_toml) = client.get_charging_settings().await
                && let Ok(parsed) = toml::from_str::<ChargingSettings>(&applied_toml)
            {
                let matches = if caps.charging_profiles {
                    parsed.profile.as_deref() == Some("balanced")
                        && parsed.priority.as_deref() == Some("performance")
                } else {
                    parsed.start_threshold == Some(40) && parsed.end_threshold == Some(80)
                };
                applied = Some(parsed);
                if matches {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
        let applied = applied.expect("charging readback after battery profile activation failed");
        if caps.charging_profiles {
            assert_eq!(
                applied.profile.as_deref(),
                Some("balanced"),
                "battery profile activation should have set charging profile to 'balanced'"
            );
            assert_eq!(
                applied.priority.as_deref(),
                Some("performance"),
                "battery profile activation should have set charging priority to 'performance'"
            );
            println!(
                "  battery profile charging applied: profile=balanced, priority=performance — OK"
            );
        } else {
            assert_eq!(
                applied.start_threshold,
                Some(40),
                "battery profile activation should have set start_threshold=40"
            );
            assert_eq!(
                applied.end_threshold,
                Some(80),
                "battery profile activation should have set end_threshold=80"
            );
            println!("  battery profile charging applied: start=40%, end=80% — OK");
        }
    }

    // Restore original assignments.
    client
        .set_active_profile(&orig_assignments.ac_profile, "ac")
        .await
        .expect("restore ac profile failed");
    client
        .set_active_profile(&orig_assignments.battery_profile, "battery")
        .await
        .expect("restore battery profile failed");
    println!(
        "  restored ac={} battery={}",
        orig_assignments.ac_profile, orig_assignments.battery_profile
    );

    // Delete the copy.
    client
        .delete_profile(&copy_id)
        .await
        .expect("delete_profile failed");
    println!("  deleted {copy_id}");

    // ── 5d. Create profile from scratch ────────────────────────
    section("Profiles — Create From Scratch");

    // Clean up stale profile from a previously failed test run.
    let _ = client.delete_profile("live-test-scratch").await;

    let create_toml = r#"
id = "live-test-scratch"
name = "Scratch Test"
description = "Created from scratch in live regression test"

[fan]
mode = "Auto"
min_speed_percent = 10

[cpu]
governor = "powersave"
"#;
    let scratch_id = client
        .create_profile(create_toml)
        .await
        .expect("create_profile failed");
    assert_eq!(scratch_id, "live-test-scratch");
    println!("  created scratch profile: {scratch_id}");

    // Verify it appears in the list.
    let list_after_create = client.list_profiles().await.unwrap();
    assert!(
        list_after_create.contains("Scratch Test"),
        "created profile not in list"
    );
    println!("  verified in profile list: OK");

    // Clean up.
    client
        .delete_profile(&scratch_id)
        .await
        .expect("delete scratch profile failed");
    let list_after_delete = client.list_profiles().await.unwrap();
    assert!(
        !list_after_delete.contains(&scratch_id),
        "scratch profile still in list after delete"
    );
    println!("  deleted scratch profile: OK");

    // ── 5e. Fan curve persistence through profile lifecycle ────
    section("Profiles — Fan Curve Persistence (regression)");

    // This tests the reported bug: "activate custom profile but curve is gone".
    // The curve must survive: create → list → activate → get_active_fan_curve.

    // Clean up stale profile from a previously failed test run.
    let _ = client.delete_profile("live-test-curve").await;

    let test_curve = vec![
        FanCurvePoint {
            temp: 30,
            speed: 20,
        },
        FanCurvePoint {
            temp: 50,
            speed: 40,
        },
        FanCurvePoint {
            temp: 70,
            speed: 65,
        },
        FanCurvePoint {
            temp: 85,
            speed: 90,
        },
        FanCurvePoint {
            temp: 95,
            speed: 100,
        },
    ];

    let curve_profile_toml = format!(
        r#"
id = "live-test-curve"
name = "Curve Persistence Test"

[fan]
mode = "CustomCurve"
min_speed_percent = 5

[[fan.curve]]
temp = 30
speed = 20

[[fan.curve]]
temp = 50
speed = 40

[[fan.curve]]
temp = 70
speed = 65

[[fan.curve]]
temp = 85
speed = 90

[[fan.curve]]
temp = 95
speed = 100

[cpu]
governor = "powersave"
"#
    );
    let curve_profile_id = client
        .create_profile(&curve_profile_toml)
        .await
        .expect("create curve profile failed");
    println!("  created curve profile: {curve_profile_id}");

    // Step 1: Verify curve is stored in the profile list.
    let list_toml = client.list_profiles().await.unwrap();
    let list: ProfileList = toml::from_str(&list_toml).unwrap();
    let stored = list
        .profiles
        .iter()
        .find(|p| p.id == curve_profile_id)
        .expect("curve profile not in list");
    assert_eq!(
        stored.fan.curve.len(),
        5,
        "curve should have 5 points in list, got {}",
        stored.fan.curve.len()
    );
    assert_eq!(stored.fan.curve[0].temp, 30);
    assert_eq!(stored.fan.curve[0].speed, 20);
    assert_eq!(stored.fan.curve[4].temp, 95);
    assert_eq!(stored.fan.curve[4].speed, 100);
    assert_eq!(stored.fan.mode, FanMode::CustomCurve);
    println!(
        "  list_profiles curve: {} points — OK",
        stored.fan.curve.len()
    );

    // Step 2: Activate the profile and verify the runtime fan curve.
    let _orig_curve_before_activation = client
        .get_active_fan_curve()
        .await
        .expect("get_active_fan_curve failed before activation");

    client
        .set_active_profile(&curve_profile_id, "ac")
        .await
        .expect("activate curve profile failed");
    // Also set as battery profile so it applies on either power state.
    client
        .set_active_profile(&curve_profile_id, "battery")
        .await
        .expect("activate curve profile as battery failed");
    println!("  activated curve profile as AC + battery");

    // Wait for the profile applier to apply the curve.
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    let active_curve_toml = client
        .get_active_fan_curve()
        .await
        .expect("get_active_fan_curve failed");
    let active_config: FanConfig =
        toml::from_str(&active_curve_toml).expect("bad FanConfig TOML after activation");
    assert_eq!(
        active_config.mode,
        FanMode::CustomCurve,
        "active fan mode should be CustomCurve after profile activation"
    );
    assert_eq!(
        active_config.curve.len(),
        5,
        "active curve should have 5 points after profile activation, got {}: {:?}",
        active_config.curve.len(),
        active_config.curve
    );
    for (i, (got, want)) in active_config
        .curve
        .iter()
        .zip(test_curve.iter())
        .enumerate()
    {
        assert_eq!(
            got.temp, want.temp,
            "curve point {i} temp mismatch: got {}, want {}",
            got.temp, want.temp
        );
        assert_eq!(
            got.speed, want.speed,
            "curve point {i} speed mismatch: got {}, want {}",
            got.speed, want.speed
        );
    }
    println!(
        "  active fan curve after activation: {} points, all match — OK",
        active_config.curve.len()
    );

    // Step 3: Update profile (change name only) and verify curve survives.
    // This tests the partial-update scenario where the TUI might send
    // incomplete TOML that drops the curve.
    let update_with_curve_toml = format!(
        r#"
id = "live-test-curve"
name = "Curve Persistence UPDATED"

[fan]
mode = "CustomCurve"
min_speed_percent = 5

[[fan.curve]]
temp = 30
speed = 20

[[fan.curve]]
temp = 50
speed = 40

[[fan.curve]]
temp = 70
speed = 65

[[fan.curve]]
temp = 85
speed = 90

[[fan.curve]]
temp = 95
speed = 100

[cpu]
governor = "powersave"
"#
    );
    client
        .update_profile(&curve_profile_id, &update_with_curve_toml)
        .await
        .expect("update curve profile failed");

    // Re-read and verify curve is still there.
    let list_after_update_toml = client.list_profiles().await.unwrap();
    let list_after_update: ProfileList = toml::from_str(&list_after_update_toml).unwrap();
    let updated_profile = list_after_update
        .profiles
        .iter()
        .find(|p| p.id == curve_profile_id)
        .expect("curve profile not in list after update");
    assert_eq!(
        updated_profile.name, "Curve Persistence UPDATED",
        "profile name should be updated"
    );
    assert_eq!(
        updated_profile.fan.curve.len(),
        5,
        "curve should still have 5 points after update, got {}",
        updated_profile.fan.curve.len()
    );
    println!(
        "  curve survives update: {} points — OK",
        updated_profile.fan.curve.len()
    );

    // Step 4: Re-activate and verify curve again.
    client
        .set_active_profile(&curve_profile_id, "ac")
        .await
        .expect("re-activate curve profile failed");
    client
        .set_active_profile(&curve_profile_id, "battery")
        .await
        .expect("re-activate curve profile as battery failed");
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    let reactivate_toml = client
        .get_active_fan_curve()
        .await
        .expect("get_active_fan_curve failed after re-activation");
    let reactivate_config: FanConfig = toml::from_str(&reactivate_toml).unwrap();
    assert_eq!(
        reactivate_config.curve.len(),
        5,
        "curve should still have 5 points after re-activation, got {}",
        reactivate_config.curve.len()
    );
    println!(
        "  curve survives re-activate: {} points — OK",
        reactivate_config.curve.len()
    );

    // Restore original AC + battery profiles and clean up.
    client
        .set_active_profile(&orig_assignments.ac_profile, "ac")
        .await
        .expect("restore ac profile failed");
    client
        .set_active_profile(&orig_assignments.battery_profile, "battery")
        .await
        .expect("restore battery profile failed");
    client
        .delete_profile(&curve_profile_id)
        .await
        .expect("delete curve profile failed");
    println!("  cleaned up curve profile");

    // Verify deletion.
    let final_list_toml = client.list_profiles().await.unwrap();
    assert!(
        !final_list_toml.contains(&curve_profile_id),
        "deleted profile still in list"
    );

    // ── 6. Global settings (save/restore) ──────────────────────
    section("Global Settings");

    let orig_settings = client
        .get_global_settings()
        .await
        .expect("get_global_settings failed");
    println!("  original: {}", orig_settings.trim().replace('\n', " | "));

    // Toggle temperature_unit.
    let has_fahrenheit = orig_settings.contains("fahrenheit");
    let test_unit = if has_fahrenheit {
        "celsius"
    } else {
        "fahrenheit"
    };
    let test_settings = format!(
        "temperature_unit = \"{test_unit}\"\nfan_control_enabled = true\ncpu_settings_enabled = true\n"
    );
    client
        .set_global_settings(&test_settings)
        .await
        .expect("set_global_settings failed");

    let readback = client.get_global_settings().await.unwrap();
    assert!(
        readback.contains(test_unit),
        "settings readback doesn't contain {test_unit}"
    );
    println!("  toggle to {test_unit}: OK");

    // Restore.
    client
        .set_global_settings(&orig_settings)
        .await
        .expect("restore global settings failed");
    println!("  restored");

    // ── 7. Keyboard backlight (thorough save/restore) ────────────
    if caps.keyboard_backlight {
        section("Keyboard Backlight");

        let orig_kbd = client
            .get_keyboard_state()
            .await
            .expect("get_keyboard_state failed");
        let orig_kbd_parsed: KeyboardState =
            toml::from_str(&orig_kbd).expect("bad KeyboardState TOML from daemon");
        println!(
            "  original: brightness={} color={:?} mode={:?}",
            orig_kbd_parsed.brightness, orig_kbd_parsed.color, orig_kbd_parsed.mode
        );

        // Test: set brightness to 75, mode to "Static", color to "#ff0000".
        let test_kbd_state = KeyboardState {
            brightness: 75,
            color: "#ff0000".into(),
            mode: "Static".into(),
        };
        let test_kbd_toml = toml::to_string(&test_kbd_state).unwrap();
        client
            .set_keyboard_state(&test_kbd_toml)
            .await
            .expect("set_keyboard_state(75, Static, #ff0000) failed");

        let rb1 = client.get_keyboard_state().await.unwrap();
        let rb1_parsed: KeyboardState = toml::from_str(&rb1).expect("bad readback TOML");
        assert_eq!(rb1_parsed.brightness, 75, "brightness should be 75");
        assert_eq!(rb1_parsed.color, "#ff0000", "color should be #ff0000");
        assert_eq!(rb1_parsed.mode, "Static", "mode should be Static");
        println!("  set brightness=75, color=#ff0000, mode=Static: OK");

        // Test: set brightness to 0 (off).
        let off_state = KeyboardState {
            brightness: 0,
            color: "".into(),
            mode: "".into(),
        };
        let off_toml = toml::to_string(&off_state).unwrap();
        client
            .set_keyboard_state(&off_toml)
            .await
            .expect("set_keyboard_state(0) failed");

        let rb2 = client.get_keyboard_state().await.unwrap();
        let rb2_parsed: KeyboardState = toml::from_str(&rb2).unwrap();
        assert_eq!(rb2_parsed.brightness, 0, "brightness should be 0");
        println!("  set brightness=0 (off): OK");

        // Test: set brightness to 100 (max).
        let max_state = KeyboardState {
            brightness: 100,
            color: "#ffffff".into(),
            mode: "Breathe".into(),
        };
        let max_toml = toml::to_string(&max_state).unwrap();
        client
            .set_keyboard_state(&max_toml)
            .await
            .expect("set_keyboard_state(100, Breathe) failed");

        let rb3 = client.get_keyboard_state().await.unwrap();
        let rb3_parsed: KeyboardState = toml::from_str(&rb3).unwrap();
        assert_eq!(rb3_parsed.brightness, 100, "brightness should be 100");
        assert_eq!(rb3_parsed.mode, "Breathe", "mode should be Breathe");
        println!("  set brightness=100, mode=Breathe: OK");

        // Restore original.
        client
            .set_keyboard_state(&orig_kbd)
            .await
            .expect("restore keyboard state failed");
        let restored = client.get_keyboard_state().await.unwrap();
        let restored_parsed: KeyboardState = toml::from_str(&restored).unwrap();
        assert_eq!(
            restored_parsed.brightness, orig_kbd_parsed.brightness,
            "restored brightness mismatch"
        );
        println!("  restored: brightness={}", restored_parsed.brightness);
    } else {
        section("Keyboard Backlight");
        println!("  skipped: keyboard backlight capability not available");
    }

    // ── 8. Power settings (read + save/restore) ────────────────
    section("Power Settings");

    let orig_power = client
        .get_power_settings()
        .await
        .expect("get_power_settings failed");
    println!("  original: {}", orig_power.trim().replace('\n', " | "));

    // Verify we see governor and EPP fields.
    assert!(
        orig_power.contains("governor") || orig_power.contains("epp"),
        "power settings should contain governor/epp for IBP Gen8"
    );

    // Toggle governor to performance and back.
    if orig_power.contains("powersave") {
        let test_power = orig_power.replace("powersave", "performance");
        client
            .set_power_settings(&test_power)
            .await
            .expect("set_power_settings failed");

        let rb = client.get_power_settings().await.unwrap();
        assert!(
            rb.contains("performance"),
            "governor didn't switch to performance"
        );
        println!("  set governor=performance: OK");

        // Restore.
        client
            .set_power_settings(&orig_power)
            .await
            .expect("restore power settings failed");
        println!("  restored governor=powersave");
    } else {
        println!("  skipping governor toggle (not powersave currently)");
    }

    // ── 9. Charging — branch on platform type ────────────────────
    if caps.charging_profiles {
        // Uniwill: uses named profiles + priority, not numeric thresholds.
        section("Charging — Uniwill Profiles");

        match client.get_charging_settings().await {
            Ok(orig_charging) => {
                let charging: ChargingSettings =
                    toml::from_str(&orig_charging).expect("bad ChargingSettings TOML");
                println!(
                    "  original: profile={:?} priority={:?}",
                    charging.profile, charging.priority
                );

                // Test 1: Cycle through all valid profiles.
                for test_profile in &["high_capacity", "balanced", "stationary"] {
                    let test_settings = ChargingSettings {
                        profile: Some(test_profile.to_string()),
                        priority: charging.priority.clone(),
                        ..Default::default()
                    };
                    let test_toml = toml::to_string(&test_settings).unwrap();
                    client
                        .set_charging_settings(&test_toml)
                        .await
                        .unwrap_or_else(|e| panic!("set profile={test_profile} failed: {e}"));

                    let rb_toml = get_charging_settings_retry(&client).await.unwrap();
                    let rb: ChargingSettings = toml::from_str(&rb_toml).unwrap();
                    assert_eq!(
                        rb.profile.as_deref(),
                        Some(*test_profile),
                        "profile readback mismatch for {test_profile}"
                    );
                    println!("  set profile={test_profile}: OK");
                }

                // Test 2: Cycle through valid priorities.
                for test_priority in &["charge_battery", "performance"] {
                    let test_settings = ChargingSettings {
                        profile: charging.profile.clone(),
                        priority: Some(test_priority.to_string()),
                        ..Default::default()
                    };
                    let test_toml = toml::to_string(&test_settings).unwrap();
                    client
                        .set_charging_settings(&test_toml)
                        .await
                        .unwrap_or_else(|e| panic!("set priority={test_priority} failed: {e}"));

                    let rb_toml = get_charging_settings_retry(&client).await.unwrap();
                    let rb: ChargingSettings = toml::from_str(&rb_toml).unwrap();
                    assert_eq!(
                        rb.priority.as_deref(),
                        Some(*test_priority),
                        "priority readback mismatch for {test_priority}"
                    );
                    println!("  set priority={test_priority}: OK");
                }

                // Test 3: Invalid profile name is rejected.
                let invalid = ChargingSettings {
                    profile: Some("invalid_profile_name".into()),
                    ..Default::default()
                };
                let invalid_toml = toml::to_string(&invalid).unwrap();
                let err = client.set_charging_settings(&invalid_toml).await;
                assert!(err.is_err(), "invalid profile name should be rejected");
                println!("  invalid profile 'invalid_profile_name' rejected: OK");

                // Restore original.
                client
                    .set_charging_settings(&orig_charging)
                    .await
                    .expect("restore charging failed");
                let restored_toml = get_charging_settings_retry(&client).await.unwrap();
                let restored: ChargingSettings = toml::from_str(&restored_toml).unwrap();
                assert_eq!(
                    restored.profile, charging.profile,
                    "restored profile mismatch"
                );
                assert_eq!(
                    restored.priority, charging.priority,
                    "restored priority mismatch"
                );
                println!(
                    "  restored original: profile={:?} priority={:?}",
                    restored.profile, restored.priority
                );
            }
            Err(e) => {
                panic!("charging backend advertised but unavailable: {e}");
            }
        }
    } else if caps.charging_thresholds {
        // Clevo: uses numeric start/end thresholds.
        section("Charging — Clevo Thresholds");

        match get_charging_settings_retry(&client).await {
            Ok(orig_charging) => {
                let charging: ChargingSettings =
                    toml::from_str(&orig_charging).expect("bad ChargingSettings TOML");
                println!(
                    "  original: start={:?} end={:?}",
                    charging.start_threshold, charging.end_threshold
                );

                let end = charging.end_threshold.unwrap_or(0);

                if end > 10 {
                    // Test 1: Lower thresholds by 5%.
                    let start = charging.start_threshold.unwrap_or(0);
                    let test_start = start.saturating_sub(5).max(1);
                    let test_end = end.saturating_sub(5).max(test_start + 1);
                    let test_settings = ChargingSettings {
                        start_threshold: Some(test_start),
                        end_threshold: Some(test_end),
                        ..Default::default()
                    };
                    let test_toml = toml::to_string(&test_settings).unwrap();
                    client
                        .set_charging_settings(&test_toml)
                        .await
                        .expect("set_charging_settings failed");

                    let rb_toml = get_charging_settings_retry(&client).await.unwrap();
                    let rb: ChargingSettings = toml::from_str(&rb_toml).unwrap();
                    assert_eq!(
                        rb.start_threshold,
                        Some(test_start),
                        "start threshold mismatch"
                    );
                    assert_eq!(rb.end_threshold, Some(test_end), "end threshold mismatch");
                    println!("  set start={test_start}% end={test_end}%: OK");

                    // Test 2: Set to 100% (full charge).
                    let full_charge = ChargingSettings {
                        start_threshold: Some(95),
                        end_threshold: Some(100),
                        ..Default::default()
                    };
                    let full_toml = toml::to_string(&full_charge).unwrap();
                    client
                        .set_charging_settings(&full_toml)
                        .await
                        .expect("set_charging_settings(100%) failed");

                    let rb2_toml = get_charging_settings_retry(&client).await.unwrap();
                    let rb2: ChargingSettings = toml::from_str(&rb2_toml).unwrap();
                    assert_eq!(rb2.end_threshold, Some(100));
                    println!("  set end=100% (full charge): OK");

                    // Test 3: Verify invalid thresholds are rejected (start >= end).
                    let invalid = ChargingSettings {
                        start_threshold: Some(80),
                        end_threshold: Some(50),
                        ..Default::default()
                    };
                    let invalid_toml = toml::to_string(&invalid).unwrap();
                    let err = client.set_charging_settings(&invalid_toml).await;
                    assert!(err.is_err(), "start >= end should be rejected");
                    println!("  invalid thresholds (start=80, end=50) rejected: OK");

                    // Restore original.
                    client
                        .set_charging_settings(&orig_charging)
                        .await
                        .expect("restore charging failed");
                    let restored_toml = get_charging_settings_retry(&client).await.unwrap();
                    let restored: ChargingSettings = toml::from_str(&restored_toml).unwrap();
                    assert_eq!(
                        restored.start_threshold, charging.start_threshold,
                        "restored start mismatch"
                    );
                    assert_eq!(
                        restored.end_threshold, charging.end_threshold,
                        "restored end mismatch"
                    );
                    println!("  restored original thresholds");
                } else {
                    println!(
                        "  thresholds are 0 — hardware may not support (skipping write tests)"
                    );
                }
            }
            Err(e) => {
                panic!("charging backend advertised but unavailable: {e}");
            }
        }
    }

    // ── 10. GPU info (graceful — no dedicated GPU control) ─────
    section("GPU Info");

    match client.get_gpu_info().await {
        Ok(gpu) => println!("  {}", gpu.trim().replace('\n', " | ")),
        Err(e) => println!("  not available: {e} (expected — IBP Gen8 has no GPU control)"),
    }

    // ── 11. Display brightness (save/restore) ─────────────────
    section("Display Brightness");

    match client.get_display_settings().await {
        Ok(orig_display_toml) => {
            let orig_display: DisplayState =
                toml::from_str(&orig_display_toml).expect("bad DisplayState TOML");
            assert!(
                orig_display.max_brightness > 0,
                "max_brightness should be > 0"
            );
            assert!(
                !orig_display.driver.is_empty(),
                "driver name should be non-empty"
            );
            println!(
                "  original: brightness={}% max_brightness={} driver={}",
                orig_display.brightness, orig_display.max_brightness, orig_display.driver
            );

            // Test: set brightness to 50%.
            let test_brightness = 50u32;
            let set_toml = format!("brightness = {test_brightness}");
            client
                .set_display_settings(&set_toml)
                .await
                .expect("set_display_settings(50) failed");

            let rb_toml = client.get_display_settings().await.unwrap();
            let rb: DisplayState = toml::from_str(&rb_toml).unwrap();
            // Allow ±2% tolerance for rounding in the brightness→raw→percent chain.
            assert!(
                rb.brightness.abs_diff(test_brightness) <= 2,
                "brightness should be ~{test_brightness}%, got {}%",
                rb.brightness
            );
            println!("  set brightness=50%: readback={}% — OK", rb.brightness);

            // Test: set brightness to 100% (max).
            let set_max = "brightness = 100";
            client
                .set_display_settings(set_max)
                .await
                .expect("set_display_settings(100) failed");
            let rb2_toml = client.get_display_settings().await.unwrap();
            let rb2: DisplayState = toml::from_str(&rb2_toml).unwrap();
            assert!(
                rb2.brightness >= 98,
                "brightness should be ~100%, got {}%",
                rb2.brightness
            );
            println!("  set brightness=100%: readback={}% — OK", rb2.brightness);

            // Test: set brightness to 10% (low).
            let set_low = "brightness = 10";
            client
                .set_display_settings(set_low)
                .await
                .expect("set_display_settings(10) failed");
            let rb3_toml = client.get_display_settings().await.unwrap();
            let rb3: DisplayState = toml::from_str(&rb3_toml).unwrap();
            assert!(
                rb3.brightness.abs_diff(10) <= 2,
                "brightness should be ~10%, got {}%",
                rb3.brightness
            );
            println!("  set brightness=10%: readback={}% — OK", rb3.brightness);

            // Restore original brightness.
            let restore_toml = format!("brightness = {}", orig_display.brightness);
            client
                .set_display_settings(&restore_toml)
                .await
                .expect("restore display brightness failed");
            let restored_toml = client.get_display_settings().await.unwrap();
            let restored: DisplayState = toml::from_str(&restored_toml).unwrap();
            assert!(
                restored.brightness.abs_diff(orig_display.brightness) <= 2,
                "restored brightness mismatch: got {}%, wanted {}%",
                restored.brightness,
                orig_display.brightness
            );
            println!("  restored brightness={}%: OK", restored.brightness);
        }
        Err(e) => {
            println!("  display brightness not available: {e} (non-fatal)");
        }
    }

    // ── Done ───────────────────────────────────────────────────
    section("PASSED — IBP Gen8 Live Regression");
    println!("  All live regression checks passed!");
}
