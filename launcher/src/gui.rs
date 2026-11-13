use gpui::{
    actions, div, prelude::*, px, rgb, size, svg, App, Bounds, Context, Div, FocusHandle,
    FontWeight, KeyBinding, PathPromptOptions, SharedString, Svg, TitlebarOptions, Window,
    WindowBounds, WindowOptions,
};
use wisp_launcher::{i18n::tr, Profile, Settings};
use std::{borrow::Cow, path::PathBuf, process::Child, time::Duration};

actions!(
    local_launcher,
    [Play, Add, Next, ShowPlay, ShowInstallation]
);

const PAGE: u32 = 0x111111;
const SURFACE: u32 = 0x2d2d2d;
const RULE: u32 = 0x3e3e3e;
const INK: u32 = 0xf0f0f0;
const MUTED: u32 = 0xa4a9ba;
const ACCENT: u32 = 0xc2c2c2;

struct Assets;
impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(path
            .strip_prefix("icons/")
            .and_then(wisp_theme::glass_icons::get)
            .map(Cow::Borrowed))
    }
    fn list(&self, _: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(vec![])
    }
}
fn icon(name: &'static str, color: u32) -> Svg {
    svg()
        .path(format!(
            "icons/{}",
            match name {
                "box" => "package-01",
                "folder" => "folder-01",
                "plus" => "plus-sign",
                "chevron-down" => "arrow-down-01",
                "external-link" => "link-01",
                "terminal" => "computer-terminal-01",
                "sliders" => "sliders-horizontal",
                _ => name,
            }
        ))
        .size(px(18.))
        .flex_none()
        .text_color(rgb(color))
}
fn text(value: impl Into<SharedString>) -> SharedString {
    value.into()
}
fn quiet_button(id: &'static str, label: &str, glyph: &'static str) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .px_4()
        .py_3()
        .rounded_lg()
        .bg(rgb(SURFACE))
        .text_color(rgb(INK))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(RULE)))
        .child(icon(glyph, MUTED))
        .child(tr(label))
}
#[derive(Clone, Copy, PartialEq)]
enum Page {
    Play,
    Installation,
}

struct Launcher {
    home: PathBuf,
    settings: Settings,
    status: String,
    child: Option<Child>,
    picking: bool,
    focus: FocusHandle,
    page: Page,
    failed: bool,
    scene: gpui::Entity<crate::scene::Scene>,
}

