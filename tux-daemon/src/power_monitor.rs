//! Power state detection and monitoring via inotify on sysfs.

use std::io;
use std::path::{Path, PathBuf};

use tokio::sync::{broadcast, watch};
use tokio_stream::StreamExt;
use tracing::{debug, info, warn};

/// Whether the laptop is on AC power or battery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PowerState {
    Ac,
    Battery,
}

/// Inotify mask for the AC `online` file itself.
#[cfg(target_os = "linux")]
fn power_file_watch_mask() -> inotify::WatchMask {
    inotify::WatchMask::MODIFY | inotify::WatchMask::ATTRIB | inotify::WatchMask::CLOSE_WRITE
}

/// Inotify mask for the parent power-supply directory as fallback.
#[cfg(target_os = "linux")]
fn power_dir_watch_mask() -> inotify::WatchMask {
    inotify::WatchMask::MODIFY
        | inotify::WatchMask::ATTRIB
        | inotify::WatchMask::CREATE
        | inotify::WatchMask::MOVED_TO
}

/// Refresh interval used as a fallback in case inotify misses an event.
const POWER_RESYNC_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);

/// Find sysfs `online` files for AC-like power supplies.
fn find_ac_online_paths() -> io::Result<Vec<PathBuf>> {
    let base = Path::new("/sys/class/power_supply");
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(base)?.flatten() {
        let supply_dir = entry.path();
        let online = supply_dir.join("online");
        if !online.exists() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        let supply_type = std::fs::read_to_string(supply_dir.join("type"))
            .unwrap_or_default()
            .trim()
            .to_string();

        let is_ac_like = supply_type.eq_ignore_ascii_case("Mains")
            || name.starts_with("AC")
            || name.starts_with("ADP");

        if is_ac_like {
            paths.push(online);
        }
    }

    if paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no AC-like power supply with online file found in /sys/class/power_supply/",
        ));
    }

    paths.sort();
    Ok(paths)
}

/// Detect the current power state from a sysfs `online` file.
pub fn detect_power_state(online_path: &Path) -> io::Result<PowerState> {
    let content = std::fs::read_to_string(online_path)?;
    match content.trim() {
        "1" => Ok(PowerState::Ac),
        "0" => Ok(PowerState::Battery),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected power supply online value: '{other}'"),
        )),
    }
}

/// Detect state from all AC-like online paths:
/// - AC if any source is online
/// - Battery if all are offline
fn detect_power_state_from_paths(online_paths: &[PathBuf]) -> io::Result<PowerState> {
    let mut any_online = false;
    for p in online_paths {
        match detect_power_state(p) {
            Ok(PowerState::Ac) => {
                any_online = true;
            }
            Ok(PowerState::Battery) => {}
            Err(e) => {
                // Don't fail hard on a single flaky supply node.
                warn!("failed reading {}: {e}", p.display());
            }
        }
    }

    if any_online {
        Ok(PowerState::Ac)
    } else {
        Ok(PowerState::Battery)
    }
}

/// Watches sysfs power supply for AC/battery transitions via inotify.
pub struct PowerStateMonitor {
    state_tx: watch::Sender<PowerState>,
    online_paths: Vec<PathBuf>,
}

impl PowerStateMonitor {
    /// Create a new monitor. Returns the monitor and a receiver for state changes.
    ///
    /// Uses the system sysfs path unless one is provided.
    pub fn new(online_path: Option<PathBuf>) -> io::Result<(Self, watch::Receiver<PowerState>)> {
        let online_paths = match online_path {
            Some(p) => vec![p],
            None => find_ac_online_paths()?,
        };

        let initial_state = detect_power_state_from_paths(&online_paths)?;
        let (state_tx, state_rx) = watch::channel(initial_state);
        info!(
            "power monitor initialized: {:?} (watching {} source(s))",
            initial_state,
            online_paths.len()
        );
        for p in &online_paths {
            debug!("power monitor source: {}", p.display());
        }

        Ok((
            Self {
                state_tx,
                online_paths,
            },
            state_rx,
        ))
    }

