//! Intel iGPU telemetry from i915 / xe sysfs (no hwmon on integrated GPUs).
//!
//! - busy %: inverse of the RC6 (idle) residency counter over the sample
//!   interval — what `intel_gpu_top` calls "RC6".
//! - clock: actual GT frequency.
//! - power: RAPL `uncore` domain energy (≈ iGPU on client CPUs).
//!
//! Rates need two samples, so the first call after start reports only the
//! clock. Everything here is plain sysfs; nothing wakes other devices.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use tux_core::dbus_types::GpuData;

const INTEL_VENDOR: &str = "0x8086";

/// Per-driver sysfs locations (relative to `/sys/class/drm/cardN`).
#[derive(Debug, Clone)]
struct GtPaths {
    /// Idle (RC6) residency in milliseconds.
    idle_ms: PathBuf,
    /// Actual GT frequency in MHz.
    freq_mhz: PathBuf,
}

impl GtPaths {
    fn for_card(card: &Path, driver: &str) -> Option<Self> {
        let candidates: &[(&str, &str)] = match driver {
            "i915" => &[
                ("gt/gt0/rc6_residency_ms", "gt/gt0/rps_act_freq_mhz"),
                ("power/rc6_residency_ms", "gt_act_freq_mhz"),
            ],
            "xe" => &[(
                "device/tile0/gt0/gtidle/idle_residency_ms",
                "device/tile0/gt0/freq0/act_freq",
            )],
            _ => &[],
        };
        candidates.iter().find_map(|(idle, freq)| {
            let (idle, freq) = (card.join(idle), card.join(freq));
            (idle.exists() && freq.exists()).then_some(Self {
                idle_ms: idle,
                freq_mhz: freq,
            })
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    at: Instant,
    idle_ms: u64,
    energy_uj: Option<u64>,
}

/// Samples one Intel integrated GPU.
#[derive(Debug)]
pub struct IntelGpu {
    name: String,
    paths: GtPaths,
    /// RAPL `uncore` energy counter (`energy_uj`), if present.
    uncore_energy: Option<PathBuf>,
    last: Mutex<Option<Sample>>,
}

impl IntelGpu {
    /// Find the Intel iGPU among `drm_base/cardN` (driver i915 or xe).
    pub fn discover(drm_base: &Path, powercap_base: &Path, pci_ids: &Path) -> Option<Self> {
        let mut cards: Vec<PathBuf> = fs::read_dir(drm_base)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("card") && !n.contains('-'))
            })
            .collect();
        cards.sort();
        cards.into_iter().find_map(|card| {
            let device = card.join("device");
            if read_trim(&device.join("vendor")).as_deref() != Some(INTEL_VENDOR) {
                return None;
            }
            let driver = fs::read_link(device.join("driver")).ok()?;
            let driver = driver.file_name()?.to_str()?.to_string();
            let paths = GtPaths::for_card(&card, &driver)?;
            let device_id = read_trim(&device.join("device")).unwrap_or_default();
            let name = pci_ids_name(pci_ids, "8086", device_id.trim_start_matches("0x"))
                .map(|n| format!("Intel {n}"))
                .unwrap_or_else(|| "Intel Graphics".to_string());
            Some(Self {
                name,
                paths,
                uncore_energy: find_rapl_uncore(powercap_base),
                last: Mutex::new(None),
            })
        })
    }

    /// Take a sample and report telemetry (rates from the previous sample).
    pub fn sample(&self) -> GpuData {
        self.sample_at(Instant::now())
    }

