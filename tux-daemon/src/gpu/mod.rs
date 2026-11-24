//! GPU power control and telemetry backends.

pub mod hwmon;
pub mod intel;
pub mod nb02;
pub mod nvidia;

use std::io;

/// GPU power control backend trait.
pub trait GpuPowerBackend: Send + Sync {
    /// Get the current cTGP offset in watts.
    fn get_ctgp_offset(&self) -> io::Result<u8>;

    /// Set the cTGP offset in watts.
    fn set_ctgp_offset(&self, watts: u8) -> io::Result<()>;

    /// Largest offset `set_ctgp_offset` accepts; `0` means not adjustable.
    fn ctgp_offset_max(&self) -> u8;

    /// NVIDIA `(default_limit, max_limit)` in watts, when known.
    fn power_limits_w(&self) -> Option<(f64, f64)> {
        None
    }
}
