//! Animation clock retained from the original Wisp launcher.
use std::sync::LazyLock;
use std::time::{Duration, Instant};
/// How the sky moves: not at all under the system's reduced motion (macOS) or with
/// `WISP_LAUNCHER_MOTION=off`; while the window is in front (and 10 s after); or always, with
/// `WISP_LAUNCHER_MOTION=always`, for recordings.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Motion {
    Still,
    InFront,
    Always,
}

pub(crate) fn motion() -> Motion {
    static MOTION: LazyLock<Motion> = LazyLock::new(|| {
        match wisp_compat::var("WISP_LAUNCHER_MOTION").as_deref() {
            Ok("off" | "reduced") => return Motion::Still,
            Ok("always") => return Motion::Always,
            Ok("full") => return Motion::InFront,
            _ => {}
        }
        #[cfg(target_os = "macos")]
        if objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion() {
            return Motion::Still;
        }
        Motion::InFront
    });
    *MOTION
}

/// The animation's own time: it runs only while the sky moves, so a window that comes back
/// picks up where it stopped instead of jumping.
#[derive(Default)]
pub(crate) struct Clock {
    banked: Duration,
    running: Option<Instant>,
}

impl Clock {
    pub(crate) fn at(&self, now: Instant) -> Duration {
        self.banked + self.running.map_or(Duration::ZERO, |since| now - since)
    }

    pub(crate) fn set(&mut self, run: bool, now: Instant) {
        match (run, self.running) {
            (true, None) => self.running = Some(now),
            (false, Some(since)) => {
                self.banked += now - since;
                self.running = None;
            }
            _ => {}
        }
    }
}
