//! NB02 (Uniwill) NVIDIA GPU power control via kernel sysfs.
//!
//! The `tuxedo_nb02_nvidia_power_ctrl` kernel module exposes cTGP offset
//! at `/sys/devices/platform/tuxedo_nvidia_power_ctrl/ctgp_offset`.
//!
//! The driver accepts any `u8`, so — like TUXEDO Control Center — the
//! writable range is bounded by the GPU's own power limits from `nvidia-smi`:
//! `0 ..= power.max_limit − power.default_limit` (e.g. RTX 5060 Mobile on
//! IBM15I10: 115 W − 50 W = 65 W). While the limits are unknown (no
//! `nvidia-smi`, or the NVIDIA driver not ready yet at boot) the offset stays
//! readable, writes are refused, and the query is retried lazily.

use std::io::{self, Read};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tracing::{info, warn};

use crate::platform::sysfs::SysfsReader;

use super::GpuPowerBackend;

const SYSFS_BASE: &str = "/sys/devices/platform/tuxedo_nvidia_power_ctrl";

/// Upper bound for a single `nvidia-smi` call so a wedged GPU can't stall startup.
const NVIDIA_SMI_TIMEOUT: Duration = Duration::from_secs(5);

/// Minimum gap between `nvidia-smi` retries while the limits are unknown.
const LIMITS_RETRY_INTERVAL: Duration = Duration::from_secs(30);

/// Source of `(default_limit_w, max_limit_w)`.
type LimitsQuery = Box<dyn Fn() -> Option<(f64, f64)> + Send + Sync>;

/// Lazily resolved cTGP offset range.
struct OffsetRange {
    query: LimitsQuery,
    retry_interval: Duration,
    /// `(max, last_attempt)`; `max` stays `None` until a query succeeds.
    state: Mutex<(Option<u8>, Option<Instant>)>,
}

impl OffsetRange {
    fn new(query: LimitsQuery, retry_interval: Duration) -> Self {
        Self {
            query,
            retry_interval,
            state: Mutex::new((None, None)),
        }
    }

    /// Maximum writable offset, querying (rate-limited) while unknown.
    fn get(&self) -> Option<u8> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let (max, last_attempt) = &mut *state;
        if max.is_none() && last_attempt.is_none_or(|t| t.elapsed() >= self.retry_interval) {
            let first = last_attempt.is_none();
            *last_attempt = Some(Instant::now());
            match (self.query)() {
                Some((default_w, max_w)) => {
                    let m = offset_max_from_limits(default_w, max_w);
                    info!(
                        "NVIDIA power limits: default {default_w} W, max {max_w} W → cTGP offset 0–{m} W"
                    );
                    *max = Some(m);
                }
                None if first => warn!(
                    "NVIDIA power limits unavailable (nvidia-smi); cTGP offset read-only, retrying every {:?}",
                    self.retry_interval
                ),
                None => {}
            }
        }
        *max
    }
}

pub struct Nb02GpuPower {
    sysfs: SysfsReader,
    range: OffsetRange,
}

impl Nb02GpuPower {
    pub fn new() -> Option<Self> {
        Self::probe(
            SYSFS_BASE,
            Box::new(query_nvidia_smi_limits),
            LIMITS_RETRY_INTERVAL,
        )
    }

    /// Probe `base` for the `ctgp_offset` attribute; `None` if absent.
    ///
    /// `limits` is only consulted when the attribute exists (once eagerly, so
    /// the range is logged at startup, then lazily while still unknown).
    fn probe(
        base: impl Into<std::path::PathBuf>,
        limits: LimitsQuery,
        retry_interval: Duration,
    ) -> Option<Self> {
        let sysfs = SysfsReader::new(base);
        if !sysfs.available() || !sysfs.exists("ctgp_offset") {
            return None;
        }
        let range = OffsetRange::new(limits, retry_interval);
        range.get();
        Some(Self { sysfs, range })
    }

    /// Unbounded test backend (0–255 W) over an existing sysfs dir.
    #[cfg(test)]
    pub fn with_path(path: impl Into<std::path::PathBuf>) -> Self {
        Self::probe(path, Box::new(|| Some((1.0, 256.0))), Duration::ZERO)
            .expect("test sysfs must contain ctgp_offset")
    }
}

