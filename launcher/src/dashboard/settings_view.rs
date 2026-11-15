//! Settings: a list of sections on the left (General, Game, Appearance, Cosmetics, Accounts,
//! Storage, About) and the chosen section's settings on the right, each a row with its name,
//! what it does and its control. Changes save at once (`wisp_launcher::prefs`).
use super::catalog_views::*;
use super::reference_views::{glow, luminous};
use super::*;
use wisp_launcher::prefs::{
    self, Accent, AfterLaunch, Backdrop, Banner, Others, Prefs, StartPage, RENDER_DISTANCES,
    UI_SIZES, WINDOW_SIZES,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Section {
    #[default]
    General,
    Game,
    Appearance,
    Cosmetics,
    Accounts,
    Phone,
    Storage,
    About,
}

impl Section {
    /// Development builds only: the section `WISP_LAUNCHER_SETTINGS` names (to look at it).
    pub(super) fn from_env() -> Self {
        let name = wisp_compat::var("WISP_LAUNCHER_SETTINGS")
            .ok()
            .filter(|_| cfg!(debug_assertions))
            .unwrap_or_default();
        Self::ALL
            .into_iter()
            .find(|s| {
                s.label().to_lowercase().starts_with(&name.to_lowercase()) && !name.is_empty()
            })
            .unwrap_or_default()
    }
    const ALL: [Self; 8] = [
        Self::General,
        Self::Game,
        Self::Appearance,
        Self::Cosmetics,
        Self::Accounts,
        Self::Phone,
        Self::Storage,
        Self::About,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Game => "Game",
            Self::Appearance => "Appearance",
            Self::Cosmetics => "Cosmetics & emotes",
            Self::Accounts => "Accounts",
            Self::Phone => "Phone",
            Self::Storage => "Storage",
            Self::About => "About",
        }
    }
    fn glyph(self) -> &'static str {
        match self {
            Self::General => "settings",
            Self::Game => "play",
            Self::Appearance => "spark",
            Self::Cosmetics => "star",
            Self::Accounts => "user-circle",
            Self::Phone => "compass",
            Self::Storage => "folder",
            Self::About => "info",
        }
    }
    fn summary(self) -> &'static str {
        match self {
            Self::General => "What Wisp does when it opens and when the game starts.",
            Self::Game => "How the game window opens and what it starts with.",
            Self::Appearance => "Colours, background, banner and layout of the launcher.",
            Self::Cosmetics => "What you show other Wisp players, and what you see of theirs.",
            Self::Accounts => "Your Microsoft account and how each instance signs in.",
            Self::Phone => "Play Wisp on your Android phone with this PC's game files.",
            Self::Storage => "Where Wisp keeps its files, and how much room they take.",
            Self::About => "Versions, licenses and where to get help.",
        }
    }
}

/// One folder on the Storage page: its name, where it is and how big it was when measured.
#[derive(Clone)]
pub(super) struct Folder {
    label: &'static str,
    path: PathBuf,
    size: u64,
}

fn heading(label: &str, s: f32) -> Div {
    div()
        .mt(px(18. * s))
        .mb(px(8. * s))
        .text_size(px(13. * s))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(0x8d909c))
        .child(tr(label).to_uppercase())
}

/// A setting: its name and what it does on the left, its control on the right.
fn row(title: &str, description: &str, control: impl IntoElement, s: f32) -> Div {
    div()
        .w_full()
        .mb(px(8. * s))
        .px(px(18. * s))
        .py(px(14. * s))
        .rounded(px(10. * s))
        .border_1()
        .border_color(rgb(tone(0x2a2b30)))
        .bg(rgb(tone(0x17181b)))
        .flex()
        .items_center()
        .gap(px(24. * s))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4. * s))
                .child(
                    div()
                        .text_size(px(16. * s))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tr(title)),
                )
                .child(
                    div()
                        .text_size(px(14. * s))
                        .line_height(px(20. * s))
                        .text_color(rgb(MUTED))
                        .child(tr(description)),
                ),
        )
        .child(div().flex_none().child(control))
}

