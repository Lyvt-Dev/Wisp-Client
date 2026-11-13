// Copyright (c) 2026 Ludovic Marie and the Horizon contributors. MIT; see LICENSE.
// Adapted from Wisp 2181ab22: only the verified download helpers used by Wisp.
//! Wisp's pinned version metadata, checksums and atomic download writes.

use serde_json::Value;
use sha1::{Digest, Sha1};
use std::fs;
use std::io::Read;
use std::path::Path;

pub type Fetch<'a> = &'a (dyn Fn(&str) -> Result<Vec<u8>, String> + Sync);

/// What a version's research-lock.json pins about the game files.
pub struct Pins {
    pub version: String,
    pub client_sha1: String,
    /// The version JSON's sha1, then the earlier ones Mojang republished.
    pub manifests: Vec<String>,
}

impl Pins {
    pub fn read(lock: &Value) -> Option<Pins> {
        let game = &lock["minecraft"];
        let text = |key: &str| game[key].as_str().map(String::from);
        let mut manifests = vec![text("manifest_sha1")?];
        if let Some(list) = game["accepted_manifest_sha1"].as_array() {
            manifests.extend(list.iter().filter_map(Value::as_str).map(String::from));
        }
        Some(Pins {
            version: text("version")?,
            client_sha1: text("client_sha1")?,
            manifests,
        })
    }

    /// Any version from Mojang's version list, by its id and its version file's sha1 (the
    /// list's): the client's sha1 then comes from that checked version file.
    #[cfg(test)]
    pub fn for_version(id: &str, manifest_sha1: &str) -> Pins {
        Pins {
            version: id.to_string(),
            client_sha1: String::new(),
            manifests: vec![manifest_sha1.to_string()],
        }
    }

    fn manifest_accepted(&self, path: &Path) -> bool {
        sha1_file(path).is_some_and(|sha1| self.manifests.contains(&sha1))
    }
}

pub(crate) fn sha1_file(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut hash = Sha1::new();
    let mut buffer = vec![0; 1 << 20];
    loop {
        match file.read(&mut buffer).ok()? {
            0 => break,
            n => hash.update(&buffer[..n]),
        }
    }
    Some(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

pub(crate) fn fetch_manifest(
    pins: &Pins,
    path: &Path,
    fetch: Fetch,
    log: &dyn Fn(String),
) -> Result<(), String> {
    if pins.manifest_accepted(path) {
        return Ok(());
    }
    let (v, sha1) = (&pins.version, &pins.manifests[0]);
    log(format!("Downloading the {v} version file from Mojang"));
    let url = format!("https://piston-meta.mojang.com/v1/packages/{sha1}/{v}.json");
    let body =
        fetch_checked(fetch, &url, sha1).map_err(|e| format!("Mojang's {v} version file {e}"))?;
    write_file(path, &body)
}

pub(crate) fn sha1_hex(data: &[u8]) -> String {
    Sha1::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A download checked against its sha1; the error says how it differs.
pub(crate) fn fetch_checked(fetch: Fetch, url: &str, sha1: &str) -> Result<Vec<u8>, String> {
    let body = fetch(url)?;
    match sha1_hex(&body) {
        actual if actual == sha1 => Ok(body),
        actual => Err(format!("is not the one Wisp pins (sha1 {actual})")),
    }
}

/// Writes a file whole: a partial one never takes its place.
pub(crate) fn write_file(path: &Path, data: &[u8]) -> Result<(), String> {
    let error = |e: std::io::Error| format!("{}: {e}", path.display());
    fs::create_dir_all(path.parent().unwrap()).map_err(error)?;
    let partial = path.with_file_name(format!(
        "{}.partial",
        path.file_name().unwrap().to_string_lossy()
    ));
    fs::write(&partial, data)
        .and_then(|_| fs::rename(&partial, path))
        .map_err(error)
}
