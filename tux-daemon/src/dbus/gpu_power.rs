//! D-Bus GPU Power interface: `com.tuxedocomputers.tccd.GpuPower`.

use std::sync::Arc;

use zbus::interface;

use crate::gpu::GpuPowerBackend;

pub struct GpuPowerInterface {
    backend: Arc<dyn GpuPowerBackend>,
}

impl GpuPowerInterface {
    pub fn new(backend: Arc<dyn GpuPowerBackend>) -> Self {
        Self { backend }
    }
}

#[interface(name = "com.tuxedocomputers.tccd.GpuPower")]
impl GpuPowerInterface {
    fn get_ctgp_offset(&self) -> zbus::fdo::Result<u8> {
        self.backend
            .get_ctgp_offset()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    fn set_ctgp_offset(&self, watts: u8) -> zbus::fdo::Result<()> {
        self.backend
            .set_ctgp_offset(watts)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    struct MockGpuPower {
        value: std::sync::Mutex<u8>,
    }

    impl MockGpuPower {
        fn new(initial: u8) -> Self {
            Self {
                value: std::sync::Mutex::new(initial),
            }
        }
    }

    impl GpuPowerBackend for MockGpuPower {
        fn get_ctgp_offset(&self) -> io::Result<u8> {
            Ok(*self.value.lock().unwrap())
        }

        fn set_ctgp_offset(&self, watts: u8) -> io::Result<()> {
            *self.value.lock().unwrap() = watts;
            Ok(())
        }
    }

    #[test]
    fn get_ctgp_returns_value() {
        let iface = GpuPowerInterface::new(Arc::new(MockGpuPower::new(12)));
        assert_eq!(iface.get_ctgp_offset().unwrap(), 12);
    }

    #[test]
    fn set_ctgp_updates_value() {
        let mock = Arc::new(MockGpuPower::new(0));
        let iface = GpuPowerInterface::new(mock.clone());
        iface.set_ctgp_offset(20).unwrap();
        assert_eq!(mock.get_ctgp_offset().unwrap(), 20);
    }

    struct FailingGpuPower;

    impl GpuPowerBackend for FailingGpuPower {
        fn get_ctgp_offset(&self) -> io::Result<u8> {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "no access"))
        }

        fn set_ctgp_offset(&self, _watts: u8) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "no access"))
        }
    }

    #[test]
    fn get_ctgp_propagates_error() {
        let iface = GpuPowerInterface::new(Arc::new(FailingGpuPower));
        assert!(iface.get_ctgp_offset().is_err());
    }

    #[test]
    fn set_ctgp_propagates_error() {
        let iface = GpuPowerInterface::new(Arc::new(FailingGpuPower));
        assert!(iface.set_ctgp_offset(10).is_err());
    }
}
