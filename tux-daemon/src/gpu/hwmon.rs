//! GPU telemetry via hwmon sysfs.
//!
//! Discovers GPU sensors from `/sys/class/hwmon/` by scanning for known
//! driver names (nvidia, i915, amdgpu, xe). For `amdgpu` the integrated-vs-
//! discrete classification is resolved at runtime via the kernel's
//! `boot_vga` flag on the hwmon's parent PCI device — the static driver
//! name alone cannot tell an APU iGPU apart from a discrete Radeon card.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tux_core::dbus_types::GpuData;

/// GPU information from hwmon sensors.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpuInfo {
    pub name: String,
    pub temperature: Option<f32>,
    pub usage_percent: Option<u8>,
    pub power_draw_w: Option<f32>,
    pub gpu_type: GpuType,
}

/// Type of GPU.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GpuType {
    Discrete,
    Integrated,
}

impl GpuType {
    /// Wire string used in the `GpuData.gpu_type` field over D-Bus.
    pub fn as_wire_str(&self) -> &'static str {
        match self {
            GpuType::Discrete => "discrete",
            GpuType::Integrated => "integrated",
        }
    }
}

impl From<GpuInfo> for GpuData {
    fn from(info: GpuInfo) -> Self {
        GpuData {
            name: info.name,
            temperature: info.temperature,
            power_draw_w: info.power_draw_w,
            usage_percent: info.usage_percent,
            gpu_type: info.gpu_type.as_wire_str().to_string(),
        }
    }
}

/// Discover GPU sensors from hwmon sysfs.
pub fn discover_gpus(hwmon_base: &Path) -> Vec<GpuInfo> {
    let entries = match fs::read_dir(hwmon_base) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut gpus = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(info) = read_gpu_from_hwmon(&path) {
            gpus.push(info);
        }
    }
    gpus
}

fn read_gpu_from_hwmon(hwmon_dir: &Path) -> Option<GpuInfo> {
    let name = read_trimmed(hwmon_dir.join("name")).ok()?;

    let (driver_name, gpu_type) = classify_gpu(&name, hwmon_dir)?;

    let temperature = read_millidegree_temp(hwmon_dir);
    let power_draw_w = read_power_microwatt(hwmon_dir);

    Some(GpuInfo {
        name: driver_name,
        temperature,
        usage_percent: None, // hwmon doesn't expose GPU usage
        power_draw_w,
        gpu_type,
    })
}

/// Classify a hwmon entry by driver name.
///
/// Returns `None` for non-GPU drivers. For `amdgpu` the classification is
/// resolved at runtime via `boot_vga`: a single `amdgpu` device on an APU
/// laptop is the integrated Radeon (`boot_vga = 1`), while a dGPU sibling
/// to an Intel/AMD iGPU has `boot_vga = 0`.
fn classify_gpu(driver_name: &str, hwmon_dir: &Path) -> Option<(String, GpuType)> {
    let gpu_type = match driver_name {
        "nvidia" => GpuType::Discrete,
        "i915" | "xe" => GpuType::Integrated,
        "amdgpu" => match read_boot_vga(hwmon_dir) {
            Some(true) => GpuType::Integrated,
            // `Some(false)` (explicit non-primary) and `None` (older kernels
            // without `boot_vga`, or hwmon not associated with a PCI device)
            // both fall back to the legacy `Discrete` classification.
            _ => GpuType::Discrete,
        },
        _ => return None,
    };
    Some((driver_name.to_string(), gpu_type))
}

/// Read `boot_vga` (`'0'` / `'1'`) from the hwmon's parent PCI device.
///
/// In real sysfs, `<hwmon>/device` is a symlink that points at the PCI
/// device, which exposes `boot_vga` directly. Tests model this as a real
/// `device/` subdirectory.
fn read_boot_vga(hwmon_dir: &Path) -> Option<bool> {
    let raw = read_trimmed(hwmon_dir.join("device/boot_vga")).ok()?;
    Some(raw == "1")
}

/// Read temp1_input (millidegrees C) → degrees C.
fn read_millidegree_temp(hwmon_dir: &Path) -> Option<f32> {
    let raw = read_trimmed(hwmon_dir.join("temp1_input")).ok()?;
    let millideg: i64 = raw.parse().ok()?;
    Some(millideg as f32 / 1000.0)
}

