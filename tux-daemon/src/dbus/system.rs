//! D-Bus System interface: `com.tuxedocomputers.tccd.System`.

use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use tokio::sync::watch;
use zbus::interface;

use crate::config::ProfileAssignments;
use crate::cpu::sampler::CpuSampler;
use crate::gpu::hwmon;
use crate::power_monitor::PowerState;
use crate::profile_store::ProfileStore;
use tux_core::dbus_types::{
    BatteryInfoResponse, CpuFreqResponse, CpuLoadResponse, SystemInfoResponse,
};

/// D-Bus object implementing the System interface.
pub struct SystemInterface {
    power_rx: watch::Receiver<PowerState>,
    assignments_rx: watch::Receiver<ProfileAssignments>,
    store: Arc<RwLock<ProfileStore>>,
    cpu_sampler: Mutex<CpuSampler>,
}

impl SystemInterface {
    pub fn new(
        power_rx: watch::Receiver<PowerState>,
        assignments_rx: watch::Receiver<ProfileAssignments>,
        store: Arc<RwLock<ProfileStore>>,
    ) -> Self {
        Self {
            power_rx,
            assignments_rx,
            store,
            cpu_sampler: Mutex::new(CpuSampler::system()),
        }
    }
}

#[interface(name = "com.tuxedocomputers.tccd.System")]
impl SystemInterface {
    /// Get system information as TOML.
    fn get_system_info(&self) -> zbus::fdo::Result<String> {
        let hostname = std::fs::read_to_string("/etc/hostname")
            .map(|h| h.trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let kernel = std::fs::read_to_string("/proc/version")
            .unwrap_or_else(|_| "unknown".to_string())
            .lines()
            .next()
            .unwrap_or("unknown")
            .to_string();

        let info = SystemInfoResponse {
            version: tux_core::version().to_string(),
            hostname,
            kernel,
        };
        toml::to_string(&info).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get the current power state: "ac" or "battery".
    fn get_power_state(&self) -> &str {
        match *self.power_rx.borrow() {
            PowerState::Ac => "ac",
            PowerState::Battery => "battery",
        }
    }

    /// Get battery information as TOML.
    fn get_battery_info(&self) -> zbus::fdo::Result<String> {
        let info = read_battery_info(Path::new("/sys/class/power_supply"));
        toml::to_string(&info).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get GPU telemetry info as TOML.
    fn get_gpu_info(&self) -> zbus::fdo::Result<String> {
        let gpus = hwmon::discover_gpus(Path::new("/sys/class/hwmon"));
        toml::to_string(&gpus).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get average CPU frequency in MHz across all online cores.
    fn get_cpu_frequency(&self) -> zbus::fdo::Result<u32> {
        cpu_frequency_mhz(Path::new("/sys/devices/system/cpu"))
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get the number of online CPU cores.
    fn get_cpu_count(&self) -> zbus::fdo::Result<u32> {
        cpu_core_count(Path::new("/sys/devices/system/cpu"))
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get the active profile name for the current power state.
    fn get_active_profile_name(&self) -> zbus::fdo::Result<String> {
        let assignments = self.assignments_rx.borrow().clone();
        let power = *self.power_rx.borrow();
        let profile_id = match power {
            PowerState::Ac => &assignments.ac_profile,
            PowerState::Battery => &assignments.battery_profile,
        };
        // Look up the profile name from the store.
        let store = self
            .store
            .read()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        match store.get(profile_id) {
            Some(p) => Ok(p.name.clone()),
            None => Ok(profile_id.clone()),
        }
    }

    /// Get CPU load (overall + per-core) as TOML.
    fn get_cpu_load(&self) -> zbus::fdo::Result<String> {
        let mut sampler = self
            .cpu_sampler
            .lock()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        let snap = sampler
            .sample()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        let resp = CpuLoadResponse {
            overall: snap.overall,
            per_core: snap.per_core,
        };
        toml::to_string(&resp).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get per-core CPU frequencies in MHz as TOML.
    fn get_per_core_frequencies(&self) -> zbus::fdo::Result<String> {
        let freqs = cpu_per_core_frequencies(Path::new("/sys/devices/system/cpu"))
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        let resp = CpuFreqResponse { per_core: freqs };
        toml::to_string(&resp).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Check if Fn Lock is supported on this system.
    fn get_fn_lock_supported(&self) -> bool {
        fn_lock_supported(FN_LOCK_PATH)
    }

    /// Get the current Fn Lock status (true = locked).
    fn get_fn_lock_status(&self) -> zbus::fdo::Result<bool> {
        fn_lock_read(FN_LOCK_PATH).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Set the Fn Lock status (true = locked).
    fn set_fn_lock_status(&self, locked: bool) -> zbus::fdo::Result<()> {
        fn_lock_write(FN_LOCK_PATH, locked).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }
}

/// Sysfs path for Fn Lock attribute (from tuxedo_keyboard driver).
const FN_LOCK_PATH: &str = "/sys/devices/platform/tuxedo_keyboard/fn_lock";

/// Check if the fn_lock sysfs attribute exists.
fn fn_lock_supported(path: &str) -> bool {
    Path::new(path).exists()
}

/// Read Fn Lock status from sysfs.
fn fn_lock_read(path: &str) -> std::io::Result<bool> {
    let val = std::fs::read_to_string(path)?;
    Ok(val.trim() == "1")
}

/// Write Fn Lock status to sysfs.
fn fn_lock_write(path: &str, locked: bool) -> std::io::Result<()> {
    std::fs::write(path, if locked { "1" } else { "0" })
}

/// Read average CPU frequency in MHz from sysfs.
fn cpu_frequency_mhz(cpu_base: &Path) -> std::io::Result<u32> {
    let mut total_khz: u64 = 0;
    let mut count: u64 = 0;
    for entry in std::fs::read_dir(cpu_base)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("cpu") && name[3..].chars().all(|c| c.is_ascii_digit()) {
            let freq_path = entry.path().join("cpufreq/scaling_cur_freq");
            if let Ok(val) = std::fs::read_to_string(&freq_path)
                && let Ok(khz) = val.trim().parse::<u64>()
            {
                total_khz += khz;
                count += 1;
            }
        }
    }
    if count == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no CPU frequency data found",
        ));
    }
    Ok((total_khz / count / 1000) as u32)
}

/// Read per-core CPU frequencies in MHz from sysfs, sorted by core index.
fn cpu_per_core_frequencies(cpu_base: &Path) -> std::io::Result<Vec<u32>> {
    let mut cores: Vec<(u32, u32)> = Vec::new();
    for entry in std::fs::read_dir(cpu_base)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("cpu") && name[3..].chars().all(|c| c.is_ascii_digit()) {
            let core_index: u32 = name[3..].parse().unwrap_or(0);
            let freq_path = entry.path().join("cpufreq/scaling_cur_freq");
            if let Ok(val) = std::fs::read_to_string(&freq_path)
                && let Ok(khz) = val.trim().parse::<u64>()
            {
                cores.push((core_index, (khz / 1000) as u32));
            }
        }
    }
    if cores.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no CPU frequency data found",
        ));
    }
    cores.sort_by_key(|(idx, _)| *idx);
    Ok(cores.into_iter().map(|(_, freq)| freq).collect())
}

/// Count online CPU cores that have cpufreq directories.
fn cpu_core_count(cpu_base: &Path) -> std::io::Result<u32> {
    let mut count: u32 = 0;
    for entry in std::fs::read_dir(cpu_base)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("cpu")
            && name[3..].chars().all(|c| c.is_ascii_digit())
            && entry.path().join("cpufreq").is_dir()
        {
            count += 1;
        }
    }
    if count == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no CPU cores found",
        ));
    }
    Ok(count)
}

