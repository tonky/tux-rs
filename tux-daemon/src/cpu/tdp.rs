//! TDP (PL1/PL2) control.
//!
//! Three backends are provided:
//! - `EcTdp`: EC-RAM based (NB05 platforms — Pulse, InfinityFlex).
//! - `RaplTdp`: Intel RAPL via `/sys/class/powercap/intel-rapl:0/`.
//! - `UniwillEcTdp`: Uniwill EC via `/dev/tuxedo_io` ioctls (e.g. IBM15I10).
//!
//! Both implement [`TdpBackend`]. Selection is driven by
//! `DeviceDescriptor::tdp_source` via [`build_backend`].

use std::io;
use std::path::Path;
use std::sync::Arc;

use tracing::{debug, info, warn};
use tux_core::device::{DeviceDescriptor, TdpBounds, TdpSource};

use crate::platform::sysfs::SysfsReader;
use crate::platform::tuxedo_io::{
    R_UW_TDP0, R_UW_TDP0_MAX, R_UW_TDP0_MIN, R_UW_TDP1, R_UW_TDP1_MAX, R_UW_TDP1_MIN,
    R_UW_TDP2_MAX, R_UW_TDP2_MIN, TuxedoIo, W_UW_TDP0, W_UW_TDP1,
};

// ─── Backend factory ─────────────────────────────────────────────────────────

/// Build the appropriate TDP backend for `descriptor`, or `None` if TDP
/// control is not available (either `TdpSource::None` or a probe failure).
///
/// Selection rules:
/// - `TdpSource::None` → always `None`.
/// - `TdpSource::Ec` → requires `descriptor.tdp` bounds; tries `EcTdp::new`.
/// - `TdpSource::Rapl` → probes RAPL sysfs; bounds come from firmware.
///   `descriptor.tdp` is ignored.
/// - `TdpSource::UniwillEc` → opens `/dev/tuxedo_io` (Uniwill interface);
///   bounds come from the driver. `descriptor.tdp` is ignored.
///
/// No fallthrough: if the requested backend is unavailable, we log and return
/// `None` — we never silently try a different backend.
pub fn build_backend(descriptor: &DeviceDescriptor) -> Option<Arc<dyn TdpBackend>> {
    build_backend_with(descriptor, open_uniwill_io)
}

/// Open `/dev/tuxedo_io` if it serves the Uniwill interface.
fn open_uniwill_io() -> Option<Arc<dyn TuxedoIo>> {
    let dev = crate::platform::tuxedo_io::TuxedoIoDevice::open()?;
    dev.is_uniwill_hardware()
        .then(|| Arc::new(dev) as Arc<dyn TuxedoIo>)
}

