//! The window's frame and Home: the artwork behind the whole window, the top navigation and
//! Home's large instance title, Launch button and instance cards.
use super::*;
use std::time::Instant;
use wisp_launcher::server_ping;

/// The bundled pictures, for a launcher that has no Minecraft files yet.
const CARD_ART: [&str; 3] = ["artwork/aurora.png", "artwork/news.png", "artwork/hero.png"];

/// The folder in the launcher's home whose pictures become the backdrop and the cards.
pub(super) const BACKGROUNDS: &str = "backgrounds";
/// How long one background picture stays before the next.
const BACKGROUND_TURN: Duration = Duration::from_secs(25);

/// Real Minecraft pictures, found in the player's own files (never shipped with Wisp: they
/// are Mojang's): pictures the player put in the backgrounds folder, each instance's newest
/// screenshot, and the title screen's panorama from the prepared game files. Looked up again
/// every few seconds.
#[derive(Default)]
pub(super) struct Art {
    checked: Option<Instant>,
    /// When the backgrounds started turning.
    started: Option<Instant>,
    /// The pictures in the backgrounds folder, by name.
    backgrounds: Vec<PathBuf>,
    /// The newest screenshot of each instance, in the profiles' order.
    screenshots: Vec<Option<PathBuf>>,
    /// The panorama's six sides (0 front, 1 right, 2 back, 3 left, 4 up, 5 down).
    panorama: Option<Vec<PathBuf>>,
}

impl Art {
    pub(super) fn refresh(&mut self, home: &std::path::Path, settings: &Settings) {
        if self
            .checked
            .is_some_and(|t| t.elapsed() < Duration::from_secs(5))
        {
            return;
        }
        self.checked = Some(Instant::now());
        self.started.get_or_insert_with(Instant::now);
        self.backgrounds = pictures(&home.join(BACKGROUNDS));
        self.screenshots = settings
            .profiles
            .iter()
            .map(|p| newest_png(&p.game_home.join("screenshots")))
            .collect();
        let resources = settings
            .current()
            .map(|p| p.resources.clone())
            .unwrap_or_else(|| home.join("resources").join(engine::version()));
        let dir = resources.join("assets/minecraft/textures/gui/title/background");
        let sides: Vec<PathBuf> = (0..6).map(|i| dir.join(format!("panorama_{i}.png"))).collect();
        self.panorama = sides.iter().all(|p| p.is_file()).then_some(sides);
    }

    /// The background picture showing now, if the folder has any: they take turns.
    fn background(&self) -> Option<PathBuf> {
        if self.backgrounds.is_empty() {
            return None;
        }
        let turns = self
            .started
            .map_or(0, |t| (t.elapsed().as_secs() / BACKGROUND_TURN.as_secs()) as usize);
        Some(self.backgrounds[turns % self.backgrounds.len()].clone())
    }

    /// A card's picture: a background picture, else the instance's screenshot, else a
    /// panorama side.
    pub(super) fn card(&self, index: usize) -> Option<gpui::ImageSource> {
        if !self.backgrounds.is_empty() {
            return Some(self.backgrounds[index % self.backgrounds.len()].clone().into());
        }
        if let Some(Some(shot)) = self.screenshots.get(index) {
            return Some(shot.clone().into());
        }
        let sides = self.panorama.as_ref()?;
        Some(sides[[1, 2, 3][index % 3]].clone().into())
    }
}

/// The pictures in `dir` the launcher can show, by name.
fn pictures(dir: &std::path::Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| matches!(x.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp"))
        })
        .collect();
    found.sort();
    found
}

/// The newest `.png` in `dir`.
fn newest_png(dir: &std::path::Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("png")))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// Where the top navigation leads, and which pages light each entry.
const NAV: [(&str, Page, &[Page]); 6] = [
    ("Home", Page::Home, &[Page::Home]),
    ("Instances", Page::Instances, &[Page::Instances, Page::Setup]),
    ("Browse", Page::Mods, &[Page::Mods, Page::Shaders, Page::Packs]),
    ("Servers", Page::Servers, &[Page::Servers]),
    ("Cosmetics", Page::Locker, &[Page::Locker]),
    ("Store", Page::Shop, &[Page::Shop]),
];

