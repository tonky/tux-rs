//! Test daemon helper: starts a daemon on the session bus for integration testing.
//!
//! Provides `TestDaemon` which wires up all backends with mock sysfs
//! and exposes a D-Bus connection for method calls.

use std::sync::{Arc, RwLock};

use tokio::sync::{broadcast, watch};
use zbus::Connection;

use tux_core::backend::fan::FanBackend;
use tux_core::device::KeyboardType;
use tux_core::dmi::DetectedDevice;
use tux_core::fan_curve::FanConfig;
use tux_core::mock::fan::MockFanBackend;

use tux_daemon::charging::ChargingBackend;
use tux_daemon::fan_engine::FanCurveEngine;
use tux_daemon::hid::{KeyboardLed, Rgb, SharedKeyboard};

struct MockKeyboard;

impl KeyboardLed for MockKeyboard {
    fn set_brightness(&mut self, _brightness: u8) -> std::io::Result<()> {
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
        Ok(())
    }

    fn device_type(&self) -> &str {
        "mock"
    }

    fn available_modes(&self) -> Vec<String> {
        vec!["static".into()]
    }
}

/// A test daemon running on the session bus with mock backends.
pub struct TestDaemon {
    #[allow(dead_code)]
    pub connection: Connection,
    #[allow(dead_code)]
    pub fan_backend: Arc<MockFanBackend>,
    shutdown_tx: broadcast::Sender<()>,
    engine_handle: Option<tokio::task::JoinHandle<()>>,
}

/// A mock charging backend for E2E testing.
#[allow(dead_code)]
pub struct MockChargingBackend {
    start: std::sync::Mutex<u8>,
    end: std::sync::Mutex<u8>,
    profile: std::sync::Mutex<Option<String>>,
    priority: std::sync::Mutex<Option<String>>,
}

#[allow(dead_code)]
impl MockChargingBackend {
    pub fn new(start: u8, end: u8) -> Self {
        Self {
            start: std::sync::Mutex::new(start),
            end: std::sync::Mutex::new(end),
            profile: std::sync::Mutex::new(None),
            priority: std::sync::Mutex::new(None),
        }
    }

    pub fn with_profile(mut self, profile: &str, priority: &str) -> Self {
        self.profile = std::sync::Mutex::new(Some(profile.to_string()));
        self.priority = std::sync::Mutex::new(Some(priority.to_string()));
        self
    }
}

impl ChargingBackend for MockChargingBackend {
    fn get_start_threshold(&self) -> std::io::Result<u8> {
        Ok(*self.start.lock().unwrap())
    }
    fn set_start_threshold(&self, pct: u8) -> std::io::Result<()> {
        *self.start.lock().unwrap() = pct;
        Ok(())
    }
    fn get_end_threshold(&self) -> std::io::Result<u8> {
        Ok(*self.end.lock().unwrap())
    }
    fn set_end_threshold(&self, pct: u8) -> std::io::Result<()> {
        *self.end.lock().unwrap() = pct;
        Ok(())
    }
    fn get_profile(&self) -> std::io::Result<Option<String>> {
        Ok(self.profile.lock().unwrap().clone())
    }
    fn set_profile(&self, profile: &str) -> std::io::Result<()> {
        *self.profile.lock().unwrap() = Some(profile.to_string());
        Ok(())
    }
    fn get_priority(&self) -> std::io::Result<Option<String>> {
        Ok(self.priority.lock().unwrap().clone())
    }
    fn set_priority(&self, priority: &str) -> std::io::Result<()> {
        *self.priority.lock().unwrap() = Some(priority.to_string());
        Ok(())
    }
}

/// Builder for configuring a TestDaemon with optional backends.
#[allow(dead_code)]
pub struct TestDaemonBuilder<'a> {
    device: &'a DetectedDevice,
    profile_dir: &'a std::path::Path,
    charging: Option<Arc<dyn ChargingBackend>>,
    tdp: Option<Arc<dyn tux_daemon::cpu::tdp::TdpBackend>>,
}

#[allow(dead_code)]
impl<'a> TestDaemonBuilder<'a> {
    pub fn new(device: &'a DetectedDevice, profile_dir: &'a std::path::Path) -> Self {
        Self {
            device,
            profile_dir,
            charging: None,
            tdp: None,
        }
    }

    pub fn with_charging(mut self, backend: Arc<dyn ChargingBackend>) -> Self {
        self.charging = Some(backend);
        self
    }

