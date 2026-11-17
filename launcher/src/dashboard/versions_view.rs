//! Instances: every instance as a large picture card (search, type tabs and filters above),
//! and the chosen one in a panel on the right with what it holds, Launch and its folders.
use super::catalog_views::InstanceFilter;
use super::*;
use gpui::AnyElement;
use wisp_launcher::catalog;

/// Instance tiles take these pictures in turn, so neighbours differ.
const ART: [&str; 4] = [
    "artwork/hero.png",
    "artwork/news.png",
    "artwork/aurora.png",
    "shop/banner_nebula_cape.png",
];
const COLUMNS: usize = 2;

fn art_for(name: &str) -> &'static str {
    let sum = name
        .bytes()
        .fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
    ART[sum % ART.len()]
}

impl Launcher {
    fn version_label(&self, p: &Profile) -> String {
        if catalog::is_bundled(p) {
            trf("Minecraft {version}", &[("version", engine::version())])
        } else {
            tr("Imported build")
        }
    }

    fn chip(&self, id: &'static str, label: &str, on: bool, count: usize) -> Stateful<Div> {
        let s = self.ui_scale;
        div()
            .id(id)
            .h(px(34. * s))
            .px(px(12. * s))
            .rounded_full()
            .border_1()
            .border_color(rgb(if on {
                accent_tone(ACCENT)
            } else {
                tone(0x2e3035)
            }))
            .bg(if on {
                rgba(accent_tone_a(0x3ccf6a22))
            } else {
                rgba(tone_a(0x17181bff))
            })
            .flex()
            .items_center()
            .gap(px(7. * s))
            .text_size(px(13. * s))
            .cursor_pointer()
            .hover(|d| d.bg(rgb(tone(0x202125))))
            .when(on, |d| {
                d.child(icon("checkmark-circle-01", accent_tone(ACCENT)).size(px(14. * s)))
            })
            .child(tr(label))
            .child(
                div()
                    .text_size(px(11. * s))
                    .text_color(rgb(MUTED))
                    .child(count.to_string()),
            )
    }


