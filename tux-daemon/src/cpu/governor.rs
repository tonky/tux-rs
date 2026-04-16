//! CPU governor and EPP control via sysfs.
//!
//! Controls `/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor`,
//! `energy_performance_preference`, and turbo boost settings.

use std::io;
use std::path::{Path, PathBuf};

/// CPU governor/EPP controller.
///
/// Writes to per-CPU sysfs attributes for all online CPUs.
pub struct CpuGovernor {
    cpu_base: PathBuf,
}

impl Default for CpuGovernor {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuGovernor {
    const DEFAULT_CPU_BASE: &str = "/sys/devices/system/cpu";

    pub fn new() -> Self {
        Self {
            cpu_base: PathBuf::from(Self::DEFAULT_CPU_BASE),
        }
    }

    #[cfg(test)]
    pub fn with_path(base: impl Into<PathBuf>) -> Self {
        Self {
            cpu_base: base.into(),
        }
    }

    /// List all cpuN directories with their parsed index.
    fn all_cpu_dirs(&self) -> io::Result<Vec<(u32, PathBuf)>> {
        let mut dirs = Vec::new();
        for entry in std::fs::read_dir(&self.cpu_base)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(stripped) = name.strip_prefix("cpu")
                && let Ok(idx) = stripped.parse::<u32>()
            {
                dirs.push((idx, entry.path()));
            }
        }
        dirs.sort_by_key(|(idx, _)| *idx);
        Ok(dirs)
    }

    /// List all cpuN directories that have a cpufreq subdirectory.
    fn cpu_dirs(&self) -> io::Result<Vec<PathBuf>> {
        Ok(self
            .all_cpu_dirs()?
            .into_iter()
            .filter(|(_, p)| p.join("cpufreq").is_dir())
            .map(|(_, p)| p)
            .collect())
    }