    pub fn with_tdp(mut self, backend: Arc<dyn tux_daemon::cpu::tdp::TdpBackend>) -> Self {
        self.tdp = Some(backend);
        self
    }

    pub async fn build(self) -> TestDaemon {
        TestDaemon::start_with_options(self.device, self.profile_dir, self.charging, self.tdp).await
    }
}

impl TestDaemon {
    /// Start a test daemon on the session bus with the given detected device.
    ///
    /// Creates a mock fan backend, profile store (in a temp dir), and registers
    /// all D-Bus interfaces on the session bus.
    #[allow(dead_code)]
    pub async fn start(device: &DetectedDevice, profile_dir: &std::path::Path) -> Self {
        Self::start_with_options(device, profile_dir, None, None).await
    }

    /// Start with optional additional backends.
    async fn start_with_options(
        device: &DetectedDevice,
        profile_dir: &std::path::Path,
        charging: Option<Arc<dyn ChargingBackend>>,
        tdp_backend: Option<Arc<dyn tux_daemon::cpu::tdp::TdpBackend>>,
    ) -> Self {
        let num_fans = device.descriptor.fans.count;
        let fan_backend = Arc::new(MockFanBackend::new(num_fans));

        // Set up some initial fan state.
        fan_backend.set_temp(45);
        for i in 0..num_fans {
            fan_backend.set_rpm(i, 2400);
        }

        let backend: Option<Arc<dyn FanBackend>> = Some(fan_backend.clone());

        let (config_tx, config_rx) = watch::channel(FanConfig::default());
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        let store = Arc::new(RwLock::new(
            tux_daemon::profile_store::ProfileStore::new(profile_dir)
                .expect("failed to create profile store"),
        ));

        let assignments = tux_daemon::config::ProfileAssignments::default();
        let (assignments_tx, assignments_rx) = watch::channel(assignments);

        let keyboards: Vec<SharedKeyboard> =
            if matches!(device.descriptor.keyboard, KeyboardType::None) {
                vec![]
            } else {
                vec![Arc::new(std::sync::Mutex::new(Box::new(MockKeyboard)))]
            };

        let applier = Arc::new(tux_daemon::profile_apply::ProfileApplier::new(
            config_tx.clone(),
            None, // no charging backend in applier
            None, // no CPU governor
            None, // no TDP backend
            None, // no GPU backend
            keyboards.clone(),
            None, // no display
        ));

        let (_power_tx, power_rx) = watch::channel(tux_daemon::power_monitor::PowerState::Ac);

        // Start fan engine and extract its failure counter before moving it.
        let engine_backend = fan_backend.clone() as Arc<dyn FanBackend>;
        let (manual_pwms_tx, manual_pwms_rx) = watch::channel(Vec::<u8>::new());
        let mut engine = FanCurveEngine::new_with_manual_pwms_no_hwmon(
            engine_backend,
            config_rx.clone(),
            manual_pwms_rx,
        );
        let fan_failure_counter = engine.failure_counter();
        let engine_shutdown = shutdown_tx.subscribe();
        let engine_handle = tokio::spawn(async move {
            engine.run(engine_shutdown).await;
        });

        let connection = tux_daemon::dbus::serve_on_bus(tux_daemon::dbus::DbusConfig {
            bus_type: tux_daemon::dbus::BusType::Session,
            device,
            fan_backend: backend.clone(),
            keyboards,
            charging,
            cpu_governor: None,
            tdp_backend,
            gpu_backend: None,
            display: None,
            config_tx: config_tx.clone(),
            config_rx: config_rx.clone(),
            store,
            assignments_tx,
            assignments_rx,
            applier,
            power_rx,
            daemon_config: std::sync::Arc::new(std::sync::RwLock::new(
                tux_daemon::config::DaemonConfig::default(),
            )),
            fan_failure_counter,
            manual_pwms_tx,
        })
        .await
        .expect("failed to start D-Bus service on session bus");

        Self {
            connection,
            fan_backend,
            shutdown_tx,
            engine_handle: Some(engine_handle),
        }
    }

    /// Gracefully shut down the test daemon.
    #[allow(dead_code)]
    pub async fn stop(mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(handle) = self.engine_handle.take() {
            let _ = tokio::time::timeout(tokio::time::Duration::from_secs(2), handle).await;
        }
    }
}

impl Drop for TestDaemon {
    fn drop(&mut self) {
        // Best-effort shutdown signal if stop() was never called (e.g., test panic).
        let _ = self.shutdown_tx.send(());
        if let Some(handle) = self.engine_handle.take() {
            handle.abort();
        }
    }
}