/// [`build_backend`] with an injectable `tuxedo_io` opener (hermetic tests).
fn build_backend_with(
    descriptor: &DeviceDescriptor,
    open_io: impl FnOnce() -> Option<Arc<dyn TuxedoIo>>,
) -> Option<Arc<dyn TdpBackend>> {
    match descriptor.tdp_source {
        TdpSource::None => {
            info!("no TDP control for this device");
            None
        }
        TdpSource::Ec => {
            let bounds = match descriptor.tdp {
                Some(b) => b,
                None => {
                    warn!(
                        "device {:?} has TdpSource::Ec but no TDP bounds; skipping",
                        descriptor.product_sku
                    );
                    return None;
                }
            };
            match EcTdp::new(bounds) {
                Some(b) => {
                    info!(
                        "EC TDP backend available (PL1: {}-{}W, PL2: {}-{}W)",
                        bounds.pl1_min, bounds.pl1_max, bounds.pl2_min, bounds.pl2_max
                    );
                    Some(Arc::new(b))
                }
                None => {
                    info!("EC TDP sysfs not available");
                    None
                }
            }
        }
        TdpSource::Rapl => match RaplTdp::probe() {
            Some(b) => {
                let bounds = b.bounds();
                info!(
                    "RAPL TDP backend available (PL1: {}-{}W, PL2: {}-{}W)",
                    bounds.pl1_min, bounds.pl1_max, bounds.pl2_min, bounds.pl2_max
                );
                Some(Arc::new(b))
            }
            None => {
                warn!(
                    "device {:?} requests RAPL but intel-rapl:0 not available; no TDP control",
                    descriptor.product_sku
                );
                None
            }
        },
        TdpSource::UniwillEc => {
            let Some(io) = open_io() else {
                warn!(
                    "device {:?} requests Uniwill EC TDP but /dev/tuxedo_io (Uniwill) is unavailable",
                    descriptor.product_sku
                );
                return None;
            };
            match UniwillEcTdp::probe(io) {
                Some(b) => {
                    let bounds = b.bounds();
                    info!(
                        "Uniwill EC TDP backend available (PL1: {}-{}W, PL2: {}-{}W, PL4 read-only: {:?}-{:?}W)",
                        bounds.pl1_min,
                        bounds.pl1_max,
                        bounds.pl2_min,
                        bounds.pl2_max,
                        bounds.pl4_min,
                        bounds.pl4_max
                    );
                    Some(Arc::new(b))
                }
                None => {
                    warn!(
                        "device {:?}: tuxedo_io reports no TDP table for this board; no TDP control",
                        descriptor.product_sku
                    );
                    None
                }
            }
        }
    }
}

// ─── EC-based TDP constants (NB05) ───────────────────────────────────────────

const SYSFS_BASE: &str = "/sys/devices/platform/tuxedo-ec";
const EC_RAM_ATTR: &str = "ec_ram";

// EC addresses for PL1/PL2 — used on NB05 (Pulse/InfinityFlex) platforms.
const EC_PL1_ADDR: u64 = 0x0783;
const EC_PL2_ADDR: u64 = 0x0784;

// ─── Intel RAPL constants (/sys/class/powercap/intel-rapl:0) ─────────────────

// Laptop-only; desktops/servers with MMIO-only RAPL zones (intel-rapl-mmio)
// are out of scope. See impl/.../follow_up.toml.
const RAPL_BASE: &str = "/sys/class/powercap/intel-rapl:0";
const RAPL_NAME_ATTR: &str = "name";
const RAPL_EXPECTED_NAME: &str = "package-0";
const RAPL_PL1_LIMIT: &str = "constraint_0_power_limit_uw";
const RAPL_PL1_MAX: &str = "constraint_0_max_power_uw";
const RAPL_PL1_NAME: &str = "constraint_0_name";
const RAPL_PL1_EXPECTED_NAME: &str = "long_term";
const RAPL_PL2_LIMIT: &str = "constraint_1_power_limit_uw";
const RAPL_PL2_MAX: &str = "constraint_1_max_power_uw";
const RAPL_PL2_NAME: &str = "constraint_1_name";
const RAPL_PL2_EXPECTED_NAME: &str = "short_term";

/// Conservative lower bound when firmware does not publish a minimum.
/// RAPL itself will clamp further if the hardware enforces a higher floor.
const RAPL_FLOOR_W: u32 = 1;

/// Microwatts per watt (RAPL sysfs unit).
const UW_PER_W: u32 = 1_000_000;

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

// ─── Intel RAPL backend ──────────────────────────────────────────────────────

/// Intel RAPL-based TDP control via `/sys/class/powercap/intel-rapl:0`.
///
/// - PL1 ≡ constraint_0 (long_term)
/// - PL2 ≡ constraint_1 (short_term)
/// - Units on sysfs are microwatts; the [`TdpBackend`] API is integer watts.
///
/// Firmware-locked systems (MSR_PKG_POWER_LIMIT bit 63 set) reject writes
/// with `EPERM`/`EACCES`. This surfaces as `io::ErrorKind::PermissionDenied`
/// from `set_pl1`/`set_pl2`; callers should log and continue rather than
/// treat it as fatal.
pub struct RaplTdp {
    sysfs: SysfsReader,
    bounds: TdpBounds,
}

