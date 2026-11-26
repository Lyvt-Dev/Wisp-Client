//! The Locker: the player's own skin wearing the items they own, drawn by
//! `wisp_cosmetics::preview` from any side. Picking items tries them on in the picture; "Equip
//! these" makes them what the player wears in game.
use super::reference_views::glow;
use super::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use wisp_cosmetics::{
    preview::{self, Draw},
    Item, Texture,
};
use wisp_launcher::shop::{self, Kind, Offer};

/// The picture's size in pixels (drawn on the CPU, then scaled to fit).
const PICTURE: u32 = 720;
const ANGLES: [(f32, &str); 4] = [
    (0., "Front"),
    (90., "Side"),
    (180., "Back"),
    (270., "Other side"),
];

#[derive(Default)]
pub(super) struct LockerUi {
    /// Turned this many degrees from the front.
    pub yaw: f32,
    /// What is tried on; None follows what is equipped.
    pub trying: Option<Vec<String>>,
    pub filter: Option<Kind>,
    /// The last picture and what it showed.
    picture: RefCell<Option<(u64, Arc<gpui::RenderImage>)>>,
    /// Whether an item has a model to draw (trails, auras and emotes show only in game).
    drawn: RefCell<HashMap<&'static str, bool>>,
}

fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Cape => "Capes",
        Kind::Wings => "Wings",
        Kind::Weapon => "Weapons",
        Kind::Bag => "Bags",
        Kind::Head => "Hats",
        Kind::Glasses => "Glasses",
        Kind::Pet => "Pets",
        Kind::Trail => "Trails",
        Kind::Aura => "Auras",
        Kind::Emote => "Emotes",
    }
}

