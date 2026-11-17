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

    /// The friends column on the right. The launcher has no friends service: it shows the
    /// player and how to play together through invite codes.
    pub(super) fn friends_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = self.ui_scale;
        let presence = if self.child.is_some() {
            tr("Playing Wisp")
        } else if self.account.is_some() {
            tr("Online in launcher")
        } else {
            tr("Not signed in")
        };
        div()
            .id("friends-panel")
            .w(px(340. * s))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(18. * s))
            .border_1()
            .border_color(rgba(0xffffff1a))
            .bg(rgba(0x111215f2))
            .shadow(vec![gpui::BoxShadow {
                color: rgba(0x00000080).into(),
                offset: gpui::point(px(0.), px(12. * s)),
                blur_radius: px(40. * s),
                spread_radius: px(0.),
                inset: false,
            }])
            .occlude()
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