impl Launcher {
    fn new(home: PathBuf, settings: Settings, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            if this
                .update(cx, |this, cx| {
                    let result = this.child.as_mut().map(Child::try_wait);
                    match result {
                        Some(Ok(Some(status))) => {
                            this.child = None;
                            this.failed = !status.success();
                            this.status = if status.success() {
                                tr("Game closed. Ready when you are.")
                            } else {
                                tr("The game stopped with an error. Open the game log for details.")
                            };
                            cx.notify();
                        }
                        Some(Err(error)) => {
                            this.status = error.to_string();
                            cx.notify();
                        }
                        _ => {}
                    }
                })
                .is_err()
            {
                break;
            }
        })
        .detach();
        Self {
            home,
            settings,
            status: tr("Your builds and worlds stay on this computer."),
            child: None,
            picking: false,
            focus,
            page: Page::Play,
            failed: false,
            scene: cx.new(|_| crate::scene::Scene::new()),
        }
    }

    fn play(&mut self, cx: &mut Context<Self>) {
        if self.child.is_some() {
            return;
        }
        self.failed = false;
        match self
            .settings
            .current()
            .ok_or_else(|| tr("Add a local build to get started."))
            .and_then(|p| p.launch().map_err(|e| e.to_string()))
        {
            Ok(child) => {
                self.child = Some(child);
                self.status = tr("Game running. You can close the launcher and keep playing.");
            }
            Err(error) => {
                self.failed = true;
                self.status = error;
            }
        }
        cx.notify();
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        let mut settings = self.settings.clone();
        settings.selected = Some(settings.profiles[index].name.clone());
        match settings.save(&self.home) {
            Ok(()) => {
                self.settings = settings;
                self.failed = false;
                self.status = tr("Build selected. Ready when you are.");
            }
            Err(error) => {
                self.failed = true;
                self.status = error.to_string();
            }
        }
        cx.notify();
    }

    fn next(&mut self, cx: &mut Context<Self>) {
        if self.settings.profiles.is_empty() {
            return;
        }
        let index = self
            .settings
            .profiles
            .iter()
            .position(|p| Some(&p.name) == self.settings.selected.as_ref())
            .unwrap_or(0);
        self.select((index + 1) % self.settings.profiles.len(), cx);
    }

    fn add(&mut self, cx: &mut Context<Self>) {
        if self.picking {
            return;
        }
        self.picking = true;
        self.status = tr("Choose your Wisp executable.");
        cx.notify();
        let binary = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr("Choose Wisp executable").into()),
        });
        cx.spawn(async move |this, cx| {
            let result: wisp_launcher::Result<Option<Profile>> = async {
                let Some(binary) = binary.await??.and_then(|mut paths| paths.pop()) else {
                    return Ok(None);
                };
                let resources = cx.update(|cx| {
                    cx.prompt_for_paths(PathPromptOptions {
                        files: false,
                        directories: true,
                        multiple: false,
                        prompt: Some(tr("Choose prepared resources").into()),
                    })
                });
                let Some(resources) = resources.await??.and_then(|mut paths| paths.pop()) else {
                    return Ok(None);
                };
                let home = cx.update(|cx| {
                    cx.prompt_for_paths(PathPromptOptions {
                        files: false,
                        directories: true,
                        multiple: false,
                        prompt: Some(tr("Choose game folder for worlds and settings").into()),
                    })
                });
                let Some(home) = home.await??.and_then(|mut paths| paths.pop()) else {
                    return Ok(None);
                };
                Ok(Some(Profile::new(
                    tr("Local build"),
                    &binary,
                    &resources,
                    &home,
                )?))
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.picking = false;
                match result {
                    Ok(Some(mut profile)) => {
                        let mut number = 1;
                        while this.settings.find(&profile.name).is_some() {
                            number += 1;
                            profile.name = format!("{} {number}", tr("Local build"));
                        }
                        let mut settings = this.settings.clone();
                        match settings
                            .add(profile)
                            .and_then(|()| settings.save(&this.home))
                        {
                            Ok(()) => {
                                this.settings = settings;
                                this.failed = false;
                                this.status = tr("Build added. You are ready to play.");
                            }
                            Err(error) => {
                                this.failed = true;
                                this.status = error.to_string();
                            }
                        }
                    }
                    Ok(None) => this.status = tr("Import cancelled. Your builds have not changed."),
                    Err(error) => {
                        this.failed = true;
                        this.status = error.to_string();
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Launcher {
    fn show_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if path.exists() {
            cx.reveal_path(&path);
        } else {
            self.failed = true;
            self.status =
                tr("This path does not exist yet. Start the game to create its folder and log.");
            cx.notify();
        }
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        crate::glass::plate(22.)
            .w(px(248.))
            .flex_none()
            .m(px(14.))
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .py_3()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("Instances")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(text(self.settings.profiles.len().to_string())),
                    ),
            )
            .child(
                div()
                    .id("build-library")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(self.settings.profiles.iter().enumerate().map(|(index, p)| {
                        let selected = self.settings.selected.as_ref() == Some(&p.name);
                        div()
                            .id(("build", index))
                            .flex()
                            .items_center()
                            .gap_3()
                            .p_3()
                            .mb_2()
                            .rounded_xl()
                            .cursor_pointer()
                            .when(selected, |s| s.bg(gpui::rgba(0xffffff18)))
                            .hover(|s| s.bg(gpui::rgba(0xffffff12)))
                            .on_click(cx.listener(move |this, _, _, cx| this.select(index, cx)))
                            .child(
                                div()
                                    .size(px(36.))
                                    .rounded_lg()
                                    .bg(rgb(0xa8a8a8))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon("box", INK)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .truncate()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(text(p.name.clone())),
                                    )
                                    .child(
                                        div().text_xs().text_color(rgb(MUTED)).child(tr(
                                            if selected { "Ready" } else { "Local build" },
                                        )),
                                    ),
                            )
                    }))
                    .when(self.settings.profiles.is_empty(), |s| {
                        s.child(
                            div()
                                .p_2()
                                .text_sm()
                                .text_color(rgb(MUTED))
                                .line_height(px(22.))
                                .child(tr("Your local instances will appear here.")),
                        )
                    }),
            )
            .child(
                quiet_button(
                    "add-build",
                    if self.picking {
                        "Choosing files…"
                    } else {
                        "New instance"
                    },
                    "plus",
                )
                .on_click(cx.listener(|this, _, _, cx| this.add(cx))),
            )
            .child(
                div()
                    .px_2()
                    .py_2()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child(tr("Local builds. Your own worlds.")),
            )
    }

    fn home_head(&self, cx: &mut Context<Self>) -> Div {
        let profile = self.settings.current();
        let ready = profile.is_some_and(|p| p.validate().is_ok()) && self.child.is_none();
        let home = profile.map(|p| p.game_home.clone());
        div()
            .flex()
            .flex_col()
            .gap_4()
            .pt_6()
            .pb_4()
            .child(
                div()
                    .self_start()
                    .px_3()
                    .py_2()
                    .rounded_lg()
                    .bg(gpui::rgba(0xffffff16))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(icon("box", ACCENT))
                    .child(tr("Rust Edition")),
            )
            .child(
                div()
                    .text_size(px(52.))
                    .line_height(px(60.))
                    .font_family(wisp_theme::glass::DISPLAY_FONT)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(
                        profile
                            .map(|p| text(p.name.clone()))
                            .unwrap_or_else(|| text(tr("Wisp"))),
                    ),
            )
            .child(
                div()
                    .text_color(rgb(MUTED))
                    .child(tr(if self.child.is_some() {
                        "Playing now"
                    } else if profile.is_some() {
                        "Ready for your next session"
                    } else {
                        "Add your local Wisp build to get started."
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .mt_4()
                    .child(
                        div()
                            .id("play")
                            .w(px(174.))
                            .h(px(56.))
                            .rounded_xl()
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap_3()
                            .bg(rgb(if ready { 0xa8a8a8 } else { SURFACE }))
                            .text_color(rgb(if ready { INK } else { MUTED }))
                            .font_weight(FontWeight::SEMIBOLD)
                            .when(ready, |button| {
                                button
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(0xb5b5b5)))
                                    .on_click(cx.listener(|this, _, _, cx| this.play(cx)))
                            })
                            .child(icon("play", if ready { INK } else { MUTED }))
                            .child(tr(if self.child.is_some() {
                                "Running"
                            } else {
                                "Play"
                            })),
                    )
                    .child(
                        quiet_button("choose-build", "Switch build", "chevron-down")
                            .on_click(cx.listener(|this, _, _, cx| this.next(cx))),
                    )
                    .when_some(home, |row, path| {
                        row.child(
                            quiet_button("game-folder", "Game folder", "folder").on_click(
                                cx.listener(move |this, _, _, cx| this.show_path(path.clone(), cx)),
                            ),
                        )
                    })
                    .when(profile.is_none(), |row| {
                        row.child(
                            quiet_button("import-home", "Add build", "plus")
                                .on_click(cx.listener(|this, _, _, cx| this.add(cx))),
                        )
                    }),
            )
    }

    fn file_row(
        &self,
        id: usize,
        label: &str,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        let display = path.display().to_string();
        div()
            .id(("file", id))
            .flex()
            .items_center()
            .gap_4()
            .py_4()
            .border_b_1()
            .border_color(rgb(RULE))
            .child(icon("folder", MUTED))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(tr(label))
                    .child(
                        div()
                            .text_xs()
                            .font_family("Geist Mono")
                            .text_color(rgb(MUTED))
                            .truncate()
                            .child(text(display)),
                    ),
            )
            .child(
                div()
                    .id(("reveal", id))
                    .p_2()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(RULE)))
                    .on_click(cx.listener(move |this, _, _, cx| this.show_path(path.clone(), cx)))
                    .child(icon("external-link", MUTED)),
            )
    }

    fn content(&self, cx: &mut Context<Self>) -> Div {
        let mut content = div().flex().flex_col().gap_6().child(self.home_head(cx));
        content = content.child(
            div()
                .flex()
                .gap_2()
                .border_b_1()
                .border_color(gpui::rgba(0xffffff20))
                .pb_3()
                .children(
                    [(Page::Play, "Overview"), (Page::Installation, "Files")]
                        .into_iter()
                        .enumerate()
                        .map(|(i, (page, label))| {
                            div()
                                .id(("section", i))
                                .px_4()
                                .py_2()
                                .rounded_lg()
                                .cursor_pointer()
                                .when(self.page == page, |s| s.bg(gpui::rgba(0xffffff20)))
                                .when(self.page != page, |s| s.text_color(rgb(MUTED)))
                                .hover(|s| s.bg(gpui::rgba(0xffffff16)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.page = page;
                                    cx.notify();
                                }))
                                .child(tr(label))
                        }),
                ),
        );
        if let Some(p) = self.settings.current() {
            let log = p.game_home.join("logs/local-launcher.log");
            let mut files = crate::glass::plate(18.)
                .px_5()
                .py_2()
                .flex()
                .flex_col()
                .child(self.file_row(0, "Executable", p.binary.clone(), cx))
                .child(self.file_row(1, "Resources", p.resources.clone(), cx));
            if self.page == Page::Installation {
                files = files.child(self.file_row(2, "Game folder", p.game_home.clone(), cx));
            }
            content = content.child(files);
            if self.page == Page::Installation {
                content = content.child(
                    div()
                        .flex()
                        .gap_3()
                        .child(
                            quiet_button("open-log", "Open game log", "terminal").on_click(
                                cx.listener(move |this, _, _, cx| this.show_path(log.clone(), cx)),
                            ),
                        )
                        .child(
                            quiet_button("open-config", "Profile settings", "sliders").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.show_path(this.home.join("local-profiles.json"), cx)
                                }),
                            ),
                        ),
                );
            }
        } else {
            content=content.child(crate::glass::plate(18.).p_6().flex().flex_col().gap_3()
                .child(div().font_weight(FontWeight::SEMIBOLD).child(tr("Bring your own build")))
                .child(div().text_color(rgb(MUTED)).line_height(px(22.)).child(tr("Choose your Wisp executable, prepared resources and a game folder. Everything stays on your computer."))));
        }
        content
    }
}

