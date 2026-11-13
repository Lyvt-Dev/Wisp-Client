//! The update window: a small second window that opens when the launcher finds a newer Wisp
//! Client, so the player sees an update happening: the version, a progress bar with the share
//! downloaded, then "Installing" until the installer's own progress window takes over.
use gpui::{div, prelude::*, px, relative, rgb, Context, FontWeight, SharedString, Window};
use wisp_launcher::i18n::{tr, trf};

/// Where the update is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Phase {
    /// Downloading, with the share done (0 to 1; `None` while the size is unknown).
    Downloading(Option<f32>),
    /// Downloaded and checked, waiting for the game to close.
    Waiting,
    Installing,
    Failed,
}

pub(super) struct UpdateWindow {
    version: SharedString,
    phase: Phase,
    error: Option<SharedString>,
}

impl UpdateWindow {
    pub(super) fn new(version: &str) -> Self {
        Self {
            version: version.to_owned().into(),
            phase: Phase::Downloading(None),
            error: None,
        }
    }

    pub(super) fn set(&mut self, phase: Phase, cx: &mut Context<Self>) {
        if self.phase != phase {
            self.phase = phase;
            cx.notify();
        }
    }

    pub(super) fn fail(&mut self, error: String, cx: &mut Context<Self>) {
        self.phase = Phase::Failed;
        self.error = Some(error.into());
        cx.notify();
    }

    /// The bar's fill (0 to 1) and the line under the title.
    fn status(&self) -> (f32, String) {
        match self.phase {
            Phase::Downloading(Some(f)) => (
                f,
                trf(
                    "Downloading… {percent}%",
                    &[("percent", &format!("{}", (f * 100.0).floor() as u32))],
                ),
            ),
            Phase::Downloading(None) => (0.0, tr("Downloading…")),
            Phase::Waiting => (1.0, tr("Ready. It installs as soon as the game is closed.")),
            Phase::Installing => (1.0, tr("Installing… Wisp Client opens again by itself.")),
            Phase::Failed => (
                0.0,
                trf(
                    "The update failed: {error}",
                    &[("error", self.error.as_deref().unwrap_or_default())],
                ),
            ),
        }
    }
}

impl Render for UpdateWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let (fill, line) = self.status();
        let bar = if self.phase == Phase::Failed {
            0xe5484d
        } else {
            0xffffff
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(14.))
            .px(px(28.))
            .bg(rgb(0x0b0b0c))
            .text_color(rgb(0xf2f2f2))
            .font_family("Geist")
            .text_size(px(15.))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(trf(
                        "Updating to Wisp Client {version}",
                        &[("version", &self.version)],
                    )),
            )
            .child(
                div()
                    .w_full()
                    .h(px(8.))
                    .rounded(px(4.))
                    .bg(rgb(0x2a2a2e))
                    .child(
                        div()
                            .h_full()
                            .w(relative(fill.clamp(0.0, 1.0)))
                            .rounded(px(4.))
                            .bg(rgb(bar)),
                    ),
            )
            .child(div().text_color(rgb(0xa1a1aa)).child(line))
    }
}
