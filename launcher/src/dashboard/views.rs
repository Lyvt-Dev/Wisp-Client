use super::*;

impl Launcher {
    pub(super) fn page_content(&self, cx: &mut Context<Self>) -> Div {
        if self.page == Page::Home {
            return self.home(cx);
        }
        match self.page {
            Page::Instances => return self.versions_catalog(cx),
            Page::Mods => return self.mods_page(cx),
            Page::Shaders => return self.shaders_catalog(cx),
            Page::Shop => return self.shop_page(cx),
            Page::Packs => return self.packs_page(cx),
            Page::Servers => return self.servers_page(cx),
            Page::Account => return self.account_page(cx),
            Page::Locker => return self.locker_page(cx),
            Page::Settings => return self.settings_page(cx),
            _ => {}
        }
        let (title, description, glyph) = match self.page {
            Page::Instances => (
                "Versions & instances",
                "Choose an instance or create a fresh game folder.",
                "box",
            ),
            Page::Account => (
                "Microsoft account",
                "Sign in with the Microsoft account that owns Minecraft.",
                "user-circle",
            ),
            Page::Setup => (
                "Game setup",
                "Prepare your own game files for the native Rust engine.",
                "cpu",
            ),
            Page::Packs => (
                "Resource packs",
                "Manage resource packs in the selected instance.",
                "layers",
            ),
            Page::Shaders => (
                "Shaders",
                "Manage shader packs in the selected instance.",
                "sun",
            ),
            Page::Servers => (
                "Servers",
                "Play multiplayer through the Rust client.",
                "server",
            ),
            Page::Mods => (
                "Mods",
                "Install and configure Wisp .amod client modules for this instance.",
                "puzzle",
            ),
            Page::Settings => (
                "Settings",
                "Your launcher, accounts and local files.",
                "settings",
            ),
            _ => (
                "About Wisp",
                "Minecraft in Rust. Your worlds stay local.",
                "spark",
            ),
        };
        let content = match self.page {
            Page::Instances => self.instances(cx),
            Page::Setup => self.setup_page(cx),
            Page::Mods => self.mods_page(cx),
            _ => self.about(),
        };
        div()
            .flex()
            .flex_col()
            .gap_5()
            .pb_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(icon(glyph, accent_tone(ACCENT)).size(px(26.)))
                    .child(
                        div()
                            .text_size(px(27.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr(title)),
                    ),
            )
            .child(quiet(description))
            .child(content)
    }