    /// An instance as a picture card: its type in a pill, a star, its name over the picture.
    fn instance_card(
        &self,
        index: usize,
        p: &Profile,
        favorite: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let s = self.ui_scale;
        let selected = self.settings.selected.as_ref() == Some(&p.name);
        let mods = catalog::enabled_mods(p);
        let picture: gpui::ImageSource = self
            .art
            .card(index)
            .unwrap_or_else(|| art_for(&p.name).into());
        let kind = if mods == 0 {
            tr("Vanilla")
        } else if mods == 1 {
            tr("1 mod")
        } else {
            trf("{count} mods", &[("count", &mods.to_string())])
        };
        div()
            .id(("version-card", index))
            .relative()
            .flex_1()
            .min_w_0()
            .h(px(290. * s))
            .rounded(px(16. * s))
            .overflow_hidden()
            .border_2()
            .border_color(rgba(if selected { 0xffffffe6 } else { 0xffffff12 }))
            .bg(rgb(tone(SURFACE)))
            .cursor_pointer()
            .hover(move |d| d.border_color(rgba(if selected { 0xffffffe6 } else { 0xffffff4d })))
            .on_click(cx.listener(move |this, _, _, cx| this.select(index, cx)))
            .child(
                img(picture)
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
            .child(div().absolute().inset_0().bg(gpui::linear_gradient(
                180.,
                gpui::linear_color_stop(rgba(0x0b0b0d10), 0.3),
                gpui::linear_color_stop(rgba(0x0b0b0de6), 1.),
            )))
            .child(
                div()
                    .absolute()
                    .top(px(16. * s))
                    .left(px(16. * s))
                    .px(px(12. * s))
                    .py(px(4. * s))
                    .rounded_full()
                    .bg(rgba(0xffffffe0))
                    .text_size(px(14. * s))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0x0b0b0d))
                    .child(kind),
            )
            .child(
                div()
                    .id(("favorite-instance", index))
                    .absolute()
                    .top(px(14. * s))
                    .right(px(14. * s))
                    .size(px(34. * s))
                    .rounded_full()
                    .bg(rgba(0x0b0b0db0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|d| d.bg(rgba(0x2a2b30e0)))
                    .child(
                        icon("star", if favorite { 0xf1c473 } else { 0xc4c6cc })
                            .size(px(17. * s)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        let name = this.settings.profiles[index].name.clone();
                        if let Err(e) = catalog::Preferences::toggle_favorite(&this.home, &name)
                        {
                            this.error(e);
                        }
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .absolute()
                    .left(px(22. * s))
                    .right(px(22. * s))
                    .bottom(px(18. * s))
                    .truncate()
                    .text_size(px(38. * s))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(p.name.clone()),
            )
    }

    /// The chosen instance: its picture, name, what it holds, Launch and its folders.
    fn instance_panel(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let panel = div()
            .w(px(430. * s))
            .flex_none()
            .p(px(14. * s))
            .rounded(px(18. * s))
            .bg(rgba(0x141518e6))
            .border_1()
            .border_color(rgba(0xffffff14))
            .flex()
            .flex_col()
            .gap(px(14. * s));
        let Some(p) = self.settings.current() else {
            return panel
                .p(px(28. * s))
                .items_center()
                .gap(px(12. * s))
                .child(icon("box", 0x6b6d75).size(px(44. * s)))
                .child(
                    div()
                        .text_size(px(22. * s))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tr("No instance yet")),
                )
                .child(quiet("Create one and Wisp installs Minecraft into it."))
                .child(
                    self.primary_button("versions-first-instance", "New instance", "plus")
                        .on_click(cx.listener(|this, _, _, cx| this.new_instance(cx))),
                );
        };
        let index = self
            .settings
            .profiles
            .iter()
            .position(|q| q.name == p.name)
            .unwrap_or(0);
        let busy = self.launching || self.child.is_some() || self.setup_busy || self.auth_busy;
        let mods = catalog::enabled_mods(p);
        let picture: gpui::ImageSource = self
            .art
            .card(index)
            .unwrap_or_else(|| art_for(&p.name).into());
        let last = self
            .content
            .launched
            .get(&p.name)
            .map(|t| trf("Played {when}", &[("when", &content::ago(*t))]))
            .unwrap_or_else(|| tr("Not played yet"));
        let tag = |label: String| {
            div()
                .px(px(12. * s))
                .py(px(5. * s))
                .rounded_full()
                .bg(rgba(0xffffff12))
                .text_size(px(14. * s))
                .text_color(rgb(0xd9dadd))
                .child(label)
        };
        let fact = |label: &str, value: String| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(16. * s))
                .child(div().text_color(rgb(0xa9abb3)).child(tr(label)))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(value))
        };
        let label = if self.child.is_some() {
            "Game running"
        } else if self.launching {
            "Starting…"
        } else {
            "Launch"
        };
        let game_home = p.game_home.clone();
        panel
            .child(
                img(picture)
                    .w_full()
                    .h(px(210. * s))
                    .rounded(px(12. * s))
                    .object_fit(ObjectFit::Cover),
            )
            .child(
                div()
                    .px(px(8. * s))
                    .flex()
                    .flex_col()
                    .gap(px(12. * s))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(32. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(p.name.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(8. * s))
                            .child(tag(self.version_label(p)))
                            .child(tag(if mods == 0 {
                                tr("Vanilla")
                            } else {
                                trf("{count} mods", &[("count", &mods.to_string())])
                            }))
                            .when(!catalog::locally_available(p), |d| {
                                d.child(tag(tr("Files missing")))
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(15. * s))
                            .text_color(rgb(0xa9abb3))
                            .child(last),
                    )
                    .child(div().h(px(1.)).bg(rgba(0xffffff14)))
                    .child(fact("Version", engine::version().to_string()))
                    .child(fact("Worlds", self.content.world_count.to_string()))
                    .child(fact("Resource packs", self.content.packs.len().to_string()))
                    .child(fact("Servers", self.content.servers.len().to_string())),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .gap(px(10. * s))
                    .child(
                        self.primary_button("instance-launch", label, "play")
                            .flex_1()
                            .h(px(50. * s))
                            .when(busy, |d| d.opacity(0.6))
                            .when(!busy, |d| {
                                d.on_click(cx.listener(|this, _, _, cx| this.play(cx)))
                            }),
                    )
                    .child(
                        self.quiet_button("instance-mods", "Mods", "puzzle")
                            .h(px(50. * s))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.page = Page::Mods;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(10. * s))
                    .child(
                        self.quiet_button("instance-folder", "Folder", "folder")
                            .flex_1()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.show_path(game_home.clone(), cx)
                            })),
                    )
                    .child(
                        self.quiet_button("instance-logs", "Logs", "terminal")
                            .flex_1()
                            .on_click(cx.listener(|this, _, _, cx| this.open_log(cx))),
                    ),
            )
    }

    pub(super) fn versions_catalog(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let query = self.version_search.read(cx).value.clone();
        let preferences = catalog::Preferences::load(&self.home).unwrap_or_default();
        let profiles_all = &self.settings.profiles;
        let count = |f: &dyn Fn(&Profile) -> bool| profiles_all.iter().filter(|p| f(p)).count();
        let total = profiles_all.len();
        let modded = count(&|p| catalog::enabled_mods(p) > 0);
        let imported = count(&|p| !catalog::is_bundled(p));
        let local = count(&|p| catalog::locally_available(p));
        let favorites = count(&|p| preferences.favorites.contains(&p.name));
        let mut profiles: Vec<_> = profiles_all
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                let mods = catalog::enabled_mods(p);
                catalog::matches(&query, &[&p.name, &p.game_home.to_string_lossy()])
                    && match self.catalog.versions_tab {
                        InstanceFilter::All => true,
                        InstanceFilter::Vanilla => mods == 0,
                        InstanceFilter::Modded => mods > 0,
                        InstanceFilter::Imported => !catalog::is_bundled(p),
                    }
                    && (!self.catalog.local_only || catalog::locally_available(p))
                    && (!self.catalog.favorites_only || preferences.favorites.contains(&p.name))
            })
            .collect();
        profiles.sort_by_key(|(_, p)| p.name.to_lowercase());
        if self.catalog.versions_reverse {
            profiles.reverse();
        }
        let tabs = [
            (InstanceFilter::All, "All", total),
            (InstanceFilter::Vanilla, "Vanilla", total - modded),
            (InstanceFilter::Modded, "Modded", modded),
            (InstanceFilter::Imported, "Imported", imported),
        ];
        let mut tiles: Vec<AnyElement> = profiles
            .iter()
            .map(|(i, p)| {
                self.instance_card(*i, p, preferences.favorites.contains(&p.name), cx)
                    .into_any_element()
            })
            .collect();
        tiles.push(
            div()
                .id("catalog-new-instance")
                .flex_1()
                .min_w_0()
                .h(px(290. * s))
                .rounded(px(16. * s))
                .border_2()
                .border_dashed()
                .border_color(rgba(0xffffff26))
                .bg(rgba(0x0b0b0d66))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(10. * s))
                .cursor_pointer()
                .hover(|d| d.border_color(rgba(0xffffff66)))
                .child(
                    div()
                        .size(px(52. * s))
                        .rounded_full()
                        .bg(rgba(0xffffff1a))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon("plus", INK).size(px(24. * s))),
                )
                .child(
                    div()
                        .text_size(px(19. * s))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tr("New instance")),
                )
                .child(
                    div()
                        .text_size(px(14. * s))
                        .text_color(rgb(0xa9abb3))
                        .child(tr("A fresh game folder with its own worlds")),
                )
                .on_click(cx.listener(|this, _, _, cx| this.new_instance(cx)))
                .into_any_element(),
        );
        let mut grid = div().flex().flex_col().gap(px(18. * s));
        let mut tiles = tiles.into_iter().peekable();
        while tiles.peek().is_some() {
            let row: Vec<_> = tiles.by_ref().take(COLUMNS).collect();
            let missing = COLUMNS - row.len();
            grid = grid.child(
                div()
                    .flex()
                    .gap(px(18. * s))
                    .children(row)
                    .children((0..missing).map(|_| div().flex_1())),
            );
        }
        let filters = div()
            .flex()
            .items_center()
            .gap(px(10. * s))
            .child(
                div()
                    .w(px(260. * s))
                    .flex_none()
                    .child(self.version_search.clone()),
            )
            .child(
                div()
                    .flex()
                    .p(px(3. * s))
                    .rounded(px(10. * s))
                    .bg(rgba(0x16171acc))
                    .border_1()
                    .border_color(rgba(0xffffff1a))
                    .children(tabs.map(|(scope, label, n)| {
                        let on = self.catalog.versions_tab == scope;
                        div()
                            .id(("version-tab", scope as usize))
                            .h(px(32. * s))
                            .px(px(12. * s))
                            .rounded(px(8. * s))
                            .flex()
                            .items_center()
                            .gap(px(6. * s))
                            .text_size(px(14. * s))
                            .cursor_pointer()
                            .when(on, |d| {
                                d.bg(rgba(0xffffff1f)).font_weight(FontWeight::SEMIBOLD)
                            })
                            .when(!on, |d| {
                                d.text_color(rgb(0xc4c6cc))
                                    .hover(|d| d.bg(rgba(0xffffff12)))
                            })
                            .child(tr(label))
                            .child(
                                div()
                                    .text_size(px(12. * s))
                                    .text_color(rgb(MUTED))
                                    .child(n.to_string()),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.catalog.versions_tab = scope;
                                cx.notify();
                            }))
                    })),
            )
            .child(
                self.chip("instances-local", "Files present", self.catalog.local_only, local)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.catalog.local_only = !this.catalog.local_only;
                        cx.notify();
                    })),
            )
            .child(
                self.chip(
                    "instances-favorites",
                    "Favorites",
                    self.catalog.favorites_only,
                    favorites,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.catalog.favorites_only = !this.catalog.favorites_only;
                    cx.notify();
                })),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("sort-instances")
                    .h(px(34. * s))
                    .px(px(12. * s))
                    .rounded(px(9. * s))
                    .border_1()
                    .border_color(rgba(0xffffff1a))
                    .flex()
                    .items_center()
                    .gap(px(6. * s))
                    .text_size(px(14. * s))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgba(0xffffff12)))
                    .child(icon("down", MUTED).size(px(14. * s)))
                    .child(tr(if self.catalog.versions_reverse {
                        "Name Z–A"
                    } else {
                        "Name A–Z"
                    }))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.catalog.versions_reverse = !this.catalog.versions_reverse;
                        cx.notify();
                    })),
            );
        let actions = div()
            .flex()
            .gap(px(10. * s))
            .child(
                self.quiet_button("catalog-import-instance", "Import build", "folder")
                    .on_click(cx.listener(|this, _, _, cx| this.import(cx))),
            )
            .child(
                self.primary_button("versions-new", "New instance", "plus")
                    .on_click(cx.listener(|this, _, _, cx| this.new_instance(cx))),
            );
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(22. * s))
            .child(self.page_head(
                ("Back to Home", Page::Home),
                "Instances",
                "Every instance is its own Minecraft: its own worlds, mods, packs and settings. Pick one to play.",
                Some(actions),
                cx,
            ))
            .when(self.catalog.activity, |d| d.child(self.activity_panel(cx)))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .gap(px(22. * s))
                    .child(
                        div()
                            .id("versions-scroll")
                            .flex_1()
                            .min_w_0()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap(px(16. * s))
                            .child(filters)
                            .when(profiles.is_empty() && total > 0, |d| {
                                d.child(quiet("No instances match these filters."))
                            })
                            .child(grid),
                    )
                    .child(self.instance_panel(cx)),
            )
    }
}
