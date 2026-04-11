//! Integration-style lifecycle tests for daemon shutdown + startup safety.
//!
//! These test the shutdown handler behavior and startup safety reset logic
//! using the mock backend, without requiring a real D-Bus connection.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::{broadcast, watch};

    use tux_core::backend::fan::FanBackend;
    use tux_core::fan_curve::FanConfig;
    use tux_core::mock::fan::MockFanBackend;

    use crate::fan_engine::FanCurveEngine;

    fn test_config() -> FanConfig {
        FanConfig {
            active_poll_ms: 10,
            idle_poll_ms: 50,
            ..FanConfig::default()
        }
    }

    async fn settle() {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    /// Simulate the startup safety reset: set_auto called for all fans before
    /// any custom curve is applied.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn startup_safety_reset_calls_set_auto() {
        let backend = Arc::new(MockFanBackend::new(2));

        // Simulate a previous crash state: fans stuck in manual with PWM values.
        backend.write_pwm(0, 200).unwrap();
        backend.write_pwm(1, 150).unwrap();
        assert!(!backend.is_auto(0));
        assert!(!backend.is_auto(1));

        // --- Startup safety reset (mirrors main.rs step 5) ---
        for i in 0..backend.num_fans() {
            backend.set_auto(i).unwrap();
        }

        assert!(
            backend.is_auto(0),
            "fan0 should be auto after startup reset"
        );
        assert!(
            backend.is_auto(1),
            "fan1 should be auto after startup reset"
        );
    }

    /// Startup safety reset happens before the engine applies any custom curve.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn startup_reset_before_engine() {
        let backend = Arc::new(MockFanBackend::new(1));
        backend.set_temp(70);

        // Simulate crash state.
        backend.write_pwm(0, 200).unwrap();
        assert!(!backend.is_auto(0));

        // Safety reset.
        for i in 0..backend.num_fans() {
            backend.set_auto(i).unwrap();
        }
        assert!(
            backend.is_auto(0),
            "auto mode should be set before engine starts"
        );

        // Now start the engine — it should override auto with custom curve.
        let (_config_tx, config_rx) = watch::channel(test_config());
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let engine_backend = backend.clone();
        let handle = tokio::spawn(async move {
            let mut engine = FanCurveEngine::new(engine_backend, config_rx);
            engine.run(shutdown_rx).await;
        });

        settle().await;

        // Engine should have written PWM, clearing auto mode.
        assert!(
            !backend.is_auto(0),
            "engine should have taken over fan control"
        );

        drop(shutdown_tx);
        handle.await.unwrap();
    }

    /// On SIGTERM, shutdown handler: sends shutdown signal, engine restores auto,
    /// then main also calls set_auto as a belt-and-suspenders measure.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_handler_restores_all_fans() {
        let backend = Arc::new(MockFanBackend::new(2));
        backend.set_temp(70);

        let (_config_tx, config_rx) = watch::channel(test_config());
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let engine_backend = backend.clone();
        let handle = tokio::spawn(async move {
            let mut engine = FanCurveEngine::new(engine_backend, config_rx);
            engine.run(shutdown_rx).await;
        });

        settle().await;
        assert!(
            !backend.is_auto(0),
            "fan0 should be manual during operation"
        );
        assert!(
            !backend.is_auto(1),
            "fan1 should be manual during operation"
        );

        // --- Simulate shutdown handler (mirrors main.rs step 13) ---
        let _ = shutdown_tx.send(());

        // Wait for engine to process shutdown.
        handle.await.unwrap();

        // Engine restores auto mode on shutdown.
        assert!(backend.is_auto(0), "fan0 should be auto after shutdown");
        assert!(backend.is_auto(1), "fan1 should be auto after shutdown");

        // Belt-and-suspenders: main also calls set_auto.
        for i in 0..backend.num_fans() {
            let _ = backend.set_auto(i);
        }
        assert!(backend.is_auto(0), "fan0 should remain auto");
        assert!(backend.is_auto(1), "fan1 should remain auto");
    }

    /// Engine shutdown restores auto even when in manual mode.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_restores_auto_from_any_mode() {
        use tux_core::fan_curve::FanMode;

        let backend = Arc::new(MockFanBackend::new(1));
        backend.set_temp(50);

        let mut config = test_config();
        config.mode = FanMode::Manual;
        let (_config_tx, config_rx) = watch::channel(config);
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let engine_backend = backend.clone();
        let handle = tokio::spawn(async move {
            let mut engine = FanCurveEngine::new(engine_backend, config_rx);
            engine.run(shutdown_rx).await;
        });

        settle().await;
        // In manual mode, engine doesn't write PWM, but set_auto should still be auto from init.

        drop(shutdown_tx);
        handle.await.unwrap();

        assert!(
            backend.is_auto(0),
            "shutdown should restore auto even from manual mode"
        );
    }
}