impl Render for Launcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::glass::begin_build(window);
        let invalid = self
            .settings
            .current()
            .and_then(|p| p.validate().err())
            .map(|e| e.to_string());
        let status = invalid.as_deref().unwrap_or(&self.status);
        div()
            .id("wisp-launcher")
            .track_focus(&self.focus)
            .key_context("LocalLauncher")
            .on_action(cx.listener(|this, _: &Play, _, cx| this.play(cx)))
            .on_action(cx.listener(|this, _: &Add, _, cx| this.add(cx)))
            .on_action(cx.listener(|this, _: &Next, _, cx| this.next(cx)))
            .on_action(cx.listener(|this, _: &ShowPlay, _, cx| {
                this.page = Page::Play;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ShowInstallation, _, cx| {
                this.page = Page::Installation;
                cx.notify();
            }))
            .relative()
            .size_full()
            .bg(rgb(PAGE))
            .text_color(rgb(INK))
            .text_size(px(14.))
            .font_family(wisp_theme::glass::FONT)
            .child(self.scene.clone())
            .child(
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex_none()
                            .h(px(76.))
                            .px_6()
                            .flex()
                            .items_center()
                            .gap_6()
                            .child(
                                div()
                                    .w(px(240.))
                                    .flex_none()
                                    .text_size(px(22.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(tr("Wisp")),
                            )
                            .child(
                                crate::glass::plate(16.).p_1().flex().gap_1().children(
                                    [(Page::Play, "Home"), (Page::Installation, "Installation")]
                                        .into_iter()
                                        .enumerate()
                                        .map(|(i, (page, label))| {
                                            div()
                                                .id(("nav", i))
                                                .px_5()
                                                .py_3()
                                                .rounded_xl()
                                                .cursor_pointer()
                                                .when(self.page == page, |s| {
                                                    s.bg(gpui::rgba(0xffffff24))
                                                })
                                                .hover(|s| s.bg(gpui::rgba(0xffffff16)))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.page = page;
                                                    cx.notify();
                                                }))
                                                .child(tr(label))
                                        }),
                                ),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child(tr("Local launcher")),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .child(self.sidebar(cx))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .id("main-scroll")
                                            .flex_1()
                                            .min_h_0()
                                            .overflow_y_scroll()
                                            .px_8()
                                            .pb_8()
                                            .child(self.content(cx)),
                                    )
                                    .child(
                                        div()
                                            .px_8()
                                            .pb_4()
                                            .pt_2()
                                            .text_xs()
                                            .text_color(rgb(if self.failed || invalid.is_some() {
                                                0xffb1a3
                                            } else {
                                                0xc5c0d2
                                            }))
                                            .child(text(status.to_owned())),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .h(px(28.))
                            .px_6()
                            .flex()
                            .justify_between()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(text(format!(
                                "{} {}",
                                tr("Wisp Client"),
                                env!("CARGO_PKG_VERSION")
                            )))
                            .child(tr("Enter to play     Ctrl+O to add a build")),
                    ),
            )
    }
}

