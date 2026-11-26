//! The bundled Rust engine and preparation of the player's own pinned Minecraft resources.
use crate::{i18n::tr, Profile};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, OnceLock,
};
use std::time::Duration;

const PINS: &str = include_str!("../../research-lock.json");

pub fn version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(|| {
        serde_json::from_str::<serde_json::Value>(PINS).expect("embedded version pins")["minecraft"]
            ["version"]
            .as_str()
            .expect("embedded Minecraft version")
            .to_owned()
    })
}

#[derive(Clone, Debug)]
pub struct Bundle {
    pub binary: PathBuf,
    pub preparer: PathBuf,
    pub kit: PathBuf,
}

pub fn bundled() -> Option<Bundle> {
    let root = wisp_compat::var_os("WISP_ENGINE_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()?
                .parent()
                .map(|p| p.join("engine"))
        })?;
    let bundle = Bundle {
        binary: root.join(if cfg!(windows) {
            "wisp-client.exe"
        } else {
            "wisp-client"
        }),
        preparer: root.join(if cfg!(windows) {
            "wisp-prepare.exe"
        } else {
            "wisp-prepare"
        }),
        kit: root.join("kit"),
    };
    (bundle.binary.is_file()
        && bundle.preparer.is_file()
        && bundle.kit.join("research-lock.json").is_file())
    .then_some(bundle)
}

pub fn check_resources(resources: &Path) -> Result<(), String> {
    let bundle =
        bundled().ok_or_else(|| tr("The Rust engine is missing. Install the full Wisp setup."))?;
    let mut command = Command::new(bundle.preparer);
    hide_console(&mut command);
    let output = command
        .args(["--check", "--root"])
        .arg(bundle.kit)
        .arg("--output")
        .arg(resources)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    let details = String::from_utf8_lossy(&output.stdout)
        .lines()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    Err(format!(
        "{} {}",
        tr("These resources are not ready for the bundled Rust engine. Use Game setup."),
        details
    ))
}

pub fn new_profile(home: &Path, resources: &Path, names: &[String]) -> Result<Profile, String> {
    check_resources(resources)?;
    let bundle = bundled().ok_or_else(|| tr("The Rust engine is missing."))?;
    let mut nonce = [0u8; 8];
    getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
    let id: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
    let mut name = tr("Default");
    let mut number = 2;
    while names.contains(&name) {
        name = format!("{} {number}", tr("Instance"));
        number += 1;
    }
    Profile::new(
        name,
        &bundle.binary,
        resources,
        &home.join("instances").join(id),
    )
    .map_err(|e| e.to_string())
}