    /// Write a value to a cpufreq attribute on all CPUs.
    ///
    /// Best-effort: tries all CPUs. Only fails if _no_ CPU could be written
    /// (e.g. attribute doesn't exist). Partial EBUSY from competing services
    /// like power-profiles-daemon is tolerated as long as at least one write
    /// succeeds.
    fn write_all(&self, attr: &str, value: &str) -> io::Result<()> {
        let dirs = self.cpu_dirs()?;
        if dirs.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no CPU cpufreq directories found",
            ));
        }
        let mut successes = 0usize;
        let mut last_err = None;
        for dir in &dirs {
            let path = dir.join("cpufreq").join(attr);
            match std::fs::write(&path, value) {
                Ok(()) => successes += 1,
                Err(e) => last_err = Some(e),
            }
        }
        if successes > 0 {
            Ok(())
        } else {
            Err(last_err.unwrap_or_else(|| io::Error::other("all CPU writes failed")))
        }
    }

    /// Read a cpufreq attribute from cpu0.
    fn read_cpu0(&self, attr: &str) -> io::Result<String> {
        let path = self.cpu_base.join("cpu0/cpufreq").join(attr);
        let val = std::fs::read_to_string(&path)?;
        Ok(val.trim().to_string())
    }

    /// Set the scaling governor for all CPUs.
    pub fn set_governor(&self, governor: &str) -> io::Result<()> {
        let available = self.available_governors()?;
        if !available.contains(&governor.to_string()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "governor '{governor}' not available (available: {})",
                    available.join(", ")
                ),
            ));
        }
        self.write_all("scaling_governor", governor)
    }

    /// Get the current scaling governor (from cpu0).
    pub fn get_governor(&self) -> io::Result<String> {
        self.read_cpu0("scaling_governor")
    }

    /// Set the energy performance preference for all CPUs.
    pub fn set_epp(&self, epp: &str) -> io::Result<()> {
        self.write_all("energy_performance_preference", epp)
    }

    /// Get the current EPP (from cpu0). Returns `None` if the attribute doesn't exist.
    pub fn get_epp(&self) -> io::Result<Option<String>> {
        let path = self
            .cpu_base
            .join("cpu0/cpufreq/energy_performance_preference");
        if !path.exists() {
            return Ok(None);
        }
        let val = std::fs::read_to_string(&path)?;
        Ok(Some(val.trim().to_string()))
    }

    /// Set the no_turbo flag.
    ///
    /// Tries intel_pstate first, then falls back to cpufreq/boost (inverted).
    pub fn set_no_turbo(&self, no_turbo: bool) -> io::Result<()> {
        let intel_path = self.cpu_base.join("intel_pstate/no_turbo");
        if intel_path.exists() {
            return std::fs::write(&intel_path, if no_turbo { "1" } else { "0" });
        }

        let boost_path = self.cpu_base.join("cpufreq/boost");
        if boost_path.exists() {
            // boost is inverted: boost=1 means turbo ON, so no_turbo=true → boost=0
            return std::fs::write(&boost_path, if no_turbo { "0" } else { "1" });
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "neither intel_pstate/no_turbo nor cpufreq/boost found",
        ))
    }

    /// Get the current no_turbo state.
    pub fn get_no_turbo(&self) -> io::Result<bool> {
        let intel_path = self.cpu_base.join("intel_pstate/no_turbo");
        if intel_path.exists() {
            let val = std::fs::read_to_string(&intel_path)?;
            return Ok(val.trim() == "1");
        }

        let boost_path = self.cpu_base.join("cpufreq/boost");
        if boost_path.exists() {
            let val = std::fs::read_to_string(&boost_path)?;
            // boost=0 means no turbo
            return Ok(val.trim() == "0");
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "neither intel_pstate/no_turbo nor cpufreq/boost found",
        ))
    }

    /// Get list of available governors (from cpu0).
    pub fn available_governors(&self) -> io::Result<Vec<String>> {
        let raw = self.read_cpu0("scaling_available_governors")?;
        Ok(raw.split_whitespace().map(String::from).collect())
    }

    /// Set the number of online CPU cores.
    ///
    /// Iterates over all logical CPUs. Sets `online=1` for the first `count` CPUs,
    /// and `online=0` for the rest. CPU0 is always skipped (cannot be taken offline).
    pub fn set_online_cores(&self, count: u32) -> io::Result<()> {
        let count = count.max(1); // At least 1 core must be online
        let dirs = self.all_cpu_dirs()?;

        let mut successes = 0usize;
        let mut last_err = None;

        for (idx, dir) in dirs {
            if idx == 0 {
                continue; // CPU 0 is usually not hotpluggable
            }
            let online_path = dir.join("online");
            if !online_path.exists() {
                continue;
            }

            let val = if idx < count { "1" } else { "0" };
            match std::fs::write(&online_path, val) {
                Ok(()) => successes += 1,
                Err(e) => last_err = Some(e),
            }
        }

        // We only error out if we tried to write something and completely failed.
        // If there were no `online` files, we just succeed.
        if successes > 0 {
            Ok(())
        } else if let Some(e) = last_err {
            Err(e)
        } else {
            Ok(())
        }
    }

    /// Set the scaling minimum frequency for all online CPUs.
    pub fn set_scaling_min_freq(&self, freq: u32) -> io::Result<()> {
        self.write_all("scaling_min_freq", &freq.to_string())
    }

    /// Set the scaling maximum frequency for all online CPUs.
    pub fn set_scaling_max_freq(&self, freq: u32) -> io::Result<()> {
        self.write_all("scaling_max_freq", &freq.to_string())
    }

    /// Get the current scaling minimum frequency from cpu0.
    pub fn get_scaling_min_freq(&self) -> io::Result<u32> {
        let raw = self.read_cpu0("scaling_min_freq")?;
        raw.parse::<u32>().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid scaling_min_freq value '{raw}': {e}"),
            )
        })
    }

    /// Get the current scaling maximum frequency from cpu0.
    pub fn get_scaling_max_freq(&self) -> io::Result<u32> {
        let raw = self.read_cpu0("scaling_max_freq")?;
        raw.parse::<u32>().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid scaling_max_freq value '{raw}': {e}"),
            )
        })
    }

    /// Get the hardware (BIOS/HW) minimum frequency in kHz from cpu0.
    pub fn get_cpuinfo_min_freq(&self) -> io::Result<u32> {
        let raw = self.read_cpu0("cpuinfo_min_freq")?;
        raw.parse::<u32>().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid cpuinfo_min_freq value '{raw}': {e}"),
            )
        })
    }

    /// Get the hardware (BIOS/HW) maximum frequency in kHz from cpu0.
    pub fn get_cpuinfo_max_freq(&self) -> io::Result<u32> {
        let raw = self.read_cpu0("cpuinfo_max_freq")?;
        raw.parse::<u32>().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid cpuinfo_max_freq value '{raw}': {e}"),
            )
        })
    }

    /// Count of CPU cores (directories with cpufreq under cpu_base).
    pub fn cpu_count(&self) -> io::Result<u32> {
        Ok(self.all_cpu_dirs()?.len() as u32)
    }
}

