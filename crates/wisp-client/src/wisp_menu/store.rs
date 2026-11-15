//! The Wisp Menu's Store, Creator and Admin pages, through the Wisp server
//! (`wisp_cosmetics::share`, the server's `shop.rs` and `creators.rs`).
//!
//! - **Store**: the coins (kept only on the server), the creator code the player supports (10 %
//!   off everything, and the creator earns 10 % of each price), the items by kind with their card,
//!   price and Buy, Equip or Take off. The server prices every purchase and checks the coins.
//! - **Creator** (creators only): their code, and how many players support them, what those
//!   bought and the coins it earned them.
//! - **Admin** (the client's owner only): make a player a creator, or no longer one.
use super::*;
use wisp_cosmetics::share::ShopState;
use wisp_shop::{Kind, OFFERS, Offer};

/// Which of the three pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorePage {
    Shop,
    Creator,
    Admin,
}

/// The pages' text fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreField {
    /// A creator code to support.
    Support,
    /// A creator's own code.
    Code,
    /// The name of a player to make a creator.
    AdminName,
}

/// A click on the Store's pages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StoreClick {
    Page(StorePage),
    /// A kind's chip (`None`: everything).
    Kind(Option<Kind>),
    /// An item's button: buy, equip or take off, as the wallet has it.
    Item(&'static str),
    Field(StoreField),
    /// The field's button.
    Submit(StoreField),
    /// Stop supporting the creator.
    ClearSupport,
    /// An admin's Remove on a creator (index into `store_creators`): the first click asks.
    Remove(usize),
}

/// How long the answer to an action stays under the title, in seconds.
const NOTICE: u64 = 6;
const GOLD: u32 = hex(0xF2C14E, 255);
const GOLD_SOFT: u32 = hex(0xF2C14E, 40);
const GREEN: u32 = hex(0x22C55E, 235);
const GREEN_SOFT: u32 = hex(0x22C55E, 40);
const RED: u32 = hex(0xFF6B6B, 230);
const CARD_MIN: f32 = 210.0;
const GAP: f32 = 12.0;

