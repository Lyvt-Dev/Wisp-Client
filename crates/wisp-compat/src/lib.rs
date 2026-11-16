//! Wisp's environment variables and the launcher's data folder, read the same way everywhere.
#![forbid(unsafe_code)]
use std::{
    env,
    ffi::{OsStr, OsString},
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
};

/// The launcher's folder under the platform's local data directory.
pub const LAUNCHER_DIR: &str = "wisp-local-launcher";

/// Values set in this process with [`set_override`], read before the real environment.
fn overrides() -> &'static Mutex<Vec<(OsString, OsString)>> {
    static OVERRIDES: OnceLock<Mutex<Vec<(OsString, OsString)>>> = OnceLock::new();
    OVERRIDES.get_or_init(Mutex::default)
}

/// Sets `key` for every later [`var`] and [`var_os`] in this process, without touching the real
/// environment (which isn't safe to change once threads run). Child processes don't see it.
pub fn set_override(key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) {
    let (key, value) = (key.as_ref().to_owned(), value.as_ref().to_owned());
    let mut overrides = overrides().lock().unwrap_or_else(|e| e.into_inner());
    overrides.retain(|(k, _)| *k != key);
    overrides.push((key, value));
}

pub fn var_os(key: impl AsRef<OsStr>) -> Option<OsString> {
    let key = key.as_ref();
    let overridden = overrides()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone());
    overridden.or_else(|| env::var_os(key))
}
pub fn var(key: impl AsRef<OsStr>) -> Result<String, env::VarError> {
    match var_os(key) {
        Some(value) => value.into_string().map_err(env::VarError::NotUnicode),
        None => Err(env::VarError::NotPresent),
    }
}
/// Keeps `key` from reaching a child process.
pub fn clear_env(command: &mut Command, key: &str) {
    command.env_remove(key);
}

/// The launcher's data folder under `base` (an absolute directory), created if missing.
pub fn launcher_home(base: &Path) -> io::Result<PathBuf> {
    if !base.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The launcher data directory must be absolute.",
        ));
    }
    fs::create_dir_all(base)?;
    Ok(fs::canonicalize(base)?.join(LAUNCHER_DIR))
}

/// The game's client preferences (`wisp-extras.txt`) in `dir`.
pub fn read_extras(dir: &Path) -> io::Result<String> {
    fs::read_to_string(dir.join("wisp-extras.txt"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_override_wins_over_the_environment() {
        let key = "WISP_COMPAT_OVERRIDE_TEST";
        assert!(var(key).is_err());
        set_override(key, "a");
        set_override(key, "b");
        assert_eq!(var(key).as_deref(), Ok("b"));
        assert_eq!(var_os(key), Some(OsString::from("b")));
    }

    #[test]
    fn the_launcher_home_is_its_folder_under_an_absolute_base() {
        let base = env::temp_dir().join(format!("wisp-compat-{}", std::process::id()));
        let home = launcher_home(&base).unwrap();
        assert_eq!(home.file_name(), Some(OsStr::new(LAUNCHER_DIR)));
        assert!(launcher_home(Path::new("relative")).is_err());
        let _ = fs::remove_dir_all(base);
    }
}
