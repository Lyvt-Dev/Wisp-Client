//! Shaders: the active pack as a showcase (preview, features, quality, off) and the instance's
//! packs as a grid with an "Off" tile, search and feature filters (`wisp_launcher::shaders`).
use super::reference_views::{glow, luminous};
use super::*;
use gpui::AnyElement;
use wisp_launcher::{catalog, shaders};

/// One pack the page shows: installed or offered.
struct Shown {
    manifest: wisp_shader_api::Manifest,
    file: String,
    local: bool,
    active: bool,
    update: bool,
}

fn preview(id: &str) -> &'static str {
    if id == shaders::AURORA_ID {
        "artwork/aurora.png"
    } else {
        "artwork/hero.png"
    }
}

impl Launcher {
    fn shader_result(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        match result {
            Ok(()) => {
                self.failed = false;
                self.status = tr("Shader settings saved. Changes apply when Minecraft starts.");
            }
            Err(e) => self.error(e),
        }
        cx.notify();
    }

    fn shader_chips(&self, m: &wisp_shader_api::Manifest) -> Div {
        let s = self.ui_scale;
        let chip = |label: &str, glyph: &'static str| {
            div()
                .h(px(26. * s))
                .px(px(9. * s))
                .rounded(px(6. * s))
                .bg(rgba(tone_a(0x232429e0)))
                .flex()
                .items_center()
                .gap(px(5. * s))
                .text_size(px(12. * s))
                .text_color(rgb(0xd9dadd))
                .child(icon(glyph, MUTED).size(px(13. * s)))
                .child(tr(label))
        };
        div()
            .flex()
            .flex_wrap()
            .gap(px(6. * s))
            .when(m.shadow.is_some(), |d| d.child(chip("Shadows", "sun")))
            .when(m.reflection.is_some(), |d| {
                d.child(chip("Reflections", "image-01"))
            })
            .when(m.sky.is_some(), |d| d.child(chip("Atmosphere", "earth")))
            .when(m.options.iter().any(|o| o.id == "clouds"), |d| {
                d.child(chip("Clouds", "layers"))
            })
    }

    fn shader_button(
        &self,
        id: impl Into<ElementId>,
        label: &str,
        glyph: &'static str,
        bright: bool,
    ) -> Stateful<Div> {
        let s = self.ui_scale;
        div()
            .id(id)
            .h(px(36. * s))
            .px(px(14. * s))
            .rounded(px(9. * s))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(7. * s))
            .text_size(px(14. * s))
            .cursor_pointer()
            .when(bright, |d| {
                d.bg(luminous())
                    .font_weight(FontWeight::SEMIBOLD)
                    .hover(|d| d.opacity(0.9))
            })
            .when(!bright, |d| {
                d.border_1()
                    .border_color(rgb(tone(0x2e3035)))
                    .hover(|d| d.bg(rgb(tone(0x202125))))
            })
            .child(icon(glyph, if bright { INK } else { MUTED }).size(px(15. * s)))
            .child(tr(label))
    }

    /// The active pack, large; or the vanilla look when none is on.
    fn shader_showcase(&self, active: Option<&Shown>, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let locked = self.mods_locked();
        let Some(pack) = active else {
            return div()
                .relative()
                .h(px(250. * s))
                .flex_none()
                .rounded(px(14. * s))
                .overflow_hidden()
                .border_1()
                .border_color(rgb(tone(RULE)))
                .child(img("artwork/hero.png").absolute().size_full().object_fit(ObjectFit::Cover))
                .child(div().absolute().size_full().bg(rgba(tone_a(0x0e0f11c8))))
                .child(
                    div()
                        .relative()
                        .size_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(10. * s))
                        .child(icon("sun", MUTED).size(px(40. * s)))
                        .child(
                            div()
                                .text_size(px(24. * s))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(tr("Shaders are off")),
                        )
                        .child(quiet("You play with Minecraft's own look. Pick a pack below to add shadows, reflections and light.")),
                );
        };
        let m = &pack.manifest;
        let file = pack.file.clone();
        let current = self
            .settings
            .current()
            .map(|p| shaders::quality(p, &pack.file))
            .unwrap_or(1);
        let aurora = m.id == shaders::AURORA_ID;
        div()
            .h(px(340. * s))
            .flex_none()
            .rounded(px(14. * s))
            .overflow_hidden()
            .border_1()
            .border_color(rgb(accent_tone(0x2f9e52)))
            .bg(rgb(tone(0x17181b)))
            .flex()
            .child(
                div()
                    .relative()
                    .w(relative(0.55))
                    .h_full()
                    .flex_none()
                    .overflow_hidden()
                    .child(img(preview(&m.id)).size_full().object_fit(ObjectFit::Cover))
                    .child(div().absolute().size_full().bg(gpui::linear_gradient(
                        90.,
                        gpui::linear_color_stop(rgba(tone_a(0x17181b00)), 0.6),
                        gpui::linear_color_stop(rgba(tone_a(0x17181bff)), 1.),
                    )))
                    .child(
                        div()
                            .absolute()
                            .top(px(14. * s))
                            .left(px(14. * s))
                            .px(px(10. * s))
                            .py(px(4. * s))
                            .rounded(px(6. * s))
                            .bg(luminous())
                            .flex()
                            .items_center()
                            .gap(px(6. * s))
                            .text_size(px(12. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(icon("check-circle", INK).size(px(13. * s)))
                            .child(tr("ACTIVE")),
                    )
                    .when(!aurora, |d| {
                        d.child(
                            div()
                                .absolute()
                                .bottom(px(10. * s))
                                .left(px(14. * s))
                                .text_size(px(11. * s))
                                .text_color(rgb(0xc4c6cc))
                                .child(tr("Artwork, not a screenshot of this pack")),
                        )
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .p(px(24. * s))
                    .flex()
                    .flex_col()
                    .gap(px(12. * s))
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(10. * s))
                            .child(
                                div()
                                    .text_size(px(28. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(m.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(13. * s))
                                    .text_color(rgb(MUTED))
                                    .child(trf("Version {version}", &[("version", &m.version)])),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(14. * s))
                            .line_height(px(20. * s))
                            .text_color(rgb(0xc4c6cc))
                            .max_h(px(60. * s))
                            .overflow_hidden()
                            .child(m.description.clone()),
                    )
                    .child(self.shader_chips(m))
                    .child(div().flex_1())
                    .when(aurora, |d| {
                        d.child(
                            div()
                                .text_size(px(12. * s))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(MUTED))
                                .child(tr("QUALITY")),
                        )
                        .child(
                            div()
                                .flex()
                                .p(px(3. * s))
                                .rounded(px(10. * s))
                                .bg(rgb(tone(0x111215)))
                                .border_1()
                                .border_color(rgb(tone(0x2e3035)))
                                .children([(0u32, "Low"), (1, "Balanced"), (2, "High")].map(|(level, label)| {
                                    let on = current == level;
                                    let file = file.clone();
                                    div()
                                        .id(("shader-quality", level as usize))
                                        .flex_1()
                                        .h(px(34. * s))
                                        .rounded(px(8. * s))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_size(px(14. * s))
                                        .cursor_pointer()
                                        .when(on, |d| d.bg(luminous()).font_weight(FontWeight::SEMIBOLD).shadow(glow(s * 0.5)))
                                        .when(!on, |d| d.text_color(rgb(0xc4c6cc)).hover(|d| d.bg(rgb(tone(0x202125)))))
                                        .when(locked, |d| d.opacity(0.5))
                                        .child(tr(label))
                                        .when(!locked && !on, |d| {
                                            d.on_click(cx.listener(move |this, _, _, cx| {
                                                let result = this
                                                    .settings
                                                    .current()
                                                    .ok_or_else(|| tr("Choose an instance first."))
                                                    .and_then(|p| shaders::set_quality(p, &file, level));
                                                this.shader_result(result, cx);
                                            }))
                                        })
                                })),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8. * s))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(12. * s))
                                    .text_color(rgb(MUTED))
                                    .child(tr("More options in Minecraft: Options > Shader Packs > Options.")),
                            )
                            .child(
                                self.shader_button("shader-off", "Turn off", "cancel-01", false)
                                    .when(locked, |d| d.opacity(0.45))
                                    .when(!locked, |d| {
                                        d.on_click(cx.listener(|this, _, _, cx| {
                                            let result = this
                                                .settings
                                                .current()
                                                .ok_or_else(|| tr("Choose an instance first."))
                                                .and_then(|p| shaders::set_active(p, None));
                                            this.shader_result(result, cx);
                                        }))
                                    }),
                            ),
                    ),
            )
    }

    fn shader_card(&self, index: usize, pack: &Shown, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = self.ui_scale;
        let locked = self.mods_locked();
        let m = &pack.manifest;
        let (local, active, update) = (pack.local, pack.active, pack.update);
        let file = pack.file.clone();
        let (label, glyph, bright) = if update {
            ("Update", "download", true)
        } else if !local {
            ("Install & use", "download", true)
        } else if active {
            ("In use", "check-circle", false)
        } else {
            ("Use", "play", true)
        };
        div()
            .id(("shader-card", index))
            .flex_1()
            .min_w_0()
            .rounded(px(12. * s))
            .overflow_hidden()
            .border_1()
            .border_color(rgb(if active {
                accent_tone(0x2f9e52)
            } else {
                tone(0x2a2b30)
            }))
            .bg(rgb(tone(0x17181b)))
            .hover(|d| d.border_color(rgb(tone(0x4a4c54))))
            .child(
                div()
                    .relative()
                    .h(px(150. * s))
                    .overflow_hidden()
                    .child(img(preview(&m.id)).size_full().object_fit(ObjectFit::Cover))
                    .child(div().absolute().size_full().bg(gpui::linear_gradient(
                        180.,
                        gpui::linear_color_stop(rgba(tone_a(0x17181b00)), 0.5),
                        gpui::linear_color_stop(rgba(tone_a(0x17181bff)), 1.),
                    )))
                    .when(active || !local || update, |d| {
                        let (tag, colour) = if update {
                            ("Update available", 0xf2c14e)
                        } else if active {
                            ("Active", accent_tone(ACCENT))
                        } else {
                            ("Not installed", 0xc4c6cc)
                        };
                        d.child(
                            div()
                                .absolute()
                                .top(px(10. * s))
                                .left(px(10. * s))
                                .px(px(8. * s))
                                .py(px(3. * s))
                                .rounded(px(6. * s))
                                .bg(rgba(tone_a(0x0e0f11d0)))
                                .text_size(px(12. * s))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(colour))
                                .child(tr(tag)),
                        )
                    }),
            )
            .child(
                div()
                    .px(px(14. * s))
                    .pb(px(14. * s))
                    .flex()
                    .flex_col()
                    .gap(px(8. * s))
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(8. * s))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(18. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(m.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12. * s))
                                    .text_color(rgb(MUTED))
                                    .child(m.version.clone()),
                            ),
                    )
                    .child(
                        div()
                            .h(px(34. * s))
                            .overflow_hidden()
                            .text_size(px(13. * s))
                            .line_height(px(17. * s))
                            .text_color(rgb(MUTED))
                            .child(m.description.clone()),
                    )
                    .child(self.shader_chips(m))
                    .child(
                        self.shader_button(("install-shader", index), label, glyph, bright)
                            .mt(px(4. * s))
                            .w_full()
                            .when(locked || (active && !update), |d| {
                                d.opacity(if locked { 0.45 } else { 1. })
                            })
                            .when(!locked && !(active && !update), |d| {
                                d.on_click(cx.listener(move |this, _, _, cx| {
                                    let Some(profile) = this.settings.current() else {
                                        this.page = Page::Instances;
                                        cx.notify();
                                        return;
                                    };
                                    let result = if update {
                                        shaders::update_aurora(profile)
                                    } else if !local {
                                        shaders::install_aurora(profile)
                                    } else {
                                        shaders::set_active(profile, Some(&file))
                                    };
                                    this.shader_result(result, cx);
                                }))
                            }),
                    ),
            )
    }

    fn shader_off_tile(&self, active: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = self.ui_scale;
        let locked = self.mods_locked();
        div()
            .id("shader-off-tile")
            .flex_1()
            .min_w_0()
            .min_h(px(320. * s))
            .rounded(px(12. * s))
            .border_1()
            .border_color(rgb(if active {
                accent_tone(0x2f9e52)
            } else {
                tone(0x2a2b30)
            }))
            .bg(rgb(tone(0x141518)))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10. * s))
            .text_center()
            .child(
                div()
                    .size(px(54. * s))
                    .rounded_full()
                    .bg(rgb(tone(0x232429)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        icon(
                            "cancel-01",
                            if active { accent_tone(ACCENT) } else { MUTED },
                        )
                        .size(px(24. * s)),
                    ),
            )
            .child(
                div()
                    .text_size(px(18. * s))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr("Off (vanilla)")),
            )
            .child(
                div()
                    .px(px(20. * s))
                    .child(quiet("Minecraft's own look, with the best frame rate.")),
            )
            .child(
                self.shader_button(
                    "shader-off-tile-button",
                    if active { "In use" } else { "Use" },
                    if active { "check-circle" } else { "play" },
                    false,
                )
                .mt(px(6. * s))
                .when(locked, |d| d.opacity(0.45))
                .when(!locked && !active, |d| {
                    d.on_click(cx.listener(|this, _, _, cx| {
                        let result = this
                            .settings
                            .current()
                            .ok_or_else(|| tr("Choose an instance first."))
                            .and_then(|p| shaders::set_active(p, None));
                        this.shader_result(result, cx);
                    }))
                }),
            )
    }

    pub(super) fn shaders_catalog(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let locked = self.mods_locked();
        let query = self.shader_search.read(cx).value.clone();
        let builtin = shaders::aurora()
            .expect("embedded Aurora manifest")
            .manifest;
        let installed = self
            .settings
            .current()
            .map(shaders::list)
            .unwrap_or_default();
        let mut packs: Vec<Shown> = installed
            .iter()
            .filter_map(|e| {
                e.pack.as_ref().ok().map(|p| Shown {
                    update: p.manifest.id == builtin.id
                        && catalog::needs_update(&p.manifest.version, &builtin.version),
                    manifest: p.manifest.clone(),
                    file: e.file.clone(),
                    local: true,
                    active: e.active,
                })
            })
            .collect();
        if !packs.iter().any(|p| p.manifest.id == builtin.id) {
            packs.push(Shown {
                manifest: builtin.clone(),
                file: shaders::AURORA_FILE.into(),
                local: false,
                active: false,
                update: false,
            });
        }
        packs.sort_by_key(|p| p.manifest.name.to_lowercase());
        let active_index = packs.iter().position(|p| p.active);
        let installed_count = packs.iter().filter(|p| p.local).count();
        let filter = |p: &&Shown| {
            let m = &p.manifest;
            catalog::matches(&query, &[&m.name, &m.description, &m.id])
                && (!self.catalog.shader_water || m.reflection.is_some())
                && (!self.catalog.shader_shadows || m.shadow.is_some())
                && (!self.catalog.shader_clouds || m.options.iter().any(|o| o.id == "clouds"))
        };
        let filtering = !query.trim().is_empty()
            || self.catalog.shader_water
            || self.catalog.shader_shadows
            || self.catalog.shader_clouds;
        let mut tiles: Vec<AnyElement> = Vec::new();
        if !filtering {
            tiles.push(
                self.shader_off_tile(active_index.is_none(), cx)
                    .into_any_element(),
            );
        }
        for (i, pack) in packs.iter().enumerate().filter(|(_, p)| filter(p)) {
            tiles.push(self.shader_card(i, pack, cx).into_any_element());
        }
        let shown = tiles.len();
        let mut grid = div().flex().flex_col().gap(px(14. * s));
        let mut tiles = tiles.into_iter().peekable();
        while tiles.peek().is_some() {
            let row: Vec<_> = tiles.by_ref().take(3).collect();
            let missing = 3 - row.len();
            grid = grid.child(div().flex().gap(px(14. * s)).children(row).children(
                (0..missing).map(|_| div().flex_1().min_w_0().border_1().border_color(rgba(0))),
            ));
        }
        let chip = |id: &'static str, label: &str, on: bool| {
            div()
                .id(id)
                .h(px(40. * s))
                .px(px(12. * s))
                .rounded(px(10. * s))
                .border_1()
                .border_color(rgb(if on {
                    accent_tone(ACCENT)
                } else {
                    tone(0x2e3035)
                }))
                .bg(if on {
                    rgba(accent_tone_a(0x3ccf6a22))
                } else {
                    rgba(0)
                })
                .flex()
                .items_center()
                .gap(px(6. * s))
                .text_size(px(14. * s))
                .cursor_pointer()
                .hover(|d| d.bg(rgb(tone(0x202125))))
                .when(on, |d| {
                    d.child(icon("checkmark-circle-01", accent_tone(ACCENT)).size(px(14. * s)))
                })
                .child(tr(label))
        };
        let summary = match active_index.map(|i| &packs[i]) {
            Some(p) => trf(
                "{name} is on for {instance}.",
                &[
                    ("name", &p.manifest.name),
                    (
                        "instance",
                        &self
                            .settings
                            .current()
                            .map(|p| p.name.clone())
                            .unwrap_or_default(),
                    ),
                ],
            ),
            None => tr("No shader is on. Pick one below; it applies when Minecraft starts."),
        };
        let page = div()
            .flex()
            .flex_col()
            .gap(px(16. * s))
            .pb(px(20. * s))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10. * s))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(30. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(tr("Shaders")),
                            )
                            .child(
                                div()
                                    .text_size(px(15. * s))
                                    .text_color(rgb(MUTED))
                                    .child(summary),
                            ),
                    )
                    .child(
                        self.shader_button("open-shaders-folder", "Shader folder", "folder", false)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_game_folder("shaderpacks", cx)
                            })),
                    )
                    .child(
                        self.shader_button(
                            "import-native-shader",
                            "Import .hshader",
                            "download",
                            true,
                        )
                        .when(locked, |d| d.opacity(0.45))
                        .when(!locked, |d| {
                            d.on_click(cx.listener(|this, _, _, cx| this.import_shader(cx)))
                        }),
                    ),
            )
            .when(locked, |d| {
                d.child(
                    div()
                        .px(px(14. * s))
                        .py(px(10. * s))
                        .rounded(px(10. * s))
                        .border_1()
                        .border_color(rgba(0xf2c14e55))
                        .bg(rgba(0xf2c14e14))
                        .flex()
                        .items_center()
                        .gap(px(10. * s))
                        .text_size(px(14. * s))
                        .child(icon("info", 0xf2c14e).size(px(16. * s)))
                        .child(tr(
                            "Minecraft is running. Close it to change shaders for this instance.",
                        )),
                )
            })
            .when(self.catalog.activity, |d| d.child(self.activity_panel(cx)))
            .child(self.shader_showcase(active_index.map(|i| &packs[i]), cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10. * s))
                    .child(
                        div()
                            .text_size(px(20. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("Your shader packs")),
                    )
                    .child(
                        div()
                            .text_size(px(14. * s))
                            .text_color(rgb(MUTED))
                            .child(installed_count.to_string()),
                    )
                    .child(div().flex_1()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10. * s))
                    .child(div().flex_1().child(self.shader_search.clone()))
                    .child(
                        chip(
                            "shadow-shader-filter",
                            "Shadows",
                            self.catalog.shader_shadows,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.catalog.shader_shadows = !this.catalog.shader_shadows;
                            cx.notify();
                        })),
                    )
                    .child(
                        chip(
                            "water-shader-filter",
                            "Reflections",
                            self.catalog.shader_water,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.catalog.shader_water = !this.catalog.shader_water;
                            cx.notify();
                        })),
                    )
                    .child(
                        chip("cloud-shader-filter", "Clouds", self.catalog.shader_clouds).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.catalog.shader_clouds = !this.catalog.shader_clouds;
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .when(shown == 0, |d| {
                d.child(quiet("No shader packs match these filters."))
            })
            .child(grid)
            .children(installed.iter().enumerate().filter_map(|(index, entry)| {
                entry.pack.as_ref().err().map(|e| {
                    div()
                        .id(("invalid-shader", index))
                        .px(px(14. * s))
                        .py(px(10. * s))
                        .rounded(px(10. * s))
                        .border_1()
                        .border_color(rgb(0x6a2f3a))
                        .text_size(px(13. * s))
                        .child(format!("{}: {e}", entry.file))
                })
            }));
        div().size_full().child(
            div()
                .id("shaders-scroll")
                .size_full()
                .overflow_y_scroll()
                .child(page),
        )
    }

    fn import_shader(&mut self, cx: &mut Context<Self>) {
        if self.mods_locked() {
            return;
        }
        let Some(profile) = self.settings.current().cloned() else {
            self.page = Page::Instances;
            cx.notify();
            return;
        };
        self.picking = true;
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr("Choose a native Wisp .hshader file").into()),
        });
        cx.spawn(async move |this, cx| {
            let picked: wisp_launcher::Result<Option<PathBuf>> =
                async { Ok(prompt.await??.and_then(|mut p| p.pop())) }.await;
            let result = match picked {
                Ok(Some(path)) => shaders::install(&profile, &path)
                    .map(|name| trf("Installed and selected {name}.", &[("name", &name)])),
                Ok(None) => Ok(tr("Shader import cancelled.")),
                Err(e) => Err(e.to_string()),
            };
            let _ = this.update(cx, |this, cx| {
                this.picking = false;
                match result {
                    Ok(message) => {
                        this.failed = false;
                        this.status = message;
                    }
                    Err(e) => this.error(e),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
