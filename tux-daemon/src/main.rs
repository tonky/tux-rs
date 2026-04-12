use std::path::Path;
use std::sync::{Arc, RwLock};

use tokio::sync::{broadcast, watch};
use tracing::{error, info};

use tux_daemon::charging;
use tux_daemon::config::{self, DaemonConfig};
use tux_daemon::cpu;
use tux_daemon::dbus;
use tux_daemon::display;
use tux_daemon::display::SharedDisplay;
use tux_daemon::fan_engine;
use tux_daemon::gpu;
use tux_daemon::hid;
use tux_daemon::platform;
use tux_daemon::power_monitor::{PowerState, PowerStateMonitor};
use tux_daemon::profile_apply::ProfileApplier;
use tux_daemon::profile_store::ProfileStore;
use tux_daemon::sleep;

mod client_tracker;
mod lifecycle;
mod tcc_import;

use client_tracker::ClientTracker;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Handle TCC profile/settings import mode (called via pkexec from TCC GUI).
    if tcc_import::is_import_mode() {
        return tcc_import::run_import();
    }
    // Handle hardware spec extraction for debugging and custom overriding.
    if std::env::args().any(|a| a == "--dump-hardware-spec" || a == "--dump-dmi") {
        let dmi_source = tux_core::dmi::SysFsDmiSource;
        match tux_core::dmi::read_dmi_info(&dmi_source) {
            Ok(dmi) => {
                println!("--- TUXEDO Hardware Spec ---");
                println!("board_vendor    = {}", dmi.board_vendor);
                println!("board_name      = {}", dmi.board_name);
                println!("product_sku     = {}", dmi.product_sku);
                println!("sys_vendor      = {}", dmi.sys_vendor);
                println!("product_name    = {}", dmi.product_name);
                println!("product_version = {}", dmi.product_version);
                println!("----------------------------");
                return Ok(());
            }
            Err(e) => {
                eprintln!("Failed to read DMI info: {}", e);
                std::process::exit(1);
            }
        }
    }

    let debug_mode = std::env::args().any(|a| a == "--debug" || a == "-d");
    let mock_mode = std::env::args().any(|a| a == "--mock");

    // 1. Load config (before tracing, so we can set the log level).
    let config = if mock_mode {
        DaemonConfig::default()
    } else {
        DaemonConfig::load(Path::new(config::DEFAULT_CONFIG_PATH))
    };
    let daemon_config_arc = Arc::new(RwLock::new(config.clone()));

    // 2. Initialize tracing.
    //    Priority: RUST_LOG env > --debug flag > config file > default (info).
    use tracing_subscriber::EnvFilter;
    let env_filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else if debug_mode {
        EnvFilter::new("tux_daemon=debug,tux_core=debug")
    } else {
        let level = &config.daemon.log_level;
        EnvFilter::new(level)
    };
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    if debug_mode {
        info!("debug mode enabled via --debug flag");
    }

    info!("tux-daemon v{} starting", tux_core::version());
    if mock_mode {
        info!("mock mode enabled via --mock flag");
    }

    // 2b. Load custom devices configuration if present.
    let custom_devices_path = Path::new("/etc/tux-daemon/custom_devices.toml");
    if custom_devices_path.exists() {
        info!("loading custom devices from {:?}", custom_devices_path);
        match std::fs::read_to_string(custom_devices_path) {
            Ok(content) => {
                #[derive(serde::Deserialize)]
                struct CustomDevicesFile {
                    #[serde(default)]
                    device: Vec<tux_core::custom_device::CustomDeviceDescriptor>,
                }
                match toml::from_str::<CustomDevicesFile>(&content) {
                    Ok(wrapper) => {
                        for custom_dev in wrapper.device {
                            info!(
                                "registered custom device override: {} ({})",
                                custom_dev.name, custom_dev.product_sku
                            );
                            let leaked = custom_dev.leak();
                            tux_core::device_table::register_custom_device(leaked);
                        }
                    }
                    Err(e) => error!("failed to parse custom devices TOML: {e}"),
                }
            }
            Err(e) => error!("failed to read custom devices file: {e}"),
        }
    }

    // 3. Detect device.
    let device = if mock_mode {
        let source = tux_core::mock::dmi::MockDmiSource::new().tuxedo_base("PULSE1403");
        tux_core::dmi::detect_device(&source).expect("mock detection should not fail")
    } else {
        let dmi_source = tux_core::dmi::SysFsDmiSource;
        match tux_core::dmi::detect_device(&dmi_source) {
            Ok(dev) => {
                info!(
                    "detected: {} (platform={:?}, exact={})",
                    dev.descriptor.name, dev.descriptor.platform, dev.exact_match
                );
                dev
            }
            Err(e) => {
                error!("failed to detect device: {e}");
                anyhow::bail!("cannot detect TUXEDO device: {e}");
            }
        }
    };

    // 4. Initialize fan backend.
    let backend = if mock_mode {
        Some(Arc::new(tux_core::mock::fan::MockFanBackend::new(
            device.descriptor.fans.count,
        )) as Arc<dyn tux_core::backend::fan::FanBackend>)
    } else {
        platform::init_fan_backend(&device)
    };

    // 5. Safety reset: restore all fans to auto before applying any custom curve.
    //    This ensures safe state even after a crash + restart.
    if let Some(ref backend) = backend {
        let n = backend.num_fans();
        for i in 0..n {
            if let Err(e) = backend.set_auto(i) {
                error!("failed to set auto mode for fan {i} on startup: {e}");
            }
        }
        info!("startup safety reset: all {n} fans restored to auto");
    }

    // 6. Create channels.
    let (config_tx, config_rx) = watch::channel(config.fan.clone());
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // 6a. Load profile store (behind RwLock for D-Bus CRUD).
    let profile_store = Arc::new(RwLock::new(ProfileStore::new("/etc/tux-daemon/profiles")?));
    {
        let store = profile_store
            .read()
            .map_err(|e| anyhow::anyhow!("profile store lock poisoned at startup: {e}"))?;
        info!("loaded {} profiles", store.list().len());

        if store.get(&config.profiles.ac_profile).is_none() {
            tracing::warn!(
                "configured ac_profile '{}' not found in store",
                config.profiles.ac_profile
            );
        }
        if store.get(&config.profiles.battery_profile).is_none() {
            tracing::warn!(
                "configured battery_profile '{}' not found in store",
                config.profiles.battery_profile
            );
        }
    }

    // 6b. Create profile assignments watch channel and applier.
    let (assignments_tx, assignments_rx) = watch::channel(config.profiles.clone());
    // Keep a clone for the SIGHUP handler to trigger re-apply after profile reload.
    let reload_assignments_tx = assignments_tx.clone();

    // 6b-i. Discover charging backend based on device capability.
    let charging_backend: Option<Arc<dyn charging::ChargingBackend>> =
        match device.descriptor.charging {
            tux_core::device::ChargingCapability::Flexicharger => {
                match charging::clevo::ClevoCharging::new() {
                    Some(b) => {
                        info!("Clevo flexicharger backend available");
                        Some(Arc::new(b))
                    }
                    None => {
                        info!("Clevo flexicharger sysfs not available");
                        None
                    }
                }
            }
            tux_core::device::ChargingCapability::EcProfilePriority => {
                match charging::uniwill::UniwillCharging::new() {
                    Some(b) => {
                        info!("Uniwill EC charging backend available");
                        Some(Arc::new(b))
                    }
                    None => {
                        info!("Uniwill charging sysfs not available");
                        None
                    }
                }
            }
            tux_core::device::ChargingCapability::None => {
                info!("no charging control for this platform");
                None
            }
        };

    if let Some(ref cb) = charging_backend
        && let Some(ref chg_settings) = config.charging
    {
        let mut errs = vec![];
        if let Some(start) = chg_settings.start_threshold
            && let Err(e) = cb.set_start_threshold(start)
        {
            errs.push(e.to_string());
        }
        if let Some(end) = chg_settings.end_threshold
            && let Err(e) = cb.set_end_threshold(end)
        {
            errs.push(e.to_string());
        }
        if let Some(ref profile) = chg_settings.profile
            && let Err(e) = cb.set_profile(profile)
        {
            errs.push(e.to_string());
        }
        if let Some(ref priority) = chg_settings.priority
            && let Err(e) = cb.set_priority(priority)
        {
            errs.push(e.to_string());
        }
        if !errs.is_empty() {
            error!("failed to apply saved charging settings: {:?}", errs);
        } else {
            info!("applied saved charging settings");
        }
    }

    // 6b-ii. Create CPU governor backend (available on all platforms with cpufreq).
    let cpu_governor: Option<Arc<cpu::governor::CpuGovernor>> = {
        let gov = cpu::governor::CpuGovernor::new();
        if cpu::governor::cpu_governor_available(std::path::Path::new("/sys/devices/system/cpu")) {
            info!("CPU governor control available");
            Some(Arc::new(gov))
        } else {
            info!("CPU governor control not available");
            None
        }
    };

    // 6b-iii. Create TDP backend (NB05 platforms with EC RAM + tdp bounds).
    let tdp_backend: Option<Arc<dyn cpu::tdp::TdpBackend>> =
        if let Some(bounds) = device.descriptor.tdp {
            match cpu::tdp::EcTdp::new(bounds) {
                Some(b) => {
                    info!(
                        "EC TDP backend available (PL1: {}-{}W, PL2: {}-{}W)",
                        bounds.pl1_min, bounds.pl1_max, bounds.pl2_min, bounds.pl2_max
                    );
                    Some(Arc::new(b))
                }
                None => {
                    info!("EC TDP sysfs not available");
                    None
                }
            }
        } else {
            info!("no TDP control for this platform");
            None
        };

    // 6b-iv. Create GPU power backend (NB02 Uniwill platforms with NVIDIA).
    let gpu_backend: Option<Arc<dyn gpu::GpuPowerBackend>> = match device.descriptor.gpu_power {
        tux_core::device::GpuPowerCapability::Nb02Nvidia => match gpu::nb02::Nb02GpuPower::new() {
            Some(b) => {
                info!("NB02 NVIDIA GPU power backend available");
                Some(Arc::new(b))
            }
            None => {
                info!("NB02 NVIDIA sysfs not available");
                None
            }
        },
        tux_core::device::GpuPowerCapability::None => {
            info!("no GPU power control for this platform");
            None
        }
    };

    // 5a. Discover ITE HID keyboards (before ProfileApplier so it can reference them).
    let mut raw_keyboards =
        hid::discover::discover_keyboards_for_device(device.descriptor.product_sku);
    if raw_keyboards.is_empty() {
        info!("no ITE HID keyboards discovered, trying sysfs");
        raw_keyboards = hid::discover::discover_sysfs_keyboards();
    }
    if raw_keyboards.is_empty() {
        info!("no keyboards discovered");
    } else {
        info!("discovered {} keyboard(s)", raw_keyboards.len());
    }
    let keyboards = hid::wrap_keyboards(raw_keyboards);

    // 6b2. Discover display backlight controllers.
    let display_backlight: Option<SharedDisplay> = {
        let display = display::DisplayBacklight::discover();
        if display.is_available() {
            Some(Arc::new(display))
        } else {
            None
        }
    };

    let applier = Arc::new(ProfileApplier::new(
        config_tx.clone(),
        charging_backend.clone(),
        cpu_governor.clone(),
        tdp_backend.clone(),
        gpu_backend.clone(),
        keyboards.clone(),
        display_backlight.clone(),
    ));

    // 6c. Start power state monitor and auto-switch task.
    //     We detect the initial state inside the monitor to avoid a race.
    //     If power monitoring isn't available, use a static Ac state for D-Bus.
    let power_rx = match PowerStateMonitor::new(None) {
        Ok((monitor, mut monitor_power_rx)) => {
            // Apply the initial profile from the monitor's detected state.
            let initial_state = *monitor_power_rx.borrow();
            let initial_profile_id = match initial_state {
                PowerState::Battery => &config.profiles.battery_profile,
                PowerState::Ac => &config.profiles.ac_profile,
            };
            {
                let store = profile_store.read().map_err(|e| {
                    anyhow::anyhow!("profile store lock poisoned during initial apply: {e}")
                })?;
                if let Some(profile) = store.get(initial_profile_id) {
                    if let Err(e) = applier.apply(profile) {
                        error!("failed to apply initial profile '{initial_profile_id}': {e}");
                    } else {
                        info!(
                            "applied initial profile '{}' (power: {:?})",
                            profile.name, initial_state
                        );
                    }
                } else {
                    tracing::warn!(
                        "initial profile '{initial_profile_id}' not found, using config defaults"
                    );
                }
            }

            // Clone a receiver for D-Bus before moving into the auto-switch task.
            let dbus_power_rx = monitor_power_rx.clone();

            let monitor_shutdown = shutdown_tx.subscribe();
            tokio::spawn(async move {
                monitor.run(monitor_shutdown).await;
            });

            // Spawn the auto-switch handler.
            let store = profile_store.clone();
            let applier_for_switch = applier.clone();
            let switch_assignments_rx = assignments_rx.clone();
            let switch_shutdown = shutdown_tx.subscribe();
            tokio::spawn(async move {
                auto_switch_loop(
                    &mut monitor_power_rx,
                    switch_assignments_rx,
                    &store,
                    &applier_for_switch,
                    switch_shutdown,
                )
                .await;
            });
            info!("power auto-switch enabled");

            dbus_power_rx
        }
        Err(e) => {
            info!("power monitoring unavailable: {e}, auto-switch disabled");
            let (_tx, rx) = watch::channel(PowerState::Ac);
            rx
        }
    };

    // 7. Create client tracker for active/idle polling.
    let _client_tracker = Arc::new(ClientTracker::new(std::time::Duration::from_secs(
        config.daemon.idle_timeout_s,
    )));

    // 8. Spawn fan curve engine (if fan control is available).
    let fan_failure_counter: std::sync::Arc<std::sync::atomic::AtomicU32>;
    let engine_handle = if let Some(ref backend) = backend {
        let mut engine = fan_engine::FanCurveEngine::new(backend.clone(), config_rx.clone());
        fan_failure_counter = engine.failure_counter();
        let shutdown_rx = shutdown_tx.subscribe();
        let handle = tokio::spawn(async move {
            engine.run(shutdown_rx).await;
        });
        info!("fan curve engine started");
        Some(handle)
    } else {
        fan_failure_counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        info!("no fan backend for this platform, fan control disabled");
        None
    };

    // 9. Register D-Bus service.
    let _conn = dbus::serve_on_bus(dbus::DbusConfig {
        bus_type: dbus::BusType::System,
        device: &device,
        fan_backend: backend.clone(),
        keyboards,
        charging: charging_backend,
        cpu_governor,
        tdp_backend,
        gpu_backend,
        display: display_backlight,
        config_tx: config_tx.clone(),
        config_rx: config_rx.clone(),
        store: profile_store.clone(),
        assignments_tx,
        assignments_rx,
        applier,
        power_rx,
        daemon_config: daemon_config_arc,
        fan_failure_counter,
    })
    .await?;

    // 10a. Spawn sleep monitor for suspend/resume handling.
    {
        let sleep_handler = Arc::new(sleep::SleepHandler::new(
            backend.clone(),
            config_tx,
            config_rx,
            vec![], // Keyboards are owned by D-Bus; USB re-enumerates on resume
        ));
        let sleep_shutdown = shutdown_tx.subscribe();
        tokio::spawn(async move {
            sleep::monitor_sleep(sleep_handler, sleep_shutdown).await;
        });
    }

    // 10b. Notify systemd we're ready (no-op without the "systemd" feature).
    #[cfg(feature = "systemd")]
    let _ = sd_notify::notify(&[sd_notify::NotifyState::Ready]);
    info!("daemon ready");

    // 11. Spawn watchdog ping task (if systemd requests it).
    #[cfg(feature = "systemd")]
    if let Some(wd_duration) = sd_notify::watchdog_enabled() {
        let interval = wd_duration / 2;
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                let _ = sd_notify::notify(&[sd_notify::NotifyState::Watchdog]);
            }
        });
    }

    // 12. Wait for SIGINT, SIGTERM, or SIGHUP.
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())?;
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => { info!("received SIGINT, shutting down"); break; },
            _ = sigterm.recv() => { info!("received SIGTERM, shutting down"); break; },
            _ = sighup.recv() => {
                info!("received SIGHUP, reloading profiles");
                match ProfileStore::new("/etc/tux-daemon/profiles") {
                    Ok(new_store) => {
                        if let Ok(mut store) = profile_store.write() {
                            *store = new_store;
                            let count = store.list().len();
                            info!("reloaded {count} profiles");
                        }
                        // Notify auto_switch_loop to re-apply the active profile
                        // with the updated data.
                        reload_assignments_tx.send_modify(|_| {});
                    }
                    Err(e) => error!("failed to reload profiles: {e}"),
                }
            },
        }
    }

    // 13. Graceful shutdown.
    #[cfg(feature = "systemd")]
    let _ = sd_notify::notify(&[sd_notify::NotifyState::Stopping]);
    let _ = shutdown_tx.send(());

    // Wait for engine to finish (it restores auto mode itself).
    if let Some(handle) = engine_handle {
        let _ = tokio::time::timeout(tokio::time::Duration::from_secs(5), handle).await;
    }

    // Belt-and-suspenders: restore auto in case engine timed out or crashed.
    if let Some(ref backend) = backend {
        for i in 0..backend.num_fans() {
            let _ = backend.set_auto(i);
        }
    }

    info!("tux-daemon stopped");
    Ok(())
}

