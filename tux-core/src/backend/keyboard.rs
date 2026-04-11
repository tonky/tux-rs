use std::io;

/// Hardware abstraction for keyboard backlight control.
///
/// Supports single-color, zoned RGB, and per-key RGB keyboards.
/// ITE HID keyboards use a separate userspace path but implement this same trait.
pub trait KeyboardBackend: Send + Sync {
    /// Set backlight brightness (0–255).
    fn set_brightness(&self, brightness: u8) -> io::Result<()>;

    /// Set color for a specific zone (zone 0 = whole keyboard for single-zone).
    fn set_color(&self, zone: u8, r: u8, g: u8, b: u8) -> io::Result<()>;

    /// Set lighting mode/effect by name.
    fn set_mode(&self, mode: &str) -> io::Result<()>;

    /// Number of independently controllable zones.
    fn zone_count(&self) -> u8;

    /// List available lighting mode names.
    fn available_modes(&self) -> Vec<String>;
}
