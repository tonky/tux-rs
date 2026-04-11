use std::io;

/// Hardware abstraction for power profile control.
pub trait ProfileBackend: Send + Sync {
    /// Get the current active profile name.
    fn get_profile(&self) -> io::Result<String>;

    /// Set the active power profile.
    fn set_profile(&self, profile: &str) -> io::Result<()>;

    /// List available power profiles.
    fn available_profiles(&self) -> Vec<String>;
}
