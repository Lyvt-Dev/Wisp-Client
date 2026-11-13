//! Automatic updates: the launcher looks for a newer `launcher-v<version>` release of Wisp Client
//! on GitHub, downloads its Windows installer beside the launcher's data, checks it against the
//! release's `.sha256`, and runs it silently once no game is running. The installer closes the
//! launcher, installs over it and starts the new one.
//!
//! Off with Settings' "Update automatically" (`Prefs::auto_update`) or `WISP_LAUNCHER_NO_SELF_UPDATE`
//! (tests and agents), and in development builds.
use semver::Version;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Where the releases are.
pub const REPOSITORY: &str = "Lyvt-Dev/Wisp-Client";
/// The installer's largest acceptable size.
const MAX_INSTALLER: u64 = 512 << 20;

/// A release newer than this launcher, and its installer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    pub version: Version,
    /// The installer's file name and where to fetch it and its checksum.
    pub name: String,
    pub installer_url: String,
    pub checksum_url: String,
}

/// This launcher's version.
pub fn current() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("the package version is semver")
}

/// Whether updates may run: not turned off, and a release build.
pub fn enabled(auto_update: bool) -> bool {
    auto_update
        && !cfg!(debug_assertions)
        && wisp_compat::var_os("WISP_LAUNCHER_NO_SELF_UPDATE").is_none()
}

