//! Apply profile settings to hardware backends.

use std::sync::Arc;

use tokio::sync::watch;
use tracing::{debug, info, warn};

use tux_core::fan_curve::{FanConfig, FanMode};
use tux_core::profile::TuxProfile;

use crate::charging::ChargingBackend;
use crate::cpu::governor::CpuGovernor;
use crate::cpu::tdp::TdpBackend;
use crate::display::SharedDisplay;
use crate::gpu::GpuPowerBackend;
use crate::hid::SharedKeyboard;

/// Applies a profile's settings to the hardware.
pub struct ProfileApplier {
    fan_config_tx: watch::Sender<FanConfig>,
    charging: Option<Arc<dyn ChargingBackend>>,
    cpu_governor: Option<Arc<CpuGovernor>>,
    tdp_backend: Option<Arc<dyn TdpBackend>>,
    gpu_backend: Option<Arc<dyn GpuPowerBackend>>,
    keyboards: Vec<SharedKeyboard>,
    display: Option<SharedDisplay>,
}

impl ProfileApplier {
    pub fn new(
        fan_config_tx: watch::Sender<FanConfig>,
        charging: Option<Arc<dyn ChargingBackend>>,
        cpu_governor: Option<Arc<CpuGovernor>>,
        tdp_backend: Option<Arc<dyn TdpBackend>>,
        gpu_backend: Option<Arc<dyn GpuPowerBackend>>,
        keyboards: Vec<SharedKeyboard>,
        display: Option<SharedDisplay>,
    ) -> Self {
        Self {
            fan_config_tx,
            charging,
            cpu_governor,
            tdp_backend,
            gpu_backend,
            keyboards,
            display,
        }
    }

    fn sanitize_positive_u32(value: i32, field: &str, profile_name: &str) -> Option<u32> {
        if value <= 0 {
            warn!(
                "profile '{}' has invalid {}={} (must be > 0), ignoring",
                profile_name, field, value
            );
            None
        } else {
            Some(value as u32)
        }
    }

    fn apply_scaling_bounds(gov: &CpuGovernor, min_freq: u32, max_freq: u32) {
        let apply_min_then_max = match (gov.get_scaling_min_freq(), gov.get_scaling_max_freq()) {
            (Ok(current_min), Ok(_current_max)) => {
                // If the requested max is below the current min, lower min first.
                max_freq < current_min
            }
            _ => {
                // Best-effort fallback when current bounds cannot be read.
                false
            }
        };

        if apply_min_then_max {
            debug!(
                "applying CPU scaling bounds with min->max ordering: min={} max={}",
                min_freq, max_freq
            );
            if let Err(e) = gov.set_scaling_min_freq(min_freq) {
                warn!("failed to set scaling min freq: {e}");
            }
            if let Err(e) = gov.set_scaling_max_freq(max_freq) {
                warn!("failed to set scaling max freq: {e}");
            }
        } else {
            debug!(
                "applying CPU scaling bounds with max->min ordering: min={} max={}",
                min_freq, max_freq
            );
            if let Err(e) = gov.set_scaling_max_freq(max_freq) {
                warn!("failed to set scaling max freq: {e}");
            }
            if let Err(e) = gov.set_scaling_min_freq(min_freq) {
                warn!("failed to set scaling min freq: {e}");
            }
        }
    }

