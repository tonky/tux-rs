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
fn power_file_watch_mask() -> inotify::WatchMask {
    inotify::WatchMask::MODIFY | inotify::WatchMask::ATTRIB | inotify::WatchMask::CLOSE_WRITE
}

/// Inotify mask for the parent power-supply directory as fallback.
fn power_dir_watch_mask() -> inotify::WatchMask {
    inotify::WatchMask::MODIFY
        | inotify::WatchMask::ATTRIB
        | inotify::WatchMask::CREATE
        | inotify::WatchMask::MOVED_TO
}

/// Find the sysfs `online` file for the AC power supply.
fn find_ac_online_path() -> io::Result<PathBuf> {
    let base = Path::new("/sys/class/power_supply");
    for entry in std::fs::read_dir(base)?.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("AC") || name_str.starts_with("ADP") {
            let online = entry.path().join("online");
            if online.exists() {
                return Ok(online);
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no AC/ADP power supply found in /sys/class/power_supply/",
    ))
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

/// Watches sysfs power supply for AC/battery transitions via inotify.
pub struct PowerStateMonitor {
    state_tx: watch::Sender<PowerState>,
    online_path: PathBuf,
}

impl PowerStateMonitor {
    /// Create a new monitor. Returns the monitor and a receiver for state changes.
    ///
    /// Uses the system sysfs path unless one is provided.
    pub fn new(online_path: Option<PathBuf>) -> io::Result<(Self, watch::Receiver<PowerState>)> {
        let online_path = match online_path {
            Some(p) => p,
            None => find_ac_online_path()?,
        };

        let initial_state = detect_power_state(&online_path)?;
        let (state_tx, state_rx) = watch::channel(initial_state);
        info!(
            "power monitor initialized: {:?} (watching {})",
            initial_state,
            online_path.display()
        );

        Ok((
            Self {
                state_tx,
                online_path,
            },
            state_rx,
        ))
    }

    /// Run the monitor loop, watching for power state changes.
    ///
    /// Debounces rapid transitions (500ms).
    pub async fn run(&self, mut shutdown: broadcast::Receiver<()>) {
        use inotify::Inotify;

        let inotify = match Inotify::init() {
            Ok(i) => i,
            Err(e) => {
                warn!("failed to init inotify: {e}, power monitoring disabled");
                return;
            }
        };

        // Watch the online file directly. Power supply state changes are often
        // reported as ATTRIB updates rather than plain MODIFY writes.
        let file_mask = power_file_watch_mask();
        if let Err(e) = inotify.watches().add(&self.online_path, file_mask) {
            warn!(
                "failed to watch {}: {e}, power monitoring disabled",
                self.online_path.display()
            );
            return;
        }

        // Also watch parent dir as a fallback for drivers that replace/recreate
        // the online file and only emit directory-level notifications.
        let watch_path = self
            .online_path
            .parent()
            .unwrap_or(Path::new("/sys/class/power_supply"));
        let _ = inotify.watches().add(watch_path, power_dir_watch_mask());

        // The buffer must live as long as the event stream — both are in this scope.
        let mut buffer = [0u8; 1024];
        let mut inotify = match inotify.into_event_stream(&mut buffer) {
            Ok(stream) => stream,
            Err(e) => {
                warn!("failed to create inotify event stream: {e}, power monitoring disabled");
                return;
            }
        };

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
                            warn!("inotify stream ended");
                            return;
                        }
                    }

                    // Debounce: wait 500ms before reading.
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                    match detect_power_state(&self.online_path) {
                        Ok(new_state) => {
                            let old_state = *self.state_tx.borrow();
                            if new_state != old_state {
                                info!("power state changed: {:?} → {:?}", old_state, new_state);
                                let _ = self.state_tx.send(new_state);
                            } else {
                                debug!("power supply event, but state unchanged: {:?}", new_state);
                            }
                        }
                        Err(e) => {
                            warn!("failed to read power state: {e}");
                        }
                    }
                }
                _ = shutdown.recv() => {
                    info!("power monitor shutting down");
                    return;
                }
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
    fn file_watch_mask_includes_attr_and_write_events() {
        let mask = power_file_watch_mask();
        assert!(mask.contains(inotify::WatchMask::ATTRIB));
        assert!(mask.contains(inotify::WatchMask::MODIFY));
        assert!(mask.contains(inotify::WatchMask::CLOSE_WRITE));
    }

    #[test]
    fn dir_watch_mask_includes_recreate_events() {
        let mask = power_dir_watch_mask();
        assert!(mask.contains(inotify::WatchMask::CREATE));
        assert!(mask.contains(inotify::WatchMask::MOVED_TO));
        assert!(mask.contains(inotify::WatchMask::ATTRIB));
    }
}