/// Run the auto-switch loop: listen for power state changes or assignment
/// changes and apply the corresponding profile.
async fn auto_switch_loop(
    power_rx: &mut watch::Receiver<PowerState>,
    mut assignments_rx: watch::Receiver<config::ProfileAssignments>,
    store: &Arc<RwLock<ProfileStore>>,
    applier: &ProfileApplier,
    mut shutdown: broadcast::Receiver<()>,
) {
    let mut apply_count: u64 = 0;
    loop {
        tokio::select! {
            result = power_rx.changed() => {
                if result.is_err() {
                    return;
                }
                apply_count += 1;
                let state = *power_rx.borrow();
                let profile_id = resolve_profile_id(&assignments_rx, state);
                tracing::debug!("auto_switch_loop: power_rx fired (apply #{apply_count}), state={state:?}, profile={profile_id}");
                apply_profile_by_id(store, applier, &profile_id, &format!("power state → {state:?} (apply #{apply_count})"));
            }
            result = assignments_rx.changed() => {
                if result.is_err() {
                    return;
                }
                apply_count += 1;
                let state = *power_rx.borrow();
                let profile_id = resolve_profile_id(&assignments_rx, state);
                tracing::debug!("auto_switch_loop: assignments_rx fired (apply #{apply_count}), state={state:?}, profile={profile_id}");
                apply_profile_by_id(store, applier, &profile_id, &format!("assignment change (apply #{apply_count})"));
            }
            _ = shutdown.recv() => {
                return;
            }
        }
    }
}

