use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

/// Mock sysfs filesystem for testing platform backends.
///
/// Creates a temporary directory tree that mimics the sysfs layout
/// produced by kernel shims. Automatically cleaned up on drop.
pub struct MockSysfs {
    root: TempDir,
}

impl MockSysfs {
    pub fn new() -> Self {
        Self {
            root: TempDir::new().expect("failed to create temp dir"),
        }
    }

    /// Root path of the mock sysfs tree.
    pub fn root(&self) -> &Path {
        self.root.path()
    }

    /// Create an attribute file with the given value. Parent dirs are created automatically.
    pub fn create_attr(&self, path: &str, value: &str) -> PathBuf {
        let full_path = self.root.path().join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent dirs");
        }
        fs::write(&full_path, value).expect("failed to write attr");
        full_path
    }

    /// Read an attribute file's contents.
    pub fn read_attr(&self, path: &str) -> String {
        let full_path = self.root.path().join(path);
        fs::read_to_string(full_path).expect("failed to read attr")
    }

    /// Create and return the path to a platform device directory.
    pub fn platform_dir(&self, name: &str) -> PathBuf {
        let dir = self.root.path().join("devices/platform").join(name);
        fs::create_dir_all(&dir).expect("failed to create platform dir");
        dir
    }

    /// Create a Uniwill fan platform sysfs tree.
    pub fn create_uniwill_tree(&self) -> PathBuf {
        let base = self.platform_dir("tuxedo-uniwill");
        self.create_attr("devices/platform/tuxedo-uniwill/temp1_input", "45000");
        self.create_attr("devices/platform/tuxedo-uniwill/pwm1", "0");
        self.create_attr("devices/platform/tuxedo-uniwill/pwm2", "0");
        self.create_attr("devices/platform/tuxedo-uniwill/fan1_input", "2400");
        self.create_attr("devices/platform/tuxedo-uniwill/fan2_input", "2400");
        self.create_attr("devices/platform/tuxedo-uniwill/pwm1_enable", "2");
        self.create_attr("devices/platform/tuxedo-uniwill/pwm2_enable", "2");
        base
    }

    /// Create a Tuxi platform sysfs tree.
    pub fn create_tuxi_tree(&self) -> PathBuf {
        let base = self.platform_dir("tuxedo-tuxi");
        self.create_attr("devices/platform/tuxedo-tuxi/temp1_input", "40000");
        self.create_attr("devices/platform/tuxedo-tuxi/pwm1", "0");
        self.create_attr("devices/platform/tuxedo-tuxi/fan1_input", "2000");
        base
    }

    /// Create a Clevo platform sysfs tree.
    pub fn create_clevo_tree(&self, num_fans: u8) -> PathBuf {
        let base = self.platform_dir("tuxedo-clevo");
        self.create_attr("devices/platform/tuxedo-clevo/temp1_input", "50000");
        for i in 1..=num_fans {
            self.create_attr(&format!("devices/platform/tuxedo-clevo/pwm{i}"), "0");
            self.create_attr(
                &format!("devices/platform/tuxedo-clevo/fan{i}_input"),
                "2800",
            );
        }
        base
    }

    /// Create an NB04 platform sysfs tree.
    pub fn create_nb04_tree(&self) -> PathBuf {
        let base = self.platform_dir("tuxedo-nb04");
        self.create_attr("devices/platform/tuxedo-nb04/temp1_input", "55000");
        self.create_attr("devices/platform/tuxedo-nb04/fan1_input", "3000");
        self.create_attr("devices/platform/tuxedo-nb04/fan2_input", "3200");
        base
    }

    /// Create a mock CPU sysfs tree with cpufreq directories.
    pub fn create_cpu_tree(&self, cores: u8) -> PathBuf {
        let base = self.root.path().join("devices/system/cpu");
        fs::create_dir_all(&base).expect("failed to create cpu dir");
        for i in 0..cores {
            let cpufreq = base.join(format!("cpu{i}/cpufreq"));
            fs::create_dir_all(&cpufreq).expect("failed to create cpufreq dir");
            fs::write(cpufreq.join("scaling_governor"), "powersave")
                .expect("failed to write governor");
            fs::write(
                cpufreq.join("scaling_available_governors"),
                "performance powersave",
            )
            .expect("failed to write available governors");
            fs::write(cpufreq.join("scaling_cur_freq"), "2400000")
                .expect("failed to write cur_freq");
            fs::write(
                cpufreq.join("energy_performance_preference"),
                "balance_performance",
            )
            .expect("failed to write epp");
        }
        base
    }

    /// Create a mock battery / AC power supply sysfs tree.
    pub fn create_power_supply(&self, name: &str, online: bool) -> PathBuf {
        let base = self.root.path().join("class/power_supply").join(name);
        fs::create_dir_all(&base).expect("failed to create power_supply dir");
        fs::write(base.join("online"), if online { "1" } else { "0" })
            .expect("failed to write online");
        fs::write(base.join("type"), "Mains").expect("failed to write type");
        base
    }

    /// Create a mock GPU hwmon sysfs tree.
    pub fn create_gpu_hwmon(&self, name: &str) -> PathBuf {
        let base = self.root.path().join("class/hwmon").join(name);
        fs::create_dir_all(&base).expect("failed to create hwmon dir");
        fs::write(base.join("name"), "nvidia").expect("failed to write name");
        fs::write(base.join("temp1_input"), "62000").expect("failed to write temp");
        fs::write(base.join("power1_average"), "45000000").expect("failed to write power");
        base
    }

    /// Create a mock EC RAM binary attribute (for TDP control).
    pub fn create_ec_ram(&self) -> PathBuf {
        let base = self.platform_dir("tuxedo-ec");
        // Create a 2048-byte EC RAM file (zeroed).
        let ec_ram = base.join("ec_ram");
        fs::write(&ec_ram, vec![0u8; 2048]).expect("failed to write ec_ram");
        base
    }

    /// Create a mock Clevo charging sysfs tree.
    pub fn create_clevo_charging(&self) -> PathBuf {
        let base = self.platform_dir("tuxedo-clevo");
        self.create_attr("devices/platform/tuxedo-clevo/charge_start_threshold", "75");
        self.create_attr("devices/platform/tuxedo-clevo/charge_end_threshold", "90");
        base
    }

    /// Create a mock Uniwill charging sysfs tree.
    pub fn create_uniwill_charging(&self) -> PathBuf {
        let base = self.platform_dir("tuxedo-uniwill");
        self.create_attr(
            "devices/platform/tuxedo-uniwill/charge_profile",
            "high_capacity",
        );
        self.create_attr("devices/platform/tuxedo-uniwill/charge_priority", "charge");
        base
    }

    /// Create a mock NVIDIA power control sysfs tree.
    pub fn create_nvidia_power_ctrl(&self) -> PathBuf {
        let base = self.platform_dir("tuxedo_nvidia_power_ctrl");
        self.create_attr("devices/platform/tuxedo_nvidia_power_ctrl/ctgp_offset", "0");
        base
    }
}