impl GpuPowerBackend for Nb02GpuPower {
    fn get_ctgp_offset(&self) -> io::Result<u8> {
        self.sysfs.read_u8("ctgp_offset")
    }

    fn set_ctgp_offset(&self, watts: u8) -> io::Result<()> {
        match self.range.get() {
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
        self.range.get().unwrap_or(0)
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
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn rtx5060() -> LimitsQuery {
        Box::new(|| Some((50.0, 115.0)))
    }

    fn counting_query(calls: &Arc<AtomicUsize>, result: Option<(f64, f64)>) -> LimitsQuery {
        let calls = calls.clone();
        Box::new(move || {
            calls.fetch_add(1, Ordering::SeqCst);
            result
        })
    }

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
        let calls = Arc::new(AtomicUsize::new(0));
        let probe = Nb02GpuPower::probe(
            tmp.path().join("missing"),
            counting_query(&calls, Some((50.0, 115.0))),
            Duration::ZERO,
        );
        assert!(probe.is_none());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "nvidia-smi must not run without sysfs"
        );
    }

    #[test]
    fn probe_returns_none_without_ctgp_offset() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(Nb02GpuPower::probe(tmp.path(), rtx5060(), Duration::ZERO).is_none());
    }

    #[test]
    fn probe_succeeds_with_ctgp_offset() {
        let tmp = tempfile::tempdir().unwrap();
        setup_sysfs(tmp.path(), 5);
        let gpu = Nb02GpuPower::probe(tmp.path(), rtx5060(), Duration::ZERO)
            .expect("probe should succeed");
        assert_eq!(gpu.get_ctgp_offset().unwrap(), 5);
    }

    // ── Bounds from NVIDIA power limits ────────────────────────────────────

    #[test]
    fn ibm15i10_rtx5060_limits_bound_offset_to_65w() {
        // nvidia-smi on IBM15I10: default 50 W, max 115 W.
        let tmp = tempfile::tempdir().unwrap();
        setup_sysfs(tmp.path(), 65);
        let gpu = Nb02GpuPower::probe(tmp.path(), rtx5060(), Duration::ZERO).unwrap();
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
        let gpu =
            Nb02GpuPower::probe(tmp.path(), Box::new(|| None), LIMITS_RETRY_INTERVAL).unwrap();
        assert_eq!(gpu.ctgp_offset_max(), 0);
        assert_eq!(gpu.get_ctgp_offset().unwrap(), 30);
        let err = gpu.set_ctgp_offset(0).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
        assert_eq!(gpu.get_ctgp_offset().unwrap(), 30);
    }

    #[test]
    fn limits_are_retried_until_nvidia_smi_answers() {
        // Boot race: the NVIDIA driver may not be ready when the daemon starts.
        let tmp = tempfile::tempdir().unwrap();
        setup_sysfs(tmp.path(), 0);
        let calls = Arc::new(AtomicUsize::new(0));
        let ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let query: LimitsQuery = {
            let (calls, ready) = (calls.clone(), ready.clone());
            Box::new(move || {
                calls.fetch_add(1, Ordering::SeqCst);
                ready.load(Ordering::SeqCst).then_some((50.0, 115.0))
            })
        };
        let gpu = Nb02GpuPower::probe(tmp.path(), query, Duration::ZERO).unwrap();
        assert_eq!(gpu.ctgp_offset_max(), 0);
        assert!(gpu.set_ctgp_offset(10).is_err());

        ready.store(true, Ordering::SeqCst);
        assert_eq!(gpu.ctgp_offset_max(), 65);
        gpu.set_ctgp_offset(10).unwrap();
        let after_success = calls.load(Ordering::SeqCst);
        gpu.ctgp_offset_max();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            after_success,
            "cached once known"
        );
    }

    #[test]
    fn limits_retry_is_rate_limited() {
        let tmp = tempfile::tempdir().unwrap();
        setup_sysfs(tmp.path(), 0);
        let calls = Arc::new(AtomicUsize::new(0));
        let gpu = Nb02GpuPower::probe(
            tmp.path(),
            counting_query(&calls, None),
            Duration::from_secs(3600),
        )
        .unwrap();
        for _ in 0..5 {
            assert_eq!(gpu.ctgp_offset_max(), 0);
            let _ = gpu.set_ctgp_offset(1);
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "only the eager probe query"
        );
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
