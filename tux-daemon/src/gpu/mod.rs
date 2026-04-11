//! GPU power control and telemetry backends.

pub mod hwmon;
pub mod nb02;

use std::io;

/// GPU power control backend trait.
pub trait GpuPowerBackend: Send + Sync {
    /// Get the current cTGP offset in watts.
    fn get_ctgp_offset(&self) -> io::Result<u8>;

    /// Set the cTGP offset in watts.
    fn set_ctgp_offset(&self, watts: u8) -> io::Result<()>;
}