    /// Run the monitor loop, watching for power state changes.
    ///
    /// Debounces rapid transitions (500ms).
    #[cfg(target_os = "linux")]
    pub async fn run(&self, mut shutdown: broadcast::Receiver<()>) {
        use inotify::Inotify;

        let inotify = match Inotify::init() {
            Ok(i) => i,
            Err(e) => {
                warn!("failed to init inotify: {e}, power monitoring falling back to polling");
                self.run_polling(shutdown).await;
                return;
            }
        };

        // Watch online files directly. Power supply state changes are often
        // reported as ATTRIB updates rather than plain MODIFY writes.
        let file_mask = power_file_watch_mask();
        for online_path in &self.online_paths {
            if let Err(e) = inotify.watches().add(online_path, file_mask) {
                warn!("failed to watch {}: {e}", online_path.display());
            }
        }

        // Also watch parent dirs as a fallback for drivers that replace/recreate
        // online files and only emit directory-level notifications.
        for online_path in &self.online_paths {
            if let Some(parent) = online_path.parent() {
                let _ = inotify.watches().add(parent, power_dir_watch_mask());
            }
        }
        let _ = inotify
            .watches()
            .add(Path::new("/sys/class/power_supply"), power_dir_watch_mask());

        // The buffer must live as long as the event stream - both are in this scope.
        let mut buffer = [0u8; 1024];
        let mut inotify = match inotify.into_event_stream(&mut buffer) {
            Ok(stream) => stream,
            Err(e) => {
                warn!(
                    "failed to create inotify event stream: {e}, power monitoring falling back to polling"
                );
                self.run_polling(shutdown).await;
                return;
            }
        };

        let mut resync_tick = tokio::time::interval(POWER_RESYNC_INTERVAL);
        resync_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                event = inotify.next() => {
                    match event {
                        Some(Ok(_)) => {}
                        Some(Err(e)) => {
                            warn!("inotify event error: {e}");
                            continue;
                        }
                        None => {
                            warn!("inotify stream ended, falling back to polling");
                            self.run_polling(shutdown).await;
                            return;
                        }
                    }

                    // Debounce: wait 500ms before reading.
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                    self.check_now();
                }
                _ = resync_tick.tick() => {
                    self.check_now();
                }
                _ = shutdown.recv() => {
                    info!("power monitor shutting down");
                    return;
                }
            }
        }
    }

    /// Run the monitor loop, watching for power state changes via polling.
    #[cfg(not(target_os = "linux"))]
    pub async fn run(&self, shutdown: broadcast::Receiver<()>) {
        self.run_polling(shutdown).await;
    }

    async fn run_polling(&self, mut shutdown: broadcast::Receiver<()>) {
        info!("power monitor starting in polling mode");
        let mut resync_tick = tokio::time::interval(POWER_RESYNC_INTERVAL);
        resync_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = resync_tick.tick() => {
                    self.check_now();
                }
                _ = shutdown.recv() => {
                    info!("power monitor shutting down");
                    return;
                }
            }
        }
    }

    fn check_now(&self) {
        match detect_power_state_from_paths(&self.online_paths) {
            Ok(new_state) => {
                let old_state = *self.state_tx.borrow();
                if new_state != old_state {
                    info!("power state changed: {:?} -> {:?}", old_state, new_state);
                    let _ = self.state_tx.send(new_state);
                } else {
                    debug!("power supply check, state unchanged: {:?}", new_state);
                }
            }
            Err(e) => {
                warn!("failed to read power state: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_ac_state() {
        let dir = tempfile::tempdir().unwrap();
        let online = dir.path().join("online");
        std::fs::write(&online, "1\n").unwrap();

        assert_eq!(detect_power_state(&online).unwrap(), PowerState::Ac);
    }

    #[test]
    fn detect_battery_state() {
        let dir = tempfile::tempdir().unwrap();
        let online = dir.path().join("online");
        std::fs::write(&online, "0\n").unwrap();

        assert_eq!(detect_power_state(&online).unwrap(), PowerState::Battery);
    }

    #[test]
    fn detect_invalid_value() {
        let dir = tempfile::tempdir().unwrap();
        let online = dir.path().join("online");
        std::fs::write(&online, "2\n").unwrap();

        assert!(detect_power_state(&online).is_err());
    }

    #[test]
    fn monitor_creates_with_initial_state() {
        let dir = tempfile::tempdir().unwrap();
        let online = dir.path().join("online");
        std::fs::write(&online, "1\n").unwrap();

        let (_monitor, rx) = PowerStateMonitor::new(Some(online)).unwrap();
        assert_eq!(*rx.borrow(), PowerState::Ac);
    }

    #[test]
    fn detect_power_state_from_multiple_paths_any_online_is_ac() {
        let dir = tempfile::tempdir().unwrap();
        let p0 = dir.path().join("ac0_online");
        let p1 = dir.path().join("ac1_online");
        std::fs::write(&p0, "0\n").unwrap();
        std::fs::write(&p1, "1\n").unwrap();

        let state = detect_power_state_from_paths(&[p0, p1]).unwrap();
        assert_eq!(state, PowerState::Ac);
    }

    #[test]
    fn detect_power_state_from_multiple_paths_all_offline_is_battery() {
        let dir = tempfile::tempdir().unwrap();
        let p0 = dir.path().join("ac0_online");
        let p1 = dir.path().join("ac1_online");
        std::fs::write(&p0, "0\n").unwrap();
        std::fs::write(&p1, "0\n").unwrap();

        let state = detect_power_state_from_paths(&[p0, p1]).unwrap();
        assert_eq!(state, PowerState::Battery);
    }

    #[test]
    fn power_state_toml_roundtrip() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct Wrapper {
            state: PowerState,
        }

        let ac = Wrapper {
            state: PowerState::Ac,
        };
        let battery = Wrapper {
            state: PowerState::Battery,
        };

        let ac_str = toml::to_string(&ac).unwrap();
        let battery_str = toml::to_string(&battery).unwrap();

        let ac_back: Wrapper = toml::from_str(&ac_str).unwrap();
        let battery_back: Wrapper = toml::from_str(&battery_str).unwrap();

        assert_eq!(ac, ac_back);
        assert_eq!(battery, battery_back);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn file_watch_mask_includes_attr_and_write_events() {
        let mask = power_file_watch_mask();
        assert!(mask.contains(inotify::WatchMask::ATTRIB));
        assert!(mask.contains(inotify::WatchMask::MODIFY));
        assert!(mask.contains(inotify::WatchMask::CLOSE_WRITE));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn dir_watch_mask_includes_recreate_events() {
        let mask = power_dir_watch_mask();
        assert!(mask.contains(inotify::WatchMask::CREATE));
        assert!(mask.contains(inotify::WatchMask::MOVED_TO));
        assert!(mask.contains(inotify::WatchMask::ATTRIB));
    }
}