/// Check if CPU governor control is available.
pub fn cpu_governor_available(base: &Path) -> bool {
    base.join("cpu0/cpufreq/scaling_governor").exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Create a fake CPU sysfs tree with N cpus.
    fn setup_cpu_tree(dir: &Path, num_cpus: usize) {
        for i in 0..num_cpus {
            let cpu_dir = dir.join(format!("cpu{i}"));
            let cpufreq = cpu_dir.join("cpufreq");
            fs::create_dir_all(&cpufreq).unwrap();
            fs::write(cpufreq.join("scaling_governor"), "powersave\n").unwrap();
            fs::write(
                cpufreq.join("scaling_available_governors"),
                "performance powersave schedutil\n",
            )
            .unwrap();
            fs::write(
                cpufreq.join("energy_performance_preference"),
                "balance_performance\n",
            )
            .unwrap();
            fs::write(cpufreq.join("scaling_min_freq"), "400000\n").unwrap();
            fs::write(cpufreq.join("scaling_max_freq"), "2000000\n").unwrap();
            fs::write(cpufreq.join("cpuinfo_min_freq"), "400000\n").unwrap();
            fs::write(cpufreq.join("cpuinfo_max_freq"), "4800000\n").unwrap();
            if i > 0 {
                fs::write(cpu_dir.join("online"), "1\n").unwrap();
            }
        }
    }

    #[test]
    fn set_governor_writes_to_all_cpus() {
        let tmp = tempfile::tempdir().unwrap();
        setup_cpu_tree(tmp.path(), 4);
        let gov = CpuGovernor::with_path(tmp.path());

        gov.set_governor("performance").unwrap();

        for i in 0..4 {
            let val =
                fs::read_to_string(tmp.path().join(format!("cpu{i}/cpufreq/scaling_governor")))
                    .unwrap();
            assert_eq!(val, "performance");
        }
    }

    #[test]
    fn set_governor_invalid_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        setup_cpu_tree(tmp.path(), 2);
        let gov = CpuGovernor::with_path(tmp.path());

        let err = gov.set_governor("nonexistent").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("not available"));
    }

    #[test]
    fn set_epp_writes_to_all_cpus() {
        let tmp = tempfile::tempdir().unwrap();
        setup_cpu_tree(tmp.path(), 2);
        let gov = CpuGovernor::with_path(tmp.path());

        gov.set_epp("power").unwrap();

        for i in 0..2 {
            let val = fs::read_to_string(
                tmp.path()
                    .join(format!("cpu{i}/cpufreq/energy_performance_preference")),
            )
            .unwrap();
            assert_eq!(val, "power");
        }
    }

    #[test]
    fn get_governor_reads_cpu0() {
        let tmp = tempfile::tempdir().unwrap();
        setup_cpu_tree(tmp.path(), 2);
        let gov = CpuGovernor::with_path(tmp.path());

        assert_eq!(gov.get_governor().unwrap(), "powersave");
    }

    #[test]
    fn get_epp_returns_none_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let cpufreq = tmp.path().join("cpu0/cpufreq");
        fs::create_dir_all(&cpufreq).unwrap();
        fs::write(cpufreq.join("scaling_governor"), "powersave\n").unwrap();
        // No energy_performance_preference file

        let gov = CpuGovernor::with_path(tmp.path());
        assert_eq!(gov.get_epp().unwrap(), None);
    }

    #[test]
    fn no_turbo_intel_pstate() {
        let tmp = tempfile::tempdir().unwrap();
        let intel = tmp.path().join("intel_pstate");
        fs::create_dir_all(&intel).unwrap();
        fs::write(intel.join("no_turbo"), "0\n").unwrap();

        let gov = CpuGovernor::with_path(tmp.path());

        assert!(!gov.get_no_turbo().unwrap());

        gov.set_no_turbo(true).unwrap();
        assert_eq!(fs::read_to_string(intel.join("no_turbo")).unwrap(), "1");
    }

    #[test]
    fn no_turbo_cpufreq_boost() {
        let tmp = tempfile::tempdir().unwrap();
        let cpufreq = tmp.path().join("cpufreq");
        fs::create_dir_all(&cpufreq).unwrap();
        fs::write(cpufreq.join("boost"), "1\n").unwrap();

        let gov = CpuGovernor::with_path(tmp.path());

        // boost=1 → no_turbo=false
        assert!(!gov.get_no_turbo().unwrap());

        gov.set_no_turbo(true).unwrap();
        // no_turbo=true → boost=0
        assert_eq!(fs::read_to_string(cpufreq.join("boost")).unwrap(), "0");
    }

    #[test]
    fn available_governors_parsed() {
        let tmp = tempfile::tempdir().unwrap();
        setup_cpu_tree(tmp.path(), 1);
        let gov = CpuGovernor::with_path(tmp.path());

        let avail = gov.available_governors().unwrap();
        assert_eq!(avail, vec!["performance", "powersave", "schedutil"]);
    }

    #[test]
    fn set_online_cores_works() {
        let tmp = tempfile::tempdir().unwrap();
        setup_cpu_tree(tmp.path(), 4);
        let gov = CpuGovernor::with_path(tmp.path());

        // Set to 2 cores online
        gov.set_online_cores(2).unwrap();

        // CPU0 is always online, so it doesn't have an online file in our test setup
        assert_eq!(
            fs::read_to_string(tmp.path().join("cpu1/online")).unwrap(),
            "1"
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join("cpu2/online")).unwrap(),
            "0"
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join("cpu3/online")).unwrap(),
            "0"
        );
    }

    #[test]
    fn set_scaling_min_max_freq_works() {
        let tmp = tempfile::tempdir().unwrap();
        setup_cpu_tree(tmp.path(), 2);
        let gov = CpuGovernor::with_path(tmp.path());

        gov.set_scaling_min_freq(800000).unwrap();
        gov.set_scaling_max_freq(3500000).unwrap();

        for i in 0..2 {
            let min_val =
                fs::read_to_string(tmp.path().join(format!("cpu{i}/cpufreq/scaling_min_freq")))
                    .unwrap();
            let max_val =
                fs::read_to_string(tmp.path().join(format!("cpu{i}/cpufreq/scaling_max_freq")))
                    .unwrap();
            assert_eq!(min_val, "800000");
            assert_eq!(max_val, "3500000");
        }
    }

    #[test]
    fn get_scaling_min_max_freq_reads_cpu0() {
        let tmp = tempfile::tempdir().unwrap();
        setup_cpu_tree(tmp.path(), 2);
        let gov = CpuGovernor::with_path(tmp.path());

        assert_eq!(gov.get_scaling_min_freq().unwrap(), 400000);
        assert_eq!(gov.get_scaling_max_freq().unwrap(), 2000000);
    }

    #[test]
    fn get_cpuinfo_min_max_freq_reads_cpu0() {
        let tmp = tempfile::tempdir().unwrap();
        setup_cpu_tree(tmp.path(), 2);
        let gov = CpuGovernor::with_path(tmp.path());

        assert_eq!(gov.get_cpuinfo_min_freq().unwrap(), 400000);
        assert_eq!(gov.get_cpuinfo_max_freq().unwrap(), 4800000);
    }

    #[test]
    fn cpu_count_returns_number_of_cpu_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        setup_cpu_tree(tmp.path(), 4);
        let gov = CpuGovernor::with_path(tmp.path());

        assert_eq!(gov.cpu_count().unwrap(), 4);
    }
}
