//! NVIDIA dGPU telemetry via NVML — without waking a runtime-suspended GPU.
//!
//! The proprietary driver exposes no hwmon. NVML (what btop and nvidia-smi
//! use) has the numbers, but any NVML query powers up a GPU that sits in
//! runtime D3, costing battery. So the PCI runtime-PM state is checked
//! first: a `suspended` GPU is reported as such and never queried.
//!
//! NVML is also initialised per query and shut down right after, so the
//! daemon holds no handle that could keep the GPU from suspending.

use std::fs;
use std::path::{Path, PathBuf};

use tux_core::dbus_types::GpuData;

const NVIDIA_VENDOR: &str = "0x10de";

/// One NVML reading.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NvSample {
    pub temperature_c: Option<f32>,
    pub usage_percent: Option<u8>,
    pub power_w: Option<f32>,
    pub clock_mhz: Option<u32>,
}

/// Source of NVML readings (real NVML, or a fake in tests).
pub trait NvQuery: Send + Sync {
    /// Query the GPU at PCI address `bdf` (e.g. `0000:01:00.0`).
    fn query(&self, bdf: &str) -> Option<NvSample>;
}

/// Real NVML, loaded at runtime (`libnvidia-ml.so.1`).
pub struct Nvml;

impl NvQuery for Nvml {
    fn query(&self, bdf: &str) -> Option<NvSample> {
        use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};
        // Dropped at the end of this call (nvmlShutdown).
        let nvml = nvml_wrapper::Nvml::init().ok()?;
        let dev = nvml.device_by_pci_bus_id(bdf).ok()?;
        Some(NvSample {
            temperature_c: dev
                .temperature(TemperatureSensor::Gpu)
                .ok()
                .map(|t| t as f32),
            usage_percent: dev.utilization_rates().ok().map(|u| u.gpu.min(100) as u8),
            power_w: dev.power_usage().ok().map(|mw| mw as f32 / 1000.0),
            clock_mhz: dev.clock_info(Clock::Graphics).ok(),
        })
    }
}

/// An NVIDIA GPU bound to the proprietary `nvidia` driver.
pub struct NvidiaGpu {
    bdf: String,
    pci_dev: PathBuf,
    name: String,
    nvml: Box<dyn NvQuery>,
}

impl NvidiaGpu {
    /// Find the first NVIDIA display device under `pci_base` (`/sys/bus/pci/devices`).
    pub fn discover(
        pci_base: &Path,
        nvidia_procfs: &Path,
        pci_ids: &Path,
        nvml: Box<dyn NvQuery>,
    ) -> Option<Self> {
        let mut devices: Vec<PathBuf> = fs::read_dir(pci_base)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .collect();
        devices.sort();
        let pci_dev = devices.into_iter().find(|d| {
            read_trim(&d.join("vendor")).as_deref() == Some(NVIDIA_VENDOR)
                && read_trim(&d.join("class")).is_some_and(|c| c.starts_with("0x03"))
                && fs::read_link(d.join("driver"))
                    .ok()
                    .and_then(|p| p.file_name().map(|n| n == "nvidia"))
                    .unwrap_or(false)
        })?;
        let bdf = pci_dev.file_name()?.to_str()?.to_string();
        let device_id = read_trim(&pci_dev.join("device")).unwrap_or_default();
        let name = procfs_model(&nvidia_procfs.join(&bdf).join("information"))
            .or_else(|| {
                super::intel::pci_ids_name(pci_ids, "10de", device_id.trim_start_matches("0x"))
                    .map(|n| format!("NVIDIA {n}"))
            })
            .unwrap_or_else(|| "NVIDIA GPU".to_string());
        Some(Self {
            bdf,
            pci_dev,
            name,
            nvml,
        })
    }

    /// PCI runtime-PM state (`active`, `suspended`, …); `active` if unknown.
    fn runtime_status(&self) -> String {
        read_trim(&self.pci_dev.join("power/runtime_status")).unwrap_or_else(|| "active".into())
    }

    /// Report telemetry, querying NVML only if the GPU is already awake.
    pub fn sample(&self) -> GpuData {
        let state = self.runtime_status();
        let s = if state == "active" {
            self.nvml.query(&self.bdf).unwrap_or_default()
        } else {
            NvSample::default()
        };
        GpuData {
            name: self.name.clone(),
            temperature: s.temperature_c,
            power_draw_w: s.power_w,
            usage_percent: s.usage_percent,
            gpu_type: "discrete".to_string(),
            power_state: Some(state),
            freq_mhz: s.clock_mhz,
        }
    }
}

