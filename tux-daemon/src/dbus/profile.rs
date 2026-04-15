//! D-Bus Profile interface: `com.tuxedocomputers.tccd.Profile`.

use std::sync::{Arc, RwLock};

use tokio::sync::watch;
use zbus::interface;
use zbus::object_server::SignalEmitter;

use crate::config::ProfileAssignments;
use crate::power_monitor::PowerState;
use crate::profile_apply::ProfileApplier;
use crate::profile_store::ProfileStore;
use tux_core::profile::TuxProfile;

/// D-Bus object implementing the Profile interface.
pub struct ProfileInterface {
    store: Arc<RwLock<ProfileStore>>,
    assignments_tx: Arc<watch::Sender<ProfileAssignments>>,
    assignments_rx: watch::Receiver<ProfileAssignments>,
    // Keep applier and power_rx in the struct for potential future use
    // and to maintain the public constructor API used by tests.
    #[allow(dead_code)]
    applier: Arc<ProfileApplier>,
    #[allow(dead_code)]
    power_rx: watch::Receiver<PowerState>,
    daemon_config: Arc<std::sync::RwLock<crate::config::DaemonConfig>>,
}

impl ProfileInterface {
    pub fn new(
        store: Arc<RwLock<ProfileStore>>,
        assignments_tx: Arc<watch::Sender<ProfileAssignments>>,
        assignments_rx: watch::Receiver<ProfileAssignments>,
        applier: Arc<ProfileApplier>,
        power_rx: watch::Receiver<PowerState>,
        daemon_config: Arc<std::sync::RwLock<crate::config::DaemonConfig>>,
    ) -> Self {
        Self {
            store,
            assignments_tx,
            assignments_rx,
            applier,
            power_rx,
            daemon_config,
        }
    }

    /// Core logic for setting the active profile — used by D-Bus method and tests.
    pub fn set_active_profile_inner(&self, id: &str, state: &str) -> zbus::fdo::Result<()> {
        // Validate state argument first (before acquiring any locks).
        if state != "ac" && state != "battery" {
            return Err(zbus::fdo::Error::InvalidArgs(format!(
                "state must be 'ac' or 'battery', got '{state}'"
            )));
        }

        // Single lock acquisition: validate, apply if needed, then update assignments.
        let store = self
            .store
            .read()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        // Validate the profile exists.
        if store.get(id).is_none() {
            return Err(zbus::fdo::Error::Failed(format!("ProfileNotFound: '{id}'")));
        }

        // Update assignments — the auto_switch_loop watcher will detect
        // this change and apply the profile.  We do NOT apply directly here
        // to avoid a double-apply race: the watcher and this method would
        // both write to the EC concurrently, and some ECs (notably Uniwill)
        // revert sysfs values when hit with rapid successive writes.
        let id_string = id.to_string();
        match state {
            "ac" => {
                self.assignments_tx
                    .send_modify(|a| a.ac_profile = id_string);
            }
            "battery" => {
                self.assignments_tx
                    .send_modify(|a| a.battery_profile = id_string);
            }
            _ => unreachable!(),
        }

        if let Ok(mut config) = self.daemon_config.write() {
            config.profiles = self.assignments_rx.borrow().clone();
            if let Err(e) = config.save(std::path::Path::new(crate::config::DEFAULT_CONFIG_PATH)) {
                tracing::warn!("failed to save profile assignments: {e}");
            }
        }

        Ok(())
    }
}

/// Map io::Error to the appropriate D-Bus error.
fn map_io_error(e: std::io::Error) -> zbus::fdo::Error {
    match e.kind() {
        std::io::ErrorKind::NotFound => zbus::fdo::Error::Failed(format!("ProfileNotFound: {e}")),
        std::io::ErrorKind::PermissionDenied => {
            zbus::fdo::Error::Failed(format!("ProfileReadOnly: {e}"))
        }
        std::io::ErrorKind::AlreadyExists => {
            zbus::fdo::Error::Failed(format!("InvalidProfile: {e}"))
        }
        _ => zbus::fdo::Error::Failed(format!("StorageError: {e}")),
    }
}

