//! Mods: the built-in client features as a grid of switch tiles grouped by category (shared with
//! the game's Options > Wisp > Extras), and the instance's Wisp `.amod` modules below them.
use super::*;
use gpui::AnyElement;
use wisp_launcher::{catalog, client_features, mods};
use wisp_mods::amod::{self, Definition};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Category {
    Hud,
    Panel,
    Combat,
    Visual,
    Utility,
}

impl Category {
    const ALL: [Self; 5] = [
        Self::Hud,
        Self::Panel,
        Self::Combat,
        Self::Visual,
        Self::Utility,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Hud => "HUD",
            Self::Panel => "Info panel",
            Self::Combat => "Combat",
            Self::Visual => "Visual",
            Self::Utility => "Utility",
        }
    }
    /// The card banner's colour.
    fn colour(self) -> u32 {
        match self {
            Self::Hud => 0x4f7cff,
            Self::Panel => 0x22a99a,
            Self::Combat => 0xe5534b,
            Self::Visual => 0xf09a2e,
            Self::Utility => 0x8b6cff,
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Self::Hud => "dashboard-square-01",
            Self::Panel => "information-circle",
            Self::Combat => "sword",
            Self::Visual => "sun",
            Self::Utility => "sliders-horizontal",
        }
    }
}

/// What each built-in feature does and where it belongs; keys are `client_features::FEATURES`.
fn about(key: &str) -> (Category, &'static str) {
    use Category::*;
    match key {
        "fullbright" => (Visual, "See clearly in caves and at night."),
        "fps" => (Hud, "Frames per second in a corner of the screen."),
        "apple_skin" => (Hud, "Saturation and food value on the hunger bar."),
        "mouseTweaks" => (Utility, "Drag and scroll to move items between slots."),
        "smart_f3" => (Hud, "A shorter, tidier F3 debug screen."),
        "shulker_preview" => (Utility, "See a shulker box's contents in its tooltip."),
        "armor_status" => (Hud, "Your armour and its durability on screen."),
        "mod_menu" => (Utility, "Right Shift opens the Wisp mod menu in game."),
        "fast_items" => (Visual, "Dropped items drawn as one model, for more FPS."),
        "hitboxes" => (Combat, "Outlines around mobs and players."),
        "wisp_hud" => (Hud, "Glass panels with sunset accents for the HUD."),
        "hud_cps" => (Panel, "Clicks per second."),
        "hud_ping" => (Panel, "Your ping to the server."),
        "hud_direction" => (Panel, "Which way you face, with yaw."),
        "hud_biome" => (Panel, "The biome you stand in."),
        "hud_server" => (Panel, "The server's address."),
        "hud_speed" => (Panel, "Your speed in blocks per second."),
        "hud_clock" => (Panel, "Your computer's clock."),
        "hud_day_time" => (Panel, "In-game time and day."),
        "hud_weather" => (Panel, "The current weather."),
        "hud_effects" => (Panel, "Status effects with level and time left."),
        "hud_durability" => (Panel, "Durability of the item in your hand."),
        "hud_arrows" => (Panel, "Arrows left while holding a bow."),
        "always_day" => (
            Visual,
            "Your sky stays at noon; the world's time is untouched.",
        ),
        "item_physics" => (Visual, "Dropped items lie flat on the ground."),
        "wavy_capes" => (Visual, "Capes bend and wave as you move."),
        "arrow_trails" => (Visual, "Flying arrows leave a trail of sparks."),
        "target_crosshair" => (Combat, "The crosshair turns red over a mob or player."),
        "freelook" => (Utility, "Hold Left Alt to look around without turning."),
        "health_indicators" => (Combat, "Health over mobs and players nearby."),
        "damage_numbers" => (Combat, "Damage pops up over whoever takes it."),
        "tnt_timer" => (Combat, "Seconds left on lit TNT."),
        "hit_info" => (Combat, "Reach and combo of your last hits."),
        "item_highlighter" => (Visual, "Dropped items glow in their rarity's colour."),
        "chat_highlights" => (Utility, "A ping when someone says your name."),
        "chat_peek" => (Utility, "Hold H to read the whole chat while playing."),
        "waypoints" => (Utility, "/wp add marks places you can see from afar."),
        "inventory_search" => (Utility, "A search box over inventories and chests."),
        _ => (Utility, ""),
    }
}

