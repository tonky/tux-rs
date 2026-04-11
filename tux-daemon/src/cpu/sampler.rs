//! CPU load sampler: reads `/proc/stat` and computes per-core + overall utilization.

use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

/// Raw CPU time counters from a single `/proc/stat` line.
#[derive(Debug, Clone, Default)]
struct CpuJiffies {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
}

impl CpuJiffies {
    /// Parse a `/proc/stat` CPU line (e.g. `cpu  1234 56 789 ...` or `cpu0 ...`).
    fn parse(line: &str) -> Option<Self> {
        let mut parts = line.split_whitespace();
        let label = parts.next()?;
        if !label.starts_with("cpu") {
            return None;
        }
        let vals: Vec<u64> = parts.filter_map(|p| p.parse().ok()).collect();
        if vals.len() < 4 {
            return None;
        }
        Some(Self {
            user: vals[0],
            nice: vals[1],
            system: vals[2],
            idle: vals[3],
            iowait: vals.get(4).copied().unwrap_or(0),
            irq: vals.get(5).copied().unwrap_or(0),
            softirq: vals.get(6).copied().unwrap_or(0),
            steal: vals.get(7).copied().unwrap_or(0),
        })
    }

    fn total(&self) -> u64 {
        self.user
            + self.nice
            + self.system
            + self.idle
            + self.iowait
            + self.irq
            + self.softirq
            + self.steal
    }

    fn idle_total(&self) -> u64 {
        self.idle + self.iowait
    }

    /// Compute utilization percentage from the delta between `self` (current) and `prev`.
    fn utilization_from(&self, prev: &Self) -> f32 {
        let total_delta = self.total().saturating_sub(prev.total());
        if total_delta == 0 {
            return 0.0;
        }
        let idle_delta = self.idle_total().saturating_sub(prev.idle_total());
        let busy_delta = total_delta.saturating_sub(idle_delta);
        (busy_delta as f32 / total_delta as f32) * 100.0
    }
}

/// Snapshot of CPU load percentages.
#[derive(Debug, Clone)]
pub struct CpuLoadSnapshot {
    /// Overall CPU utilization (0–100%).
    pub overall: f32,
    /// Per-core utilization, indexed by core number.
    pub per_core: Vec<f32>,
}

/// Stateful sampler that tracks previous jiffies to compute deltas.
pub struct CpuSampler {
    proc_stat_path: PathBuf,
    prev_overall: Option<CpuJiffies>,
    prev_per_core: Vec<CpuJiffies>,
}

impl CpuSampler {
    /// Create a new sampler reading from the given `/proc/stat` path.
    pub fn new(proc_stat_path: &Path) -> Self {
        Self {
            proc_stat_path: proc_stat_path.to_path_buf(),
            prev_overall: None,
            prev_per_core: Vec::new(),
        }
    }

    /// Create a sampler using the default `/proc/stat`.
    pub fn system() -> Self {
        Self::new(Path::new("/proc/stat"))
    }

    /// Take a sample and compute load deltas from the previous sample.
    ///
    /// On the first call, returns 0% for all cores (no previous data to diff).
    pub fn sample(&mut self) -> io::Result<CpuLoadSnapshot> {
        let (overall_jiffies, per_core_jiffies) = self.read_stat()?;

        let overall = match &self.prev_overall {
            Some(prev) => overall_jiffies.utilization_from(prev),
            None => 0.0,
        };

        let per_core: Vec<f32> = per_core_jiffies
            .iter()
            .enumerate()
            .map(|(i, current)| {
                self.prev_per_core
                    .get(i)
                    .map(|prev| current.utilization_from(prev))
                    .unwrap_or(0.0)
            })
            .collect();

        self.prev_overall = Some(overall_jiffies);
        self.prev_per_core = per_core_jiffies;

        Ok(CpuLoadSnapshot { overall, per_core })
    }

    /// Read and parse `/proc/stat`.
    fn read_stat(&self) -> io::Result<(CpuJiffies, Vec<CpuJiffies>)> {
        let file = std::fs::File::open(&self.proc_stat_path)?;
        let reader = io::BufReader::new(file);

        let mut overall = None;
        let mut per_core = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.starts_with("cpu") {
                if line.starts_with("cpu ") || (line.len() >= 4 && &line[..4] == "cpu ") {
                    // Aggregate "cpu " line
                    overall = CpuJiffies::parse(&line);
                } else if line.starts_with("cpu")
                    && line.as_bytes().get(3).is_some_and(|b| b.is_ascii_digit())
                {
                    // Per-core "cpuN ..." line
                    if let Some(jiffies) = CpuJiffies::parse(&line) {
                        per_core.push(jiffies);
                    }
                }
            } else if !per_core.is_empty() {
                // Past the cpu lines, stop parsing
                break;
            }
        }

