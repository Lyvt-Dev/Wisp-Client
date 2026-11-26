//! The friends panel and the pieces the window shell shares (avatar, instance menu, plates),
//! after the owner's launcher reference. The frame and Home are in `shell.rs`.
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
fn dot(on: bool, s: f32) -> Div {
    div()
        .size(px(9. * s))
        .flex_none()
        .rounded_full()
        .bg(rgb(if on { accent_tone(0x3ccf6a) } else { 0x6b6d75 }))
}

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

    pub(super) fn avatar(&self, size: f32) -> Div {
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

    /// Social: the friends panel over the right of the page, opened from the top bar. The
    /// player, then adding friends, requests, friends, daily coins and servers.
    pub(super) fn friends_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = self.ui_scale;
        let presence = if self.child.is_some() {
            tr("Playing Wisp")
        } else if self.account.is_some() {
            tr("Online in launcher")
        } else {
            tr("Not signed in")
        };
        let online = self.friends.as_ref().map_or(0, |f| f.online());
        div()
            .id("friends-panel")
            .w(px(360. * s))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(20. * s))
            .border_1()
            .border_color(rgba(0xffffff1a))
            .bg(rgba(0x111215f5))
            .shadow(vec![gpui::BoxShadow {
                color: rgba(0x00000099).into(),
                offset: gpui::point(px(0.), px(16. * s)),
                blur_radius: px(48. * s),
                spread_radius: px(0.),
                inset: false,
            }])
            .occlude()
            .child(
                div()
                    .flex_none()
                    .px(px(20. * s))
                    .pt(px(18. * s))
                    .pb(px(12. * s))
                    .flex()
                    .items_center()
                    .gap(px(10. * s))
                    .child(
                        div()
                            .text_size(px(24. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("Social")),
                    )
                    .child(
                        div()
                            .px(px(10. * s))
                            .py(px(3. * s))
                            .rounded_full()
                            .bg(rgba(0xffffff12))
                            .flex()
                            .items_center()
                            .gap(px(6. * s))
                            .text_size(px(12. * s))
                            .text_color(rgb(0xd9dadd))
                            .child(div().size(px(7. * s)).rounded_full().bg(rgb(if online > 0 {
                                0x4ade80
                            } else {
                                0x5b5d65
                            })))
                            .child(trf("{count} online", &[("count", &online.to_string())])),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("friends-close")
                            .size(px(32. * s))
                            .rounded(px(9. * s))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|d| d.bg(rgba(0xffffff14)))
                            .child(icon("close", INK).size(px(17. * s)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_prefs(|p| p.side_panel = false, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .id("friends-self")
                    .mx(px(14. * s))
                    .px(px(12. * s))
                    .py(px(12. * s))
                    .rounded(px(14. * s))
                    .bg(rgba(0xffffff0a))
                    .border_1()
                    .border_color(rgba(0xffffff12))
                    .flex()
                    .items_center()
                    .gap(px(12. * s))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgba(0xffffff14)))
                    .child(
                        div().relative().child(self.avatar(44.)).child(
                            div()
                                .absolute()
                                .right(px(-3. * s))
                                .bottom(px(-3. * s))
                                .p(px(2.5 * s))
                                .rounded_full()
                                .bg(rgb(0x121316))
                                .child(div().size(px(10. * s)).rounded_full().bg(rgb(
                                    if self.account.is_some() {
                                        0x4ade80
                                    } else {
                                        0x5b5d65
                                    },
                                ))),
                        ),
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
                                    .child(self.account_name()),
                            )
                            .child(
                                div()
                                    .text_size(px(13. * s))
                                    .text_color(rgb(if self.account.is_some() {
                                        0x4ade80
                                    } else {
                                        0xa9abb3
                                    }))
                                    .child(presence),
                            ),
                    )
                    .child(icon("arrow", 0x8e9099).size(px(15. * s)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Account;
                        cx.notify();
                    })),
            )
            .child(self.side_sections(cx))
    }

    /// The instance list that opens under the Launch button.
    pub(super) fn instance_menu(&self, cx: &mut Context<Self>) -> Div {
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
            .top(px(62. * s))
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
