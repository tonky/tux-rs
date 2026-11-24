//! GPU power control and telemetry backends.

pub mod hwmon;
pub mod intel;
pub mod nb02;
pub mod nvidia;

use std::io;
use std::path::{Path, PathBuf};

use tracing::info;
use tux_core::dbus_types::GpuData;

/// GPU power control backend trait.
pub trait GpuPowerBackend: Send + Sync {
    /// Get the current cTGP offset in watts.
    fn get_ctgp_offset(&self) -> io::Result<u8>;

    /// Set the cTGP offset in watts.
    fn set_ctgp_offset(&self, watts: u8) -> io::Result<()>;

    /// Largest offset `set_ctgp_offset` accepts; `0` means not adjustable.
    fn ctgp_offset_max(&self) -> u8;

    /// NVIDIA `(default_limit, max_limit)` in watts, when known.
    fn power_limits_w(&self) -> Option<(f64, f64)> {
        None
    }
}

/// GPU telemetry sources: hwmon (e.g. amdgpu) plus Intel i915/xe and NVIDIA
/// (NVML, runtime-PM guarded) which have no hwmon.
pub struct GpuTelemetry {
    hwmon_base: PathBuf,
    intel: Option<intel::IntelGpu>,
    nvidia: Option<nvidia::NvidiaGpu>,
}

impl GpuTelemetry {
    /// hwmon only (previous behaviour; used by tests and as a default).
    pub fn hwmon_only(hwmon_base: impl Into<PathBuf>) -> Self {
        Self {
            hwmon_base: hwmon_base.into(),
            intel: None,
            nvidia: None,
        }
    }

    /// Discover GPUs on the running system. Reads sysfs/procfs only — this
    /// never wakes a runtime-suspended dGPU.
    pub fn discover() -> Self {
        let pci_ids = Path::new("/usr/share/hwdata/pci.ids");
        let intel = intel::IntelGpu::discover(
            Path::new("/sys/class/drm"),
            Path::new("/sys/class/powercap"),
            pci_ids,
        );
        let nvidia = nvidia::NvidiaGpu::discover(
            Path::new("/sys/bus/pci/devices"),
            Path::new("/proc/driver/nvidia/gpus"),
            pci_ids,
            Box::new(nvidia::Nvml),
        );
        info!(
            "GPU telemetry: intel={} nvidia={}",
            intel.is_some(),
            nvidia.is_some()
        );
        Self {
            hwmon_base: PathBuf::from("/sys/class/hwmon"),
            intel,
            nvidia,
        }
    }

    /// Current readings for every known GPU (discrete first).
    pub fn sample(&self) -> Vec<GpuData> {
        let mut gpus: Vec<GpuData> = Vec::new();
        if let Some(ref nv) = self.nvidia {
            gpus.push(nv.sample());
        }
        gpus.extend(
            hwmon::discover_gpus(&self.hwmon_base)
                .into_iter()
                .map(GpuData::from),
        );
        if let Some(ref igpu) = self.intel {
            gpus.push(igpu.sample());
        }
        gpus
    }
}
