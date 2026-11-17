//! The Store: Wisp Coins and the items for your Wisp player (`wisp_launcher::shop`).
//!
//! Laid out like Home: the chosen item large on the left over its own picture (the window's
//! backdrop), with its button and price, the coins and the creator code under it; every item
//! stacked on the right, with search and the kinds there are.
use super::friends_view::FriendCmd;
use super::locker_view::{stand_in_skin, to_render};
use super::*;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use wisp_cosmetics::{
    preview::{self, Draw},
    Item, Texture,
};
use wisp_launcher::{
    catalog,
    shop::{self, Kind, Offer},
};

/// The figure's picture in pixels (drawn on the CPU, then scaled to fit), and the step its
/// turning is drawn in (finer steps would only be drawn more often).
const FIGURE: u32 = 640;
const FIGURE_STEP: f32 = 3.0;
/// Where the figure starts: its back, a little turned, so a cape shows.
const FIGURE_START: f32 = 200.0;

pub(super) struct ShopUi {
    /// The kind the list shows; None: every kind.
    pub tab: Option<Kind>,
    /// The item shown large (its id); None: the first.
    pub selected: Option<&'static str>,
    /// What the last purchase or change did, shown under the button.
    pub notice: Option<(String, bool)>,
    /// A purchase or change is with the Wisp server: the buttons wait for its answer.
    pub busy: bool,
    /// The figure turned this many degrees from the front.
    pub yaw: f32,
    /// Where a drag that turns the figure was at the last move (window x, pixels).
    pub drag: Option<f32>,
    /// The last picture of the figure and what it showed.
    figure: RefCell<Option<(u64, Arc<gpui::RenderImage>)>>,
}

impl Default for ShopUi {
    fn default() -> Self {
        Self {
            tab: None,
            selected: None,
            notice: None,
            busy: false,
            yaw: FIGURE_START,
            drag: None,
            figure: RefCell::new(None),
        }
    }
}

const GOLD: u32 = 0xf2c14e;

fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Cape => "Capes",
        Kind::Wings => "Wings",
        Kind::Weapon => "Weapons",
        Kind::Head => "Hats",
        Kind::Pet => "Pets",
        Kind::Trail => "Trails",
        Kind::Aura => "Auras",
        Kind::Emote => "Emotes",
    }
}

/// One of a kind, for an item's line.
fn kind_one(kind: Kind) -> &'static str {
    match kind {
        Kind::Cape => "Cape",
        Kind::Wings => "Wings",
        Kind::Weapon => "Weapon",
        Kind::Head => "Hat",
        Kind::Pet => "Pet",
        Kind::Trail => "Trail",
        Kind::Aura => "Aura",
        Kind::Emote => "Emote",
    }
}

/// An item's tier by its price: name and colour.
fn tier(offer: &Offer) -> (&'static str, u32) {
    if offer.founders() {
        ("Founders", GOLD)
    } else if offer.price >= 1500 {
        ("Legendary", 0xf59e0b)
    } else if offer.price >= 1000 {
        ("Epic", 0xa66bff)
    } else if offer.price >= 400 {
        ("Rare", 0x3d8bfd)
    } else {
        ("Common", 0x9aa0a8)
    }
}