impl RaplTdp {
    /// Probe the real RAPL sysfs tree. Returns `None` if RAPL is unavailable
    /// or the package-0 domain cannot be identified.
    pub fn probe() -> Option<Self> {
        Self::probe_at(Path::new(RAPL_BASE))
    }

    /// Probe a RAPL sysfs tree rooted at `base`. Extracted so tests can
    /// point at a hermetic tempdir.
    pub fn probe_at(base: &Path) -> Option<Self> {
        let sysfs = SysfsReader::new(base);
        if !sysfs.available() || !sysfs.exists(RAPL_NAME_ATTR) {
            debug!("RAPL probe: no intel-rapl:0 at {base:?}");
            return None;
        }
        match sysfs.read_str(RAPL_NAME_ATTR) {
            Ok(name) if name == RAPL_EXPECTED_NAME => {}
            Ok(other) => {
                debug!("RAPL probe: domain name {other:?} != {RAPL_EXPECTED_NAME:?}; skipping");
                return None;
            }
            Err(e) => {
                warn!("RAPL probe: failed to read {RAPL_NAME_ATTR}: {e}");
                return None;
            }
        }
        // Defensive: the index→PL mapping (constraint_0 = PL1, constraint_1 = PL2)
        // is convention, not guaranteed by the kernel API. Verify that firmware
        // agrees with our assumption before caching bounds.
        if !Self::constraint_name_matches(&sysfs, RAPL_PL1_NAME, RAPL_PL1_EXPECTED_NAME)
            || !Self::constraint_name_matches(&sysfs, RAPL_PL2_NAME, RAPL_PL2_EXPECTED_NAME)
        {
            return None;
        }
        match Self::read_bounds(&sysfs) {
            Ok(bounds) => Some(Self { sysfs, bounds }),
            Err(e) => {
                warn!("RAPL probe: failed to read bounds: {e}");
                None
            }
        }
    }

    fn constraint_name_matches(sysfs: &SysfsReader, attr: &str, expected: &str) -> bool {
        match sysfs.read_str(attr) {
            Ok(n) if n == expected => true,
            Ok(other) => {
                debug!("RAPL probe: {attr} is {other:?}, expected {expected:?}; skipping");
                false
            }
            Err(e) => {
                // Missing name attrs on older kernels would land here. Treat
                // as a probe failure so we don't silently mis-map PL1/PL2.
                debug!("RAPL probe: failed to read {attr}: {e}; skipping");
                false
            }
        }
    }

    #[cfg(test)]
    pub fn with_path(path: impl Into<std::path::PathBuf>, bounds: TdpBounds) -> Self {
        Self {
            sysfs: SysfsReader::new(path),
            bounds,
        }
    }

    fn read_bounds(sysfs: &SysfsReader) -> io::Result<TdpBounds> {
        let pl1_max_uw = sysfs.read_u32(RAPL_PL1_MAX)?;
        let pl2_max_uw = sysfs.read_u32(RAPL_PL2_MAX)?;
        // Intentional floor division: a firmware-advertised 28.5 W bound becomes
        // 28 W. This keeps the user-facing unit at integer watts and errs on
        // the conservative side — RAPL itself still enforces the real limit.
        let pl1_max = (pl1_max_uw / UW_PER_W).max(RAPL_FLOOR_W);
        // Some firmware (e.g. IBP1XI08MK1) reports 0 for constraint_1_max_power_uw.
        // Fall back to pl1_max so the TUI field remains usable.
        let pl2_max = match pl2_max_uw / UW_PER_W {
            0 => pl1_max,
            w => w,
        };
        Ok(TdpBounds {
            pl1_min: RAPL_FLOOR_W,
            pl1_max,
            pl2_min: RAPL_FLOOR_W,
            pl2_max,
            pl4_min: None,
            pl4_max: None,
        })
    }
}