pub fn hide_console(command: &mut Command) {
    wisp_compat::clear_env(command, "WISP_ACCOUNT_SESSIONS");
    wisp_compat::clear_env(command, "WISP_LAUNCHER_EXE");
    command
        .env_remove("WISP_ACCOUNT_SESSIONS")
        .env_remove("WISP_LAUNCHER_EXE");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

#[derive(Clone, Debug)]
pub struct Progress {
    pub message: String,
    pub fraction: Option<f32>,
}

/// Preparation runs in an app-owned job on Windows, so cancellation also stops its Java workers.
pub fn prepare(
    home: &Path,
    minecraft: &Path,
    server: &Path,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<PathBuf, String> {
    prepare_with_java(home, minecraft, server, None, cancel, progress)
}

pub(crate) fn prepare_with_java(
    home: &Path,
    minecraft: &Path,
    server: &Path,
    java_home: Option<&Path>,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<PathBuf, String> {
    let bundle =
        bundled().ok_or_else(|| tr("The Rust engine is missing. Install the full Wisp setup."))?;
    let root = home.join("engine-workspace");
    copy_tree(&bundle.kit, &root)?;
    let resources = home.join("resources").join(version());
    std::fs::create_dir_all(home.join("logs")).map_err(|e| e.to_string())?;
    let log = std::fs::File::create(home.join("logs/prepare.log")).map_err(|e| e.to_string())?;
    let mut command = Command::new(bundle.preparer);
    hide_console(&mut command);
    if let Some(java_home) = java_home {
        command.arg("--java-home").arg(java_home);
    }
    let mut child = command
        .arg("--prepare")
        .arg("--complete")
        .args(["--progress", "json"])
        .arg("--root")
        .arg(&root)
        .arg("--output")
        .arg(&resources)
        .arg("--minecraft-dir")
        .arg(minecraft)
        .arg("--server-jar")
        .arg(server)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(log.try_clone().map_err(|e| e.to_string())?)
        .spawn()
        .map_err(|e| e.to_string())?;
    let job = match Job::new(&child) {
        Ok(job) => job,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| tr("Could not read setup progress."))?;
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut log = log;
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = writeln!(log, "{line}");
            let _ = sender.send(line);
        }
    });
    let mut last_error = None;
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            drop(job);
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(tr("Game setup cancelled."));
        }
        match receiver.recv_timeout(Duration::from_millis(150)) {
            Ok(line) => {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                    if let Some(error) = value["error"].as_str() {
                        last_error = Some(error.to_owned());
                    }
                    if let Some(message) = value["message"].as_str() {
                        progress(Progress {
                            message: message.to_owned(),
                            fraction: value["fraction"].as_f64().map(|x| x as f32),
                        });
                    } else if let Some(fraction) = value["fraction"].as_f64() {
                        progress(Progress {
                            message: tr("Preparing Minecraft resources…"),
                            fraction: Some(fraction as f32),
                        });
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => {}
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
    };
    let _ = reader.join();
    // A fast failure can exit while its final error line is still queued.
    for line in receiver.try_iter() {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
            if let Some(error) = value["error"].as_str() {
                last_error = Some(error.to_owned());
            }
        }
    }
    drop(job);
    if !status.success() {
        return Err(last_error.unwrap_or_else(|| tr("Game setup failed. Check the preparation log and confirm that Minecraft 26.3 and JDK 25 are installed.")));
    }
    check_resources(&resources)?;
    Ok(resources)
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), String> {
    if std::fs::symlink_metadata(target).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(tr("The game setup directory must not be a symbolic link."));
    }
    std::fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let destination = target.join(entry.file_name());
        if kind.is_symlink()
            || std::fs::symlink_metadata(&destination).is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err(tr("A symbolic link was found in the game setup files."));
        }
        if kind.is_dir() {
            copy_tree(&entry.path(), &destination)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), destination).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(windows)]
struct Job(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Job {
    fn new(child: &std::process::Child) -> Result<Self, String> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::*;
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            return Err(tr("Could not start the game setup task."));
        }
        let guard = Self(job);
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let set = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        };
        if set == 0 || unsafe { AssignProcessToJobObject(job, child.as_raw_handle()) } == 0 {
            return Err(tr("Could not manage the game setup task."));
        }
        Ok(guard)
    }
}
#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
#[cfg(not(windows))]
struct Job;
#[cfg(not(windows))]
impl Job {
    fn new(_: &std::process::Child) -> Result<Self, String> {
        Ok(Self)
    }
}
#[cfg(not(windows))]
impl Drop for Job {
    fn drop(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kit_update_preserves_generated_resources_and_rejects_symlinks() {
        let mut nonce = [0u8; 12];
        getrandom::fill(&mut nonce).unwrap();
        let suffix: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
        let root = std::env::temp_dir().join(format!("wisp-kit-test-{suffix}"));
        let source = root.join("source");
        let target = root.join("target");
        std::fs::create_dir_all(source.join("exporters")).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(source.join("exporters/Original.java"), "new source").unwrap();
        std::fs::write(target.join("player-resource.bin"), "keep").unwrap();
        copy_tree(&source, &target).unwrap();
        assert_eq!(
            std::fs::read_to_string(target.join("exporters/Original.java")).unwrap(),
            "new source"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("player-resource.bin")).unwrap(),
            "keep"
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("outside"), source.join("link")).unwrap();
            assert!(copy_tree(&source, &target).is_err());
            assert!(!root.join("outside").exists());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