/// Read power1_input (microwatts) → watts.
fn read_power_microwatt(hwmon_dir: &Path) -> Option<f32> {
    let raw = read_trimmed(hwmon_dir.join("power1_input")).ok()?;
    let microwatt: u64 = raw.parse().ok()?;
    Some(microwatt as f32 / 1_000_000.0)
}

fn read_trimmed(path: PathBuf) -> io::Result<String> {
    fs::read_to_string(&path).map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_hwmon(dir: &Path, name: &str, temp: Option<&str>, power: Option<&str>) -> PathBuf {
        let hwmon = dir.join("hwmon0");
        fs::create_dir_all(&hwmon).unwrap();
        fs::write(hwmon.join("name"), format!("{name}\n")).unwrap();
        if let Some(t) = temp {
            fs::write(hwmon.join("temp1_input"), format!("{t}\n")).unwrap();
        }
        if let Some(p) = power {
            fs::write(hwmon.join("power1_input"), format!("{p}\n")).unwrap();
        }
        dir.to_path_buf()
    }

    /// Create an additional hwmon directory at `hwmon{idx}`.
    fn add_hwmon(dir: &Path, idx: usize, name: &str, temp: Option<&str>) -> PathBuf {
        let hwmon = dir.join(format!("hwmon{idx}"));
        fs::create_dir_all(&hwmon).unwrap();
        fs::write(hwmon.join("name"), format!("{name}\n")).unwrap();
        if let Some(t) = temp {
            fs::write(hwmon.join("temp1_input"), format!("{t}\n")).unwrap();
        }
        hwmon
    }

    /// Write `boot_vga` under `<hwmon>/device/`.
    fn write_boot_vga(hwmon: &Path, value: &str) {
        fs::create_dir_all(hwmon.join("device")).unwrap();
        fs::write(hwmon.join("device/boot_vga"), format!("{value}\n")).unwrap();
    }

    #[test]
    fn discover_nvidia_gpu() {
        let tmp = tempfile::tempdir().unwrap();
        // 45000 millidegrees = 45.0°C, 25500000 microwatts = 25.5W
        setup_hwmon(tmp.path(), "nvidia", Some("45000"), Some("25500000"));

        let gpus = discover_gpus(tmp.path());
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].name, "nvidia");
        assert_eq!(gpus[0].gpu_type, GpuType::Discrete);
        assert!((gpus[0].temperature.unwrap() - 45.0).abs() < 0.01);
        assert!((gpus[0].power_draw_w.unwrap() - 25.5).abs() < 0.01);
        assert!(gpus[0].usage_percent.is_none());
    }

    #[test]
    fn discover_intel_igpu() {
        let tmp = tempfile::tempdir().unwrap();
        setup_hwmon(tmp.path(), "i915", Some("52000"), None);

        let gpus = discover_gpus(tmp.path());
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].name, "i915");
        assert_eq!(gpus[0].gpu_type, GpuType::Integrated);
        assert!((gpus[0].temperature.unwrap() - 52.0).abs() < 0.01);
        assert!(gpus[0].power_draw_w.is_none());
    }

    #[test]
    fn discover_intel_igpu_xe() {
        let tmp = tempfile::tempdir().unwrap();
        setup_hwmon(tmp.path(), "xe", Some("48000"), None);

        let gpus = discover_gpus(tmp.path());
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].name, "xe");
        assert_eq!(gpus[0].gpu_type, GpuType::Integrated);
    }

    #[test]
    fn discover_amdgpu_apu_classified_integrated() {
        let tmp = tempfile::tempdir().unwrap();
        setup_hwmon(tmp.path(), "amdgpu", Some("60000"), Some("30000000"));
        write_boot_vga(&tmp.path().join("hwmon0"), "1");

        let gpus = discover_gpus(tmp.path());
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].name, "amdgpu");
        assert_eq!(
            gpus[0].gpu_type,
            GpuType::Integrated,
            "amdgpu with boot_vga=1 is the primary VGA → APU iGPU"
        );
    }

    #[test]
    fn discover_amdgpu_dgpu_classified_discrete() {
        let tmp = tempfile::tempdir().unwrap();
        setup_hwmon(tmp.path(), "amdgpu", Some("70000"), Some("80000000"));
        write_boot_vga(&tmp.path().join("hwmon0"), "0");

        let gpus = discover_gpus(tmp.path());
        assert_eq!(gpus.len(), 1);
        assert_eq!(
            gpus[0].gpu_type,
            GpuType::Discrete,
            "amdgpu with boot_vga=0 is a non-primary discrete card"
        );
    }

    #[test]
    fn discover_amdgpu_missing_boot_vga_falls_back_to_discrete() {
        // Pre-existing behaviour for older kernels (or hwmons not bound to a
        // PCI device): when boot_vga can't be read, treat amdgpu as discrete.
        let tmp = tempfile::tempdir().unwrap();
        setup_hwmon(tmp.path(), "amdgpu", Some("60000"), Some("30000000"));

        let gpus = discover_gpus(tmp.path());
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].gpu_type, GpuType::Discrete);
    }

    #[test]
    fn discover_hybrid_amd_apu_plus_nvidia_dgpu() {
        let tmp = tempfile::tempdir().unwrap();
        let amd = add_hwmon(tmp.path(), 0, "amdgpu", Some("55000"));
        write_boot_vga(&amd, "1");
        add_hwmon(tmp.path(), 1, "nvidia", Some("70000"));

        let mut gpus = discover_gpus(tmp.path());
        gpus.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].name, "amdgpu");
        assert_eq!(gpus[0].gpu_type, GpuType::Integrated);
        assert_eq!(gpus[1].name, "nvidia");
        assert_eq!(gpus[1].gpu_type, GpuType::Discrete);
    }

    #[test]
    fn discover_hybrid_intel_igpu_plus_nvidia_dgpu() {
        let tmp = tempfile::tempdir().unwrap();
        add_hwmon(tmp.path(), 0, "i915", Some("50000"));
        add_hwmon(tmp.path(), 1, "nvidia", Some("75000"));

        let mut gpus = discover_gpus(tmp.path());
        gpus.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].name, "i915");
        assert_eq!(gpus[0].gpu_type, GpuType::Integrated);
        assert_eq!(gpus[1].name, "nvidia");
        assert_eq!(gpus[1].gpu_type, GpuType::Discrete);
    }

    #[test]
    fn no_gpu_present_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        // Non-GPU hwmon device
        setup_hwmon(tmp.path(), "coretemp", Some("75000"), None);

        let gpus = discover_gpus(tmp.path());
        assert!(gpus.is_empty());
    }

    #[test]
    fn missing_sensors_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        let hwmon = tmp.path().join("hwmon0");
        fs::create_dir_all(&hwmon).unwrap();
        fs::write(hwmon.join("name"), "nvidia\n").unwrap();
        // No temp or power files

        let gpus = discover_gpus(tmp.path());
        assert_eq!(gpus.len(), 1);
        assert!(gpus[0].temperature.is_none());
        assert!(gpus[0].power_draw_w.is_none());
    }

    #[test]
    fn multiple_gpus_discovered() {
        let tmp = tempfile::tempdir().unwrap();

        let nvidia = tmp.path().join("hwmon0");
        fs::create_dir_all(&nvidia).unwrap();
        fs::write(nvidia.join("name"), "nvidia\n").unwrap();
        fs::write(nvidia.join("temp1_input"), "70000\n").unwrap();

        let intel = tmp.path().join("hwmon1");
        fs::create_dir_all(&intel).unwrap();
        fs::write(intel.join("name"), "i915\n").unwrap();
        fs::write(intel.join("temp1_input"), "50000\n").unwrap();

        let gpus = discover_gpus(tmp.path());
        assert_eq!(gpus.len(), 2);
    }

    #[test]
    fn missing_hwmon_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let nonexistent = tmp.path().join("does_not_exist");

        let gpus = discover_gpus(&nonexistent);
        assert!(gpus.is_empty());
    }

    #[test]
    fn invalid_temp_value_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        let hwmon = tmp.path().join("hwmon0");
        fs::create_dir_all(&hwmon).unwrap();
        fs::write(hwmon.join("name"), "nvidia\n").unwrap();
        fs::write(hwmon.join("temp1_input"), "not_a_number\n").unwrap();

        let gpus = discover_gpus(tmp.path());
        assert_eq!(gpus.len(), 1);
        assert!(gpus[0].temperature.is_none());
    }

    #[test]
    fn gpu_data_conversion_carries_wire_strings() {
        let info = GpuInfo {
            name: "amdgpu".into(),
            temperature: Some(55.0),
            usage_percent: None,
            power_draw_w: Some(12.5),
            gpu_type: GpuType::Integrated,
        };
        let data: GpuData = info.into();
        assert_eq!(data.name, "amdgpu");
        assert_eq!(data.gpu_type, "integrated");
        assert_eq!(data.temperature, Some(55.0));
        assert_eq!(data.power_draw_w, Some(12.5));
    }
}