#[inline]
fn uw_to_w(uw: u32) -> u32 {
    uw / UW_PER_W
}

impl TdpBackend for RaplTdp {
    fn get_pl1(&self) -> io::Result<u32> {
        Ok(uw_to_w(self.sysfs.read_u32(RAPL_PL1_LIMIT)?))
    }

    fn set_pl1(&self, watts: u32) -> io::Result<()> {
        let w = watts.clamp(self.bounds.pl1_min, self.bounds.pl1_max);
        self.sysfs
            .write_u32(RAPL_PL1_LIMIT, w.saturating_mul(UW_PER_W))
    }

    fn get_pl2(&self) -> io::Result<u32> {
        Ok(uw_to_w(self.sysfs.read_u32(RAPL_PL2_LIMIT)?))
    }

    fn set_pl2(&self, watts: u32) -> io::Result<()> {
        let w = watts.clamp(self.bounds.pl2_min, self.bounds.pl2_max);
        self.sysfs
            .write_u32(RAPL_PL2_LIMIT, w.saturating_mul(UW_PER_W))
    }

    fn bounds(&self) -> &TdpBounds {
        &self.bounds
    }
}

// ─── Uniwill EC backend (tuxedo_io) ──────────────────────────────────────────

/// Uniwill EC TDP control through the `tuxedo_io` ioctl interface.
///
/// The driver owns a per-board bounds table (e.g. X5AR45xS: PL1/PL2 10–90 W,
/// PL4 10–230 W) and is the source of truth for bounds. Its ioctls never fail:
/// reads report errors as a negative errno *value*, and out-of-range writes
/// are dropped silently, so every write here is clamped and verified by
/// reading it back. PL4 is exposed in the bounds only (read-only).
///
/// On boards with `uniwill_custom_profile_mode_needed` the driver switches the
/// EC performance profile to "overboost" before each TDP write.
pub struct UniwillEcTdp {
    io: Arc<dyn TuxedoIo>,
    bounds: TdpBounds,
}

impl UniwillEcTdp {
    /// Read bounds from the driver; `None` if the board has no TDP table or
    /// the current PL1 cannot be read.
    pub fn probe(io: Arc<dyn TuxedoIo>) -> Option<Self> {
        let read = |cmd| Self::read_watts(io.as_ref(), cmd).ok();
        let bounds = TdpBounds {
            pl1_min: read(R_UW_TDP0_MIN)?,
            pl1_max: read(R_UW_TDP0_MAX)?,
            pl2_min: read(R_UW_TDP1_MIN)?,
            pl2_max: read(R_UW_TDP1_MAX)?,
            pl4_min: read(R_UW_TDP2_MIN),
            pl4_max: read(R_UW_TDP2_MAX),
        };
        if bounds.pl1_min > bounds.pl1_max || bounds.pl2_min > bounds.pl2_max {
            warn!("Uniwill EC TDP: inconsistent driver bounds {bounds:?}; skipping");
            return None;
        }
        if let Err(e) = Self::read_watts(io.as_ref(), R_UW_TDP0) {
            warn!("Uniwill EC TDP: PL1 not readable: {e}");
            return None;
        }
        Some(Self { io, bounds })
    }

    /// Decode a tuxedo_io read: non-negative watts, or a negative errno.
    fn read_watts(io: &dyn TuxedoIo, cmd: u64) -> io::Result<u32> {
        let v = io.read_i32(cmd)?;
        if v < 0 {
            Err(io::Error::from_raw_os_error(-v))
        } else {
            Ok(v as u32)
        }
    }

    fn set_verified(&self, label: &str, write: u64, read: u64, watts: u32) -> io::Result<()> {
        self.io.write_i32(write, watts as i32)?;
        let actual = Self::read_watts(self.io.as_ref(), read)?;
        if actual == watts {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "EC did not accept {label}={watts} W (reads back {actual} W)"
            )))
        }
    }
}

