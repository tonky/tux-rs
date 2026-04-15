//! Event types flowing through the TUI event loop.

use crossterm::event::KeyEvent;
use tux_core::fan_curve::FanCurvePoint;
use tux_core::profile::TuxProfile;

/// Application-level events.
pub enum AppEvent {
    /// A keyboard event from the terminal.
    Key(KeyEvent),
    /// Terminal resized.
    Resize(u16, u16),
    /// Data received from the daemon via D-Bus.
    DbusData(DbusUpdate),
    /// Periodic tick for polling.
    Tick,
}

/// Updates received from the D-Bus daemon.
#[derive(Debug)]
#[allow(dead_code)]
pub enum DbusUpdate {
    /// Connection status changed.
    ConnectionStatus(crate::model::ConnectionStatus),
    /// Live dashboard telemetry.
    DashboardTelemetry {
        cpu_temp: Option<f32>,
        fan_speeds: Vec<u32>,
        fan_duties: Vec<u8>,
        fan_rpm_available: Vec<bool>,
        power_state: Option<String>,
        cpu_freq_mhz: Option<u32>,
        active_profile: Option<String>,
        cpu_load_overall: Option<f32>,
        cpu_load_per_core: Option<Vec<f32>>,
        cpu_freq_per_core: Option<Vec<u32>>,
    },
    /// Fan engine health status (TOML-encoded `FanHealthResponse`).
    FanHealth(String),
    /// Fan hardware info (one-time).
    FanInfo { num_fans: u8, max_rpm: u32 },
    /// CPU core count (one-time).
    CpuCoreCount(u32),
    /// Device name (one-time).
    DeviceName(String),
    /// Platform name (one-time).
    Platform(String),
    /// Daemon version (one-time).
    DaemonVersion(String),
    /// System info TOML (one-time).
    SystemInfo(String),
    /// Battery info TOML.
    BatteryInfo(String),
    /// Capabilities TOML (one-time).
    Capabilities(String),
    /// Active fan curve loaded from daemon.
    FanCurve(Vec<FanCurvePoint>),
    /// Fan curve saved successfully.
    FanCurveSaved,
    /// List of profiles loaded from daemon.
    ProfileList(Vec<TuxProfile>),
    /// Profile assignments loaded.
    ProfileAssignments {
        ac_profile: String,
        battery_profile: String,
    },
    /// A profile operation completed successfully (copy, delete, save, set-active).
    ProfileOperationDone(String),
    /// A profile operation failed.
    ProfileOperationError(String),
    /// Settings form data loaded from daemon.
    SettingsData(String),
    /// Keyboard state loaded from daemon.
    KeyboardData(String),
    /// Charging settings loaded from daemon.
    ChargingData(String),
    /// GPU/power info loaded from daemon.
    GpuInfo(String),
    /// Power settings loaded from daemon.
    PowerData(String),
    /// Display settings loaded from daemon.
    DisplayData(String),
    /// Webcam device list loaded from daemon.
    WebcamDevices(Vec<String>),
    /// Webcam controls loaded for a device.
    WebcamData(String),
    /// A form-tab save succeeded (tab name for status).
    FormSaved(String),
    /// A form-tab save failed (tab name + error).
    FormSaveError(String),
}