/// The newest published (not draft, not prerelease) `launcher-v*` release in GitHub's release
/// list `releases` that is newer than `current` and has a Windows installer and its checksum.
pub fn newest(releases: &Value, current: &Version) -> Option<Update> {
    releases
        .as_array()?
        .iter()
        .filter(|r| {
            !r["draft"].as_bool().unwrap_or(true) && !r["prerelease"].as_bool().unwrap_or(true)
        })
        .filter_map(|r| {
            let version =
                Version::parse(r["tag_name"].as_str()?.strip_prefix("launcher-v")?).ok()?;
            let name = format!("Wisp-Client-{version}-Setup-x64.exe");
            let asset = |file: &str| {
                r["assets"].as_array()?.iter().find_map(|a| {
                    (a["name"].as_str()? == file)
                        .then(|| a["browser_download_url"].as_str().map(str::to_owned))
                        .flatten()
                })
            };
            Some(Update {
                installer_url: asset(&name)?,
                checksum_url: asset(&format!("{name}.sha256"))?,
                name,
                version,
            })
        })
        .filter(|u| u.version > *current)
        .max_by(|a, b| a.version.cmp(&b.version))
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(60))
        .user_agent(concat!("wisp-client-launcher/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// Asks GitHub for a newer release.
pub fn check() -> Result<Option<Update>, String> {
    let url = format!("https://api.github.com/repos/{REPOSITORY}/releases?per_page=30");
    let releases: Value = agent()
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;
    Ok(newest(&releases, &current()))
}

/// The SHA-256 a `.sha256` sidecar names for `name` (`<hex>  <name>`).
pub fn expected_digest(sidecar: &str, name: &str) -> Option<String> {
    let mut parts = sidecar.split_whitespace();
    let hex = parts.next()?.to_ascii_lowercase();
    let file = parts.next()?.trim_start_matches('*');
    (file == name && hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then_some(hex)
}

/// Downloads `update`'s installer into `dir` (kept there once checked) and returns its path; one
/// already downloaded and checked is reused.
pub fn download(update: &Update, dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let sidecar = agent()
        .get(&update.checksum_url)
        .call()
        .map_err(|e| e.to_string())?
        .into_string()
        .map_err(|e| e.to_string())?;
    let expected = expected_digest(&sidecar, &update.name)
        .ok_or("The release's checksum file is malformed.")?;
    let path = dir.join(&update.name);
    if let Ok(bytes) = std::fs::read(&path) {
        if hex(&Sha256::digest(&bytes)) == expected {
            return Ok(path);
        }
    }
    let mut body = Vec::new();
    agent()
        .get(&update.installer_url)
        .call()
        .map_err(|e| e.to_string())?
        .into_reader()
        .take(MAX_INSTALLER + 1)
        .read_to_end(&mut body)
        .map_err(|e| e.to_string())?;
    if body.len() as u64 > MAX_INSTALLER {
        return Err("The update is larger than expected.".into());
    }
    if hex(&Sha256::digest(&body)) != expected {
        return Err("The downloaded update doesn't match its checksum.".into());
    }
    let part = path.with_extension("exe.part");
    std::fs::write(&part, &body).map_err(|e| e.to_string())?;
    std::fs::rename(&part, &path).map_err(|e| e.to_string())?;
    // Older downloads are no longer needed.
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path() != path {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    Ok(path)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Where downloaded updates wait, in the launcher's data folder.
pub fn updates_dir(home: &Path) -> PathBuf {
    home.join("updates")
}

/// Whether one of `programs` is running: Windows won't open a running program's file for writing.
/// The installer closes whatever uses its files, so it must wait for a game started by an earlier
/// launcher too.
pub fn in_use(programs: &[&Path]) -> bool {
    programs
        .iter()
        .any(|p| p.is_file() && std::fs::OpenOptions::new().append(true).open(p).is_err())
}

/// Starts the installer at `path` silently; it closes this launcher, installs and starts the new
/// one. The caller quits right after.
pub fn install(path: &Path) -> Result<(), String> {
    if !cfg!(windows) {
        return Err("Automatic updates install on Windows only.".into());
    }
    std::process::Command::new(path)
        .args([
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/CLOSEAPPLICATIONS",
        ])
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn release(tag: &str, draft: bool, prerelease: bool, with_files: bool) -> Value {
        let version = tag.trim_start_matches("launcher-v");
        let name = format!("Wisp-Client-{version}-Setup-x64.exe");
        let assets = if with_files {
            json!([
                {"name": name, "browser_download_url": format!("https://x/{name}")},
                {"name": format!("{name}.sha256"), "browser_download_url": format!("https://x/{name}.sha256")},
            ])
        } else {
            json!([])
        };
        json!({"tag_name": tag, "draft": draft, "prerelease": prerelease, "assets": assets})
    }

    #[test]
    fn the_newest_published_launcher_release_with_an_installer_wins() {
        let releases = json!([
            release("launcher-v0.14.1", false, false, true),
            release("launcher-v0.16.0", true, false, true),
            release("launcher-v0.15.0", false, false, true),
            release("launcher-v0.15.5", false, true, true),
            release("launcher-v0.15.2", false, false, false),
            release("nightly-2026-11-13", false, false, true),
        ]);
        let found = newest(&releases, &Version::new(0, 14, 0)).unwrap();
        assert_eq!(found.version, Version::new(0, 15, 0));
        assert_eq!(found.name, "Wisp-Client-0.15.0-Setup-x64.exe");
        assert!(found.checksum_url.ends_with(".exe.sha256"));
        // Nothing newer than what runs.
        assert!(newest(&releases, &Version::new(0, 15, 0)).is_none());
        assert!(newest(&json!({"message": "rate limited"}), &Version::new(0, 1, 0)).is_none());
    }

    #[test]
    fn the_checksum_sidecar_names_the_installer() {
        let hex = "a".repeat(64);
        let name = "Wisp-Client-0.15.0-Setup-x64.exe";
        assert_eq!(
            expected_digest(&format!("{hex}  {name}\n"), name),
            Some(hex.clone())
        );
        assert_eq!(expected_digest(&format!("{hex}  other.exe"), name), None);
        assert_eq!(expected_digest("short  x", name), None);
    }

    #[test]
    fn a_downloaded_installer_that_matches_is_reused() {
        let dir = std::env::temp_dir().join(format!("wisp-updates-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let name = "Wisp-Client-9.0.0-Setup-x64.exe";
        std::fs::write(dir.join(name), b"installer").unwrap();
        let digest = hex(&Sha256::digest(b"installer"));
        assert_eq!(
            expected_digest(&format!("{digest}  {name}"), name),
            Some(digest)
        );
        // A file nothing runs isn't in use, and a missing one neither.
        assert!(!in_use(&[&dir.join(name), &dir.join("missing.exe")]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
