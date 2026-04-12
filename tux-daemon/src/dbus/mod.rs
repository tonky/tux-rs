//! D-Bus server setup and lifecycle.

use std::sync::atomic::AtomicU32;
use std::sync::{Arc, RwLock};

use tokio::sync::watch;
use tracing::info;
use zbus::connection::Builder;

use tux_core::backend::fan::FanBackend;
use tux_core::dmi::DetectedDevice;
use tux_core::fan_curve::FanConfig;

use crate::charging::ChargingBackend;
use crate::config::ProfileAssignments;
use crate::cpu::governor::CpuGovernor;
use crate::cpu::tdp::TdpBackend;
use crate::display::SharedDisplay;
use crate::gpu::GpuPowerBackend;
use crate::hid::SharedKeyboard;
use crate::power_monitor::PowerState;
use crate::profile_apply::ProfileApplier;
use crate::profile_store::ProfileStore;

mod charging;
mod cpu;
mod device;
mod fan;
mod gpu_power;
mod keyboard;
mod profile;
mod settings;
mod system;
#[cfg(feature = "tcc-compat")]
mod tcc_compat;

pub use charging::ChargingInterface;
pub use cpu::CpuInterface;
pub use device::DeviceInterface;
pub use fan::FanInterface;
pub use fan::FanInterfaceDeps;
pub use gpu_power::GpuPowerInterface;
pub use keyboard::KeyboardInterface;
pub use profile::ProfileInterface;
pub use settings::SettingsInterface;
pub use system::SystemInterface;
#[cfg(feature = "tcc-compat")]
pub use tcc_compat::TccCompatInterface;

const BUS_NAME: &str = "com.tuxedocomputers.tccd";
const OBJECT_PATH: &str = "/com/tuxedocomputers/tccd";

/// Which D-Bus bus to connect to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusType {
    System,
    Session,
}

/// All resources needed to start the D-Bus server.
pub struct DbusConfig<'a> {
    pub bus_type: BusType,
    pub device: &'a DetectedDevice,
    pub fan_backend: Option<Arc<dyn FanBackend>>,
    pub keyboards: Vec<SharedKeyboard>,
    pub charging: Option<Arc<dyn ChargingBackend>>,
    pub cpu_governor: Option<Arc<CpuGovernor>>,
    pub tdp_backend: Option<Arc<dyn TdpBackend>>,
    pub gpu_backend: Option<Arc<dyn GpuPowerBackend>>,
    pub display: Option<SharedDisplay>,
    pub config_tx: watch::Sender<FanConfig>,
    pub config_rx: watch::Receiver<FanConfig>,
    pub store: Arc<RwLock<ProfileStore>>,
    pub assignments_tx: watch::Sender<ProfileAssignments>,
    pub assignments_rx: watch::Receiver<ProfileAssignments>,
    pub applier: Arc<ProfileApplier>,
    pub power_rx: watch::Receiver<PowerState>,
    pub daemon_config: Arc<RwLock<crate::config::DaemonConfig>>,
    /// Consecutive temp-read failure counter shared with the fan curve engine.
    pub fan_failure_counter: Arc<AtomicU32>,
    /// Sender for manual PWM setpoints, shared with the fan curve engine for
    /// EC-override re-application (Inwill tuxedo_uw_fan workaround).
    pub manual_pwms_tx: tokio::sync::watch::Sender<Vec<u8>>,
}

