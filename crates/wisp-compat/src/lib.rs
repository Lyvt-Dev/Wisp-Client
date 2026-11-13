//! Wisp's environment variables and the launcher's data folder, read the same way everywhere.
#![forbid(unsafe_code)]
use std::{
    env,
    ffi::{OsStr, OsString},
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

/// The launcher's folder under the platform's local data directory.
pub const LAUNCHER_DIR: &str = "wisp-local-launcher";

pub fn var_os(key: impl AsRef<OsStr>) -> Option<OsString> {
    env::var_os(key)
}
pub fn var(key: impl AsRef<OsStr>) -> Result<String, env::VarError> {
    env::var(key)
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
    fn the_launcher_home_is_its_folder_under_an_absolute_base() {
        let base = env::temp_dir().join(format!("wisp-compat-{}", std::process::id()));
        let home = launcher_home(&base).unwrap();
        assert_eq!(home.file_name(), Some(OsStr::new(LAUNCHER_DIR)));
        assert!(launcher_home(Path::new("relative")).is_err());
        let _ = fs::remove_dir_all(base);
    }
}
