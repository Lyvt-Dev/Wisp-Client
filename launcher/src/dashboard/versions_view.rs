//! Versions: the selected instance at the top (artwork, what it holds, Launch, shortcuts), then
//! every instance as a tile, with search, type tabs, filter chips and sort above them.
use super::catalog_views::InstanceFilter;
use super::reference_views::{glow, luminous};
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
const COLUMNS: usize = 3;

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

    /// The selected instance's banner.
    fn selected_instance(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let Some(p) = self.settings.current() else {
            return div()
                .h(px(220. * s))
                .rounded(px(14. * s))
                .border_1()
                .border_color(rgb(tone(RULE)))
                .bg(rgb(tone(SURFACE)))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(12. * s))
                .child(icon("box", tone(0x5d6068)).size(px(44. * s)))
                .child(
                    div()
                        .text_size(px(20. * s))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tr("No instance yet")),
                )
                .child(quiet("Create one and Wisp installs Minecraft into it."))
                .child(
                    div()
                        .id("versions-first-instance")
                        .mt(px(6. * s))
                        .px(px(20. * s))
                        .py(px(10. * s))
                        .rounded(px(10. * s))
                        .bg(luminous())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(15. * s))
                        .cursor_pointer()
                        .child(tr("New instance"))
                        .on_click(cx.listener(|this, _, _, cx| this.new_instance(cx))),
                );
        };
        let busy = self.launching || self.child.is_some() || self.setup_busy || self.auth_busy;
        let game_home = p.game_home.clone();
        let mods = catalog::enabled_mods(p);
        let last = self
            .content
            .launched
            .get(&p.name)
            .map(|t| content::ago(*t))
            .unwrap_or_else(|| tr("never"));
        let stat = |value: String, label: &str, glyph: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(px(10. * s))
                .child(
                    div()
                        .size(px(36. * s))
                        .rounded(px(9. * s))
                        .bg(rgba(0xffffff14))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(glyph, INK).size(px(18. * s))),
                )
                .child(
                    div()
                        .child(
                            div()
                                .text_size(px(18. * s))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(value),
                        )
                        .child(
                            div()
                                .text_size(px(12. * s))
                                .text_color(rgb(0xc4c6cc))
                                .child(tr(label)),
                        ),
                )
        };
        let shortcut = |id: &'static str, label: &str, glyph: &'static str| {
            div()
                .id(id)
                .h(px(38. * s))
                .px(px(14. * s))
                .rounded(px(9. * s))
                .bg(rgba(tone_a(0x0e0f11b0)))
                .border_1()
                .border_color(rgba(0xffffff1f))
                .flex()
                .items_center()
                .gap(px(8. * s))
                .text_size(px(14. * s))
                .cursor_pointer()
                .hover(|d| d.bg(rgba(tone_a(0x232429e0))))
                .child(icon(glyph, INK).size(px(16. * s)))
                .child(tr(label))
        };
        let label = if self.child.is_some() {
            "Game running"
        } else if self.launching {
            "Starting…"
        } else if self.setup_busy {
            "Installing…"
        } else {
            "Launch"
        };
        div()
            .relative()
            .h(px(270. * s))
            .flex_none()
            .rounded(px(14. * s))
            .overflow_hidden()
            .border_1()
            .border_color(rgb(tone(RULE)))
            .child(
                img(art_for(&p.name))
                    .absolute()
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
            .child(div().absolute().size_full().bg(gpui::linear_gradient(
                90.,
                gpui::linear_color_stop(rgba(tone_a(0x0e0f11f5)), 0.35),
                gpui::linear_color_stop(rgba(tone_a(0x0e0f1140)), 1.),
            )))
            .child(
                div()
                    .relative()
                    .size_full()
                    .p(px(28. * s))
                    .flex()
                    .flex_col()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6. * s))
                                    .child(
                                        div()
                                            .text_size(px(12. * s))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb(accent_tone(0x6fe792)))
                                            .child(tr("SELECTED INSTANCE")),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(36. * s))
                                            .line_height(px(42. * s))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(p.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .gap(px(8. * s))
                                            .text_size(px(14. * s))
                                            .text_color(rgb(0xd9dadd))
                                            .child(self.version_label(p))
                                            .child("·")
                                            .child(tr(if mods == 0 { "Vanilla" } else { "Modded" }))
                                            .child("·")
                                            .child(trf("Played {when}", &[("when", &last)])),
                                    ),
                            )
                            .child(
                                div()
                                    .id("versions-launch")
                                    .h(px(56. * s))
                                    .px(px(34. * s))
                                    .rounded(px(10. * s))
                                    .border_1()
                                    .border_color(rgb(accent_tone(0x6fe792)))
                                    .bg(luminous())
                                    .shadow(glow(s))
                                    .flex()
                                    .items_center()
                                    .gap(px(10. * s))
                                    .text_size(px(20. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .when(busy, |d| d.opacity(0.6))
                                    .when(!busy, |d| {
                                        d.cursor_pointer()
                                            .hover(|d| d.opacity(0.9))
                                            .on_click(cx.listener(|this, _, _, cx| this.play(cx)))
                                    })
                                    .child(icon("play", INK).size(px(20. * s)))
                                    .child(tr(label).to_uppercase()),
                            ),
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
                                    .gap(px(28. * s))
                                    .child(stat(
                                        self.content.world_count.to_string(),
                                        "Worlds",
                                        "earth",
                                    ))
                                    .child(stat(mods.to_string(), "Mods on", "puzzle"))
                                    .child(stat(
                                        self.content.packs.len().to_string(),
                                        "Resource packs",
                                        "layers",
                                    ))
                                    .child(stat(
                                        self.content.servers.len().to_string(),
                                        "Servers",
                                        "server",
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(px(8. * s))
                                    .child(
                                        shortcut("versions-folder", "Folder", "folder").on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.show_path(game_home.clone(), cx)
                                            }),
                                        ),
                                    )
                                    .child(shortcut("versions-mods", "Mods", "puzzle").on_click(
                                        cx.listener(|this, _, _, cx| {
                                            this.page = Page::Mods;
                                            cx.notify();
                                        }),
                                    ))
                                    .child(
                                        shortcut("versions-logs", "Logs", "terminal").on_click(
                                            cx.listener(|this, _, _, cx| this.open_log(cx)),
                                        ),
                                    ),
                            ),
                    ),
            )
    }

    fn instance_tile(
        &self,
        index: usize,
        p: &Profile,
        favorite: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let s = self.ui_scale;
        let selected = self.settings.selected.as_ref() == Some(&p.name);
        let busy = self.launching || self.child.is_some() || self.setup_busy || self.auth_busy;
        let mods = catalog::enabled_mods(p);
        let last = self
            .content
            .launched
            .get(&p.name)
            .map(|t| trf("Played {when}", &[("when", &content::ago(*t))]))
            .unwrap_or_else(|| tr("Not played yet"));
        let expanded = self.catalog.expanded_instance == Some(index);
        let badge = |label: String| {
            div()
                .px(px(8. * s))
                .py(px(2. * s))
                .rounded(px(5. * s))
                .bg(rgb(tone(0x232429)))
                .text_size(px(11. * s))
                .text_color(rgb(0xc4c6cc))
                .child(label)
        };
        let mut tile = div()
            .id(("version-card", index))
            .flex_1()
            .min_w_0()
            .rounded(px(12. * s))
            .overflow_hidden()
            .border_1()
            .border_color(rgb(if selected {
                accent_tone(0x2f9e52)
            } else {
                tone(0x2a2b30)
            }))
            .bg(rgb(tone(0x17181b)))
            .cursor_pointer()
            .hover(|d| d.border_color(rgb(tone(0x4a4c54))))
            .on_click(cx.listener(move |this, _, _, cx| this.select(index, cx)))
            .child(
                div()
                    .relative()
                    .h(px(110. * s))
                    .overflow_hidden()
                    .child(
                        img(art_for(&p.name))
                            .size_full()
                            .object_fit(ObjectFit::Cover),
                    )
                    .child(div().absolute().size_full().bg(gpui::linear_gradient(
                        180.,
                        gpui::linear_color_stop(rgba(tone_a(0x17181b00)), 0.4),
                        gpui::linear_color_stop(rgba(tone_a(0x17181bff)), 1.),
                    )))
                    .when(selected, |d| {
                        d.child(
                            div()
                                .absolute()
                                .top(px(8. * s))
                                .left(px(8. * s))
                                .px(px(8. * s))
                                .py(px(3. * s))
                                .rounded(px(6. * s))
                                .bg(luminous())
                                .text_size(px(11. * s))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(tr("SELECTED")),
                        )
                    })
                    .child(
                        div()
                            .id(("favorite-instance", index))
                            .absolute()
                            .top(px(8. * s))
                            .right(px(8. * s))
                            .size(px(28. * s))
                            .rounded_full()
                            .bg(rgba(tone_a(0x0e0f11c0)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .child(
                                icon("star", if favorite { 0xf1c473 } else { 0xc4c6cc })
                                    .size(px(15. * s)),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                let name = this.settings.profiles[index].name.clone();
                                if let Err(e) =
                                    catalog::Preferences::toggle_favorite(&this.home, &name)
                                {
                                    this.error(e);
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .px(px(14. * s))
                    .pb(px(12. * s))
                    .flex()
                    .flex_col()
                    .gap(px(8. * s))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(18. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(p.name.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6. * s))
                            .child(badge(self.version_label(p)))
                            .child(badge(if mods == 0 {
                                tr("Vanilla")
                            } else {
                                if mods == 1 {
                                    tr("1 mod")
                                } else {
                                    trf("{count} mods", &[("count", &mods.to_string())])
                                }
                            }))
                            .when(!catalog::locally_available(p), |d| {
                                d.child(badge(tr("Files missing")))
                            }),
                    )
                    .child(
                        div()
                            .mt(px(2. * s))
                            .flex()
                            .items_center()
                            .gap(px(8. * s))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .items_center()
                                    .gap(px(6. * s))
                                    .text_size(px(12. * s))
                                    .text_color(rgb(MUTED))
                                    .child(icon("clock", MUTED).size(px(13. * s)))
                                    .child(div().truncate().child(last)),
                            )
                            .child(
                                div()
                                    .id(("instance-menu", index))
                                    .size(px(32. * s))
                                    .rounded(px(8. * s))
                                    .bg(rgb(tone(0x232429)))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|d| d.bg(rgb(tone(0x2c2e33))))
                                    .child(icon("more-horizontal", INK).size(px(16. * s)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.catalog.expanded_instance =
                                            if this.catalog.expanded_instance == Some(index) {
                                                None
                                            } else {
                                                Some(index)
                                            };
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id(("play-instance", index))
                                    .h(px(32. * s))
                                    .px(px(14. * s))
                                    .rounded(px(8. * s))
                                    .bg(luminous())
                                    .flex()
                                    .items_center()
                                    .gap(px(6. * s))
                                    .text_size(px(13. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .when(busy, |d| d.opacity(0.45))
                                    .when(!busy, |d| {
                                        d.cursor_pointer().hover(|d| d.opacity(0.88)).on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.select(index, cx);
                                                this.play(cx);
                                            }),
                                        )
                                    })
                                    .child(icon("play", INK).size(px(13. * s)))
                                    .child(tr("Play")),
                            ),
                    ),
            );
        if expanded {
            let dir = p.game_home.clone();
            let logs = p.game_home.join("logs");
            let item = |id: (&'static str, usize), label: &str, glyph: &'static str| {
                div()
                    .id(id)
                    .flex_1()
                    .h(px(32. * s))
                    .rounded(px(8. * s))
                    .border_1()
                    .border_color(rgb(tone(0x2e3035)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(6. * s))
                    .text_size(px(12. * s))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(tone(0x202125))))
                    .child(icon(glyph, MUTED).size(px(14. * s)))
                    .child(tr(label))
            };
            tile = tile.child(
                div()
                    .px(px(14. * s))
                    .pb(px(12. * s))
                    .flex()
                    .gap(px(8. * s))
                    .child(
                        item(("instance-folder", index), "Folder", "folder").on_click(cx.listener(
                            move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.show_path(dir.clone(), cx);
                            },
                        )),
                    )
                    .child(item(("instance-logs", index), "Logs", "terminal").on_click(
                        cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.show_path(logs.clone(), cx);
                        }),
                    )),
            );
        }
        tile
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
                self.instance_tile(*i, p, preferences.favorites.contains(&p.name), cx)
                    .into_any_element()
            })
            .collect();
        tiles.push(
            div()
                .id("catalog-new-instance")
                .flex_1()
                .min_w_0()
                .min_h(px(230. * s))
                .rounded(px(12. * s))
                .border_2()
                .border_dashed()
                .border_color(rgb(tone(0x34363d)))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(10. * s))
                .cursor_pointer()
                .hover(|d| {
                    d.border_color(rgb(accent_tone(0x2f9e52)))
                        .bg(rgb(tone(0x141518)))
                })
                .child(
                    div()
                        .size(px(44. * s))
                        .rounded_full()
                        .bg(rgb(tone(0x232429)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon("plus", INK).size(px(22. * s))),
                )
                .child(
                    div()
                        .text_size(px(16. * s))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tr("New instance")),
                )
                .child(
                    div()
                        .text_size(px(12. * s))
                        .text_color(rgb(MUTED))
                        .child(tr("A fresh game folder with its own worlds")),
                )
                .on_click(cx.listener(|this, _, _, cx| this.new_instance(cx)))
                .into_any_element(),
        );
        let mut grid = div().flex().flex_col().gap(px(14. * s));
        let mut tiles = tiles.into_iter().peekable();
        while tiles.peek().is_some() {
            let row: Vec<_> = tiles.by_ref().take(COLUMNS).collect();
            let missing = COLUMNS - row.len();
            grid = grid.child(
                div()
                    .flex()
                    .gap(px(14. * s))
                    .children(row)
                    .children((0..missing).map(|_| div().flex_1())),
            );
        }
        let page = div()
            .flex()
            .flex_col()
            .gap(px(18. * s))
            .pb(px(20. * s))
            .child(self.selected_instance(cx))
            .when(self.catalog.activity, |d| d.child(self.activity_panel(cx)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10. * s))
                    .child(
                        div()
                            .text_size(px(20. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("Your instances")),
                    )
                    .child(
                        div()
                            .text_size(px(14. * s))
                            .text_color(rgb(MUTED))
                            .child(total.to_string()),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("catalog-import-instance")
                            .h(px(36. * s))
                            .px(px(14. * s))
                            .rounded(px(9. * s))
                            .border_1()
                            .border_color(rgb(tone(0x2e3035)))
                            .flex()
                            .items_center()
                            .gap(px(8. * s))
                            .text_size(px(14. * s))
                            .cursor_pointer()
                            .hover(|d| d.bg(rgb(tone(0x202125))))
                            .child(icon("folder", MUTED).size(px(15. * s)))
                            .child(tr("Import build"))
                            .on_click(cx.listener(|this, _, _, cx| this.import(cx))),
                    )
                    .child(
                        div()
                            .id("versions-new")
                            .h(px(36. * s))
                            .px(px(14. * s))
                            .rounded(px(9. * s))
                            .bg(luminous())
                            .flex()
                            .items_center()
                            .gap(px(8. * s))
                            .text_size(px(14. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .hover(|d| d.opacity(0.9))
                            .child(icon("plus", INK).size(px(15. * s)))
                            .child(tr("New instance"))
                            .on_click(cx.listener(|this, _, _, cx| this.new_instance(cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10. * s))
                    .child(
                        div()
                            .w(px(280. * s))
                            .flex_none()
                            .child(self.version_search.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .p(px(3. * s))
                            .rounded(px(10. * s))
                            .bg(rgb(tone(0x17181b)))
                            .border_1()
                            .border_color(rgb(tone(0x2e3035)))
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
                                    .text_size(px(13. * s))
                                    .cursor_pointer()
                                    .when(on, |d| {
                                        d.bg(rgb(tone(0x2a2b30))).font_weight(FontWeight::SEMIBOLD)
                                    })
                                    .when(!on, |d| {
                                        d.text_color(rgb(0xc4c6cc))
                                            .hover(|d| d.bg(rgb(tone(0x202125))))
                                    })
                                    .child(tr(label))
                                    .child(
                                        div()
                                            .text_size(px(11. * s))
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
                        self.chip(
                            "instances-local",
                            "Files present",
                            self.catalog.local_only,
                            local,
                        )
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
                            .border_color(rgb(tone(0x2e3035)))
                            .flex()
                            .items_center()
                            .gap(px(6. * s))
                            .text_size(px(13. * s))
                            .cursor_pointer()
                            .hover(|d| d.bg(rgb(tone(0x202125))))
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
                    ),
            )
            .when(profiles.is_empty() && total > 0, |d| {
                d.child(quiet("No instances match these filters."))
            })
            .child(grid);
        div().size_full().child(
            div()
                .id("versions-scroll")
                .size_full()
                .overflow_y_scroll()
                .child(page),
        )
    }
}