/// Build and start the D-Bus connection on the specified bus.
///
/// Registers Device, Profile, Settings, and System interfaces always.
/// Registers the Fan interface when a backend is available.
/// Registers the Keyboard interface when HID keyboards are discovered.
pub async fn serve_on_bus(config: DbusConfig<'_>) -> zbus::Result<zbus::Connection> {
    let DbusConfig {
        bus_type,
        device,
        fan_backend: backend,
        keyboards,
        charging,
        cpu_governor,
        tdp_backend,
        gpu_backend,
        display,
        config_tx,
        config_rx,
        store,
        assignments_tx,
        assignments_rx,
        applier,
        power_rx,
        daemon_config,
        fan_failure_counter,
        manual_pwms_tx,
    } = config;

    // Clone resources needed by the TCC compat interface before they're consumed.
    #[cfg(feature = "tcc-compat")]
    let compat_fan_backend = backend.clone();
    #[cfg(feature = "tcc-compat")]
    let compat_config_rx = config_rx.clone();
    #[cfg(feature = "tcc-compat")]
    let compat_store = store.clone();
    #[cfg(feature = "tcc-compat")]
    let compat_assignments_rx = assignments_rx.clone();
    #[cfg(feature = "tcc-compat")]
    let compat_power_rx = power_rx.clone();
    #[cfg(feature = "tcc-compat")]
    let compat_charging = charging.clone();
    #[cfg(feature = "tcc-compat")]
    let compat_gpu = gpu_backend.clone();
    #[cfg(feature = "tcc-compat")]
    let compat_cpu_governor = cpu_governor.clone();
    #[cfg(feature = "tcc-compat")]
    let compat_tdp = tdp_backend.clone();

    let device_iface = DeviceInterface::new(
        device.descriptor.name.to_string(),
        format!("{:?}", device.descriptor.platform),
    );

    let has_fan = backend.is_some();
    let fan_count = backend.as_ref().map_or(0, |b| b.num_fans());

    let assignments_tx = Arc::new(assignments_tx);

    let profile_iface = ProfileInterface::new(
        store.clone(),
        assignments_tx.clone(),
        assignments_rx.clone(),
        applier,
        power_rx.clone(),
    );

    let settings_iface = SettingsInterface::new(
        device,
        has_fan,
        fan_count,
        keyboards.clone(),
        cpu_governor.clone(),
        display,
        charging.is_some(),
    );

    // Clone before SystemInterface consumes them by value
    let fan_store = store.clone();
    let fan_assignments_rx = assignments_rx.clone();
    let fan_power_rx = power_rx.clone();

    let system_iface = SystemInterface::new(power_rx, assignments_rx, store);

    let mut builder = match bus_type {
        BusType::System => Builder::system()?,
        BusType::Session => Builder::session()?,
    };
    builder = builder
        .name(BUS_NAME)?
        .serve_at(OBJECT_PATH, device_iface)?
        .serve_at(OBJECT_PATH, profile_iface)?
        .serve_at(OBJECT_PATH, settings_iface)?
        .serve_at(OBJECT_PATH, system_iface)?;

    if let Some(backend) = backend {
        let fan_iface = FanInterface::new(FanInterfaceDeps {
            backend,
            config_tx,
            config_rx,
            store: fan_store,
            assignments_rx: fan_assignments_rx,
            power_rx: fan_power_rx,
            failure_counter: fan_failure_counter,
            manual_pwms_tx,
        });
        builder = builder.serve_at(OBJECT_PATH, fan_iface)?;
    }

    if !keyboards.is_empty() {
        let kb_iface = KeyboardInterface::new(keyboards.clone());
        builder = builder.serve_at(OBJECT_PATH, kb_iface)?;
        info!("keyboard LED interface registered");
    }

    if !matches!(
        device.descriptor.charging,
        tux_core::device::ChargingCapability::None
    ) {
        let has_backend = charging.is_some();
        let charging_iface = ChargingInterface::new(charging, daemon_config.clone());
        builder = builder.serve_at(OBJECT_PATH, charging_iface)?;
        info!(
            "charging interface registered (backend: {})",
            if has_backend { "active" } else { "unavailable" }
        );
    }

    if let Some(governor) = cpu_governor {
        let cpu_iface = CpuInterface::new(governor, tdp_backend);
        builder = builder.serve_at(OBJECT_PATH, cpu_iface)?;
        info!("CPU governor/TDP interface registered");
    }

    if let Some(gpu) = gpu_backend.clone() {
        let gpu_iface = GpuPowerInterface::new(gpu);
        builder = builder.serve_at(OBJECT_PATH, gpu_iface)?;
        info!("GPU power control interface registered");
    }

    #[cfg(feature = "tcc-compat")]
    {
        let compat = TccCompatInterface::new(
            device,
            compat_fan_backend,
            compat_config_rx,
            compat_store,
            assignments_tx, // Arc clone shared via ProfileInterface
            compat_assignments_rx,
            compat_power_rx,
            compat_charging,
            keyboards, // shared keyboard references
            compat_gpu,
            compat_cpu_governor,
            compat_tdp,
        );
        builder = builder.serve_at(OBJECT_PATH, compat)?;
        info!("TCC compatibility interface registered");
    }

    let conn = builder.build().await?;

    info!("D-Bus service registered as {BUS_NAME} at {OBJECT_PATH}");
    Ok(conn)
}
