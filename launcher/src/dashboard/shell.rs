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
        let sides: Vec<PathBuf> = (0..6)
            .map(|i| dir.join(format!("panorama_{i}.png")))
            .collect();
        self.panorama = sides.iter().all(|p| p.is_file()).then_some(sides);
    }

    /// The background picture showing now, if the folder has any: they take turns.
    fn background(&self) -> Option<PathBuf> {
        if self.backgrounds.is_empty() {
            return None;
        }
        let turns = self.started.map_or(0, |t| {
            (t.elapsed().as_secs() / BACKGROUND_TURN.as_secs()) as usize
        });
        Some(self.backgrounds[turns % self.backgrounds.len()].clone())
    }

    /// A card's picture: a background picture, else the instance's screenshot, else a
    /// panorama side.
    pub(super) fn card(&self, index: usize) -> Option<gpui::ImageSource> {
        if !self.backgrounds.is_empty() {
            return Some(
                self.backgrounds[index % self.backgrounds.len()]
                    .clone()
                    .into(),
            );
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
            p.extension().and_then(|x| x.to_str()).is_some_and(|x| {
                matches!(
                    x.to_ascii_lowercase().as_str(),
                    "png" | "jpg" | "jpeg" | "webp"
                )
            })
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
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("png"))
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// Where the top navigation leads, and which pages light each entry.
const NAV: [(&str, Page, &[Page]); 6] = [
    ("Home", Page::Home, &[Page::Home]),
    (
        "Instances",
        Page::Instances,
        &[Page::Instances, Page::Setup],
    ),
    (
        "Browse",
        Page::Mods,
        &[Page::Mods, Page::Shaders, Page::Packs],
    ),
    ("Servers", Page::Servers, &[Page::Servers]),
    ("Cosmetics", Page::Locker, &[Page::Locker]),
    ("Store", Page::Shop, &[Page::Shop]),
];

impl Launcher {
    /// Home and the Store lay themselves out over the whole picture, without the pages' margins.
    pub(super) fn full_bleed(&self) -> bool {
        matches!(self.page, Page::Home | Page::Shop)
    }

    /// The artwork behind the whole window: open on Home and the Store, a dim band at the top
    /// elsewhere.
    pub(super) fn backdrop(&self) -> Div {
        let home = self.full_bleed();
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
        let layer = div().absolute().inset_0().overflow_hidden();
        // The Store: the chosen item's own picture.
        if let Some(offer) = self.shop_selected().filter(|_| self.page == Page::Shop) {
            let banner = format!("banner_{}.png", offer.id);
            let picture = if wisp_launcher::shop::picture(&banner).is_some() {
                format!("shop/{banner}")
            } else {
                format!("shop/{}.png", offer.id)
            };
            return layer.child(img(picture).size_full().object_fit(ObjectFit::Cover));
        }
        let shot = self.art.background().or_else(|| {
            self.settings
                .profiles
                .iter()
                .position(|p| Some(&p.name) == self.settings.selected.as_ref())
                .and_then(|i| self.art.screenshots.get(i).cloned().flatten())
        });
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
                    .gap(px(10. * s))
                    .cursor_pointer()
                    .child(img("artwork/logo.png").size(px(32. * s)))
                    .child(
                        div()
                            .text_size(px(28. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(wisp_theme::brand::NAME),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Home;
                        cx.notify();
                    })),
            )
            .child(drag("drag-left"))
            .child(
                // The pages in a glass capsule; the one shown is a white pill.
                div()
                    .flex()
                    .items_center()
                    .gap(px(2. * s))
                    .p(px(5. * s))
                    .rounded_full()
                    .bg(rgba(0x0b0b0d8c))
                    .border_1()
                    .border_color(rgba(0xffffff1a))
                    .children(NAV.iter().enumerate().map(|(i, (label, page, lit))| {
                        let page = *page;
                        let active = lit.contains(&self.page);
                        div()
                            .id(("nav", i))
                            .h(px(38. * s))
                            .px(px(18. * s))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .text_size(px(16. * s))
                            .cursor_pointer()
                            .when(active, |d| {
                                d.bg(rgb(0xffffff))
                                    .text_color(rgb(0x0b0b0d))
                                    .font_weight(FontWeight::SEMIBOLD)
                            })
                            .when(!active, |d| {
                                d.text_color(rgb(0xb4b6bd))
                                    .hover(|d| d.bg(rgba(0xffffff14)).text_color(rgb(INK)))
                            })
                            .child(tr(label))
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

    /// Home: the selected instance large over the artwork and Launch on
    /// the left, the featured server and friends under them, the instances stacked on the right.
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
        let subtitle = match last_played {
            Some(when) => trf("Played {when}", &[("when", &when)]),
            None => tr("Your worlds stay on this computer."),
        };

        // Launch: a white pill whose play sign sits in a dark circle.
        let launch = div()
            .id("home-launch")
            .h(px(62. * s))
            .pl(px(8. * s))
            .pr(px(34. * s))
            .rounded_full()
            .bg(rgb(0xffffff))
            .flex()
            .items_center()
            .gap(px(14. * s))
            .text_size(px(20. * s))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0x0b0b0d))
            .child(
                div()
                    .size(px(46. * s))
                    .rounded_full()
                    .bg(rgb(0x0b0b0d))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon("play", 0xffffff).size(px(18. * s))),
            )
            .child(tr(label))
            .when(busy, |d| d.opacity(0.6))
            .when(!busy, |d| {
                d.cursor_pointer()
                    .hover(|d| d.bg(rgb(0xe4e5e8)))
                    .on_click(cx.listener(|this, _, _, cx| this.play(cx)))
            });
        let round = |id: &'static str, glyph: &'static str| {
            div()
                .id(id)
                .size(px(52. * s))
                .rounded_full()
                .bg(rgba(0x0b0b0d99))
                .border_1()
                .border_color(rgba(0xffffff26))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|d| d.bg(rgba(0xffffff1f)))
                .child(icon(glyph, INK).size(px(20. * s)))
        };

        let title = div()
            .flex()
            .flex_col()
            .gap(px(10. * s))
            .child(
                div()
                    .max_w(px(820. * s))
                    .truncate()
                    .text_size(px(72. * s))
                    .line_height(px(80. * s))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(name),
            )
            .child(
                div()
                    .text_size(px(18. * s))
                    .text_color(rgb(0xc9cbd1))
                    .child(subtitle),
            )
            .child(
                div()
                    .mt(px(14. * s))
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(10. * s))
                    .child(launch)
                    .child(round("home-instances", "down").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.instance_menu = !this.instance_menu;
                            cx.notify();
                        },
                    )))
                    .child(
                        round("home-instance-settings", "settings").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.page = Page::Instances;
                                cx.notify();
                            },
                        )),
                    )
                    .when(self.instance_menu, |d| {
                        d.child(
                            gpui::deferred(self.instance_menu(cx).w(px(360. * s))).with_priority(1),
                        )
                    }),
            )
            .when_some(self.setup_fraction, |d, fraction| {
                d.child(
                    div()
                        .mt(px(6. * s))
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
            .gap(px(40. * s))
            .px(px(64. * s))
            .pb(px(40. * s))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().flex_1().min_h(px(20. * s)))
                    .child(title)
                    .child(div().h(px(36. * s)).flex_none())
                    .child(
                        div()
                            .flex()
                            .gap(px(14. * s))
                            .child(self.featured_compact(cx))
                            .child(self.friends_strip(cx)),
                    ),
            )
            .child(self.instance_shelf(cx))
    }

    /// Friends online as overlapping heads, opening Social.
    fn friends_strip(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = self.ui_scale;
        let online: Vec<_> = self
            .friends
            .as_ref()
            .map(|l| l.friends.iter().filter(|f| f.online).take(4).collect())
            .unwrap_or_default();
        let (title, line) = if online.is_empty() {
            (tr("Social"), tr("Add friends and join them"))
        } else {
            (
                trf("{count} online", &[("count", &online.len().to_string())]),
                tr("Join your friends"),
            )
        };
        let mut heads = div().flex().items_center();
        for (i, friend) in online.iter().enumerate() {
            heads = heads.child(
                div()
                    .when(i > 0, |d| d.ml(px(-10. * s)))
                    .rounded(px(10. * s))
                    .border_2()
                    .border_color(rgb(0x111215))
                    .child(self.head(&friend.uuid, &friend.name, 36., None)),
            );
        }
        if online.is_empty() {
            heads = heads.child(
                div()
                    .size(px(40. * s))
                    .rounded_full()
                    .bg(rgba(0xffffff14))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon("user-multiple-02", INK).size(px(20. * s))),
            );
        }
        div()
            .id("home-friends")
            .w(px(300. * s))
            .flex_none()
            .p(px(14. * s))
            .rounded(px(16. * s))
            .bg(rgba(0x0b0b0dc4))
            .border_1()
            .border_color(rgba(0xffffff1f))
            .flex()
            .items_center()
            .gap(px(14. * s))
            .cursor_pointer()
            .hover(|d| d.border_color(rgba(0xffffff40)))
            .child(heads)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(17. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(13. * s))
                            .text_color(rgb(0xb4b6bd))
                            .child(line),
                    ),
            )
            .child(icon("arrow", 0x8e9099).size(px(15. * s)))
            .on_click(cx.listener(|this, _, _, cx| this.set_prefs(|p| p.side_panel = true, cx)))
    }

    /// The instances stacked on the right of Home: a picture, the name and when it was played;
    /// the chosen one is outlined. Then the way to all of them.
    fn instance_shelf(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let current = self.settings.current().map(|p| p.name.clone());
        let art = |i: usize| -> gpui::ImageSource {
            self.art
                .card(i)
                .unwrap_or_else(|| CARD_ART[i % CARD_ART.len()].into())
        };
        let row = |id: ElementId,
                   picture: gpui::ImageSource,
                   label: String,
                   line: String,
                   chosen: bool| {
            div()
                .id(id)
                .h(px(96. * s))
                .flex_none()
                .p(px(8. * s))
                .pr(px(16. * s))
                .rounded(px(18. * s))
                .bg(rgba(if chosen { 0x1b1c20f0 } else { 0x0b0b0dc4 }))
                .border_1()
                .border_color(rgba(if chosen { 0xffffffd9 } else { 0xffffff1a }))
                .flex()
                .items_center()
                .gap(px(14. * s))
                .cursor_pointer()
                .hover(move |d| d.border_color(rgba(if chosen { 0xffffffd9 } else { 0xffffff4d })))
                .child(
                    img(picture)
                        .w(px(132. * s))
                        .h_full()
                        .flex_none()
                        .rounded(px(12. * s))
                        .object_fit(ObjectFit::Cover),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .truncate()
                                .text_size(px(20. * s))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(label),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(13. * s))
                                .text_color(rgb(0xa9abb3))
                                .child(line),
                        ),
                )
                .when(chosen, |d| {
                    d.child(
                        div()
                            .size(px(26. * s))
                            .flex_none()
                            .rounded_full()
                            .bg(rgb(0xffffff))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon("tick-02", 0x0b0b0d).size(px(15. * s))),
                    )
                })
        };
        let mut shelf = div()
            .w(px(400. * s))
            .flex_none()
            .flex()
            .flex_col()
            .justify_end()
            .gap(px(10. * s))
            .child(
                div()
                    .px(px(4. * s))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(13. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xb4b6bd))
                            .child(tr("YOUR INSTANCES")),
                    )
                    .child(
                        div()
                            .id("home-all-instances")
                            .flex()
                            .items_center()
                            .gap(px(6. * s))
                            .text_size(px(14. * s))
                            .text_color(rgb(0xd9dadd))
                            .cursor_pointer()
                            .hover(|d| d.text_color(rgb(INK)))
                            .child(trf(
                                "All {count}",
                                &[("count", &self.settings.profiles.len().to_string())],
                            ))
                            .child(icon("arrow", 0xd9dadd).size(px(14. * s)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.page = Page::Instances;
                                cx.notify();
                            })),
                    ),
            );
        for (i, p) in self.settings.profiles.iter().take(4).enumerate() {
            let chosen = current.as_ref() == Some(&p.name);
            let line = self
                .content
                .launched
                .get(&p.name)
                .map(|t| trf("Played {when}", &[("when", &content::ago(*t))]))
                .unwrap_or_else(|| trf("Minecraft {version}", &[("version", engine::version())]));
            shelf = shelf.child(
                row(
                    ("home-card", i).into(),
                    art(i),
                    p.name.clone(),
                    line,
                    chosen,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.instance_menu = false;
                    this.select(i, cx);
                })),
            );
        }
        shelf.child(
            div()
                .id("home-new-instance")
                .h(px(56. * s))
                .flex_none()
                .rounded(px(18. * s))
                .border_1()
                .border_dashed()
                .border_color(rgba(0xffffff33))
                .bg(rgba(0x0b0b0d66))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(8. * s))
                .text_size(px(15. * s))
                .cursor_pointer()
                .hover(|d| d.border_color(rgba(0xffffff80)))
                .child(icon("plus", INK).size(px(16. * s)))
                .child(tr("New instance"))
                .on_click(cx.listener(|this, _, _, cx| this.new_instance(cx))),
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
                // Where this page sits: the page it came from, then this one.
                div()
                    .flex()
                    .items_center()
                    .gap(px(8. * s))
                    .text_size(px(14. * s))
                    .child(
                        div()
                            .id("page-back")
                            .text_color(rgb(0x9a9ca4))
                            .cursor_pointer()
                            .hover(|d| d.text_color(rgb(INK)))
                            .child(tr(back_label.trim_start_matches("Back to ")))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.page = back_page;
                                cx.notify();
                            })),
                    )
                    .child(icon("chevron-right", 0x6b6d75).size(px(13. * s)))
                    .child(div().text_color(rgb(0xd9dadd)).child(tr(title))),
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
    pub(super) fn primary_button(
        &self,
        id: &'static str,
        label: &str,
        glyph: &'static str,
    ) -> Stateful<Div> {
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
    pub(super) fn quiet_button(
        &self,
        id: impl Into<ElementId>,
        label: &str,
        glyph: &'static str,
    ) -> Stateful<Div> {
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
