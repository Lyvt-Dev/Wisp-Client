use std::{fs, path::PathBuf, process::Command};
use wisp_launcher::{Profile, Settings};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "wisp-local-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn profile(&self) -> Profile {
        let binary = self.0.join("local game");
        fs::write(&binary, "placeholder").unwrap();
        let resources = self.0.join("prepared resources");
        fs::create_dir_all(&resources).unwrap();
        Profile::new("Local".into(), &binary, &resources, &self.0.join("worlds")).unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn profiles_round_trip_without_touching_old_accounts_or_game_data() {
    let scratch = Scratch::new();
    fs::write(scratch.0.join("accounts.json"), "private upstream data").unwrap();
    let mut settings = Settings::load(&scratch.0).unwrap();
    settings.add(scratch.profile()).unwrap();
    settings.save(&scratch.0).unwrap();
    let restored = Settings::load(&scratch.0).unwrap();
    assert_eq!(restored.current().unwrap().name, "Local");
    assert!(restored.current().unwrap().binary.is_absolute());
    assert!(!restored.current().unwrap().game_home.exists());
    assert_eq!(
        fs::read_to_string(scratch.0.join("accounts.json")).unwrap(),
        "private upstream data"
    );
    assert!(settings.add(scratch.profile()).is_err());
}

#[test]
fn legacy_profiles_remain_local_and_online_launch_requires_credentials_before_writes() {
    let scratch = Scratch::new();
    let mut legacy = serde_json::to_value(scratch.profile()).unwrap();
    legacy.as_object_mut().unwrap().remove("use_account");
    let mut profile: Profile = serde_json::from_value(legacy).unwrap();
    assert!(!profile.use_account);
    profile.use_account = true;
    assert!(profile.launch().is_err());
    assert!(profile.launch_with_credentials(None).is_err());
    assert!(!profile.game_home.exists());
}

#[test]
fn corrupt_settings_are_reported_and_preserved() {
    let scratch = Scratch::new();
    fs::write(scratch.0.join("local-profiles.json"), "invalid json").unwrap();
    assert!(Settings::load(&scratch.0).is_err());
    assert_eq!(
        fs::read_to_string(scratch.0.join("local-profiles.json")).unwrap(),
        "invalid json"
    );
}

#[test]
fn missing_build_is_rejected_before_creating_game_files() {
    let scratch = Scratch::new();
    let p = scratch.profile();
    fs::remove_file(&p.binary).unwrap();
    assert!(p.launch().is_err());
    assert!(!p.game_home.exists());
}

#[cfg(unix)]
#[test]
fn starts_local_process_with_literal_arguments_isolated_home_and_logs() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new();
    let mut p = scratch.profile();
    // A local process fixture verifies the actual child environment and argument boundaries.
    fs::write(&p.binary, "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$WISP_HOME\" \"$WISP_TELEMETRY_URL\" \"$WISP_COSMETICS\" \"${WISP_ACCOUNT_SESSIONS-unset}\" \"$@\"\nprintf 'fixture stderr\\n' >&2\nexit 7\n").unwrap();
    fs::set_permissions(&p.binary, fs::Permissions::from_mode(0o700)).unwrap();
    p.args = vec!["--world".into(), "a world; $(echo unsafe)".into()];
    assert_eq!(p.launch().unwrap().wait().unwrap().code(), Some(7));
    let log = fs::read_to_string(p.game_home.join("logs/local-launcher.log")).unwrap();
    let lines: Vec<_> = log.lines().collect();
    assert_eq!(lines[0], p.game_home.to_str().unwrap());
    assert_eq!(lines[1], p.game_home.to_str().unwrap());
    assert_eq!(&lines[2..6], &["off", "0", "unset", "--resources"]);
    assert_eq!(lines[6], p.resources.to_str().unwrap());
    assert_eq!(
        &lines[7..],
        &["--world", "a world; $(echo unsafe)", "fixture stderr"]
    );
}

#[test]
fn cli_remove_keeps_installation_and_worlds() {
    let scratch = Scratch::new();
    let p = scratch.profile();
    fs::create_dir_all(&p.game_home).unwrap();
    fs::write(p.game_home.join("world.txt"), "keep").unwrap();
    let mut settings = Settings::default();
    settings.add(p.clone()).unwrap();
    settings.save(&scratch.0).unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_wisp-launcher"))
        .env("WISP_LAUNCHER_HOME", &scratch.0)
        .args(["remove", "Local"])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(Settings::load(&scratch.0).unwrap().profiles.is_empty());
    assert!(p.binary.is_file());
    assert_eq!(
        fs::read_to_string(p.game_home.join("world.txt")).unwrap(),
        "keep"
    );
}

#[cfg(windows)]
#[test]
fn game_process_has_no_console_and_keeps_its_log_output() {
    use std::os::windows::process::CommandExt;
    let scratch = Scratch::new();
    let source = scratch.0.join("console_probe.rs");
    let binary = scratch.0.join("console_probe.exe");
    fs::write(
        &source,
        r#"
        #[link(name = "kernel32")]
        extern "system" { fn GetConsoleWindow() -> *mut std::ffi::c_void; }
        fn main() {
            println!("console={}", !unsafe { GetConsoleWindow() }.is_null());
            println!("arguments={:?}", std::env::args().skip(1).collect::<Vec<_>>());
            eprintln!("game stderr is still logged");
        }
    "#,
    )
    .unwrap();
    let compiled = Command::new("rustc")
        .args(["--edition=2021", "--crate-name", "wisp_console_probe"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .creation_flags(0x08000000)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let mut profile = scratch.profile();
    profile.binary = binary;
    profile.args = vec!["--world".into(), "A world with spaces".into()];
    assert!(profile.launch().unwrap().wait().unwrap().success());
    let log = fs::read_to_string(profile.game_home.join("logs/local-launcher.log")).unwrap();
    assert!(log.contains("console=false"), "{log}");
    assert!(
        log.contains("--resources") && log.contains("A world with spaces"),
        "{log}"
    );
    assert!(log.contains("game stderr is still logged"), "{log}");
}