    fn sample_at(&self, now: Instant) -> GpuData {
        let freq_mhz = read_u64(&self.paths.freq_mhz).and_then(|f| u32::try_from(f).ok());
        let current = read_u64(&self.paths.idle_ms).map(|idle_ms| Sample {
            at: now,
            idle_ms,
            energy_uj: self.uncore_energy.as_deref().and_then(read_u64),
        });

        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        let (mut usage_percent, mut power_draw_w) = (None, None);
        if let (Some(prev), Some(cur)) = (*last, current) {
            let dt = cur.at.saturating_duration_since(prev.at);
            let dt_ms = dt.as_secs_f64() * 1000.0;
            if dt_ms >= 1.0 && cur.idle_ms >= prev.idle_ms {
                let idle = (cur.idle_ms - prev.idle_ms) as f64 / dt_ms;
                usage_percent = Some(((1.0 - idle.min(1.0)) * 100.0).round() as u8);
            }
            if let (Some(e0), Some(e1)) = (prev.energy_uj, cur.energy_uj)
                && e1 >= e0
                && dt_ms >= 1.0
            {
                power_draw_w = Some(((e1 - e0) as f64 / (dt_ms * 1000.0)) as f32);
            }
        }
        if current.is_some() {
            *last = current;
        }

        GpuData {
            name: self.name.clone(),
            temperature: None,
            power_draw_w,
            usage_percent,
            gpu_type: "integrated".to_string(),
            power_state: None,
            freq_mhz,
        }
    }
}