impl Default for MockSysfs {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_read_attr() {
        let sysfs = MockSysfs::new();
        sysfs.create_attr("test/attr", "hello");
        assert_eq!(sysfs.read_attr("test/attr"), "hello");
    }

    #[test]
    fn platform_dir_created() {
        let sysfs = MockSysfs::new();
        let dir = sysfs.platform_dir("tuxedo-uniwill");
        assert!(dir.exists());
        assert!(dir.is_dir());
    }

    #[test]
    fn uniwill_tree_has_expected_files() {
        let sysfs = MockSysfs::new();
        let base = sysfs.create_uniwill_tree();
        assert!(base.join("temp1_input").exists());
        assert!(base.join("pwm1").exists());
        assert!(base.join("pwm2").exists());
        assert!(base.join("fan1_input").exists());
        assert!(base.join("fan2_input").exists());
        assert!(base.join("pwm1_enable").exists());
        assert!(base.join("pwm2_enable").exists());
    }

    #[test]
    fn tuxi_tree_has_expected_files() {
        let sysfs = MockSysfs::new();
        let base = sysfs.create_tuxi_tree();
        assert!(base.join("temp1_input").exists());
        assert!(base.join("pwm1").exists());
        assert!(base.join("fan1_input").exists());
    }

    #[test]
    fn clevo_tree_fan_count() {
        let sysfs = MockSysfs::new();
        let base = sysfs.create_clevo_tree(3);
        assert!(base.join("pwm1").exists());
        assert!(base.join("pwm2").exists());
        assert!(base.join("pwm3").exists());
        assert!(base.join("fan1_input").exists());
        assert!(base.join("fan3_input").exists());
    }