#[interface(name = "com.tuxedocomputers.tccd.Profile")]
impl ProfileInterface {
    /// List all profiles (builtins + custom) as a TOML string.
    fn list_profiles(&self) -> zbus::fdo::Result<String> {
        let store = self
            .store
            .read()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        let profiles: Vec<TuxProfile> = store.list().into_iter().cloned().collect();
        // Wrap in a table for valid TOML serialization.
        #[derive(serde::Serialize)]
        struct ProfileList {
            profiles: Vec<TuxProfile>,
        }
        let wrapper = ProfileList { profiles };
        toml::to_string(&wrapper).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get a single profile by ID as a TOML string.
    fn get_profile(&self, id: &str) -> zbus::fdo::Result<String> {
        let store = self
            .store
            .read()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        let profile = store
            .get(id)
            .ok_or_else(|| zbus::fdo::Error::Failed(format!("ProfileNotFound: '{id}'")))?;
        toml::to_string_pretty(profile).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Create a new custom profile from a TOML string. Returns the new ID.
    fn create_profile(&self, toml_str: &str) -> zbus::fdo::Result<String> {
        let profile: TuxProfile = toml::from_str(toml_str)
            .map_err(|e| zbus::fdo::Error::Failed(format!("InvalidProfile: {e}")))?;
        let mut store = self
            .store
            .write()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        store.create(profile).map_err(map_io_error)
    }

    /// Update an existing custom profile from a TOML string.
    fn update_profile(&self, id: &str, toml_str: &str) -> zbus::fdo::Result<()> {
        let profile: TuxProfile = toml::from_str(toml_str)
            .map_err(|e| zbus::fdo::Error::Failed(format!("InvalidProfile: {e}")))?;
        let mut store = self
            .store
            .write()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        store.update(id, profile).map_err(map_io_error)?;

        // If the updated profile is currently active, re-apply it to hardware
        // by nudging the assignments watcher (triggers auto_switch_loop).
        let assignments = self.assignments_rx.borrow();
        let power = *self.power_rx.borrow();
        let active_id = match power {
            PowerState::Ac => &assignments.ac_profile,
            PowerState::Battery => &assignments.battery_profile,
        };
        if active_id == id {
            drop(assignments);
            // send_modify with identity triggers changed() notification.
            self.assignments_tx.send_modify(|_| {});
        }

        Ok(())
    }

    /// Delete a custom profile by ID.
    fn delete_profile(&self, id: &str) -> zbus::fdo::Result<()> {
        let mut store = self
            .store
            .write()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        store.delete(id).map_err(map_io_error)
    }

    /// Copy a profile, returning the new profile's ID.
    fn copy_profile(&self, id: &str) -> zbus::fdo::Result<String> {
        let mut store = self
            .store
            .write()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        store.copy(id).map_err(map_io_error)
    }

    /// Set the active profile for a given power state ("ac" or "battery").
    ///
    /// Validates the profile exists, updates assignments, and applies
    /// immediately if the current power state matches.
    async fn set_active_profile(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        id: &str,
        state: &str,
    ) -> zbus::fdo::Result<()> {
        self.set_active_profile_inner(id, state)?;

        // Emit the ProfileChanged signal.
        Self::profile_changed(&emitter, id, state)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("signal emission failed: {e}")))?;

        Ok(())
    }

    /// Get the current profile assignments as TOML.
    fn get_profile_assignments(&self) -> zbus::fdo::Result<String> {
        let assignments = self.assignments_rx.borrow().clone();
        toml::to_string(&assignments).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Signal emitted on profile activation.
    #[zbus(signal)]
    async fn profile_changed(
        signal_emitter: &zbus::object_server::SignalEmitter<'_>,
        profile_id: &str,
        state: &str,
    ) -> zbus::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tux_core::fan_curve::FanConfig;

    #[derive(serde::Deserialize)]
    struct ProfileList {
        profiles: Vec<tux_core::profile::TuxProfile>,
    }

    fn make_test_iface(dir: &std::path::Path) -> (ProfileInterface, watch::Receiver<FanConfig>) {
        let store = Arc::new(RwLock::new(ProfileStore::new(dir).unwrap()));
        let assignments = ProfileAssignments::default();
        let (assignments_tx, assignments_rx) = watch::channel(assignments);
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
        let (_, power_rx) = watch::channel(PowerState::Ac);
        let daemon_config = Arc::new(std::sync::RwLock::new(
            crate::config::DaemonConfig::default(),
        ));

        (
            ProfileInterface::new(
                store,
                Arc::new(assignments_tx),
                assignments_rx,
                applier,
                power_rx,
                daemon_config,
            ),
            config_rx,
        )
    }

    #[test]
    fn list_profiles_returns_builtins() {
        let dir = tempfile::tempdir().unwrap();
        let (iface, _rx) = make_test_iface(dir.path());
        let result = iface.list_profiles().unwrap();
        let list: ProfileList = toml::from_str(&result).unwrap();
        assert_eq!(list.profiles.len(), 4);
    }

    #[test]
    fn get_profile_returns_correct_id() {
        let dir = tempfile::tempdir().unwrap();
        let (iface, _rx) = make_test_iface(dir.path());
        let result = iface.get_profile("__office__").unwrap();
        let profile: tux_core::profile::TuxProfile = toml::from_str(&result).unwrap();
        assert_eq!(profile.id, "__office__");
        assert_eq!(profile.name, "Office");
    }

    #[test]
    fn get_profile_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let (iface, _rx) = make_test_iface(dir.path());
        let err = iface.get_profile("nonexistent").unwrap_err();
        assert!(err.to_string().contains("ProfileNotFound"));
    }

