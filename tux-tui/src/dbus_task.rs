//! D-Bus polling and command loop for the TUI.
//!
//! This module contains the TUI-specific bridge code that connects
//! [`DaemonClient`] to the TUI event loop via channels.

use std::time::Duration;

use tokio::sync::mpsc;
use zbus::zvariant::OwnedValue;

use crate::event::{AppEvent, DbusUpdate};
use crate::model::ConnectionStatus;
use tux_tui::dbus_client::DaemonClient;

/// Spawn a task that connects to the daemon, polls dashboard data, and sends updates.
/// Reconnects with exponential backoff if the connection fails or is lost.
pub async fn run_dbus_task(
    session_bus: bool,
    tx: mpsc::Sender<AppEvent>,
    mut cmd_rx: mpsc::Receiver<crate::DbusCommand>,
) {
    const INITIAL_BACKOFF: Duration = Duration::from_secs(1);
    const MAX_BACKOFF: Duration = Duration::from_secs(30);
    let mut backoff = INITIAL_BACKOFF;

    loop {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::ConnectionStatus(
                ConnectionStatus::Connecting,
            )))
            .await;

        let client = match DaemonClient::connect(session_bus).await {
            Ok(c) => {
                backoff = INITIAL_BACKOFF; // Reset on successful connect.
                let _ = tx
                    .send(AppEvent::DbusData(DbusUpdate::ConnectionStatus(
                        ConnectionStatus::Connected,
                    )))
                    .await;
                c
            }
            Err(_) => {
                let _ = tx
                    .send(AppEvent::DbusData(DbusUpdate::ConnectionStatus(
                        ConnectionStatus::Disconnected,
                    )))
                    .await;
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
                continue;
            }
        };

        // Fetch one-time data: system info, capabilities, device info, fan curve.
        let num_fans = fetch_info_data(&client, &tx).await;

        // Track consecutive poll failures to detect connection loss.
        let mut consecutive_failures: u32 = 0;
        const MAX_CONSECUTIVE_FAILURES: u32 = 3;
        // Last active profile seen from telemetry. Used to refresh fan curve when
        // profile changes due to explicit assignment or power auto-switch.
        let mut last_active_profile: Option<String> = None;

        // Poll + command loop.
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if tx.is_closed() {
                        return; // TUI shut down.
                    }
                    if poll_dashboard_checked(&client, &tx, num_fans, &mut last_active_profile).await {
                        consecutive_failures = 0;
                    } else {
                        consecutive_failures += 1;
                        if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                            break;
                        }
                    }
                }
                cmd = cmd_rx.recv() => {
                    if !handle_command(&client, &tx, cmd).await {
                        return; // Command channel closed — TUI shut down.
                    }
                }
            }
        }

        // Connection lost — notify and back off before retry.
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::ConnectionStatus(
                ConnectionStatus::Disconnected,
            )))
            .await;
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