fn read_trim(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn read_u64(path: &Path) -> Option<u64> {
    read_trim(path)?.parse().ok()
}

/// RAPL `uncore` subdomain energy counter under `powercap_base`.
fn find_rapl_uncore(powercap_base: &Path) -> Option<PathBuf> {
    let mut zones: Vec<PathBuf> = fs::read_dir(powercap_base)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .collect();
    zones.sort();
    zones.into_iter().find_map(|zone| {
        (read_trim(&zone.join("name")).as_deref() == Some("uncore"))
            .then(|| zone.join("energy_uj"))
            .filter(|p| p.exists())
    })
}

/// Look up a device name in a `pci.ids` database (`vendor`/`device` in hex).
pub(crate) fn pci_ids_name(pci_ids: &Path, vendor: &str, device: &str) -> Option<String> {
    let db = fs::read_to_string(pci_ids).ok()?;
    let mut in_vendor = false;
    for line in db.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if !line.starts_with('\t') {
            in_vendor = line
                .split_whitespace()
                .next()
                .is_some_and(|v| v.eq_ignore_ascii_case(vendor));
            continue;
        }
        if in_vendor && !line.starts_with("\t\t") {
            let entry = line.trim_start_matches('\t');
            if let Some((id, name)) = entry.split_once(char::is_whitespace)
                && id.eq_ignore_ascii_case(device)
            {
                return Some(name.trim().to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const PCI_IDS: &str = "\
# comment
8086  Intel Corporation
\t7d45  Meteor Lake-P [Intel Graphics]
\t7d51  Arrow Lake-P [Arc Pro 130T/140T]
\t\t1d72 2501  Some subsystem
10de  NVIDIA Corporation
\t2d19  GB206M [GeForce RTX 5060 Max-Q / Mobile]
";

    /// Builds `drm/card1` (i915) + `powercap/intel-rapl:0:1` (uncore).
    fn fixture(idle_ms: u64, energy_uj: u64) -> (tempfile::TempDir, IntelGpu) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let pci_dev = root.join("devices/0000:00:02.0");
        let drv = root.join("drivers/i915");
        fs::create_dir_all(&pci_dev).unwrap();
        fs::create_dir_all(&drv).unwrap();
        fs::write(pci_dev.join("vendor"), "0x8086\n").unwrap();
        fs::write(pci_dev.join("device"), "0x7d51\n").unwrap();
        std::os::unix::fs::symlink(&drv, pci_dev.join("driver")).unwrap();
        let card = root.join("drm/card1");
        fs::create_dir_all(card.join("gt/gt0")).unwrap();
        std::os::unix::fs::symlink(&pci_dev, card.join("device")).unwrap();
        fs::create_dir_all(root.join("drm/card1-eDP-1")).unwrap();
        fs::write(card.join("gt/gt0/rc6_residency_ms"), format!("{idle_ms}\n")).unwrap();
        fs::write(card.join("gt/gt0/rps_act_freq_mhz"), "800\n").unwrap();
        let zone = root.join("powercap/intel-rapl:0:1");
        fs::create_dir_all(&zone).unwrap();
        fs::write(zone.join("name"), "uncore\n").unwrap();
        fs::write(zone.join("energy_uj"), format!("{energy_uj}\n")).unwrap();
        fs::write(root.join("pci.ids"), PCI_IDS).unwrap();
        let gpu = IntelGpu::discover(
            &root.join("drm"),
            &root.join("powercap"),
            &root.join("pci.ids"),
        )
        .expect("iGPU discovered");
        (tmp, gpu)
    }

    fn set(tmp: &tempfile::TempDir, rel: &str, v: u64) {
        fs::write(tmp.path().join(rel), format!("{v}\n")).unwrap();
    }

    #[test]
    fn discovers_i915_card_with_pci_ids_name() {
        let (_tmp, gpu) = fixture(0, 0);
        assert_eq!(gpu.name, "Intel Arrow Lake-P [Arc Pro 130T/140T]");
        assert!(gpu.uncore_energy.is_some());
    }

    #[test]
    fn first_sample_has_clock_only() {
        let (_tmp, gpu) = fixture(1000, 5_000_000);
        let d = gpu.sample_at(Instant::now());
        assert_eq!(d.freq_mhz, Some(800));
        assert_eq!((d.usage_percent, d.power_draw_w), (None, None));
        assert_eq!(d.gpu_type, "integrated");
    }

    #[test]
    fn usage_and_power_from_deltas() {
        let (tmp, gpu) = fixture(1000, 5_000_000);
        let t0 = Instant::now();
        gpu.sample_at(t0);
        // Over 2 s: 1.5 s in RC6 (25 % busy), 3 J uncore energy (1.5 W).
        set(&tmp, "drm/card1/gt/gt0/rc6_residency_ms", 2500);
        set(&tmp, "powercap/intel-rapl:0:1/energy_uj", 8_000_000);
        let d = gpu.sample_at(t0 + Duration::from_secs(2));
        assert_eq!(d.usage_percent, Some(25));
        assert!((d.power_draw_w.unwrap() - 1.5).abs() < 1e-3);
    }

    #[test]
    fn counter_wrap_or_reset_yields_no_rate() {
        let (tmp, gpu) = fixture(5000, 9_000_000);
        let t0 = Instant::now();
        gpu.sample_at(t0);
        set(&tmp, "drm/card1/gt/gt0/rc6_residency_ms", 100);
        set(&tmp, "powercap/intel-rapl:0:1/energy_uj", 10);
        let d = gpu.sample_at(t0 + Duration::from_secs(1));
        assert_eq!((d.usage_percent, d.power_draw_w), (None, None));
    }

    #[test]
    fn idle_above_interval_clamps_to_zero_busy() {
        let (tmp, gpu) = fixture(0, 0);
        let t0 = Instant::now();
        gpu.sample_at(t0);
        set(&tmp, "drm/card1/gt/gt0/rc6_residency_ms", 1200);
        let d = gpu.sample_at(t0 + Duration::from_secs(1));
        assert_eq!(d.usage_percent, Some(0));
    }

    #[test]
    fn non_intel_cards_are_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(IntelGpu::discover(tmp.path(), tmp.path(), tmp.path()).is_none());
    }

    #[test]
    fn pci_ids_lookup_skips_subsystems_and_other_vendors() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("pci.ids");
        fs::write(&db, PCI_IDS).unwrap();
        assert_eq!(
            pci_ids_name(&db, "10de", "2d19").as_deref(),
            Some("GB206M [GeForce RTX 5060 Max-Q / Mobile]")
        );
        assert_eq!(pci_ids_name(&db, "8086", "2d19"), None);
        assert_eq!(pci_ids_name(&db, "8086", "1d72"), None, "subsystem line");
    }
}
