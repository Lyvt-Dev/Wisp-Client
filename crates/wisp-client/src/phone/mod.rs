//! The phone app (`packaging/android`, `crates/wisp-android`): its setup screen, Microsoft
//! sign-in and the copy of the game files from the player's PC. All of it also builds on a PC,
//! where `--phone-setup` shows the same screen for testing.
pub mod msa;
pub mod setup;
pub mod text;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The phone's folders.
#[derive(Clone, Debug)]
pub struct Paths {
    /// Where the game files go (`resources/<version>` in the game's home).
    pub resources: PathBuf,
    /// The app's private folder: the saved sign-in.
    pub private: PathBuf,
}

/// What the platform does that the game can't by itself.
pub struct Platform {
    /// Opens a link in the phone's browser.
    pub open_url: fn(&str),
}

static PLATFORM: OnceLock<Platform> = OnceLock::new();

/// Set once by the Android entry point, before the game starts.
pub fn set_platform(platform: Platform) {
    let _ = PLATFORM.set(platform);
}

/// Running as the phone app.
pub fn on_phone() -> bool {
    PLATFORM.get().is_some()
}

/// Opens `url` in the browser: the phone's when the app set one, else the PC's.
pub fn open_url(url: &str) {
    match PLATFORM.get() {
        Some(platform) => (platform.open_url)(url),
        None => crate::game::open_link(url),
    }
}

/// The file in the private folder that keeps the PC launcher's Wisp server.
const WISP_SERVER_FILE: &str = "wisp-server.txt";

/// Keeps the Wisp server the PC's launcher uses (the files came from it), or forgets it when
/// that launcher has none.
pub fn save_wisp_server(private: &Path, server: Option<&str>) {
    let file = private.join(WISP_SERVER_FILE);
    let result = match server.map(str::trim).filter(|s| !s.is_empty()) {
        Some(server) => {
            std::fs::create_dir_all(private).and_then(|()| std::fs::write(&file, server))
        }
        None => match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
    };
    if let Err(e) = result {
        log::warn!("Wisp: couldn't keep the Wisp server address: {e}");
    }
}