    /// Apply a profile's settings to hardware.
    pub fn apply(&self, profile: &TuxProfile) -> anyhow::Result<()> {
        // 1. Update fan curve engine config from profile.
        if profile.fan.enabled {
            let fan_config = FanConfig {
                mode: profile.fan.mode,
                min_speed_percent: profile.fan.min_speed_percent,
                curve: profile.fan.curve.clone(),
                ..FanConfig::default()
            };
            self.fan_config_tx.send(fan_config)?;
            info!(
                "applied fan settings from profile '{}': mode={:?}",
                profile.name, profile.fan.mode
            );
        } else {
            // Fan control disabled → set to auto.
            let auto_config = FanConfig {
                mode: FanMode::Auto,
                ..FanConfig::default()
            };
            self.fan_config_tx.send(auto_config)?;
            info!(
                "fan control disabled in profile '{}', set to auto",
                profile.name
            );
        }

        // 2. CPU governor + EPP + turbo + cores + frequencies.
        if let Some(ref gov) = self.cpu_governor {
            if let Err(e) = gov.set_governor(&profile.cpu.governor) {
                warn!("failed to set CPU governor: {e}");
            }
            if let Some(ref epp) = profile.cpu.energy_performance_preference
                && let Err(e) = gov.set_epp(epp)
            {
                warn!("failed to set CPU EPP: {e}");
            }
            if let Err(e) = gov.set_no_turbo(profile.cpu.no_turbo) {
                warn!("failed to set no_turbo: {e}");
            }
            if let Some(cores) = profile.cpu.online_cores {
                if let Some(cores) =
                    Self::sanitize_positive_u32(cores, "online_cores", &profile.name)
                    && let Err(e) = gov.set_online_cores(cores)
                {
                    warn!("failed to set online cores: {e}");
                }
            } else {
                // online_cores=None means "unmanaged" — restore all cores so the
                // profile doesn't leave the system in a reduced-core state from a
                // previous profile. Writing 256 is safe: the kernel clamps to the
                // actual CPU count.
                if let Err(e) = gov.set_online_cores(256) {
                    warn!("failed to reset online cores: {e}");
                }
            }

            let min_freq = profile.cpu.scaling_min_frequency.and_then(|v| {
                Self::sanitize_positive_u32(v, "scaling_min_frequency", &profile.name)
            });
            let max_freq = profile.cpu.scaling_max_frequency.and_then(|v| {
                Self::sanitize_positive_u32(v, "scaling_max_frequency", &profile.name)
            });

            match (min_freq, max_freq) {
                (Some(min_freq), Some(max_freq)) => {
                    if min_freq > max_freq {
                        warn!(
                            "profile '{}' has invalid CPU frequency bounds (min={} > max={}), skipping both",
                            profile.name, min_freq, max_freq
                        );
                    } else {
                        Self::apply_scaling_bounds(gov, min_freq, max_freq);
                    }
                }
                (Some(min_freq), None) => {
                    if let Err(e) = gov.set_scaling_min_freq(min_freq) {
                        warn!("failed to set scaling min freq: {e}");
                    }
                }
                (None, Some(max_freq)) => {
                    if let Err(e) = gov.set_scaling_max_freq(max_freq) {
                        warn!("failed to set scaling max freq: {e}");
                    }
                }
                (None, None) => {}
            }

            info!(
                "applied CPU settings from profile '{}': governor={}, no_turbo={}, cores={:?}, min_freq={:?}, max_freq={:?}",
                profile.name,
                profile.cpu.governor,
                profile.cpu.no_turbo,
                profile.cpu.online_cores,
                profile.cpu.scaling_min_frequency,
                profile.cpu.scaling_max_frequency
            );
        }

        // 3. Keyboard backlight brightness.
        if !self.keyboards.is_empty() {
            let brightness = profile.keyboard.brightness;
            // Profile brightness is percentage-like (0-100). Convert to hardware scale.
            let hw_brightness = ((brightness.min(100) as f32 / 100.0) * 255.0) as u8;
            for kb_lock in &self.keyboards {
                if let Ok(mut kb) = kb_lock.lock() {
                    if hw_brightness == 0 {
                        if let Err(e) = kb.turn_off() {
                            warn!("failed to turn off keyboard backlight: {e}");
                        }
                    } else {
                        if let Err(e) = kb.set_brightness(hw_brightness) {
                            warn!("failed to set keyboard brightness: {e}");
                        }
                        if let Err(e) = kb.turn_on() {
                            warn!("failed to turn on keyboard backlight: {e}");
                        }
                        if let Err(e) = kb.set_brightness(hw_brightness) {
                            warn!("failed to re-apply keyboard brightness: {e}");
                        }
                    }
                    if let Err(e) = kb.flush() {
                        warn!("failed to flush keyboard state: {e}");
                    }
                }
            }
            info!(
                "applied keyboard brightness={} (hw={}) from profile '{}'",
                brightness, hw_brightness, profile.name
            );
        }

        // 4. Apply charging settings.
        if let Some(ref backend) = self.charging {
            let cs = &profile.charging;
            debug!(
                "profile '{}' charging settings: profile={:?}, priority={:?}, start={:?}, end={:?}",
                profile.name, cs.profile, cs.priority, cs.start_threshold, cs.end_threshold
            );
            if let Some(start) = cs.start_threshold {
                debug!("setting charge start threshold: {start}");
                if let Err(e) = backend.set_start_threshold(start) {
                    warn!("failed to set charge start threshold: {e}");
                }
            }
            if let Some(end) = cs.end_threshold {
                debug!("setting charge end threshold: {end}");
                if let Err(e) = backend.set_end_threshold(end) {
                    warn!("failed to set charge end threshold: {e}");
                }
            }
            if let Some(ref p) = cs.profile {
                debug!("setting charge profile: {p}");
                match backend.get_profile() {
                    Ok(Some(current)) if current == *p => {
                        debug!("charge profile already set to {p}, skipping write");
                    }
                    Ok(_) => {
                        if let Err(e) = backend.set_profile(p) {
                            warn!("failed to set charge profile '{p}': {e}");
                            if e.kind() == std::io::ErrorKind::Other {
                                std::thread::sleep(std::time::Duration::from_millis(300));
                                if let Err(e2) = backend.set_profile(p) {
                                    warn!(
                                        "second attempt failed to set charge profile '{p}': {e2}"
                                    );
                                } else {
                                    info!("set_profile({p}) → Ok(()) on second attempt");
                                }
                            }
                        } else {
                            info!("set_profile({p}) → Ok(())");
                        }
                    }
                    Err(e) => {
                        warn!(
                            "failed to read current charge profile, attempting write '{p}' anyway: {e}"
                        );
                        if let Err(e) = backend.set_profile(p) {
                            warn!("failed to set charge profile '{p}' after read error: {e}");
                            if e.kind() == std::io::ErrorKind::Other {
                                std::thread::sleep(std::time::Duration::from_millis(300));
                                if let Err(e2) = backend.set_profile(p) {
                                    warn!(
                                        "second attempt failed to set charge profile '{p}' after read error: {e2}"
                                    );
                                } else {
                                    info!(
                                        "set_profile({p}) → Ok(()) on second attempt (after read error)"
                                    );
                                }
                            }
                        } else {
                            info!("set_profile({p}) → Ok(()) (after read error)");
                        }
                    }
                }
            }
            if let Some(ref p) = cs.priority {
                debug!("setting charge priority: {p}");
                match backend.get_priority() {
                    Ok(Some(current)) if current == *p => {
                        debug!("charge priority already set to {p}, skipping write");
                    }
                    Ok(_) => {
                        if let Err(e) = backend.set_priority(p) {
                            warn!("failed to set charge priority '{p}': {e}");
                            if e.kind() == std::io::ErrorKind::Other {
                                std::thread::sleep(std::time::Duration::from_millis(300));
                                if let Err(e2) = backend.set_priority(p) {
                                    warn!(
                                        "second attempt failed to set charge priority '{p}': {e2}"
                                    );
                                } else {
                                    info!("set_priority({p}) → Ok(()) on second attempt");
                                }
                            }
                        } else {
                            info!("set_priority({p}) → Ok(())");
                        }
                    }
                    Err(e) => {
                        warn!(
                            "failed to read current charge priority, attempting write '{p}' anyway: {e}"
                        );
                        if let Err(e) = backend.set_priority(p) {
                            warn!("failed to set charge priority '{p}' after read error: {e}");
                            if e.kind() == std::io::ErrorKind::Other {
                                std::thread::sleep(std::time::Duration::from_millis(300));
                                if let Err(e2) = backend.set_priority(p) {
                                    warn!(
                                        "second attempt failed to set charge priority '{p}' after read error: {e2}"
                                    );
                                } else {
                                    info!(
                                        "set_priority({p}) → Ok(()) on second attempt (after read error)"
                                    );
                                }
                            }
                        } else {
                            info!("set_priority({p}) → Ok(()) (after read error)");
                        }
                    }
                }
            }
            info!("applied charging settings from profile '{}'", profile.name);
        }

        // 5. Display brightness.
        if let Some(ref display) = self.display
            && let Some(brightness) = profile.display.brightness
        {
            if let Err(e) = display.set_brightness_percent(brightness as u32) {
                warn!("failed to set display brightness: {e}");
            }
            info!(
                "applied display brightness={}% from profile '{}'",
                brightness, profile.name
            );
        }

        // 6. TDP settings.
        if let Some(ref tdp) = self.tdp_backend
            && let Some(ref settings) = profile.tdp
        {
            if let Some(pl1) = settings.pl1
                && let Err(e) = tdp.set_pl1(pl1)
            {
                warn!("failed to set TDP PL1: {e}");
            }
            if let Some(pl2) = settings.pl2
                && let Err(e) = tdp.set_pl2(pl2)
            {
                warn!("failed to set TDP PL2: {e}");
            }
            info!("applied TDP settings from profile '{}'", profile.name);
        }

        // 7. GPU power settings (cTGP offset).
        if let Some(ref gpu) = self.gpu_backend
            && let Some(ref settings) = profile.gpu
        {
            if let Some(ctgp) = settings.ctgp_offset
                && let Err(e) = gpu.set_ctgp_offset(ctgp)
            {
                warn!("failed to set cTGP offset: {e}");
            }
            info!("applied GPU power settings from profile '{}'", profile.name);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charging::ChargingBackend;
    use tux_core::fan_curve::FanCurvePoint;
    use tux_core::profile::{CpuSettings, FanProfileSettings};

    fn test_profile_with_fan(mode: FanMode, enabled: bool) -> TuxProfile {
        TuxProfile {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: String::new(),
            is_default: false,
            fan: FanProfileSettings {
                enabled,
                mode,
                min_speed_percent: 20,
                max_speed_percent: 100,
                curve: vec![
                    FanCurvePoint {
                        temp: 50,
                        speed: 30,
                    },
                    FanCurvePoint {
                        temp: 80,
                        speed: 80,
                    },
                ],
                ..Default::default()
            },
            cpu: CpuSettings::default(),
            keyboard: Default::default(),
            display: Default::default(),
            charging: Default::default(),
            odm_profile: None,
            tdp: None,
            gpu: None,
        }
    }

    #[test]
    fn apply_sends_fan_config() {
        let (tx, rx) = watch::channel(FanConfig::default());
        let applier = ProfileApplier::new(tx, None, None, None, None, vec![], None);

        let profile = test_profile_with_fan(FanMode::CustomCurve, true);
        applier.apply(&profile).unwrap();

        let config = rx.borrow();
        assert_eq!(config.mode, FanMode::CustomCurve);
        assert_eq!(config.min_speed_percent, 20);
        assert_eq!(config.curve.len(), 2);
        assert_eq!(config.curve[0].temp, 50);
    }

    #[test]
    fn apply_disabled_fan_sets_auto() {
        let (tx, rx) = watch::channel(FanConfig::default());
        let applier = ProfileApplier::new(tx, None, None, None, None, vec![], None);

        let profile = test_profile_with_fan(FanMode::CustomCurve, false);
        applier.apply(&profile).unwrap();

        assert_eq!(rx.borrow().mode, FanMode::Auto);
    }

    #[test]
    fn apply_auto_mode() {
        let (tx, rx) = watch::channel(FanConfig::default());
        let applier = ProfileApplier::new(tx, None, None, None, None, vec![], None);

        let profile = test_profile_with_fan(FanMode::Auto, true);
        applier.apply(&profile).unwrap();

        assert_eq!(rx.borrow().mode, FanMode::Auto);
    }

    #[test]
    fn apply_charging_clevo_thresholds() {
        use crate::charging::clevo::ClevoCharging;
        use tux_core::mock::sysfs::MockSysfs;
        use tux_core::profile::ChargingSettings;

        let mock = MockSysfs::new();
        let base = mock.platform_dir("tuxedo-clevo");
        mock.create_attr("devices/platform/tuxedo-clevo/charge_start_threshold", "40");
        mock.create_attr("devices/platform/tuxedo-clevo/charge_end_threshold", "80");
        let backend: Arc<dyn ChargingBackend> = Arc::new(ClevoCharging::with_path(base));

        let (tx, _rx) = watch::channel(FanConfig::default());
        let applier =
            ProfileApplier::new(tx, Some(backend.clone()), None, None, None, vec![], None);

        let mut profile = test_profile_with_fan(FanMode::Auto, true);
        profile.charging = ChargingSettings {
            start_threshold: Some(20),
            end_threshold: Some(95),
            ..Default::default()
        };
        applier.apply(&profile).unwrap();

        assert_eq!(backend.get_start_threshold().unwrap(), 20);
        assert_eq!(backend.get_end_threshold().unwrap(), 95);
    }

    #[test]
    fn apply_charging_none_is_noop() {
        let (tx, _rx) = watch::channel(FanConfig::default());
        let applier = ProfileApplier::new(tx, None, None, None, None, vec![], None);

        let mut profile = test_profile_with_fan(FanMode::Auto, true);
        profile.charging = tux_core::profile::ChargingSettings {
            start_threshold: Some(20),
            end_threshold: Some(95),
            ..Default::default()
        };
        // Should not panic with no charging backend.
        applier.apply(&profile).unwrap();
    }

    #[test]
    fn apply_scales_profile_keyboard_brightness_to_hardware() {
        use crate::hid::{KeyboardLed, Rgb};
        use std::sync::{Arc, Mutex};

        #[derive(Default)]
        struct Calls {
            brightness: Option<u8>,
            flush_count: usize,
        }

        struct MockKb(Arc<Mutex<Calls>>);

        impl KeyboardLed for MockKb {
            fn set_brightness(&mut self, b: u8) -> std::io::Result<()> {
                self.0.lock().unwrap().brightness = Some(b);
                Ok(())
            }

            fn set_color(&mut self, _zone: u8, _color: Rgb) -> std::io::Result<()> {
                Ok(())
            }

            fn set_mode(&mut self, _mode: &str) -> std::io::Result<()> {
                Ok(())
            }

            fn zone_count(&self) -> u8 {
                1
            }

            fn turn_off(&mut self) -> std::io::Result<()> {
                Ok(())
            }

            fn turn_on(&mut self) -> std::io::Result<()> {
                Ok(())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                self.0.lock().unwrap().flush_count += 1;
                Ok(())
            }

            fn device_type(&self) -> &str {
                "mock"
            }

            fn available_modes(&self) -> Vec<String> {
                vec!["static".to_string()]
            }
        }

        let calls = Arc::new(Mutex::new(Calls::default()));
        let kb: SharedKeyboard = Arc::new(Mutex::new(Box::new(MockKb(calls.clone()))));

        let (tx, _rx) = watch::channel(FanConfig::default());
        let applier = ProfileApplier::new(tx, None, None, None, None, vec![kb], None);

        let mut profile = test_profile_with_fan(FanMode::Auto, true);
        profile.keyboard.brightness = 50;

        applier.apply(&profile).unwrap();

        let c = calls.lock().unwrap();
        assert_eq!(c.brightness, Some(127));
        assert_eq!(c.flush_count, 1);
    }

    struct MockCharging {
        profile: std::sync::Mutex<Option<String>>,
        priority: std::sync::Mutex<Option<String>>,
        profile_writes: std::sync::atomic::AtomicUsize,
        priority_writes: std::sync::atomic::AtomicUsize,
        fail_profile_read: bool,
        fail_priority_read: bool,
    }

    impl MockCharging {
        fn new(profile: Option<&str>, priority: Option<&str>) -> Self {
            Self {
                profile: std::sync::Mutex::new(profile.map(ToOwned::to_owned)),
                priority: std::sync::Mutex::new(priority.map(ToOwned::to_owned)),
                profile_writes: std::sync::atomic::AtomicUsize::new(0),
                priority_writes: std::sync::atomic::AtomicUsize::new(0),
                fail_profile_read: false,
                fail_priority_read: false,
            }
        }

        fn with_read_failures(fail_profile_read: bool, fail_priority_read: bool) -> Self {
            Self {
                profile: std::sync::Mutex::new(Some("balanced".to_string())),
                priority: std::sync::Mutex::new(Some("performance".to_string())),
                profile_writes: std::sync::atomic::AtomicUsize::new(0),
                priority_writes: std::sync::atomic::AtomicUsize::new(0),
                fail_profile_read,
                fail_priority_read,
            }
        }
    }

    impl ChargingBackend for MockCharging {
        fn get_start_threshold(&self) -> std::io::Result<u8> {
            Ok(0)
        }

        fn set_start_threshold(&self, _pct: u8) -> std::io::Result<()> {
            Ok(())
        }

        fn get_end_threshold(&self) -> std::io::Result<u8> {
            Ok(0)
        }

        fn set_end_threshold(&self, _pct: u8) -> std::io::Result<()> {
            Ok(())
        }

        fn get_profile(&self) -> std::io::Result<Option<String>> {
            if self.fail_profile_read {
                return Err(std::io::Error::other("profile read failed"));
            }
            Ok(self.profile.lock().unwrap().clone())
        }

        fn set_profile(&self, profile: &str) -> std::io::Result<()> {
            self.profile_writes
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            *self.profile.lock().unwrap() = Some(profile.to_string());
            Ok(())
        }

        fn get_priority(&self) -> std::io::Result<Option<String>> {
            if self.fail_priority_read {
                return Err(std::io::Error::other("priority read failed"));
            }
            Ok(self.priority.lock().unwrap().clone())
        }

        fn set_priority(&self, priority: &str) -> std::io::Result<()> {
            self.priority_writes
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            *self.priority.lock().unwrap() = Some(priority.to_string());
            Ok(())
        }
    }

    #[test]
    fn apply_charging_skips_redundant_profile_priority_writes() {
        let backend = Arc::new(MockCharging::new(Some("balanced"), Some("performance")));
        let charging: Arc<dyn ChargingBackend> = backend.clone();
        let (tx, _rx) = watch::channel(FanConfig::default());
        let applier = ProfileApplier::new(tx, Some(charging), None, None, None, vec![], None);

        let mut profile = test_profile_with_fan(FanMode::Auto, true);
        profile.charging.profile = Some("balanced".to_string());
        profile.charging.priority = Some("performance".to_string());

        applier.apply(&profile).unwrap();

        assert_eq!(
            backend
                .profile_writes
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
        assert_eq!(
            backend
                .priority_writes
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
    }

    #[test]
    fn apply_charging_attempts_writes_when_read_fails() {
        let backend = Arc::new(MockCharging::with_read_failures(true, true));
        let charging: Arc<dyn ChargingBackend> = backend.clone();
        let (tx, _rx) = watch::channel(FanConfig::default());
        let applier = ProfileApplier::new(tx, Some(charging), None, None, None, vec![], None);

        let mut profile = test_profile_with_fan(FanMode::Auto, true);
        profile.charging.profile = Some("high_capacity".to_string());
        profile.charging.priority = Some("charge_battery".to_string());

        applier.apply(&profile).unwrap();

        assert_eq!(
            backend
                .profile_writes
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
        assert_eq!(
            backend
                .priority_writes
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
    }

    #[test]
    fn apply_cpu_governor_and_tdp() {
        use crate::cpu::governor::CpuGovernor;
        use crate::cpu::tdp::EcTdp;
        use tux_core::device::TdpBounds;
        use tux_core::profile::TdpSettings;

        let tmp = tempfile::tempdir().unwrap();
        let cpu_dir = tmp.path().join("cpu");

        // Set up fake CPU sysfs
        for i in 0..2 {
            let cpu_i_dir = cpu_dir.join(format!("cpu{i}"));
            let cpufreq = cpu_i_dir.join("cpufreq");
            std::fs::create_dir_all(&cpufreq).unwrap();
            std::fs::write(cpufreq.join("scaling_governor"), "powersave\n").unwrap();
            std::fs::write(
                cpufreq.join("scaling_available_governors"),
                "performance powersave\n",
            )
            .unwrap();
            std::fs::write(
                cpufreq.join("energy_performance_preference"),
                "balance_performance\n",
            )
            .unwrap();
            std::fs::write(cpufreq.join("scaling_min_freq"), "400000\n").unwrap();
            std::fs::write(cpufreq.join("scaling_max_freq"), "2000000\n").unwrap();

            if i > 0 {
                std::fs::write(cpu_i_dir.join("online"), "1\n").unwrap();
            }
        }
        let intel = cpu_dir.join("intel_pstate");
        std::fs::create_dir_all(&intel).unwrap();
        std::fs::write(intel.join("no_turbo"), "0\n").unwrap();

        // Set up fake EC RAM for TDP
        let ec_dir = tmp.path().join("ec");
        std::fs::create_dir_all(&ec_dir).unwrap();
        let ec_data = vec![0u8; 0x0800];
        std::fs::write(ec_dir.join("ec_ram"), &ec_data).unwrap();

        let gov = Arc::new(CpuGovernor::with_path(&cpu_dir));
        let bounds = TdpBounds {
            pl1_min: 5,
            pl1_max: 28,
            pl2_min: 10,
            pl2_max: 40,
            pl4_min: None,
            pl4_max: None,
        };
        let tdp: Arc<dyn crate::cpu::tdp::TdpBackend> = Arc::new(EcTdp::with_path(&ec_dir, bounds));

        let (tx, _rx) = watch::channel(FanConfig::default());
        let applier = ProfileApplier::new(
            tx,
            None,
            Some(gov.clone()),
            Some(tdp.clone()),
            None,
            vec![],
            None,
        );

        let mut profile = test_profile_with_fan(FanMode::Auto, true);
        profile.cpu.governor = "performance".to_string();
        profile.cpu.energy_performance_preference = Some("power".to_string());
        profile.cpu.no_turbo = true;
        profile.cpu.online_cores = Some(1);
        profile.cpu.scaling_min_frequency = Some(800000);
        profile.cpu.scaling_max_frequency = Some(3500000);
        profile.tdp = Some(TdpSettings {
            pl1: Some(20),
            pl2: Some(35),
        });

        applier.apply(&profile).unwrap();

        assert_eq!(gov.get_governor().unwrap(), "performance");
        assert!(gov.get_no_turbo().unwrap());
        assert_eq!(
            std::fs::read_to_string(cpu_dir.join("cpu1/online"))
                .unwrap()
                .trim(),
            "0"
        );
        assert_eq!(
            std::fs::read_to_string(cpu_dir.join("cpu0/cpufreq/scaling_min_freq"))
                .unwrap()
                .trim(),
            "800000"
        );
        assert_eq!(
            std::fs::read_to_string(cpu_dir.join("cpu0/cpufreq/scaling_max_freq"))
                .unwrap()
                .trim(),
            "3500000"
        );
        assert_eq!(tdp.get_pl1().unwrap(), 20);
        assert_eq!(tdp.get_pl2().unwrap(), 35);
    }

    #[test]
    fn apply_cpu_invalid_negative_values_are_ignored() {
        use crate::cpu::governor::CpuGovernor;

        let tmp = tempfile::tempdir().unwrap();
        let cpu_dir = tmp.path().join("cpu");

        for i in 0..2 {
            let cpu_i_dir = cpu_dir.join(format!("cpu{i}"));
            let cpufreq = cpu_i_dir.join("cpufreq");
            std::fs::create_dir_all(&cpufreq).unwrap();
            std::fs::write(cpufreq.join("scaling_governor"), "powersave\n").unwrap();
            std::fs::write(
                cpufreq.join("scaling_available_governors"),
                "performance powersave\n",
            )
            .unwrap();
            std::fs::write(
                cpufreq.join("energy_performance_preference"),
                "balance_performance\n",
            )
            .unwrap();
            std::fs::write(cpufreq.join("scaling_min_freq"), "400000\n").unwrap();
            std::fs::write(cpufreq.join("scaling_max_freq"), "2000000\n").unwrap();

            if i > 0 {
                std::fs::write(cpu_i_dir.join("online"), "1\n").unwrap();
            }
        }

        let intel = cpu_dir.join("intel_pstate");
        std::fs::create_dir_all(&intel).unwrap();
        std::fs::write(intel.join("no_turbo"), "0\n").unwrap();

        let gov = Arc::new(CpuGovernor::with_path(&cpu_dir));
        let (tx, _rx) = watch::channel(FanConfig::default());
        let applier = ProfileApplier::new(tx, None, Some(gov), None, None, vec![], None);

        let mut profile = test_profile_with_fan(FanMode::Auto, true);
        profile.cpu.online_cores = Some(-1);
        profile.cpu.scaling_min_frequency = Some(-800000);
        profile.cpu.scaling_max_frequency = Some(-3500000);

        applier.apply(&profile).unwrap();

        assert_eq!(
            std::fs::read_to_string(cpu_dir.join("cpu1/online"))
                .unwrap()
                .trim(),
            "1"
        );
        assert_eq!(
            std::fs::read_to_string(cpu_dir.join("cpu0/cpufreq/scaling_min_freq"))
                .unwrap()
                .trim(),
            "400000"
        );
        assert_eq!(
            std::fs::read_to_string(cpu_dir.join("cpu0/cpufreq/scaling_max_freq"))
                .unwrap()
                .trim(),
            "2000000"
        );
    }

    #[test]
    fn apply_cpu_invalid_min_max_range_is_ignored() {
        use crate::cpu::governor::CpuGovernor;

        let tmp = tempfile::tempdir().unwrap();
        let cpu_dir = tmp.path().join("cpu");

        for i in 0..2 {
            let cpu_i_dir = cpu_dir.join(format!("cpu{i}"));
            let cpufreq = cpu_i_dir.join("cpufreq");
            std::fs::create_dir_all(&cpufreq).unwrap();
            std::fs::write(cpufreq.join("scaling_governor"), "powersave\n").unwrap();
            std::fs::write(
                cpufreq.join("scaling_available_governors"),
                "performance powersave\n",
            )
            .unwrap();
            std::fs::write(
                cpufreq.join("energy_performance_preference"),
                "balance_performance\n",
            )
            .unwrap();
            std::fs::write(cpufreq.join("scaling_min_freq"), "600000\n").unwrap();
            std::fs::write(cpufreq.join("scaling_max_freq"), "2200000\n").unwrap();

            if i > 0 {
                std::fs::write(cpu_i_dir.join("online"), "1\n").unwrap();
            }
        }

        let intel = cpu_dir.join("intel_pstate");
        std::fs::create_dir_all(&intel).unwrap();
        std::fs::write(intel.join("no_turbo"), "0\n").unwrap();

        let gov = Arc::new(CpuGovernor::with_path(&cpu_dir));
        let (tx, _rx) = watch::channel(FanConfig::default());
        let applier = ProfileApplier::new(tx, None, Some(gov), None, None, vec![], None);

        let mut profile = test_profile_with_fan(FanMode::Auto, true);
        profile.cpu.scaling_min_frequency = Some(3_500_000);
        profile.cpu.scaling_max_frequency = Some(800_000);

        applier.apply(&profile).unwrap();

        assert_eq!(
            std::fs::read_to_string(cpu_dir.join("cpu0/cpufreq/scaling_min_freq"))
                .unwrap()
                .trim(),
            "600000"
        );
        assert_eq!(
            std::fs::read_to_string(cpu_dir.join("cpu0/cpufreq/scaling_max_freq"))
                .unwrap()
                .trim(),
            "2200000"
        );
    }

    #[test]
    fn apply_gpu_power_settings() {
        use crate::gpu::GpuPowerBackend;
        use tux_core::profile::GpuSettings;

        struct MockGpuPower {
            value: std::sync::Mutex<u8>,
        }

        impl MockGpuPower {
            fn new() -> Self {
                Self {
                    value: std::sync::Mutex::new(0),
                }
            }
        }

        impl GpuPowerBackend for MockGpuPower {
            fn get_ctgp_offset(&self) -> std::io::Result<u8> {
                Ok(*self.value.lock().unwrap())
            }
            fn set_ctgp_offset(&self, watts: u8) -> std::io::Result<()> {
                *self.value.lock().unwrap() = watts;
                Ok(())
            }
        }

        let mock: Arc<dyn GpuPowerBackend> = Arc::new(MockGpuPower::new());
        let (tx, _rx) = watch::channel(FanConfig::default());
        let applier = ProfileApplier::new(tx, None, None, None, Some(mock.clone()), vec![], None);

        let mut profile = test_profile_with_fan(FanMode::Auto, true);
        profile.gpu = Some(GpuSettings {
            ctgp_offset: Some(15),
        });

        applier.apply(&profile).unwrap();
        assert_eq!(mock.get_ctgp_offset().unwrap(), 15);
    }

    #[test]
    fn apply_gpu_none_is_noop() {
        let (tx, _rx) = watch::channel(FanConfig::default());
        let applier = ProfileApplier::new(tx, None, None, None, None, vec![], None);

        let mut profile = test_profile_with_fan(FanMode::Auto, true);
        profile.gpu = Some(tux_core::profile::GpuSettings {
            ctgp_offset: Some(10),
        });
        // Should not panic with no GPU backend.
        applier.apply(&profile).unwrap();
    }
}
