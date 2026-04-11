//! CLI mode for scripted/E2E testing.
//!
//! Provides a headless interface to read daemon state and verify
//! TUI ↔ D-Bus communication without rendering a terminal.

use tux_core::dbus_types::{
    CapabilitiesResponse, DashboardSnapshot, ProfileList, SystemInfoResponse,
};
use tux_core::fan_curve::FanConfig;
use tux_tui::dbus_client::DaemonClient;

/// CLI subcommands for headless operation.
#[derive(Debug, Clone)]
pub enum CliCommand {
    /// Dump dashboard telemetry as JSON.
    Dashboard,
    /// Dump the active fan curve as JSON.
    FanCurve,
    /// Dump all profiles as JSON.
    Profiles,
    /// Dump capabilities as JSON.
    Capabilities,
    /// Dump system info as JSON.
    SystemInfo,
    /// Dump the entire model state as JSON.
    Json,
}

/// Run a CLI command against the daemon, returning JSON output.
pub async fn run_cli(command: CliCommand, session_bus: bool) -> Result<String, String> {
    let client = DaemonClient::connect(session_bus)
        .await
        .map_err(|e| format!("failed to connect to daemon: {e}"))?;

    match command {
        CliCommand::Dashboard => dump_dashboard(&client).await,
        CliCommand::FanCurve => dump_fan_curve(&client).await,
        CliCommand::Profiles => dump_profiles(&client).await,
        CliCommand::Capabilities => dump_capabilities(&client).await,
        CliCommand::SystemInfo => dump_system_info(&client).await,
        CliCommand::Json => dump_all(&client).await,
    }
}

async fn dump_dashboard(client: &DaemonClient) -> Result<String, String> {
    let temp = client
        .get_temperature(0)
        .await
        .map(|t| t as f32 / 1000.0)
        .ok();

    let fan_info = client.get_fan_info().await.map_err(|e| e.to_string())?;
    let num_fans = fan_info.3;

    let mut fan_speeds = Vec::new();
    for i in 0..num_fans as u32 {
        if let Ok(rpm) = client.get_fan_speed(i).await {
            fan_speeds.push(rpm);
        }
    }

    let power_state = client
        .get_power_state()
        .await
        .unwrap_or_else(|_| "unknown".to_string());

    let snapshot = DashboardSnapshot {
        cpu_temp: temp,
        fan_speeds,
        power_state,
    };

    serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())
}

async fn dump_fan_curve(client: &DaemonClient) -> Result<String, String> {
    let toml_str = client
        .get_active_fan_curve()
        .await
        .map_err(|e| e.to_string())?;
    let config: FanConfig = toml::from_str(&toml_str).map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&config).map_err(|e| e.to_string())
}

async fn dump_profiles(client: &DaemonClient) -> Result<String, String> {
    let toml_str = client.list_profiles().await.map_err(|e| e.to_string())?;
    let list: ProfileList = toml::from_str(&toml_str).map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&list.profiles).map_err(|e| e.to_string())
}

async fn dump_capabilities(client: &DaemonClient) -> Result<String, String> {
    let toml_str = client.get_capabilities().await.map_err(|e| e.to_string())?;
    let caps: CapabilitiesResponse = toml::from_str(&toml_str).map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&caps).map_err(|e| e.to_string())
}

async fn dump_system_info(client: &DaemonClient) -> Result<String, String> {
    let toml_str = client.get_system_info().await.map_err(|e| e.to_string())?;
    let info: SystemInfoResponse = toml::from_str(&toml_str).map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&info).map_err(|e| e.to_string())
}

async fn dump_all(client: &DaemonClient) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct FullDump {
        dashboard: DashboardSnapshot,
        fan_curve: FanConfig,
        profiles: Vec<tux_core::profile::TuxProfile>,
        capabilities: CapabilitiesResponse,
        system_info: SystemInfoResponse,
    }

    let dashboard_str = dump_dashboard(client).await?;
    let dashboard: DashboardSnapshot =
        serde_json::from_str(&dashboard_str).map_err(|e| e.to_string())?;

    let fan_curve_str = dump_fan_curve(client).await?;
    let fan_curve: FanConfig = serde_json::from_str(&fan_curve_str).map_err(|e| e.to_string())?;

    let profiles_str = dump_profiles(client).await?;
    let profiles: Vec<tux_core::profile::TuxProfile> =
        serde_json::from_str(&profiles_str).map_err(|e| e.to_string())?;

    let capabilities_str = dump_capabilities(client).await?;
    let capabilities: CapabilitiesResponse =
        serde_json::from_str(&capabilities_str).map_err(|e| e.to_string())?;

    let system_info_str = dump_system_info(client).await?;
    let system_info: SystemInfoResponse =
        serde_json::from_str(&system_info_str).map_err(|e| e.to_string())?;

    let full = FullDump {
        dashboard,
        fan_curve,
        profiles,
        capabilities,
        system_info,
    };

    serde_json::to_string_pretty(&full).map_err(|e| e.to_string())
}

/// Parse CLI command from args. Returns None if no CLI command found.
pub fn parse_cli_command(args: &[String]) -> Option<CliCommand> {
    for arg in args {
        match arg.as_str() {
            "--dump-dashboard" => return Some(CliCommand::Dashboard),
            "--dump-fan-curve" => return Some(CliCommand::FanCurve),
            "--dump-profiles" => return Some(CliCommand::Profiles),
            "--dump-Capabilities" | "--dump-capabilities" => return Some(CliCommand::Capabilities),
            "--dump-system-info" => return Some(CliCommand::SystemInfo),
            "--json" => return Some(CliCommand::Json),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dump_dashboard() {
        let args = vec!["tux-tui".to_string(), "--dump-dashboard".to_string()];
        let cmd = parse_cli_command(&args);
        assert!(matches!(cmd, Some(CliCommand::Dashboard)));
    }

    #[test]
    fn parse_dump_profiles() {
        let args = vec![
            "tux-tui".to_string(),
            "--session".to_string(),
            "--dump-profiles".to_string(),
        ];
        let cmd = parse_cli_command(&args);
        assert!(matches!(cmd, Some(CliCommand::Profiles)));
    }

    #[test]
    fn parse_dump_fan_curve() {
        let args = vec!["tux-tui".to_string(), "--dump-fan-curve".to_string()];
        let cmd = parse_cli_command(&args);
        assert!(matches!(cmd, Some(CliCommand::FanCurve)));
    }

    #[test]
    fn parse_dump_capabilities() {
        let args = vec!["tux-tui".to_string(), "--dump-capabilities".to_string()];
        let cmd = parse_cli_command(&args);
        assert!(matches!(cmd, Some(CliCommand::Capabilities)));
    }

    #[test]
    fn parse_dump_system_info() {
        let args = vec!["tux-tui".to_string(), "--dump-system-info".to_string()];
        let cmd = parse_cli_command(&args);
        assert!(matches!(cmd, Some(CliCommand::SystemInfo)));
    }

    #[test]
    fn parse_dump_json() {
        let args = vec!["tux-tui".to_string(), "--json".to_string()];
        let cmd = parse_cli_command(&args);
        assert!(matches!(cmd, Some(CliCommand::Json)));
    }

    #[test]
    fn parse_no_cli_command() {
        let args = vec!["tux-tui".to_string(), "--session".to_string()];
        let cmd = parse_cli_command(&args);
        assert!(cmd.is_none());
    }
}
