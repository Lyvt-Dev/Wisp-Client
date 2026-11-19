//! The beta testers' build (feature `beta`, `build-installer.ps1 -Beta`): the launcher opens only
//! during the beta test, 22 November 2026 from 12:00 to 22:00 German time (CET, UTC+1). Before,
//! it says to wait for the test; after, it thanks the tester and asks for feedback, in German.
//! The published launcher is built without the feature and never shows this.
use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, FontWeight, TitlebarOptions, Window,
    WindowBounds, WindowOptions,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The beta test's start and end (Unix seconds): 22.11.2026 12:00 and 22:00 CET.
pub const START: u64 = 1_795_345_200;
pub const END: u64 = 1_795_381_200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// Before the test: wait.
    Before,
    /// During it: the launcher as usual.
    Open,
    /// After it: thanks.
    After,
}

/// Where `now` (Unix seconds) falls.
pub fn gate_at(now: u64) -> Gate {
    if now < START {
        Gate::Before
    } else if now < END {
        Gate::Open
    } else {
        Gate::After
    }
}

/// The time now; in a development build `WISP_BETA_NOW` (Unix seconds) stands in for it, to see
/// the screens before and after the test.
pub fn now() -> u64 {
    #[cfg(debug_assertions)]
    if let Some(t) = std::env::var("WISP_BETA_NOW")
        .ok()
        .and_then(|t| t.parse().ok())
    {
        return t;
    }
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub fn gate() -> Gate {
    gate_at(now())
}

/// The time left until the start, in German: "2 Tage, 3 Std., 4 Min., 5 Sek.".
pub fn countdown(seconds: u64) -> String {
    let (d, h, m, s) = (
        seconds / 86_400,
        seconds / 3_600 % 24,
        seconds / 60 % 60,
        seconds % 60,
    );
    let mut parts = Vec::new();
    if d > 0 {
        parts.push(format!("{d} {}", if d == 1 { "Tag" } else { "Tage" }));
    }
    if d > 0 || h > 0 {
        parts.push(format!("{h} Std."));
    }
    parts.push(format!("{m} Min."));
    parts.push(format!("{s} Sek."));
    parts.join(", ")
}

/// The window shown outside the test.
struct Notice;

impl Render for Notice {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let (title, body, extra) = match gate() {
            Gate::Before => (
                "Der Beta-Test hat noch nicht begonnen",
                "Bitte warte auf den Beta-Test: Er läuft am 22.11. von 12:00 bis 22:00 Uhr.",
                Some(format!("Noch {}", countdown(START.saturating_sub(now())))),
            ),
            Gate::Open => (
                "Der Beta-Test hat begonnen!",
                "Schließe dieses Fenster und starte den Wisp Launcher neu, um mitzuspielen.",
                None,
            ),
            Gate::After => (
                "Danke fürs Spielen unseres Beta-Launchers!",
                "Der Beta-Test ist vorbei. Du bist herzlich eingeladen, uns Feedback zu hinterlassen.",
                None,
            ),
        };
        div()
            .size_full()
            .bg(rgb(0x0b0b0d))
            .text_color(rgb(0xffffff))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(14.))
            .px(px(48.))
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0x8e9099))
                    .child("WISP CLIENT · BETA"),
            )
            .child(
                div()
                    .text_size(px(30.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_center()
                    .child(title),
            )
            .child(
                div()
                    .max_w(px(520.))
                    .text_size(px(16.))
                    .line_height(px(24.))
                    .text_center()
                    .text_color(rgb(0xc9cbd1))
                    .child(body),
            )
            .when_some(extra, |d, text| {
                d.child(
                    div()
                        .mt(px(8.))
                        .text_size(px(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0xf2c14e))
                        .child(text),
                )
            })
    }
}

/// Opens the notice (the countdown ticks every second).
pub fn open_notice(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(720.), px(420.)), cx);
    let window = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                app_id: Some("wisp-launcher".into()),
                titlebar: Some(TitlebarOptions {
                    title: Some("Wisp Client Beta".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Notice),
        )
        .expect("beta window");
    cx.spawn(async move |cx| loop {
        cx.background_executor().timer(Duration::from_secs(1)).await;
        if window.update(cx, |_, _, cx| cx.notify()).is_err() {
            break;
        }
    })
    .detach();
}

/// While the launcher is open in the test: at its end, every window gives way to the thanks.
pub fn close_at_end(cx: &mut App) {
    cx.spawn(async move |cx| loop {
        cx.background_executor().timer(Duration::from_secs(5)).await;
        if gate() == Gate::After {
            cx.update(|cx| {
                let open = cx.windows();
                open_notice(cx);
                for w in open {
                    let _ = w.update(cx, |_, window, _| window.remove_window());
                }
            });
            break;
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_test_runs_on_22_november_from_12_to_22_german_time() {
        // 22.11.2026 11:59:59 CET, 12:00, 21:59:59 and 22:00.
        assert_eq!(gate_at(START - 1), Gate::Before);
        assert_eq!(gate_at(START), Gate::Open);
        assert_eq!(gate_at(END - 1), Gate::Open);
        assert_eq!(gate_at(END), Gate::After);
        assert_eq!(END - START, 10 * 3_600);
        // 2026-11-22T11:00:00Z: 20,779 days after 1970, 11 hours.
        assert_eq!(START, 20_779 * 86_400 + 11 * 3_600);
    }

    #[test]
    fn the_countdown_is_german() {
        assert_eq!(countdown(65), "1 Min., 5 Sek.");
        assert_eq!(countdown(90_061), "1 Tag, 1 Std., 1 Min., 1 Sek.");
        assert_eq!(countdown(2 * 86_400), "2 Tage, 0 Std., 0 Min., 0 Sek.");
    }
}
