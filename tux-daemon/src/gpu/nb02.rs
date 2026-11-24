//! NB02 (Uniwill) NVIDIA GPU power control via kernel sysfs.
//!
//! The `tuxedo_nb02_nvidia_power_ctrl` kernel module exposes cTGP offset
//! at `/sys/devices/platform/tuxedo_nvidia_power_ctrl/ctgp_offset`.
//!
//! The driver accepts any `u8`, so — like TUXEDO Control Center — the
//! writable range is bounded by the GPU's own power limits from `nvidia-smi`:
//! `0 ..= power.max_limit − power.default_limit` (e.g. RTX 5060 Mobile on
//! IBM15I10: 115 W − 50 W = 65 W). Without `nvidia-smi` the offset stays
//! readable but writes are refused.

use std::io::{self, Read};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tracing::{info, warn};

use crate::platform::sysfs::SysfsReader;

use super::GpuPowerBackend;

const SYSFS_BASE: &str = "/sys/devices/platform/tuxedo_nvidia_power_ctrl";

/// Upper bound for a single `nvidia-smi` call so a wedged GPU can't stall startup.
const NVIDIA_SMI_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Nb02GpuPower {
    sysfs: SysfsReader,
    /// Maximum writable offset in watts; `None` when the range is unknown.
    offset_max: Option<u8>,
}

impl Nb02GpuPower {
    pub fn new() -> Option<Self> {
        Self::probe(SYSFS_BASE, query_nvidia_smi_limits)
    }

    /// Probe `base` for the `ctgp_offset` attribute; `None` if absent.
    ///
    /// `limits` is only consulted when the attribute exists and returns
    /// `(default_limit_w, max_limit_w)`.
    fn probe(
        base: impl Into<std::path::PathBuf>,
        limits: impl FnOnce() -> Option<(f64, f64)>,
    ) -> Option<Self> {
        let sysfs = SysfsReader::new(base);
        if !sysfs.available() || !sysfs.exists("ctgp_offset") {
            return None;
        }
        let offset_max = match limits() {
            Some((default_w, max_w)) => {
                let max = offset_max_from_limits(default_w, max_w);
                info!(
                    "NVIDIA power limits: default {default_w} W, max {max_w} W → cTGP offset 0–{max} W"
                );
                Some(max)
            }
            None => {
                warn!("NVIDIA power limits unavailable (nvidia-smi); cTGP offset is read-only");
                None
            }
        };
        Some(Self { sysfs, offset_max })
    }

    #[cfg(test)]
    pub fn with_path(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            sysfs: SysfsReader::new(path),
            offset_max: Some(u8::MAX),
        }
    }
}

impl GpuPowerBackend for Nb02GpuPower {
    fn get_ctgp_offset(&self) -> io::Result<u8> {
        self.sysfs.read_u8("ctgp_offset")
    }

    fn set_ctgp_offset(&self, watts: u8) -> io::Result<()> {
        match self.offset_max {
            None => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "cTGP offset range unknown (nvidia-smi unavailable); refusing to write",
            )),
            Some(max) if watts > max => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("cTGP offset {watts} W exceeds maximum {max} W"),
            )),
            Some(_) => self.sysfs.write_u8("ctgp_offset", watts),
        }
    }

    fn ctgp_offset_max(&self) -> u8 {
        self.offset_max.unwrap_or(0)
    }
}

/// Offset range from NVIDIA limits: `max − default`, rounded, clamped to `u8`.
fn offset_max_from_limits(default_w: f64, max_w: f64) -> u8 {
    (max_w - default_w).round().clamp(0.0, f64::from(u8::MAX)) as u8
}

/// Parse `nvidia-smi --format=csv,noheader,nounits
/// --query-gpu=power.default_limit,power.max_limit` output (first GPU).
fn parse_nvidia_smi_limits(output: &str) -> Option<(f64, f64)> {
    let line = output.lines().map(str::trim).find(|l| !l.is_empty())?;
    let mut parts = line.split(',').map(|p| p.trim().parse::<f64>());
    let default_w = parts.next()?.ok()?;
    let max_w = parts.next()?.ok()?;
    (default_w.is_finite() && max_w.is_finite() && default_w > 0.0 && max_w >= default_w)
        .then_some((default_w, max_w))
}

