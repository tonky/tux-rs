//! Track active D-Bus clients and provide active/idle polling intervals.
//!
//! When at least one client is connected, the engine uses the faster
//! `active_poll_ms` interval. When all clients disconnect, a grace period
//! (`idle_timeout`) prevents oscillation before switching to the slower
//! `idle_poll_ms` interval.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::Notify;
use tokio::time::Instant;

/// Tracks D-Bus client connections and determines polling mode.
#[allow(dead_code)]
pub struct ClientTracker {
    active_clients: AtomicUsize,
    idle_timeout: Duration,
    /// Instant when the last client disconnected (None if clients are connected or never disconnected).
    last_disconnect: std::sync::Mutex<Option<Instant>>,
    /// Notified when a client connects (wakes the engine to switch to active).
    wake: Arc<Notify>,
}

#[allow(dead_code)]
impl ClientTracker {
    pub fn new(idle_timeout: Duration) -> Self {
        Self {
            active_clients: AtomicUsize::new(0),
            idle_timeout,
            last_disconnect: std::sync::Mutex::new(None),
            wake: Arc::new(Notify::new()),
        }
    }

    /// A D-Bus client connected — switch to active immediately.
    pub fn on_client_connected(&self) {
        self.active_clients.fetch_add(1, Ordering::Release);
        *self.last_disconnect.lock().unwrap() = None;
        self.wake.notify_one();
    }

    /// A D-Bus client disconnected — start grace period if no clients remain.
    pub fn on_client_disconnected(&self) {
        let prev = self.active_clients.load(Ordering::Acquire);
        if prev == 0 {
            return; // No clients to disconnect — caller bug, but don't underflow.
        }
        self.active_clients.fetch_sub(1, Ordering::Release);
        if prev == 1 {
            // We were the last client.
            *self.last_disconnect.lock().unwrap() = Some(Instant::now());
        }
    }

    /// Whether any clients are currently connected.
    pub fn has_active_clients(&self) -> bool {
        self.active_clients.load(Ordering::Acquire) > 0
    }

    /// Returns true if we are in active mode (clients connected, or still
    /// within the idle grace period after last disconnect).
    pub fn is_active(&self) -> bool {
        if self.has_active_clients() {
            return true;
        }
        // Check grace period.
        let guard = self.last_disconnect.lock().unwrap();
        match *guard {
            Some(disconnect_time) => disconnect_time.elapsed() < self.idle_timeout,
            None => false, // No clients have ever connected.
        }
    }

    /// Get the poll interval in ms based on the current active/idle state.
    pub fn poll_interval_ms(&self, active_ms: u64, idle_ms: u64) -> u64 {
        if self.is_active() { active_ms } else { idle_ms }
    }

    /// Get a handle to the wake notifier (for the engine to listen on).
    pub fn wake_notify(&self) -> Arc<Notify> {
        self.wake.clone()
    }

    /// Current number of active clients.
    pub fn client_count(&self) -> usize {
        self.active_clients.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_makes_active() {
        let tracker = ClientTracker::new(Duration::from_secs(30));
        assert!(!tracker.has_active_clients());
        assert!(!tracker.is_active());

        tracker.on_client_connected();
        assert!(tracker.has_active_clients());
        assert!(tracker.is_active());
    }

    #[test]
    fn disconnect_with_grace_period() {
        let tracker = ClientTracker::new(Duration::from_secs(30));
        tracker.on_client_connected();
        tracker.on_client_disconnected();

        assert!(!tracker.has_active_clients());
        // Should still be active during grace period.
        assert!(tracker.is_active());
    }

    #[tokio::test]
    async fn disconnect_becomes_idle_after_timeout() {
        let tracker = ClientTracker::new(Duration::from_millis(50));
        tracker.on_client_connected();
        tracker.on_client_disconnected();

        assert!(tracker.is_active(), "should be active during grace period");

        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(!tracker.is_active(), "should be idle after grace period");
    }

    #[test]
    fn rapid_reconnect_stays_active() {
        let tracker = ClientTracker::new(Duration::from_secs(30));
        tracker.on_client_connected();
        tracker.on_client_disconnected();
        // Reconnect during grace period.
        tracker.on_client_connected();

        assert!(tracker.has_active_clients());
        assert!(tracker.is_active());
    }

    #[test]
    fn multiple_clients() {
        let tracker = ClientTracker::new(Duration::from_secs(30));
        tracker.on_client_connected();
        tracker.on_client_connected();
        assert_eq!(tracker.client_count(), 2);

        tracker.on_client_disconnected();
        assert_eq!(tracker.client_count(), 1);
        assert!(tracker.has_active_clients());

        tracker.on_client_disconnected();
        assert!(!tracker.has_active_clients());
    }

    #[test]
    fn poll_interval_active_vs_idle() {
        let tracker = ClientTracker::new(Duration::from_millis(0));
        // No clients, no grace → idle
        assert_eq!(tracker.poll_interval_ms(2000, 10000), 10000);

        tracker.on_client_connected();
        assert_eq!(tracker.poll_interval_ms(2000, 10000), 2000);
    }

    #[tokio::test]
    async fn wake_notified_on_connect() {
        let tracker = Arc::new(ClientTracker::new(Duration::from_secs(30)));
        let wake = tracker.wake_notify();

        let tracker2 = tracker.clone();
        let handle = tokio::spawn(async move {
            wake.notified().await;
            true
        });

        // Small delay to ensure the task is waiting.
        tokio::time::sleep(Duration::from_millis(10)).await;
        tracker2.on_client_connected();

        let notified = tokio::time::timeout(Duration::from_millis(100), handle)
            .await
            .expect("should complete within timeout")
            .unwrap();
        assert!(notified);
    }

    #[test]
    fn disconnect_without_connect_does_not_underflow() {
        let tracker = ClientTracker::new(Duration::from_secs(30));
        assert_eq!(tracker.client_count(), 0);

        // Should be a no-op, not underflow.
        tracker.on_client_disconnected();
        assert_eq!(tracker.client_count(), 0);
        assert!(!tracker.is_active());
    }
}
