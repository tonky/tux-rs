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

        // 2. CPU governor + EPP + turbo.
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
            info!(
                "applied CPU settings from profile '{}': governor={}, no_turbo={}",
                profile.name, profile.cpu.governor, profile.cpu.no_turbo
            );
        }

        // 3. Keyboard backlight brightness.
        if !self.keyboards.is_empty() {
            let brightness = profile.keyboard.brightness;
            // Profile stores brightness as 0–255; forward directly to hardware.
            for kb_lock in &self.keyboards {
                if let Ok(mut kb) = kb_lock.lock() {
                    if let Err(e) = kb.set_brightness(brightness) {
                        warn!("failed to set keyboard brightness: {e}");
                    }
                    if let Err(e) = kb.flush() {
                        warn!("failed to flush keyboard state: {e}");
                    }
                }
            }
            info!(
                "applied keyboard brightness={} from profile '{}'",
                brightness, profile.name
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
                let result = backend.set_profile(p);
                info!("set_profile({p}) → {result:?}");
            }
            if let Some(ref p) = cs.priority {
                debug!("setting charge priority: {p}");
                let result = backend.set_priority(p);
                info!("set_priority({p}) → {result:?}");
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
    fn apply_cpu_governor_and_tdp() {
        use crate::cpu::governor::CpuGovernor;
        use crate::cpu::tdp::EcTdp;
        use tux_core::device::TdpBounds;
        use tux_core::profile::TdpSettings;

        let tmp = tempfile::tempdir().unwrap();
        let cpu_dir = tmp.path().join("cpu");

        // Set up fake CPU sysfs
        for i in 0..2 {
            let cpufreq = cpu_dir.join(format!("cpu{i}/cpufreq"));
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
        profile.tdp = Some(TdpSettings {
            pl1: Some(20),
            pl2: Some(35),
        });

        applier.apply(&profile).unwrap();

        assert_eq!(gov.get_governor().unwrap(), "performance");
        assert!(gov.get_no_turbo().unwrap());
        assert_eq!(tdp.get_pl1().unwrap(), 20);
        assert_eq!(tdp.get_pl2().unwrap(), 35);
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
