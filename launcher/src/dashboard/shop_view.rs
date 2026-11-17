//! The Shop: Wisp Coins, capes, wings, weapons, hats, pets, trails, auras and emotes
//! (`wisp_launcher::shop`).
//!
//! A store page: a compact header with the balance, a featured carousel with a thumbnail picker
//! (it turns by itself), category tabs with counts beside search, filter and sort, then cards in
//! rows of four. On the Featured tab the cards are grouped by category. Each card's tier colour
//! follows its price.
use super::catalog_views::cat_panel;
use super::friends_view::FriendCmd;
use super::*;
use wisp_launcher::{
    catalog,
    shop::{self, Kind, Offer},
};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Show {
    #[default]
    All,
    Owned,
    Equipped,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Sort {
    #[default]
    Featured,
    PriceLow,
    PriceHigh,
}

#[derive(Default)]
pub(super) struct ShopUi {
    /// None is the Featured tab: every kind, grouped.
    pub tab: Option<Kind>,
    pub show: Show,
    pub sort: Sort,
    pub slide: usize,
    /// Ticks of the 200 ms timer since the carousel last turned.
    pub slide_ticks: u32,
    /// What the last purchase or change did, shown above the items.
    pub notice: Option<(String, bool)>,
    /// A purchase or change is with the Wisp server: the buttons wait for its answer.
    pub busy: bool,
}

/// The carousel's slides: an item, its chip, title and text.
pub(super) const SLIDES: [(&str, &str, &str, &str); 3] = [
    (
        shop::FOUNDERS,
        "Founders edition",
        "Wisp Founders Cape",
        "A night-sky cape with the Wisp star, free for Wisp's first players. It stays on your account for good.",
    ),
    (
        "cosmic_fox",
        "New pet",
        "Astra the Cosmic Fox",
        "A fox spun from deep space rides on your shoulder: starlit fur, cyan eyes and a tail that ends in a glowing star.",
    ),
    (
        "storm_aura",
        "New: auras",
        "Storm Aura",
        "Lightning crackles around you wherever you go, bright even in the dark.",
    ),
];
const GOLD: u32 = 0xf2c14e;
/// Cards in a row.
const COLUMNS: usize = 4;

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

fn kind_icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Cape => "cape",
        Kind::Wings => "wings",
        Kind::Weapon => "sword",
        Kind::Head => "hat",
        Kind::Pet => "pet",
        Kind::Trail => "trail",
        Kind::Aura => "aura",
        Kind::Emote => "emote",
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

/// The glass plates of search, filter, sort, notices and the header pills.
fn field(s: f32) -> Div {
    div()
        .rounded(px(12. * s))
        .border_1()
        .border_color(rgba(0xffffff1f))
        .bg(rgba(0x0b0b0dc4))
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

    /// The creator code row: who is supported, or a field to enter a code.
    fn shop_creator(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let row = field(s)
            .px(px(14. * s))
            .py(px(10. * s))
            .flex()
            .items_center()
            .gap(px(12. * s))
            .child(icon("star", 0xf2c14e).size(px(18. * s)));
        match &self.server_wallet.supporting {
            Some((name, code)) => row
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(14. * s))
                        .child(trf(
                            "You support {name} ({code}): {percent} % off everything, and {name} earns from every item you buy.",
                            &[
                                ("name", name),
                                ("code", code.as_deref().unwrap_or("")),
                                ("percent", &self.server_wallet.discount.to_string()),
                            ],
                        )),
                )
                .child(
                    button("shop-creator-clear", "Remove code", "close").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.shop.notice = Some((tr("Asking the Wisp server…"), false));
                            this.friends_send(FriendCmd::Support(None));
                            cx.notify();
                        },
                    )),
                ),
            None => row
                .child(
                    div()
                        .flex_none()
                        .text_size(px(14. * s))
                        .text_color(rgb(MUTED))
                        .child(tr("Creator code")),
                )
                .child(div().flex_1().min_w_0().child(self.creator_input.clone()))
                .child(
                    button("shop-creator-use", "Use code", "check-circle").on_click(cx.listener(
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
                    )),
                ),
        }
    }

    /// Buy, claim, equip or take off: `big` is the carousel's button.
    fn shop_button(
        &self,
        key: &str,
        offer: &'static Offer,
        big: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let s = self.ui_scale;
        let short = !self.wallet.owns(offer.id)
            && !offer.founders()
            && self.wallet.coins < shop::discounted(offer.price, self.server_wallet.discount);
        let (label, glyph, bright) = if self.wallet.wears(offer.id) {
            ("Take off", "check-circle", false)
        } else if self.wallet.owns(offer.id) {
            ("Equip", "star", false)
        } else if offer.founders() {
            ("Claim free", "cape", true)
        } else if big {
            ("Buy now", "cart", true)
        } else {
            ("Buy", "cart", true)
        };
        let id = offer.id;
        div()
            .id(SharedString::from(format!("shop-{key}-{id}")))
            .h(px(if big { 52. } else { 34. } * s))
            .px(px(if big { 28. } else { 14. } * s))
            .flex_none()
            .rounded_full()
            .when(bright, |d| d.bg(rgb(0xffffff)).text_color(rgb(0x0b0b0d)))
            .when(!bright, |d| {
                d.bg(rgba(0xffffff14))
                    .border_1()
                    .border_color(rgba(0xffffff2e))
            })
            .when(short, |d| d.opacity(0.55))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(if big { 10. } else { 6. } * s))
            .text_size(px(if big { 17. } else { 13. } * s))
            .font_weight(FontWeight::SEMIBOLD)
            .cursor_pointer()
            .hover(|d| d.opacity(0.86))
            .child(
                icon(glyph, if bright { 0x0b0b0d } else { INK })
                    .size(px(if big { 19. } else { 14. } * s)),
            )
            .child(tr(label))
            .on_click(cx.listener(move |this, _, _, cx| this.shop_act(id, cx)))
    }

    fn shop_title(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let owned = self.wallet.owned.len();
        let collected = self.wallet.bonus_day == Some(shop::today());
        div()
            .flex()
            .items_end()
            .gap(px(14. * s))
            .child(div().flex_1().min_w_0().child(self.page_head(
                ("Back to Home", Page::Home),
                "Store",
                "Capes, wings, pets, trails, auras and emotes for your Wisp player.",
                None,
                cx,
            )))
            .child(
                field(s)
                    .id("shop-owned")
                    .h(px(46. * s))
                    .px(px(14. * s))
                    .flex()
                    .items_center()
                    .gap(px(9. * s))
                    .cursor_pointer()
                    .when(self.shop.show == Show::Owned, |d| d.border_color(rgba(0xffffffcc)))
                    .hover(|d| d.bg(rgba(0xffffff14)))
                    .child(icon("star", INK).size(px(19. * s)))
                    .child(
                        div()
                            .text_size(px(15. * s))
                            .child(trf("{count} owned", &[("count", &owned.to_string())])),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.shop.show = if this.shop.show == Show::Owned {
                            Show::All
                        } else {
                            Show::Owned
                        };
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("shop-coins")
                    .h(px(46. * s))
                    .pl(px(12. * s))
                    .pr(px(16. * s))
                    .rounded(px(10. * s))
                    .border_1()
                    .border_color(rgba(0xf2c14e55))
                    .bg(gpui::linear_gradient(
                        90.,
                        gpui::linear_color_stop(rgba(0xf2c14e26), 0.),
                        gpui::linear_color_stop(rgba(tone_a(0x17181bff)), 1.),
                    ))
                    .flex()
                    .items_center()
                    .gap(px(10. * s))
                    .cursor_pointer()
                    .child(
                        div()
                            .size(px(28. * s))
                            .rounded_full()
                            .bg(rgba(0xf2c14e30))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon("coin", GOLD).size(px(17. * s))),
                    )
                    .child(
                        div()
                            .child(
                                div()
                                    .text_size(px(17. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(coins(self.wallet.coins)),
                            )
                            .child(
                                div()
                                    .text_size(px(11. * s))
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
                    })),
            )
    }

    /// The featured carousel: the slide's art and text on the left, the slide picker on the right.
    fn shop_banner(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let current = self.shop.slide % SLIDES.len();
        let (id, chip, title, text) = SLIDES[current];
        let offer = shop::offer(id).expect("carousel items are in the shop");
        let (tier_name, tier_colour) = tier(offer);
        div()
            .flex()
            .gap(px(14. * s))
            .h(px(300. * s))
            .flex_none()
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .rounded(px(20. * s))
                    .border_1()
                    .border_color(rgba(0xffffff1f))
                    .overflow_hidden()
                    .bg(rgba(0x0b0b0dc4))
                    .child(
                        img(format!("shop/banner_{id}.png"))
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .object_fit(ObjectFit::Cover),
                    )
                    .child(div().absolute().size_full().bg(gpui::linear_gradient(
                        90.,
                        gpui::linear_color_stop(rgba(tone_a(0x0e0f11f2)), 0.3),
                        gpui::linear_color_stop(rgba(tone_a(0x0e0f1100)), 0.7),
                    )))
                    .child(
                        div()
                            .relative()
                            .h_full()
                            .pl(px(40. * s))
                            .flex()
                            .flex_col()
                            .justify_center()
                            .gap(px(12. * s))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(8. * s))
                                    .child(
                                        div()
                                            .px(px(12. * s))
                                            .py(px(4. * s))
                                            .rounded_full()
                                            .bg(rgb(0xffffff))
                                            .text_color(rgb(0x0b0b0d))
                                            .text_size(px(13. * s))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(tr(chip).to_uppercase()),
                                    )
                                    .child(
                                        div()
                                            .px(px(12. * s))
                                            .py(px(4. * s))
                                            .rounded_full()
                                            .bg(rgba(0x0b0b0d99))
                                            .border_1()
                                            .border_color(rgb(tier_colour))
                                            .text_color(rgb(tier_colour))
                                            .text_size(px(13. * s))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(tr(tier_name).to_uppercase()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(40. * s))
                                    .line_height(px(46. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(tr(title)),
                            )
                            .child(
                                div()
                                    .max_w(px(460. * s))
                                    .text_size(px(16. * s))
                                    .line_height(px(23. * s))
                                    .text_color(rgb(0xd9dadd))
                                    .child(tr(text)),
                            )
                            .child(
                                div()
                                    .mt(px(8. * s))
                                    .flex()
                                    .items_center()
                                    .gap(px(18. * s))
                                    .child(self.shop_button("banner", offer, true, cx))
                                    .child(price(offer, self.server_wallet.discount, 22., s)),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom(px(16. * s))
                            .left(px(40. * s))
                            .flex()
                            .gap(px(6. * s))
                            .children((0..SLIDES.len()).map(|i| {
                                div()
                                    .h(px(4. * s))
                                    .w(px(if i == current { 28. } else { 12. } * s))
                                    .rounded_full()
                                    .bg(rgba(if i == current { 0xffffffff } else { 0xffffff40 }))
                            })),
                    ),
            )
            .child(
                div()
                    .w(px(250. * s))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(10. * s))
                    .children(SLIDES.iter().enumerate().map(|(i, (id, chip, title, _))| {
                        let on = i == current;
                        div()
                            .id(("shop-slide", i))
                            .flex_1()
                            .relative()
                            .rounded(px(16. * s))
                            .overflow_hidden()
                            .border_2()
                            .border_color(rgba(if on { 0xffffffd9 } else { 0xffffff14 }))
                            .cursor_pointer()
                            .child(
                                img(format!("shop/{id}.png"))
                                    .absolute()
                                    .size_full()
                                    .object_fit(ObjectFit::Cover),
                            )
                            .child(div().absolute().size_full().bg(gpui::linear_gradient(
                                90.,
                                gpui::linear_color_stop(rgba(tone_a(0x0e0f11e8)), 0.),
                                gpui::linear_color_stop(rgba(tone_a(0x0e0f1120)), 1.),
                            )))
                            .when(!on, |d| d.opacity(0.75).hover(|d| d.opacity(1.)))
                            .child(
                                div()
                                    .relative()
                                    .h_full()
                                    .px(px(14. * s))
                                    .flex()
                                    .flex_col()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_size(px(11. * s))
                                            .text_color(rgb(0xb4b6bd))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(tr(chip).to_uppercase()),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(15. * s))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(tr(title)),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.shop.slide = i;
                                this.shop.slide_ticks = 0;
                                cx.notify();
                            }))
                    })),
            )
    }

    fn shop_tab(&self, i: usize, kind: Option<Kind>, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = self.ui_scale;
        let on = self.shop.tab == kind;
        let (label, glyph) = match kind {
            None => ("Featured", "star"),
            Some(k) => (kind_label(k), kind_icon(k)),
        };
        let count = shop::OFFERS
            .iter()
            .filter(|o| kind.is_none_or(|k| o.kind == k))
            .count();
        div()
            .id(("shop-tab", i))
            .h(px(38. * s))
            .px(px(14. * s))
            .rounded_full()
            .flex()
            .items_center()
            .gap(px(8. * s))
            .cursor_pointer()
            .when(on, |d| d.bg(rgb(0xffffff)))
            .when(!on, |d| d.hover(|d| d.bg(rgba(0xffffff14))))
            .child(icon(glyph, if on { 0x0b0b0d } else { 0xb4b6bd }).size(px(16. * s)))
            .child(
                div()
                    .text_size(px(15. * s))
                    .text_color(rgb(if on { 0x0b0b0d } else { 0xc4c6cc }))
                    .when(on, |d| d.font_weight(FontWeight::SEMIBOLD))
                    .child(tr(label)),
            )
            .child(
                div()
                    .text_size(px(12. * s))
                    .text_color(rgb(if on { 0x55575e } else { 0x8e9099 }))
                    .child(count.to_string()),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.shop.tab = kind;
                cx.notify();
            }))
    }

    /// Cards in rows of four that fill their row, so every card lines up.
    fn shop_rows(&self, offers: &[&'static Offer], cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let mut grid = div().w_full().flex().flex_col().gap(px(14. * s));
        for row in offers.chunks(COLUMNS) {
            let mut line = div().flex().gap(px(14. * s));
            for &offer in row {
                line = line.child(self.shop_card(offer, cx));
            }
            for _ in row.len()..COLUMNS {
                line = line.child(div().flex_1());
            }
            grid = grid.child(line);
        }
        grid
    }

    /// A category's heading on the Featured tab: its name, count and a link to its own tab.
    fn shop_section(&self, kind: Kind, count: usize, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        div()
            .flex()
            .items_center()
            .gap(px(10. * s))
            .child(icon(kind_icon(kind), INK).size(px(20. * s)))
            .child(
                div()
                    .text_size(px(20. * s))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr(kind_label(kind))),
            )
            .child(
                div()
                    .text_size(px(14. * s))
                    .text_color(rgb(MUTED))
                    .child(count.to_string()),
            )
            .child(
                div()
                    .flex_1()
                    .h(px(1.))
                    .mx(px(8. * s))
                    .bg(rgba(0xffffff14)),
            )
            .child(
                div()
                    .id(SharedString::from(format!("shop-see-{}", kind_icon(kind))))
                    .flex()
                    .items_center()
                    .gap(px(4. * s))
                    .px(px(8. * s))
                    .py(px(3. * s))
                    .rounded(px(6. * s))
                    .cursor_pointer()
                    .text_size(px(14. * s))
                    .text_color(rgb(MUTED))
                    .hover(|d| d.bg(rgba(0xffffff14)).text_color(rgb(INK)))
                    .child(tr("See all"))
                    .child(icon("chevron-right", MUTED).size(px(14. * s)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.shop.tab = Some(kind);
                        cx.notify();
                    })),
            )
    }

    fn shop_card(&self, offer: &'static Offer, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = self.ui_scale;
        let owned = self.wallet.owns(offer.id);
        let worn = self.wallet.wears(offer.id);
        let (tier_name, tier_colour) = tier(offer);
        let state = if worn {
            Some(("Equipped", 0xffffff))
        } else if owned {
            Some(("Owned", 0xc4c6cc))
        } else {
            None
        };
        div()
            .id(SharedString::from(format!("shop-card-{}", offer.id)))
            .flex_1()
            .min_w_0()
            .p(px(8. * s))
            .rounded(px(18. * s))
            .border_1()
            .border_color(rgba(if worn { 0xffffffcc } else { 0xffffff17 }))
            .bg(rgba(0x111215e6))
            .hover(move |d| d.border_color(rgba(if worn { 0xffffffcc } else { 0xffffff47 })))
            .child(
                div()
                    .relative()
                    .h(px(150. * s))
                    .rounded(px(12. * s))
                    .overflow_hidden()
                    .bg(gpui::linear_gradient(
                        0.,
                        gpui::linear_color_stop(rgba((tier_colour << 8) | 0x40), 0.),
                        gpui::linear_color_stop(rgba(0x16171aff), 1.),
                    ))
                    .child(
                        img(format!("shop/{}.png", offer.id))
                            .size_full()
                            .object_fit(ObjectFit::Cover),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom_0()
                            .h(px(3. * s))
                            .bg(rgb(tier_colour)),
                    )
                    .when_some(state, |d, (label, colour)| {
                        d.child(
                            div()
                                .absolute()
                                .top(px(8. * s))
                                .right(px(8. * s))
                                .px(px(8. * s))
                                .py(px(3. * s))
                                .rounded_full()
                                .bg(rgba(0x0b0b0dd0))
                                .flex()
                                .items_center()
                                .gap(px(5. * s))
                                .text_size(px(12. * s))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(colour))
                                .child(icon("check-circle", colour).size(px(12. * s)))
                                .child(tr(label)),
                        )
                    })
                    .when_some(offer.creator, |d, creator| {
                        d.child(
                            div()
                                .absolute()
                                .top(px(8. * s))
                                .left(px(8. * s))
                                .px(px(8. * s))
                                .py(px(3. * s))
                                .rounded_full()
                                .bg(rgba(0x0b0b0dd0))
                                .text_size(px(11. * s))
                                .child(format!("{} {creator}", tr("by"))),
                        )
                    }),
            )
            .child(
                div()
                    .px(px(6. * s))
                    .pt(px(12. * s))
                    .pb(px(6. * s))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8. * s))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(16. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(tr(offer.name)),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(11. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(tier_colour))
                                    .child(tr(tier_name).to_uppercase()),
                            ),
                    )
                    .child(
                        // Two lines for every blurb, so the cards of a row line up.
                        div()
                            .mt(px(4. * s))
                            .h(px(34. * s))
                            .overflow_hidden()
                            .text_size(px(13. * s))
                            .line_height(px(17. * s))
                            .text_color(rgb(MUTED))
                            .child(tr(offer.blurb)),
                    )
                    .child(
                        div()
                            .mt(px(10. * s))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(price(offer, self.server_wallet.discount, 16., s))
                            .child(self.shop_button("buy", offer, false, cx)),
                    ),
            )
    }

    pub(super) fn shop_page(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let query = self.shop_search.read(cx).value.clone();
        let ui = &self.shop;
        let mut offers: Vec<&'static Offer> = shop::OFFERS
            .iter()
            .filter(|o| ui.tab.is_none_or(|k| o.kind == k))
            .filter(|o| match ui.show {
                Show::All => true,
                Show::Owned => self.wallet.owns(o.id),
                Show::Equipped => self.wallet.wears(o.id),
            })
            .filter(|o| catalog::matches(&query, &[o.name, o.blurb, o.id]))
            .collect();
        match ui.sort {
            Sort::Featured => {}
            Sort::PriceLow => offers.sort_by_key(|o| o.price),
            Sort::PriceHigh => offers.sort_by_key(|o| std::cmp::Reverse(o.price)),
        }
        let mut grid = div().w_full().flex().flex_col().gap(px(14. * s));
        if offers.is_empty() {
            grid = grid.child(
                cat_panel(s)
                    .w_full()
                    .p(px(28. * s))
                    .child(quiet(match ui.show {
                        Show::All => "No items match your search.",
                        _ => "Nothing here yet. Buy an item to see it in this list.",
                    })),
            );
        }
        if ui.tab.is_none() {
            // Grouped by category, in the tabs' order; a category with nothing to show is left out.
            for kind in Kind::ALL {
                let of_kind: Vec<&'static Offer> =
                    offers.iter().copied().filter(|o| o.kind == kind).collect();
                if of_kind.is_empty() {
                    continue;
                }
                grid = grid.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12. * s))
                        .mb(px(12. * s))
                        .child(self.shop_section(kind, of_kind.len(), cx))
                        .child(self.shop_rows(&of_kind, cx)),
                );
            }
        } else {
            grid = grid.child(self.shop_rows(&offers, cx));
        }
        let show_label = match ui.show {
            Show::All => "All items",
            Show::Owned => "Owned",
            Show::Equipped => "Equipped",
        };
        let sort_label = match ui.sort {
            Sort::Featured => "Featured",
            Sort::PriceLow => "Price: low to high",
            Sort::PriceHigh => "Price: high to low",
        };
        let control = |id: &'static str, glyph: &'static str, label: &str| {
            field(s)
                .id(id)
                .h(px(40. * s))
                .flex_none()
                .px(px(12. * s))
                .flex()
                .items_center()
                .gap(px(8. * s))
                .cursor_pointer()
                .hover(|d| d.bg(rgba(0xffffff14)))
                .child(icon(glyph, INK).size(px(16. * s)))
                .child(div().text_size(px(14. * s)).child(tr(label)))
                .child(icon("down", MUTED).size(px(14. * s)))
        };
        div()
            .flex()
            .flex_col()
            .gap(px(18. * s))
            .pb(px(20. * s))
            .child(self.shop_title(cx))
            .when_some(self.shop_state.clone(), |d, why| {
                d.child(
                    field(s)
                        .px(px(14. * s))
                        .py(px(10. * s))
                        .border_color(rgba(0xff8a8a66))
                        .flex()
                        .items_center()
                        .gap(px(10. * s))
                        .child(icon("info", 0xf08a96).size(px(18. * s)))
                        .child(div().flex_1().text_size(px(14. * s)).child(why)),
                )
            })
            .when(self.shop_state.is_none(), |d| {
                d.child(self.shop_creator(cx))
            })
            .child(self.shop_banner(cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10. * s))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(2. * s))
                            .p(px(5. * s))
                            .rounded(px(26. * s))
                            .bg(rgba(0x0b0b0d8c))
                            .border_1()
                            .border_color(rgba(0xffffff1a))
                            .children(
                                std::iter::once(None)
                                    .chain(Kind::ALL.into_iter().map(Some))
                                    .enumerate()
                                    .map(|(i, kind)| self.shop_tab(i, kind, cx)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10. * s))
                    .child(div().flex_1().child(self.shop_search.clone()))
                    .child(
                        control("shop-filter", "sliders-horizontal", show_label).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.shop.show = match this.shop.show {
                                    Show::All => Show::Owned,
                                    Show::Owned => Show::Equipped,
                                    Show::Equipped => Show::All,
                                };
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        control("shop-sort", "refresh", sort_label).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.shop.sort = match this.shop.sort {
                                    Sort::Featured => Sort::PriceLow,
                                    Sort::PriceLow => Sort::PriceHigh,
                                    Sort::PriceHigh => Sort::Featured,
                                };
                                cx.notify();
                            },
                        )),
                    ),
            )
            .when_some(ui.notice.clone(), |d, (text, bad)| {
                d.child(
                    field(s)
                        .px(px(14. * s))
                        .py(px(10. * s))
                        .border_color(rgba(if bad { 0xff8a8a66 } else { 0xffffff40 }))
                        .flex()
                        .items_center()
                        .gap(px(10. * s))
                        .child(
                            icon(
                                if bad { "info" } else { "check-circle" },
                                if bad { 0xf08a96 } else { 0x4ade80 },
                            )
                            .size(px(18. * s)),
                        )
                        .child(div().flex_1().text_size(px(14. * s)).child(text)),
                )
            })
            .child(grid)
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
    fn every_carousel_item_is_in_the_shop() {
        for (id, ..) in SLIDES {
            assert!(shop::offer(id).is_some(), "{id}");
        }
    }
}
