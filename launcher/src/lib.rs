//! Local instances, the bundled Rust engine, and opt-in Microsoft device sign-in.
pub mod auth;
pub mod catalog;
pub mod client_features;
pub mod engine;
pub mod friends;
pub mod i18n;
pub mod installation;
pub mod local_content;
pub mod mods;
mod mojang;
pub mod prefs;
pub mod server_ping;
pub mod shaders;
pub mod shop;
mod sources;
pub mod updater;

use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub binary: PathBuf,
    pub resources: PathBuf,
    pub game_home: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub use_account: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    pub selected: Option<String>,
    pub profiles: Vec<Profile>,
}

/// Deliberately separate from the upstream launcher's accounts and consent files.
pub fn launcher_home() -> Result<PathBuf> {
    if let Some(path) = wisp_compat::var_os("WISP_LAUNCHER_HOME").filter(|p| !p.is_empty()) {
        return Ok(path.into());
    }
    Ok(wisp_compat::launcher_home(&dirs::data_local_dir().ok_or(
        "No local data directory; set WISP_LAUNCHER_HOME",
    )?)?)
}

impl Settings {
    pub fn load(home: &Path) -> Result<Self> {
        match fs::read(home.join("local-profiles.json")) {
            Ok(bytes) => {
                let settings: Self = serde_json::from_slice(&bytes)?;
                Ok(settings)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, home: &Path) -> Result<()> {
        fs::create_dir_all(home)?;
        let bytes = serde_json::to_vec_pretty(self)?;
        // Write beside the destination, then replace it, keeping a failed write
        // from truncating the player's existing profiles.
        let temporary = home.join(format!(
            ".local-profiles-{}-{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let result = (|| -> Result<()> {
            let mut file = File::options()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, home.join("local-profiles.json"))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result?;
        Ok(())
    }

    pub fn current(&self) -> Option<&Profile> {
        self.selected.as_ref().and_then(|name| self.find(name))
    }

    pub fn find(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.name == name)
    }

    pub fn add(&mut self, profile: Profile) -> Result<()> {
        if profile.name.trim().is_empty() {
            return Err("Profile name must not be empty".into());
        }
        if self.find(&profile.name).is_some() {
            return Err("Profile already exists".into());
        }
        self.selected = Some(profile.name.clone());
        self.profiles.push(profile);
        Ok(())
    }
}

impl Profile {
    /// Resolve paths on import, so launching never depends on the launcher's working directory.
    pub fn new(name: String, binary: &Path, resources: &Path, home: &Path) -> Result<Self> {
        let profile = Self {
            name,
            binary: fs::canonicalize(binary)?,
            resources: fs::canonicalize(resources)?,
            game_home: std::path::absolute(home)?,
            args: Vec::new(),
            use_account: false,
        };
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<()> {
        if !self.binary.is_absolute() || !self.binary.is_file() {
            return Err("Choose an existing local Wisp executable".into());
        }
        if !self.resources.is_absolute() || !self.resources.is_dir() {
            return Err("Choose an existing prepared resources folder".into());
        }
        if !self.game_home.is_absolute() {
            return Err("Game folder must be an absolute path".into());
        }
        if self.game_home.exists() && !self.game_home.is_dir() {
            return Err("Game folder is not a directory".into());
        }
        Ok(())
    }

    pub fn command(&self) -> Result<Command> {
        self.validate()?;
        let mut command = Command::new(&self.binary);
        // The game opens its own window; keep its console hidden on Windows.
        engine::hide_console(&mut command);
        command
            .current_dir(&self.game_home)
            .arg("--resources")
            .arg(&self.resources)
            .args(&self.args)
            .env("WISP_HOME", &self.game_home)
            .env("WISP_TELEMETRY_URL", "off")
            .env("WISP_RELAY_URL", "")
            .env("WISP_FRIENDS_PATH", "direct")
            .env("WISP_FRIENDS_UPNP", "0")
            .env("WISP_COSMETICS", "0")
            .env_remove("WISP_ACCOUNT_SESSIONS")
            .env_remove("WISP_LAUNCHER_EXE")
            .env("WISP_NO_DISCORD", "1");
        wisp_compat::clear_env(&mut command, "WISP_ACCOUNT_SESSIONS");
        wisp_compat::clear_env(&mut command, "WISP_LAUNCHER_EXE");
        for key in [
            "LIBGL_ALWAYS_SOFTWARE",
            "GALLIUM_DRIVER",
            "WEBKIT_DISABLE_DMABUF_RENDERER",
            "WEBKIT_DISABLE_COMPOSITING_MODE",
        ] {
            command.env_remove(key);
        }
        Ok(command)
    }

    pub fn launch(&self) -> Result<Child> {
        if self.use_account {
            return Err(i18n::tr("This instance uses a Microsoft account. Start it through Wisp's account-aware Play action.").into());
        }
        self.launch_with_credentials(None)
    }

    pub fn launch_with_credentials(
        &self,
        credentials: Option<&auth::GameCredentials>,
    ) -> Result<Child> {
        self.launch_dressed(credentials, None)
    }

    /// Launches with the shop's worn items (`shop::worn_dir`): only the signed-in player wears
    /// them; an offline instance's own player is the only one they apply to anyway.
    pub fn launch_dressed(
        &self,
        credentials: Option<&auth::GameCredentials>,
        worn: Option<&Path>,
    ) -> Result<Child> {
        if self.use_account && credentials.is_none() {
            return Err(i18n::tr(auth::NEEDS_OWNER).into());
        }
        if credentials.is_some()
            && self
                .args
                .iter()
                .any(|a| a == "--session-stdin" || a.starts_with("--session-file"))
        {
            return Err(i18n::tr(
                "Remove custom session arguments before using a Microsoft account.",
            )
            .into());
        }
        let mut command = self.command()?;
        // With a cosmetics server, the game shows what other Wisp players wear and publishes
        // what this player wears (`WISP_COSMETICS_LOCAL`) and the emotes they play.
        let service = shop::service();
        if worn.is_some() || service.is_some() {
            command.env("WISP_COSMETICS", "1");
        }
        if let Some(api) = &service {
            command.env("WISP_COSMETICS_API", api);
        }
        if let Some(dir) = worn {
            command.env("WISP_COSMETICS_LOCAL", dir);
            if let Some(credentials) = credentials {
                command.env("WISP_COSMETICS_LOCAL_PLAYER", credentials.uuid());
            }
        }
        fs::create_dir_all(&self.game_home)?;
        let logs = self.game_home.join("logs");
        fs::create_dir_all(&logs)?;
        if prefs::Prefs::load(&launcher_home().unwrap_or_default()).keep_logs {
            let _ = fs::rename(
                logs.join("local-launcher.log"),
                logs.join("local-launcher.prev.log"),
            );
        }
        let log = File::create(logs.join("local-launcher.log"))?;
        command.stdout(log.try_clone()?).stderr(log);
        if credentials.is_some() {
            command.arg("--session-stdin").stdin(Stdio::piped());
        } else {
            command.stdin(Stdio::null());
        }
        let mut child = command.spawn()?;
        if let Some(credentials) = credentials {
            let sent = child
                .stdin
                .take()
                .ok_or_else(|| i18n::tr("Could not open the game's account pipe."))
                .and_then(|input| credentials.write_to(input));
            if let Err(error) = sent {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
        Ok(child)
    }
}