pub fn run(home: PathBuf, settings: Settings) {
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            cx.text_system()
                .add_fonts(vec![
                    Cow::Borrowed(wisp_theme::fonts::GEIST_REGULAR),
                    Cow::Borrowed(wisp_theme::fonts::GEIST_SEMIBOLD),
                    Cow::Borrowed(wisp_theme::fonts::GEIST_MONO),
                    Cow::Borrowed(wisp_theme::fonts::BRICOLAGE_REGULAR),
                    Cow::Borrowed(wisp_theme::fonts::BRICOLAGE_SEMIBOLD),
                    Cow::Borrowed(wisp_theme::fonts::BRICOLAGE_DISPLAY_SEMIBOLD),
                ])
                .expect("bundled font");
            cx.bind_keys([
                KeyBinding::new("enter", Play, Some("LocalLauncher")),
                KeyBinding::new("ctrl-o", Add, Some("LocalLauncher")),
                KeyBinding::new("cmd-o", Add, Some("LocalLauncher")),
                KeyBinding::new("tab", Next, Some("LocalLauncher")),
                KeyBinding::new("ctrl-1", ShowPlay, Some("LocalLauncher")),
                KeyBinding::new("ctrl-2", ShowInstallation, Some("LocalLauncher")),
                KeyBinding::new("cmd-1", ShowPlay, Some("LocalLauncher")),
                KeyBinding::new("cmd-2", ShowInstallation, Some("LocalLauncher")),
            ]);
            let bounds = Bounds::centered(None, size(px(1120.), px(800.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(960.), px(640.))),
                    app_id: Some("wisp-local-launcher".into()),
                    titlebar: Some(TitlebarOptions {
                        title: Some(tr("Wisp Client").into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Launcher::new(home, settings, window, cx)),
            )
            .expect("launcher window");
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.activate(true);
        });
}