/// A plain stand-in skin when the account has none (or isn't signed in).
pub(super) fn stand_in_skin() -> Texture {
    let mut rgba = vec![0u8; 64 * 64 * 4];
    let mut fill = |x0: usize, y0: usize, w: usize, h: usize, c: [u8; 3]| {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                let i = (y * 64 + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
    };
    fill(0, 0, 32, 16, [0x6b, 0x6e, 0x77]); // head
    fill(8, 8, 8, 8, [0xb8, 0x8f, 0x74]); // face
    fill(16, 16, 24, 16, [0x3a, 0x3d, 0x46]); // body
    fill(40, 16, 16, 16, [0x46, 0x49, 0x52]); // right arm
    fill(32, 48, 16, 16, [0x46, 0x49, 0x52]); // left arm
    fill(0, 16, 16, 16, [0x24, 0x26, 0x2c]); // right leg
    fill(16, 48, 16, 16, [0x24, 0x26, 0x2c]); // left leg
    Texture {
        width: 64,
        height: 64,
        rgba,
    }
}

/// Straight-alpha RGBA as the BGRA, premultiplied picture GPUI draws.
pub(super) fn to_render(image: &preview::Image) -> Arc<gpui::RenderImage> {
    let mut buf = image::RgbaImage::new(image.width, image.height);
    for (px, src) in buf.pixels_mut().zip(image.rgba.as_chunks::<4>().0) {
        let a = src[3] as u32;
        let pre = |c: u8| (c as u32 * a / 255) as u8;
        *px = image::Rgba([pre(src[2]), pre(src[1]), pre(src[0]), src[3]]);
    }
    Arc::new(gpui::RenderImage::new([image::Frame::new(buf)]))
}

impl Launcher {
    /// What the picture shows: tried on, or else what is worn.
    fn locker_selection(&self) -> Vec<String> {
        self.locker
            .trying
            .clone()
            .unwrap_or_else(|| self.wallet.equipped.clone())
    }

    fn locker_drawn(&self, offer: &'static Offer) -> bool {
        *self
            .locker
            .drawn
            .borrow_mut()
            .entry(offer.id)
            .or_insert_with(|| {
                offer
                    .item
                    .and_then(|bytes| Item::parse(bytes).ok())
                    .is_some_and(|item| item.roots().next().is_some())
            })
    }

    /// The player wearing the selection, drawn again only when something changed.
    fn locker_picture(&self, selection: &[String]) -> Arc<gpui::RenderImage> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.avatar_key.hash(&mut hasher);
        self.skin.is_some().hash(&mut hasher);
        self.locker.yaw.to_bits().hash(&mut hasher);
        selection.hash(&mut hasher);
        let key = hasher.finish();
        if let Some((k, picture)) = self.locker.picture.borrow().as_ref() {
            if *k == key {
                return picture.clone();
            }
        }
        let (texture, slim) = match &self.skin {
            Some((rgba, slim)) => (
                Texture {
                    width: 64,
                    height: 64,
                    rgba: rgba.clone(),
                },
                *slim,
            ),
            None => (stand_in_skin(), false),
        };
        let body = preview::player_model(slim);
        let items: Vec<Item> = selection
            .iter()
            .filter_map(|id| shop::offer(id)?.item)
            .filter_map(|bytes| Item::parse(bytes).ok())
            .collect();
        let mut draws = vec![Draw {
            root: &body,
            base: glam::Mat4::IDENTITY,
            texture: &texture,
            poses: Vec::new(),
        }];
        for item in &items {
            draws.extend(Draw::item(item, &body, 0.0));
        }
        let picture = to_render(&preview::render(
            &draws,
            PICTURE,
            PICTURE,
            self.locker.yaw,
            8.0,
        ));
        *self.locker.picture.borrow_mut() = Some((key, picture.clone()));
        picture
    }

    /// Tries `id` on (taking off what shares its place) or takes it off again.
    fn locker_toggle(&mut self, id: &'static str, cx: &mut Context<Self>) {
        let mut selection = self.locker_selection();
        if let Some(at) = selection.iter().position(|s| s == id) {
            selection.remove(at);
        } else if let Some(offer) = shop::offer(id) {
            if offer.kind != Kind::Emote {
                selection.retain(|s| {
                    shop::offer(s)
                        .is_none_or(|o| o.kind == Kind::Emote || !o.kind.shares_slot(offer.kind))
                });
            }
            selection.push(id.to_owned());
        }
        self.locker.trying = Some(selection);
        cx.notify();
    }

    /// Makes the selection what is worn in game.
    fn locker_equip(&mut self, cx: &mut Context<Self>) {
        if let Some(why) = self.shop_state.clone() {
            self.error(why);
            cx.notify();
            return;
        }
        // The Wisp server keeps what is worn; its answer updates the wallet.
        let selection: Vec<String> = self
            .locker_selection()
            .iter()
            .map(|id| id.to_string())
            .collect();
        self.friends_send(super::friends_view::FriendCmd::Outfit(
            selection,
            tr("Equipped. You'll wear these the next time you play."),
        ));
        self.locker.trying = None;
        self.failed = false;
        self.status = tr("Equipped. You'll wear these the next time you play.");
        cx.notify();
    }

    fn locker_button(
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
                d.border_1()
                    .border_color(rgb(tone(0x2e3035)))
                    .hover(|d| d.bg(rgb(tone(0x202125))))
            })
            .child(icon(glyph, if bright { 0x0b0b0d } else { MUTED }).size(px(15. * s)))
            .child(tr(label))
    }

    pub(super) fn locker_page(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let owned: Vec<&'static Offer> = shop::OFFERS
            .iter()
            .filter(|o| self.wallet.owns(o.id))
            .collect();
        let selection = self.locker_selection();
        let changed = self.locker.trying.as_ref().is_some_and(|t| {
            let mut a = t.clone();
            let mut b = self.wallet.equipped.clone();
            a.sort();
            b.sort();
            a != b
        });
        let summary = trf(
            "{owned} items owned, {worn} equipped. Pick items to try them on, then equip the look you like.",
            &[
                ("owned", &owned.len().to_string()),
                ("worn", &self.wallet.equipped.len().to_string()),
            ],
        );
        let actions = div()
            .flex()
            .items_center()
            .gap(px(10. * s))
            .when(changed, |d| {
                d.child(
                    self.locker_button("locker-reset", "Reset", "refresh", false)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.locker.trying = None;
                            cx.notify();
                        })),
                )
            })
            .child(
                self.locker_button(
                    "locker-equip",
                    if changed { "Equip these" } else { "Equipped" },
                    "check-circle",
                    changed,
                )
                .when(!changed, |d| d.opacity(0.6))
                .when(changed, |d| {
                    d.on_click(cx.listener(|this, _, _, cx| this.locker_equip(cx)))
                }),
            );
        let header = self.page_head(
            ("Back to Home", Page::Home),
            "Cosmetics",
            &summary,
            Some(actions),
            cx,
        );
        if owned.is_empty() {
            return div()
                .flex()
                .flex_col()
                .gap(px(16. * s))
                .child(header)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(12. * s))
                        .child(self.empty_state(
                            "cape",
                            "Your locker is empty",
                            "Items you buy or claim in the shop show up here to try on.",
                        ))
                        .child(
                            self.locker_button("locker-shop", "Open the shop", "cart", true)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.page = Page::Shop;
                                    cx.notify();
                                })),
                        ),
                );
        }
        // The picture with its turn controls.
        let picture = self.locker_picture(&selection);
        let worn_names: Vec<String> = selection
            .iter()
            .filter_map(|id| shop::offer(id))
            .map(|o| tr(o.name))
            .collect();
        let stage = div()
            .w(relative(0.42))
            .flex_none()
            .h(px(640. * s))
            .rounded(px(14. * s))
            .overflow_hidden()
            .border_1()
            .border_color(rgb(tone(RULE)))
            .bg(gpui::linear_gradient(
                180.,
                gpui::linear_color_stop(rgb(tone(0x1b1c21)), 0.),
                gpui::linear_color_stop(rgb(tone(0x0e0f11)), 1.),
            ))
            .flex()
            .flex_col()
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .absolute()
                            .bottom(px(54. * s))
                            .left(relative(0.25))
                            .w(relative(0.5))
                            .h(px(26. * s))
                            .rounded_full()
                            .bg(rgba(accent_tone_a(0x3ccf6a26)))
                            .shadow(glow(s)),
                    )
                    .child(img(picture).size(px(500. * s)))
                    .when(self.skin.is_none(), |d| {
                        d.child(
                            div()
                                .absolute()
                                .top(px(14. * s))
                                .left(px(14. * s))
                                .right(px(14. * s))
                                .text_size(px(12. * s))
                                .text_color(rgb(MUTED))
                                .child(tr("Sign in to see your own skin. A stand-in wears the items for now.")),
                        )
                    }),
            )
            .child(
                div()
                    .p(px(12. * s))
                    .flex()
                    .flex_col()
                    .gap(px(10. * s))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(6. * s))
                            .child(
                                self.locker_button("locker-left", "", "chevron-left", false)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.locker.yaw = (this.locker.yaw + 315.) % 360.;
                                        cx.notify();
                                    })),
                            )
                            .children(ANGLES.iter().enumerate().map(|(i, (yaw, label))| {
                                let on = (self.locker.yaw - yaw).abs() < 0.5;
                                let yaw = *yaw;
                                div()
                                    .id(("locker-angle", i))
                                    .h(px(34. * s))
                                    .px(px(12. * s))
                                    .rounded(px(8. * s))
                                    .flex()
                                    .items_center()
                                    .text_size(px(13. * s))
                                    .cursor_pointer()
                                    .when(on, |d| d.bg(rgb(tone(0x2a2b30))).font_weight(FontWeight::SEMIBOLD))
                                    .when(!on, |d| d.text_color(rgb(0xc4c6cc)).hover(|d| d.bg(rgb(tone(0x202125)))))
                                    .child(tr(label))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.locker.yaw = yaw;
                                        cx.notify();
                                    }))
                            }))
                            .child(
                                self.locker_button("locker-right", "", "chevron-right", false)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.locker.yaw = (this.locker.yaw + 45.) % 360.;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_center()
                            .truncate()
                            .text_size(px(12. * s))
                            .text_color(rgb(MUTED))
                            .child(if worn_names.is_empty() {
                                tr("Wearing nothing. Pick items on the right.")
                            } else {
                                trf("Wearing: {items}", &[("items", &worn_names.join(", "))])
                            }),
                    ),
            );
        // The owned items, by kind.
        let kinds: Vec<Kind> = Kind::ALL
            .into_iter()
            .filter(|k| owned.iter().any(|o| o.kind == *k))
            .collect();
        let tab = |id: usize, label: String, on: bool| {
            div()
                .id(("locker-tab", id))
                .h(px(34. * s))
                .px(px(12. * s))
                .rounded(px(8. * s))
                .flex()
                .items_center()
                .text_size(px(13. * s))
                .cursor_pointer()
                .when(on, |d| {
                    d.bg(rgb(tone(0x232429))).font_weight(FontWeight::SEMIBOLD)
                })
                .when(!on, |d| {
                    d.text_color(rgb(0xc4c6cc))
                        .hover(|d| d.bg(rgb(tone(0x1c1d21))))
                })
                .child(label)
        };
        let mut tabs = vec![tab(
            99,
            trf("All {n}", &[("n", &owned.len().to_string())]),
            self.locker.filter.is_none(),
        )
        .on_click(cx.listener(|this, _, _, cx| {
            this.locker.filter = None;
            cx.notify();
        }))];
        for (i, kind) in kinds.iter().copied().enumerate() {
            let n = owned.iter().filter(|o| o.kind == kind).count();
            tabs.push(
                tab(
                    i,
                    format!("{} {n}", tr(kind_label(kind))),
                    self.locker.filter == Some(kind),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.locker.filter = Some(kind);
                    cx.notify();
                })),
            );
        }
        let shown: Vec<&'static Offer> = owned
            .iter()
            .copied()
            .filter(|o| self.locker.filter.is_none_or(|k| o.kind == k))
            .collect();
        let mut cards: Vec<gpui::AnyElement> = shown
            .iter()
            .map(|offer| {
                let id = offer.id;
                let trying = selection.iter().any(|s| s == id);
                let worn = self.wallet.wears(id);
                let drawn = self.locker_drawn(offer);
                div()
                    .id(SharedString::from(format!("locker-item-{id}")))
                    .flex_1()
                    .min_w_0()
                    .rounded(px(12. * s))
                    .overflow_hidden()
                    .border_2()
                    .border_color(rgb(if trying {
                        accent_tone(ACCENT)
                    } else {
                        tone(0x2a2b30)
                    }))
                    .bg(rgb(tone(0x17181b)))
                    .cursor_pointer()
                    .hover(|d| {
                        d.border_color(rgb(if trying {
                            accent_tone(ACCENT)
                        } else {
                            tone(0x4a4c54)
                        }))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.locker_toggle(id, cx)))
                    .child(
                        div()
                            .relative()
                            .h(px(96. * s))
                            .overflow_hidden()
                            .child(
                                img(SharedString::from(format!("shop/{id}.png")))
                                    .size_full()
                                    .object_fit(ObjectFit::Cover),
                            )
                            .when(trying, |d| {
                                d.child(
                                    div()
                                        .absolute()
                                        .top(px(6. * s))
                                        .right(px(6. * s))
                                        .size(px(22. * s))
                                        .rounded_full()
                                        .bg(rgb(0xffffff))
                                        .text_color(rgb(0x0b0b0d))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            icon("checkmark-circle-01", 0x0b0b0d).size(px(14. * s)),
                                        ),
                                )
                            })
                            .when(worn, |d| {
                                d.child(
                                    div()
                                        .absolute()
                                        .top(px(6. * s))
                                        .left(px(6. * s))
                                        .px(px(7. * s))
                                        .py(px(2. * s))
                                        .rounded(px(5. * s))
                                        .bg(rgba(tone_a(0x0e0f11d0)))
                                        .text_size(px(11. * s))
                                        .text_color(rgb(accent_tone(0x6fe792)))
                                        .child(tr("Equipped")),
                                )
                            }),
                    )
                    .child(
                        div()
                            .px(px(10. * s))
                            .py(px(8. * s))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(14. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(tr(offer.name)),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(11. * s))
                                    .text_color(rgb(MUTED))
                                    .child(if drawn {
                                        tr(kind_label(offer.kind))
                                    } else {
                                        tr("Shows in game only")
                                    }),
                            ),
                    )
                    .into_any_element()
            })
            .collect();
        while !cards.len().is_multiple_of(3) {
            cards.push(
                div()
                    .flex_1()
                    .min_w_0()
                    .border_2()
                    .border_color(rgba(0))
                    .into_any_element(),
            );
        }
        let mut grid = div().flex().flex_col().gap(px(10. * s));
        let mut cards = cards.into_iter().peekable();
        while cards.peek().is_some() {
            grid = grid.child(
                div()
                    .flex()
                    .gap(px(10. * s))
                    .children(cards.by_ref().take(3)),
            );
        }
        let page = div()
            .flex()
            .flex_col()
            .gap(px(16. * s))
            .pb(px(20. * s))
            .child(header)
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(18. * s))
                    .child(stage)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(12. * s))
                            .child(div().flex().flex_wrap().gap(px(4. * s)).children(tabs))
                            .child(grid)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8. * s))
                                    .text_size(px(12. * s))
                                    .text_color(rgb(tone(0x6b6e77)))
                                    .child(icon("info", tone(0x6b6e77)).size(px(13. * s)))
                                    .child(tr("Trails, auras and emotes play in game; the picture shows what you wear on your body.")),
                            ),
                    ),
            );
        div().size_full().child(
            div()
                .id("locker-scroll")
                .size_full()
                .overflow_y_scroll()
                .child(page),
        )
    }
}