/// An on/off switch.
fn switch(id: impl Into<ElementId>, on: bool, s: f32) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(48. * s))
        .h(px(27. * s))
        .rounded_full()
        .p(px(3. * s))
        .cursor_pointer()
        .border_1()
        .border_color(rgb(if on { 0xd6d7da } else { tone(0x53555f) }))
        .when(on, |d| d.bg(luminous()).shadow(glow(s * 0.6)))
        .when(!on, |d| d.bg(rgb(tone(0x212226))))
        .flex()
        .when(on, |d| d.justify_end())
        .child(
            div()
                .size(px(19. * s))
                .rounded_full()
                .bg(rgb(if on { 0xffffff } else { 0x7d818e })),
        )
}

/// One option of a segmented choice.
fn segment(id: impl Into<ElementId>, label: &str, on: bool, s: f32) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(34. * s))
        .px(px(13. * s))
        .rounded(px(9. * s))
        .flex()
        .items_center()
        .text_size(px(14. * s))
        .cursor_pointer()
        .when(on, |d| d.bg(luminous()).shadow(glow(s * 0.5)))
        .when(!on, |d| {
            d.text_color(rgb(MUTED))
                .hover(|d| d.bg(rgb(tone(0x2b2c31))))
        })
        .child(tr(label))
}

fn segments(s: f32) -> Div {
    div()
        .flex()
        .flex_wrap()
        .justify_end()
        .gap(px(4. * s))
        .p(px(4. * s))
        .rounded(px(12. * s))
        .bg(rgb(tone(0x191a1d)))
        .border_1()
        .border_color(rgb(tone(0x35373e)))
}

fn mono(value: impl Into<SharedString>, s: f32) -> Div {
    div()
        .text_size(px(13. * s))
        .font_family("Geist Mono")
        .text_color(rgb(MUTED))
        .truncate()
        .child(value.into())
}

fn size_label((w, h): (u32, u32)) -> String {
    if w == 0 {
        tr("Game default")
    } else {
        format!("{w} × {h}")
    }
}

impl Launcher {
    /// Changes the launcher's preferences with `change`, saves them and tells the player.
    pub(super) fn set_prefs(&mut self, change: impl FnOnce(&mut Prefs), cx: &mut Context<Self>) {
        let mut next = self.prefs.clone();
        change(&mut next);
        if next == self.prefs {
            return;
        }
        let cosmetics = (next.show_mine, next.others, next.auras, next.share_emotes)
            != (
                self.prefs.show_mine,
                self.prefs.others,
                self.prefs.auras,
                self.prefs.share_emotes,
            );
        match next.save(&self.home) {
            Ok(()) => {
                self.prefs = next;
                if cosmetics {
                    self.write_cosmetic_prefs();
                }
                self.failed = false;
                self.status = tr("Settings saved.");
            }
            Err(e) => self.error(e.to_string()),
        }
        cx.notify();
    }

    /// Puts the cosmetics switches into every instance's game folder, where the game reads them.
    pub(super) fn write_cosmetic_prefs(&self) {
        for profile in &self.settings.profiles {
            if let Err(e) = self.prefs.write_cosmetics(&profile.game_home) {
                eprintln!("Wisp settings: {}: {e}", profile.game_home.display());
            }
        }
    }