/// Poll dashboard and return `true` if any call succeeded (connection alive).
async fn poll_dashboard_checked(
    client: &DaemonClient,
    tx: &mpsc::Sender<AppEvent>,
    num_fans: u8,
    last_active_profile: &mut Option<String>,
) -> bool {
    // Temperature (millidegrees → degrees).
    let temp_result = client.get_temperature(0).await;
    let temp = temp_result.as_ref().map(|t| *t as f32 / 1000.0).ok();
    let any_ok = temp_result.is_ok();

    // Fan telemetry — poll all known fans via GetFanData.
    let mut fan_speeds = Vec::new();
    let mut fan_duties = Vec::new();
    let mut fan_rpm_available = Vec::new();
    for i in 0..num_fans as u32 {
        if let Ok(toml_str) = client.get_fan_data(i).await
            && let Ok(data) = toml::from_str::<tux_core::dbus_types::FanData>(&toml_str)
        {
            fan_speeds.push(data.rpm);
            fan_duties.push(data.duty_percent);
            fan_rpm_available.push(data.rpm_available);
        } else if let Ok(rpm) = client.get_fan_speed(i).await {
            // Fallback for older daemons without GetFanData.
            fan_speeds.push(rpm);
            fan_duties.push(0);
            fan_rpm_available.push(rpm > 0);
        }
    }

    // Power state.
    let power = client.get_power_state().await.ok();

    // CPU frequency.
    let cpu_freq = client.get_cpu_frequency().await.ok();

    // Active profile name.
    let profile = client.get_active_profile_name().await.ok();

    // CPU load (overall + per-core).
    let (cpu_load_overall, cpu_load_per_core) = if let Ok(toml_str) = client.get_cpu_load().await
        && let Ok(resp) = toml::from_str::<tux_core::dbus_types::CpuLoadResponse>(&toml_str)
    {
        (Some(resp.overall), Some(resp.per_core))
    } else {
        (None, None)
    };

    // Per-core CPU frequencies.
    let cpu_freq_per_core = if let Ok(toml_str) = client.get_per_core_frequencies().await
        && let Ok(resp) = toml::from_str::<tux_core::dbus_types::CpuFreqResponse>(&toml_str)
    {
        Some(resp.per_core)
    } else {
        None
    };

    // Package power draw (RAPL).
    let power_draw_w = client
        .get_package_power_w()
        .await
        .ok()
        .filter(|&w| w > 0.0)
        .map(|w| w as f32);

    let _ = tx
        .send(AppEvent::DbusData(DbusUpdate::DashboardTelemetry {
            cpu_temp: temp,
            fan_speeds,
            fan_duties,
            fan_rpm_available,
            power_state: power,
            cpu_freq_mhz: cpu_freq,
            active_profile: profile.clone(),
            cpu_load_overall,
            cpu_load_per_core,
            cpu_freq_per_core,
            power_draw_w,
        }))
        .await;

    // Keep fan curve editor in sync with the actually active profile.
    // This handles both manual profile assignment and daemon auto-switch
    // when power state changes.
    if should_refresh_fan_curve(last_active_profile, profile.as_deref())
        && let Ok(toml_str) = client.get_active_fan_curve().await
        && let Ok(config) = toml::from_str::<tux_core::fan_curve::FanConfig>(&toml_str)
    {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::FanCurve(config.curve)))
            .await;
    }

    // Poll fan health separately — non-fatal if the method is unavailable.
    if let Ok(toml_str) = client.get_fan_health().await {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::FanHealth(toml_str)))
            .await;
    }

    // Keep Info tab battery telemetry fresh instead of only loading it once at startup.
    if let Ok(toml_str) = client.get_battery_info().await {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::BatteryInfo(toml_str)))
            .await;
    }

    any_ok
}

/// Returns true when the active profile changed and fan curve should be refreshed.
/// Updates `last_active_profile` to the new value when changed.
fn should_refresh_fan_curve(
    last_active_profile: &mut Option<String>,
    current_profile: Option<&str>,
) -> bool {
    let Some(current_profile) = current_profile else {
        return false;
    };
    if last_active_profile.as_deref() == Some(current_profile) {
        return false;
    }
    *last_active_profile = Some(current_profile.to_string());
    true
}