fn read_trim(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// `Model:` line of `/proc/driver/nvidia/gpus/<bdf>/information` (no GPU wake-up).
fn procfs_model(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("Model:"))
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeNvml {
        calls: Arc<AtomicUsize>,
        sample: NvSample,
    }

    impl NvQuery for FakeNvml {
        fn query(&self, bdf: &str) -> Option<NvSample> {
            assert_eq!(bdf, "0000:01:00.0");
            self.calls.fetch_add(1, Ordering::SeqCst);
            Some(self.sample)
        }
    }

    fn fixture(status: &str) -> (tempfile::TempDir, NvidiaGpu, Arc<AtomicUsize>) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let dev = root.join("pci/0000:01:00.0");
        fs::create_dir_all(dev.join("power")).unwrap();
        fs::create_dir_all(root.join("drivers/nvidia")).unwrap();
        std::os::unix::fs::symlink(root.join("drivers/nvidia"), dev.join("driver")).unwrap();
        fs::write(dev.join("vendor"), "0x10de\n").unwrap();
        fs::write(dev.join("device"), "0x2d19\n").unwrap();
        fs::write(dev.join("class"), "0x030000\n").unwrap();
        fs::write(dev.join("power/runtime_status"), format!("{status}\n")).unwrap();
        // An unrelated NVIDIA function (HDA audio) must be skipped.
        let audio = root.join("pci/0000:01:00.1");
        fs::create_dir_all(&audio).unwrap();
        fs::write(audio.join("vendor"), "0x10de\n").unwrap();
        fs::write(audio.join("class"), "0x040300\n").unwrap();
        let proc_gpu = root.join("proc/0000:01:00.0");
        fs::create_dir_all(&proc_gpu).unwrap();
        fs::write(
            proc_gpu.join("information"),
            "Model: \t\t NVIDIA GeForce RTX 5060 Laptop GPU\nIRQ:   16\n",
        )
        .unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let nvml = FakeNvml {
            calls: calls.clone(),
            sample: NvSample {
                temperature_c: Some(48.0),
                usage_percent: Some(37),
                power_w: Some(21.5),
                clock_mhz: Some(1890),
            },
        };
        let gpu = NvidiaGpu::discover(
            &root.join("pci"),
            &root.join("proc"),
            &root.join("pci.ids"),
            Box::new(nvml),
        )
        .expect("dGPU discovered");
        (tmp, gpu, calls)
    }

    #[test]
    fn suspended_gpu_is_never_queried() {
        // Battery: querying NVML would wake the dGPU from runtime D3.
        let (_tmp, gpu, calls) = fixture("suspended");
        for _ in 0..3 {
            let d = gpu.sample();
            assert_eq!(d.power_state.as_deref(), Some("suspended"));
            assert_eq!(
                (d.temperature, d.usage_percent, d.power_draw_w),
                (None, None, None)
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn active_gpu_reports_nvml_values() {
        let (_tmp, gpu, calls) = fixture("active");
        let d = gpu.sample();
        assert_eq!(d.name, "NVIDIA GeForce RTX 5060 Laptop GPU");
        assert_eq!(d.gpu_type, "discrete");
        assert_eq!(d.power_state.as_deref(), Some("active"));
        assert_eq!(d.temperature, Some(48.0));
        assert_eq!(d.usage_percent, Some(37));
        assert_eq!(d.power_draw_w, Some(21.5));
        assert_eq!(d.freq_mhz, Some(1890));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn name_falls_back_to_pci_ids_without_procfs() {
        let (tmp, _, _) = fixture("suspended");
        fs::remove_file(tmp.path().join("proc/0000:01:00.0/information")).unwrap();
        fs::write(
            tmp.path().join("pci.ids"),
            "10de  NVIDIA Corporation\n\t2d19  GB206M [GeForce RTX 5060 Max-Q / Mobile]\n",
        )
        .unwrap();
        let gpu = NvidiaGpu::discover(
            &tmp.path().join("pci"),
            &tmp.path().join("proc"),
            &tmp.path().join("pci.ids"),
            Box::new(FakeNvml {
                calls: Arc::new(AtomicUsize::new(0)),
                sample: NvSample::default(),
            }),
        )
        .unwrap();
        assert_eq!(gpu.name, "NVIDIA GB206M [GeForce RTX 5060 Max-Q / Mobile]");
    }

    #[test]
    fn no_nvidia_driver_means_no_gpu() {
        let tmp = tempfile::tempdir().unwrap();
        let dev = tmp.path().join("pci/0000:01:00.0");
        fs::create_dir_all(&dev).unwrap();
        fs::write(dev.join("vendor"), "0x10de\n").unwrap();
        fs::write(dev.join("class"), "0x030000\n").unwrap();
        // No driver symlink (e.g. nouveau unbound / vfio).
        assert!(
            NvidiaGpu::discover(
                &tmp.path().join("pci"),
                tmp.path(),
                tmp.path(),
                Box::new(Nvml),
            )
            .is_none()
        );
    }
}