fn switch(on: bool, s: f32) -> Div {
    div()
        .w(px(38. * s))
        .h(px(22. * s))
        .flex_none()
        .rounded_full()
        .p(px(3. * s))
        .when(on, |d| d.bg(rgb(0xffffff)).justify_end())
        .when(!on, |d| d.bg(rgb(tone(0x2c2e33))))
        .flex()
        .child(
            div()
                .size(px(16. * s))
                .rounded_full()
                .bg(rgb(if on { 0x0b0b0d } else { 0x80838c })),
        )
}

impl Launcher {
    pub(super) fn mods_locked(&self) -> bool {
        self.child.is_some() || self.launching || self.setup_busy || self.picking
    }

    pub(super) fn mod_result(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        match result {
            Ok(()) => {
                self.failed = false;
                self.status = tr("Mod settings saved. Changes apply when Minecraft starts.");
            }
            Err(e) => self.error(e),
        }
        cx.notify();
    }

    fn mods_action(
        &self,
        id: &'static str,
        label: &str,
        glyph: &'static str,
        bright: bool,
    ) -> Stateful<Div> {
        let s = self.ui_scale;
        div()
            .id(id)
            .h(px(38. * s))
            .px(px(14. * s))
            .rounded(px(9. * s))
            .flex()
            .items_center()
            .gap(px(8. * s))
            .text_size(px(14. * s))
            .cursor_pointer()
            .when(bright, |d| {
                d.bg(rgb(0xffffff))
                    .text_color(rgb(0x0b0b0d))
                    .font_weight(FontWeight::SEMIBOLD)
            })
            .when(!bright, |d| {
                d.border_1()
                    .border_color(rgb(tone(0x2e3035)))
                    .hover(|d| d.bg(rgb(tone(0x202125))))
            })
            .child(icon(glyph, if bright { 0x0b0b0d } else { MUTED }).size(px(15. * s)))
            .child(tr(label))
    }

