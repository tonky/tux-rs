//! Commands produced by the update layer, executed asynchronously against D-Bus.

use tux_core::fan_curve::FanCurvePoint;

/// A side-effect command to be executed after updating state.
#[derive(Debug)]
pub enum Command {
    /// No operation.
    None,
    /// Quit the application.
    Quit,
    /// Save the fan curve to the daemon.
    SaveFanCurve(Vec<FanCurvePoint>),
    /// Fetch the current fan curve from the daemon.
    FetchFanCurve,
    /// Fetch the list of profiles + assignments.
    FetchProfiles,
    /// Copy a profile by ID.
    CopyProfile(String),
    /// Create a new profile from TOML (used for copy-with-current-state).
    CreateProfile(String),
    /// Delete a profile by ID.
    DeleteProfile(String),
    /// Save a profile (id + TOML).
    SaveProfile { id: String, toml: String },
    /// Set the active profile for a power state (id, "ac" or "battery").
    SetActiveProfile { id: String, state: String },
    /// Save settings form as TOML.
    SaveSettings(String),
    /// Save keyboard form as TOML.
    SaveKeyboard(String),
    /// Save charging form as TOML.
    SaveCharging(String),
    /// Save power settings as TOML.
    SavePower(String),
    /// Save display settings as TOML.
    SaveDisplay(String),
    /// Save webcam controls as TOML (includes device name).
    SaveWebcam { device: String, toml: String },
}