/// The Wisp server saved by [`save_wisp_server`].
pub fn wisp_server(private: &Path) -> Option<String> {
    std::fs::read_to_string(private.join(WISP_SERVER_FILE))
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// Turns on the Store, cosmetics and friends for this run with the saved Wisp server, as the
/// PC launcher does for the game it starts (`WISP_COSMETICS`, `WISP_COSMETICS_API`). Call
/// before the game starts: some of them are read once.
pub fn use_wisp_server(paths: &Paths) {
    if let Some(server) = wisp_server(&paths.private) {
        wisp_compat::set_override("WISP_COSMETICS", "1");
        wisp_compat::set_override("WISP_COSMETICS_API", server);
    }
}

/// The saved sign-in as the game's session: the account the game plays as, and its renewal.
/// Called once the setup is done. False when nobody is signed in (the game plays offline).
pub fn start_session(paths: &Paths) -> Option<String> {
    let account = msa::Account::load(&paths.private)?;
    let renew_dir = paths.private.join("renew");
    // A token near its end is renewed before the game starts with it.
    let account = if account.needs_renewal() {
        match account.renew() {
            Ok(fresh) => {
                let _ = fresh.save(&paths.private);
                fresh
            }
            Err(e) => {
                log::warn!("Wisp: renewing the Minecraft session failed: {e}");
                account
            }
        }
    } else {
        account
    };
    match wisp_client::auth::AuthSession::parse(&account.session_json()) {
        Ok(session) => {
            msa::renew_in_background(paths.private.clone(), renew_dir);
            wisp_client::auth::set_current(session);
            wisp_client::auth::prepare_chat_keys(true);
            Some(account.name)
        }
        Err(e) => {
            log::warn!("Wisp: the saved sign-in is not usable: {e}");
            None
        }
    }
}

/// Vanilla options a phone holds (`options.txt`): fast graphics, no clouds, shadows or
/// smooth lighting, a short view and simulation distance, 60 frames at most, and no keyboard
/// tutorial (its toasts name W, A, S, D and Space). Mipmaps stay: they save memory bandwidth,
/// which a phone's GPU has least of.
const PHONE_OPTIONS: &[(&str, &str)] = &[
    ("tutorialStep", "none"),
    ("renderDistance", "6"),
    ("simulationDistance", "5"),
    ("graphicsPreset", "\"fast\""),
    ("renderClouds", "\"false\""),
    ("entityShadows", "false"),
    ("ao", "false"),
    ("biomeBlendRadius", "0"),
    ("particles", "2"),
    ("maxFps", "60"),
    ("cutoutLeaves", "false"),
    ("vignette", "false"),
    ("weatherRadius", "3"),
    ("entityDistanceScaling", "0.75"),
    ("chunkSectionFadeInTime", "0.0"),
    ("menuBackgroundBlurriness", "0"),
];

/// Wisp's graphics options for a phone (`wisp-graphics.txt`): the world drawn at half of the
/// screen's pixels (a phone's screen has more than its GPU can fill), culling on, battery mode
/// (60 frames in a world, half the workers: a phone that stays cool keeps its speed).
const PHONE_GRAPHICS: &[(&str, &str)] = &[
    ("resolutionScale", "50"),
    // Chunk meshes uploaded at most 2 MiB a frame: a phone stutters when a frame uploads many.
    ("uploadBudgetKiB", "2048"),
    ("entityCulling", "sync"),
    ("particleCulling", "async"),
    ("leafCulling", "solid_aggressive"),
    ("batteryMode", "true"),
    ("animatedTextures", "false"),
];

/// Bumped when the phone's defaults change: each install takes them once more.
const PHONE_DEFAULTS_VERSION: &str = "2";
const PHONE_DEFAULTS_MARKER: &str = "phone-defaults.txt";

/// Puts the phone's options into the game's home (`home`) once per [`PHONE_DEFAULTS_VERSION`]:
/// on the first start, and once after an update that changed them. Only these keys change;
/// the player's other options stay.
pub fn apply_phone_defaults(home: &Path) {
    let marker = home.join(PHONE_DEFAULTS_MARKER);
    if std::fs::read_to_string(&marker).is_ok_and(|v| v.trim() == PHONE_DEFAULTS_VERSION) {
        return;
    }
    let result = set_keys(&home.join("options.txt"), phone_options)
        .and_then(|()| {
            set_keys(&home.join(wisp_client::graphics::FILE), |text| {
                with_keys(text, PHONE_GRAPHICS)
            })
        })
        .and_then(|()| std::fs::write(&marker, PHONE_DEFAULTS_VERSION));
    if let Err(e) = result {
        log::warn!("Wisp: couldn't write the phone's options: {e}");
    }
}

/// `options.txt` with the phone's options. A file without a `version` (a new one) gets this
/// game's: without it, vanilla reads the file as one from before 1.13 and its upgrades
/// replace some of the phone's options (the graphics preset turns "custom").
fn phone_options(text: &str) -> String {
    let options = with_keys(text, PHONE_OPTIONS);
    if options.lines().any(|line| line.starts_with("version:")) {
        options
    } else {
        format!("version:{}\n{options}", wisp_core::version::DATA_VERSION)
    }
}

/// Rewrites a `key:value` file through `edit` (a missing file is made).
fn set_keys(file: &Path, edit: impl FnOnce(&str) -> String) -> std::io::Result<()> {
    let old = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    std::fs::write(file, edit(&old))
}

fn with_keys(text: &str, keys: &[(&str, &str)]) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let key = line.split_once(':').map_or(line, |(k, _)| k);
        if !keys.iter().any(|(k, _)| *k == key) {
            out.push_str(line);
            out.push('\n');
        }
    }
    for (key, value) in keys {
        out.push_str(key);
        out.push(':');
        out.push_str(value);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod defaults_tests {
    use super::*;

    #[test]
    fn phone_keys_replace_theirs_and_keep_the_rest() {
        let text = "version:4700\nrenderDistance:12\nlang:\"de_de\"\n";
        let out = with_keys(text, &[("renderDistance", "6"), ("maxFps", "60")]);
        assert_eq!(
            out,
            "version:4700\nlang:\"de_de\"\nrenderDistance:6\nmaxFps:60\n"
        );
    }

    #[test]
    fn the_phone_options_parse_as_vanilla_and_wisp_read_them() {
        let options = phone_options("");
        let graphics = wisp_client::graphics::Graphics::parse(&with_keys("", PHONE_GRAPHICS));
        assert_eq!(graphics.resolution_scale, 50);
        assert!(graphics.battery && !graphics.animated_textures);
        // Every key is one Wisp writes back: none is misspelt.
        let written = graphics.serialize();
        for (key, value) in PHONE_GRAPHICS {
            assert!(written.contains(&format!("{key}:{value}\n")), "{key}");
        }
        // Vanilla's options file reads each one as written (none falls back to its default).
        let parsed = wisp_client::options_file::OptionsFile::parse(&options);
        assert_eq!(parsed.name("graphicsPreset"), "fast");
        assert_eq!(parsed.name("renderClouds"), "false");
        assert_eq!(parsed.int("renderDistance"), 6);
        assert_eq!(parsed.int("maxFps"), 60);
        assert_eq!(parsed.int("particles"), 2);
        assert!(!parsed.bool("ao") && !parsed.bool("entityShadows"));
    }

    #[test]
    fn defaults_apply_once_per_version() {
        let dir = std::env::temp_dir().join(format!("wisp-phone-defaults-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("options.txt"), "renderDistance:12\n").unwrap();
        apply_phone_defaults(&dir);
        let options = std::fs::read_to_string(dir.join("options.txt")).unwrap();
        assert!(options.contains("renderDistance:6\n"));
        // The player's later change stays.
        std::fs::write(dir.join("options.txt"), "renderDistance:10\n").unwrap();
        apply_phone_defaults(&dir);
        let options = std::fs::read_to_string(dir.join("options.txt")).unwrap();
        assert_eq!(options, "renderDistance:10\n");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
