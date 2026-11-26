//! Local catalog facts and launcher-owned history. No invented download counts or ratings.
use crate::{engine, mods, Profile};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub favorites: BTreeSet<String>,
    pub launched: BTreeMap<String, u64>,
}
impl Preferences {
    pub fn load(home: &Path) -> Result<Self, String> {
        match std::fs::read(home.join("catalog-preferences.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    fn save(&self, home: &Path) -> Result<(), String> {
        wisp_mods::amod::atomic_write(
            &home.join("catalog-preferences.json"),
            &serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
    }
    pub fn toggle_favorite(home: &Path, name: &str) -> Result<(), String> {
        let mut data = Self::load(home)?;
        if !data.favorites.remove(name) {
            data.favorites.insert(name.to_string());
        }
        data.save(home)
    }
    pub fn launched(home: &Path, name: &str) -> Result<(), String> {
        let mut data = Self::load(home)?;
        data.launched.insert(
            name.to_string(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_secs(),
        );
        data.save(home)
    }
}
pub fn is_bundled(profile: &Profile) -> bool {
    engine::bundled().is_some_and(|bundle| {
        std::fs::canonicalize(&profile.binary)
            .ok()
            .zip(std::fs::canonicalize(bundle.binary).ok())
            .is_some_and(|(a, b)| a == b)
    })
}
pub fn locally_available(profile: &Profile) -> bool {
    profile.binary.is_file() && profile.resources.is_dir()
}
pub fn enabled_mods(profile: &Profile) -> usize {
    mods::list(profile)
        .iter()
        .filter(|m| m.enabled && m.definition.is_ok())
        .count()
}
pub fn matches(query: &str, fields: &[&str]) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || fields
            .iter()
            .any(|field| field.to_lowercase().contains(&query))
}
pub fn needs_update(installed: &str, available: &str) -> bool {
    semver::Version::parse(installed)
        .ok()
        .zip(semver::Version::parse(available).ok())
        .is_some_and(|(a, b)| b > a)
}
pub fn elapsed(timestamp: u64) -> (u64, &'static str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(timestamp, |d| d.as_secs());
    let seconds = now.saturating_sub(timestamp);
    if seconds < 60 {
        (0, "now")
    } else if seconds < 3600 {
        (seconds / 60, "minutes")
    } else if seconds < 86400 {
        (seconds / 3600, "hours")
    } else {
        (seconds / 86400, "days")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_uses_actual_metadata_and_handles_unicode() {
        assert!(matches(" ZOOM ", &["Zoom", "camera"]));
        assert!(matches("ü", &["Überblick"]));
        assert!(!matches("Fabric", &["Wisp", "Zoom"]));
        assert!(needs_update("1.9.0", "1.10.0"));
        assert!(!needs_update("1.10.0", "1.9.0"));
        assert!(!needs_update("1.0.0", "1.0.0"));
        assert!(!needs_update("unknown", "1.0.0"));
    }
    #[test]
    fn favorites_and_launch_history_round_trip_and_corrupt_files_are_preserved() {
        let root = std::env::temp_dir().join(format!(
            "wisp-catalog-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        Preferences::toggle_favorite(&root, "My world").unwrap();
        Preferences::launched(&root, "My world").unwrap();
        let data = Preferences::load(&root).unwrap();
        assert!(data.favorites.contains("My world"));
        assert!(data.launched.contains_key("My world"));
        std::fs::write(root.join("catalog-preferences.json"), b"corrupt").unwrap();
        assert!(Preferences::toggle_favorite(&root, "Other").is_err());
        assert_eq!(
            std::fs::read(root.join("catalog-preferences.json")).unwrap(),
            b"corrupt"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
