//! The profile: the signed-in player's skin, name and what they have (coins, items, instances),
//! their equipped cosmetics, every saved account, and the Microsoft sign-in flows.
use super::*;
use wisp_launcher::shop;

const EDIT_SKIN: &str = "https://www.minecraft.net/msaprofile/mygames/editskin";

impl Launcher {
    fn profile_button(
        &self,
        id: impl Into<ElementId>,
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
                    .hover(|d| d.opacity(0.9))
            })
            .when(!bright, |d| {
                d.bg(rgba(tone_a(0x0e0f11b0)))
                    .border_1()
                    .border_color(rgba(0xffffff26))
                    .hover(|d| d.bg(rgba(tone_a(0x232429e0))))
            })
            .child(icon(glyph, if bright { 0x0b0b0d } else { 0xd9dadd }).size(px(15. * s)))
            .child(tr(label))
    }

    fn profile_stat(&self, value: String, label: &str, glyph: &'static str, colour: u32) -> Div {
        let s = self.ui_scale;
        div()
            .flex()
            .items_center()
            .gap(px(10. * s))
            .child(
                div()
                    .size(px(38. * s))
                    .rounded(px(10. * s))
                    .bg(rgba(0xffffff12))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(glyph, colour).size(px(18. * s))),
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
    }

    /// The big card on top: skin, name, stats and actions.
    fn profile_hero(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let busy = self.auth_busy;
        let signed = self.account.as_ref();
        let model = signed.and_then(|a| a.account.skin.as_ref()).map(|(_, m)| {
            if m == "slim" {
                tr("Slim arms")
            } else {
                tr("Classic arms")
            }
        });
        let figure = div()
            .w(px(170. * s))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .justify_end()
            .pb(px(18. * s))
            .child(match self.body.clone().filter(|_| signed.is_some()) {
                Some(body) => img(body).w(px(112. * s)).h(px(224. * s)).into_any_element(),
                None => div()
                    .w(px(112. * s))
                    .h(px(224. * s))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon("user", tone(0x5d6068)).size(px(90. * s)))
                    .into_any_element(),
            })
            .child(
                div()
                    .mt(px(-6. * s))
                    .w(px(120. * s))
                    .h(px(14. * s))
                    .rounded_full()
                    .bg(rgba(0xffffff26)),
            );
        let details = match signed {
            Some(a) => {
                let uuid = a.account.uuid.clone();
                let uuid_copy = uuid.clone();
                let owned = self.wallet.owned.len();
                let worn = self.wallet.equipped.len();
                let me = a.account.uuid.clone();
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap(px(12. * s))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8. * s))
                            .child(div().size(px(9. * s)).rounded_full().bg(rgb(0x4ade80)))
                            .child(
                                div()
                                    .text_size(px(13. * s))
                                    .text_color(rgb(0x4ade80))
                                    .child(tr(if self.child.is_some() { "Playing Wisp" } else { "Online in launcher" })),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(40. * s))
                            .line_height(px(44. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(a.account.name.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(8. * s))
                            .child(
                                div()
                                    .id("profile-uuid")
                                    .h(px(28. * s))
                                    .px(px(10. * s))
                                    .rounded(px(7. * s))
                                    .bg(rgba(tone_a(0x0e0f11b0)))
                                    .flex()
                                    .items_center()
                                    .gap(px(7. * s))
                                    .font_family("Geist Mono")
                                    .text_size(px(12. * s))
                                    .text_color(rgb(0xc4c6cc))
                                    .cursor_pointer()
                                    .hover(|d| d.bg(rgba(tone_a(0x232429e0))))
                                    .child(uuid)
                                    .child(icon("copy", MUTED).size(px(13. * s)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(uuid_copy.clone()));
                                        this.failed = false;
                                        this.status = tr("UUID copied.");
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .h(px(28. * s))
                                    .px(px(10. * s))
                                    .rounded(px(7. * s))
                                    .bg(rgba(0x4ade8026))
                                    .flex()
                                    .items_center()
                                    .gap(px(6. * s))
                                    .text_size(px(12. * s))
                                    .text_color(rgb(0x4ade80))
                                    .child(icon("shield-check", 0x4ade80).size(px(13. * s)))
                                    .child(tr("Owns Minecraft: Java Edition")),
                            )
                            .when_some(model, |d, m| {
                                d.child(
                                    div()
                                        .h(px(28. * s))
                                        .px(px(10. * s))
                                        .rounded(px(7. * s))
                                        .bg(rgba(tone_a(0x0e0f11b0)))
                                        .flex()
                                        .items_center()
                                        .text_size(px(12. * s))
                                        .text_color(rgb(0xc4c6cc))
                                        .child(m),
                                )
                            }),
                    )
                    .child(
                        div()
                            .mt(px(6. * s))
                            .flex()
                            .gap(px(26. * s))
                            .child(self.profile_stat(self.wallet.coins.to_string(), "Coins", "coin", 0xf2c14e))
                            .child(self.profile_stat(owned.to_string(), "Items owned", "star", INK))
                            .child(self.profile_stat(worn.to_string(), "Equipped", "cape", INK))
                            .child(self.profile_stat(self.settings.profiles.len().to_string(), "Instances", "box", INK)),
                    )
                    .child(
                        div()
                            .mt(px(6. * s))
                            .flex()
                            .gap(px(8. * s))
                            .child(
                                self.profile_button("profile-skin", "Change skin", "pencil-edit-02", true)
                                    .on_click(|_, _, cx| cx.open_url(EDIT_SKIN)),
                            )
                            .child(
                                self.profile_button("profile-shop", "Cosmetics", "cart", false)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.page = Page::Shop;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                self.profile_button("profile-sign-out", "Sign out", "log-out", false)
                                    .on_click(cx.listener(move |this, _, _, cx| this.remove_account(me.clone(), cx))),
                            ),
                    )
            }
            None => div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .justify_center()
                .gap(px(12. * s))
                .child(
                    div()
                        .text_size(px(36. * s))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tr(if busy { "Signing in…" } else { "Sign in to Wisp" })),
                )
                .child(
                    div()
                        .max_w(px(520. * s))
                        .text_size(px(15. * s))
                        .line_height(px(22. * s))
                        .text_color(rgb(0xd9dadd))
                        .child(tr("Use the Microsoft account that owns Minecraft: Java Edition. You need it for online servers, your skin and your cosmetics.")),
                )
                .child(
                    div()
                        .mt(px(6. * s))
                        .flex()
                        .gap(px(8. * s))
                        .child(
                            self.profile_button("add-account", if busy { "Signing in…" } else { "Sign in with Microsoft" }, "user", true)
                                .when(busy, |d| d.opacity(0.6))
                                .when(!busy, |d| d.on_click(cx.listener(|this, _, _, cx| this.sign_in(cx)))),
                        )
                        .child(
                            self.profile_button("add-account-code", "Use a code instead", "copy", false)
                                .when(!busy, |d| d.on_click(cx.listener(|this, _, _, cx| this.sign_in_with_code(cx)))),
                        ),
                ),
        };
        div()
            .relative()
            .h(px(330. * s))
            .flex_none()
            .rounded(px(20. * s))
            .overflow_hidden()
            .border_1()
            .border_color(rgba(0xffffff1a))
            .bg(rgba(0x0b0b0dc4))
            .child(
                div()
                    .relative()
                    .size_full()
                    .px(px(28. * s))
                    .flex()
                    .gap(px(24. * s))
                    .child(figure)
                    .child(details),
            )
    }

    pub(super) fn account_page(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        if auth::client_id().is_none() {
            return div()
                .flex()
                .flex_col()
                .gap(px(12. * s))
                .child(self.empty_state(
                    "info",
                    "Microsoft sign-in setup required",
                    "This build has no Microsoft application ID configured.",
                ))
                .child(
                    self.profile_button("signin-guide", "Open setup guide", "arrow", true)
                        .self_center()
                        .on_click(|_, _, cx| cx.open_url(GUIDE)),
                );
        }
        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(16. * s))
            .pb(px(20. * s))
            .child(self.page_head(
                ("Back to Home", Page::Home),
                "Account",
                "Your Microsoft account, your skin and the Wisp items you wear.",
                None,
                cx,
            ));
        if self.install_after_sign_in {
            page = page.child(quiet("Sign in to install Minecraft. Wisp downloads the files and opens the game right after."));
        }
        if self.browser_waiting {
            page = page.child(
                div()
                    .p(px(18. * s))
                    .rounded(px(16. * s))
                    .border_1()
                    .border_color(rgba(0xffffff40))
                    .bg(rgba(0x0b0b0dc4))
                    .flex()
                    .items_center()
                    .gap(px(16. * s))
                    .child(icon("shield-check", INK).size(px(30. * s)))
                    .child(
                        div()
                            .flex_1()
                            .child(div().text_size(px(17. * s)).font_weight(FontWeight::SEMIBOLD).child(tr("Waiting for the Microsoft window")))
                            .child(quiet("Sign in on the Microsoft page that just opened. Your password goes only to Microsoft, never to Wisp.")),
                    )
                    .child(self.profile_button("cancel-browser", "Cancel", "cancel-01", false).on_click(cx.listener(|this, _, _, cx| {
                        this.auth_cancel.store(true, Ordering::Relaxed);
                        this.status = tr("Cancelling sign-in…");
                        cx.notify();
                    }))),
            );
        } else if let Some(code) = &self.device {
            let user_code = code.user_code.clone();
            let copy = user_code.clone();
            page = page.child(
                div()
                    .p(px(20. * s))
                    .rounded(px(16. * s))
                    .border_1()
                    .border_color(rgba(0xffffff40))
                    .bg(rgba(0x0b0b0dc4))
                    .flex()
                    .items_center()
                    .gap(px(20. * s))
                    .child(
                        div()
                            .flex_1()
                            .child(div().text_size(px(17. * s)).font_weight(FontWeight::SEMIBOLD).child(tr("Sign in at microsoft.com/link")))
                            .child(quiet("Enter this code there. Your Microsoft password is never entered in Wisp.")),
                    )
                    .child(
                        div()
                            .px(px(18. * s))
                            .py(px(8. * s))
                            .rounded(px(10. * s))
                            .bg(rgb(tone(0x111215)))
                            .font_family("Geist Mono")
                            .text_size(px(28. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(user_code),
                    )
                    .child(self.profile_button("copy-device-code", "Copy", "copy", false).on_click(move |_, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy.clone()))
                    }))
                    .child(self.profile_button("open-microsoft", "Open page", "arrow", true).on_click(|_, _, cx| cx.open_url("https://www.microsoft.com/link")))
                    .child(self.profile_button("cancel-sign-in", "Cancel", "cancel-01", false).on_click(cx.listener(|this, _, _, cx| {
                        this.auth_cancel.store(true, Ordering::Relaxed);
                        this.status = tr("Cancelling sign-in…");
                        cx.notify();
                    }))),
            );
        }
        page = page.child(self.profile_hero(cx));

        // Equipped cosmetics.
        let equipped: Vec<_> = self
            .wallet
            .equipped
            .iter()
            .filter_map(|id| shop::offer(id))
            .collect();
        let section = |glyph: &'static str, title: &str, note: String| {
            div()
                .mt(px(6. * s))
                .flex()
                .items_center()
                .gap(px(10. * s))
                .child(icon(glyph, INK).size(px(18. * s)))
                .child(
                    div()
                        .text_size(px(19. * s))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tr(title)),
                )
                .child(
                    div()
                        .text_size(px(13. * s))
                        .text_color(rgb(MUTED))
                        .child(note),
                )
                .child(
                    div()
                        .flex_1()
                        .h(px(1.))
                        .ml(px(8. * s))
                        .bg(rgb(tone(0x26272c))),
                )
        };
        page = page.child(section("cape", "Equipped", equipped.len().to_string()));
        if equipped.is_empty() {
            page = page.child(quiet(
                "Nothing equipped yet. Claim the Wisp Founders Cape in the Store.",
            ));
        } else {
            let mut cards: Vec<gpui::AnyElement> = equipped
                .iter()
                .enumerate()
                .map(|(i, offer)| {
                    div()
                        .id(("profile-item", i))
                        .flex_1()
                        .min_w_0()
                        .rounded(px(12. * s))
                        .overflow_hidden()
                        .border_1()
                        .border_color(rgb(tone(0x2a2b30)))
                        .bg(rgba(0x0b0b0dc4))
                        .cursor_pointer()
                        .hover(|d| d.border_color(rgb(tone(0x4a4c54))))
                        .child(
                            div().h(px(96. * s)).overflow_hidden().child(
                                img(SharedString::from(format!("shop/{}.png", offer.id)))
                                    .size_full()
                                    .object_fit(ObjectFit::Cover),
                            ),
                        )
                        .child(
                            div()
                                .px(px(12. * s))
                                .py(px(10. * s))
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(15. * s))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(tr(offer.name)),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(12. * s))
                                        .text_color(rgb(MUTED))
                                        .child(tr(offer.blurb)),
                                ),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.page = Page::Shop;
                            cx.notify();
                        }))
                        .into_any_element()
                })
                .collect();
            while !cards.len().is_multiple_of(4) {
                cards.push(
                    div()
                        .flex_1()
                        .min_w_0()
                        .border_1()
                        .border_color(rgba(0))
                        .into_any_element(),
                );
            }
            let mut grid = div().flex().flex_col().gap(px(12. * s));
            let mut cards = cards.into_iter().peekable();
            while cards.peek().is_some() {
                grid = grid.child(
                    div()
                        .flex()
                        .gap(px(12. * s))
                        .children(cards.by_ref().take(4)),
                );
            }
            page = page.child(grid);
        }

        // Every saved account: the one that plays first.
        let active = self.account.as_ref().map(|a| a.account.uuid.clone());
        let mut accounts = self.accounts.clone();
        accounts.sort_by_key(|a| Some(&a.uuid) != active.as_ref());
        page = page.child(
            section("user-multiple-02", "Accounts", accounts.len().to_string()).child(
                div().flex().gap(px(8. * s)).child(
                    self.profile_button("add-account-more", "Add account", "plus", false)
                        .when(!self.auth_busy, |d| {
                            d.on_click(cx.listener(|this, _, _, cx| this.sign_in(cx)))
                        }),
                ),
            ),
        );
        for (index, a) in accounts.iter().enumerate() {
            let on = Some(&a.uuid) == active.as_ref();
            let (switch_to, forget) = (a.uuid.clone(), a.uuid.clone());
            page = page.child(
                div()
                    .id(("account-row", index))
                    .p(px(12. * s))
                    .rounded(px(12. * s))
                    .border_1()
                    .border_color(rgba(if on { 0xffffffcc } else { 0xffffff1a }))
                    .bg(rgba(0x0b0b0dc4))
                    .flex()
                    .items_center()
                    .gap(px(14. * s))
                    .child(
                        div()
                            .size(px(44. * s))
                            .flex_none()
                            .rounded(px(8. * s))
                            .overflow_hidden()
                            .bg(rgb(tone(0x232429)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when_some(self.avatar.clone().filter(|_| on), |d, avatar| {
                                d.child(img(avatar).size_full())
                            })
                            .when(!on || self.avatar.is_none(), |d| {
                                d.child(icon("user", MUTED).size(px(22. * s)))
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(16. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(a.name.clone()),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .font_family("Geist Mono")
                                    .text_size(px(12. * s))
                                    .text_color(rgb(MUTED))
                                    .child(a.uuid.clone()),
                            ),
                    )
                    .when(on, |d| {
                        d.child(
                            div()
                                .px(px(10. * s))
                                .py(px(4. * s))
                                .rounded(px(6. * s))
                                .bg(rgba(0x4ade8026))
                                .text_size(px(12. * s))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(0x4ade80))
                                .child(tr("Playing")),
                        )
                    })
                    .when(!on, |d| {
                        d.child(
                            self.profile_button(("use-account", index), "Use", "refresh", true)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.switch_account(switch_to.clone(), cx)
                                })),
                        )
                    })
                    .child(
                        self.profile_button(("remove-account", index), "Remove", "log-out", false)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.remove_account(forget.clone(), cx)
                            })),
                    ),
            );
        }
        if accounts.is_empty() {
            page = page.child(quiet(
                "No account yet. Add the Microsoft account that owns Minecraft: Java Edition.",
            ));
        }
        page = page.child(
            div()
                .flex()
                .items_center()
                .gap(px(8. * s))
                .text_size(px(12. * s))
                .text_color(rgb(tone(0x6b6e77)))
                .child(icon("shield-check", tone(0x6b6e77)).size(px(14. * s)))
                .child(tr(if cfg!(windows) {
                    "Accounts are kept encrypted for your Windows user. Your password only ever goes to Microsoft."
                } else {
                    "Sign-ins last until Wisp closes. Tokens are never saved in plain text."
                })),
        );
        div().size_full().child(
            div()
                .id("profile-scroll")
                .size_full()
                .overflow_y_scroll()
                .child(page),
        )
    }
}