fn resolve_profile_id(
    assignments_rx: &watch::Receiver<config::ProfileAssignments>,
    state: PowerState,
) -> String {
    let assignments = assignments_rx.borrow().clone();
    match state {
        PowerState::Ac => assignments.ac_profile,
        PowerState::Battery => assignments.battery_profile,
    }
}

fn apply_profile_by_id(
    store: &Arc<RwLock<ProfileStore>>,
    applier: &ProfileApplier,
    profile_id: &str,
    reason: &str,
) {
    let Ok(store) = store.read() else {
        error!("profile store lock poisoned, cannot apply profile");
        return;
    };
    match store.get(profile_id) {
        Some(profile) => {
            if let Err(e) = applier.apply(profile) {
                error!("failed to apply profile '{profile_id}' on {reason}: {e}");
            } else {
                info!("{reason}: applied profile '{}'", profile.name);
            }
        }
        None => {
            tracing::warn!(
                "profile '{profile_id}' not found on {reason}, keeping current settings"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tux_core::fan_curve::{FanConfig, FanMode};

    #[tokio::test(flavor = "multi_thread")]
    async fn auto_switch_ac_to_battery_applies_profile() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(RwLock::new(ProfileStore::new(dir.path()).unwrap()));

        let (config_tx, config_rx) = watch::channel(FanConfig::default());
        let applier = Arc::new(ProfileApplier::new(
            config_tx,
            None,
            None,
            None,
            None,
            vec![],
            None,
        ));

        let assignments = config::ProfileAssignments::default();
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let shutdown_rx = shutdown_tx.subscribe();
        let (_assignments_tx, assignments_rx) = watch::channel(assignments);

        let (power_tx, mut power_rx) = watch::channel(PowerState::Ac);

        let store2 = store.clone();
        let applier2 = applier.clone();
        let assignments_rx2 = assignments_rx.clone();
        let handle = tokio::spawn(async move {
            auto_switch_loop(
                &mut power_rx,
                assignments_rx2,
                &store2,
                &applier2,
                shutdown_rx,
            )
            .await;
        });

        power_tx.send(PowerState::Battery).unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let config = config_rx.borrow();
        assert_eq!(config.mode, FanMode::CustomCurve);
        assert_eq!(config.min_speed_percent, 0);

        shutdown_tx.send(()).unwrap();
        let _ = tokio::time::timeout(tokio::time::Duration::from_secs(1), handle).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn auto_switch_unknown_profile_no_crash() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(RwLock::new(ProfileStore::new(dir.path()).unwrap()));

        let (config_tx, _config_rx) = watch::channel(FanConfig::default());
        let applier = Arc::new(ProfileApplier::new(
            config_tx,
            None,
            None,
            None,
            None,
            vec![],
            None,
        ));

        let assignments = config::ProfileAssignments {
            ac_profile: "nonexistent_profile".to_string(),
            battery_profile: "also_nonexistent".to_string(),
        };
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let shutdown_rx = shutdown_tx.subscribe();
        let (_assignments_tx, assignments_rx) = watch::channel(assignments);

        let (power_tx, mut power_rx) = watch::channel(PowerState::Ac);

        let store2 = store.clone();
        let applier2 = applier.clone();
        let assignments_rx2 = assignments_rx.clone();
        let handle = tokio::spawn(async move {
            auto_switch_loop(
                &mut power_rx,
                assignments_rx2,
                &store2,
                &applier2,
                shutdown_rx,
            )
            .await;
        });

        power_tx.send(PowerState::Battery).unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        power_tx.send(PowerState::Ac).unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        shutdown_tx.send(()).unwrap();
        let _ = tokio::time::timeout(tokio::time::Duration::from_secs(1), handle).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn auto_switch_battery_to_ac_applies_profile() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(RwLock::new(ProfileStore::new(dir.path()).unwrap()));

        let (config_tx, config_rx) = watch::channel(FanConfig::default());
        let applier = Arc::new(ProfileApplier::new(
            config_tx,
            None,
            None,
            None,
            None,
            vec![],
            None,
        ));

        let assignments = config::ProfileAssignments::default();
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let shutdown_rx = shutdown_tx.subscribe();
        let (_assignments_tx, assignments_rx) = watch::channel(assignments);

        let (power_tx, mut power_rx) = watch::channel(PowerState::Battery);

        let store2 = store.clone();
        let applier2 = applier.clone();
        let assignments_rx2 = assignments_rx.clone();
        let handle = tokio::spawn(async move {
            auto_switch_loop(
                &mut power_rx,
                assignments_rx2,
                &store2,
                &applier2,
                shutdown_rx,
            )
            .await;
        });

        power_tx.send(PowerState::Ac).unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let config = config_rx.borrow();
        assert_eq!(config.mode, FanMode::CustomCurve);
        assert_eq!(config.min_speed_percent, 20);

        shutdown_tx.send(()).unwrap();
        let _ = tokio::time::timeout(tokio::time::Duration::from_secs(1), handle).await;
    }

    /// Regression: changing assignments (not just power state) should trigger
    /// profile apply via `assignments_rx.changed()`.
    #[tokio::test(flavor = "multi_thread")]
    async fn auto_switch_assignment_change_applies_profile() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(RwLock::new(ProfileStore::new(dir.path()).unwrap()));

        let (config_tx, config_rx) = watch::channel(FanConfig::default());
        let applier = Arc::new(ProfileApplier::new(
            config_tx,
            None,
            None,
            None,
            None,
            vec![],
            None,
        ));

        let assignments = config::ProfileAssignments::default();
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let shutdown_rx = shutdown_tx.subscribe();
        let (assignments_tx, assignments_rx) = watch::channel(assignments);

        let (_power_tx, mut power_rx) = watch::channel(PowerState::Ac);

        let store2 = store.clone();
        let applier2 = applier.clone();
        let assignments_rx2 = assignments_rx.clone();
        let handle = tokio::spawn(async move {
            auto_switch_loop(
                &mut power_rx,
                assignments_rx2,
                &store2,
                &applier2,
                shutdown_rx,
            )
            .await;
        });

        // config_rx starts with default config = the __default_ac__ profile's config.
        // Now change the assignment to __max_energy_save__ which has min_speed=0.
        assignments_tx
            .send(config::ProfileAssignments {
                ac_profile: "__max_energy_save__".to_string(),
                battery_profile: "__max_energy_save__".to_string(),
            })
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let config = config_rx.borrow();
        // Max Energy Save has min_speed_percent=0, mode=CustomCurve
        assert_eq!(config.mode, FanMode::CustomCurve);
        assert_eq!(config.min_speed_percent, 0);

        shutdown_tx.send(()).unwrap();
        let _ = tokio::time::timeout(tokio::time::Duration::from_secs(1), handle).await;
    }
}