    /// Measures the Storage page's folders off the interface thread.
    fn measure_storage(&mut self, cx: &mut Context<Self>) {
        let mut folders = vec![
            ("Launcher data", self.home.clone()),
            ("Logs", self.home.join("logs")),
            ("Shop items", wisp_launcher::shop::worn_dir(&self.home)),
        ];
        if let Some(p) = self.settings.current() {
            folders.push(("Selected instance", p.game_home.clone()));
            folders.push(("Game resources", p.resources.clone()));
            folders.push(("Cosmetics cache", p.game_home.join("assets/cosmetics")));
        }
        self.storage = None;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let measured = folders
                .into_iter()
                .map(|(label, path)| Folder {
                    label,
                    size: prefs::folder_size(&path),
                    path,
                })
                .collect();
            let _ = sender.send(Event::Storage(measured));
        });
        cx.notify();
    }

    pub(super) fn settings_page(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let section = self.settings_section;
        let mut nav = div()
            .w(px(240. * s))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(2. * s));
        for (index, item) in Section::ALL.into_iter().enumerate() {
            let on = item == section;
            nav = nav.child(
                div()
                    .id(("settings-section", index))
                    .relative()
                    .h(px(42. * s))
                    .px(px(14. * s))
                    .rounded(px(9. * s))
                    .flex()
                    .items_center()
                    .gap(px(12. * s))
                    .cursor_pointer()
                    .when(on, |d| d.bg(rgb(tone(0x1f2024))))
                    .when(!on, |d| d.hover(|d| d.bg(rgb(tone(0x18191d)))))
                    .when(on, |d| {
                        d.child(
                            div()
                                .absolute()
                                .left_0()
                                .top(px(11. * s))
                                .w(px(3. * s))
                                .h(px(20. * s))
                                .rounded_r(px(3. * s))
                                .bg(rgb(accent_tone(ACCENT))),
                        )
                    })
                    .child(
                        icon(item.glyph(), if on { accent_tone(ACCENT) } else { MUTED })
                            .size(px(18. * s)),
                    )
                    .child(
                        div()
                            .text_size(px(15. * s))
                            .text_color(rgb(if on { INK } else { 0xc4c6cc }))
                            .when(on, |d| d.font_weight(FontWeight::SEMIBOLD))
                            .child(tr(item.label())),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings_section = item;
                        if item == Section::Storage {
                            this.measure_storage(cx);
                        }
                        cx.notify();
                    })),
            );
        }
        let body = match section {
            Section::General => self.settings_general(cx),
            Section::Game => self.settings_game(cx),
            Section::Appearance => self.settings_appearance(cx),
            Section::Cosmetics => self.settings_cosmetics(cx),
            Section::Accounts => self.settings_accounts(cx),
            Section::Phone => self.settings_phone(cx),
            Section::Storage => self.settings_storage(cx),
            Section::About => self.settings_about(cx),
        };
        let page = div()
            .flex()
            .flex_col()
            .gap(px(18. * s))
            .pb(px(20. * s))
            .child(
                div()
                    .child(
                        div()
                            .text_size(px(30. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("Settings")),
                    )
                    .child(
                        div()
                            .text_size(px(15. * s))
                            .text_color(rgb(MUTED))
                            .child(trf(
                                "Wisp {version}. Changes save as you make them.",
                                &[("version", env!("CARGO_PKG_VERSION"))],
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(28. * s))
                    .child(nav)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10. * s))
                                    .child(
                                        icon(section.glyph(), accent_tone(ACCENT))
                                            .size(px(22. * s)),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(22. * s))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(tr(section.label())),
                                    ),
                            )
                            .child(
                                div()
                                    .mt(px(4. * s))
                                    .text_size(px(14. * s))
                                    .text_color(rgb(MUTED))
                                    .child(tr(section.summary())),
                            )
                            .child(body),
                    ),
            );
        div().size_full().child(
            div()
                .id("settings-scroll")
                .size_full()
                .overflow_y_scroll()
                .child(page),
        )
    }

    fn settings_general(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let p = &self.prefs;
        let mut after = segments(s);
        for (i, choice) in AfterLaunch::ALL.into_iter().enumerate() {
            after = after.child(
                segment(
                    ("after-launch", i),
                    choice.label(),
                    p.after_launch == choice,
                    s,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_prefs(|p| p.after_launch = choice, cx)
                })),
            );
        }
        let mut start = segments(s);
        for (i, choice) in StartPage::ALL.into_iter().enumerate() {
            start = start.child(
                segment(("start-page", i), choice.label(), p.start_page == choice, s).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.set_prefs(|p| p.start_page = choice, cx)
                    }),
                ),
            );
        }
        div()
            .flex()
            .flex_col()
            .child(heading("Launching", s))
            .child(row(
                "When the game starts",
                "Keep Wisp open, tuck it into the taskbar, or close it. The game keeps running either way.",
                after,
                s,
            ))
            .child(heading("Startup", s))
            .child(row(
                "Open on",
                "The page Wisp shows when it starts.",
                start,
                s,
            ))
            .child(heading("Updates", s))
            .child(row(
                "Update automatically",
                "Download new Wisp Client versions in the background and install them once no game is running.",
                switch("auto-update", p.auto_update, s).on_click(cx.listener(|this, _, _, cx| {
                    this.set_prefs(|p| p.auto_update = !p.auto_update, cx)
                })),
                s,
            ))
            .child(heading("Wisp server", s))
            .child(row(
                "Server address",
                "The Wisp server for friends and shared cosmetics, like https://wisp.example.com. Empty: the one built into this launcher. The game uses it too.",
                div().w(px(360. * s)).child(self.service_input.clone()),
                s,
            ))
            .child(row(
                "Status",
                "Whether friends work with this address right now.",
                div()
                    .max_w(px(360. * s))
                    .text_size(px(13. * s))
                    .text_color(rgb(if self.friends.is_some() { accent_tone(0x6fe792) } else { MUTED }))
                    .child(match (&self.friends, &self.friends_state) {
                        (Some(_), _) => tr("Connected"),
                        (None, Some(why)) => why.clone(),
                        (None, None) => tr("Connecting…"),
                    }),
                s,
            ))
    }

    fn settings_game(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let p = &self.prefs;
        let mut window = segments(s).max_w(px(430. * s));
        for (i, size) in WINDOW_SIZES.into_iter().enumerate() {
            window = window.child(
                segment(("window-size", i), &size_label(size), p.window == size, s).on_click(
                    cx.listener(move |this, _, _, cx| this.set_prefs(|p| p.window = size, cx)),
                ),
            );
        }
        let mut distance = segments(s).max_w(px(430. * s));
        for (i, chunks) in RENDER_DISTANCES.into_iter().enumerate() {
            let label = if chunks == 0 {
                tr("Game setting")
            } else {
                trf("{n} chunks", &[("n", &chunks.to_string())])
            };
            distance = distance.child(
                segment(
                    ("render-distance", i),
                    &label,
                    p.render_distance == chunks,
                    s,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_prefs(|p| p.render_distance = chunks, cx)
                })),
            );
        }
        let args = p.game_args();
        div()
            .flex()
            .flex_col()
            .child(heading("Window", s))
            .child(row(
                "Window size",
                "The size the game window opens at. Game default remembers the last size you used.",
                window,
                s,
            ))
            .child(row(
                "Start in fullscreen",
                "Open the game fullscreen. F11 switches back in the game.",
                switch("fullscreen", p.fullscreen, s).on_click(cx.listener(|this, _, _, cx| {
                    this.set_prefs(|p| p.fullscreen = !p.fullscreen, cx)
                })),
                s,
            ))
            // Linux only: Wayland can hold a window's frames to the display's refresh.
            .when(cfg!(target_os = "linux"), |d| {
                d.child(row(
                    "Run through X11 (XWayland)",
                    "On a Wayland desktop the game can be held to your screen's refresh rate even with VSync off. This opens it through XWayland instead, with no such cap.",
                    switch("x11", p.x11, s).on_click(cx.listener(|this, _, _, cx| {
                        this.set_prefs(|p| p.x11 = !p.x11, cx)
                    })),
                    s,
                ))
            })
            .child(heading("World", s))
            .child(row(
                "Render distance",
                "How far you see, for this start only. Game setting uses what you chose in the game's Video Settings.",
                distance,
                s,
            ))
            .child(heading("Logs", s))
            .child(row(
                "Keep the previous log",
                "Save the last game's log as local-launcher.prev.log before a new start, handy after a crash.",
                switch("keep-logs", p.keep_logs, s).on_click(cx.listener(|this, _, _, cx| {
                    this.set_prefs(|p| p.keep_logs = !p.keep_logs, cx)
                })),
                s,
            ))
            .child(row(
                "Game log",
                "The selected instance's log of its last start.",
                cat_button("open-game-log", "Open logs", "terminal", false, s).on_click(
                    cx.listener(|this, _, _, cx| this.open_game_folder("logs", cx)),
                ),
                s,
            ))
            .child(
                div()
                    .mt(px(14. * s))
                    .p(px(14. * s))
                    .rounded(px(12. * s))
                    .bg(rgb(tone(0x191a1d)))
                    .flex()
                    .flex_col()
                    .gap(px(6. * s))
                    .child(
                        div()
                            .text_size(px(13. * s))
                            .text_color(rgb(0x8d909c))
                            .child(tr("Starts the game with")),
                    )
                    .child(mono(
                        if args.is_empty() {
                            tr("(no extra arguments)")
                        } else {
                            args.join(" ")
                        },
                        s,
                    )),
            )
    }

    fn settings_appearance(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let p = &self.prefs;
        let mut size = segments(s);
        for (i, factor) in UI_SIZES.into_iter().enumerate() {
            let label = if (factor - 1.0).abs() < 1e-3 {
                tr("Default")
            } else {
                format!("{}%", (factor * 100.0).round())
            };
            size = size.child(
                segment(("ui-size", i), &label, (p.ui_size - factor).abs() < 1e-3, s).on_click(
                    cx.listener(move |this, _, _, cx| this.set_prefs(|p| p.ui_size = factor, cx)),
                ),
            );
        }
        let accents = div().flex().flex_wrap().gap(px(12. * s)).children(
            Accent::ALL.into_iter().enumerate().map(|(i, accent)| {
                let on = p.accent == accent;
                div()
                    .id(("accent", i))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(6. * s))
                    .cursor_pointer()
                    .child(
                        div()
                            .size(px(42. * s))
                            .rounded_full()
                            .border_2()
                            .border_color(if on { rgb(0xffffff) } else { rgba(0) })
                            .p(px(3. * s))
                            .child(
                                div()
                                    .size_full()
                                    .rounded_full()
                                    .bg(rgb(accent.rgb()))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(on, |d| {
                                        d.child(
                                            icon("checkmark-circle-01", 0xffffff).size(px(16. * s)),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12. * s))
                            .text_color(rgb(if on { INK } else { MUTED }))
                            .child(tr(accent.label())),
                    )
                    .on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.set_prefs(|p| p.accent = accent, cx)
                        }),
                    )
            }),
        );
        let backdrops = div().flex().flex_wrap().gap(px(12. * s)).children(
            Backdrop::ALL.into_iter().enumerate().map(|(i, backdrop)| {
                let on = p.backdrop == backdrop;
                let [frame, page, panel] = theme::preview(backdrop);
                div()
                    .id(("backdrop", i))
                    .w(px(132. * s))
                    .flex()
                    .flex_col()
                    .gap(px(6. * s))
                    .cursor_pointer()
                    .child(
                        div()
                            .h(px(78. * s))
                            .rounded(px(10. * s))
                            .border_2()
                            .border_color(if on {
                                rgb(accent_tone(ACCENT))
                            } else {
                                rgb(tone(0x2b2c32))
                            })
                            .bg(rgb(frame))
                            .overflow_hidden()
                            .flex()
                            .child(div().w(px(18. * s)).h_full())
                            .child(
                                div()
                                    .flex_1()
                                    .mt(px(10. * s))
                                    .rounded_tl(px(6. * s))
                                    .bg(rgb(page))
                                    .p(px(7. * s))
                                    .flex()
                                    .flex_col()
                                    .gap(px(5. * s))
                                    .child(div().h(px(22. * s)).rounded(px(4. * s)).bg(rgb(panel)))
                                    .child(
                                        div()
                                            .w(px(40. * s))
                                            .h(px(8. * s))
                                            .rounded(px(3. * s))
                                            .bg(rgb(accent_tone(ACCENT))),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(13. * s))
                            .text_color(rgb(if on { INK } else { MUTED }))
                            .child(tr(backdrop.label())),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_prefs(|p| p.backdrop = backdrop, cx)
                    }))
            }),
        );
        let banners = div().flex().flex_wrap().gap(px(12. * s)).children(
            Banner::ALL.into_iter().enumerate().map(|(i, banner)| {
                let on = p.banner == banner;
                div()
                    .id(("banner", i))
                    .w(px(160. * s))
                    .flex()
                    .flex_col()
                    .gap(px(6. * s))
                    .cursor_pointer()
                    .child(
                        div()
                            .h(px(84. * s))
                            .rounded(px(10. * s))
                            .border_2()
                            .border_color(if on {
                                rgb(accent_tone(ACCENT))
                            } else {
                                rgb(tone(0x2b2c32))
                            })
                            .overflow_hidden()
                            .child(img(banner.asset()).size_full().object_fit(ObjectFit::Cover)),
                    )
                    .child(
                        div()
                            .text_size(px(13. * s))
                            .text_color(rgb(if on { INK } else { MUTED }))
                            .child(tr(banner.label())),
                    )
                    .on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.set_prefs(|p| p.banner = banner, cx)
                        }),
                    )
            }),
        );
        let block = |title: &str, description: &str, control: Div| {
            div()
                .w_full()
                .mb(px(8. * s))
                .px(px(18. * s))
                .py(px(16. * s))
                .rounded(px(10. * s))
                .border_1()
                .border_color(rgb(tone(0x2a2b30)))
                .bg(rgb(tone(0x17181b)))
                .flex()
                .flex_col()
                .gap(px(12. * s))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4. * s))
                        .child(
                            div()
                                .text_size(px(16. * s))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(tr(title)),
                        )
                        .child(
                            div()
                                .text_size(px(14. * s))
                                .text_color(rgb(MUTED))
                                .child(tr(description)),
                        ),
                )
                .child(control)
        };
        div()
            .flex()
            .flex_col()
            .child(heading("Colours", s))
            .child(block(
                "Accent colour",
                "Buttons, highlights and the active page.",
                accents,
            ))
            .child(block(
                "Background",
                "The greys behind everything.",
                backdrops,
            ))
            .child(heading("Home", s))
            .child(block(
                "Banner artwork",
                "The picture behind the Launch button.",
                banners,
            ))
            .child(heading("Layout", s))
            .child(row(
                "Interface size",
                "Make everything in Wisp larger or smaller. It still fits itself to the window.",
                size,
                s,
            ))
            .child(row(
                "Side panel",
                "Daily reward, instances, equipped items and friends on the right of wide windows.",
                switch("side-panel", p.side_panel, s).on_click(cx.listener(|this, _, _, cx| {
                    this.set_prefs(|p| p.side_panel = !p.side_panel, cx)
                })),
                s,
            ))
            .child(
                div()
                    .id("appearance-reset")
                    .mt(px(12. * s))
                    .self_start()
                    .px(px(14. * s))
                    .py(px(8. * s))
                    .rounded(px(8. * s))
                    .border_1()
                    .border_color(rgb(tone(0x35373e)))
                    .text_size(px(14. * s))
                    .text_color(rgb(MUTED))
                    .cursor_pointer()
                    .hover(|d| d.text_color(rgb(INK)))
                    .child(tr("Reset appearance"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_prefs(
                            |p| {
                                let d = Prefs::default();
                                p.accent = d.accent;
                                p.backdrop = d.backdrop;
                                p.banner = d.banner;
                                p.ui_size = d.ui_size;
                                p.side_panel = d.side_panel;
                            },
                            cx,
                        )
                    })),
            )
    }

    fn settings_cosmetics(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let p = &self.prefs;
        let mut others = segments(s);
        for (i, choice) in Others::ALL.into_iter().enumerate() {
            others = others.child(
                segment(("others", i), choice.label(), p.others == choice, s).on_click(
                    cx.listener(move |this, _, _, cx| this.set_prefs(|p| p.others = choice, cx)),
                ),
            );
        }
        let online = wisp_launcher::shop::service().is_some();
        let worn = self.wallet.worn_count();
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .mb(px(6. * s))
                    .p(px(14. * s))
                    .rounded(px(12. * s))
                    .bg(rgb(if online { tone(0x1f2a24) } else { tone(0x222327) }))
                    .flex()
                    .items_center()
                    .gap(px(12. * s))
                    .child(icon(if online { "shield-check" } else { "info" }, accent_tone(ACCENT)))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(14. * s))
                            .line_height(px(20. * s))
                            .child(if online {
                                trf(
                                    "Online: other Wisp players see the {n} items you wear and your emotes, on any server. Sign in with Microsoft to share them.",
                                    &[("n", &worn.to_string())],
                                )
                            } else {
                                tr("The cosmetics server isn't set up in this build: only you see your items.")
                            }),
                    ),
            )
            .child(heading("You", s))
            .child(row(
                "Show my cosmetics",
                "Other Wisp players see what you wear. Off, everyone sees you without them, you included.",
                switch("show-mine", p.show_mine, s).on_click(cx.listener(|this, _, _, cx| {
                    this.set_prefs(|p| p.show_mine = !p.show_mine, cx)
                })),
                s,
            ))
            .child(row(
                "Share my emotes",
                "Players near you see your emotes too. Off, only you see them.",
                switch("share-emotes", p.share_emotes, s).on_click(cx.listener(|this, _, _, cx| {
                    this.set_prefs(|p| p.share_emotes = !p.share_emotes, cx)
                })),
                s,
            ))
            .child(heading("Other players", s))
            .child(row(
                "Their cosmetics",
                "What you see on other Wisp players. Hidden, the game never asks what they wear.",
                others,
                s,
            ))
            .child(row(
                "Their auras and trails",
                "Particles around and behind other players.",
                switch("auras", p.auras, s).on_click(cx.listener(|this, _, _, cx| {
                    this.set_prefs(|p| p.auras = !p.auras, cx)
                })),
                s,
            ))
            .child(row(
                "Wardrobe",
                "Buy, equip and take off items.",
                cat_button("settings-shop", "Open the Shop", "shopping-cart-01", false, s)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Shop;
                        cx.notify();
                    })),
                s,
            ))
    }

    fn settings_accounts(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let account: Div = match &self.account {
            Some(a) => div()
                .flex()
                .flex_col()
                .gap(px(4. * s))
                .child(
                    div()
                        .text_size(px(18. * s))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(text(a.account.name.clone())),
                )
                .child(mono(a.account.uuid.clone(), s)),
            None => div().child(quiet("Not signed in")),
        };
        let mode = self.settings.current().map(|p| {
            if p.use_account {
                tr("Microsoft account")
            } else {
                tr("Local player")
            }
        });
        div()
            .flex()
            .flex_col()
            .child(heading("Microsoft", s))
            .child(row(
                "Signed in as",
                "The account that owns Minecraft. Needed for online servers and to share your cosmetics.",
                div()
                    .flex()
                    .items_center()
                    .gap(px(16. * s))
                    .child(account)
                    .child(
                        cat_button(
                            "settings-account",
                            if self.account.is_some() { "Open profile" } else { "Sign in" },
                            "user-circle",
                            self.account.is_none(),
                            s,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.page = Page::Account;
                            cx.notify();
                        })),
                    ),
                s,
            ))
            .child(row(
                "Sign-in storage",
                if cfg!(windows) {
                    "Saved sign-ins are encrypted for your Windows user. Your password never goes through Wisp."
                } else {
                    "Sign-ins last until Wisp closes. Tokens are never saved in plain text."
                },
                icon("shield-check", accent_tone(ACCENT)),
                s,
            ))
            .child(heading("Selected instance", s))
            .child(row(
                "Plays as",
                "Each instance plays with your Microsoft account or as a local player with no sign-in.",
                div()
                    .flex()
                    .items_center()
                    .gap(px(14. * s))
                    .when_some(mode, |d, m| d.child(badge(m, s, true)))
                    .child(
                        cat_button("settings-account-mode", "Switch", "refresh", false, s)
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_account(cx))),
                    ),
                s,
            ))
    }

    fn settings_storage(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let mut list = div().flex().flex_col();
        match &self.storage {
            None => list = list.child(div().py(px(16. * s)).child(quiet("Measuring…"))),
            Some(folders) => {
                for (index, folder) in folders.iter().enumerate() {
                    let path = folder.path.clone();
                    list = list.child(row(
                        folder.label,
                        &folder.path.display().to_string(),
                        div()
                            .flex()
                            .items_center()
                            .gap(px(14. * s))
                            .child(badge(prefs::human_size(folder.size), s, false))
                            .child(
                                cat_button(("open-folder", index), "Open", "folder", false, s)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.show_path(path.clone(), cx)
                                    })),
                            ),
                        s,
                    ));
                }
            }
        }
        div()
            .flex()
            .flex_col()
            .child(heading("Folders", s))
            .child(list)
            .child(heading("Clean up", s))
            .child(row(
                "Cosmetics cache",
                "Items downloaded for other players. They download again when needed.",
                cat_button("clear-cosmetics", "Clear", "close", false, s).on_click(cx.listener(
                    |this, _, _, cx| {
                        let mut cleared = 0;
                        for p in &this.settings.profiles {
                            if std::fs::remove_dir_all(p.game_home.join("assets/cosmetics")).is_ok()
                            {
                                cleared += 1;
                            }
                        }
                        this.failed = false;
                        this.status = trf(
                            "Cleared the cosmetics cache of {n} instances.",
                            &[("n", &cleared.to_string())],
                        );
                        this.measure_storage(cx);
                    },
                )),
                s,
            ))
            .child(row(
                "Measure again",
                "Folder sizes are measured when you open this page.",
                cat_button("measure-storage", "Refresh", "refresh", false, s)
                    .on_click(cx.listener(|this, _, _, cx| this.measure_storage(cx))),
                s,
            ))
    }

    fn settings_phone(&self, cx: &mut Context<Self>) -> Div {
        use wisp_launcher::phone_link::Status;
        let s = self.ui_scale;
        let running = self.phone_link.is_some();
        let toggle = cat_button(
            "phone-link",
            if running { "Stop" } else { "Connect a phone" },
            "compass",
            !running,
            s,
        )
        .on_click(cx.listener(|this, _, _, cx| this.toggle_phone_link(cx)));
        let mut page = div()
            .flex()
            .flex_col()
            .child(heading("Wisp on your phone", s))
            .child(row(
                "Connect a phone",
                "The Wisp app for Android plays with the game files this PC prepared from your Minecraft. Open the app on your phone, choose Get them from my PC, and type the code shown here. Both need the same Wi-Fi; about 800 MB are copied. If Windows asks, allow Wisp on private networks.",
                toggle,
                s,
            ));
        if let Some(link) = &self.phone_link {
            let state = match link.status() {
                Status::Waiting => tr("Waiting for your phone…"),
                Status::Sending { sent, total } => trf(
                    "Copying to your phone: {sent} of {total} MB",
                    &[
                        ("sent", &(sent / 1_000_000).to_string()),
                        ("total", &(total / 1_000_000).to_string()),
                    ],
                ),
                Status::Sent(_) => tr("Done: the game files are on your phone."),
                Status::Closed => tr("Closed. Press Stop, then Connect a phone for a new code."),
            };
            let mut card = div()
                .w_full()
                .mb(px(8. * s))
                .px(px(18. * s))
                .py(px(18. * s))
                .rounded(px(10. * s))
                .border_1()
                .border_color(rgb(tone(0x2a2b30)))
                .bg(rgb(tone(0x17181b)))
                .flex()
                .flex_col()
                .gap(px(8. * s))
                .child(
                    div()
                        .text_size(px(14. * s))
                        .text_color(rgb(MUTED))
                        .child(tr("Code for the phone")),
                )
                .child(
                    div()
                        .text_size(px(44. * s))
                        .font_family("Geist Mono")
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("{} {}", &link.code[..3], &link.code[3..])),
                );
            if let Some(address) = &link.address {
                card = card.child(mono(
                    trf("This PC's address: {address}", &[("address", address)]),
                    s,
                ));
            }
            if !link.discoverable {
                card = card.child(mono(
                    tr("The phone can't find this PC by itself here: type the address in the app."),
                    s,
                ));
            }
            page = page.child(card.child(div().text_size(px(15. * s)).child(state)));
        }
        page
    }

    /// Starts or stops offering the selected instance's game files to a phone.
    fn toggle_phone_link(&mut self, cx: &mut Context<Self>) {
        if self.phone_link.take().is_some() {
            cx.notify();
            return;
        }
        let Some(profile) = self.settings.current() else {
            self.error(tr(
                "Create an instance and press Play once, then connect the phone.",
            ));
            cx.notify();
            return;
        };
        match wisp_launcher::phone_link::PhoneLink::start(&profile.resources, engine::version()) {
            Ok(link) => self.phone_link = Some(link),
            Err(e) => self.error(tr(&e)),
        }
        cx.notify();
    }

    fn settings_about(&self, _cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        div()
            .flex()
            .flex_col()
            .child(heading("Versions", s))
            .child(row(
                "Wisp launcher",
                "The launcher you are using.",
                badge(env!("CARGO_PKG_VERSION"), s, true),
                s,
            ))
            .child(row(
                "Rust engine",
                "The native game bundled with the launcher.",
                badge(trf("Minecraft {v}", &[("v", engine::version())]), s, false),
                s,
            ))
            .child(row(
                "Microsoft sign-in",
                "Whether this build can sign in with Microsoft.",
                badge(
                    tr(if auth::client_id().is_some() {
                        "Configured"
                    } else {
                        "Not configured"
                    }),
                    s,
                    auth::client_id().is_some(),
                ),
                s,
            ))
            .child(heading("Help", s))
            .child(row(
                "Release notes and source",
                "What changed in each version, and the code.",
                cat_button("open-releases", "Open GitHub", "arrow", false, s).on_click(|_, _, cx| {
                    cx.open_url("https://github.com/Lyvt-Dev/Wisp-Client/releases")
                }),
                s,
            ))
            .child(row(
                "Licenses",
                "Wisp is MIT licensed. Original notices ship with the installer. Your worlds stay on this computer.",
                icon("shield-check", accent_tone(ACCENT)),
                s,
            ))
    }
}
