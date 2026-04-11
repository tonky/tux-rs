//! D-Bus CPU interface: `com.tuxedocomputers.tccd.Cpu`.
//!
//! Exposes CPU governor, EPP, turbo, and TDP control over D-Bus.

use std::sync::Arc;

use zbus::interface;

use crate::cpu::governor::CpuGovernor;
use crate::cpu::tdp::TdpBackend;

/// D-Bus object for CPU governor + TDP control.
pub struct CpuInterface {
    governor: Arc<CpuGovernor>,
    tdp: Option<Arc<dyn TdpBackend>>,
}

impl CpuInterface {
    pub fn new(governor: Arc<CpuGovernor>, tdp: Option<Arc<dyn TdpBackend>>) -> Self {
        Self { governor, tdp }
    }
}

#[interface(name = "com.tuxedocomputers.tccd.Cpu")]
impl CpuInterface {
    /// Get the current CPU governor.
    fn get_governor(&self) -> zbus::fdo::Result<String> {
        self.governor
            .get_governor()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Set the CPU governor for all CPUs.
    fn set_governor(&self, governor: &str) -> zbus::fdo::Result<()> {
        self.governor
            .set_governor(governor)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get the current energy performance preference. Returns empty string if unavailable.
    fn get_epp(&self) -> zbus::fdo::Result<String> {
        match self.governor.get_epp() {
            Ok(Some(epp)) => Ok(epp),
            Ok(None) => Ok(String::new()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }

    /// Set the energy performance preference for all CPUs.
    fn set_epp(&self, epp: &str) -> zbus::fdo::Result<()> {
        self.governor
            .set_epp(epp)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get whether turbo boost is disabled.
    fn get_no_turbo(&self) -> zbus::fdo::Result<bool> {
        self.governor
            .get_no_turbo()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Set the turbo boost disable flag.
    fn set_no_turbo(&self, no_turbo: bool) -> zbus::fdo::Result<()> {
        self.governor
            .set_no_turbo(no_turbo)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get available CPU governors.
    fn available_governors(&self) -> zbus::fdo::Result<Vec<String>> {
        self.governor
            .available_governors()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get PL1 power limit in watts. Returns 0 if TDP control is unavailable.
    fn get_pl1(&self) -> zbus::fdo::Result<u32> {
        match &self.tdp {
            Some(tdp) => tdp
                .get_pl1()
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string())),
            None => Ok(0),
        }
    }

    /// Set PL1 power limit in watts.
    fn set_pl1(&self, watts: u32) -> zbus::fdo::Result<()> {
        match &self.tdp {
            Some(tdp) => tdp
                .set_pl1(watts)
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string())),
            None => Err(zbus::fdo::Error::NotSupported(
                "TDP control not available on this platform".to_string(),
            )),
        }
    }

    /// Get PL2 power limit in watts. Returns 0 if TDP control is unavailable.
    fn get_pl2(&self) -> zbus::fdo::Result<u32> {
        match &self.tdp {
            Some(tdp) => tdp
                .get_pl2()
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string())),
            None => Ok(0),
        }
    }

    /// Set PL2 power limit in watts.
    fn set_pl2(&self, watts: u32) -> zbus::fdo::Result<()> {
        match &self.tdp {
            Some(tdp) => tdp
                .set_pl2(watts)
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string())),
            None => Err(zbus::fdo::Error::NotSupported(
                "TDP control not available on this platform".to_string(),
            )),
        }
    }

    /// Get TDP bounds as TOML.
    fn get_tdp_bounds(&self) -> zbus::fdo::Result<String> {
        match &self.tdp {
            Some(tdp) => {
                let b = tdp.bounds();
                let toml = format!(
                    "pl1_min = {}\npl1_max = {}\npl2_min = {}\npl2_max = {}\n",
                    b.pl1_min, b.pl1_max, b.pl2_min, b.pl2_max
                );
                Ok(toml)
            }
            None => Ok(String::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::tdp::EcTdp;
    use std::fs;
    use tux_core::device::TdpBounds;

    fn test_bounds() -> TdpBounds {
        TdpBounds {
            pl1_min: 5,
            pl1_max: 28,
            pl2_min: 10,
            pl2_max: 40,
            pl4_min: None,
            pl4_max: None,
        }
    }

    #[test]
    fn tdp_set_get_via_interface() {
        let tmp = tempfile::tempdir().unwrap();
        let ec_path = tmp.path().join("ec_ram");
        let data = vec![0u8; 0x0800];
        fs::write(&ec_path, &data).unwrap();

        let tdp = EcTdp::with_path(tmp.path(), test_bounds());
        let gov = Arc::new(CpuGovernor::with_path(tmp.path()));
        let iface = CpuInterface::new(gov, Some(Arc::new(tdp)));

        iface.set_pl1(20).unwrap();
        assert_eq!(iface.get_pl1().unwrap(), 20);

        iface.set_pl2(30).unwrap();
        assert_eq!(iface.get_pl2().unwrap(), 30);
    }

    #[test]
    fn tdp_none_returns_zero_for_gets() {
        let gov = Arc::new(CpuGovernor::with_path("/nonexistent"));
        let iface = CpuInterface::new(gov, None);

        assert_eq!(iface.get_pl1().unwrap(), 0);
        assert_eq!(iface.get_pl2().unwrap(), 0);
    }

    #[test]
    fn tdp_none_returns_error_for_sets() {
        let gov = Arc::new(CpuGovernor::with_path("/nonexistent"));
        let iface = CpuInterface::new(gov, None);

        assert!(iface.set_pl1(20).is_err());
        assert!(iface.set_pl2(30).is_err());
    }

    #[test]
    fn tdp_bounds_toml() {
        let tmp = tempfile::tempdir().unwrap();
        let ec_path = tmp.path().join("ec_ram");
        let data = vec![0u8; 0x0800];
        fs::write(&ec_path, &data).unwrap();

        let tdp = EcTdp::with_path(tmp.path(), test_bounds());
        let gov = Arc::new(CpuGovernor::with_path(tmp.path()));
        let iface = CpuInterface::new(gov, Some(Arc::new(tdp)));

        let toml = iface.get_tdp_bounds().unwrap();
        assert!(toml.contains("pl1_min = 5"));
        assert!(toml.contains("pl1_max = 28"));
    }
}