/// Handle a single D-Bus command. Returns `false` when the channel is closed.
async fn handle_command(
    client: &DaemonClient,
    tx: &mpsc::Sender<AppEvent>,
    cmd: Option<crate::DbusCommand>,
) -> bool {
    match cmd {
        Some(crate::DbusCommand::SaveFanCurve(points)) => {
            execute_save_fan_curve(client, points, tx).await;
        }
        Some(crate::DbusCommand::FetchFanCurve) => {
            if let Ok(toml_str) = client.get_active_fan_curve().await
                && let Ok(config) = toml::from_str::<tux_core::fan_curve::FanConfig>(&toml_str)
            {
                let _ = tx
                    .send(AppEvent::DbusData(DbusUpdate::FanCurve(config.curve)))
                    .await;
            }
        }
        Some(crate::DbusCommand::FetchProfiles) => {
            fetch_profiles(client, tx).await;
        }
        Some(crate::DbusCommand::CopyProfile(id)) => match client.copy_profile(&id).await {
            Ok(new_id) => {
                let _ = tx
                    .send(AppEvent::DbusData(DbusUpdate::ProfileOperationDone(
                        format!("Copied → {new_id}"),
                    )))
                    .await;
                fetch_profiles(client, tx).await;
            }
            Err(e) => {
                let _ = tx
                    .send(AppEvent::DbusData(DbusUpdate::ProfileOperationError(
                        e.to_string(),
                    )))
                    .await;
            }
        },
        Some(crate::DbusCommand::CreateProfile(toml)) => match client.create_profile(&toml).await {
            Ok(new_id) => {
                let _ = tx
                    .send(AppEvent::DbusData(DbusUpdate::ProfileOperationDone(
                        format!("Created → {new_id}"),
                    )))
                    .await;
                fetch_profiles(client, tx).await;
            }
            Err(e) => {
                let _ = tx
                    .send(AppEvent::DbusData(DbusUpdate::ProfileOperationError(
                        e.to_string(),
                    )))
                    .await;
            }
        },
        Some(crate::DbusCommand::DeleteProfile(id)) => match client.delete_profile(&id).await {
            Ok(()) => {
                let _ = tx
                    .send(AppEvent::DbusData(DbusUpdate::ProfileOperationDone(
                        "Profile deleted".into(),
                    )))
                    .await;
                fetch_profiles(client, tx).await;
            }
            Err(e) => {
                let _ = tx
                    .send(AppEvent::DbusData(DbusUpdate::ProfileOperationError(
                        e.to_string(),
                    )))
                    .await;
            }
        },
        Some(crate::DbusCommand::SaveProfile { id, toml }) => {
            match client.update_profile(&id, &toml).await {
                Ok(()) => {
                    let _ = tx
                        .send(AppEvent::DbusData(DbusUpdate::ProfileOperationDone(
                            "Profile saved".into(),
                        )))
                        .await;
                    fetch_profiles(client, tx).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppEvent::DbusData(DbusUpdate::ProfileOperationError(
                            e.to_string(),
                        )))
                        .await;
                }
            }
        }
        Some(crate::DbusCommand::SetActiveProfile { id, state }) => {
            match client.set_active_profile(&id, &state).await {
                Ok(()) => {
                    let _ = tx
                        .send(AppEvent::DbusData(DbusUpdate::ProfileOperationDone(
                            format!("Set {state} profile → {id}"),
                        )))
                        .await;
                    fetch_profile_assignments(client, tx).await;
                    // Re-fetch the active fan curve so the editor reflects the new profile.
                    if let Ok(toml_str) = client.get_active_fan_curve().await
                        && let Ok(config) =
                            toml::from_str::<tux_core::fan_curve::FanConfig>(&toml_str)
                    {
                        let _ = tx
                            .send(AppEvent::DbusData(DbusUpdate::FanCurve(config.curve)))
                            .await;
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(AppEvent::DbusData(DbusUpdate::ProfileOperationError(
                            e.to_string(),
                        )))
                        .await;
                }
            }
        }
        Some(crate::DbusCommand::SaveSettings(toml)) => {
            execute_form_save(tx, "settings", || client.set_global_settings(&toml)).await;
        }
        Some(crate::DbusCommand::SaveKeyboard(toml)) => {
            execute_form_save(tx, "keyboard", || client.set_keyboard_state(&toml)).await;
        }
        Some(crate::DbusCommand::SaveCharging(toml)) => {
            execute_form_save(tx, "charging", || client.set_charging_settings(&toml)).await;
        }
        Some(crate::DbusCommand::SavePower(toml)) => {
            execute_form_save(tx, "power", || client.set_power_settings(&toml)).await;
        }
        Some(crate::DbusCommand::SaveDisplay(toml)) => {
            execute_form_save(tx, "display", || client.set_display_settings(&toml)).await;
        }
        Some(crate::DbusCommand::SaveWebcam { device, toml }) => {
            execute_form_save(tx, "webcam", || client.set_webcam_controls(&device, &toml)).await;
        }
        None => return false, // Command channel closed.
    }
    true
}

