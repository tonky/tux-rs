//! Profile store: CRUD operations on TuxProfiles with TOML file persistence.
//!
//! Built-in profiles are always available and cannot be modified or deleted.
//! Custom profiles are stored as individual TOML files in the profiles directory.

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

use tracing::{info, warn};

use tux_core::profile::{TuxProfile, builtin_profiles};

/// CRUD store for profiles, backed by TOML files on disk.
pub struct ProfileStore {
    profiles_dir: PathBuf,
    builtins: HashMap<String, TuxProfile>,
    custom_profiles: HashMap<String, TuxProfile>,
}

#[allow(dead_code)] // CRUD methods used by D-Bus API in Phase 5.3
impl ProfileStore {
    /// Create a new store, loading custom profiles from `dir`.
    ///
    /// Creates the directory if it doesn't exist.
    pub fn new(dir: impl Into<PathBuf>) -> io::Result<Self> {
        let profiles_dir = dir.into();
        std::fs::create_dir_all(&profiles_dir)?;

        let builtins: HashMap<String, TuxProfile> = builtin_profiles()
            .into_iter()
            .map(|p| (p.id.clone(), p))
            .collect();

        let mut store = Self {
            profiles_dir,
            builtins,
            custom_profiles: HashMap::new(),
        };
        store.load_custom_profiles();
        Ok(store)
    }

    /// List all profiles (built-ins first, then custom sorted by ID).
    pub fn list(&self) -> Vec<&TuxProfile> {
        let mut result: Vec<&TuxProfile> = self.builtins.values().collect();
        result.sort_by_key(|p| &p.id);
        let mut custom: Vec<&TuxProfile> = self.custom_profiles.values().collect();
        custom.sort_by_key(|p| &p.id);
        result.extend(custom);
        result
    }

    /// Get a profile by ID (checks both built-in and custom).
    pub fn get(&self, id: &str) -> Option<&TuxProfile> {
        self.builtins
            .get(id)
            .or_else(|| self.custom_profiles.get(id))
    }

