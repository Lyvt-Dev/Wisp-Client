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
