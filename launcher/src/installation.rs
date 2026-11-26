//! Wisp-style automatic setup around Wisp's bundled Rust client and preparer.
use crate::{auth, engine, i18n::tr, mojang, sources, Profile};
use serde_json::Value;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Installed {
    pub profile: Profile,
    pub account: auth::SignedIn,
}

/// Network game downloads require a fresh ownership check, as in the original Wisp launcher.
pub fn install(
    home: &Path,
    cached: Option<auth::SignedIn>,
    names: &[String],
    cancel: &AtomicBool,
    progress: &(dyn Fn(engine::Progress) + Sync),
) -> Result<Installed, String> {
    std::fs::create_dir_all(home.join("logs")).map_err(|e| e.to_string())?;
    let log = std::sync::Mutex::new(
        std::fs::File::create(home.join("logs/install.log")).map_err(|e| e.to_string())?,
    );
    let report = |step: engine::Progress| {
        if let Ok(mut log) = log.lock() {
            let _ = writeln!(log, "{}", step.message);
        }
        progress(step);
    };
    install_inner(home, cached, names, cancel, &report).inspect_err(|message| {
        report(engine::Progress {
            message: message.clone(),
            fraction: None,
        });
    })
}

fn install_inner(
    home: &Path,
    cached: Option<auth::SignedIn>,
    names: &[String],
    cancel: &AtomicBool,
    progress: &(dyn Fn(engine::Progress) + Sync),
) -> Result<Installed, String> {
    cancelled(cancel)?;
    let account = auth::for_game(home, cached, &|phase| {
        progress(engine::Progress {
            message: phase.label(),
            fraction: None,
        });
    })?;
    cancelled(cancel)?;
    if engine::bundled().is_none() {
        return Err(tr(
            "The bundled Rust game is missing. Reinstall Wisp using its full setup.",
        ));
    }
    let resources = home.join("resources").join(engine::version());
    if engine::check_resources(&resources).is_err() {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(15))
            .timeout_read(std::time::Duration::from_secs(30))
            .user_agent(concat!("Wisp/", env!("CARGO_PKG_VERSION")))
            .build();
        let fetch = |url: &str| fetch(&agent, url, cancel);
        let log = |message: String| {
            let fraction = mojang::progress().map(|(done, total)| done as f32 / total as f32);
            progress(engine::Progress {
                message: tr(&message),
                fraction,
            });
        };
        let lock: Value = serde_json::from_str(include_str!("../../research-lock.json"))
            .map_err(|_| tr("Wisp's bundled version information could not be read."))?;
        let inputs = provision(&lock, home, &fetch, cancel, &log)?;
        cancelled(cancel)?;
        engine::prepare_with_java(
            home,
            &inputs.minecraft,
            &inputs.server,
            Some(&inputs.java),
            cancel,
            progress,
        )?;
    }
    cancelled(cancel)?;
    let mut profile = engine::new_profile(home, &resources, names)?;
    profile.use_account = true;
    Ok(Installed { profile, account })
}

fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err(tr(
            "Game installation cancelled. Run setup again to resume.",
        ))
    } else {
        Ok(())
    }
}

fn fetch(agent: &ureq::Agent, url: &str, cancel: &AtomicBool) -> Result<Vec<u8>, String> {
    cancelled(cancel)?;
    if !url.starts_with("https://") {
        return Err(tr("The game download must use HTTPS."));
    }
    let response = agent.get(url).call().map_err(|_| {
        tr("A game download failed. Check your connection and run setup again to resume.")
    })?;
    let mut reader = response.into_reader();
    let mut data = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        cancelled(cancel)?;
        let count = reader
            .read(&mut chunk)
            .map_err(|_| tr("A game download was interrupted. Run setup again to resume."))?;
        if count == 0 {
            return Ok(data);
        }
        if data.len() + count > 512 * 1024 * 1024 {
            return Err(tr("The game download is larger than expected."));
        }
        data.extend_from_slice(&chunk[..count]);
    }
}

struct Inputs {
    minecraft: PathBuf,
    server: PathBuf,
    java: PathBuf,
}

fn provision(
    lock: &Value,
    home: &Path,
    fetch: sources::Fetch,
    cancel: &AtomicBool,
    log: &(dyn Fn(String) + Sync),
) -> Result<Inputs, String> {
    let pins = sources::Pins::read(lock)
        .ok_or_else(|| tr("The bundled Minecraft version pins are incomplete."))?;
    let minecraft = home.join("minecraft");
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    };
    mojang::download(&pins, &minecraft, os, 8, fetch, cancel, log)?;
    cancelled(cancel)?;
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(
            minecraft
                .join("versions")
                .join(&pins.version)
                .join(format!("{}.json", pins.version)),
        )
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let server = minecraft
        .join("server-jars")
        .join(format!("{}.jar", pins.version));
    let expected = lock["minecraft"]["server_sha1"]
        .as_str()
        .ok_or_else(|| tr("The bundled version has no server checksum."))?;
    if manifest["downloads"]["server"]["sha1"].as_str() != Some(expected) {
        return Err(tr(
            "The server download does not match the bundled Rust engine.",
        ));
    }
    if sources::sha1_file(&server).as_deref() != Some(expected) {
        log(tr("Downloading the matching Minecraft server files…"));
        let url = manifest["downloads"]["server"]["url"]
            .as_str()
            .ok_or_else(|| tr("The game version does not list server files."))?;
        let body = sources::fetch_checked(fetch, url, expected)?;
        sources::write_file(&server, &body)?;
    }
    cancelled(cancel)?;
    log(tr("Getting Minecraft's preparation runtime…"));
    let component = manifest["javaVersion"]["component"]
        .as_str()
        .ok_or_else(|| tr("The game version does not list its preparation runtime."))?;
    mojang::relative(component)?;
    let runtime =
        mojang::java_runtime(component, &minecraft.join("runtime"), 8, fetch, cancel, log)?;
    let mac_home = runtime.join("jre.bundle/Contents/Home");
    let java = if mac_home.is_dir() { mac_home } else { runtime };
    let compiler = java
        .join("bin")
        .join(if cfg!(windows) { "javac.exe" } else { "javac" });
    if !compiler.is_file() {
        return Err(tr(
            "Minecraft's preparation runtime is incomplete. Retry game setup.",
        ));
    }
    Ok(Inputs {
        minecraft,
        server,
        java,
    })
}

#[cfg(test)]
mod tests;
