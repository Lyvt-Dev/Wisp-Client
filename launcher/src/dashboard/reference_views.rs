//! The window shell (top bar, icon rail, friends panel) and Home, after the owner's launcher
//! reference. Account, instance, coins and running state come from live data.
use super::*;

/// The soft green halo of primary buttons.
pub(super) fn glow(s: f32) -> Vec<gpui::BoxShadow> {
    vec![gpui::BoxShadow {
        color: rgba(accent_tone_a(0x2fbf5a40)).into(),
        offset: gpui::point(px(0.), px(4. * s)),
        blur_radius: px(18. * s),
        spread_radius: px(0.),
        inset: false,
    }]
}
pub(super) fn surface(s: f32) -> Div {
    div()
        .rounded(px(12. * s))
        .border_1()
        .border_color(rgb(tone(RULE)))
        .bg(rgb(tone(SURFACE)))
}
/// The green of primary actions.
pub(super) fn luminous() -> gpui::Background {
    gpui::linear_gradient(
        180.,
        gpui::linear_color_stop(rgb(accent_tone(0x46d36f)), 0.),
        gpui::linear_color_stop(rgb(accent_tone(0x23a04a)), 1.),
    )
}
fn dot(on: bool, s: f32) -> Div {
    div()
        .size(px(9. * s))
        .flex_none()
        .rounded_full()
        .bg(rgb(if on { accent_tone(0x3ccf6a) } else { 0x6b6d75 }))
}

/// A Home news card: real releases, shaders and shop items.
struct News {
    picture: &'static str,
    title: &'static str,
    detail: &'static str,
    tag: Option<&'static str>,
    page: Page,
}
const NEWS: [News; 6] = [
    News {
        picture: "artwork/news.png",
        title: "Wisp update",
        detail: "Client modules, HUD editor and inventory search",
        tag: Some("New"),
        page: Page::About,
    },
    News {
        picture: "artwork/aurora.png",
        title: "Aurora shaders",
        detail: "Shadows, reflections and volumetric light",
        tag: None,
        page: Page::Shaders,
    },
    News {
        picture: "shop/cosmic_fox.png",
        title: "Cosmic Fox",
        detail: "A glowing shoulder pet in the shop",
        tag: Some("Shop"),
        page: Page::Shop,
    },
    News {
        picture: "shop/founders_cape.png",
        title: "Founders Cape",
        detail: "Claim it free in the shop",
        tag: Some("Free"),
        page: Page::Shop,
    },
    News {
        picture: "artwork/hero.png",
        title: "Client modules",
        detail: "Freelook, waypoints, damage numbers and more",
        tag: None,
        page: Page::Mods,
    },
    News {
        picture: "shop/storm_aura.png",
        title: "Storm Aura",
        detail: "Lightning crackles around you",
        tag: Some("Shop"),
        page: Page::Shop,
    },
];

impl Launcher {
    fn account_name(&self) -> String {
        self.account
            .as_ref()
            .map(|a| a.account.name.clone())
            .unwrap_or_else(|| {
                tr(if self.auth_busy {
                    "Loading…"
                } else {
                    "Sign in"
                })
            })
    }

    fn avatar(&self, size: f32) -> Div {
        let s = self.ui_scale;
        div()
            .size(px(size * s))
            .flex_none()
            .rounded(px(4. * s))
            .overflow_hidden()
            .bg(rgb(tone(RAISED)))
            .flex()
            .items_center()
            .justify_center()
            .when_some(self.avatar.clone(), |d, avatar| {
                d.child(img(avatar).size_full())
            })
            .when(self.avatar.is_none(), |d| {
                d.child(icon("user-circle", MUTED).size(px(size * 0.8 * s)))
            })
    }

