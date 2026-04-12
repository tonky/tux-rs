//! Fan curve engine: temperature polling, curve interpolation, and PWM control.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use tokio::sync::{broadcast, watch};
use tracing::{debug, info, warn};

use tux_core::backend::fan::FanBackend;
use tux_core::fan_curve::{FanConfig, FanMode, interpolate, percent_to_pwm};

/// Number of consecutive temperature-read failures tolerated before forcing
/// a reduced safety PWM when the engine already has a previously computed setpoint.
const SAFETY_FAILURE_THRESHOLD: u32 = 5;

/// Safety fallback duty used after persistent temperature-read failures.
const SAFETY_FAILURE_PERCENT: u8 = 60;

/// Core fan control loop: polls temperature, interpolates curve, writes PWM.
pub struct FanCurveEngine {
    backend: Arc<dyn FanBackend>,
    config_rx: watch::Receiver<FanConfig>,
    /// Per-fan manual PWM setpoints. Updated by D-Bus `set_fan_speed` calls.
    /// Only used when `backend.requires_manual_reapply()` is true.
    manual_pwms_rx: watch::Receiver<Vec<u8>>,
    /// Shared counter of consecutive temperature-read failures (reset on success).
    consecutive_failures: Arc<AtomicU32>,
}

impl FanCurveEngine {
    pub fn new(backend: Arc<dyn FanBackend>, config_rx: watch::Receiver<FanConfig>) -> Self {
        let (_, manual_pwms_rx) = watch::channel(Vec::new());
        Self {
            backend,
            config_rx,
            manual_pwms_rx,
            consecutive_failures: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Create an engine wired to a manual-PWM watch channel.
    ///
    /// Used by `main.rs` to wire the D-Bus `set_fan_speed` handler to the
    /// engine so Inwill's EC-override workaround can re-apply setpoints each tick.
    pub fn new_with_manual_pwms(
        backend: Arc<dyn FanBackend>,
        config_rx: watch::Receiver<FanConfig>,
        manual_pwms_rx: watch::Receiver<Vec<u8>>,
    ) -> Self {
        Self {
            backend,
            config_rx,
            manual_pwms_rx,
            consecutive_failures: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Return an `Arc` handle to the consecutive-failure counter.
    ///
    /// Callers (e.g. the D-Bus layer) clone this to observe engine health
    /// without needing a direct reference to the engine itself.
    pub fn failure_counter(&self) -> Arc<AtomicU32> {
        self.consecutive_failures.clone()
    }

    /// Run the engine until a shutdown signal is received.
    pub async fn run(&mut self, mut shutdown: broadcast::Receiver<()>) {
        let mut last_temp: Option<u8> = None;
        let mut last_pwm: Option<u8> = None;
        let mut current_mode = FanMode::Auto;
        let mut last_config: Option<FanConfig> = None;

        loop {
            let config = self.config_rx.borrow_and_update().clone();

            // Detect config changes by value comparison — eliminates the
            // TOCTOU race between has_changed() and borrow_and_update().
            let config_changed = last_config.as_ref() != Some(&config);
            if config_changed {
                debug!("config change detected (value diff), resetting hysteresis");
                last_temp = None;
                last_pwm = None;
                last_config = Some(config.clone());
            }

            // Mode change handling.
            if config.mode != current_mode {
                info!("fan mode changed: {:?} → {:?}", current_mode, config.mode);
                match config.mode {
                    FanMode::Auto => self.set_all_auto(),
                    FanMode::Manual => {
                        // Setpoints will be read from manual_pwms_rx; no local state to clear.
                    }
                    FanMode::CustomCurve => {
                        last_temp = None;
                        last_pwm = None;
                    }
                }
                current_mode = config.mode;
            }

            let poll_ms = match current_mode {
                FanMode::Auto => config.idle_poll_ms,
                FanMode::Manual => {
                    // On backends where the EC periodically restores its own fan
                    // table (e.g. Inwill tuxedo_uw_fan), re-apply the user's
                    // setpoints on every idle tick. Other backends are single-write.
                    if self.backend.requires_manual_reapply() {
                        let pwms = self.manual_pwms_rx.borrow();
                        if !pwms.is_empty() {
                            for (i, &pwm) in pwms.iter().enumerate() {
                                if let Err(e) = self.backend.write_pwm(i as u8, pwm) {
                                    warn!("failed to re-apply manual PWM to fan {i}: {e}");
                                }
                            }
                        }
                    }
                    config.idle_poll_ms
                }
                FanMode::CustomCurve => {
                    self.tick_custom_curve(&config, &mut last_temp, &mut last_pwm)
                }
            };

            tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(poll_ms)) => {}
                _ = self.config_rx.changed() => {
                    debug!("config change detected, re-evaluating");
                    last_temp = None;
                    last_pwm = None;
                    continue;
                }
                _ = shutdown.recv() => {
                    info!("fan engine shutting down, restoring auto mode");
                    self.set_all_auto();
                    return;
                }
            }
        }
    }

    /// Execute one CustomCurve tick. Returns the poll interval to use.
    ///
    /// PWM is always re-written on every tick, even when hysteresis
    /// suppresses speed recalculation.  Some ECs (notably Uniwill with
    /// universal fan tables) run their own internal ramp-up loop and
    /// will override a stale PWM value within seconds.
    fn tick_custom_curve(
        &self,
        config: &FanConfig,
        last_temp: &mut Option<u8>,
        last_pwm: &mut Option<u8>,
    ) -> u64 {
        let temp = match self.backend.read_temp() {
            Ok(t) => {
                self.consecutive_failures.store(0, Ordering::Relaxed);
                t
            }
            Err(e) => {
                let n = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
                if n < SAFETY_FAILURE_THRESHOLD
                    && let Some(pwm) = *last_pwm
                {
                    warn!(
                        "failed to read temperature: {e}, keeping previous PWM {pwm} on transient failure {n}/{SAFETY_FAILURE_THRESHOLD}"
                    );
                    self.set_all_pwm(pwm);
                    return config.active_poll_ms;
                }
                warn!(
                    "failed to read temperature: {e}, setting {SAFETY_FAILURE_PERCENT}% safety after consecutive failure {n}"
                );
                self.set_all_percent(SAFETY_FAILURE_PERCENT);
                return config.active_poll_ms;
            }
        };

        // Hysteresis check — only recalculate speed when temp changed enough.
        let poll_ms;
        let pwm = if let Some(prev) = *last_temp {
            let diff = (temp as i16 - prev as i16).unsigned_abs() as u8;
            if diff < config.hysteresis_degrees {
                poll_ms = config.idle_poll_ms;
                // Re-use last computed PWM (still re-written below).
                match *last_pwm {
                    Some(p) => p,
                    None => {
                        // Should not happen, but compute anyway.
                        let speed = interpolate(&config.curve, temp).max(config.min_speed_percent);
                        percent_to_pwm(speed)
                    }
                }
            } else {
                *last_temp = Some(temp);
                poll_ms = config.active_poll_ms;
                let speed = interpolate(&config.curve, temp).max(config.min_speed_percent);
                let p = percent_to_pwm(speed);
                debug!("temp={temp}°C → speed={speed}% → pwm={p}");
                p
            }
        } else {
            *last_temp = Some(temp);
            poll_ms = config.active_poll_ms;
            let speed = interpolate(&config.curve, temp).max(config.min_speed_percent);
            let p = percent_to_pwm(speed);
            debug!("temp={temp}°C → speed={speed}% → pwm={p}");
            p
        };

        *last_pwm = Some(pwm);

        // Always write PWM — the EC may override stale values.
        let num_fans = self.backend.num_fans();
        for i in 0..num_fans {
            if let Err(e) = self.backend.write_pwm(i, pwm) {
                warn!("failed to write PWM to fan {i}: {e}");
            }
        }

        poll_ms
    }

    fn set_all_auto(&self) {
        let num_fans = self.backend.num_fans();
        for i in 0..num_fans {
            if let Err(e) = self.backend.set_auto(i) {
                warn!("failed to set auto mode for fan {i}: {e}");
            }
        }
    }

    fn set_all_percent(&self, percent: u8) {
        let pwm = percent_to_pwm(percent);
        self.set_all_pwm(pwm);
    }

    fn set_all_pwm(&self, pwm: u8) {
        let num_fans = self.backend.num_fans();
        for i in 0..num_fans {
            if let Err(e) = self.backend.write_pwm(i, pwm) {
                warn!("failed to write PWM to fan {i}: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tux_core::fan_curve::FanCurvePoint;
    use tux_core::mock::fan::MockFanBackend;

    fn test_config() -> FanConfig {
        FanConfig {
            active_poll_ms: 10,
            idle_poll_ms: 50,
            ..FanConfig::default()
        }
    }

    /// Wait for the engine to process at least one tick.
    async fn settle() {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    /// Poll the mock backend until fan 0 reaches the expected PWM value,
    /// or panic after a generous timeout.  Eliminates timing-dependent
    /// flakiness that a fixed sleep cannot guarantee.
    async fn await_pwm(backend: &MockFanBackend, expected: u8) {
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(2);
        loop {
            if backend.read_pwm(0).unwrap() == expected {
                return;
            }
            if tokio::time::Instant::now() >= deadline {
                panic!(
                    "timed out waiting for pwm={expected}, got {}",
                    backend.read_pwm(0).unwrap()
                );
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn custom_curve_writes_correct_pwm() {
        let backend = Arc::new(MockFanBackend::new(2));
        backend.set_temp(70); // 70°C → between 50→30% and 75→70% = 62%

        let (_config_tx, config_rx) = watch::channel(test_config());
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let engine_backend = backend.clone();
        let handle = tokio::spawn(async move {
            let mut engine = FanCurveEngine::new(engine_backend, config_rx);
            engine.run(shutdown_rx).await;
        });

        settle().await;

        // Read PWM before shutdown (shutdown restores auto which clears PWM).
        let pwm = backend.read_pwm(0).unwrap();
        let pwm1 = backend.read_pwm(1).unwrap();

        drop(shutdown_tx);
        handle.await.unwrap();

        // 62% → pwm = (62 * 255 + 50) / 100 = 158
        assert_eq!(pwm, 158, "fan0 PWM should be 158 for 70°C");
        assert_eq!(pwm1, 158, "fan1 PWM should be 158 for 70°C");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn hysteresis_prevents_update() {
        let backend = Arc::new(MockFanBackend::new(1));
        backend.set_temp(70);

        let config = FanConfig {
            hysteresis_degrees: 5,
            active_poll_ms: 10,
            idle_poll_ms: 10,
            ..FanConfig::default()
        };
        let (_config_tx, config_rx) = watch::channel(config);
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let engine_backend = backend.clone();
        let handle = tokio::spawn(async move {
            let mut engine = FanCurveEngine::new(engine_backend, config_rx);
            engine.run(shutdown_rx).await;
        });

        // Let first tick happen at 70°C.
        settle().await;
        let pwm_70 = backend.read_pwm(0).unwrap();

        // Change temp by 2°C (below hysteresis of 5) — should NOT update.
        backend.set_temp(72);
        settle().await;
        let pwm_72 = backend.read_pwm(0).unwrap();
        assert_eq!(pwm_70, pwm_72, "PWM should not change within hysteresis");

        // Change temp by 10°C (above hysteresis) — SHOULD update.
        backend.set_temp(80);
        settle().await;
        let pwm_80 = backend.read_pwm(0).unwrap();
        // 80°C → between 75→70% and 100→100% = 76% → pwm = (76 * 255 + 50) / 100 = 194
        assert_eq!(pwm_80, 194, "PWM should update after exceeding hysteresis");

        drop(shutdown_tx);
        handle.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn min_speed_overrides_low_curve_value() {
        let backend = Arc::new(MockFanBackend::new(1));
        backend.set_temp(10); // 10°C → curve says 4%, but min_speed = 25%

        let (_config_tx, config_rx) = watch::channel(test_config());
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let engine_backend = backend.clone();
        let handle = tokio::spawn(async move {
            let mut engine = FanCurveEngine::new(engine_backend, config_rx);
            engine.run(shutdown_rx).await;
        });

        settle().await;

        // Read before shutdown (shutdown restores auto which clears PWM).
        let pwm = backend.read_pwm(0).unwrap();

        drop(shutdown_tx);
        handle.await.unwrap();

        // min_speed_percent = 25 → pwm = (25 * 255 + 50) / 100 = 64
        assert_eq!(pwm, 64, "min speed should override curve value of 0%");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn mode_switch_to_auto_calls_set_auto() {
        let backend = Arc::new(MockFanBackend::new(2));
        backend.set_temp(70);

        let config = test_config();
        let (config_tx, config_rx) = watch::channel(config.clone());
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let engine_backend = backend.clone();
        let handle = tokio::spawn(async move {
            let mut engine = FanCurveEngine::new(engine_backend, config_rx);
            engine.run(shutdown_rx).await;
        });

        // Let CustomCurve run to write PWM.
        settle().await;
        assert!(!backend.is_auto(0), "should be manual after curve write");

        // Switch to Auto.
        let mut auto_config = config;
        auto_config.mode = FanMode::Auto;
        config_tx.send(auto_config).unwrap();
        settle().await;

        assert!(backend.is_auto(0), "fan0 should be auto");
        assert!(backend.is_auto(1), "fan1 should be auto");

        drop(shutdown_tx);
        handle.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn config_change_takes_effect() {
        let backend = Arc::new(MockFanBackend::new(1));
        backend.set_temp(50); // 50°C → default curve = 30%

        let (config_tx, config_rx) = watch::channel(test_config());
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let engine_backend = backend.clone();
        let handle = tokio::spawn(async move {
            let mut engine = FanCurveEngine::new(engine_backend, config_rx);
            engine.run(shutdown_rx).await;
        });

        // First tick at 50°C: 30% > min 25% → 30% → pwm 77.
        settle().await;
        assert_eq!(backend.read_pwm(0).unwrap(), 77);

        // Change curve to always 80%.
        let new_config = FanConfig {
            curve: vec![FanCurvePoint { temp: 0, speed: 80 }],
            min_speed_percent: 0,
            hysteresis_degrees: 0,
            active_poll_ms: 10,
            idle_poll_ms: 10,
            ..FanConfig::default()
        };
        config_tx.send(new_config).unwrap();
        settle().await;

        // 80% → pwm 204
        assert_eq!(backend.read_pwm(0).unwrap(), 204);

        drop(shutdown_tx);
        handle.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_restores_auto() {
        let backend = Arc::new(MockFanBackend::new(1));
        backend.set_temp(70);

        let (_config_tx, config_rx) = watch::channel(test_config());
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let engine_backend = backend.clone();
        let handle = tokio::spawn(async move {
            let mut engine = FanCurveEngine::new(engine_backend, config_rx);
            engine.run(shutdown_rx).await;
        });

        settle().await;
        assert!(!backend.is_auto(0));

        drop(shutdown_tx);
        handle.await.unwrap();

        assert!(backend.is_auto(0), "shutdown should restore auto mode");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn transient_temp_read_failure_keeps_last_pwm() {
        let backend = Arc::new(MockFanBackend::new(1));
        backend.set_temp(50);

        let (_config_tx, config_rx) = watch::channel(test_config());
        let engine = FanCurveEngine::new(backend.clone(), config_rx);
        let config = test_config();
        let mut last_temp = Some(50);
        let mut last_pwm = Some(77);

        backend.write_pwm(0, 77).unwrap();
        backend.set_fail_temp(true);
        let poll_ms = engine.tick_custom_curve(&config, &mut last_temp, &mut last_pwm);

        assert_eq!(poll_ms, config.active_poll_ms);
        assert_eq!(backend.read_pwm(0).unwrap(), 77);
        assert_eq!(engine.failure_counter().load(Ordering::Relaxed), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn repeated_temp_read_failure_sets_reduced_safety_speed() {
        let backend = Arc::new(MockFanBackend::new(1));
        backend.set_temp(50);

        let (_config_tx, config_rx) = watch::channel(test_config());
        let engine = FanCurveEngine::new(backend.clone(), config_rx);
        let config = test_config();
        let mut last_temp = Some(50);
        let mut last_pwm = Some(77);

        backend.write_pwm(0, 77).unwrap();
        backend.set_fail_temp(true);

        engine.tick_custom_curve(&config, &mut last_temp, &mut last_pwm);
        assert_eq!(backend.read_pwm(0).unwrap(), 77);

        engine.tick_custom_curve(&config, &mut last_temp, &mut last_pwm);
        assert_eq!(backend.read_pwm(0).unwrap(), 77);

        engine.tick_custom_curve(&config, &mut last_temp, &mut last_pwm);
        assert_eq!(backend.read_pwm(0).unwrap(), 77);

        engine.tick_custom_curve(&config, &mut last_temp, &mut last_pwm);
        assert_eq!(backend.read_pwm(0).unwrap(), 77);

        engine.tick_custom_curve(&config, &mut last_temp, &mut last_pwm);
        assert_eq!(backend.read_pwm(0).unwrap(), 153);
        assert_eq!(engine.failure_counter().load(Ordering::Relaxed), 5);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn failure_counter_increments_on_read_failure() {
        let backend = Arc::new(MockFanBackend::new(1));
        backend.set_temp(70);

        let (_config_tx, config_rx) = watch::channel(test_config());
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let engine_backend = backend.clone();
        let mut engine = FanCurveEngine::new(engine_backend, config_rx);
        let counter = engine.failure_counter();

        let handle = tokio::spawn(async move {
            engine.run(shutdown_rx).await;
        });

        // Normal operation: counter should be 0.
        settle().await;
        assert_eq!(counter.load(Ordering::Relaxed), 0, "no failures yet");

        // Inject failures.
        backend.set_fail_temp(true);
        settle().await;
        let after_failure = counter.load(Ordering::Relaxed);
        assert!(
            after_failure >= 1,
            "counter should increment on failure, got {after_failure}"
        );

        drop(shutdown_tx);
        handle.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn failure_counter_resets_on_success() {
        let backend = Arc::new(MockFanBackend::new(1));
        backend.set_temp(70);

        let (_config_tx, config_rx) = watch::channel(test_config());
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let engine_backend = backend.clone();
        let mut engine = FanCurveEngine::new(engine_backend, config_rx);
        let counter = engine.failure_counter();

        let handle = tokio::spawn(async move {
            engine.run(shutdown_rx).await;
        });

        // Inject failures so counter goes above zero.
        backend.set_fail_temp(true);
        settle().await;
        assert!(counter.load(Ordering::Relaxed) >= 1, "should have failures");

        // Recover: stop failing, counter should reset on next successful read.
        backend.set_fail_temp(false);
        settle().await;
        assert_eq!(
            counter.load(Ordering::Relaxed),
            0,
            "counter should reset to 0 after successful temp read"
        );

        drop(shutdown_tx);
        handle.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn config_change_bypasses_hysteresis() {
        let backend = Arc::new(MockFanBackend::new(1));
        backend.set_temp(70); // 70°C → default curve ~62%

        let config = FanConfig {
            hysteresis_degrees: 10, // large hysteresis
            active_poll_ms: 10,
            idle_poll_ms: 10,
            ..FanConfig::default()
        };
        let (config_tx, config_rx) = watch::channel(config);
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let engine_backend = backend.clone();
        let handle = tokio::spawn(async move {
            let mut engine = FanCurveEngine::new(engine_backend, config_rx);
            engine.run(shutdown_rx).await;
        });

        // Wait for first tick to write 70°C → ~62% → pwm 158.
        await_pwm(&backend, 158).await;

        // Now change config (different curve) WITHOUT changing temperature.
        // The new curve maps everything to 80%.
        let new_config = FanConfig {
            curve: vec![FanCurvePoint { temp: 0, speed: 80 }],
            min_speed_percent: 0,
            hysteresis_degrees: 10,
            active_poll_ms: 10,
            idle_poll_ms: 10,
            ..FanConfig::default()
        };
        config_tx.send(new_config).unwrap();
        tokio::task::yield_now().await;

        // Let engine process the config change.
        settle().await;

        // 80% → pwm 204.
        assert_eq!(backend.read_pwm(0).unwrap(), 204);

        drop(shutdown_tx);
        handle.await.unwrap();
    }
}
