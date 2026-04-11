//! tux-core: Hardware model system for TUXEDO laptops.

pub mod backend;
pub mod custom_device;
pub mod dbus_types;
pub mod device;
pub mod device_table;
pub mod dmi;
pub mod fan_curve;
pub mod mock;
pub mod platform;
pub mod profile;
pub mod registers;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_set() {
        assert!(!version().is_empty());
    }
}