/// Coins with thousands grouped: 12,000.
pub fn coins(n: u32) -> String {
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

/// A kind's name, plural for the chips.
fn kind_name(kind: Kind) -> String {
    let (key, name) = match kind {
        Kind::Cape => ("capes", "Capes"),
        Kind::Wings => ("wings", "Wings"),
        Kind::Weapon => ("weapons", "Weapons"),
        Kind::Head => ("hats", "Hats"),
        Kind::Pet => ("pets", "Pets"),
        Kind::Trail => ("trails", "Trails"),
        Kind::Aura => ("auras", "Auras"),
        Kind::Emote => ("emotes", "Emotes"),
    };
    lang::get_or(&format!("wisp.menu.store.kind.{key}"), name)
}

/// What an item's button does now, its label and whether it is the bright one.
fn item_button(offer: &Offer, owned: bool, worn: bool, affordable: bool) -> (String, bool) {
    if worn {
        (lang::get_or("wisp.menu.store.take_off", "Take off"), false)
    } else if owned {
        (lang::get_or("wisp.menu.store.equip", "Equip"), true)
    } else if offer.founders() {
        (lang::get_or("wisp.menu.store.claim", "Claim free"), true)
    } else {
        (lang::get_or("wisp.menu.store.buy", "Buy"), affordable)
    }
}

impl Menu {
    pub(super) fn draw_store(
        &mut self,
        ui: &mut Ui,
        state: Option<&ShopState>,
        page: StorePage,
        rect: (f32, f32, f32, f32),
    ) {
        let (mx, my, mw, _) = rect;
        let Some(state) = state else {
            self.store_empty(
                ui,
                (mx, my + 60.0, mw),
                &lang::get_or("wisp.menu.store.no_server", "No Wisp server"),
                &lang::get_or(
                    "wisp.menu.store.no_server_hint",
                    "The Shop runs on the Wisp server: set its address in the launcher, Settings, General.",
                ),
            );
            return;
        };
        match page {
            StorePage::Shop => self.draw_shop(ui, state, rect),
            StorePage::Creator => self.draw_creator(ui, state, rect),
            StorePage::Admin => self.draw_admin(ui, state, rect),
        }
    }

    /// The title, and under it the last action's answer or `line`.
    fn store_title(
        &mut self,
        ui: &mut Ui,
        state: &ShopState,
        (mx, my, mw): (f32, f32, f32),
        title: &str,
        line: &str,
    ) {
        ui.text(Face::Heading, 26.0, mx, my - 4.0, INK, title);
        let notice = state
            .notice
            .as_ref()
            .filter(|(_, _, at)| at.elapsed().as_secs() < NOTICE);
        let (text, colour) = match (notice, &state.problem) {
            (Some((text, bad, _)), _) => (text.clone(), if *bad { RED } else { ACCENT_SOFT }),
            (None, Some(problem)) => (problem.clone(), RED),
            (None, None) => (line.to_owned(), MUTED),
        };
        let text = fit(ui, Face::Medium, 13.0, &text, mw * 0.62);
        ui.text(Face::Medium, 13.0, mx, my + 32.0, colour, &text);
    }

    /// The coins, as a gold pill at the right of the title.
    fn coin_pill(ui: &mut Ui, coins_now: u32, right: f32, y: f32) {
        let label = coins(coins_now);
        let w = ui.measure(Face::Medium, 15.0, &label) + 46.0;
        let x = right - w;
        ui.rounded(x, y, w, 36.0, 12.0, GOLD_SOFT);
        ui.border(x, y, w, 36.0, 12.0, hex(0xF2C14E, 90));
        ui.circle(x + 18.0, y + 18.0, 8.0, GOLD);
        ui.circle(x + 18.0, y + 18.0, 4.0, hex(0xC9962A, 255));
        ui.text_line(Face::Medium, 15.0, 36.0, x + 32.0, y, INK, &label);
    }

    fn draw_shop(
        &mut self,
        ui: &mut Ui,
        state: &ShopState,
        (mx, my, mw, mh): (f32, f32, f32, f32),
    ) {
        let wallet = state.wallet.clone().unwrap_or_default();
        let line = match &wallet.supporting {
            Some((name, _)) => format!(
                "{} {name}  \u{b7}  {} % {}",
                lang::get_or("wisp.menu.store.supporting", "Supporting"),
                wallet.discount,
                lang::get_or("wisp.menu.store.off", "off everything"),
            ),
            None => lang::get_or(
                "wisp.menu.store.line",
                "Capes, wings, pets, trails, auras and emotes for your Wisp player.",
            ),
        };
        let title = lang::get_or("wisp.menu.store", "Store");
        self.store_title(ui, state, (mx, my, mw), &title, &line);
        if state.wallet.is_some() {
            Self::coin_pill(ui, wallet.coins, mx + mw, my - 2.0);
        }
        // The kinds.
        let mut x = mx;
        let cy = my + 62.0;
        for kind in std::iter::once(None).chain(Kind::ALL.into_iter().map(Some)) {
            let name = match kind {
                None => lang::get_or("wisp.menu.category.all", "All"),
                Some(k) => kind_name(k),
            };
            let w = ui.measure(Face::Medium, 12.0, &name) + 24.0;
            if x + w > mx + mw {
                break;
            }
            let selected = self.store_kind == kind;
            let fill = if selected {
                ACCENT
            } else if hover(ui, x, cy, w, 28.0) {
                CARD_HOVER
            } else {
                CARD
            };
            ui.rounded(x, cy, w, 28.0, 14.0, fill);
            let colour = if selected { ON_ACCENT } else { INK_SOFT };
            ui.text_line(Face::Medium, 12.0, 28.0, x + 12.0, cy, colour, &name);
            self.hit(ui, x, cy, w, 28.0, Action::Store(StoreClick::Kind(kind)));
            x += w + 6.0;
        }
        // The creator code.
        let fy = cy + 40.0;
        match &wallet.supporting {
            Some((name, code)) => {
                ui.rounded(mx, fy, mw, 38.0, 12.0, GREEN_SOFT);
                ui.border(mx, fy, mw, 38.0, 12.0, hex(0x22C55E, 80));
                let text = format!(
                    "{} {name} ({})  \u{b7}  {} {} % {}",
                    lang::get_or("wisp.menu.store.you_support", "You support"),
                    code.as_deref().unwrap_or(""),
                    lang::get_or("wisp.menu.store.you_get", "you get"),
                    wallet.discount,
                    lang::get_or("wisp.menu.store.off_short", "off"),
                );
                let text = fit(ui, Face::Medium, 13.0, &text, mw - 150.0);
                ui.text_line(Face::Medium, 13.0, 38.0, mx + 14.0, fy, INK, &text);
                let label = lang::get_or("wisp.menu.store.remove_code", "Remove code");
                let bw = ui.measure(Face::Medium, 12.0, &label) + 24.0;
                let bx = mx + mw - bw - 6.0;
                let fill = if hover(ui, bx, fy + 5.0, bw, 28.0) {
                    CARD_HOVER
                } else {
                    CARD
                };
                ui.rounded(bx, fy + 5.0, bw, 28.0, 10.0, fill);
                ui.text_line(Face::Medium, 12.0, 28.0, bx + 12.0, fy + 5.0, INK, &label);
                self.hit(
                    ui,
                    bx,
                    fy + 5.0,
                    bw,
                    28.0,
                    Action::Store(StoreClick::ClearSupport),
                );
            }
            None => {
                let hint = lang::get_or(
                    "wisp.menu.store.code_hint",
                    "Creator code: 10 % off, and the creator earns from it",
                );
                let label = lang::get_or("wisp.menu.store.use_code", "Use code");
                self.store_field(
                    ui,
                    StoreField::Support,
                    &hint,
                    &label,
                    state.busy,
                    (mx, fy, mw),
                );
            }
        }
        // The items.
        let gy = fy + 52.0;
        let gh = mh - (gy - my);
        let offers: Vec<&'static Offer> = OFFERS
            .iter()
            .filter(|o| self.store_kind.is_none_or(|k| k == o.kind))
            .collect();
        let cols = ((mw + GAP) / (CARD_MIN + GAP)).floor().max(1.0);
        let cw = ((mw - (cols - 1.0) * GAP) / cols).floor();
        let art = (cw * 156.0 / 480.0).round();
        let ch = art + 78.0;
        let rows = (offers.len() as f32 / cols).ceil();
        self.scroll_room = rows * (ch + GAP) - GAP - gh;
        self.scroll = self.scroll.clamp(0.0, self.scroll_room.max(0.0));
        let start = ui.mark();
        for (i, offer) in offers.iter().enumerate() {
            let (col, row) = ((i as f32 % cols), (i as f32 / cols).floor());
            let x = mx + col * (cw + GAP);
            let y = gy + row * (ch + GAP) - self.scroll;
            if y + ch < gy || y > gy + gh {
                continue;
            }
            self.store_card(
                ui,
                offer,
                &wallet,
                state.busy,
                (x, y, cw, art, ch),
                (gy, gh),
            );
        }
        ui.clip_since(start, mx - 2.0, gy, mw + 4.0, gh);
    }

    /// One item: its card, name, kind, price and button.
    fn store_card(
        &mut self,
        ui: &mut Ui,
        offer: &'static Offer,
        wallet: &wisp_cosmetics::share::ShopWallet,
        busy: bool,
        (x, y, w, art, h): (f32, f32, f32, f32, f32),
        (gy, gh): (f32, f32),
    ) {
        let owned = wallet.owned.iter().any(|o| o == offer.id);
        let worn = wallet.equipped.iter().any(|o| o == offer.id);
        let price = wisp_shop::discounted(offer.price, wallet.discount);
        let affordable = wallet.coins >= price;
        let visible = |y0: f32, y1: f32| y1 > gy && y0 < gy + gh;
        ui.rounded(x, y, w, h, 14.0, CARD);
        let sprite = format!("shop/{}", offer.id);
        if ui.has_sprite(&sprite) {
            ui.sprite(&sprite, x + 1.0, y + 1.0, w - 2.0, art);
        } else {
            ui.rounded(x, y, w, art, 14.0, CARD_HOVER);
        }
        ui.border(
            x,
            y,
            w,
            h,
            14.0,
            if worn { hex(0x22C55E, 140) } else { STROKE },
        );
        if worn {
            let label = lang::get_or("wisp.menu.store.worn", "Equipped");
            let lw = ui.measure(Face::Medium, 11.0, &label) + 16.0;
            ui.rounded(x + 8.0, y + 8.0, lw, 20.0, 10.0, GREEN);
            ui.text_line(
                Face::Medium,
                11.0,
                20.0,
                x + 16.0,
                y + 8.0,
                hex(0x0B0B0D, 255),
                &label,
            );
        }
        let ty = y + art + 10.0;
        let name = fit(ui, Face::Medium, 14.0, offer.name, w - 24.0);
        ui.text(Face::Medium, 14.0, x + 12.0, ty - 2.0, INK, &name);
        let kind = kind_name(offer.kind);
        let by = y + h - 36.0;
        // The price, the full one struck through when a creator code takes 10 % off.
        if owned {
            let label = lang::get_or("wisp.menu.store.owned", "Owned");
            ui.text_line(
                Face::Regular,
                12.0,
                28.0,
                x + 12.0,
                by,
                MUTED,
                &format!("{kind}  \u{b7}  {label}"),
            );
        } else if offer.founders() {
            let label = lang::get_or("wisp.menu.store.free", "Free");
            ui.text_line(Face::Medium, 13.0, 28.0, x + 12.0, by, GOLD, &label);
        } else {
            ui.circle(x + 19.0, by + 14.0, 6.0, GOLD);
            let label = coins(price);
            let pw = ui.text_line(Face::Medium, 13.0, 28.0, x + 30.0, by, INK, &label);
            if price < offer.price {
                let full = coins(offer.price);
                let fx = x + 36.0 + pw;
                let fw = ui.text_line(Face::Regular, 11.0, 28.0, fx, by, MUTED, &full);
                ui.rounded(fx, by + 14.0, fw, 1.0, 0.0, MUTED);
            }
        }
        // The button.
        let (label, bright) = item_button(offer, owned, worn, affordable);
        let bw = ui.measure(Face::Medium, 12.0, &label) + 24.0;
        let bx = x + w - bw - 10.0;
        let hovered = hover(ui, bx, by, bw, 28.0) && visible(by, by + 28.0);
        let fill = match (bright && !busy, hovered) {
            (true, true) => ACCENT_HOVER,
            (true, false) => ACCENT,
            (false, true) => CARD_HOVER,
            (false, false) => hex(0xFFFFFF, 16),
        };
        ui.rounded(bx, by, bw, 28.0, 10.0, fill);
        let colour = if bright && !busy { ON_ACCENT } else { INK_SOFT };
        ui.text_line(Face::Medium, 12.0, 28.0, bx + 12.0, by, colour, &label);
        if visible(by, by + 28.0) && !busy {
            self.hit(
                ui,
                bx,
                by,
                bw,
                28.0,
                Action::Store(StoreClick::Item(offer.id)),
            );
        }
    }

    /// A text field with its button: the creator code to support, a creator's own code, or a
    /// name for the admins.
    fn store_field(
        &mut self,
        ui: &mut Ui,
        field: StoreField,
        hint: &str,
        label: &str,
        busy: bool,
        (x, y, w): (f32, f32, f32),
    ) {
        let bw = ui.measure(Face::Medium, 13.0, label) + 28.0;
        let h = 38.0;
        let fw = w - bw - 8.0;
        let focused = self.store_focus == Some(field);
        ui.rounded(x, y, fw, h, 12.0, FIELD);
        if focused {
            ui.border(x, y, fw, h, 12.0, ACCENT_SOFT);
        }
        let empty = self.store_input.is_empty() || !focused;
        let shown = if empty {
            hint.to_owned()
        } else {
            self.store_input.clone()
        };
        let text = fit(ui, Face::Regular, 13.0, &shown, fw - 28.0);
        let colour = if empty { FAINT } else { INK };
        let tw = ui.text_line(Face::Regular, 13.0, h, x + 14.0, y, colour, &text);
        if focused && blink() {
            let cx = if empty { x + 14.0 } else { x + 15.0 + tw };
            ui.rounded(cx, y + 10.0, 1.0, h - 20.0, 0.0, INK);
        }
        self.hit(ui, x, y, fw, h, Action::Store(StoreClick::Field(field)));
        let ready = focused && !self.store_input.trim().is_empty() && !busy;
        let bx = x + w - bw;
        let fill = match (ready, hover(ui, bx, y, bw, h)) {
            (true, true) => ACCENT_HOVER,
            (true, false) => ACCENT,
            (false, _) => CARD_HOVER,
        };
        ui.rounded(bx, y, bw, h, 12.0, fill);
        let colour = if ready { ON_ACCENT } else { MUTED };
        ui.text_line(Face::Medium, 13.0, h, bx + 14.0, y, colour, label);
        self.hit(ui, bx, y, bw, h, Action::Store(StoreClick::Submit(field)));
    }

    fn draw_creator(
        &mut self,
        ui: &mut Ui,
        state: &ShopState,
        (mx, my, mw, _): (f32, f32, f32, f32),
    ) {
        let me = &state.me;
        let title = lang::get_or("wisp.menu.creator", "Creator");
        let line = lang::get_or(
            "wisp.menu.creator.line",
            "Players who enter your code get 10 % off and wear your badge; you earn 10 % of every item they buy.",
        );
        self.store_title(ui, state, (mx, my, mw), &title, &line);
        if let Some(wallet) = &state.wallet {
            Self::coin_pill(ui, wallet.coins, mx + mw, my - 2.0);
        }
        // The numbers.
        let ty = my + 64.0;
        let tiles = [
            (
                lang::get_or("wisp.menu.creator.supporters", "Players with your code"),
                me.supporters.to_string(),
            ),
            (
                lang::get_or("wisp.menu.creator.purchases", "Purchases with it"),
                me.purchases.to_string(),
            ),
            (
                lang::get_or("wisp.menu.creator.earned", "Coins earned"),
                coins(me.earned),
            ),
        ];
        let tw = (mw - 2.0 * GAP) / 3.0;
        for (i, (label, value)) in tiles.iter().enumerate() {
            let x = mx + i as f32 * (tw + GAP);
            ui.rounded(x, ty, tw, 86.0, 14.0, CARD);
            ui.border(x, ty, tw, 86.0, 14.0, STROKE);
            ui.text(
                Face::Heading,
                28.0,
                x + 16.0,
                ty + 10.0,
                if i == 2 { GOLD } else { INK },
                value,
            );
            let label = fit(ui, Face::Regular, 12.0, label, tw - 32.0);
            ui.text(Face::Regular, 12.0, x + 16.0, ty + 56.0, MUTED, &label);
        }
        // The code.
        let cy = ty + 110.0;
        ui.text(
            Face::Medium,
            15.0,
            mx,
            cy,
            INK,
            &lang::get_or("wisp.menu.creator.code", "Your creator code"),
        );
        let current = match &me.code {
            Some(code) => format!(
                "{}: {code}  \u{b7}  {}",
                lang::get_or("wisp.menu.creator.now", "Now"),
                lang::get_or(
                    "wisp.menu.creator.change_hint",
                    "type a new one to change it"
                ),
            ),
            None => lang::get_or(
                "wisp.menu.creator.none",
                "You have no code yet: 3 to 16 letters, digits or _",
            ),
        };
        let save = lang::get_or("wisp.menu.creator.save", "Save code");
        self.store_field(
            ui,
            StoreField::Code,
            &current,
            &save,
            state.busy,
            (mx, cy + 26.0, mw),
        );
        // The badge.
        let by = cy + 84.0;
        ui.rounded(mx, by, mw, 54.0, 14.0, CARD);
        ui.border(mx, by, mw, 54.0, 14.0, STROKE);
        let (dot, text) = if me.badge {
            (
                GREEN,
                lang::get_or(
                    "wisp.menu.creator.badge",
                    "Your badge is set: you and everyone with your code wear it before their name.",
                ),
            )
        } else {
            (
                GOLD,
                lang::get_or(
                    "wisp.menu.creator.no_badge",
                    "No badge yet: the Wisp team gives creators their badge.",
                ),
            )
        };
        ui.circle(mx + 22.0, by + 27.0, 5.0, dot);
        let text = fit(ui, Face::Regular, 13.0, &text, mw - 56.0);
        ui.text_line(Face::Regular, 13.0, 54.0, mx + 38.0, by, INK_SOFT, &text);
    }

    fn draw_admin(
        &mut self,
        ui: &mut Ui,
        state: &ShopState,
        (mx, my, mw, mh): (f32, f32, f32, f32),
    ) {
        let title = lang::get_or("wisp.menu.admin", "Admin");
        let line = format!(
            "{} {}  \u{b7}  {}",
            state.creators.len(),
            lang::get_or("wisp.menu.admin.creators", "creators"),
            lang::get_or(
                "wisp.menu.admin.badges",
                "badges are set on the server: \"3 - Creator verwalten\"",
            ),
        );
        self.store_title(ui, state, (mx, my, mw), &title, &line);
        let hint = lang::get_or("wisp.menu.admin.name", "Minecraft name of the new creator");
        let label = lang::get_or("wisp.menu.admin.add", "Make creator");
        self.store_field(
            ui,
            StoreField::AdminName,
            &hint,
            &label,
            state.busy,
            (mx, my + 62.0, mw),
        );
        self.store_creators = state.creators.iter().map(|c| c.uuid.clone()).collect();
        let ly = my + 116.0;
        let lh = mh - (ly - my);
        let row = 58.0;
        self.scroll_room = state.creators.len() as f32 * (row + 8.0) - lh;
        self.scroll = self.scroll.clamp(0.0, self.scroll_room.max(0.0));
        let start = ui.mark();
        if state.creators.is_empty() {
            let text = lang::get_or("wisp.menu.admin.empty", "No creators yet.");
            ui.text(Face::Regular, 13.0, mx, ly + 8.0, MUTED, &text);
        }
        for (i, c) in state.creators.iter().enumerate() {
            let y = ly + i as f32 * (row + 8.0) - self.scroll;
            if y + row < ly || y > ly + lh {
                continue;
            }
            ui.rounded(mx, y, mw, row, 14.0, CARD);
            ui.border(mx, y, mw, row, 14.0, STROKE);
            ui.circle(
                mx + 26.0,
                y + row / 2.0,
                14.0,
                hex(super::friend_tint(&c.name), 255),
            );
            let initial = c
                .name
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string();
            ui.text_centered(
                Face::Medium,
                13.0,
                mx + 26.0,
                y + row / 2.0 - 9.0,
                hex(0x0B0B0D, 255),
                &initial,
            );
            ui.text(Face::Medium, 14.0, mx + 52.0, y + 10.0, INK, &c.name);
            let detail = format!(
                "{}  \u{b7}  {} {}  \u{b7}  {} {}  \u{b7}  {}",
                c.code.as_deref().unwrap_or("no code yet"),
                c.supporters,
                lang::get_or("wisp.menu.admin.supporters", "supporters"),
                coins(c.earned),
                lang::get_or("wisp.menu.admin.coins", "coins"),
                if c.badge {
                    lang::get_or("wisp.menu.admin.has_badge", "badge set")
                } else {
                    lang::get_or("wisp.menu.admin.no_badge", "no badge")
                },
            );
            let detail = fit(ui, Face::Regular, 12.0, &detail, mw - 200.0);
            ui.text(Face::Regular, 12.0, mx + 52.0, y + 30.0, MUTED, &detail);
            let asking = self.store_confirm == Some(i);
            let label = if asking {
                lang::get_or("wisp.menu.admin.sure", "Click again to remove")
            } else {
                lang::get_or("wisp.menu.admin.remove", "Remove")
            };
            let bw = ui.measure(Face::Medium, 12.0, &label) + 24.0;
            let bx = mx + mw - bw - 12.0;
            let by = y + (row - 28.0) / 2.0;
            let fill = match (asking, hover(ui, bx, by, bw, 28.0)) {
                (true, _) => RED,
                (false, true) => CARD_HOVER,
                (false, false) => hex(0xFFFFFF, 16),
            };
            ui.rounded(bx, by, bw, 28.0, 10.0, fill);
            ui.text_line(Face::Medium, 12.0, 28.0, bx + 12.0, by, INK, &label);
            if by + 28.0 > ly && by < ly + lh {
                self.hit(ui, bx, by, bw, 28.0, Action::Store(StoreClick::Remove(i)));
            }
        }
        ui.clip_since(start, mx - 2.0, ly, mw + 4.0, lh);
    }

    /// A plate in the middle of the page: why there is nothing to show, and what to do.
    fn store_empty(&mut self, ui: &mut Ui, (mx, y, mw): (f32, f32, f32), head: &str, hint: &str) {
        ui.rounded(mx, y, mw, 110.0, 16.0, CARD);
        ui.border(mx, y, mw, 110.0, 16.0, STROKE);
        ui.text_centered(Face::Heading, 18.0, mx + mw / 2.0, y + 28.0, INK, head);
        let hint = fit(ui, Face::Regular, 13.0, hint, mw - 40.0);
        ui.text_centered(Face::Regular, 13.0, mx + mw / 2.0, y + 62.0, MUTED, &hint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coins_are_grouped_and_buttons_follow_the_wallet() {
        assert_eq!(coins(0), "0");
        assert_eq!(coins(1_100), "1,100");
        assert_eq!(coins(1_234_567), "1,234,567");
        let cape = OFFERS.iter().find(|o| !o.founders()).unwrap();
        let founders = OFFERS.iter().find(|o| o.founders()).unwrap();
        // Worn: take off; owned: equip; not owned: buy, bright only when there are coins enough.
        assert!(!item_button(cape, true, true, true).1);
        assert!(item_button(cape, true, false, false).1);
        assert!(item_button(cape, false, false, true).1);
        assert!(!item_button(cape, false, false, false).1);
        assert!(item_button(founders, false, false, false).1);
    }
}