/// Coins as the shop writes them: 1,200.
fn coins(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A glass plate, as Home's cards.
fn glass(s: f32) -> Div {
    div()
        .rounded(px(16. * s))
        .bg(rgba(0x0b0b0dc4))
        .border_1()
        .border_color(rgba(0xffffff1f))
}

/// An item's price: with a creator's `discount` (percent) the lower price, the full one struck
/// through beside it. The Wisp server charges the same.
fn price(offer: &Offer, discount: u32, size: f32, s: f32) -> Div {
    let paid = shop::discounted(offer.price, discount);
    div()
        .flex()
        .items_center()
        .gap(px(7. * s))
        .text_size(px(size * s))
        .font_weight(FontWeight::SEMIBOLD)
        .when(offer.founders(), |d| d.child(tr("Free")))
        .when(!offer.founders(), |d| {
            d.child(icon("coin", GOLD).size(px((size + 1.) * s)))
                .child(coins(paid))
                .when(paid < offer.price, |d| {
                    d.child(
                        div()
                            .text_size(px(size * 0.7 * s))
                            .text_color(rgb(MUTED))
                            .line_through()
                            .child(coins(offer.price)),
                    )
                })
        })
}

impl Launcher {
    /// The item shown large: the chosen one, else the first the shop sells.
    pub(super) fn shop_selected(&self) -> Option<&'static Offer> {
        self.shop
            .selected
            .and_then(shop::offer)
            .or_else(|| shop::OFFERS.first())
    }

    /// Buys, equips or takes off `id` on the Wisp server, which keeps the coins and checks
    /// every purchase; its answer updates the wallet (`FriendsUpdate::Wallet`).
    fn shop_act(&mut self, id: &'static str, cx: &mut Context<Self>) {
        if self.shop.busy {
            return;
        }
        if let Some(why) = &self.shop_state {
            self.shop.notice = Some((why.clone(), true));
            cx.notify();
            return;
        }
        let emote = shop::offer(id).is_some_and(|o| o.kind == Kind::Emote);
        let command = if self.wallet.wears(id) {
            FriendCmd::Unequip(
                id.to_owned(),
                tr(if emote {
                    "Taken out of your emote wheel."
                } else {
                    "Taken off. You won't wear it the next time you play."
                }),
            )
        } else if self.wallet.owns(id) {
            FriendCmd::Equip(
                id.to_owned(),
                tr(if emote {
                    "In your emote wheel: press B in game to play it."
                } else {
                    "Equipped. You'll wear it the next time you play."
                }),
            )
        } else {
            FriendCmd::Buy(
                id.to_owned(),
                tr(if emote {
                    "Bought and added to your emote wheel: press B in game to play it."
                } else {
                    "Added to your items and equipped."
                }),
            )
        };
        self.shop.busy = true;
        self.shop.notice = Some((tr("Asking the Wisp server…"), false));
        self.friends_send(command);
        cx.notify();
    }

    /// The chosen item's main action, a white pill as Home's Launch: buy, claim, equip or take
    /// off.
    fn shop_button(&self, offer: &'static Offer, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = self.ui_scale;
        let short = !self.wallet.owns(offer.id)
            && !offer.founders()
            && self.wallet.coins < shop::discounted(offer.price, self.server_wallet.discount);
        let (label, glyph) = if self.wallet.wears(offer.id) {
            ("Take off", "check-circle")
        } else if self.wallet.owns(offer.id) {
            ("Equip", "star")
        } else if offer.founders() {
            ("Claim free", "cape")
        } else {
            ("Buy", "cart")
        };
        let id = offer.id;
        div()
            .id(SharedString::from(format!("shop-act-{id}")))
            .h(px(62. * s))
            .pl(px(8. * s))
            .pr(px(34. * s))
            .rounded_full()
            .bg(rgb(0xffffff))
            .flex()
            .items_center()
            .gap(px(14. * s))
            .text_size(px(20. * s))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0x0b0b0d))
            .child(
                div()
                    .size(px(46. * s))
                    .rounded_full()
                    .bg(rgb(0x0b0b0d))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(glyph, 0xffffff).size(px(19. * s))),
            )
            .child(tr(label))
            .when(short || self.shop.busy, |d| d.opacity(0.55))
            .cursor_pointer()
            .hover(|d| d.bg(rgb(0xe4e5e8)))
            .on_click(cx.listener(move |this, _, _, cx| this.shop_act(id, cx)))
    }

    /// The coins: the balance and the daily gift, as a glass card beside the creator code.
    fn shop_coins(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = self.ui_scale;
        let collected = self.wallet.bonus_day == Some(shop::today());
        glass(s)
            .id("shop-coins")
            .w(px(250. * s))
            .flex_none()
            .p(px(14. * s))
            .flex()
            .items_center()
            .gap(px(14. * s))
            .border_color(rgba(0xf2c14e40))
            .cursor_pointer()
            .hover(|d| d.border_color(rgba(0xf2c14e80)))
            .child(
                div()
                    .size(px(44. * s))
                    .flex_none()
                    .rounded_full()
                    .bg(rgba(0xf2c14e26))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon("coin", GOLD).size(px(22. * s))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(22. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(coins(self.wallet.coins)),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(13. * s))
                            .text_color(rgb(0xd8c58f))
                            .child(tr(if collected {
                                "Daily +100 collected"
                            } else {
                                "+100 every day"
                            })),
                    ),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.shop.notice = Some((
                    trf(
                        "Wisp Coins are earned, never sold: you get {daily} each day you open Wisp.",
                        &[("daily", &shop::DAILY.to_string())],
                    ),
                    false,
                ));
                cx.notify();
            }))
    }

    /// The creator code: who is supported, or a field to enter a code.
    fn shop_creator(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let card = glass(s)
            .flex_1()
            .min_w_0()
            .max_w(px(460. * s))
            .p(px(14. * s))
            .flex()
            .items_center()
            .gap(px(12. * s))
            .child(
                div()
                    .size(px(44. * s))
                    .flex_none()
                    .rounded_full()
                    .bg(rgba(0xffffff14))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon("star", INK).size(px(20. * s))),
            );
        let small = |id: &'static str, label: &str| {
            div()
                .id(id)
                .flex_none()
                .h(px(36. * s))
                .px(px(14. * s))
                .rounded_full()
                .bg(rgba(0xffffff14))
                .border_1()
                .border_color(rgba(0xffffff2e))
                .flex()
                .items_center()
                .text_size(px(14. * s))
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .hover(|d| d.bg(rgba(0xffffff26)))
                .child(tr(label))
        };
        match &self.server_wallet.supporting {
            Some((name, _)) => card
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .truncate()
                                .text_size(px(17. * s))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(trf("Supporting {name}", &[("name", name)])),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(13. * s))
                                .text_color(rgb(0xb4b6bd))
                                .child(trf(
                                    "{percent} % off everything",
                                    &[("percent", &self.server_wallet.discount.to_string())],
                                )),
                        ),
                )
                .child(small("shop-creator-clear", "Remove").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.shop.notice = Some((tr("Asking the Wisp server…"), false));
                        this.friends_send(FriendCmd::Support(None));
                        cx.notify();
                    },
                ))),
            None => card
                .child(div().flex_1().min_w_0().child(self.creator_input.clone()))
                .child(small("shop-creator-use", "Use code").on_click(cx.listener(
                    |this, _, _, cx| {
                        let code = this.creator_input.read(cx).value.trim().to_owned();
                        if code.is_empty() {
                            return;
                        }
                        this.creator_input.update(cx, |input, cx| input.set("", cx));
                        this.shop.notice = Some((tr("Asking the Wisp server…"), false));
                        this.friends_send(FriendCmd::Support(Some(code)));
                        cx.notify();
                    },
                ))),
        }
    }

    /// The items stacked on the right: search, the kinds there are, then each item with its
    /// picture, kind, price and whether it is owned or worn; the one shown large is outlined.
    fn shop_list(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let query = self.shop_search.read(cx).value.clone();
        let chosen = self.shop_selected().map(|o| o.id);
        let kinds: Vec<Kind> = Kind::ALL
            .into_iter()
            .filter(|k| shop::OFFERS.iter().any(|o| o.kind == *k))
            .collect();
        let offers: Vec<&'static Offer> = shop::OFFERS
            .iter()
            .filter(|o| self.shop.tab.is_none_or(|k| o.kind == k))
            .filter(|o| catalog::matches(&query, &[o.name, o.blurb, o.id]))
            .collect();
        let pill = |id: (&'static str, usize), label: String, on: bool| {
            div()
                .id(id)
                .h(px(32. * s))
                .px(px(12. * s))
                .rounded_full()
                .flex()
                .items_center()
                .text_size(px(13. * s))
                .cursor_pointer()
                .when(on, |d| {
                    d.bg(rgb(0xffffff))
                        .text_color(rgb(0x0b0b0d))
                        .font_weight(FontWeight::SEMIBOLD)
                })
                .when(!on, |d| {
                    d.text_color(rgb(0xc4c6cc))
                        .hover(|d| d.bg(rgba(0xffffff14)))
                })
                .child(label)
        };
        let mut list = div()
            .id("shop-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(10. * s));
        if offers.is_empty() {
            list = list.child(
                div()
                    .px(px(4. * s))
                    .text_size(px(14. * s))
                    .text_color(rgb(0xa9abb3))
                    .child(tr("No items match your search.")),
            );
        }
        for (i, offer) in offers.iter().enumerate() {
            let on = chosen == Some(offer.id);
            let (tier_name, tier_colour) = tier(offer);
            let state = if self.wallet.wears(offer.id) {
                Some(tr("Equipped"))
            } else if self.wallet.owns(offer.id) {
                Some(tr("Owned"))
            } else {
                None
            };
            let id = offer.id;
            list = list.child(
                div()
                    .id(("shop-item", i))
                    .h(px(96. * s))
                    .flex_none()
                    .p(px(8. * s))
                    .pr(px(16. * s))
                    .rounded(px(18. * s))
                    .bg(rgba(if on { 0x1b1c20f0 } else { 0x0b0b0dc4 }))
                    .border_1()
                    .border_color(rgba(if on { 0xffffffd9 } else { 0xffffff1a }))
                    .flex()
                    .items_center()
                    .gap(px(14. * s))
                    .cursor_pointer()
                    .hover(move |d| d.border_color(rgba(if on { 0xffffffd9 } else { 0xffffff4d })))
                    .child(
                        img(format!("shop/{}.png", offer.id))
                            .w(px(132. * s))
                            .h_full()
                            .flex_none()
                            .rounded(px(12. * s))
                            .object_fit(ObjectFit::Cover),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(3. * s))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(18. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(tr(offer.name)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6. * s))
                                    .text_size(px(12. * s))
                                    .child(
                                        div()
                                            .text_color(rgb(tier_colour))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(tr(tier_name)),
                                    )
                                    .child(div().text_color(rgb(0x6b6d75)).child("·"))
                                    .child(
                                        div()
                                            .text_color(rgb(0xa9abb3))
                                            .child(tr(kind_one(offer.kind))),
                                    ),
                            )
                            .child(match state {
                                Some(state) => div()
                                    .text_size(px(13. * s))
                                    .text_color(rgb(0x4ade80))
                                    .child(state),
                                None => price(offer, self.server_wallet.discount, 14., s),
                            }),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.shop.selected = Some(id);
                        this.shop.notice = None;
                        cx.notify();
                    })),
            );
        }
        div()
            .w(px(420. * s))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(12. * s))
            .child(
                div()
                    .px(px(4. * s))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(13. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xb4b6bd))
                            .child(tr("ALL ITEMS")),
                    )
                    .child(
                        div()
                            .text_size(px(13. * s))
                            .text_color(rgb(0x8e9099))
                            .child(trf(
                                "{owned} of {all} owned",
                                &[
                                    ("owned", &self.wallet.owned.len().to_string()),
                                    ("all", &shop::OFFERS.len().to_string()),
                                ],
                            )),
                    ),
            )
            .child(self.shop_search.clone())
            .when(kinds.len() > 1, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(2. * s))
                        .p(px(4. * s))
                        .rounded(px(22. * s))
                        .bg(rgba(0x0b0b0d8c))
                        .border_1()
                        .border_color(rgba(0xffffff1a))
                        .child(
                            pill(("shop-kind", 99), tr("All"), self.shop.tab.is_none()).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.shop.tab = None;
                                    cx.notify();
                                }),
                            ),
                        )
                        .children(kinds.iter().enumerate().map(|(i, kind)| {
                            let kind = *kind;
                            pill(
                                ("shop-kind", i),
                                tr(kind_label(kind)),
                                self.shop.tab == Some(kind),
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.shop.tab = Some(kind);
                                    cx.notify();
                                },
                            ))
                        })),
                )
            })
            .child(list)
    }

    /// The player's own skin (a stand-in before signing in) wearing `offer`, turned to the
    /// figure's angle; drawn again only when the angle, the skin or the item changes.
    fn shop_picture(&self, offer: &'static Offer) -> Arc<gpui::RenderImage> {
        let yaw = (self.shop.yaw / FIGURE_STEP).round() * FIGURE_STEP;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.avatar_key.hash(&mut hasher);
        self.skin.is_some().hash(&mut hasher);
        yaw.to_bits().hash(&mut hasher);
        offer.id.hash(&mut hasher);
        let key = hasher.finish();
        if let Some((k, picture)) = self.shop.figure.borrow().as_ref() {
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
        let item = offer.item.and_then(|bytes| Item::parse(bytes).ok());
        let mut draws = vec![Draw {
            root: &body,
            base: glam::Mat4::IDENTITY,
            texture: &texture,
            poses: Vec::new(),
        }];
        if let Some(item) = &item {
            draws.extend(Draw::item(item, &body, 0.0));
        }
        let picture = to_render(&preview::render(&draws, FIGURE, FIGURE, yaw, 8.0));
        *self.shop.figure.borrow_mut() = Some((key, picture.clone()));
        picture
    }

    /// The figure in the middle of the Store: drag across it to turn it; the arrows turn it a
    /// quarter, Front and Back face it straight.
    fn shop_figure(&self, offer: &'static Offer, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let size = px(560. * s);
        let round = |id: &'static str, glyph: &'static str| {
            div()
                .id(id)
                .size(px(40. * s))
                .rounded_full()
                .bg(rgba(0x0b0b0d99))
                .border_1()
                .border_color(rgba(0xffffff26))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|d| d.bg(rgba(0xffffff1f)))
                .child(icon(glyph, INK).size(px(18. * s)))
        };
        let face = |id: &'static str, label: &str, yaw: f32| {
            let on = ((self.shop.yaw - yaw).rem_euclid(360.))
                .min((yaw - self.shop.yaw).rem_euclid(360.))
                < 1.;
            div()
                .id(id)
                .h(px(40. * s))
                .px(px(16. * s))
                .rounded_full()
                .flex()
                .items_center()
                .text_size(px(14. * s))
                .cursor_pointer()
                .when(on, |d| {
                    d.bg(rgb(0xffffff))
                        .text_color(rgb(0x0b0b0d))
                        .font_weight(FontWeight::SEMIBOLD)
                })
                .when(!on, |d| {
                    d.bg(rgba(0x0b0b0d99))
                        .border_1()
                        .border_color(rgba(0xffffff26))
                        .hover(|d| d.bg(rgba(0xffffff1f)))
                })
                .child(tr(label))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.shop.yaw = yaw;
                    cx.notify();
                }))
        };
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(14. * s))
            .child(
                div()
                    .id("shop-figure")
                    .relative()
                    .size(size)
                    .cursor_grab()
                    .when(self.shop.drag.is_some(), |d| d.cursor_grabbing())
                    // A soft shadow at the feet.
                    .child(
                        div()
                            .absolute()
                            .left(px(200. * s))
                            .right(px(200. * s))
                            .bottom(px(22. * s))
                            .h(px(16. * s))
                            .rounded_full()
                            .bg(rgba(0x00000066)),
                    )
                    .child(
                        img(self.shop_picture(offer))
                            .absolute()
                            .inset_0()
                            .size_full(),
                    )
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                            this.shop.drag = Some(f32::from(event.position.x));
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8. * s))
                    .child(
                        round("shop-turn-left", "chevron-left").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.shop.yaw = (this.shop.yaw + 90.).rem_euclid(360.);
                                cx.notify();
                            },
                        )),
                    )
                    .child(face("shop-face-front", "Front", 0.))
                    .child(face("shop-face-back", "Back", 180.))
                    .child(
                        round("shop-turn-right", "chevron-right").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.shop.yaw = (this.shop.yaw - 90.).rem_euclid(360.);
                                cx.notify();
                            },
                        )),
                    ),
            )
            .child(
                div()
                    .text_size(px(13. * s))
                    .text_color(rgb(0x8e9099))
                    .child(tr(if self.skin.is_some() {
                        "Your skin. Drag to turn it."
                    } else {
                        "Sign in to see it on your own skin. Drag to turn it."
                    })),
            )
    }

    pub(super) fn shop_page(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let Some(offer) = self.shop_selected() else {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(quiet("New cosmetics are coming soon."));
        };
        let (tier_name, tier_colour) = tier(offer);
        let line = trf(
            "{tier} {kind}",
            &[
                ("tier", tr(tier_name).as_str()),
                ("kind", tr(kind_one(offer.kind)).as_str()),
            ],
        );
        let status = self
            .shop
            .notice
            .clone()
            .or_else(|| self.shop_state.clone().map(|why| (why, true)));
        let title = div()
            .flex()
            .flex_col()
            .gap(px(10. * s))
            .child(
                div()
                    .text_size(px(16. * s))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(tier_colour))
                    .child(line),
            )
            .child(
                div()
                    .max_w(px(820. * s))
                    .text_size(px(64. * s))
                    .line_height(px(72. * s))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr(offer.name)),
            )
            .child(
                div()
                    .max_w(px(560. * s))
                    .text_size(px(17. * s))
                    .line_height(px(25. * s))
                    .text_color(rgb(0xc9cbd1))
                    .child(tr(offer.blurb)),
            )
            .child(
                div()
                    .mt(px(14. * s))
                    .flex()
                    .items_center()
                    .gap(px(22. * s))
                    .child(self.shop_button(offer, cx))
                    .child(price(offer, self.server_wallet.discount, 24., s)),
            )
            .when_some(status, |d, (text, bad)| {
                d.child(
                    div()
                        .max_w(px(620. * s))
                        .text_size(px(14. * s))
                        .text_color(rgb(if bad { 0xf08a96 } else { 0xb4b6bd }))
                        .child(text),
                )
            });
        div()
            .size_full()
            .relative()
            .flex()
            .gap(px(40. * s))
            .px(px(64. * s))
            .pt(px(8. * s))
            .pb(px(40. * s))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                let Some(last) = this.shop.drag else {
                    return;
                };
                if event.pressed_button != Some(gpui::MouseButton::Left) {
                    this.shop.drag = None;
                    cx.notify();
                    return;
                }
                let x = f32::from(event.position.x);
                this.shop.yaw = (this.shop.yaw - (x - last) * 0.6).rem_euclid(360.);
                this.shop.drag = Some(x);
                cx.notify();
            }))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.shop.drag = None;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom(px(40. * s))
                    .left(px(500. * s))
                    .right(px(484. * s))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(self.shop_figure(offer, cx)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().flex_1().min_h(px(20. * s)))
                    .child(title)
                    .child(div().h(px(36. * s)).flex_none())
                    .child(
                        div()
                            .flex()
                            .gap(px(14. * s))
                            .child(self.shop_coins(cx))
                            .child(self.shop_creator(cx)),
                    ),
            )
            .child(self.shop_list(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coins_are_grouped_by_thousands() {
        assert_eq!(coins(0), "0");
        assert_eq!(coins(750), "750");
        assert_eq!(coins(1_200), "1,200");
        assert_eq!(coins(1_234_567), "1,234,567");
    }

    #[test]
    fn the_founders_cape_is_free() {
        let cape = shop::offer(shop::FOUNDERS).unwrap();
        assert_eq!(tier(cape).0, "Founders");
        assert_eq!(cape.price, 0);
    }
}
