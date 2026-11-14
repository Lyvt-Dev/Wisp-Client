//! Wisp client module packages, API 1. Modules run in the Rust engine without a rebuild.
//! A package declares its module and camera capability; it cannot execute arbitrary code.
use serde::{Deserialize, Serialize};
use std::{
    cell::Cell,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

pub const MANIFEST: &str = "wisp-client-mod.toml";
pub const DIR: &str = "wisp-mods";
pub const ZOOM_SOURCE: &str = include_str!("../../../mods/wisp-mods/zoom/wisp-client-mod.toml");
/// Proximity voice chat compatible with Simple Voice Chat servers (`wisp-voicechat`).
pub const VOICECHAT_SOURCE: &str =
    include_str!("../../../mods/wisp-mods/voicechat/wisp-client-mod.toml");
/// Lifts the render distance cap to `i32::MAX` chunks.
pub const MAX_RENDER_SOURCE: &str =
    include_str!("../../../mods/wisp-mods/max_render/wisp-client-mod.toml");
const MAX_PACKAGE: u64 = 1024 * 1024;
const MAX_MANIFEST: u64 = 64 * 1024;
/// A floating-point precision guard, rather than the previous gameplay limit of 10x.
pub const MAX_FACTOR: f64 = 1_000_000_000_000.0;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub format: String,
    pub api: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub module: String,
    pub capabilities: Vec<String>,
    /// The zoom module's settings (absent from other modules).
    #[serde(default)]
    pub zoom: ZoomConfig,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ZoomConfig {
    pub key: String,
    pub initial_factor: f64,
    pub scroll_step: f64,
    pub smooth: bool,
    pub scale_sensitivity: bool,
}
impl Default for ZoomConfig {
    fn default() -> Self {
        Self {
            key: "key.keyboard.z".into(),
            initial_factor: 4.,
            scroll_step: 1.25,
            smooth: true,
            scale_sensitivity: true,
        }
    }
}
impl ZoomConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !self.key.starts_with("key.keyboard.") && !self.key.starts_with("key.mouse.") {
            return Err("Zoom needs a keyboard or mouse binding".into());
        }
        if !self.initial_factor.is_finite() || !(1.0..=MAX_FACTOR).contains(&self.initial_factor) {
            return Err("Invalid starting zoom factor".into());
        }
        if !self.scroll_step.is_finite() || !(1.01..=4.).contains(&self.scroll_step) {
            return Err("Invalid zoom scroll step".into());
        }
        Ok(())
    }
}
impl Definition {
    pub fn parse(text: &str) -> Result<Self, String> {
        let m: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        if m.format != "wisp-client-mod" || m.api != 1 {
            return Err("Unsupported Wisp client mod API".into());
        }
        if m.id.is_empty()
            || m.id.len() > 64
            || !m
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err("Invalid mod ID".into());
        }
        if m.name.trim().is_empty()
            || m.name.len() > 128
            || m.version.trim().is_empty()
            || m.version.len() > 64
            || m.description.len() > 2048
        {
            return Err("Invalid mod metadata".into());
        }
        match (m.module.as_str(), m.capabilities.as_slice()) {
            ("zoom", [c]) if c == "camera-zoom" => m.zoom.validate()?,
            ("voicechat", [c]) if c == "voice-chat" => {}
            ("max_render", [c]) if c == "render-distance" => {}
            _ => {
                return Err(
                    "This Wisp build supports the camera-zoom, voice-chat and render-distance client modules"
                        .into(),
                );
            }
        }
        Ok(m)
    }
    /// The zoom module (its settings apply only to it).
    pub fn is_zoom(&self) -> bool {
        self.module == "zoom"
    }
    pub fn is_voicechat(&self) -> bool {
        self.module == "voicechat"
    }
    pub fn is_max_render(&self) -> bool {
        self.module == "max_render"
    }
    pub fn read(path: &Path) -> Result<Self, String> {
        if path.is_dir() {
            return Self::parse(
                &fs::read_to_string(path.join(MANIFEST)).map_err(|e| e.to_string())?,
            );
        }
        if fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_PACKAGE {
            return Err("Client mod package is too large".into());
        }
        let mut zip = zip::ZipArchive::new(fs::File::open(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if zip.len() != 1 {
            return Err("Client packages may contain only their manifest".into());
        }
        let mut entry = zip
            .by_name(MANIFEST)
            .map_err(|_| "Client mod manifest missing")?;
        if entry.size() > MAX_MANIFEST {
            return Err("Client mod manifest is too large".into());
        }
        let mut text = String::new();
        entry
            .by_ref()
            .take(MAX_MANIFEST + 1)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        if text.len() as u64 > MAX_MANIFEST {
            return Err("Client mod manifest is too large".into());
        }
        Self::parse(&text)
    }
}
pub fn is_client(path: &Path) -> bool {
    if path.is_dir() {
        return path.join(MANIFEST).is_file();
    }
    let Ok(file) = fs::File::open(path) else {
        return false;
    };
    let Ok(mut zip) = zip::ZipArchive::new(file) else {
        return false;
    };
    zip.by_name(MANIFEST).is_ok()
}
pub fn package(text: &str) -> Result<Vec<u8>, String> {
    Definition::parse(text)?;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file(MANIFEST, zip::write::SimpleFileOptions::default())
        .map_err(|e| e.to_string())?;
    zip.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(zip.finish().map_err(|e| e.to_string())?.into_inner())
}
pub fn find(game: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(game.join(DIR))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("amod"))
                && is_client(p)
        })
        .collect();
    paths.sort();
    paths
}
/// Whether the game folder has a Max Render module installed and not switched off.
pub fn max_render_on(game: &Path) -> bool {
    find(game)
        .iter()
        .any(|p| Definition::read(p).is_ok_and(|m| m.is_max_render() && enabled(game, &m.id)))
}
/// Client modules are installed by explicit user choice, and may be switched off afterwards.
pub fn enabled(game: &Path, id: &str) -> bool {
    let text = fs::read_to_string(game.join("wisp-mods.txt")).unwrap_or_default();
    let key = format!("mod.{id}");
    !text
        .lines()
        .filter_map(|l| l.split_once(':'))
        .any(|(k, v)| k.trim() == key && v.trim() == "false")
}
pub fn set_enabled(game: &Path, id: &str, on: bool) -> Result<(), String> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err("Invalid mod ID".into());
    }
    let file = game.join("wisp-mods.txt");
    let key = format!("mod.{id}");
    let mut text = match fs::read_to_string(&file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.to_string()),
    }
    .lines()
    .filter(|l| l.split_once(':').is_none_or(|(k, _)| k.trim() != key))
    .collect::<Vec<_>>()
    .join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(&format!("{key}:{on}\n"));
    atomic_write(&file, text.as_bytes())
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("No parent directory")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let tmp = path.with_extension(format!("tmp-{}-{nonce}", std::process::id()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map_err(|e: std::io::Error| e.to_string())
}
pub fn config_path(game: &Path, id: &str) -> PathBuf {
    game.join(DIR)
        .join(".wisp-mod-data")
        .join(id)
        .join("zoom.json")
}
pub fn config(game: &Path, m: &Definition) -> ZoomConfig {
    fs::read(config_path(game, &m.id))
        .ok()
        .and_then(|b| serde_json::from_slice::<ZoomConfig>(&b).ok())
        .filter(|c| c.validate().is_ok())
        .unwrap_or_else(|| m.zoom.clone())
}
pub fn save_config(game: &Path, id: &str, c: &ZoomConfig) -> Result<(), String> {
    c.validate()?;
    atomic_write(
        &config_path(game, id),
        &serde_json::to_vec_pretty(c).map_err(|e| e.to_string())?,
    )
}

/// Magnification is stored logarithmically so wheel input cannot overflow the projection.
#[derive(Debug)]
pub struct ZoomState {
    pub config: ZoomConfig,
    held: Cell<bool>,
    target_log: f64,
    current_log: Cell<f64>,
    sampled: Cell<Option<Instant>>,
}
impl ZoomState {
    pub fn new(config: ZoomConfig) -> Self {
        Self {
            target_log: config.initial_factor.ln(),
            config,
            held: Cell::new(false),
            current_log: Cell::new(0.),
            sampled: Cell::new(None),
        }
    }
    pub fn key(&mut self, key: &str, pressed: bool, playing: bool) {
        if key == self.config.key {
            if pressed && !self.held.get() {
                self.target_log = self.config.initial_factor.ln();
            }
            self.held.set(pressed && playing);
        }
    }
    pub fn release(&mut self) {
        self.held.set(false);
        self.current_log.set(0.);
        self.sampled.set(None);
    }
    pub fn scroll(&mut self, steps: f32) -> bool {
        if !self.held.get() || !steps.is_finite() || steps == 0. {
            return false;
        }
        self.target_log = (self.target_log + f64::from(steps) * self.config.scroll_step.ln())
            .clamp(0., MAX_FACTOR.ln());
        true
    }
    pub fn step(&self, playing: bool, elapsed: f64) -> f64 {
        if !playing {
            self.held.set(false);
            self.current_log.set(0.);
            return 1.;
        }
        let target = if self.held.get() { self.target_log } else { 0. };
        let value = if self.config.smooth {
            target + (self.current_log.get() - target) * (-elapsed.clamp(0., 1.) / 0.06).exp()
        } else {
            target
        };
        self.current_log
            .set(if value.abs() < 1e-7 { 0. } else { value });
        self.current_log.get().exp()
    }
    pub fn factor(&self, playing: bool) -> f64 {
        let now = Instant::now();
        let dt = self.sampled.replace(Some(now)).map_or(1. / 60., |old| {
            now.saturating_duration_since(old).as_secs_f64()
        });
        self.step(playing, dt)
    }
    pub fn fov(degrees: f32, factor: f64) -> f32 {
        if factor <= 1. {
            degrees
        } else {
            (f64::from(degrees) / factor).clamp(1e-10, 179.) as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scratch() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "wisp-amod-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn real_package_round_trip_and_capabilities_are_checked() {
        let p = scratch();
        let file = p.join("Zoom.amod");
        fs::write(&file, package(ZOOM_SOURCE).unwrap()).unwrap();
        assert!(is_client(&file));
        let m = Definition::read(&file).unwrap();
        assert_eq!(m.id, "zoom");
        assert!(Definition::parse(&ZOOM_SOURCE.replace("camera-zoom", "network")).is_err());
        let voice = Definition::parse(VOICECHAT_SOURCE).unwrap();
        assert!(voice.is_voicechat() && !voice.is_zoom());
        assert!(Definition::parse(&VOICECHAT_SOURCE.replace("voice-chat", "camera-zoom")).is_err());
        let max = Definition::parse(MAX_RENDER_SOURCE).unwrap();
        assert!(max.is_max_render() && !max.is_zoom());
        assert!(
            Definition::parse(&MAX_RENDER_SOURCE.replace("render-distance", "voice-chat")).is_err()
        );
        let installed = p.join("game");
        fs::create_dir_all(installed.join(DIR)).unwrap();
        assert!(!max_render_on(&installed));
        fs::write(
            installed.join(DIR).join("max_render.amod"),
            package(MAX_RENDER_SOURCE).unwrap(),
        )
        .unwrap();
        assert!(max_render_on(&installed));
        set_enabled(&installed, "max_render", false).unwrap();
        assert!(!max_render_on(&installed));
        assert!(Definition::parse(&ZOOM_SOURCE.replace("api = 1", "api = 2")).is_err());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn zoom_goes_beyond_ten_and_releases_without_changing_hotbar() {
        let mut z = ZoomState::new(ZoomConfig {
            smooth: false,
            ..Default::default()
        });
        assert!(!z.scroll(10.));
        z.key("key.keyboard.z", true, true);
        assert!(z.scroll(25.));
        let factor = z.step(true, 0.1);
        assert!(factor > 1000.);
        assert!(ZoomState::fov(70., factor) < 0.07);
        assert!(z.scroll(1000000.));
        let f = ZoomState::fov(70., z.step(true, 0.1));
        assert!(f.is_finite() && f > 0.);
        z.release();
        assert_eq!(z.step(true, 1.), 1.);
        assert!(!z.scroll(1.));
    }
    #[test]
    fn smooth_zoom_recovers_after_focus_loss_and_rejects_invalid_values() {
        let mut z = ZoomState::new(ZoomConfig::default());
        z.key("key.keyboard.z", true, true);
        let first = z.step(true, 0.016);
        assert!(first > 1. && first < 4.);
        assert!(!z.scroll(f32::NAN));
        z.release();
        assert_eq!(z.step(false, 1.), 1.);
        assert!(
            ZoomConfig {
                initial_factor: f64::INFINITY,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
    #[test]
    fn enable_switch_preserves_other_mods_and_keybindings() {
        let p = scratch();
        fs::write(
            p.join("wisp-mods.txt"),
            "mod.other:true\nkey.zoom.zoom:key.keyboard.c\nmod.zoom:false\n",
        )
        .unwrap();
        set_enabled(&p, "zoom", true).unwrap();
        let t = fs::read_to_string(p.join("wisp-mods.txt")).unwrap();
        assert!(t.contains("mod.other:true") && t.contains("key.zoom.zoom:key.keyboard.c"));
        assert!(enabled(&p, "zoom"));
        set_enabled(&p, "zoom", false).unwrap();
        assert!(!enabled(&p, "zoom"));
        fs::remove_dir_all(p).unwrap();
    }
}
