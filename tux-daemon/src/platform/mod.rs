use std::sync::Arc;

use tux_core::backend::fan::FanBackend;
use tux_core::dmi::DetectedDevice;
use tux_core::platform::Platform;

use tracing::debug;

pub mod sysfs;
pub mod tuxedo_io;

mod td_clevo;
mod td_nb04;
mod td_nb05;
mod td_tuxi;
mod td_uniwill;
mod td_uw_fan;

pub use td_clevo::TdClevoFanBackend;
pub use td_nb04::TdNb04FanBackend;
pub use td_nb05::TdNb05FanBackend;
pub use td_tuxi::TdTuxiFanBackend;
pub use td_uniwill::TdUniwillFanBackend;
pub use td_uw_fan::TdUwFanBackend;

/// Create the appropriate `FanBackend` for the detected hardware.
///
/// Uses tuxedo-drivers for all platforms. Returns `None` for platforms without
/// direct fan control (NB04) or when the driver is not loaded.
pub fn init_fan_backend(device: &DetectedDevice) -> Option<Arc<dyn FanBackend>> {
    match device.descriptor.platform {
        Platform::Uniwill => {
            // Prefer the sysfs interface from tuxedo_uw_fan if available.
            if let Some(backend) = TdUwFanBackend::new() {
                debug!("Uniwill: using tuxedo_uw_fan sysfs backend");
                return Some(Arc::new(backend));
            }
            // Fallback: ioctl via tuxedo_io (requires tuxedo_keyboard WMI registration).
            let io_dev = match tuxedo_io::TuxedoIoDevice::open() {
                Some(d) => d,
                None => {
                    debug!("Uniwill: failed to open {}", tuxedo_io::TUXEDO_IO_DEVICE);
                    return None;
                }
            };
            if !io_dev.is_uniwill_hardware() {
                debug!("Uniwill: R_HWCHECK_UW returned false/error");
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