        let overall = overall.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "no aggregate cpu line in /proc/stat",
            )
        })?;

        Ok((overall, per_core))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const SAMPLE_PROC_STAT_1: &str = "\
cpu  10000 200 3000 80000 500 100 50 10 0 0
cpu0 2500 50 750 20000 125 25 12 2 0 0
cpu1 2500 50 750 20000 125 25 13 3 0 0
cpu2 2500 50 750 20000 125 25 12 2 0 0
cpu3 2500 50 750 20000 125 25 13 3 0 0
intr 12345678
";

    const SAMPLE_PROC_STAT_2: &str = "\
cpu  15000 200 5000 82000 500 100 50 10 0 0
cpu0 4000 50 1250 20500 125 25 12 2 0 0
cpu1 3500 50 1250 20500 125 25 13 3 0 0
cpu2 4000 50 1250 20500 125 25 12 2 0 0
cpu3 3500 50 1250 20500 125 25 13 3 0 0
intr 23456789
";

    #[test]
    fn first_sample_returns_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let stat_path = tmp.path().join("stat");
        fs::write(&stat_path, SAMPLE_PROC_STAT_1).unwrap();

        let mut sampler = CpuSampler::new(&stat_path);
        let snap = sampler.sample().unwrap();

        assert_eq!(snap.overall, 0.0);
        assert_eq!(snap.per_core.len(), 4);
        assert!(snap.per_core.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn second_sample_computes_delta() {
        let tmp = tempfile::tempdir().unwrap();
        let stat_path = tmp.path().join("stat");

        // First sample
        fs::write(&stat_path, SAMPLE_PROC_STAT_1).unwrap();
        let mut sampler = CpuSampler::new(&stat_path);
        let _ = sampler.sample().unwrap();

        // Second sample
        fs::write(&stat_path, SAMPLE_PROC_STAT_2).unwrap();
        let snap = sampler.sample().unwrap();

        // Delta: total went from 93860 to 102860 = 9000 total delta
        // idle went from 80500 to 82500 = 2000 idle delta
        // busy = 7000, utilization = 7000/9000 * 100 ≈ 77.78%
        assert!(
            snap.overall > 70.0 && snap.overall < 85.0,
            "overall={}",
            snap.overall
        );
        assert_eq!(snap.per_core.len(), 4);
        // Each core should show non-zero utilization
        for (i, &load) in snap.per_core.iter().enumerate() {
            assert!(load > 0.0, "core {i} load should be > 0, got {load}");
            assert!(load <= 100.0, "core {i} load should be <= 100, got {load}");
        }
    }

    #[test]
    fn identical_samples_give_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let stat_path = tmp.path().join("stat");

        fs::write(&stat_path, SAMPLE_PROC_STAT_1).unwrap();
        let mut sampler = CpuSampler::new(&stat_path);
        let _ = sampler.sample().unwrap();

        // Same data again → 0 delta → 0% load
        let snap = sampler.sample().unwrap();
        assert_eq!(snap.overall, 0.0);
        assert!(snap.per_core.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn full_load_sample() {
        let tmp = tempfile::tempdir().unwrap();
        let stat_path = tmp.path().join("stat");

        let idle = "\
cpu  1000 0 0 9000 0 0 0 0 0 0
cpu0 500 0 0 4500 0 0 0 0 0 0
cpu1 500 0 0 4500 0 0 0 0 0 0
intr 0
";
        let busy = "\
cpu  6000 0 0 9000 0 0 0 0 0 0
cpu0 3000 0 0 4500 0 0 0 0 0 0
cpu1 3000 0 0 4500 0 0 0 0 0 0
intr 0
";

        fs::write(&stat_path, idle).unwrap();
        let mut sampler = CpuSampler::new(&stat_path);
        let _ = sampler.sample().unwrap();

        fs::write(&stat_path, busy).unwrap();
        let snap = sampler.sample().unwrap();

        // 5000 busy added, 0 idle added → 100% utilization
        assert!(
            (snap.overall - 100.0).abs() < 0.01,
            "overall={}",
            snap.overall
        );
        assert_eq!(snap.per_core.len(), 2);
        for load in &snap.per_core {
            assert!((*load - 100.0).abs() < 0.01, "per_core load={load}");
        }
    }

    #[test]
    fn parse_jiffies_minimal() {
        let j = CpuJiffies::parse("cpu  100 0 50 800 0 0 0 0").unwrap();
        assert_eq!(j.user, 100);
        assert_eq!(j.system, 50);
        assert_eq!(j.idle, 800);
        assert_eq!(j.total(), 950);
    }

    #[test]
    fn parse_jiffies_only_four_fields() {
        let j = CpuJiffies::parse("cpu0 100 0 50 800").unwrap();
        assert_eq!(j.iowait, 0);
        assert_eq!(j.steal, 0);
    }

    #[test]
    fn parse_non_cpu_line_returns_none() {
        assert!(CpuJiffies::parse("intr 12345").is_none());
    }
}
