//! Suspend/resume handler via logind PrepareForSleep signal.
//!
//! Saves fan state on suspend, restores it on resume. Handles keyboard
//! turn_off/turn_on for ITE HID devices across sleep cycles.

use std::sync::{Arc, Mutex};

use tokio::sync::watch;
use tracing::{info, warn};

use tux_core::backend::fan::FanBackend;
use tux_core::fan_curve::FanConfig;

use crate::hid::KeyboardLed;

/// Saved hardware state for suspend/resume.
struct SavedState {
    fan_config: FanConfig,
}

/// Handles suspend/resume events from logind PrepareForSleep.
pub struct SleepHandler {
    fan_backend: Option<Arc<dyn FanBackend>>,
    fan_config_rx: watch::Receiver<FanConfig>,
    keyboards: Vec<Arc<Mutex<Box<dyn KeyboardLed>>>>,
    saved_state: Mutex<Option<SavedState>>,
    fan_config_tx: watch::Sender<FanConfig>,
}

impl SleepHandler {
    pub fn new(
        fan_backend: Option<Arc<dyn FanBackend>>,
        fan_config_tx: watch::Sender<FanConfig>,
        fan_config_rx: watch::Receiver<FanConfig>,
        keyboards: Vec<Arc<Mutex<Box<dyn KeyboardLed>>>>,
    ) -> Self {
        Self {
            fan_backend,
            fan_config_rx,
            keyboards,
            saved_state: Mutex::new(None),
            fan_config_tx,
        }
    }

    /// Called when system is about to suspend (PrepareForSleep=true).
    pub fn on_suspend(&self) {
        info!("preparing for suspend");

        // 1. Save current fan config.
        let config = self.fan_config_rx.borrow().clone();
        *self.saved_state.lock().unwrap() = Some(SavedState { fan_config: config });

        // 2. Restore fans to auto (safe for sleep).
        if let Some(ref backend) = self.fan_backend {
            for i in 0..backend.num_fans() {
                if let Err(e) = backend.set_auto(i) {
                    warn!("failed to set fan {i} to auto before suspend: {e}");
                }
            }
            info!("fans set to auto for suspend");
        }

        // 3. Turn off keyboard LEDs.
        for (i, kb) in self.keyboards.iter().enumerate() {
            if let Ok(mut kb) = kb.lock()
                && let Err(e) = kb.turn_off()
            {
                warn!("failed to turn off keyboard {i} for suspend: {e}");
            }
        }
        if !self.keyboards.is_empty() {
            info!("keyboard LEDs turned off for suspend");
        }
    }

    /// Called when system resumes (PrepareForSleep=false).
    pub fn on_resume(&self) {
        info!("resuming from suspend");

        // 1. Restore fan config.
        let saved = self.saved_state.lock().unwrap().take();
        if let Some(state) = saved {
            if let Err(e) = self.fan_config_tx.send(state.fan_config) {
                warn!("failed to restore fan config after resume: {e}");
            }
            info!("fan config restored after resume");
        }

        // 2. Turn on keyboard LEDs (restores previous brightness/color/mode).
        for (i, kb) in self.keyboards.iter().enumerate() {
            if let Ok(mut kb) = kb.lock()
                && let Err(e) = kb.turn_on()
            {
                warn!("failed to turn on keyboard {i} after resume: {e}");
            }
        }
        if !self.keyboards.is_empty() {
            info!("keyboard LEDs restored after resume");
        }
    }
}

