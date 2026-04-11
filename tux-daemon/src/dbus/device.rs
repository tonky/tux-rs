//! D-Bus Device interface: `com.tuxedocomputers.tccd.Device`.

use zbus::interface;

/// D-Bus object implementing the Device interface.
pub struct DeviceInterface {
    device_name: String,
    platform: String,
    daemon_version: String,
}

impl DeviceInterface {
    pub fn new(device_name: String, platform: String) -> Self {
        Self {
            device_name,
            platform,
            daemon_version: tux_core::version().to_string(),
        }
    }
}

#[interface(name = "com.tuxedocomputers.tccd.Device")]
impl DeviceInterface {
    /// Human-readable device model name.
    #[zbus(property)]
    fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Platform identifier (e.g., "Uniwill", "Clevo").
    #[zbus(property)]
    fn platform(&self) -> &str {
        &self.platform
    }

    /// Daemon version string.
    #[zbus(property)]
    fn daemon_version(&self) -> &str {
        &self.daemon_version
    }
}
