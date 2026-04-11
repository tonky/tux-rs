use std::sync::Arc;

use tux_core::backend::fan::FanBackend;
use tux_core::dmi::DetectedDevice;
use tux_core::platform::Platform;

pub mod sysfs;

mod clevo;
mod nb05;
mod tuxi;
mod uniwill;

pub use clevo::ClevoFanBackend;
pub use nb05::Nb05FanBackend;
pub use tuxi::TuxiFanBackend;
pub use uniwill::UniwillFanBackend;

/// Create the appropriate `FanBackend` for the detected hardware.
///
/// Returns `None` for platforms without direct fan control (NB04).
pub fn init_fan_backend(device: &DetectedDevice) -> Option<Arc<dyn FanBackend>> {
    match device.descriptor.platform {
        Platform::Uniwill => {
            let backend = UniwillFanBackend::new()?;
            Some(Arc::new(backend))
        }
        Platform::Tuxi => {
            let backend = TuxiFanBackend::new()?;
            Some(Arc::new(backend))
        }
        Platform::Clevo => {
            let backend = ClevoFanBackend::new(device.descriptor.fans.count)?;
            Some(Arc::new(backend))
        }
        Platform::Nb05 => {
            let backend = Nb05FanBackend::new(device)?;
            Some(Arc::new(backend))
        }
        Platform::Nb04 => None,
    }
}