    /// Create a new custom profile. Returns the ID on success.
    pub fn create(&mut self, profile: TuxProfile) -> io::Result<String> {
        let id = &profile.id;
        Self::validate_id(id)?;
        if self.builtins.contains_key(id) || self.custom_profiles.contains_key(id) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("profile with id '{id}' already exists"),
            ));
        }
        self.persist_profile(&profile)?;
        let id = profile.id.clone();
        self.custom_profiles.insert(id.clone(), profile);
        Ok(id)
    }

    /// Update an existing custom profile.
    pub fn update(&mut self, id: &str, profile: TuxProfile) -> io::Result<()> {
        if self.builtins.contains_key(id) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "built-in profiles cannot be updated",
            ));
        }
        if !self.custom_profiles.contains_key(id) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("profile '{id}' not found"),
            ));
        }
        self.persist_profile(&profile)?;
        self.custom_profiles.insert(id.to_string(), profile);
        Ok(())
    }

    /// Update only the fan curve settings within an existing profile.
    /// Used when the user edits the fan curve from the TUI — persists the
    /// curve back to the active profile so it survives profile switches.
    pub fn update_fan_settings(
        &mut self,
        id: &str,
        config: &tux_core::fan_curve::FanConfig,
    ) -> io::Result<()> {
        // Builtin profiles can't be modified on disk.
        if self.builtins.contains_key(id) {
            return Ok(());
        }
        let profile = match self.custom_profiles.get_mut(id) {
            Some(p) => p,
            None => return Ok(()), // profile not found — nothing to persist
        };
        profile.fan.mode = config.mode;
        profile.fan.min_speed_percent = config.min_speed_percent;
        profile.fan.curve = config.curve.clone();
        let snapshot = profile.clone();
        self.persist_profile(&snapshot)?;
        Ok(())
    }

    /// Delete a custom profile.
    pub fn delete(&mut self, id: &str) -> io::Result<()> {
        if self.builtins.contains_key(id) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "built-in profiles cannot be deleted",
            ));
        }
        if self.custom_profiles.remove(id).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("profile '{id}' not found"),
            ));
        }
        let path = self.profile_path(id)?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Copy a profile (built-in or custom) as a new custom profile.
    ///
    /// Returns the ID of the new profile.
    pub fn copy(&mut self, id: &str) -> io::Result<String> {
        let source = self
            .get(id)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, format!("profile '{id}' not found"))
            })?
            .clone();

        let new_id = self.generate_copy_id(&source.name);
        let mut copy = source;
        copy.id = new_id.clone();
        copy.name = format!("{} (Copy)", copy.name);
        copy.is_default = false;

        self.create(copy)?;
        Ok(new_id)
    }

    // --- Private helpers ---

    /// Load all TOML files from the profiles directory.
    fn load_custom_profiles(&mut self) {
        let entries = match std::fs::read_dir(&self.profiles_dir) {
            Ok(entries) => entries,
            Err(e) => {
                warn!("failed to read profiles directory: {e}");
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                match std::fs::read_to_string(&path) {
                    Ok(contents) => match toml::from_str::<TuxProfile>(&contents) {
                        Ok(profile) => {
                            if self.custom_profiles.contains_key(&profile.id) {
                                warn!(
                                    "duplicate profile ID '{}' at {}; skipping",
                                    profile.id,
                                    path.display()
                                );
                                continue;
                            }
                            info!("loaded custom profile: {} ({})", profile.name, profile.id);
                            self.custom_profiles.insert(profile.id.clone(), profile);
                        }
                        Err(e) => {
                            warn!("invalid profile TOML at {}: {e}", path.display());
                        }
                    },
                    Err(e) => {
                        warn!("failed to read {}: {e}", path.display());
                    }
                }
            }
        }
    }

    /// Validate a profile ID is safe for use as a filename.
    fn validate_id(id: &str) -> io::Result<()> {
        if id.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "profile ID must not be empty",
            ));
        }
        if id.contains("..") || id.contains('/') || id.contains('\\') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "profile ID cannot contain path separators or '..'",
            ));
        }
        if id.starts_with("__") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "custom profile IDs must not start with '__'",
            ));
        }
        Ok(())
    }

    /// Write a profile to its TOML file atomically (write-then-rename).
    fn persist_profile(&self, profile: &TuxProfile) -> io::Result<()> {
        let path = self.profile_path(&profile.id)?;
        let contents = toml::to_string_pretty(profile)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        let temp_path = path.with_extension("toml.tmp");
        std::fs::write(&temp_path, &contents)?;
        std::fs::rename(&temp_path, &path)?;
        Ok(())
    }

    /// Get the file path for a profile ID (with path traversal check).
    fn profile_path(&self, id: &str) -> io::Result<PathBuf> {
        if id.contains("..") || id.contains('/') || id.contains('\\') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "profile ID contains path traversal characters",
            ));
        }
        Ok(self.profiles_dir.join(format!("{id}.toml")))
    }

    /// Generate a unique ID for a copied profile.
    fn generate_copy_id(&self, name: &str) -> String {
        let base = name
            .to_lowercase()
            .replace(' ', "-")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect::<String>();

        let candidate = format!("{base}-copy");
        if !self.builtins.contains_key(&candidate) && !self.custom_profiles.contains_key(&candidate)
        {
            return candidate;
        }

        for i in 2.. {
            let candidate = format!("{base}-copy-{i}");
            if !self.builtins.contains_key(&candidate)
                && !self.custom_profiles.contains_key(&candidate)
            {
                return candidate;
            }
        }
        unreachable!("could not generate unique copy ID")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tux_core::fan_curve::{FanCurvePoint, FanMode};
    use tux_core::profile::{CpuSettings, FanProfileSettings};

    fn test_profile(id: &str) -> TuxProfile {
        TuxProfile {
            id: id.to_string(),
            name: format!("Test {id}"),
            description: String::new(),
            is_default: false,
            fan: FanProfileSettings {
                enabled: true,
                mode: FanMode::Auto,
                min_speed_percent: 0,
                max_speed_percent: 100,
                curve: vec![],
                ..Default::default()
            },
            cpu: CpuSettings::default(),
            keyboard: Default::default(),
            display: Default::default(),
            charging: Default::default(),
            odm_profile: None,
            tdp: None,
            gpu: None,
        }
    }

    #[test]
    fn list_returns_builtins() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(dir.path()).unwrap();
        let profiles = store.list();
        assert_eq!(profiles.len(), 4, "should have 4 built-in profiles");
        for p in &profiles {
            assert!(p.is_default);
        }
    }

    #[test]
    fn builtin_cannot_be_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ProfileStore::new(dir.path()).unwrap();
        let result = store.delete("__quiet__");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn builtin_cannot_be_updated() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ProfileStore::new(dir.path()).unwrap();
        let profile = test_profile("__quiet__");
        let result = store.update("__quiet__", profile);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn create_and_get_custom_profile() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ProfileStore::new(dir.path()).unwrap();

        let profile = test_profile("my-gaming");
        store.create(profile.clone()).unwrap();

        let retrieved = store.get("my-gaming").unwrap();
        assert_eq!(retrieved.name, "Test my-gaming");
        assert_eq!(store.list().len(), 5); // 4 builtins + 1 custom
    }

    #[test]
    fn create_persists_to_file_and_reloads() {
        let dir = tempfile::tempdir().unwrap();

        // Create and persist.
        {
            let mut store = ProfileStore::new(dir.path()).unwrap();
            let mut profile = test_profile("persist-test");
            profile.fan.curve = vec![FanCurvePoint {
                temp: 50,
                speed: 60,
            }];
            store.create(profile).unwrap();
        }

        // Reload from disk.
        {
            let store = ProfileStore::new(dir.path()).unwrap();
            let p = store.get("persist-test").unwrap();
            assert_eq!(p.fan.curve.len(), 1);
            assert_eq!(p.fan.curve[0].temp, 50);
        }
    }

    #[test]
    fn duplicate_id_on_create_errors() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ProfileStore::new(dir.path()).unwrap();

        store.create(test_profile("dupe")).unwrap();
        let result = store.create(test_profile("dupe"));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::AlreadyExists);
    }

    #[test]
    fn update_custom_profile() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ProfileStore::new(dir.path()).unwrap();

        store.create(test_profile("updatable")).unwrap();
        let mut updated = test_profile("updatable");
        updated.name = "Updated Name".to_string();
        store.update("updatable", updated).unwrap();

        let p = store.get("updatable").unwrap();
        assert_eq!(p.name, "Updated Name");

        // Reload verifies persistence.
        let store2 = ProfileStore::new(dir.path()).unwrap();
        assert_eq!(store2.get("updatable").unwrap().name, "Updated Name");
    }

    #[test]
    fn delete_custom_profile() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ProfileStore::new(dir.path()).unwrap();

        store.create(test_profile("deleteme")).unwrap();
        assert!(store.get("deleteme").is_some());

        store.delete("deleteme").unwrap();
        assert!(store.get("deleteme").is_none());
        assert_eq!(store.list().len(), 4); // back to builtins only

        // File should be gone.
        assert!(!dir.path().join("deleteme.toml").exists());
    }

    #[test]
    fn copy_builtin_creates_custom() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ProfileStore::new(dir.path()).unwrap();

        let new_id = store.copy("__quiet__").unwrap();
        assert!(!new_id.starts_with("__"));

        let copied = store.get(&new_id).unwrap();
        assert!(!copied.is_default);
        assert!(copied.name.contains("Copy"));
        assert_eq!(store.list().len(), 5);
    }

    #[test]
    fn empty_profiles_dir_returns_builtins_only() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(dir.path()).unwrap();
        assert_eq!(store.list().len(), 4);
    }

    #[test]
    fn invalid_toml_in_profiles_dir_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("bad.toml"), "not valid toml {{{").unwrap();

        let store = ProfileStore::new(dir.path()).unwrap();
        // Should still have the 4 builtins, bad file skipped
        assert_eq!(store.list().len(), 4);
    }

    #[test]
    fn path_traversal_in_id_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ProfileStore::new(dir.path()).unwrap();

        let result = store.create(test_profile("../../../etc/evil"));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidInput);

        let result = store.create(test_profile("sub/dir"));
        assert!(result.is_err());

        let result = store.create(test_profile(""));
        assert!(result.is_err());
    }

    #[test]
    fn dunder_id_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ProfileStore::new(dir.path()).unwrap();

        let result = store.create(test_profile("__sneaky__"));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }
}
