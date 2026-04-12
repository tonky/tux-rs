use std::sync::Arc;

use tux_core::backend::fan::FanBackend;
use tux_core::dmi::DetectedDevice;
use tux_core::platform::Platform;

pub mod sysfs;
pub mod tuxedo_io;

mod td_clevo;
mod td_nb04;
mod td_nb05;
mod td_tuxi;
mod td_uniwill;

pub use td_clevo::TdClevoFanBackend;
pub use td_nb04::TdNb04FanBackend;
pub use td_nb05::TdNb05FanBackend;
pub use td_tuxi::TdTuxiFanBackend;
pub use td_uniwill::TdUniwillFanBackend;

/// Create the appropriate `FanBackend` for the detected hardware.
///
/// Uses tuxedo-drivers for all platforms. Returns `None` for platforms without
/// direct fan control (NB04) or when the driver is not loaded.
pub fn init_fan_backend(device: &DetectedDevice) -> Option<Arc<dyn FanBackend>> {
    match device.descriptor.platform {
        Platform::Uniwill => {
            let io_dev = tuxedo_io::TuxedoIoDevice::open()?;
            if !io_dev.is_uniwill_hardware() {
                return None;
            }
            let backend =
                TdUniwillFanBackend::new(Arc::new(io_dev) as Arc<dyn tuxedo_io::TuxedoIo>);
            Some(Arc::new(backend))
        }
        Platform::Tuxi => {
            let num_fans = device.descriptor.fans.count;
            let backend = TdTuxiFanBackend::new(num_fans)?;
            Some(Arc::new(backend))
        }
        Platform::Clevo => {
            let io_dev = tuxedo_io::TuxedoIoDevice::open()?;
            if !io_dev.is_clevo_hardware() {
                return None;
            }
            let backend = TdClevoFanBackend::new(
                Arc::new(io_dev) as Arc<dyn tuxedo_io::TuxedoIo>,
                device.descriptor.fans.count,
            );
            Some(Arc::new(backend))
        }
        Platform::Nb05 => {
            let num_fans = device.descriptor.fans.count;
            let backend = TdNb05FanBackend::new(num_fans)?;
            Some(Arc::new(backend))
        }
        Platform::Nb04 => None,
    }
}