impl Launcher {
    /// The artwork behind the whole window: open on Home, a dim band at the top elsewhere.
    pub(super) fn backdrop(&self) -> Div {
        let home = self.page == Page::Home;
        let page = tone(PAGE);
        let shade = |a: u32| rgba((page << 8) | a);
        div()
            .absolute()
            .inset_0()
            .bg(rgb(page))
            .child(self.backdrop_picture().when(!home, |d| d.opacity(0.35)))
            .child(div().absolute().inset_0().bg(if home {
                // Dark under the top bar and at the bottom, where the text and cards sit.
                gpui::linear_gradient(
                    180.,
                    gpui::linear_color_stop(shade(0xb0), 0.),
                    gpui::linear_color_stop(shade(0x30), 0.3),
                )
            } else {
                gpui::linear_gradient(
                    180.,
                    gpui::linear_color_stop(shade(0xa0), 0.),
                    gpui::linear_color_stop(shade(0xff), 0.42),
                )
            }))
            .when(home, |d| {
                d.child(div().absolute().inset_0().bg(gpui::linear_gradient(
                    180.,
                    gpui::linear_color_stop(shade(0x00), 0.35),
                    gpui::linear_color_stop(shade(0xf0), 1.),
                )))
                .child(div().absolute().inset_0().bg(gpui::linear_gradient(
                    90.,
                    gpui::linear_color_stop(shade(0xb8), 0.),
                    gpui::linear_color_stop(shade(0x00), 0.6),
                )))
            })
    }

    /// A picture from the backgrounds folder; else the selected instance's newest screenshot;
    /// else the title screen's panorama, its left and front sides side by side (they meet
    /// seamlessly); else the bundled banner.
    fn backdrop_picture(&self) -> Div {
        let shot = self.art.background().or_else(|| {
            self.settings
                .profiles
                .iter()
                .position(|p| Some(&p.name) == self.settings.selected.as_ref())
                .and_then(|i| self.art.screenshots.get(i).cloned().flatten())
        });
        let layer = div().absolute().inset_0().overflow_hidden();
        if let Some(shot) = shot {
            return layer.child(img(shot).size_full().object_fit(ObjectFit::Cover));
        }
        if let Some(sides) = &self.art.panorama {
            let (w, h) = self.viewport;
            let side = h.max(w / 2.);
            return layer.child(
                div()
                    .absolute()
                    .top(px((h - side) / 2.))
                    .left(px((w - 2. * side) / 2.))
                    .flex()
                    .child(img(sides[3].clone()).size(px(side)))
                    .child(img(sides[0].clone()).size(px(side))),
            );
        }
        layer.child(
            img(self.prefs.banner.asset())
                .size_full()
                .object_fit(ObjectFit::Cover),
        )
    }