impl TdpBackend for UniwillEcTdp {
    fn get_pl1(&self) -> io::Result<u32> {
        Self::read_watts(self.io.as_ref(), R_UW_TDP0)
    }

    fn set_pl1(&self, watts: u32) -> io::Result<()> {
        let w = watts.clamp(self.bounds.pl1_min, self.bounds.pl1_max);
        self.set_verified("PL1", W_UW_TDP0, R_UW_TDP0, w)
    }

    fn get_pl2(&self) -> io::Result<u32> {
        Self::read_watts(self.io.as_ref(), R_UW_TDP1)
    }

    fn set_pl2(&self, watts: u32) -> io::Result<()> {
        let w = watts.clamp(self.bounds.pl2_min, self.bounds.pl2_max);
        self.set_verified("PL2", W_UW_TDP1, R_UW_TDP1, w)
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

    // ─── RAPL tests ──────────────────────────────────────────────────────

    /// Populate a fake `intel-rapl:0` directory with realistic attributes.
    fn setup_rapl(dir: &std::path::Path) {
        fs::write(dir.join(RAPL_NAME_ATTR), "package-0\n").unwrap();
        fs::write(dir.join(RAPL_PL1_NAME), "long_term\n").unwrap();
        fs::write(dir.join(RAPL_PL1_LIMIT), "15000000\n").unwrap(); // 15 W
        fs::write(dir.join(RAPL_PL1_MAX), "28000000\n").unwrap(); //   28 W
        fs::write(dir.join(RAPL_PL2_NAME), "short_term\n").unwrap();
        fs::write(dir.join(RAPL_PL2_LIMIT), "28000000\n").unwrap(); // 28 W
        fs::write(dir.join(RAPL_PL2_MAX), "40000000\n").unwrap(); //   40 W
    }

    fn rapl_bounds_from_sysfs() -> TdpBounds {
        TdpBounds {
            pl1_min: RAPL_FLOOR_W,
            pl1_max: 28,
            pl2_min: RAPL_FLOOR_W,
            pl2_max: 40,
            pl4_min: None,
            pl4_max: None,
        }
    }

    fn read_uw(path: &std::path::Path) -> u32 {
        fs::read_to_string(path).unwrap().trim().parse().unwrap()
    }

    #[test]
    fn rapl_get_pl1_reads_sysfs() {
        let tmp = tempfile::tempdir().unwrap();
        setup_rapl(tmp.path());
        let tdp = RaplTdp::with_path(tmp.path(), rapl_bounds_from_sysfs());

        assert_eq!(tdp.get_pl1().unwrap(), 15);
    }

    #[test]
    fn rapl_get_pl2_reads_sysfs() {
        let tmp = tempfile::tempdir().unwrap();
        setup_rapl(tmp.path());
        let tdp = RaplTdp::with_path(tmp.path(), rapl_bounds_from_sysfs());

        assert_eq!(tdp.get_pl2().unwrap(), 28);
    }

    #[test]
    fn rapl_set_pl1_writes_microwatts() {
        let tmp = tempfile::tempdir().unwrap();
        setup_rapl(tmp.path());
        let tdp = RaplTdp::with_path(tmp.path(), rapl_bounds_from_sysfs());

        tdp.set_pl1(20).unwrap();
        assert_eq!(read_uw(&tmp.path().join(RAPL_PL1_LIMIT)), 20_000_000);

        // Above bounds → clamped to 28 W.
        tdp.set_pl1(100).unwrap();
        assert_eq!(read_uw(&tmp.path().join(RAPL_PL1_LIMIT)), 28_000_000);

        // Below floor → clamped to 1 W.
        tdp.set_pl1(0).unwrap();
        assert_eq!(read_uw(&tmp.path().join(RAPL_PL1_LIMIT)), 1_000_000);
    }

    #[test]
    fn rapl_set_pl2_writes_microwatts() {
        let tmp = tempfile::tempdir().unwrap();
        setup_rapl(tmp.path());
        let tdp = RaplTdp::with_path(tmp.path(), rapl_bounds_from_sysfs());

        tdp.set_pl2(35).unwrap();
        assert_eq!(read_uw(&tmp.path().join(RAPL_PL2_LIMIT)), 35_000_000);

        tdp.set_pl2(250).unwrap();
        assert_eq!(read_uw(&tmp.path().join(RAPL_PL2_LIMIT)), 40_000_000);

        tdp.set_pl2(0).unwrap();
        assert_eq!(read_uw(&tmp.path().join(RAPL_PL2_LIMIT)), 1_000_000);
    }

    #[test]
    fn rapl_probe_reads_bounds() {
        let tmp = tempfile::tempdir().unwrap();
        setup_rapl(tmp.path());
        let tdp = RaplTdp::probe_at(tmp.path()).expect("probe should succeed");

        let b = tdp.bounds();
        assert_eq!(b.pl1_min, 1);
        assert_eq!(b.pl1_max, 28);
        assert_eq!(b.pl2_min, 1);
        assert_eq!(b.pl2_max, 40);
    }

    #[test]
    fn rapl_probe_rejects_wrong_name() {
        let tmp = tempfile::tempdir().unwrap();
        setup_rapl(tmp.path());
        // Overwrite with a non-package domain (e.g. psys or dram sub-zone).
        fs::write(tmp.path().join(RAPL_NAME_ATTR), "psys\n").unwrap();

        assert!(RaplTdp::probe_at(tmp.path()).is_none());
    }

    #[test]
    fn rapl_probe_requires_name_attr() {
        let tmp = tempfile::tempdir().unwrap();
        // Empty dir: no `name` file.
        assert!(RaplTdp::probe_at(tmp.path()).is_none());
    }

    #[test]
    fn rapl_probe_missing_base_returns_none() {
        // A path that doesn't exist at all must not panic. Build it under a
        // tempdir (which we let go out of scope first) so we don't depend on
        // any absolute filesystem layout.
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist");
        drop(tmp);
        assert!(RaplTdp::probe_at(&missing).is_none());
    }

    #[test]
    fn rapl_probe_rejects_mismatched_constraint_name() {
        // Firmware reorders constraints (constraint_0 != "long_term"):
        // probe must refuse rather than silently mis-map PL1/PL2.
        let tmp = tempfile::tempdir().unwrap();
        setup_rapl(tmp.path());
        fs::write(tmp.path().join(RAPL_PL1_NAME), "peak_power\n").unwrap();

        assert!(RaplTdp::probe_at(tmp.path()).is_none());
    }

    #[test]
    fn rapl_probe_rejects_unparseable_bounds() {
        // A bogus max-power value must produce `None` (logged warn!), not a
        // panic and not a half-initialised backend.
        let tmp = tempfile::tempdir().unwrap();
        setup_rapl(tmp.path());
        fs::write(tmp.path().join(RAPL_PL1_MAX), "not-a-number\n").unwrap();

        assert!(RaplTdp::probe_at(tmp.path()).is_none());
    }

    // NOTE: firmware-lock behaviour (MSR_PKG_POWER_LIMIT bit 63 → EPERM on
    // write) cannot be exercised hermetically because root (which the test
    // harness and the live daemon both run as) bypasses DAC `0o444`. The
    // `set_pl{1,2}` signatures already enforce `io::Result` handling at
    // every caller; validating the real behaviour is deferred to the
    // Stage 4 live-test on hardware.

    // ─── build_backend factory tests ────────────────────────────────

    use tux_core::device::*;
    use tux_core::platform::Platform;
    use tux_core::registers::PlatformRegisters;

    fn test_descriptor(tdp_source: TdpSource, tdp: Option<TdpBounds>) -> DeviceDescriptor {
        DeviceDescriptor {
            name: "Test Device",
            product_sku: "TEST_TDP",
            platform: Platform::Uniwill,
            fans: FanCapability {
                count: 1,
                control: FanControlType::Direct,
                pwm_scale: 200,
            },
            keyboard: KeyboardType::None,
            sensors: SensorSet {
                cpu_temp: true,
                gpu_temp: false,
                fan_rpm: &[true],
            },
            charging: ChargingCapability::None,
            tdp,
            tdp_source,
            gpu_power: GpuPowerCapability::None,
            registers: PlatformRegisters::Uniwill,
        }
    }

    #[test]
    fn factory_none_returns_none() {
        let desc = test_descriptor(TdpSource::None, None);
        assert!(build_backend(&desc).is_none());
    }

    #[test]
    fn factory_none_ignores_bounds() {
        // Even if bounds are present, TdpSource::None must not probe anything.
        let desc = test_descriptor(TdpSource::None, Some(test_bounds()));
        assert!(build_backend(&desc).is_none());
    }

    #[test]
    fn factory_ec_without_bounds_returns_none() {
        let desc = test_descriptor(TdpSource::Ec, None);
        assert!(build_backend(&desc).is_none());
    }

    #[test]
    fn factory_uniwill_ec_uses_driver_bounds() {
        let desc = test_descriptor(TdpSource::UniwillEc, None);
        let backend = build_backend_with(&desc, || {
            Some(Arc::new(UniwillEcSim::x5ar45xs()) as Arc<dyn TuxedoIo>)
        })
        .expect("backend");
        assert_eq!(backend.bounds().pl1_max, 90);
        assert_eq!(backend.get_pl1().unwrap(), 90);
    }

    #[test]
    fn factory_uniwill_ec_without_device_returns_none() {
        let desc = test_descriptor(TdpSource::UniwillEc, None);
        assert!(build_backend_with(&desc, || None).is_none());
    }

    #[test]
    fn factory_uniwill_ec_without_driver_table_returns_none() {
        let desc = test_descriptor(TdpSource::UniwillEc, None);
        let sim = UniwillEcSim {
            table: None,
            ..UniwillEcSim::x5ar45xs()
        };
        assert!(build_backend_with(&desc, || Some(Arc::new(sim) as Arc<dyn TuxedoIo>)).is_none());
    }

    #[test]
    fn factory_non_uniwill_sources_never_open_tuxedo_io() {
        for source in [TdpSource::None, TdpSource::Ec] {
            let desc = test_descriptor(source, None);
            let _ = build_backend_with(&desc, || panic!("{source:?} must not open tuxedo_io"));
        }
    }

    #[test]
    // build_backend calls the real RAPL sysfs path; on Linux hardware that path
    // exists, so this test cannot be hermetic. The absent-path contract is covered
    // by the dedicated `rapl_probe_missing_dir_returns_none` test above.
    #[cfg_attr(target_os = "linux", ignore)]
    fn factory_rapl_without_sysfs_returns_none() {
        let desc = test_descriptor(TdpSource::Rapl, None);
        assert!(build_backend(&desc).is_none());
    }

    // ─── UniwillEcTdp (tuxedo_io) ───────────────────────────────────

    /// Simulates tuxedo_io's Uniwill TDP ioctls for board X5AR45xS.
    struct UniwillEcSim {
        /// `None` models a board without a driver TDP table (-ENODEV).
        table: Option<[(i32, i32); 3]>,
        current: std::sync::Mutex<[i32; 3]>,
        /// When set, writes are dropped even if in range (EC refuses).
        drop_writes: bool,
    }

    const ENODEV: i32 = 19;

    impl UniwillEcSim {
        fn x5ar45xs() -> Self {
            Self {
                table: Some([(10, 90), (10, 90), (10, 230)]),
                current: std::sync::Mutex::new([90, 90, 230]),
                drop_writes: false,
            }
        }
    }

    impl TuxedoIo for UniwillEcSim {
        fn read_i32(&self, cmd: u64) -> io::Result<i32> {
            let idx = |base: u64| (cmd - base) as usize;
            let Some(table) = self.table else {
                return Ok(-ENODEV);
            };
            Ok(match cmd {
                R_UW_TDP0..=crate::platform::tuxedo_io::R_UW_TDP2 => {
                    self.current.lock().unwrap()[idx(R_UW_TDP0)]
                }
                R_UW_TDP0_MIN..=R_UW_TDP2_MIN => table[idx(R_UW_TDP0_MIN)].0,
                R_UW_TDP0_MAX..=R_UW_TDP2_MAX => table[idx(R_UW_TDP0_MAX)].1,
                _ => return Err(io::Error::from(io::ErrorKind::Unsupported)),
            })
        }

        fn write_i32(&self, cmd: u64, val: i32) -> io::Result<()> {
            let i = (cmd - W_UW_TDP0) as usize;
            if let Some(table) = self.table {
                // Driver: out-of-range → -EINVAL internally, ioctl still Ok.
                let (min, max) = table[i];
                if (min..=max).contains(&val) && !self.drop_writes {
                    self.current.lock().unwrap()[i] = val;
                }
            }
            Ok(())
        }

        fn ioctl_noarg(&self, _cmd: u64) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn uniwill_ec_probe_reads_driver_bounds() {
        let tdp = UniwillEcTdp::probe(Arc::new(UniwillEcSim::x5ar45xs())).unwrap();
        let b = tdp.bounds();
        assert_eq!((b.pl1_min, b.pl1_max), (10, 90));
        assert_eq!((b.pl2_min, b.pl2_max), (10, 90));
        assert_eq!((b.pl4_min, b.pl4_max), (Some(10), Some(230)));
        assert_eq!(tdp.get_pl1().unwrap(), 90);
        assert_eq!(tdp.get_pl2().unwrap(), 90);
    }

    #[test]
    fn uniwill_ec_probe_without_driver_table_is_none() {
        let sim = UniwillEcSim {
            table: None,
            ..UniwillEcSim::x5ar45xs()
        };
        assert!(UniwillEcTdp::probe(Arc::new(sim)).is_none());
    }

    #[test]
    fn uniwill_ec_set_roundtrip() {
        let tdp = UniwillEcTdp::probe(Arc::new(UniwillEcSim::x5ar45xs())).unwrap();
        tdp.set_pl1(45).unwrap();
        tdp.set_pl2(60).unwrap();
        assert_eq!(tdp.get_pl1().unwrap(), 45);
        assert_eq!(tdp.get_pl2().unwrap(), 60);
    }

    #[test]
    fn uniwill_ec_set_clamps_to_driver_bounds() {
        // The driver would silently drop 5 W / 200 W; we clamp instead.
        let tdp = UniwillEcTdp::probe(Arc::new(UniwillEcSim::x5ar45xs())).unwrap();
        tdp.set_pl1(5).unwrap();
        assert_eq!(tdp.get_pl1().unwrap(), 10);
        tdp.set_pl2(200).unwrap();
        assert_eq!(tdp.get_pl2().unwrap(), 90);
    }

    #[test]
    fn uniwill_ec_set_detects_silently_dropped_write() {
        let sim = UniwillEcSim {
            drop_writes: true,
            ..UniwillEcSim::x5ar45xs()
        };
        let tdp = UniwillEcTdp::probe(Arc::new(sim)).unwrap();
        let err = tdp.set_pl1(45).unwrap_err();
        assert!(err.to_string().contains("did not accept PL1=45"), "{err}");
    }

    #[test]
    fn uniwill_ec_negative_read_is_errno() {
        let v = UniwillEcTdp::read_watts(
            &UniwillEcSim {
                table: None,
                ..UniwillEcSim::x5ar45xs()
            },
            R_UW_TDP0,
        )
        .unwrap_err();
        assert_eq!(v.raw_os_error(), Some(ENODEV));
    }
}