    /// The thin bar over the whole window: brand, live state, coins, account, window buttons.
    pub(super) fn titlebar(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let running = usize::from(self.child.is_some());
        let state = if self.child.is_some() {
            tr("Playing")
        } else if self.launching {
            tr("Starting…")
        } else if self.setup_busy {
            tr("Installing…")
        } else {
            tr("Ready")
        };
        let pill = |id: &'static str| {
            div()
                .id(id)
                .h(px(28. * s))
                .px(px(10. * s))
                .rounded(px(6. * s))
                .bg(rgb(tone(0x16171a)))
                .border_1()
                .border_color(rgb(tone(0x222328)))
                .flex()
                .items_center()
                .gap(px(7. * s))
                .text_size(px(14. * s))
                .cursor_pointer()
                .hover(|d| d.bg(rgb(tone(RAISED))))
        };
        let control = |id: &'static str| {
            div()
                .id(id)
                .w(px(46. * s))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
        };
        div()
            .h(px(42. * s))
            .flex_none()
            .flex()
            .items_center()
            .bg(rgb(tone(FRAME)))
            .child(
                div()
                    .pl(px(16. * s))
                    .flex()
                    .items_center()
                    .gap(px(14. * s))
                    .text_size(px(14. * s))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8. * s))
                            .child(img("artwork/logo.png").size(px(20. * s)))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(tr("Wisp Client")),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(7. * s))
                            .child(dot(self.child.is_none() && !self.failed, s))
                            .child(state),
                    )
                    .child(div().text_color(rgb(MUTED)).child(trf(
                        "Minecraft {version}",
                        &[("version", engine::version())],
                    ))),
            )
            .child(
                div()
                    .id("titlebar-drag")
                    .h_full()
                    .flex_1()
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8. * s))
                    .pr(px(8. * s))
                    .when(self.is_catalog(), |d| {
                        d.child(
                            pill("catalog-activity")
                                .child(icon("notification-01", MUTED).size(px(15. * s)))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.catalog.activity = !this.catalog.activity;
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        pill("titlebar-coins")
                            .child(icon("coin", 0xf2c14e).size(px(15. * s)))
                            .child(self.wallet.coins.to_string())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.page = Page::Shop;
                                cx.notify();
                            })),
                    )
                    .child(
                        pill("titlebar-account")
                            .child(self.avatar(18.))
                            .child(
                                div()
                                    .max_w(px(160. * s))
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.account_name()),
                            )
                            .child(dot(self.account.is_some(), s))
                            .child(icon("down", MUTED).size(px(13. * s)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.page = Page::Account;
                                cx.notify();
                            })),
                    )
                    .child(
                        pill("titlebar-running")
                            .text_color(rgb(MUTED))
                            .child(trf(
                                if running == 1 {
                                    "{count} instance running"
                                } else {
                                    "{count} instances running"
                                },
                                &[("count", &running.to_string())],
                            ))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.page = Page::Instances;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                control("minimize")
                    .hover(|d| d.bg(rgb(tone(RAISED))))
                    .on_click(|_, window, cx| {
                        cx.stop_propagation();
                        window.minimize_window();
                    })
                    .child(icon("minus", MUTED).size(px(16. * s))),
            )
            .child(
                control("maximize")
                    .hover(|d| d.bg(rgb(tone(RAISED))))
                    .on_click(|_, window, cx| {
                        cx.stop_propagation();
                        window.zoom_window();
                    })
                    .child(
                        div()
                            .size(px(11. * s))
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(MUTED)),
                    ),
            )
            .child(
                control("close-window")
                    .hover(|d| d.bg(rgb(0xc42b3c)))
                    .on_click(|_, window, cx| {
                        cx.stop_propagation();
                        window.remove_window();
                    })
                    .child(icon("close", MUTED).size(px(16. * s))),
            )
    }

    fn rail_button(
        &self,
        id: usize,
        page: Page,
        glyph: &'static str,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let s = self.ui_scale;
        let active = self.page == page || (page == Page::Settings && self.page == Page::About);
        div()
            .id(("navigation", id))
            .relative()
            .w_full()
            .h(px(52. * s))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(
                div()
                    .size(px(42. * s))
                    .rounded(px(10. * s))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(active, |d| d.bg(rgb(tone(0x1f2024))))
                    .hover(|d| d.bg(rgb(tone(0x1a1b1f))))
                    .child(icon(glyph, if active { INK } else { 0x8b8d95 }).size(px(22. * s))),
            )
            .when(active, |d| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(14. * s))
                        .w(px(3. * s))
                        .h(px(24. * s))
                        .rounded_r(px(3. * s))
                        .bg(rgb(accent_tone(ACCENT))),
                )
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.page = page;
                this.instance_menu = false;
                cx.notify();
            }))
    }

    /// The narrow icon rail on the left.
    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let nav = [
            (Page::Home, "rocket"),
            (Page::Instances, "game-controller-01"),
            (Page::Mods, "puzzle"),
            (Page::Shaders, "sun"),
            (Page::Packs, "layers"),
            (Page::Servers, "server"),
            (Page::Locker, "cape"),
            (Page::Shop, "cart"),
        ];
        div()
            .w(px(64. * s))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .py(px(6. * s))
            .bg(rgb(tone(FRAME)))
            .children(
                nav.into_iter()
                    .enumerate()
                    .map(|(i, (page, glyph))| self.rail_button(i, page, glyph, cx)),
            )
            .child(div().flex_1())
            .child(self.rail_button(90, Page::Account, "user", cx))
            .child(self.rail_button(91, Page::Settings, "gear", cx))
    }

    /// The friends column on the right. The launcher has no friends service: it shows the
    /// player and how to play together through invite codes.
    pub(super) fn friends_panel(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let presence = if self.child.is_some() {
            tr("Playing Wisp")
        } else if self.account.is_some() {
            tr("Online in launcher")
        } else {
            tr("Not signed in")
        };
        div()
            .w(px(300. * s))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(rgb(tone(RULE)))
            .bg(rgb(tone(0x141518)))
            .child(
                div()
                    .h(px(52. * s))
                    .flex_none()
                    .px(px(18. * s))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(6. * s))
                            .child(
                                div()
                                    .text_size(px(17. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(tr("Friends")),
                            )
                            .child(
                                div()
                                    .text_size(px(14. * s))
                                    .text_color(rgb(MUTED))
                                    .child(trf(
                                        "({count} online)",
                                        &[(
                                            "count",
                                            &self
                                                .friends
                                                .as_ref()
                                                .map_or(0, |f| f.online())
                                                .to_string(),
                                        )],
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .id("friends-servers")
                            .text_size(px(13. * s))
                            .text_color(rgb(MUTED))
                            .cursor_pointer()
                            .hover(|d| d.text_color(rgb(INK)))
                            .child(tr("Servers"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.page = Page::Servers;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("friends-self")
                    .mx(px(10. * s))
                    .px(px(8. * s))
                    .py(px(7. * s))
                    .rounded(px(8. * s))
                    .flex()
                    .items_center()
                    .gap(px(10. * s))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(tone(0x1c1d21))))
                    .child(
                        div().relative().child(self.avatar(34.)).child(
                            div()
                                .absolute()
                                .right(px(-3. * s))
                                .bottom(px(-3. * s))
                                .p(px(2. * s))
                                .rounded_full()
                                .bg(rgb(tone(0x141518)))
                                .child(dot(self.account.is_some(), s)),
                        ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(15. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.account_name()),
                            )
                            .child(
                                div()
                                    .text_size(px(13. * s))
                                    .text_color(rgb(if self.account.is_some() {
                                        accent_tone(0x3ccf6a)
                                    } else {
                                        MUTED
                                    }))
                                    .child(presence),
                            ),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Account;
                        cx.notify();
                    })),
            )
            .child(self.side_sections(cx))
    }

    pub(super) fn home(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        div()
            .flex()
            .flex_col()
            .pb(px(20. * s))
            .child(
                div()
                    .h(px(50. * s))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(8. * s))
                    .text_size(px(17. * s))
                    .child(div().text_color(rgb(MUTED)).child(tr("Welcome back,")))
                    .child(self.avatar(22.))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.account_name()),
                    )
                    .child(dot(self.account.is_some(), s))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(13. * s))
                            .text_color(rgb(MUTED))
                            .child(trf(
                                "Wisp {wisp} · Minecraft {minecraft}",
                                &[
                                    ("wisp", env!("CARGO_PKG_VERSION")),
                                    ("minecraft", engine::version()),
                                ],
                            )),
                    ),
            )
            .child(self.hero(cx))
            .child(div().mt(px(14. * s)).child(self.featured_server(cx)))
            .child(
                div()
                    .mt(px(14. * s))
                    .flex()
                    .items_start()
                    .gap(px(14. * s))
                    .child(self.home_worlds(cx))
                    .child(self.home_shortcuts(cx)),
            )
            .child(
                div()
                    .h(px(52. * s))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(8. * s))
                            .child(
                                div()
                                    .text_size(px(20. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(tr("Latest News")),
                            )
                            .child(
                                div()
                                    .text_size(px(15. * s))
                                    .text_color(rgb(MUTED))
                                    .child(trf(
                                        "({count} posts)",
                                        &[("count", &NEWS.len().to_string())],
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .id("all-updates")
                            .flex()
                            .items_center()
                            .gap(px(6. * s))
                            .text_size(px(14. * s))
                            .text_color(rgb(MUTED))
                            .cursor_pointer()
                            .hover(|d| d.text_color(rgb(INK)))
                            .child(tr("Explore Wisp"))
                            .child(icon("arrow", MUTED).size(px(14. * s)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.page = Page::About;
                                cx.notify();
                            })),
                    ),
            )
            .child(div().flex().flex_col().gap(px(14. * s)).children(
                NEWS.chunks(3).enumerate().map(|(row, cards)| {
                    div().flex().gap(px(14. * s)).children(
                        cards
                            .iter()
                            .enumerate()
                            .map(|(i, news)| self.news_card(row * 3 + i, news, cx)),
                    )
                }),
            ))
    }

    fn news_card(&self, index: usize, news: &News, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = self.ui_scale;
        let page = news.page;
        let title = if news.page == Page::About {
            trf("Wisp {version}", &[("version", env!("CARGO_PKG_VERSION"))])
        } else {
            tr(news.title)
        };
        div()
            .id(("news", index))
            .relative()
            .flex_1()
            .min_w_0()
            .h(px(184. * s))
            .rounded(px(12. * s))
            .overflow_hidden()
            .border_1()
            .border_color(rgb(tone(RULE)))
            .bg(rgb(tone(SURFACE)))
            .cursor_pointer()
            .hover(|d| d.border_color(rgb(tone(0x4a4c54))))
            .child(
                img(news.picture)
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
            .child(div().absolute().inset_0().bg(gpui::linear_gradient(
                180.,
                gpui::linear_color_stop(rgba(tone_a(0x0b0b0d00)), 0.35),
                gpui::linear_color_stop(rgba(tone_a(0x0b0b0de8)), 1.),
            )))
            .when_some(news.tag, |d, tag| {
                d.child(
                    div()
                        .absolute()
                        .top(px(10. * s))
                        .right(px(10. * s))
                        .px(px(9. * s))
                        .py(px(3. * s))
                        .rounded(px(6. * s))
                        .bg(rgb(0x7c4dff))
                        .text_size(px(12. * s))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tr(tag)),
                )
            })
            .child(
                div()
                    .absolute()
                    .left(px(14. * s))
                    .right(px(52. * s))
                    .bottom(px(12. * s))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(19. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(13. * s))
                            .text_color(rgb(0xc4c6cc))
                            .child(tr(news.detail)),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .right(px(12. * s))
                    .bottom(px(14. * s))
                    .size(px(26. * s))
                    .rounded_full()
                    .bg(rgb(INK))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon("arrow", tone(0x111215)).size(px(15. * s))),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.page = page;
                cx.notify();
            }))
    }

    fn home_card(&self, title: &str, link: Option<(&'static str, &str)>) -> Div {
        let s = self.ui_scale;
        div()
            .p(px(16. * s))
            .rounded(px(14. * s))
            .border_1()
            .border_color(rgb(tone(RULE)))
            .bg(rgb(tone(SURFACE)))
            .flex()
            .flex_col()
            .gap(px(10. * s))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(18. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr(title)),
                    )
                    .when_some(link, |d, (_, label)| {
                        d.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(5. * s))
                                .text_size(px(13. * s))
                                .text_color(rgb(MUTED))
                                .child(tr(label))
                                .child(icon("arrow", MUTED).size(px(13. * s))),
                        )
                    }),
            )
    }

    /// The latest worlds, each opening straight into the game.
    fn home_worlds(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let busy = self.launching || self.child.is_some() || self.setup_busy || self.auth_busy;
        let mut card = self.home_card("Jump back in", None).flex_1().min_w_0();
        if self.content.worlds.is_empty() {
            return card.child(
                div()
                    .flex_1()
                    .py(px(24. * s))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(8. * s))
                    .child(icon("earth", tone(0x5d6068)).size(px(32. * s)))
                    .child(quiet(
                        "No worlds yet. Launch the game and create one; it shows up here.",
                    )),
            );
        }
        for (i, world) in self.content.worlds.iter().take(3).enumerate() {
            let folder = world.item.folder.clone();
            card =
                card.child(
                    div()
                        .id(("world", i))
                        .p(px(8. * s))
                        .rounded(px(10. * s))
                        .flex()
                        .items_center()
                        .gap(px(12. * s))
                        .hover(|d| d.bg(rgb(tone(0x1f2024))))
                        .when(!busy, |d| {
                            d.cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.launch_with(
                                        vec!["--quickPlaySingleplayer".into(), folder.clone()],
                                        cx,
                                    )
                                }))
                        })
                        .child(content::thumb(world.icon.as_ref(), "earth", 44., s))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(15. * s))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(world.item.folder.clone()),
                                )
                                .child(div().text_size(px(12. * s)).text_color(rgb(MUTED)).child(
                                    trf(
                                        "Played {when}",
                                        &[("when", &content::ago(world.item.played))],
                                    ),
                                )),
                        )
                        .child(
                            div()
                                .size(px(30. * s))
                                .flex_none()
                                .rounded_full()
                                .bg(luminous())
                                .flex()
                                .items_center()
                                .justify_center()
                                .when(busy, |d| d.opacity(0.4))
                                .child(icon("play", INK).size(px(13. * s))),
                        ),
                );
        }
        card.child(
            div()
                .id("worlds-folder")
                .mt(px(2. * s))
                .flex()
                .items_center()
                .gap(px(6. * s))
                .text_size(px(13. * s))
                .text_color(rgb(MUTED))
                .cursor_pointer()
                .hover(|d| d.text_color(rgb(INK)))
                .child(icon("folder", MUTED).size(px(14. * s)))
                .child(trf(
                    "All {count} worlds in the saves folder",
                    &[("count", &self.content.world_count.to_string())],
                ))
                .on_click(cx.listener(|this, _, _, cx| this.open_game_folder("saves", cx))),
        )
    }

    /// Six shortcut tiles with what each page holds.
    fn home_shortcuts(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let tile = |id: &'static str, label: &str, detail: String, glyph: &'static str| {
            div()
                .id(id)
                .flex_1()
                .min_w_0()
                .p(px(12. * s))
                .rounded(px(10. * s))
                .bg(rgb(tone(0x1c1d21)))
                .border_1()
                .border_color(rgb(tone(0x26272c)))
                .flex()
                .items_center()
                .gap(px(10. * s))
                .cursor_pointer()
                .hover(|d| d.bg(rgb(tone(0x232429))).border_color(rgb(tone(0x3a3c43))))
                .child(
                    div()
                        .size(px(34. * s))
                        .flex_none()
                        .rounded(px(9. * s))
                        .bg(rgba(accent_tone_a(0x3ccf6a1f)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(glyph, accent_tone(ACCENT)).size(px(17. * s))),
                )
                .child(
                    div()
                        .min_w_0()
                        .child(
                            div()
                                .truncate()
                                .text_size(px(14. * s))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(tr(label)),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(12. * s))
                                .text_color(rgb(MUTED))
                                .child(detail),
                        ),
                )
        };
        let go = |page: Page| {
            cx.listener(
                move |this: &mut Self,
                      _: &gpui::ClickEvent,
                      _: &mut Window,
                      cx: &mut Context<Self>| {
                    this.page = page;
                    cx.notify();
                },
            )
        };
        let count = |n: usize, one: &str, many: &str| {
            if n == 1 {
                tr(one)
            } else {
                trf(many, &[("n", &n.to_string())])
            }
        };
        let instances = count(self.settings.profiles.len(), "1 instance", "{n} instances");
        let packs = count(
            self.content.packs.len(),
            "1 resource pack",
            "{n} resource packs",
        );
        let servers = count(
            self.content.servers.len(),
            "1 saved server",
            "{n} saved servers",
        );
        self.home_card("Shortcuts", None)
            .w(px(460. * s))
            .flex_none()
            .child(
                div()
                    .flex()
                    .gap(px(8. * s))
                    .child(
                        tile(
                            "quick-versions",
                            "Versions",
                            instances,
                            "game-controller-01",
                        )
                        .on_click(go(Page::Instances)),
                    )
                    .child(
                        tile("quick-mods", "Mods", tr("Client features"), "puzzle")
                            .on_click(go(Page::Mods)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(8. * s))
                    .child(
                        tile("quick-shaders", "Shaders", tr("Light and shadows"), "sun")
                            .on_click(go(Page::Shaders)),
                    )
                    .child(tile("quick-packs", "Packs", packs, "layers").on_click(go(Page::Packs))),
            )
            .child(
                div()
                    .flex()
                    .gap(px(8. * s))
                    .child(
                        tile("quick-servers", "Servers", servers, "server")
                            .on_click(go(Page::Servers)),
                    )
                    .child(
                        tile("quick-logs", "Logs", tr("The last game's log"), "terminal")
                            .on_click(cx.listener(|this, _, _, cx| this.open_log(cx))),
                    ),
            )
    }

    /// The artwork banner with the Launch button in its centre.
    pub(super) fn hero(&self, cx: &mut Context<Self>) -> Div {
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
            "Launch game"
        };
        let detail = match profile {
            Some(p) => trf(
                "{instance} · Minecraft {version}",
                &[("instance", &p.name), ("version", engine::version())],
            ),
            None => trf("Minecraft {version}", &[("version", engine::version())]),
        };
        let launch = div()
            .relative()
            .flex()
            .h(px(78. * s))
            .rounded(px(10. * s))
            .border_1()
            .border_color(rgb(accent_tone(0x6fe792)))
            .bg(luminous())
            .shadow(glow(s))
            .when(busy, |d| d.opacity(0.65))
            .child(
                div()
                    .id("hero-play")
                    .w(px(300. * s))
                    .h_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(3. * s))
                    .rounded_l(px(10. * s))
                    .when(!busy, |d| {
                        d.cursor_pointer()
                            .hover(|d| d.bg(rgba(0xffffff14)))
                            .on_click(cx.listener(|this, _, _, cx| this.play(cx)))
                    })
                    .child(
                        div()
                            .text_size(px(27. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr(label).to_uppercase()),
                    )
                    .child(
                        div()
                            .max_w(px(270. * s))
                            .truncate()
                            .text_size(px(14. * s))
                            .text_color(rgb(accent_tone(0xe3f8e9)))
                            .child(detail),
                    ),
            )
            .child(div().w(px(1.)).my(px(14. * s)).bg(rgba(0xffffff40)))
            .child(
                div()
                    .id("instance-selector")
                    .w(px(46. * s))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_r(px(10. * s))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgba(0xffffff14)))
                    .child(icon("down", INK).size(px(18. * s)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.instance_menu = !this.instance_menu;
                        cx.notify();
                    })),
            )
            .when(self.instance_menu, |d| {
                d.child(gpui::deferred(self.instance_menu(cx)).with_priority(1))
            });
        let last_played = self
            .content
            .launched
            .iter()
            .max_by_key(|(_, t)| **t)
            .map(|(name, t)| (name.clone(), content::ago(*t)));
        div()
            .relative()
            .h(px(330. * s))
            .flex_none()
            .rounded(px(14. * s))
            .border_1()
            .border_color(rgb(tone(RULE)))
            .child(
                img(self.prefs.banner.asset())
                    .absolute()
                    .inset_0()
                    .size_full()
                    .rounded(px(14. * s))
                    .object_fit(ObjectFit::Cover),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(14. * s))
                    .bg(gpui::linear_gradient(
                        180.,
                        gpui::linear_color_stop(rgba(tone_a(0x0b0b0d40)), 0.),
                        gpui::linear_color_stop(rgba(tone_a(0x0b0b0dc0)), 1.),
                    )),
            )
            .when_some(last_played, |d, (name, when)| {
                d.child(
                    div()
                        .absolute()
                        .left(px(14. * s))
                        .top(px(14. * s))
                        .px(px(10. * s))
                        .py(px(6. * s))
                        .rounded(px(8. * s))
                        .bg(rgba(tone_a(0x0b0b0dc0)))
                        .flex()
                        .items_center()
                        .gap(px(8. * s))
                        .text_size(px(13. * s))
                        .child(icon("clock", MUTED).size(px(14. * s)))
                        .child(div().text_color(rgb(MUTED)).child(tr("Last played")))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(name))
                        .child(div().text_color(rgb(MUTED)).child(when)),
                )
            })
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(12. * s))
                    .child(launch)
                    .when_some(self.setup_fraction, |d, fraction| {
                        d.child(
                            div()
                                .w(px(346. * s))
                                .h(px(5. * s))
                                .rounded_full()
                                .bg(rgba(0xffffff26))
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(fraction.clamp(0., 1.)))
                                        .rounded_full()
                                        .bg(rgb(accent_tone(ACCENT))),
                                ),
                        )
                    })
                    .child(
                        div()
                            .max_w(px(560. * s))
                            .px(px(12. * s))
                            .py(px(5. * s))
                            .rounded(px(6. * s))
                            .bg(rgba(tone_a(0x0b0b0da0)))
                            .truncate()
                            .text_size(px(14. * s))
                            .text_color(rgb(if self.failed { 0xf08a96 } else { 0xc4c6cc }))
                            .child(tr(&self.status)),
                    ),
            )
            .child(
                div()
                    .id("hero-settings")
                    .absolute()
                    .right(px(14. * s))
                    .bottom(px(14. * s))
                    .size(px(34. * s))
                    .rounded_full()
                    .bg(rgba(tone_a(0x0b0b0dc0)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(tone(RAISED))))
                    .child(icon("gear", INK).size(px(18. * s)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Settings;
                        cx.notify();
                    })),
            )
    }

    /// The instance list that opens under the Launch button.
    fn instance_menu(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let current = self.settings.current().map(|p| p.name.clone());
        let row = |id: ElementId, label: String, chosen: bool| {
            div()
                .id(id)
                .px(px(12. * s))
                .py(px(9. * s))
                .rounded(px(6. * s))
                .flex()
                .items_center()
                .gap(px(10. * s))
                .text_size(px(15. * s))
                .cursor_pointer()
                .hover(|d| d.bg(rgb(tone(RAISED))))
                .child(dot(chosen, s))
                .child(div().truncate().child(label))
        };
        div()
            .absolute()
            .top(px(84. * s))
            .left_0()
            .right_0()
            .p(px(6. * s))
            .rounded(px(10. * s))
            .border_1()
            .border_color(rgb(tone(RULE)))
            .bg(rgb(tone(0x1b1c20)))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap(px(2. * s))
            .children(self.settings.profiles.iter().enumerate().map(|(i, p)| {
                row(
                    ("instance-option", i).into(),
                    p.name.clone(),
                    current.as_ref() == Some(&p.name),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.instance_menu = false;
                    this.select(i, cx);
                }))
            }))
            .child(
                row("instance-menu-new".into(), tr("New instance"), false).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.instance_menu = false;
                        this.new_instance(cx);
                    },
                )),
            )
    }
}