/// Read battery information from sysfs.
///
/// Looks for `BAT0`, `BAT1`, etc. under `/sys/class/power_supply/` and reads
/// standard ACPI battery attributes. Returns a default (not-present) response
/// if no battery is found.
fn read_battery_info(base: &Path) -> BatteryInfoResponse {
    // Find the first BAT* entry.
    let bat_path = match std::fs::read_dir(base) {
        Ok(entries) => entries.flatten().find_map(|e| {
            let name = e.file_name();
            if name.to_string_lossy().starts_with("BAT") {
                Some(e.path())
            } else {
                None
            }
        }),
        Err(_) => None,
    };
    let Some(bat) = bat_path else {
        return BatteryInfoResponse::default();
    };

    let read_str = |attr: &str| -> String {
        std::fs::read_to_string(bat.join(attr))
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    };
    let read_u32 = |attr: &str| -> u32 { read_str(attr).parse().unwrap_or(0) };
    let read_i32 = |attr: &str| -> i32 { read_str(attr).parse().unwrap_or(0) };

    // Sysfs reports in µAh / µA / µV — convert to mAh / mA / mV.
    let charge_now_mah = read_u32("charge_now") / 1000;
    let charge_full_mah = read_u32("charge_full") / 1000;
    let charge_full_design_mah = read_u32("charge_full_design") / 1000;

    let status = read_str("status");
    // current_now is unsigned in sysfs; sign it based on status.
    let current_raw = read_i32("current_now").unsigned_abs() / 1000;
    let current_now_ma = match status.as_str() {
        "Discharging" => -(current_raw as i32),
        _ => current_raw as i32,
    };

    let health_percent = if charge_full_design_mah > 0 {
        ((charge_full_mah as u64 * 100) / charge_full_design_mah as u64) as u32
    } else {
        0
    };

    BatteryInfoResponse {
        present: true,
        capacity_percent: read_u32("capacity"),
        status,
        cycle_count: {
            // Prefer BAT*/raw_cycle_count, but normalize known unstable encodings
            // observed on some firmware (e.g. 12836/13348 carrying 36 in low byte).
            // Then try tuxedo_keyboard platform raw counter, then legacy cycle_count.
            let raw = {
                let bat_raw = read_battery_raw_cycle_count(&bat);
                if bat_raw > 0 {
                    bat_raw
                } else {
                    std::fs::read_to_string("/sys/devices/platform/tuxedo_keyboard/raw_cycle_count")
                        .map(|s| s.trim().parse::<u32>().unwrap_or(0))
                        .unwrap_or(0)
                }
            };

            if raw > 0 {
                raw
            } else {
                read_u32("cycle_count")
            }
        },
        charge_now_mah,
        charge_full_mah,
        charge_full_design_mah,
        current_now_ma,
        voltage_now_mv: read_u32("voltage_now") / 1000,
        voltage_design_mv: read_u32("voltage_min_design") / 1000,
        technology: read_str("technology"),
        manufacturer: read_str("manufacturer"),
        model_name: read_str("model_name"),
        health_percent,
    }
}