/// Fetch one-time info data for the Info tab. Returns detected num_fans.
async fn fetch_info_data(client: &DaemonClient, tx: &mpsc::Sender<AppEvent>) -> u8 {
    // Device info.
    if let Ok(name) = client.get_device_property("DeviceName").await
        && let Ok(s) = <String as TryFrom<OwnedValue>>::try_from(name)
    {
        let _ = tx.send(AppEvent::DbusData(DbusUpdate::DeviceName(s))).await;
    }
    if let Ok(platform) = client.get_device_property("Platform").await
        && let Ok(s) = <String as TryFrom<OwnedValue>>::try_from(platform)
    {
        let _ = tx.send(AppEvent::DbusData(DbusUpdate::Platform(s))).await;
    }
    if let Ok(version) = client.get_device_property("DaemonVersion").await
        && let Ok(s) = <String as TryFrom<OwnedValue>>::try_from(version)
    {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::DaemonVersion(s)))
            .await;
    }

    // System info.
    if let Ok(info) = client.get_system_info().await {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::SystemInfo(info)))
            .await;
    }

    // Capabilities.
    if let Ok(caps) = client.get_capabilities().await {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::Capabilities(caps)))
            .await;
    }

    // Fan info.
    let mut num_fans: u8 = 2; // Default fallback.
    if let Ok(info) = client.get_fan_info().await {
        num_fans = info.3;
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::FanInfo {
                num_fans: info.3,
                max_rpm: info.0,
            }))
            .await;
    }

    // CPU core count (one-time).
    if let Ok(count) = client.get_cpu_count().await {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::CpuCoreCount(count)))
            .await;
    }

    // Hardware CPU limits (core count, hw min/max freq) — one-time.
    if let Ok(toml_str) = client.get_cpu_hw_limits().await
        && let Ok(limits) = toml::from_str::<tux_core::dbus_types::CpuHwLimits>(&toml_str)
    {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::CpuHwLimits(limits)))
            .await;
    }

    // TDP bounds (one-time) — empty string means TDP unavailable.
    if let Ok(toml_str) = client.get_tdp_bounds().await
        && !toml_str.is_empty()
        && let Ok(bounds) = toml::from_str::<tux_core::device::TdpBounds>(&toml_str)
    {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::TdpBounds(bounds)))
            .await;
    }

    // Fan curve.
    if let Ok(toml_str) = client.get_active_fan_curve().await
        && let Ok(config) = toml::from_str::<tux_core::fan_curve::FanConfig>(&toml_str)
    {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::FanCurve(config.curve)))
            .await;
    }

    // Profiles + assignments.
    fetch_profiles(client, tx).await;

    // Form-backed tab data (best-effort; these D-Bus methods may not exist yet).
    if let Ok(s) = client.get_global_settings().await {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::SettingsData(s)))
            .await;
    }
    if let Ok(s) = client.get_keyboard_state().await {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::KeyboardData(s)))
            .await;
    }
    if let Ok(s) = client.get_charging_settings().await {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::ChargingData(s)))
            .await;
    }
    if let Ok(s) = client.get_gpu_info().await {
        let _ = tx.send(AppEvent::DbusData(DbusUpdate::GpuInfo(s))).await;
    }
    if let Ok(s) = client.get_power_settings().await {
        let _ = tx.send(AppEvent::DbusData(DbusUpdate::PowerData(s))).await;
    }
    if let Ok(s) = client.get_display_settings().await {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::DisplayData(s)))
            .await;
    }

    // Battery info.
    if let Ok(s) = client.get_battery_info().await {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::BatteryInfo(s)))
            .await;
    }

    num_fans
}