    /// A built-in feature as a card: a coloured banner with its category's icon, its name,
    /// what it does, and its switch.
    fn feature_tile(
        &self,
        index: usize,
        key: &'static str,
        label: &'static str,
        on: bool,
        locked: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let s = self.ui_scale;
        let (category, text) = about(key);
        let hue = category.colour();
        div()
            .id(("builtin-feature", index))
            .flex_1()
            .min_w_0()
            .rounded(px(16. * s))
            .overflow_hidden()
            .border_1()
            .border_color(rgba(if on { 0xffffff59 } else { 0xffffff14 }))
            .bg(rgba(0x16171ae6))
            .flex()
            .flex_col()
            .when(locked, |d| d.opacity(0.5))
            .when(!locked, |d| {
                d.cursor_pointer()
                    .hover(|d| d.border_color(rgba(0xffffff40)))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.mods_locked() {
                    return;
                }
                let result = this
                    .settings
                    .current()
                    .ok_or_else(|| tr("Select an instance first."))
                    .and_then(|profile| client_features::toggle(profile, key));
                this.mod_result(result, cx);
            }))
            .child(
                div()
                    .relative()
                    .h(px(118. * s))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui::linear_gradient(
                        160.,
                        gpui::linear_color_stop(rgba((hue << 8) | 0x59), 0.),
                        gpui::linear_color_stop(rgba((hue << 8) | 0x0d), 1.),
                    ))
                    .child(
                        div()
                            .size(px(64. * s))
                            .rounded(px(16. * s))
                            .bg(rgb(hue))
                            .shadow(vec![gpui::BoxShadow {
                                color: rgba((hue << 8) | 0x55).into(),
                                offset: gpui::point(px(0.), px(6. * s)),
                                blur_radius: px(22. * s),
                                spread_radius: px(0.),
                                inset: false,
                            }])
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon(category.icon(), 0xffffff).size(px(30. * s))),
                    ),
            )
            .child(
                div()
                    .px(px(18. * s))
                    .pt(px(14. * s))
                    .pb(px(16. * s))
                    .flex()
                    .flex_col()
                    .gap(px(6. * s))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(19. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr(label)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6. * s))
                            .text_size(px(13. * s))
                            .text_color(rgb(0xa9abb3))
                            .child(tr("by Wisp"))
                            .child(icon("check-circle", 0x3ccf6a).size(px(13. * s))),
                    )
                    .child(
                        div()
                            .h(px(40. * s))
                            .overflow_hidden()
                            .text_size(px(14. * s))
                            .line_height(px(20. * s))
                            .text_color(rgb(0xc4c6cc))
                            .child(tr(text)),
                    )
                    .child(
                        div()
                            .mt(px(6. * s))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8. * s))
                                    .text_size(px(13. * s))
                                    .text_color(rgb(if on { INK } else { 0x8e9099 }))
                                    .child(switch(on, s))
                                    .child(tr(if on { "On" } else { "Off" })),
                            )
                            .child(
                                div()
                                    .px(px(10. * s))
                                    .py(px(3. * s))
                                    .rounded_full()
                                    .bg(rgba(0xffffff12))
                                    .flex()
                                    .items_center()
                                    .gap(px(5. * s))
                                    .text_size(px(12. * s))
                                    .text_color(rgb(0xd9dadd))
                                    .child(icon("check-circle", 0xd9dadd).size(px(12. * s)))
                                    .child(tr("Built in")),
                            ),
                    ),
            )
    }

    /// The `.amod` modules: Zoom (installable from Wisp) and anything imported.
    fn amod_section(&self, profile: &Profile, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let locked = self.mods_locked();
        let builtin = Definition::parse(amod::ZOOM_SOURCE).expect("embedded Zoom manifest");
        let installed = mods::list(profile);
        let mut cards: Vec<AnyElement> = Vec::new();
        let zoom_installed = installed
            .iter()
            .any(|e| e.definition.as_ref().is_ok_and(|m| m.id == builtin.id));
        let card = |id: (&'static str, usize)| {
            div()
                .id(id)
                .w_full()
                .p(px(16. * s))
                .rounded(px(12. * s))
                .border_1()
                .border_color(rgb(tone(0x2a2b30)))
                .bg(rgb(tone(0x17181b)))
                .flex()
                .flex_col()
                .gap(px(12. * s))
        };
        for (index, entry) in installed.into_iter().enumerate() {
            let m = match entry.definition {
                Ok(m) => m,
                Err(e) => {
                    cards.push(
                        card(("amod-broken", index))
                            .border_color(rgb(0x6a2f3a))
                            .child(quiet(&format!(
                                "{}: {e}",
                                entry.path.file_name().unwrap_or_default().to_string_lossy()
                            )))
                            .into_any_element(),
                    );
                    continue;
                }
            };
            let on = entry.enabled;
            let update = m.id == builtin.id && catalog::needs_update(&m.version, &builtin.version);
            let definition = m.clone();
            let config = mods::config(profile, &m);
            let options = [
                trf(
                    "Starting zoom {factor}×",
                    &[("factor", &config.initial_factor.to_string())],
                ),
                trf(
                    "Smooth zoom: {state}",
                    &[("state", &tr(if config.smooth { "On" } else { "Off" }))],
                ),
                trf(
                    "Scale sensitivity: {state}",
                    &[(
                        "state",
                        &tr(if config.scale_sensitivity {
                            "On"
                        } else {
                            "Off"
                        }),
                    )],
                ),
            ];
            cards.push(
                card(("amod", index))
                    .when(on, |d| d.border_color(rgb(accent_tone(0x2f9e52))))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(14. * s))
                            .child(
                                div()
                                    .size(px(52. * s))
                                    .flex_none()
                                    .rounded(px(12. * s))
                                    .bg(rgb(tone(0x232429)))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon(if m.module == "zoom" { "zoom" } else { "puzzle" }, INK).size(px(26. * s))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8. * s))
                                            .child(
                                                div()
                                                    .text_size(px(17. * s))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(m.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .px(px(7. * s))
                                                    .rounded(px(5. * s))
                                                    .bg(rgb(tone(0x232429)))
                                                    .text_size(px(11. * s))
                                                    .text_color(rgb(MUTED))
                                                    .child(m.version.clone()),
                                            )
                                            .when(update, |d| {
                                                d.child(
                                                    div()
                                                        .px(px(7. * s))
                                                        .rounded(px(5. * s))
                                                        .bg(rgba(0xf2c14e30))
                                                        .text_size(px(11. * s))
                                                        .text_color(rgb(0xf2c14e))
                                                        .child(tr("Update available")),
                                                )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13. * s))
                                            .text_color(rgb(MUTED))
                                            .child(m.description.clone()),
                                    ),
                            )
                            .when(update, |d| {
                                d.child(
                                    self.mods_action("amod-update", "Update", "download", true)
                                        .when(locked, |d| d.opacity(0.45))
                                        .when(!locked, |d| {
                                            d.on_click(cx.listener(|this, _, _, cx| {
                                                let result = this
                                                    .settings
                                                    .current()
                                                    .ok_or_else(|| tr("Choose an instance first."))
                                                    .and_then(mods::install_zoom);
                                                this.mod_result(result, cx);
                                            }))
                                        }),
                                )
                            })
                            .child(
                                div()
                                    .id(("toggle-amod", index))
                                    .when(locked, |d| d.opacity(0.45))
                                    .when(!locked, |d| {
                                        d.cursor_pointer().on_click(cx.listener(move |this, _, _, cx| {
                                            let result = this
                                                .settings
                                                .current()
                                                .ok_or_else(|| tr("Choose an instance first."))
                                                .and_then(|p| mods::set_enabled(p, &definition, !on));
                                            this.mod_result(result, cx);
                                        }))
                                    })
                                    .child(switch(on, s * 1.15)),
                            ),
                    )
                    .when(m.module == "zoom", |d| {
                        d.child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(px(8. * s))
                                .children(options.into_iter().enumerate().map(|(option, label)| {
                                    let m = m.clone();
                                    div()
                                        .id(("amod-option", index * 3 + option))
                                        .h(px(32. * s))
                                        .px(px(12. * s))
                                        .rounded(px(8. * s))
                                        .border_1()
                                        .border_color(rgb(tone(0x2e3035)))
                                        .flex()
                                        .items_center()
                                        .gap(px(6. * s))
                                        .text_size(px(13. * s))
                                        .cursor_pointer()
                                        .hover(|d| d.bg(rgb(tone(0x202125))))
                                        .when(locked, |d| d.opacity(0.45))
                                        .child(icon("settings", MUTED).size(px(13. * s)))
                                        .child(label)
                                        .when(!locked, |d| {
                                            d.on_click(cx.listener(move |this, _, _, cx| {
                                                let result = this
                                                    .settings
                                                    .current()
                                                    .ok_or_else(|| tr("Choose an instance first."))
                                                    .and_then(|p| mods::change_option(p, &m, option));
                                                this.mod_result(result, cx);
                                            }))
                                        })
                                }))
                                .child(
                                    div()
                                        .w_full()
                                        .text_size(px(12. * s))
                                        .text_color(rgb(MUTED))
                                        .child(tr("Hold Z in game and scroll to zoom further. Change the key in Options > Mods > Zoom.")),
                                ),
                        )
                    })
                    .into_any_element(),
            );
        }
        if !zoom_installed {
            cards.push(
                card(("amod-zoom-offer", 0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(14. * s))
                            .child(
                                div()
                                    .size(px(52. * s))
                                    .flex_none()
                                    .rounded(px(12. * s))
                                    .bg(rgb(tone(0x232429)))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon("zoom", INK).size(px(26. * s))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_size(px(17. * s))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(builtin.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13. * s))
                                            .text_color(rgb(MUTED))
                                            .child(builtin.description.clone()),
                                    ),
                            )
                            .child(
                                self.mods_action("install-zoom", "Install", "download", true)
                                    .when(locked, |d| d.opacity(0.45))
                                    .when(!locked, |d| {
                                        d.on_click(cx.listener(|this, _, _, cx| {
                                            let result = this
                                                .settings
                                                .current()
                                                .ok_or_else(|| tr("Choose an instance first."))
                                                .and_then(mods::install_zoom);
                                            this.mod_result(result, cx);
                                        }))
                                    }),
                            ),
                    )
                    .into_any_element(),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(12. * s))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10. * s))
                    .child(icon("puzzle", accent_tone(ACCENT)).size(px(20. * s)))
                    .child(
                        div()
                            .text_size(px(20. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("Wisp modules")),
                    )
                    .child(
                        div()
                            .text_size(px(13. * s))
                            .text_color(rgb(MUTED))
                            .child(tr(".amod files in this instance")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .h(px(1.))
                            .mx(px(8. * s))
                            .bg(rgb(tone(0x26272c))),
                    ),
            )
            .children(cards)
    }

    /// What Browse shows: these mods, shaders or resource packs (each its own page).
    pub(super) fn browse_kinds(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let kinds = [
            ("Mods", Page::Mods, "puzzle"),
            ("Shaders", Page::Shaders, "sun"),
            ("Resource packs", Page::Packs, "layers"),
        ];
        div()
            .flex()
            .p(px(4. * s))
            .rounded(px(12. * s))
            .bg(rgba(0x16171acc))
            .border_1()
            .border_color(rgba(0xffffff1a))
            .children(
                kinds
                    .into_iter()
                    .enumerate()
                    .map(|(i, (label, page, glyph))| {
                        let on = self.page == page;
                        div()
                            .id(("browse-kind", i))
                            .h(px(38. * s))
                            .px(px(16. * s))
                            .rounded(px(9. * s))
                            .flex()
                            .items_center()
                            .gap(px(8. * s))
                            .text_size(px(15. * s))
                            .cursor_pointer()
                            .when(on, |d| {
                                d.bg(rgba(0xffffff1f)).font_weight(FontWeight::SEMIBOLD)
                            })
                            .when(!on, |d| {
                                d.text_color(rgb(0xb4b6bd))
                                    .hover(|d| d.bg(rgba(0xffffff12)))
                            })
                            .child(icon(glyph, if on { INK } else { 0xb4b6bd }).size(px(17. * s)))
                            .child(tr(label))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.page = page;
                                cx.notify();
                            }))
                    }),
            )
    }

    /// "for <instance>": the instance Browse changes, with the list to pick another.
    pub(super) fn browse_for(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let name = self
            .settings
            .current()
            .map_or_else(|| tr("No instance yet"), |p| p.name.clone());
        div()
            .flex()
            .items_center()
            .gap(px(12. * s))
            .child(
                div()
                    .text_size(px(18. * s))
                    .text_color(rgb(0xa9abb3))
                    .child(tr("for")),
            )
            .child(
                div()
                    .relative()
                    .child(
                        div()
                            .id("browse-instance")
                            .h(px(42. * s))
                            .px(px(18. * s))
                            .rounded(px(10. * s))
                            .bg(rgba(0x16171acc))
                            .border_1()
                            .border_color(rgba(0xffffff1a))
                            .flex()
                            .items_center()
                            .gap(px(10. * s))
                            .text_size(px(16. * s))
                            .cursor_pointer()
                            .hover(|d| d.bg(rgba(0xffffff12)))
                            .child(trf(
                                "{instance} · Minecraft {version}",
                                &[("instance", &name), ("version", engine::version())],
                            ))
                            .child(icon("down", MUTED).size(px(14. * s)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.instance_menu = !this.instance_menu;
                                cx.notify();
                            })),
                    )
                    .when(self.instance_menu, |d| {
                        d.child(
                            gpui::deferred(
                                self.instance_menu(cx).top(px(48. * s)).min_w(px(300. * s)),
                            )
                            .with_priority(1),
                        )
                    }),
            )
    }

    pub(super) fn mods_page(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let locked = self.mods_locked();
        let Some(profile) = self.settings.current() else {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(12. * s))
                        .child(icon("puzzle", tone(0x5d6068)).size(px(44. * s)))
                        .child(
                            div()
                                .text_size(px(20. * s))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(tr("No instance yet")),
                        )
                        .child(quiet(
                            "Create an instance first, then turn on features for it.",
                        )),
                );
        };
        let values = client_features::read(profile);
        let query = self.mod_search.read(cx).value.clone();
        let features = client_features::FEATURES;
        let on_of = |key: &str| {
            values
                .as_ref()
                .ok()
                .and_then(|v| v.get(key).copied())
                .unwrap_or(false)
        };
        let enabled = features.iter().filter(|(k, _)| on_of(k)).count();
        let chosen = self.catalog.mods_category;
        let shown: Vec<(usize, &'static str, &'static str)> = features
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, (key, label))| {
                let (category, text) = about(key);
                chosen.is_none_or(|c| Category::ALL[c] == category)
                    && (!self.catalog.enabled_only || on_of(key))
                    && catalog::matches(&query, &[label, text, key])
            })
            .map(|(i, (k, l))| (i, k, l))
            .collect();

        // The categories down the left, as in a store.
        let entry = |id: usize, label: String, count: usize, on: bool| {
            div()
                .id(("mods-category", id))
                .py(px(9. * s))
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(18. * s))
                .cursor_pointer()
                .text_color(rgb(if on { INK } else { 0x9a9ca4 }))
                .when(on, |d| d.font_weight(FontWeight::SEMIBOLD))
                .hover(|d| d.text_color(rgb(INK)))
                .child(label)
                .child(
                    div()
                        .text_size(px(13. * s))
                        .text_color(rgb(0x6b6d75))
                        .child(count.to_string()),
                )
        };
        let mut sidebar = div()
            .w(px(230. * s))
            .flex_none()
            .flex()
            .flex_col()
            .child(
                div()
                    .pb(px(8. * s))
                    .text_size(px(13. * s))
                    .text_color(rgb(0x8e9099))
                    .child(tr("CATEGORIES")),
            )
            .child(
                entry(99, tr("All"), features.len(), chosen.is_none()).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.catalog.mods_category = None;
                        cx.notify();
                    },
                )),
            );
        for (i, category) in Category::ALL.into_iter().enumerate() {
            let count = features
                .iter()
                .filter(|(k, _)| about(k).0 == category)
                .count();
            sidebar = sidebar.child(
                entry(i, tr(category.label()), count, chosen == Some(i)).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.catalog.mods_category = Some(i);
                        cx.notify();
                    },
                )),
            );
        }
        sidebar = sidebar
            .child(div().h(px(1.)).my(px(14. * s)).bg(rgba(0xffffff14)))
            .child(
                div()
                    .id("enabled-module-filter")
                    .flex()
                    .items_center()
                    .gap(px(10. * s))
                    .text_size(px(15. * s))
                    .cursor_pointer()
                    .child(switch(self.catalog.enabled_only, s * 0.9))
                    .child(trf("Only on ({count})", &[("count", &enabled.to_string())]))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.catalog.enabled_only = !this.catalog.enabled_only;
                        cx.notify();
                    })),
            )
            .child(div().h(px(18. * s)))
            .child(
                self.quiet_button("open-catalog-mods", "Mods folder", "folder")
                    .on_click(cx.listener(|this, _, _, cx| this.open_game_folder("wisp-mods", cx))),
            )
            .child(div().h(px(10. * s)))
            .child(
                self.quiet_button("import-catalog-amod", "Import .amod", "download")
                    .when(locked, |d| d.opacity(0.45))
                    .when(!locked, |d| {
                        d.on_click(cx.listener(|this, _, _, cx| this.import_amod(cx)))
                    }),
            );

        let mut grid = div().flex().flex_col().gap(px(18. * s));
        for row in shown.chunks(3) {
            let missing = 3 - row.len();
            grid = grid.child(
                div()
                    .flex()
                    .gap(px(18. * s))
                    .children(row.iter().map(|(i, key, label)| {
                        self.feature_tile(*i, key, label, on_of(key), locked, cx)
                    }))
                    .children((0..missing).map(|_| div().flex_1().min_w_0())),
            );
        }

        let toolbar = div()
            .flex()
            .items_center()
            .gap(px(12. * s))
            .child(self.browse_for(cx))
            .child(div().flex_1())
            .child(self.browse_kinds(cx))
            .child(
                div()
                    .w(px(320. * s))
                    .flex_none()
                    .child(self.mod_search.clone()),
            );

        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(18. * s))
            .child(self.page_head(
                ("Back to Instances", Page::Instances),
                "Browse Mods",
                &trf(
                    "{on} of {all} Wisp features are on. Changes apply when Minecraft starts.",
                    &[
                        ("on", &enabled.to_string()),
                        ("all", &features.len().to_string()),
                    ],
                ),
                None,
                cx,
            ))
            .child(toolbar)
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
                            "Minecraft is running. Close it to change mods for this instance.",
                        )),
                )
            })
            .when_some(values.as_ref().err(), |d, error| d.child(quiet(error)))
            .when(self.catalog.activity, |d| d.child(self.activity_panel(cx)))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .gap(px(28. * s))
                    .child(sidebar)
                    .child(
                        div()
                            .id("mods-scroll")
                            .flex_1()
                            .min_w_0()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap(px(18. * s))
                            .when(shown.is_empty(), |d| {
                                d.child(quiet("No features match these filters."))
                            })
                            .child(grid)
                            .child(self.amod_section(profile, cx))
                            .child(div().h(px(8. * s))),
                    ),
            )
    }

    pub(super) fn import_amod(&mut self, cx: &mut Context<Self>) {
        if self.mods_locked() || self.settings.current().is_none() {
            return;
        }
        let profile = self.settings.current().unwrap().clone();
        self.picking = true;
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr("Choose an Wisp .amod file").into()),
        });
        cx.spawn(async move |this, cx| {
            let picked: wisp_launcher::Result<Option<PathBuf>> =
                async { Ok(prompt.await??.and_then(|mut p| p.pop())) }.await;
            let result = match picked {
                Ok(Some(path)) => {
                    mods::install(&profile, &path).map(|m| mods::installed_message(&m))
                }
                Ok(None) => Ok(tr("Mod import cancelled.")),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_feature_has_a_description() {
        for (key, _) in client_features::FEATURES {
            assert!(!about(key).1.is_empty(), "{key}");
        }
    }
}
