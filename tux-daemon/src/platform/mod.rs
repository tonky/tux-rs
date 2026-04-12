use std::sync::Arc;

use tux_core::backend::fan::FanBackend;
use tux_core::dmi::DetectedDevice;
use tux_core::platform::Platform;

pub mod sysfs;
pub mod tuxedo_io;

mod clevo;
mod nb05;
mod td_clevo;
mod td_nb04;
mod td_nb05;
mod td_tuxi;
mod td_uniwill;
mod tuxi;
mod uniwill;

pub use clevo::ClevoFanBackend;
pub use nb05::Nb05FanBackend;
pub use td_clevo::TdClevoFanBackend;
pub use td_nb04::TdNb04FanBackend;
pub use td_nb05::TdNb05FanBackend;
pub use td_tuxi::TdTuxiFanBackend;
pub use td_uniwill::TdUniwillFanBackend;
pub use tuxi::TuxiFanBackend;
pub use uniwill::UniwillFanBackend;

/// Create the appropriate `FanBackend` for the detected hardware.
///
/// For platforms with tuxedo-drivers support, the new td_* backend is preferred.
/// Falls back to the legacy tux-kmod backend when tuxedo-drivers is not loaded.
/// Returns `None` for platforms without direct fan control (NB04).
pub fn init_fan_backend(device: &DetectedDevice) -> Option<Arc<dyn FanBackend>> {
    match device.descriptor.platform {
        Platform::Uniwill => {
            // Try tuxedo-drivers (tuxedo_io) first; fall back to tux-kmod sysfs.
            if let Some(io_dev) = tuxedo_io::TuxedoIoDevice::open() {
                if io_dev.is_uniwill_hardware() {
                    let backend =
                        TdUniwillFanBackend::new(Arc::new(io_dev) as Arc<dyn tuxedo_io::TuxedoIo>);
                    return Some(Arc::new(backend));
                }
            }
            let backend = UniwillFanBackend::new()?;
            Some(Arc::new(backend))
        }
        Platform::Tuxi => {
            // Prefer tuxedo-drivers (TdTuxi); fall back to tux-kmod sysfs.
            let num_fans = device.descriptor.fans.count;
            if let Some(backend) = TdTuxiFanBackend::new(num_fans) {
                return Some(Arc::new(backend));
            }
            let backend = TuxiFanBackend::new()?;
            Some(Arc::new(backend))
        }
        Platform::Clevo => {
            // Try tuxedo-drivers (tuxedo_io) first; fall back to tux-kmod sysfs.
            if let Some(io_dev) = tuxedo_io::TuxedoIoDevice::open() {
                if io_dev.is_clevo_hardware() {
                    let backend = TdClevoFanBackend::new(
                        Arc::new(io_dev) as Arc<dyn tuxedo_io::TuxedoIo>,
                        device.descriptor.fans.count,
                    );
                    return Some(Arc::new(backend));
                }
            }
            let backend = ClevoFanBackend::new(device.descriptor.fans.count)?;
            Some(Arc::new(backend))
        }
        Platform::Nb05 => {
            // Prefer tuxedo-drivers interface (TdNb05); fall back to tux-kmod (Nb05) if not present.
            let num_fans = device.descriptor.fans.count;
            if let Some(backend) = TdNb05FanBackend::new(num_fans) {
                return Some(Arc::new(backend));
            }
            let backend = Nb05FanBackend::new(device)?;
            Some(Arc::new(backend))
        }
        Platform::Nb04 => None,
    }
}
