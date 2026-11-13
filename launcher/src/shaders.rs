//! Native Wisp shaders, installed and selected per instance.
use crate::{i18n::tr, Profile};
use std::{
    fs,
    path::{Path, PathBuf},
};
use wisp_mods::amod::atomic_write;
use wisp_shader_api::{check, Pack};

pub const AURORA_ID: &str = "wisp_aurora";
pub const AURORA_FILE: &str = "wisp_aurora.hshader";
pub const SOURCES: [(&str, &[u8]); 11] = [
    (
        "README.md",
        include_bytes!("../../shaders/wisp/aurora/README.md"),
    ),
    (
        "wisp-shaders.toml",
        include_bytes!("../../shaders/wisp/aurora/wisp-shaders.toml"),
    ),
    (
        "common.wgsl",
        include_bytes!("../../shaders/wisp/aurora/common.wgsl"),
    ),
    (
        "sky.wgsl",
        include_bytes!("../../shaders/wisp/aurora/sky.wgsl"),
    ),
    (
        "material.wgsl",
        include_bytes!("../../shaders/wisp/aurora/material.wgsl"),
    ),
    (
        "vertex.wgsl",
        include_bytes!("../../shaders/wisp/aurora/vertex.wgsl"),
    ),
    (
        "weather.wgsl",
        include_bytes!("../../shaders/wisp/aurora/weather.wgsl"),
    ),
    (
        "bloom.wgsl",
        include_bytes!("../../shaders/wisp/aurora/bloom.wgsl"),
    ),
    (
        "atmosphere.wgsl",
        include_bytes!("../../shaders/wisp/aurora/atmosphere.wgsl"),
    ),
    (
        "finish.wgsl",
        include_bytes!("../../shaders/wisp/aurora/finish.wgsl"),
    ),
    (
        "LICENSE",
        include_bytes!("../../shaders/wisp/aurora/LICENSE"),
    ),
];
pub fn aurora() -> Result<Pack, String> {
    Pack::from_files(
        SOURCES
            .iter()
            .map(|(name, bytes)| (name.to_string(), bytes.to_vec()))
            .collect(),
    )
}
fn validate(pack: &Pack) -> Result<(), String> {
    check(pack).map(|_| ()).map_err(|problems| {
        problems
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })
}
pub struct Installed {
    pub file: String,
    pub path: PathBuf,
    pub pack: Result<Pack, String>,
    pub active: bool,
}
pub fn selected(profile: &Profile) -> String {
    fs::read_to_string(profile.game_home.join("wisp-graphics.txt"))
        .unwrap_or_default()
        .lines()
        .rev()
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim() == "pickedShaderPack")
        .map(|(_, v)| v.trim().to_string())
        .unwrap_or_default()
}
pub fn list(profile: &Profile) -> Vec<Installed> {
    let chosen = selected(profile);
    let mut paths: Vec<_> = fs::read_dir(profile.game_home.join("shaderpacks"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("hshader"))
                || p.join(wisp_shader_api::MANIFEST).is_file()
        })
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let file = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let active = chosen == file;
            let pack = Pack::read(&path);
            Installed {
                file,
                path,
                pack,
                active,
            }
        })
        .collect()
}
pub fn install_aurora(profile: &Profile) -> Result<(), String> {
    let pack = aurora()?;
    install_pack(profile, &pack)?;
    set_active(profile, Some(AURORA_FILE))
}
pub fn update_aurora(profile: &Profile) -> Result<(), String> {
    install_pack(profile, &aurora()?).map(|_| ())
}
pub fn install(profile: &Profile, source: &Path) -> Result<String, String> {
    if !source
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("hshader"))
    {
        return Err(tr("Choose a native Wisp .hshader package."));
    }
    let pack = Pack::read(source)?;
    let file = install_pack(profile, &pack)?;
    set_active(profile, Some(&file))?;
    Ok(pack.manifest.name)
}
fn install_pack(profile: &Profile, pack: &Pack) -> Result<String, String> {
    validate(pack)?;
    let file = format!("{}.hshader", pack.manifest.id);
    atomic_write(
        &profile.game_home.join("shaderpacks").join(&file),
        &pack.package()?,
    )?;
    Ok(file)
}
fn plain_file(file: &str) -> bool {
    !file.is_empty()
        && !file.contains(['/', '\\', ':', '\r', '\n'])
        && matches!(
            Path::new(file).components().next(),
            Some(std::path::Component::Normal(_))
        )
        && Path::new(file).components().count() == 1
}
/// Replaces just one preference and keeps unknown keys and other graphics settings intact.
fn setting(path: &Path, key: &str, value: &str, separator: char) -> Result<(), String> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.to_string()),
    };
    let mut lines = text
        .lines()
        .filter(|l| l.split_once(separator).is_none_or(|(k, _)| k.trim() != key))
        .collect::<Vec<_>>()
        .join("\n");
    if !lines.is_empty() {
        lines.push('\n');
    }
    lines.push_str(&format!("{key}{separator}{value}\n"));
    atomic_write(path, lines.as_bytes())
}
pub fn set_active(profile: &Profile, file: Option<&str>) -> Result<(), String> {
    if let Some(file) = file {
        if !plain_file(file) {
            return Err(tr("Invalid shader file name."));
        }
        validate(&Pack::read(
            &profile.game_home.join("shaderpacks").join(file),
        )?)?;
    }
    setting(
        &profile.game_home.join("wisp-graphics.txt"),
        "pickedShaderPack",
        file.unwrap_or(""),
        ':',
    )
}
pub fn set_quality(profile: &Profile, file: &str, quality: u32) -> Result<(), String> {
    if !plain_file(file) || quality > 2 {
        return Err(tr("Invalid shader quality."));
    }
    let path = profile.game_home.join("shaderpacks").join(file);
    if Pack::read(&path)?.manifest.id != AURORA_ID {
        return Err(tr("Open this shader's options in Minecraft."));
    }
    let mut options = path.as_os_str().to_owned();
    options.push(".txt");
    let options = PathBuf::from(options);
    for (key, value) in [
        ("quality", quality),
        ("shadow_resolution", quality),
        ("water_reflections", u32::from(quality > 0)),
    ] {
        setting(&options, key, &value.to_string(), '=')?;
    }
    Ok(())
}
pub fn quality(profile: &Profile, file: &str) -> u32 {
    if !plain_file(file) {
        return 1;
    }
    let path = profile
        .game_home
        .join("shaderpacks")
        .join(format!("{file}.txt"));
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .rev()
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == "quality")
        .and_then(|(_, v)| v.trim().parse::<u32>().ok())
        .filter(|v| *v <= 2)
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aurora_runs_through_the_real_shader_checker_and_round_trips() {
        let pack = aurora().unwrap();
        validate(&pack).unwrap();
        assert_eq!(pack.manifest.id, AURORA_ID);
        assert_eq!(pack.manifest.post.len(), 3);
        let bytes = pack.package().unwrap();
        assert_eq!(bytes, pack.package().unwrap());
        assert!(pack.manifest.shadow.is_some() && pack.manifest.reflection.is_some());
    }
    #[test]
    fn selection_quality_and_import_preserve_other_instance_data() {
        let root = std::env::temp_dir().join(format!(
            "wisp-shader-install-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let profile = Profile {
            name: "Test".into(),
            binary: root.join("game"),
            resources: root.join("resources"),
            game_home: root.join("one"),
            args: vec![],
            use_account: false,
        };
        fs::create_dir_all(&profile.game_home).unwrap();
        fs::write(
            profile.game_home.join("wisp-graphics.txt"),
            "resolutionScale:75\ncustom:value\n",
        )
        .unwrap();
        install_aurora(&profile).unwrap();
        assert_eq!(selected(&profile), AURORA_FILE);
        assert!(list(&profile)[0].active);
        let options = profile
            .game_home
            .join("shaderpacks")
            .join(format!("{AURORA_FILE}.txt"));
        fs::write(&options, "exposure=1.3\nother=7\n").unwrap();
        set_quality(&profile, AURORA_FILE, 2).unwrap();
        assert_eq!(quality(&profile, AURORA_FILE), 2);
        let text = fs::read_to_string(&options).unwrap();
        assert!(text.contains("exposure=1.3") && text.contains("other=7"));
        set_active(&profile, None).unwrap();
        let text = fs::read_to_string(profile.game_home.join("wisp-graphics.txt")).unwrap();
        assert!(text.contains("resolutionScale:75") && text.contains("custom:value"));
        assert!(set_active(&profile, Some("../outside.hshader")).is_err());
        let broken = root.join("broken.hshader");
        fs::write(&broken, b"invalid").unwrap();
        assert!(install(&profile, &broken).is_err());
        assert!(!root.join("two").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
