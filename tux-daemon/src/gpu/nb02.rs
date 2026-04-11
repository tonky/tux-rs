//! NB02 (Uniwill) NVIDIA GPU power control via kernel sysfs.
//!
//! The `tuxedo_nb02_nvidia_power_ctrl` kernel module exposes cTGP offset
//! at `/sys/devices/platform/tuxedo_nvidia_power_ctrl/ctgp_offset`.

use std::io;

use crate::platform::sysfs::SysfsReader;

use super::GpuPowerBackend;

const SYSFS_BASE: &str = "/sys/devices/platform/tuxedo_nvidia_power_ctrl";

pub struct Nb02GpuPower {
    sysfs: SysfsReader,
}

impl Nb02GpuPower {
    pub fn new() -> Option<Self> {
        let sysfs = SysfsReader::new(SYSFS_BASE);
        if !sysfs.available() || !sysfs.exists("ctgp_offset") {
            return None;
        }
        Some(Self { sysfs })
    }

    #[cfg(test)]
    pub fn with_path(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            sysfs: SysfsReader::new(path),
        }
    }
}

impl GpuPowerBackend for Nb02GpuPower {
    fn get_ctgp_offset(&self) -> io::Result<u8> {
        self.sysfs.read_u8("ctgp_offset")
    }

    fn set_ctgp_offset(&self, watts: u8) -> io::Result<()> {
        self.sysfs.write_u8("ctgp_offset", watts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_sysfs(dir: &std::path::Path, ctgp: u8) {
        fs::write(dir.join("ctgp_offset"), format!("{ctgp}\n")).unwrap();
    }

    #[test]
    fn get_ctgp_offset_reads_sysfs() {
        let tmp = tempfile::tempdir().unwrap();
        setup_sysfs(tmp.path(), 10);
        let gpu = Nb02GpuPower::with_path(tmp.path());

        assert_eq!(gpu.get_ctgp_offset().unwrap(), 10);
    }

    #[test]
    fn set_ctgp_offset_writes_sysfs() {
        let tmp = tempfile::tempdir().unwrap();
        setup_sysfs(tmp.path(), 0);
        let gpu = Nb02GpuPower::with_path(tmp.path());

        gpu.set_ctgp_offset(15).unwrap();
        assert_eq!(gpu.get_ctgp_offset().unwrap(), 15);
    }

    #[test]
    fn get_ctgp_offset_handles_zero() {
        let tmp = tempfile::tempdir().unwrap();
        setup_sysfs(tmp.path(), 0);
        let gpu = Nb02GpuPower::with_path(tmp.path());

        assert_eq!(gpu.get_ctgp_offset().unwrap(), 0);
    }

    #[test]
    fn get_ctgp_offset_handles_max() {
        let tmp = tempfile::tempdir().unwrap();
        setup_sysfs(tmp.path(), 255);
        let gpu = Nb02GpuPower::with_path(tmp.path());

        assert_eq!(gpu.get_ctgp_offset().unwrap(), 255);
    }

    #[test]
    fn new_returns_none_when_sysfs_missing() {
        assert!(Nb02GpuPower::new().is_none());
    }
}