/// Execute a SaveFanCurve command: serialize the curve to TOML and send to daemon.
pub async fn execute_save_fan_curve(
    client: &DaemonClient,
    points: Vec<tux_core::fan_curve::FanCurvePoint>,
    tx: &mpsc::Sender<AppEvent>,
) {
    let mut config: tux_core::fan_curve::FanConfig = client
        .get_active_fan_curve()
        .await
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();
    config.curve = points;
    if let Ok(toml_str) = toml::to_string_pretty(&config)
        && client.set_fan_curve(&toml_str).await.is_ok()
    {
        let _ = tx.send(AppEvent::DbusData(DbusUpdate::FanCurveSaved)).await;
    }
}

/// Helper struct for deserializing profile assignments TOML.
#[derive(serde::Deserialize)]
struct ProfileAssignmentsToml {
    ac_profile: String,
    battery_profile: String,
}

/// Helper struct for deserializing the profile list wrapper.
#[derive(serde::Deserialize)]
struct ProfileListToml {
    profiles: Vec<tux_core::profile::TuxProfile>,
}

/// Fetch profiles and assignments from daemon, sending updates.
async fn fetch_profiles(client: &DaemonClient, tx: &mpsc::Sender<AppEvent>) {
    // Profiles.
    if let Ok(toml_str) = client.list_profiles().await
        && let Ok(list) = toml::from_str::<ProfileListToml>(&toml_str)
    {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::ProfileList(list.profiles)))
            .await;
    }

    // Assignments.
    fetch_profile_assignments(client, tx).await;
}

/// Fetch profile assignments from daemon.
async fn fetch_profile_assignments(client: &DaemonClient, tx: &mpsc::Sender<AppEvent>) {
    if let Ok(toml_str) = client.get_profile_assignments().await
        && let Ok(a) = toml::from_str::<ProfileAssignmentsToml>(&toml_str)
    {
        let _ = tx
            .send(AppEvent::DbusData(DbusUpdate::ProfileAssignments {
                ac_profile: a.ac_profile,
                battery_profile: a.battery_profile,
            }))
            .await;
    }
}

/// Execute a form save operation, sending FormSaved or FormSaveError.
async fn execute_form_save<F, Fut>(tx: &mpsc::Sender<AppEvent>, tab_name: &str, op: F)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<(), zbus::Error>>,
{
    match op().await {
        Ok(()) => {
            let _ = tx
                .send(AppEvent::DbusData(DbusUpdate::FormSaved(
                    tab_name.to_string(),
                )))
                .await;
        }
        Err(e) => {
            let _ = tx
                .send(AppEvent::DbusData(DbusUpdate::FormSaveError(format!(
                    "{tab_name}: {e}"
                ))))
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::should_refresh_fan_curve;

    #[test]
    fn first_seen_profile_triggers_refresh() {
        let mut last = None;
        assert!(should_refresh_fan_curve(&mut last, Some("quiet")));
        assert_eq!(last.as_deref(), Some("quiet"));
    }

    #[test]
    fn unchanged_profile_does_not_refresh() {
        let mut last = Some("quiet".to_string());
        assert!(!should_refresh_fan_curve(&mut last, Some("quiet")));
        assert_eq!(last.as_deref(), Some("quiet"));
    }

    #[test]
    fn changed_profile_triggers_refresh() {
        let mut last = Some("quiet".to_string());
        assert!(should_refresh_fan_curve(&mut last, Some("new")));
        assert_eq!(last.as_deref(), Some("new"));
    }

    #[test]
    fn missing_current_profile_does_not_refresh_or_reset_last() {
        let mut last = Some("quiet".to_string());
        assert!(!should_refresh_fan_curve(&mut last, None));
        assert_eq!(last.as_deref(), Some("quiet"));
    }
}
