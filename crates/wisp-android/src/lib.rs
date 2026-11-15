//! The Wisp phone app. Android loads `libwisp_android.so` through its NativeActivity (no Java
//! code in the APK) and calls [`android_main`] on a thread of its own; this sets up what a PC's
//! launcher would (folders, the command line, the browser, the log) and runs the game.
#![cfg(target_os = "android")]

use std::path::PathBuf;
use std::sync::OnceLock;
use winit::platform::android::activity::{AndroidApp, WindowManagerFlags};

#[global_allocator]
static ALLOCATOR: wisp_threads::Allocator = wisp_threads::ALLOCATOR;

/// The activity, for the calls into Android (the browser).
static APP: OnceLock<AndroidApp> = OnceLock::new();

/// Android's entry point (android-activity's `ANativeActivity_onCreate` calls it).
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    logcat::redirect_stdio();
    let _ = APP.set(app.clone());

    // The game's home: the app's folder on shared storage, which a PC can reach over USB
    // (Android/data/<package>/files): worlds, screenshots, logs and the game files. The sign-in
    // stays in the private folder no other app or PC can read.
    let private = app
        .internal_data_path()
        .unwrap_or_else(|| PathBuf::from("/data/local/tmp/wisp"));
    let home = app
        .external_data_path()
        .unwrap_or_else(|| private.join("home"));
    let cache = private.join("cache");
    for dir in [&home, &private, &cache] {
        let _ = std::fs::create_dir_all(dir);
    }
    let resources = home.join("resources").join(wisp_core::VANILLA_VERSION);

    // SAFETY: the first thing this process runs: no other thread reads the environment yet.
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_DATA_HOME", &home);
        std::env::set_var("XDG_CONFIG_HOME", home.join("config"));
        std::env::set_var("XDG_CACHE_HOME", &cache);
        std::env::set_var("TMPDIR", &cache);
        std::env::set_var("WISP_HOME", &home);
        // The game asks for a fresh Minecraft session here; the app answers (phone::msa).
        std::env::set_var("WISP_SESSION_RENEW", private.join("renew"));
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    if let Some(language) = app.config().language() {
        wisp_client::phone::text::set_language(&language);
    }
    wisp_client::phone::set_platform(wisp_client::phone::Platform {
        open_url: open_url_in_browser,
    });
    // The screen stays on while Wisp is in front; fullscreen without the status bar.
    app.set_window_flags(
        WindowManagerFlags::KEEP_SCREEN_ON | WindowManagerFlags::FULLSCREEN,
        WindowManagerFlags::empty(),
    );
    wisp_client::game::set_android_app(app);

    let path = |p: &PathBuf| p.to_string_lossy().into_owned();
    wisp_client::entry::set_args(vec![
        "--home".into(),
        path(&home),
        "--game-dir".into(),
        path(&home),
        "--resources".into(),
        path(&resources),
        "--phone-private".into(),
        path(&private),
        "--fullscreen".into(),
    ]);
    wisp_client::entry::main();
    log::info!("Wisp: the game ended");
    // A new start of the app must be a new process: the game's one-time state (the event loop,
    // the installed hooks) can't be made twice.
    std::process::exit(0);
}

/// Opens `url` in the phone's browser: an `ACTION_VIEW` intent from the activity.
fn open_url_in_browser(url: &str) {
    if let Err(e) = open_url(url) {
        log::warn!("Wisp: could not open {url}: {e}");
    }
}

fn open_url(url: &str) -> jni::errors::Result<()> {
    use jni::JavaVM;
    use jni::objects::{JObject, JValue};
    let Some(app) = APP.get() else {
        return Ok(());
    };
    // SAFETY: android-activity hands out the process's JavaVM and the live activity; both stay
    // valid while the app runs.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast())? };
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let mut env = vm.attach_current_thread()?;
    let text = env.new_string(url)?;
    let uri = env
        .call_static_method(
            "android/net/Uri",
            "parse",
            "(Ljava/lang/String;)Landroid/net/Uri;",
            &[JValue::Object(&text)],
        )?
        .l()?;
    let action = env.new_string("android.intent.action.VIEW")?;
    let intent = env.new_object(
        "android/content/Intent",
        "(Ljava/lang/String;Landroid/net/Uri;)V",
        &[JValue::Object(&action), JValue::Object(&uri)],
    )?;
    let result = env.call_method(
        &activity,
        "startActivity",
        "(Landroid/content/Intent;)V",
        &[JValue::Object(&intent)],
    );
    // No browser: Java threw. Clear it, so the next call into Java isn't refused.
    if result.is_err() && env.exception_check()? {
        env.exception_clear()?;
    }
    result.map(|_| ())
}

/// The game logs to standard output and error; Android throws both away. This sends them to
/// logcat (`adb logcat -s wisp`), line by line.
mod logcat {
    use std::ffi::CString;
    use std::io::{BufRead, BufReader};
    use std::os::fd::FromRawFd;

    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_write(
            prio: i32,
            tag: *const libc::c_char,
            text: *const libc::c_char,
        ) -> i32;
    }
    const INFO: i32 = 4;

    pub fn redirect_stdio() {
        let mut fds = [0; 2];
        // SAFETY: plain POSIX calls on descriptors this function owns.
        unsafe {
            if libc::pipe(fds.as_mut_ptr()) != 0 {
                return;
            }
            libc::dup2(fds[1], libc::STDOUT_FILENO);
            libc::dup2(fds[1], libc::STDERR_FILENO);
            libc::close(fds[1]);
        }
        // SAFETY: the read end is ours alone from here on.
        let read = unsafe { std::fs::File::from_raw_fd(fds[0]) };
        let _ = std::thread::Builder::new()
            .name("logcat".into())
            .spawn(move || {
                let tag = CString::new("wisp").expect("no NUL");
                for line in BufReader::new(read).lines().map_while(Result::ok) {
                    let text = CString::new(line.replace('\0', " ")).expect("NULs replaced");
                    // SAFETY: both strings are NUL-terminated and live through the call.
                    unsafe { __android_log_write(INFO, tag.as_ptr(), text.as_ptr()) };
                }
            });
    }
}
