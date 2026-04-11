//! Charging threshold and profile control backends.

pub mod clevo;
pub mod uniwill;

use std::io;

/// Backend for controlling battery charging behaviour.
///
/// Two strategies exist:
/// - **Flexicharger** (Clevo): start/end thresholds as percentages (0–100%).
/// - **EC Profile+Priority** (Uniwill): named charge profiles + priority setting.
pub trait ChargingBackend: Send + Sync {
    /// Get the start threshold (0–100%). Charging begins when battery drops below this.
    fn get_start_threshold(&self) -> io::Result<u8>;

    /// Set the start threshold (0–100%).
    fn set_start_threshold(&self, pct: u8) -> io::Result<()>;

    /// Get the end threshold (0–100%). Charging stops when battery reaches this.
    fn get_end_threshold(&self) -> io::Result<u8>;

    /// Set the end threshold (0–100%).
    fn set_end_threshold(&self, pct: u8) -> io::Result<()>;

    /// Get the current charge profile name, if applicable.
    fn get_profile(&self) -> io::Result<Option<String>>;

    /// Set the charge profile by name, if applicable.
    fn set_profile(&self, profile: &str) -> io::Result<()>;

    /// Get the current charge priority, if applicable.
    fn get_priority(&self) -> io::Result<Option<String>>;

    /// Set the charge priority, if applicable.
    fn set_priority(&self, priority: &str) -> io::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify the trait is object-safe.
    fn _assert_object_safe(_: &dyn ChargingBackend) {}
}