    /// The bar over the window: the wordmark, the pages, then coins, friends, news, settings,
    /// the account and the window's buttons.
    pub(super) fn topbar(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let round = |id: &'static str| {
            div()
                .id(id)
                .relative()
                .size(px(40. * s))
                .flex_none()
                .rounded(px(10. * s))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|d| d.bg(rgba(0xffffff14)))
        };
        let control = |id: &'static str| {
            div()
                .id(id)
                .size(px(40. * s))
                .flex_none()
                .rounded(px(10. * s))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
        };
        let drag = |id: &'static str| {
            div()
                .id(id)
                .h_full()
                .flex_1()
                .min_w(px(24. * s))
                .window_control_area(WindowControlArea::Drag)
        };
        let online = self.friends.as_ref().map_or(0, |f| f.online());
        div()
            .h(px(84. * s))
            .flex_none()
            .flex()
            .items_center()
            .px(px(40. * s))
            .gap(px(8. * s))
            .child(
                div()
                    .id("wordmark")
                    .flex()
                    .items_center()
                    .gap(px(12. * s))
                    .cursor_pointer()
                    .child(img("artwork/logo.png").size(px(30. * s)))
                    .child(
                        div()
                            .text_size(px(30. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("WISP"),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Home;
                        cx.notify();
                    })),
            )
            .child(drag("drag-left"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(40. * s))
                    .children(NAV.iter().enumerate().map(|(i, (label, page, lit))| {
                        let page = *page;
                        let active = lit.contains(&self.page);
                        div()
                            .id(("nav", i))
                            .relative()
                            .py(px(10. * s))
                            .text_size(px(19. * s))
                            .text_color(rgb(if active { INK } else { 0x8e9099 }))
                            .when(active, |d| d.font_weight(FontWeight::SEMIBOLD))
                            .cursor_pointer()
                            .hover(|d| d.text_color(rgb(INK)))
                            .child(tr(label))
                            .when(active, |d| {
                                d.child(
                                    div()
                                        .absolute()
                                        .bottom_0()
                                        .left(relative(0.2))
                                        .right(relative(0.2))
                                        .h(px(3. * s))
                                        .rounded_full()
                                        .bg(rgb(INK)),
                                )
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.page = page;
                                this.instance_menu = false;
                                cx.notify();
                            }))
                    })),
            )
            .child(drag("drag-right"))
            .child(
                div()
                    .id("topbar-coins")
                    .h(px(36. * s))
                    .px(px(12. * s))
                    .rounded(px(10. * s))
                    .bg(rgba(0x0b0b0d99))
                    .border_1()
                    .border_color(rgba(0xffffff1a))
                    .flex()
                    .items_center()
                    .gap(px(7. * s))
                    .text_size(px(15. * s))
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .hover(|d| d.bg(rgba(0xffffff14)))
                    .child(icon("coin", 0xf2c14e).size(px(16. * s)))
                    .child(self.wallet.coins.to_string())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Shop;
                        cx.notify();
                    })),
            )
            .child(
                round("topbar-friends")
                    .when(self.prefs.side_panel, |d| d.bg(rgba(0xffffff1f)))
                    .child(icon("user-multiple-02", INK).size(px(23. * s)))
                    .when(online > 0, |d| {
                        d.child(
                            div()
                                .absolute()
                                .top(px(3. * s))
                                .right(px(1. * s))
                                .min_w(px(17. * s))
                                .h(px(17. * s))
                                .px(px(4. * s))
                                .rounded_full()
                                .bg(rgb(0x3ccf6a))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(11. * s))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(0x0b0b0d))
                                .child(online.to_string()),
                        )
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_prefs(|p| p.side_panel = !p.side_panel, cx)
                    })),
            )
            .child(
                round("topbar-news")
                    .child(icon("notification-01", INK).size(px(23. * s)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.is_catalog() {
                            this.catalog.activity = !this.catalog.activity;
                        } else {
                            this.page = Page::About;
                        }
                        cx.notify();
                    })),
            )
            .child(
                round("topbar-settings")
                    .when(matches!(self.page, Page::Settings | Page::About), |d| {
                        d.bg(rgba(0xffffff1f))
                    })
                    .child(icon("gear", INK).size(px(23. * s)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Settings;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("topbar-account")
                    .ml(px(4. * s))
                    .rounded(px(8. * s))
                    .cursor_pointer()
                    .border_2()
                    .border_color(rgba(if self.page == Page::Account {
                        0xffffffcc
                    } else {
                        0x00000000
                    }))
                    .child(self.avatar(40.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Account;
                        cx.notify();
                    })),
            )
            .child(div().w(px(10. * s)))
            .child(
                control("minimize")
                    .hover(|d| d.bg(rgba(0xffffff14)))
                    .on_click(|_, window, cx| {
                        cx.stop_propagation();
                        window.minimize_window();
                    })
                    .child(icon("minus", INK).size(px(20. * s))),
            )
            .child(
                control("maximize")
                    .hover(|d| d.bg(rgba(0xffffff14)))
                    .on_click(|_, window, cx| {
                        cx.stop_propagation();
                        window.zoom_window();
                    })
                    .child(
                        div()
                            .size(px(14. * s))
                            .rounded(px(3. * s))
                            .border_2()
                            .border_color(rgb(INK)),
                    ),
            )
            .child(
                control("close-window")
                    .hover(|d| d.bg(rgb(0xc42b3c)))
                    .on_click(|_, window, cx| {
                        cx.stop_propagation();
                        window.remove_window();
                    })
                    .child(icon("close", INK).size(px(20. * s))),
            )
    }

    /// Home: the selected instance large over the artwork, Launch, and the instances as cards.
    pub(super) fn home(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let profile = self.settings.current();
        let busy = self.launching || self.child.is_some() || self.setup_busy || self.auth_busy;
        let label = if self.child.is_some() {
            "Game running"
        } else if self.launching {
            "Starting…"
        } else if self.setup_busy {
            "Installing…"
        } else if profile.is_none() {
            "Install & play"
        } else {
            "Launch"
        };
        let name = profile.map_or_else(|| tr("No instance yet"), |p| p.name.clone());
        let last_played = self
            .content
            .launched
            .iter()
            .max_by_key(|(_, t)| **t)
            .map(|(_, t)| content::ago(*t));
        let mut subtitle = trf(
            "Minecraft {version} · Wisp",
            &[("version", engine::version())],
        );
        if let Some(when) = last_played {
            subtitle = format!("{subtitle} · {}", trf("Played {when}", &[("when", &when)]));
        }

        let launch = div()
            .id("home-launch")
            .h(px(58. * s))
            .px(px(40. * s))
            .rounded(px(12. * s))
            .bg(rgb(0xffffff))
            .flex()
            .items_center()
            .gap(px(10. * s))
            .text_size(px(20. * s))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0x0b0b0d))
            .child(icon("play", 0x0b0b0d).size(px(18. * s)))
            .child(tr(label))
            .when(busy, |d| d.opacity(0.6))
            .when(!busy, |d| {
                d.cursor_pointer()
                    .hover(|d| d.bg(rgb(0xe4e5e8)))
                    .on_click(cx.listener(|this, _, _, cx| this.play(cx)))
            });
        let side_button = |id: &'static str, glyph: &'static str| {
            div()
                .id(id)
                .size(px(58. * s))
                .rounded(px(12. * s))
                .bg(rgba(0x0b0b0d8c))
                .border_1()
                .border_color(rgba(0xffffff26))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|d| d.bg(rgba(0xffffff1f)))
                .child(icon(glyph, INK).size(px(22. * s)))
        };

        let title = div()
            .flex()
            .flex_col()
            .gap(px(6. * s))
            .child(
                div()
                    .max_w(px(900. * s))
                    .truncate()
                    .text_size(px(76. * s))
                    .line_height(px(86. * s))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(name),
            )
            .child(
                div()
                    .text_size(px(19. * s))
                    .text_color(rgb(0xc9cbd1))
                    .child(subtitle),
            )
            .child(
                div()
                    .mt(px(18. * s))
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(12. * s))
                    .child(launch)
                    .child(side_button("home-instances", "down").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.instance_menu = !this.instance_menu;
                            cx.notify();
                        },
                    )))
                    .child(side_button("home-instance-settings", "settings").on_click(
                        cx.listener(|this, _, _, cx| {
                            this.page = Page::Instances;
                            cx.notify();
                        }),
                    ))
                    .when(self.instance_menu, |d| {
                        d.child(
                            gpui::deferred(self.instance_menu(cx).w(px(360. * s)))
                                .with_priority(1),
                        )
                    }),
            )
            .when_some(self.setup_fraction, |d, fraction| {
                d.child(
                    div()
                        .mt(px(12. * s))
                        .w(px(360. * s))
                        .h(px(5. * s))
                        .rounded_full()
                        .bg(rgba(0xffffff26))
                        .child(
                            div()
                                .h_full()
                                .w(relative(fraction.clamp(0., 1.)))
                                .rounded_full()
                                .bg(rgb(INK)),
                        ),
                )
            })
            .when(!self.status.is_empty(), |d| {
                d.child(
                    div()
                        .mt(px(8. * s))
                        .max_w(px(620. * s))
                        .truncate()
                        .text_size(px(14. * s))
                        .text_color(rgb(if self.failed { 0xf08a96 } else { 0xb4b6bd }))
                        .child(tr(&self.status)),
                )
            });

        div()
            .size_full()
            .flex()
            .flex_col()
            .px(px(64. * s))
            .pb(px(40. * s))
            .child(div().flex_1().min_h(px(20. * s)))
            .child(
                div()
                    .flex()
                    .items_end()
                    .justify_between()
                    .gap(px(24. * s))
                    .child(title)
                    .child(
                        self.featured_compact(cx),
                    ),
            )
            .child(div().h(px(40. * s)).flex_none())
            .child(self.instance_cards(cx))
    }

    /// The instances as picture cards, the selected one outlined, then a tile for all of them.
    fn instance_cards(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let current = self.settings.current().map(|p| p.name.clone());
        let height = px(210. * s);
        let card = |id: ElementId, picture: gpui::ImageSource, label: String, chosen: bool| {
            div()
                .id(id)
                .relative()
                .flex_1()
                .min_w_0()
                .max_w(px(540. * s))
                .h(height)
                .rounded(px(16. * s))
                .overflow_hidden()
                .border_2()
                .border_color(rgba(if chosen { 0xffffffe6 } else { 0xffffff14 }))
                .bg(rgb(tone(SURFACE)))
                .cursor_pointer()
                .hover(move |d| {
                    d.border_color(rgba(if chosen { 0xffffffe6 } else { 0xffffff59 }))
                })
                .child(
                    img(picture)
                        .absolute()
                        .inset_0()
                        .size_full()
                        .object_fit(ObjectFit::Cover),
                )
                .child(div().absolute().inset_0().bg(gpui::linear_gradient(
                    180.,
                    gpui::linear_color_stop(rgba(0x0b0b0d00), 0.35),
                    gpui::linear_color_stop(rgba(0x0b0b0de0), 1.),
                )))
                .child(
                    div()
                        .absolute()
                        .left(px(22. * s))
                        .right(px(22. * s))
                        .bottom(px(18. * s))
                        .truncate()
                        .text_size(px(30. * s))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(label),
                )
        };
        let art = |i: usize| -> gpui::ImageSource {
            self.art
                .card(i)
                .unwrap_or_else(|| CARD_ART[i % CARD_ART.len()].into())
        };
        let mut row = div().flex().gap(px(20. * s));
        for (i, p) in self.settings.profiles.iter().take(3).enumerate() {
            let chosen = current.as_ref() == Some(&p.name);
            row = row.child(
                card(("home-card", i).into(), art(i), p.name.clone(), chosen).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.instance_menu = false;
                        this.select(i, cx);
                    }),
                ),
            );
        }
        if self.settings.profiles.is_empty() {
            row = row.child(
                card("home-card-new".into(), art(0), tr("New instance"), false).on_click(
                    cx.listener(|this, _, _, cx| this.new_instance(cx)),
                ),
            );
        }
        row.child(
            div()
                .id("home-all-instances")
                .w(px(130. * s))
                .h(height)
                .flex_none()
                .rounded(px(16. * s))
                .bg(rgba(0x2a2b30cc))
                .border_1()
                .border_color(rgba(0xffffff14))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(10. * s))
                .cursor_pointer()
                .hover(|d| d.bg(rgba(0x3a3b42cc)))
                .child(icon("dashboard-square-01", INK).size(px(40. * s)))
                .child(
                    div()
                        .text_size(px(13. * s))
                        .text_color(rgb(0xc9cbd1))
                        .child(tr("All")),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.page = Page::Instances;
                    cx.notify();
                })),
        )
    }

    /// The featured server as a small glass card beside Home's title.
    fn featured_compact(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = self.ui_scale;
        let busy = self.launching || self.child.is_some() || self.setup_busy || self.auth_busy;
        let icon_from_list = self
            .content
            .servers
            .iter()
            .find(|sv| server_ping::is_featured(&sv.item.address))
            .and_then(|sv| sv.icon.clone());
        let (dot, players) = match self.pings.get(server_ping::FEATURED_ADDRESS) {
            Some(Some(Ok(st))) => (0x3ccf6a, format!("{} / {}", st.online, st.max)),
            Some(Some(Err(_))) => (0xd9534f, tr("Offline")),
            _ => (0x9a9ca4, tr("Checking…")),
        };
        div()
            .id("home-featured")
            .w(px(420. * s))
            .flex_none()
            .p(px(14. * s))
            .rounded(px(16. * s))
            .bg(rgba(0x0b0b0dc4))
            .border_1()
            .border_color(rgba(0xffffff1f))
            .flex()
            .items_center()
            .gap(px(14. * s))
            .child(
                div()
                    .size(px(60. * s))
                    .flex_none()
                    .rounded(px(12. * s))
                    .overflow_hidden()
                    .map(|d| match icon_from_list {
                        Some(icon) => d.child(img(icon).size_full()),
                        None => d.child(
                            img("artwork/zedix.png")
                                .size_full()
                                .object_fit(ObjectFit::Cover),
                        ),
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(3. * s))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6. * s))
                            .text_size(px(11. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xf2c14e))
                            .child(icon("star", 0xf2c14e).size(px(11. * s)))
                            .child(tr("FEATURED SERVER")),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(20. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(server_ping::FEATURED_NAME),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6. * s))
                            .text_size(px(13. * s))
                            .text_color(rgb(0xb4b6bd))
                            .child(div().size(px(7. * s)).rounded_full().bg(rgb(dot)))
                            .child(players),
                    ),
            )
            .child(
                div()
                    .id("home-featured-join")
                    .flex_none()
                    .h(px(42. * s))
                    .px(px(20. * s))
                    .rounded(px(10. * s))
                    .bg(rgba(0xffffff1f))
                    .border_1()
                    .border_color(rgba(0xffffff33))
                    .flex()
                    .items_center()
                    .gap(px(7. * s))
                    .text_size(px(15. * s))
                    .font_weight(FontWeight::SEMIBOLD)
                    .when(busy, |d| d.opacity(0.45))
                    .when(!busy, |d| {
                        d.cursor_pointer()
                            .hover(|d| d.bg(rgba(0xffffff33)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.launch_with(
                                    vec![
                                        "--quickPlayMultiplayer".into(),
                                        server_ping::FEATURED_ADDRESS.into(),
                                    ],
                                    cx,
                                )
                            }))
                    })
                    .child(icon("play", INK).size(px(14. * s)))
                    .child(tr("Join")),
            )
    }

    /// A page's head, as on every page but Home: a way back, the title, one line about it,
    /// and the page's own buttons on the right.
    pub(super) fn page_head(
        &self,
        back: (&str, Page),
        title: &str,
        about: &str,
        actions: Option<Div>,
        cx: &mut Context<Self>,
    ) -> Div {
        let s = self.ui_scale;
        let (back_label, back_page) = back;
        div()
            .flex()
            .flex_col()
            .gap(px(6. * s))
            .child(
                div()
                    .id("page-back")
                    .self_start()
                    .flex()
                    .items_center()
                    .gap(px(8. * s))
                    .text_size(px(16. * s))
                    .text_color(rgb(0xd9dadd))
                    .cursor_pointer()
                    .hover(|d| d.text_color(rgb(INK)))
                    .child(icon("chevron-left", INK).size(px(16. * s)))
                    .child(tr(back_label))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.page = back_page;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_end()
                    .justify_between()
                    .gap(px(20. * s))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6. * s))
                            .child(
                                div()
                                    .text_size(px(50. * s))
                                    .line_height(px(60. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(tr(title)),
                            )
                            .child(
                                div()
                                    .text_size(px(17. * s))
                                    .text_color(rgb(0xa9abb3))
                                    .child(tr(about)),
                            ),
                    )
                    .when_some(actions, |d, a| d.child(a)),
            )
    }

    /// A white button with dark type: a page's main action.
    pub(super) fn primary_button(&self, id: &'static str, label: &str, glyph: &'static str) -> Stateful<Div> {
        let s = self.ui_scale;
        div()
            .id(id)
            .h(px(44. * s))
            .px(px(20. * s))
            .rounded(px(10. * s))
            .bg(rgb(0xffffff))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(8. * s))
            .text_size(px(16. * s))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0x0b0b0d))
            .cursor_pointer()
            .hover(|d| d.bg(rgb(0xe4e5e8)))
            .child(icon(glyph, 0x0b0b0d).size(px(16. * s)))
            .child(tr(label))
    }

    /// A quiet button: a dark glass plate with a light rim.
    pub(super) fn quiet_button(&self, id: impl Into<ElementId>, label: &str, glyph: &'static str) -> Stateful<Div> {
        let s = self.ui_scale;
        div()
            .id(id)
            .h(px(44. * s))
            .px(px(18. * s))
            .rounded(px(10. * s))
            .bg(rgba(0x16171acc))
            .border_1()
            .border_color(rgba(0xffffff1f))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(8. * s))
            .text_size(px(16. * s))
            .cursor_pointer()
            .hover(|d| d.bg(rgba(0xffffff1a)))
            .child(icon(glyph, INK).size(px(16. * s)))
            .child(tr(label))
    }
}