/// Monitor logind PrepareForSleep signal and call handler on suspend/resume.
pub async fn monitor_sleep(
    handler: Arc<SleepHandler>,
    mut shutdown: tokio::sync::broadcast::Receiver<()>,
) {
    use futures_util::StreamExt;

    let conn = match zbus::Connection::system().await {
        Ok(c) => c,
        Err(e) => {
            warn!("failed to connect to system D-Bus for sleep monitoring: {e}");
            return;
        }
    };

    let proxy = match LogindManagerProxy::new(&conn).await {
        Ok(p) => p,
        Err(e) => {
            warn!("failed to create logind proxy: {e}");
            return;
        }
    };

    let mut stream = match proxy.receive_prepare_for_sleep().await {
        Ok(s) => s,
        Err(e) => {
            warn!("failed to subscribe to PrepareForSleep signal: {e}");
            return;
        }
    };

    info!("monitoring logind PrepareForSleep signal");

    loop {
        tokio::select! {
            signal = stream.next() => {
                let Some(signal) = signal else { break };
                match signal.args() {
                    Ok(args) => {
                        if args.start {
                            handler.on_suspend();
                        } else {
                            handler.on_resume();
                        }
                    }
                    Err(e) => warn!("failed to parse PrepareForSleep args: {e}"),
                }
            }
            _ = shutdown.recv() => {
                info!("sleep monitor shutting down");
                break;
            }
        }
    }
}

/// Generated proxy for org.freedesktop.login1.Manager
#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait LogindManager {
    #[zbus(signal)]
    fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tux_core::fan_curve::{FanConfig, FanMode};
    use tux_core::mock::fan::MockFanBackend;

    #[test]
    fn on_suspend_saves_fan_config_and_sets_auto() {
        let backend = Arc::new(MockFanBackend::new(2));
        backend.set_temp(50);
        backend.write_pwm(0, 100).unwrap();
        backend.write_pwm(1, 150).unwrap();

        let config = FanConfig {
            mode: FanMode::CustomCurve,
            min_speed_percent: 30,
            ..FanConfig::default()
        };
        let (tx, rx) = watch::channel(config);
        let handler = SleepHandler::new(Some(backend.clone()), tx, rx, vec![]);

        handler.on_suspend();

        // Fans should be in auto mode.
        assert!(backend.is_auto(0));
        assert!(backend.is_auto(1));

        // State should be saved.
        let saved = handler.saved_state.lock().unwrap();
        assert!(saved.is_some());
        assert_eq!(
            saved.as_ref().unwrap().fan_config.mode,
            FanMode::CustomCurve
        );
        assert_eq!(saved.as_ref().unwrap().fan_config.min_speed_percent, 30);
    }

    #[test]
    fn on_resume_restores_fan_config() {
        let backend = Arc::new(MockFanBackend::new(1));
        let config = FanConfig {
            mode: FanMode::CustomCurve,
            min_speed_percent: 25,
            ..FanConfig::default()
        };
        let (tx, rx) = watch::channel(config);
        let handler = SleepHandler::new(Some(backend.clone()), tx.clone(), rx, vec![]);

        // Suspend to save state.
        handler.on_suspend();
        assert!(backend.is_auto(0));

        // Overwrite config to simulate engine clearing it during suspend.
        tx.send(FanConfig::default()).unwrap();

        // Resume.
        handler.on_resume();

        // Config should be restored via the tx channel.
        let restored = handler.fan_config_rx.borrow();
        assert_eq!(restored.mode, FanMode::CustomCurve);
        assert_eq!(restored.min_speed_percent, 25);
    }

    #[test]
    fn on_resume_without_suspend_is_noop() {
        let backend = Arc::new(MockFanBackend::new(1));
        let (tx, rx) = watch::channel(FanConfig::default());
        let handler = SleepHandler::new(Some(backend), tx, rx, vec![]);

        // Should not panic.
        handler.on_resume();
    }

    #[test]
    fn on_suspend_no_fan_backend_is_safe() {
        let (tx, rx) = watch::channel(FanConfig::default());
        let handler = SleepHandler::new(None, tx, rx, vec![]);

        // Should not panic.
        handler.on_suspend();
    }

    #[test]
    fn on_suspend_resume_no_keyboards_is_safe() {
        let backend = Arc::new(MockFanBackend::new(1));
        let (tx, rx) = watch::channel(FanConfig::default());
        let handler = SleepHandler::new(Some(backend), tx, rx, vec![]);

        handler.on_suspend();
        handler.on_resume();
    }
}