fn query_nvidia_smi_limits() -> Option<(f64, f64)> {
    let mut child = Command::new("nvidia-smi")
        .args([
            "--format=csv,noheader,nounits",
            "--query-gpu=power.default_limit,power.max_limit",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + NVIDIA_SMI_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) => return None,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                warn!("nvidia-smi did not finish within {NVIDIA_SMI_TIMEOUT:?}; killing it");
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    parse_nvidia_smi_limits(&out)
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

    // Probe tests use tempdirs: the real /sys path exists on NB02 machines
    // with an NVIDIA dGPU (e.g. IBM15I10), so they must not touch it.

    #[test]
    fn probe_returns_none_when_dir_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let limits_called = std::cell::Cell::new(false);
        let probe = Nb02GpuPower::probe(tmp.path().join("missing"), || {
            limits_called.set(true);
            None
        });
        assert!(probe.is_none());
        assert!(
            !limits_called.get(),
            "nvidia-smi must not run without sysfs"
        );
    }

    #[test]
    fn probe_returns_none_without_ctgp_offset() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(Nb02GpuPower::probe(tmp.path(), || Some((50.0, 115.0))).is_none());
    }

    #[test]
    fn probe_succeeds_with_ctgp_offset() {
        let tmp = tempfile::tempdir().unwrap();
        setup_sysfs(tmp.path(), 5);
        let gpu =
            Nb02GpuPower::probe(tmp.path(), || Some((50.0, 115.0))).expect("probe should succeed");
        assert_eq!(gpu.get_ctgp_offset().unwrap(), 5);
    }

    // ── Bounds from NVIDIA power limits ────────────────────────────────────

    #[test]
    fn ibm15i10_rtx5060_limits_bound_offset_to_65w() {
        // nvidia-smi on IBM15I10: default 50 W, max 115 W.
        let tmp = tempfile::tempdir().unwrap();
        setup_sysfs(tmp.path(), 65);
        let gpu = Nb02GpuPower::probe(tmp.path(), || Some((50.0, 115.0))).unwrap();
        assert_eq!(gpu.ctgp_offset_max(), 65);

        gpu.set_ctgp_offset(65).unwrap();
        let err = gpu.set_ctgp_offset(66).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            gpu.get_ctgp_offset().unwrap(),
            65,
            "rejected write must not land"
        );
    }

    #[test]
    fn unknown_limits_make_offset_read_only() {
        let tmp = tempfile::tempdir().unwrap();
        setup_sysfs(tmp.path(), 30);
        let gpu = Nb02GpuPower::probe(tmp.path(), || None).unwrap();
        assert_eq!(gpu.ctgp_offset_max(), 0);
        assert_eq!(gpu.get_ctgp_offset().unwrap(), 30);
        let err = gpu.set_ctgp_offset(0).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
        assert_eq!(gpu.get_ctgp_offset().unwrap(), 30);
    }

    #[test]
    fn offset_max_rounds_and_clamps() {
        assert_eq!(offset_max_from_limits(50.0, 115.0), 65);
        assert_eq!(offset_max_from_limits(80.0, 80.4), 0);
        assert_eq!(offset_max_from_limits(80.0, 95.6), 16);
        assert_eq!(offset_max_from_limits(10.0, 400.0), 255);
    }

    #[test]
    fn parse_nvidia_smi_limits_variants() {
        assert_eq!(
            parse_nvidia_smi_limits("50.00, 115.00\n"),
            Some((50.0, 115.0))
        );
        // Multi-GPU: first line wins.
        assert_eq!(
            parse_nvidia_smi_limits("\n80.00, 150.00\n60.00, 90.00\n"),
            Some((80.0, 150.0))
        );
        assert_eq!(parse_nvidia_smi_limits("[N/A], [N/A]"), None);
        assert_eq!(
            parse_nvidia_smi_limits("115.00, 50.00"),
            None,
            "max < default"
        );
        assert_eq!(parse_nvidia_smi_limits("50.00"), None);
        assert_eq!(parse_nvidia_smi_limits(""), None);
    }
}