    fn instances(&self, cx: &mut Context<Self>) -> Div {
        let mut content = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                panel()
                    .p_5()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(icon("cpu", accent_tone(ACCENT)).size(px(34.)))
                    .child(
                        div()
                            .flex_1()
                            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(trf(
                                "Rust engine · Minecraft {version}",
                                &[("version", engine::version())],
                            )))
                            .child(quiet(
                                "The native engine is bundled with the full Windows installer.",
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        button("new-instance", "New instance", "plus")
                            .on_click(cx.listener(|this, _, _, cx| this.new_instance(cx))),
                    )
                    .child(
                        button("import-instance", "Import local build", "download")
                            .on_click(cx.listener(|this, _, _, cx| this.import(cx))),
                    )
                    .child(button("prepare-files", "Game setup", "settings").on_click(
                        cx.listener(|this, _, _, cx| {
                            this.page = Page::Setup;
                            cx.notify();
                        }),
                    )),
            );
        for (index, profile) in self.settings.profiles.iter().enumerate() {
            let selected = self.settings.selected.as_ref() == Some(&profile.name);
            let mode = if profile.use_account {
                "Microsoft account"
            } else {
                "Local player"
            };
            content = content.child(
                panel()
                    .id(("instance", index))
                    .p_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .when(selected, |s| s.border_color(rgb(0x737684)))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(tone(RAISED))))
                    .on_click(cx.listener(move |this, _, _, cx| this.select(index, cx)))
                    .child(icon("box", accent_tone(ACCENT)).size(px(30.)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(text(profile.name.clone())),
                            )
                            .child(quiet(mode))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x857b94))
                                    .truncate()
                                    .child(text(profile.game_home.display().to_string())),
                            ),
                    )
                    .when(selected, |s| {
                        s.child(icon("check-circle", accent_tone(ACCENT)))
                    }),
            );
        }
        if self.settings.profiles.is_empty() {
            content=content.child(panel().p_6().child(quiet("Choose New instance or Install & play. Wisp downloads and prepares the required Minecraft files, then starts the bundled Rust game.")));
        }
        if self.settings.current().is_some() {
            content = content.child(
                panel()
                    .p_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(icon("user-circle", accent_tone(ACCENT)))
                    .child(
                        div()
                            .flex_1()
                            .child(quiet("Choose how the selected instance signs in.")),
                    )
                    .child(
                        button("toggle-account", "Switch account mode", "refresh")
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_account(cx))),
                    ),
            );
        }
        content
    }

    fn setup_page(&self, cx: &mut Context<Self>) -> Div {
        let mut content=div().flex().flex_col().gap_4()
            .child(panel().p_6().flex().flex_col().gap_4()
                .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(trf("Minecraft {version} · native Rust engine",&[("version",engine::version())])))
                .child(quiet("The Rust game is already included. Wisp downloads the required Minecraft files and preparation runtime automatically, creates your instance, and starts the game."))
                .child(quiet("Sign in with a Microsoft account that owns Minecraft: Java Edition. Downloads are verified and can resume after interruption. Gameplay runs in Rust."))
                .child(div().flex().gap_3()
                    .child(button("setup-start",if self.setup_busy {"Installing…"}else{"Install & play"},"download")
                        .on_click(cx.listener(|this,_,_,cx|this.install(cx))))
                    .child(button("setup-log","Open setup logs","terminal").on_click(cx.listener(|this,_,_,cx|this.show_path(this.home.join("logs"),cx))))))
            .child(panel().p_5().flex().flex_col().gap_3()
                .child(quiet("Advanced: reuse files you already have instead of downloading them."))
                .child(div().flex().gap_3()
                    .child(button("prepare-local","Prepare local files","folder").on_click(cx.listener(|this,_,_,cx|this.prepare(cx))))
                    .child(button("use-prepared","Use prepared resources","folder").on_click(cx.listener(|this,_,_,cx|this.use_prepared(cx))))));
        if self.setup_busy {
            content = content.child(
                panel()
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(quiet(&self.status))
                    .child(
                        div().h(px(5.)).rounded_full().bg(rgb(tone(RULE))).child(
                            div()
                                .h_full()
                                .rounded_full()
                                .bg(rgb(accent_tone(ACCENT)))
                                .w(relative(self.setup_fraction.unwrap_or(0.).clamp(0., 1.))),
                        ),
                    )
                    .child(
                        button("setup-cancel", "Cancel setup", "close")
                            .self_start()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.setup_cancel.store(true, Ordering::Relaxed);
                                this.status = tr("Stopping game setup…");
                                cx.notify();
                            })),
                    ),
            );
        }
        content
    }

    pub(super) fn empty_state(&self, glyph: &'static str, title: &str, detail: &str) -> Div {
        let s = self.ui_scale;
        panel()
            .py(px(48. * s))
            .px(px(24. * s))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(10. * s))
            .text_center()
            .child(icon(glyph, tone(0x5d6068)).size(px(44. * s)))
            .child(
                div()
                    .text_size(px(18. * s))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr(title)),
            )
            .child(
                div()
                    .max_w(px(520. * s))
                    .text_size(px(14. * s))
                    .line_height(px(20. * s))
                    .text_color(rgb(MUTED))
                    .child(tr(detail)),
            )
    }

    fn about(&self) -> Div {
        let s = self.ui_scale;
        let fact = |glyph: &'static str, title: &str, detail: &str| {
            div()
                .flex_1()
                .min_w_0()
                .p(px(16. * s))
                .rounded(px(12. * s))
                .border_1()
                .border_color(rgb(tone(RULE)))
                .bg(rgb(tone(SURFACE)))
                .flex()
                .flex_col()
                .gap(px(8. * s))
                .child(icon(glyph, accent_tone(ACCENT)).size(px(26. * s)))
                .child(
                    div()
                        .text_size(px(16. * s))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tr(title)),
                )
                .child(
                    div()
                        .text_size(px(13. * s))
                        .line_height(px(19. * s))
                        .text_color(rgb(MUTED))
                        .child(tr(detail)),
                )
        };
        div()
            .flex()
            .flex_col()
            .gap(px(14. * s))
            .pb(px(20. * s))
            .child(
                panel().p(px(20. * s)).flex().items_center().gap(px(16. * s))
                    .child(img("artwork/logo.png").size(px(56. * s)))
                    .child(
                        div()
                            .child(
                                div()
                                    .text_size(px(24. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(trf("Wisp {version}", &[("version", env!("CARGO_PKG_VERSION"))])),
                            )
                            .child(quiet("Minecraft Java Edition rebuilt in Rust, with its own launcher.")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(14. * s))
                    .child(fact("cpu", "Native engine", "Gameplay runs in Rust. Your worlds stay on this computer."))
                    .child(fact("puzzle", "Client modules", "Freelook, waypoints, damage numbers, HUD editor, inventory search and more on the Mods page."))
                    .child(fact("sun", "Aurora shaders", "Shadows, reflections, clouds and volumetric light, per instance.")),
            )
            .child(
                div()
                    .flex()
                    .gap(px(14. * s))
                    .child(fact("cart", "Shop", "Capes, wings, pets, trails, auras and emotes for coins you earn by playing."))
                    .child(fact("user", "Microsoft sign-in", "Sign in with the account that owns Minecraft; switch between several."))
                    .child(fact("gear", "Make it yours", "Pick the accent colour, background and banner in Settings, Appearance.")),
            )
            .child(quiet("Original licenses and contributor notices are included with the installer."))
    }
}
