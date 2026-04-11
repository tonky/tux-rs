//! GPU telemetry via hwmon sysfs.
//!
//! Discovers GPU sensors from `/sys/class/hwmon/` by scanning for known
//! driver names (nvidia, i915, amdgpu).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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

/// Known hwmon driver names for GPU discovery.
const GPU_DRIVERS: &[(&str, GpuType)] = &[
    ("nvidia", GpuType::Discrete),
    ("amdgpu", GpuType::Discrete),
    ("i915", GpuType::Integrated),
    ("xe", GpuType::Integrated),
];

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

    let (driver_name, gpu_type) = GPU_DRIVERS.iter().find(|(drv, _)| *drv == name.as_str())?;

    let temperature = read_millidegree_temp(hwmon_dir);
    let power_draw_w = read_power_microwatt(hwmon_dir);

    Some(GpuInfo {
        name: driver_name.to_string(),
        temperature,
        usage_percent: None, // hwmon doesn't expose GPU usage
        power_draw_w,
        gpu_type: *gpu_type,
    })
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
    fn discover_amdgpu() {
        let tmp = tempfile::tempdir().unwrap();
        setup_hwmon(tmp.path(), "amdgpu", Some("60000"), Some("30000000"));

        let gpus = discover_gpus(tmp.path());
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].name, "amdgpu");
        assert_eq!(gpus[0].gpu_type, GpuType::Discrete);
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
}