    #[test]
    fn nb04_tree_has_expected_files() {
        let sysfs = MockSysfs::new();
        let base = sysfs.create_nb04_tree();
        assert!(base.join("temp1_input").exists());
        assert!(base.join("fan1_input").exists());
        assert!(base.join("fan2_input").exists());
    }

    #[test]
    fn attr_values_correct() {
        let sysfs = MockSysfs::new();
        sysfs.create_uniwill_tree();
        assert_eq!(
            sysfs.read_attr("devices/platform/tuxedo-uniwill/temp1_input"),
            "45000"
        );
        assert_eq!(
            sysfs.read_attr("devices/platform/tuxedo-uniwill/fan1_input"),
            "2400"
        );
    }

    #[test]
    fn cpu_tree_creates_all_cores() {
        let sysfs = MockSysfs::new();
        let base = sysfs.create_cpu_tree(4);
        for i in 0..4u8 {
            let cpufreq = base.join(format!("cpu{i}/cpufreq"));
            assert!(cpufreq.join("scaling_governor").exists());
            assert!(cpufreq.join("scaling_available_governors").exists());
            assert!(cpufreq.join("scaling_cur_freq").exists());
            assert!(cpufreq.join("energy_performance_preference").exists());
        }
        assert!(!base.join("cpu4/cpufreq").exists());
    }

    #[test]
    fn power_supply_online() {
        let sysfs = MockSysfs::new();
        let ac = sysfs.create_power_supply("AC0", true);
        assert_eq!(fs::read_to_string(ac.join("online")).unwrap(), "1");

        let bat = sysfs.create_power_supply("BAT0", false);
        assert_eq!(fs::read_to_string(bat.join("online")).unwrap(), "0");
    }

    #[test]
    fn gpu_hwmon_tree() {
        let sysfs = MockSysfs::new();
        let hwmon = sysfs.create_gpu_hwmon("hwmon0");
        assert_eq!(fs::read_to_string(hwmon.join("name")).unwrap(), "nvidia");
        assert_eq!(
            fs::read_to_string(hwmon.join("temp1_input")).unwrap(),
            "62000"
        );
    }

    #[test]
    fn ec_ram_created() {
        let sysfs = MockSysfs::new();
        let base = sysfs.create_ec_ram();
        let ec_ram = base.join("ec_ram");
        assert!(ec_ram.exists());
        let data = fs::read(&ec_ram).unwrap();
        assert_eq!(data.len(), 2048);
    }

    #[test]
    fn clevo_charging_tree() {
        let sysfs = MockSysfs::new();
        sysfs.create_clevo_charging();
        assert_eq!(
            sysfs.read_attr("devices/platform/tuxedo-clevo/charge_start_threshold"),
            "75"
        );
        assert_eq!(
            sysfs.read_attr("devices/platform/tuxedo-clevo/charge_end_threshold"),
            "90"
        );
    }

    #[test]
    fn nvidia_power_ctrl_tree() {
        let sysfs = MockSysfs::new();
        sysfs.create_nvidia_power_ctrl();
        assert_eq!(
            sysfs.read_attr("devices/platform/tuxedo_nvidia_power_ctrl/ctgp_offset"),
            "0"
        );
    }
}
