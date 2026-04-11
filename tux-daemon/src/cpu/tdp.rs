//! TDP (PL1/PL2) control via EC RAM.
//!
//! Reads/writes power limits through the tuxedo-ec binary sysfs attribute.
//! Values are clamped to the `TdpBounds` defined in the device descriptor.

use std::io;

use tux_core::device::TdpBounds;

use crate::platform::sysfs::SysfsReader;

const SYSFS_BASE: &str = "/sys/devices/platform/tuxedo-ec";
const EC_RAM_ATTR: &str = "ec_ram";

// EC addresses for PL1/PL2 — used on NB05 (Pulse/InfinityFlex) platforms.
const EC_PL1_ADDR: u64 = 0x0783;
const EC_PL2_ADDR: u64 = 0x0784;

/// TDP backend trait for reading/writing power limits.
pub trait TdpBackend: Send + Sync {
    fn get_pl1(&self) -> io::Result<u32>;
    fn set_pl1(&self, watts: u32) -> io::Result<()>;
    fn get_pl2(&self) -> io::Result<u32>;
    fn set_pl2(&self, watts: u32) -> io::Result<()>;
    fn bounds(&self) -> &TdpBounds;
}

/// EC-based TDP control for NB05 platforms.
pub struct EcTdp {
    sysfs: SysfsReader,
    bounds: TdpBounds,
}

impl EcTdp {
    pub fn new(bounds: TdpBounds) -> Option<Self> {
        let sysfs = SysfsReader::new(SYSFS_BASE);
        if !sysfs.available() || !sysfs.exists(EC_RAM_ATTR) {
            return None;
        }
        Some(Self { sysfs, bounds })
    }

    #[cfg(test)]
    pub fn with_path(path: impl Into<std::path::PathBuf>, bounds: TdpBounds) -> Self {
        Self {
            sysfs: SysfsReader::new(path),
            bounds,
        }
    }

    fn ec_read(&self, addr: u64) -> io::Result<u8> {
        let buf = self.sysfs.pread(EC_RAM_ATTR, addr, 1)?;
        Ok(buf[0])
    }

    fn ec_write(&self, addr: u64, val: u8) -> io::Result<()> {
        self.sysfs.pwrite(EC_RAM_ATTR, addr, &[val])
    }
}

impl TdpBackend for EcTdp {
    fn get_pl1(&self) -> io::Result<u32> {
        Ok(self.ec_read(EC_PL1_ADDR)? as u32)
    }

    fn set_pl1(&self, watts: u32) -> io::Result<()> {
        let clamped = watts.clamp(self.bounds.pl1_min, self.bounds.pl1_max);
        self.ec_write(EC_PL1_ADDR, clamped as u8)
    }

    fn get_pl2(&self) -> io::Result<u32> {
        Ok(self.ec_read(EC_PL2_ADDR)? as u32)
    }

    fn set_pl2(&self, watts: u32) -> io::Result<()> {
        let clamped = watts.clamp(self.bounds.pl2_min, self.bounds.pl2_max);
        self.ec_write(EC_PL2_ADDR, clamped as u8)
    }

    fn bounds(&self) -> &TdpBounds {
        &self.bounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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

    fn setup_ec_ram(dir: &std::path::Path) -> std::path::PathBuf {
        let ec_path = dir.join(EC_RAM_ATTR);
        // Create a file large enough for our EC addresses
        let mut data = vec![0u8; 0x0800];
        data[EC_PL1_ADDR as usize] = 15; // 15W PL1
        data[EC_PL2_ADDR as usize] = 25; // 25W PL2
        fs::write(&ec_path, &data).unwrap();
        dir.to_path_buf()
    }

    #[test]
    fn get_pl1_reads_ec() {
        let tmp = tempfile::tempdir().unwrap();
        setup_ec_ram(tmp.path());
        let tdp = EcTdp::with_path(tmp.path(), test_bounds());

        assert_eq!(tdp.get_pl1().unwrap(), 15);
    }

    #[test]
    fn get_pl2_reads_ec() {
        let tmp = tempfile::tempdir().unwrap();
        setup_ec_ram(tmp.path());
        let tdp = EcTdp::with_path(tmp.path(), test_bounds());

        assert_eq!(tdp.get_pl2().unwrap(), 25);
    }

    #[test]
    fn set_pl1_clamps_to_bounds() {
        let tmp = tempfile::tempdir().unwrap();
        setup_ec_ram(tmp.path());
        let tdp = EcTdp::with_path(tmp.path(), test_bounds());

        // Within bounds
        tdp.set_pl1(20).unwrap();
        assert_eq!(tdp.get_pl1().unwrap(), 20);

        // Below min → clamped to 5
        tdp.set_pl1(1).unwrap();
        assert_eq!(tdp.get_pl1().unwrap(), 5);

        // Above max → clamped to 28
        tdp.set_pl1(100).unwrap();
        assert_eq!(tdp.get_pl1().unwrap(), 28);
    }

    #[test]
    fn set_pl2_clamps_to_bounds() {
        let tmp = tempfile::tempdir().unwrap();
        setup_ec_ram(tmp.path());
        let tdp = EcTdp::with_path(tmp.path(), test_bounds());

        tdp.set_pl2(35).unwrap();
        assert_eq!(tdp.get_pl2().unwrap(), 35);

        // Below min → 10
        tdp.set_pl2(2).unwrap();
        assert_eq!(tdp.get_pl2().unwrap(), 10);

        // Above max → 40
        tdp.set_pl2(99).unwrap();
        assert_eq!(tdp.get_pl2().unwrap(), 40);
    }

    #[test]
    fn bounds_returned() {
        let tmp = tempfile::tempdir().unwrap();
        setup_ec_ram(tmp.path());
        let b = test_bounds();
        let tdp = EcTdp::with_path(tmp.path(), b);

        assert_eq!(tdp.bounds().pl1_min, 5);
        assert_eq!(tdp.bounds().pl1_max, 28);
    }
}
