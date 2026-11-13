//! Per-instance Wisp client modules. Imports are validated before installation.
use crate::{
    i18n::{tr, trf},
    Profile,
};
use std::path::{Path, PathBuf};
use wisp_mods::amod::{self, Definition, ZoomConfig};

pub struct Installed {
    pub path: PathBuf,
    pub definition: Result<Definition, String>,
    pub enabled: bool,
}
pub fn list(profile: &Profile) -> Vec<Installed> {
    let mut paths: Vec<_> = std::fs::read_dir(profile.game_home.join(amod::DIR))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("amod"))
        })
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let definition = Definition::read(&path);
            let enabled = definition
                .as_ref()
                .is_ok_and(|m| amod::enabled(&profile.game_home, &m.id));
            Installed {
                path,
                definition,
                enabled,
            }
        })
        .collect()
}
pub fn install(profile: &Profile, source: &Path) -> Result<Definition, String> {
    if !source
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("amod"))
    {
        return Err(tr("Choose an Wisp .amod file."));
    }
    let m = Definition::read(source)?;
    let bytes = std::fs::read(source).map_err(|e| e.to_string())?;
    let destination = profile
        .game_home
        .join(amod::DIR)
        .join(format!("{}.amod", m.id));
    amod::atomic_write(&destination, &bytes)?;
    amod::set_enabled(&profile.game_home, &m.id, true)?;
    Ok(m)
}
pub fn install_zoom(profile: &Profile) -> Result<(), String> {
    let m = Definition::parse(amod::ZOOM_SOURCE)?;
    let on = !profile.game_home.join(amod::DIR).join("zoom.amod").exists()
        || amod::enabled(&profile.game_home, &m.id);
    amod::atomic_write(
        &profile.game_home.join(amod::DIR).join("zoom.amod"),
        &amod::package(amod::ZOOM_SOURCE)?,
    )?;
    amod::set_enabled(&profile.game_home, &m.id, on)
}
pub fn set_enabled(profile: &Profile, m: &Definition, on: bool) -> Result<(), String> {
    amod::set_enabled(&profile.game_home, &m.id, on)
}
pub fn config(profile: &Profile, m: &Definition) -> ZoomConfig {
    amod::config(&profile.game_home, m)
}
pub fn change_option(profile: &Profile, m: &Definition, index: usize) -> Result<(), String> {
    let mut c = config(profile, m);
    match index {
        0 => {
            c.initial_factor = if c.initial_factor >= amod::MAX_FACTOR {
                2.
            } else {
                (c.initial_factor * 2.).min(amod::MAX_FACTOR)
            }
        }
        1 => c.smooth = !c.smooth,
        2 => c.scale_sensitivity = !c.scale_sensitivity,
        _ => return Err(tr("Unknown mod setting.")),
    }
    amod::save_config(&profile.game_home, &m.id, &c)
}
pub fn installed_message(m: &Definition) -> String {
    trf(
        "Installed {name}. Start Minecraft to use it.",
        &[("name", &m.name)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installation_is_per_instance_and_keeps_worlds_and_other_preferences() {
        let root = std::env::temp_dir().join(format!(
            "wisp-amod-launcher-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("one/saves/World")).unwrap();
        std::fs::write(root.join("one/saves/World/level.dat"), b"world").unwrap();
        std::fs::write(
            root.join("one/wisp-mods.txt"),
            "mod.other:true\nkey.zoom.zoom:key.keyboard.c\n",
        )
        .unwrap();
        let profile = Profile {
            name: "One".into(),
            binary: root.join("game.exe"),
            resources: root.join("resources"),
            game_home: root.join("one"),
            args: vec![],
            use_account: false,
        };
        install_zoom(&profile).unwrap();
        let list = list(&profile);
        assert_eq!(list.len(), 1);
        let m = list[0].definition.as_ref().unwrap();
        assert!(list[0].enabled);
        set_enabled(&profile, m, false).unwrap();
        install_zoom(&profile).unwrap();
        assert!(!amod::enabled(&profile.game_home, "zoom"));
        change_option(&profile, m, 1).unwrap();
        assert!(!config(&profile, m).smooth);
        assert_eq!(
            std::fs::read(root.join("one/saves/World/level.dat")).unwrap(),
            b"world"
        );
        assert!(!root.join("two").exists());
        let text = std::fs::read_to_string(root.join("one/wisp-mods.txt")).unwrap();
        assert!(text.contains("mod.other:true") && text.contains("key.zoom.zoom:key.keyboard.c"));
        let bad = root.join("bad.amod");
        std::fs::write(&bad, b"invalid").unwrap();
        assert!(install(&profile, &bad).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