    #[test]
    fn create_then_list_includes_new() {
        let dir = tempfile::tempdir().unwrap();
        let (iface, _rx) = make_test_iface(dir.path());

        let toml = r#"
id = "custom1"
name = "Custom Profile"
description = "Test"
is_default = false

[fan]
enabled = true
mode = "CustomCurve"
min_speed_percent = 20
max_speed_percent = 100
curve = []

[cpu]
governor = "schedutil"
no_turbo = false

[keyboard]
brightness = 100

[display]

[charging]
"#;
        let id = iface.create_profile(toml).unwrap();
        assert_eq!(id, "custom1");

        let list = iface.list_profiles().unwrap();
        let parsed: ProfileList = toml::from_str(&list).unwrap();
        assert_eq!(parsed.profiles.len(), 5);
        assert!(parsed.profiles.iter().any(|p| p.id == "custom1"));
    }

    #[test]
    fn update_then_get_reflects_change() {
        let dir = tempfile::tempdir().unwrap();
        let (iface, _rx) = make_test_iface(dir.path());

        let create_toml = r#"
id = "custom2"
name = "Original Name"
description = "Test"
is_default = false

[fan]
enabled = true
mode = "Auto"
min_speed_percent = 0
max_speed_percent = 100
curve = []

[cpu]
governor = "powersave"
no_turbo = false

[keyboard]
brightness = 50

[display]

[charging]
"#;
        iface.create_profile(create_toml).unwrap();

        let update_toml = r#"
id = "custom2"
name = "Updated Name"
description = "Updated"
is_default = false

[fan]
enabled = true
mode = "CustomCurve"
min_speed_percent = 30
max_speed_percent = 100
curve = []

[cpu]
governor = "performance"
no_turbo = false

[keyboard]
brightness = 80

[display]

[charging]
"#;
        iface.update_profile("custom2", update_toml).unwrap();

        let result = iface.get_profile("custom2").unwrap();
        let profile: tux_core::profile::TuxProfile = toml::from_str(&result).unwrap();
        assert_eq!(profile.name, "Updated Name");
        assert_eq!(profile.fan.min_speed_percent, 30);
    }

    #[test]
    fn delete_then_list_excludes() {
        let dir = tempfile::tempdir().unwrap();
        let (iface, _rx) = make_test_iface(dir.path());

        let toml = r#"
id = "to_delete"
name = "Delete Me"
description = ""
is_default = false

[fan]
enabled = false
mode = "Auto"
min_speed_percent = 0
max_speed_percent = 100
curve = []

[cpu]
governor = "powersave"
no_turbo = false

[keyboard]
brightness = 50

[display]

[charging]
"#;
        iface.create_profile(toml).unwrap();
        assert!(iface.get_profile("to_delete").is_ok());

        iface.delete_profile("to_delete").unwrap();

        let err = iface.get_profile("to_delete").unwrap_err();
        assert!(err.to_string().contains("ProfileNotFound"));
    }

    #[test]
    fn delete_builtin_fails() {
        let dir = tempfile::tempdir().unwrap();
        let (iface, _rx) = make_test_iface(dir.path());
        let err = iface.delete_profile("__office__").unwrap_err();
        assert!(err.to_string().contains("ProfileReadOnly"));
    }

    #[test]
    fn update_builtin_fails() {
        let dir = tempfile::tempdir().unwrap();
        let (iface, _rx) = make_test_iface(dir.path());
        let err = iface.update_profile("__office__", "").unwrap_err();
        // Could be InvalidProfile (parse error) or ProfileReadOnly
        assert!(
            err.to_string().contains("InvalidProfile")
                || err.to_string().contains("ProfileReadOnly")
        );
    }

    #[test]
    fn copy_profile_creates_new() {
        let dir = tempfile::tempdir().unwrap();
        let (iface, _rx) = make_test_iface(dir.path());

        let new_id = iface.copy_profile("__office__").unwrap();
        assert_ne!(new_id, "__office__");

        let list = iface.list_profiles().unwrap();
        let parsed: ProfileList = toml::from_str(&list).unwrap();
        assert_eq!(parsed.profiles.len(), 5);

        let copy = parsed.profiles.iter().find(|p| p.id == new_id).unwrap();
        assert!(copy.name.contains("Copy"));
    }

    #[test]
    fn set_active_profile_updates_assignments() {
        let dir = tempfile::tempdir().unwrap();
        let (iface, _rx) = make_test_iface(dir.path());

        iface.set_active_profile_inner("__quiet__", "ac").unwrap();

        let assignments_toml = iface.get_profile_assignments().unwrap();
        let assignments: ProfileAssignments = toml::from_str(&assignments_toml).unwrap();
        assert_eq!(assignments.ac_profile, "__quiet__");
        // battery should be unchanged
        assert_eq!(assignments.battery_profile, "__quiet__");
    }

    #[test]
    fn set_active_profile_invalid_id_fails() {
        let dir = tempfile::tempdir().unwrap();
        let (iface, _rx) = make_test_iface(dir.path());

        let err = iface
            .set_active_profile_inner("nonexistent", "ac")
            .unwrap_err();
        assert!(err.to_string().contains("ProfileNotFound"));
    }

    #[test]
    fn create_invalid_toml_fails() {
        let dir = tempfile::tempdir().unwrap();
        let (iface, _rx) = make_test_iface(dir.path());

        let err = iface.create_profile("not valid toml {{{").unwrap_err();
        assert!(err.to_string().contains("InvalidProfile"));
    }
}