/// Normalize flaky raw cycle counters seen on some Uniwill firmware.
///
/// Some systems occasionally return a 16-bit packed value where the real
/// cycle count is in the low byte (e.g. 12836 -> 36, 13348 -> 36).
fn normalize_raw_cycle_count(raw: u32) -> u32 {
    if raw == 0 {
        return 0;
    }
    if raw > u8::MAX as u32 {
        let low = raw & 0xFF;
        if low > 0 {
            return low;
        }
    }
    raw
}

/// Read BAT*/raw_cycle_count multiple times and return a stable candidate.
///
/// We take the minimum non-zero normalized value from a short burst to reject
/// transient high spikes while keeping monotonically increasing real counts.
fn read_battery_raw_cycle_count(bat_path: &Path) -> u32 {
    let mut best: Option<u32> = None;
    for _ in 0..5 {
        let sample = std::fs::read_to_string(bat_path.join("raw_cycle_count"))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .map(normalize_raw_cycle_count)
            .unwrap_or(0);
        if sample > 0 {
            best = Some(best.map_or(sample, |cur| cur.min(sample)));
        }
    }
    best.unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_store::ProfileStore;
    use std::fs;

    fn make_test_iface() -> SystemInterface {
        let (_, power_rx) = watch::channel(PowerState::Ac);
        let (_, assignments_rx) = watch::channel(ProfileAssignments::default());
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(RwLock::new(ProfileStore::new(dir.path()).unwrap()));
        SystemInterface::new(power_rx, assignments_rx, store)
    }

    fn make_test_iface_with_store(
        dir: &Path,
    ) -> (SystemInterface, watch::Sender<ProfileAssignments>) {
        let (_, power_rx) = watch::channel(PowerState::Ac);
        let (assignments_tx, assignments_rx) = watch::channel(ProfileAssignments::default());
        let store = Arc::new(RwLock::new(ProfileStore::new(dir).unwrap()));
        (
            SystemInterface::new(power_rx, assignments_rx, store),
            assignments_tx,
        )
    }

    #[test]
    fn get_system_info_contains_version() {
        let iface = make_test_iface();
        let info = iface.get_system_info().unwrap();
        assert!(info.contains(&tux_core::version().to_string()));
    }

    #[test]
    fn get_power_state_ac() {
        let (_, power_rx) = watch::channel(PowerState::Ac);
        let (_, assignments_rx) = watch::channel(ProfileAssignments::default());
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(RwLock::new(ProfileStore::new(dir.path()).unwrap()));
        let iface = SystemInterface::new(power_rx, assignments_rx, store);
        assert_eq!(iface.get_power_state(), "ac");
    }

    #[test]
    fn get_power_state_battery() {
        let (_, power_rx) = watch::channel(PowerState::Battery);
        let (_, assignments_rx) = watch::channel(ProfileAssignments::default());
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(RwLock::new(ProfileStore::new(dir.path()).unwrap()));
        let iface = SystemInterface::new(power_rx, assignments_rx, store);
        assert_eq!(iface.get_power_state(), "battery");
    }

    #[test]
    fn cpu_frequency_from_sysfs() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        for i in 0..4 {
            let cpufreq = base.join(format!("cpu{i}/cpufreq"));
            fs::create_dir_all(&cpufreq).unwrap();
            // 2400 MHz = 2400000 kHz
            fs::write(cpufreq.join("scaling_cur_freq"), "2400000\n").unwrap();
        }
        let freq = cpu_frequency_mhz(base).unwrap();
        assert_eq!(freq, 2400);
    }

    #[test]
    fn cpu_frequency_mixed_speeds() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let freqs = [1000000u64, 3000000]; // 1000 + 3000 avg = 2000
        for (i, khz) in freqs.iter().enumerate() {
            let cpufreq = base.join(format!("cpu{i}/cpufreq"));
            fs::create_dir_all(&cpufreq).unwrap();
            fs::write(cpufreq.join("scaling_cur_freq"), format!("{khz}\n")).unwrap();
        }
        let freq = cpu_frequency_mhz(base).unwrap();
        assert_eq!(freq, 2000);
    }

    #[test]
    fn cpu_frequency_no_cpus() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(cpu_frequency_mhz(tmp.path()).is_err());
    }

    #[test]
    fn cpu_core_count_from_sysfs() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        for i in 0..8 {
            let cpufreq = base.join(format!("cpu{i}/cpufreq"));
            fs::create_dir_all(&cpufreq).unwrap();
        }
        let count = cpu_core_count(base).unwrap();
        assert_eq!(count, 8);
    }

    #[test]
    fn cpu_core_count_ignores_non_cpu_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        // Create 2 real CPUs
        for i in 0..2 {
            let cpufreq = base.join(format!("cpu{i}/cpufreq"));
            fs::create_dir_all(&cpufreq).unwrap();
        }
        // Create non-CPU dirs that should be ignored
        fs::create_dir_all(base.join("cpufreq")).unwrap();
        fs::create_dir_all(base.join("cpuidle")).unwrap();
        let count = cpu_core_count(base).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn get_active_profile_name_default() {
        let dir = tempfile::tempdir().unwrap();
        let (iface, _) = make_test_iface_with_store(dir.path());
        // Default AC assignment is __office__
        let name = iface.get_active_profile_name().unwrap();
        assert_eq!(name, "Office");
    }

    #[test]
    fn fn_lock_supported_false_when_missing() {
        assert!(!fn_lock_supported("/nonexistent/path/fn_lock"));
    }

    #[test]
    fn fn_lock_read_write() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("fn_lock");
        fs::write(&path, "0").unwrap();
        let path_str = path.to_str().unwrap();
        assert!(!fn_lock_read(path_str).unwrap());

        fn_lock_write(path_str, true).unwrap();
        assert!(fn_lock_read(path_str).unwrap());

        fn_lock_write(path_str, false).unwrap();
        assert!(!fn_lock_read(path_str).unwrap());
    }

    #[test]
    fn fn_lock_supported_when_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("fn_lock");
        fs::write(&path, "0").unwrap();
        assert!(fn_lock_supported(path.to_str().unwrap()));
    }

    #[test]
    fn per_core_frequencies_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        // Create cores out of order to verify sorting
        let freqs_khz = [(2, 3000000u64), (0, 1000000), (1, 2000000), (3, 4000000)];
        for (i, khz) in &freqs_khz {
            let cpufreq = base.join(format!("cpu{i}/cpufreq"));
            fs::create_dir_all(&cpufreq).unwrap();
            fs::write(cpufreq.join("scaling_cur_freq"), format!("{khz}\n")).unwrap();
        }
        let freqs = cpu_per_core_frequencies(base).unwrap();
        assert_eq!(freqs, vec![1000, 2000, 3000, 4000]);
    }

    #[test]
    fn per_core_frequencies_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(cpu_per_core_frequencies(tmp.path()).is_err());
    }

    #[test]
    fn battery_info_from_sysfs() {
        let tmp = tempfile::tempdir().unwrap();
        let bat = tmp.path().join("BAT0");
        fs::create_dir_all(&bat).unwrap();

        fs::write(bat.join("capacity"), "85\n").unwrap();
        fs::write(bat.join("status"), "Discharging\n").unwrap();
        fs::write(bat.join("cycle_count"), "35\n").unwrap();
        // raw_cycle_count would be at /sys/devices/platform/tuxedo_keyboard/ on real hardware;
        // in tests the platform path is absent so cycle_count is the fallback.
        // Values in µAh / µA / µV as per sysfs convention
        fs::write(bat.join("charge_now"), "4200000\n").unwrap();
        fs::write(bat.join("charge_full"), "5000000\n").unwrap();
        fs::write(bat.join("charge_full_design"), "5300000\n").unwrap();
        fs::write(bat.join("current_now"), "1500000\n").unwrap();
        fs::write(bat.join("voltage_now"), "15800000\n").unwrap();
        fs::write(bat.join("voltage_min_design"), "15480000\n").unwrap();
        fs::write(bat.join("technology"), "Li-ion\n").unwrap();
        fs::write(bat.join("manufacturer"), "OEM\n").unwrap();
        fs::write(bat.join("model_name"), "standard\n").unwrap();

        let info = read_battery_info(tmp.path());
        assert!(info.present);
        assert_eq!(info.capacity_percent, 85);
        assert_eq!(info.status, "Discharging");
        assert_eq!(info.cycle_count, 35); // Falls back to cycle_count in test (no platform device)
        assert_eq!(info.charge_now_mah, 4200);
        assert_eq!(info.charge_full_mah, 5000);
        assert_eq!(info.charge_full_design_mah, 5300);
        assert_eq!(info.current_now_ma, -1500); // Negative when discharging
        assert_eq!(info.voltage_now_mv, 15800);
        assert_eq!(info.voltage_design_mv, 15480);
        assert_eq!(info.technology, "Li-ion");
        assert_eq!(info.manufacturer, "OEM");
        assert_eq!(info.health_percent, 94); // 5000/5300 ≈ 94%
    }

    #[test]
    fn battery_info_no_battery() {
        let tmp = tempfile::tempdir().unwrap();
        // No BAT* directory → not present
        let info = read_battery_info(tmp.path());
        assert!(!info.present);
    }

    #[test]
    fn battery_info_charging_positive_current() {
        let tmp = tempfile::tempdir().unwrap();
        let bat = tmp.path().join("BAT0");
        fs::create_dir_all(&bat).unwrap();

        fs::write(bat.join("capacity"), "60\n").unwrap();
        fs::write(bat.join("status"), "Charging\n").unwrap();
        fs::write(bat.join("cycle_count"), "0\n").unwrap();
        fs::write(bat.join("charge_now"), "3000000\n").unwrap();
        fs::write(bat.join("charge_full"), "5000000\n").unwrap();
        fs::write(bat.join("charge_full_design"), "5000000\n").unwrap();
        fs::write(bat.join("current_now"), "2000000\n").unwrap();
        fs::write(bat.join("voltage_now"), "16000000\n").unwrap();
        fs::write(bat.join("voltage_min_design"), "15000000\n").unwrap();
        fs::write(bat.join("technology"), "Li-ion\n").unwrap();
        fs::write(bat.join("manufacturer"), "OEM\n").unwrap();
        fs::write(bat.join("model_name"), "standard\n").unwrap();

        let info = read_battery_info(tmp.path());
        assert!(info.present);
        assert_eq!(info.current_now_ma, 2000); // Positive when charging
        assert_eq!(info.health_percent, 100);
    }

    #[test]
    fn battery_info_cycle_count_fallback() {
        // raw_cycle_count is at /sys/devices/platform/tuxedo_keyboard/ on real hardware;
        // in tests that path is absent, so cycle_count is the fallback.
        let tmp = tempfile::tempdir().unwrap();
        let bat = tmp.path().join("BAT0");
        fs::create_dir_all(&bat).unwrap();

        fs::write(bat.join("capacity"), "90\n").unwrap();
        fs::write(bat.join("status"), "Full\n").unwrap();
        fs::write(bat.join("cycle_count"), "42\n").unwrap();
        fs::write(bat.join("charge_now"), "5000000\n").unwrap();
        fs::write(bat.join("charge_full"), "5000000\n").unwrap();
        fs::write(bat.join("charge_full_design"), "5000000\n").unwrap();
        fs::write(bat.join("current_now"), "0\n").unwrap();
        fs::write(bat.join("voltage_now"), "16000000\n").unwrap();
        fs::write(bat.join("voltage_min_design"), "15000000\n").unwrap();
        fs::write(bat.join("technology"), "Li-ion\n").unwrap();
        fs::write(bat.join("manufacturer"), "OEM\n").unwrap();
        fs::write(bat.join("model_name"), "standard\n").unwrap();

        let info = read_battery_info(tmp.path());
        assert_eq!(info.cycle_count, 42); // Falls back to ACPI cycle_count
    }

    #[test]
    fn battery_info_prefers_bat_raw_cycle_count() {
        let tmp = tempfile::tempdir().unwrap();
        let bat = tmp.path().join("BAT0");
        fs::create_dir_all(&bat).unwrap();

        fs::write(bat.join("capacity"), "90\n").unwrap();
        fs::write(bat.join("status"), "Full\n").unwrap();
        fs::write(bat.join("raw_cycle_count"), "36\n").unwrap();
        fs::write(bat.join("cycle_count"), "0\n").unwrap();
        fs::write(bat.join("charge_now"), "5000000\n").unwrap();
        fs::write(bat.join("charge_full"), "5000000\n").unwrap();
        fs::write(bat.join("charge_full_design"), "5000000\n").unwrap();
        fs::write(bat.join("current_now"), "0\n").unwrap();
        fs::write(bat.join("voltage_now"), "16000000\n").unwrap();
        fs::write(bat.join("voltage_min_design"), "15000000\n").unwrap();
        fs::write(bat.join("technology"), "Li-ion\n").unwrap();
        fs::write(bat.join("manufacturer"), "OEM\n").unwrap();
        fs::write(bat.join("model_name"), "standard\n").unwrap();

        let info = read_battery_info(tmp.path());
        assert_eq!(info.cycle_count, 36);
    }

    #[test]
    fn battery_info_normalizes_large_bat_raw_cycle_count() {
        let tmp = tempfile::tempdir().unwrap();
        let bat = tmp.path().join("BAT0");
        fs::create_dir_all(&bat).unwrap();

        fs::write(bat.join("capacity"), "90\n").unwrap();
        fs::write(bat.join("status"), "Full\n").unwrap();
        fs::write(bat.join("raw_cycle_count"), "12836\n").unwrap();
        fs::write(bat.join("cycle_count"), "0\n").unwrap();
        fs::write(bat.join("charge_now"), "5000000\n").unwrap();
        fs::write(bat.join("charge_full"), "5000000\n").unwrap();
        fs::write(bat.join("charge_full_design"), "5000000\n").unwrap();
        fs::write(bat.join("current_now"), "0\n").unwrap();
        fs::write(bat.join("voltage_now"), "16000000\n").unwrap();
        fs::write(bat.join("voltage_min_design"), "15000000\n").unwrap();
        fs::write(bat.join("technology"), "Li-ion\n").unwrap();
        fs::write(bat.join("manufacturer"), "OEM\n").unwrap();
        fs::write(bat.join("model_name"), "standard\n").unwrap();

        let info = read_battery_info(tmp.path());
        assert_eq!(info.cycle_count, 36);
    }
}
