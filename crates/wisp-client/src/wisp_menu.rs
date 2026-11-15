//! The Wisp Menu (`docs/extras.md`): Right Shift over the game opens a panel in the launcher's
//! Glass look (`docs/design-system.md`): a sidebar of categories with Edit HUD, a grid of module
//! cards with their switches, and each module's settings page. This file lays it out and draws
//! it; the game (`game/wisp_menu.rs`) feeds it the settings, draws the cards' item icons and runs
//! the actions. One pass draws and records where everything is, and clicks read that record.
//! The Friends tab (`friends.rs`) lists the player's Wisp friends; the Store, Creator and Admin
//! tabs (`store.rs`) are the Shop on the Wisp server.
mod friends;
mod store;
pub use friends::{
    FriendClick, FriendsView, tint as friend_tint, whereabouts as friend_whereabouts,
};
pub use store::{StoreClick, StoreField, StorePage, coins as store_coins};

use crate::{
    extras::{
        Extras,
        features::{self, Category, Feature, Kind, SWATCHES, Setting, WIDGET_SETTINGS},
    },
    lang,
    ui_theme::{Face, Ui, color::hex},
};

// The launcher's dark look in the HUD encoding: neutral greys, the violet for what is selected,
// green for what is on.
const SCRIM: u32 = 0x050507;
const PANEL: u32 = hex(0x0F0F12, 244);
const HEADER: u32 = hex(0x16161B, 255);
const ACCENT: u32 = hex(wisp_theme::glass::ACCENT, 255);
const ACCENT_HOVER: u32 = hex(0xFFFFFF, 255);
const ACCENT_SOFT: u32 = hex(0xD4D4D8, 255);
/// Text on the accent: it is near white, so the text is dark.
const ON_ACCENT: u32 = hex(wisp_theme::glass::ON_LIGHT, 255);
const ON: u32 = hex(0x22C55E, 235);
const ON_HOVER: u32 = hex(0x2BD46A, 255);
const INK: u32 = hex(0xFFFFFF, 255);
const INK_SOFT: u32 = hex(0xFFFFFF, 210);
const MUTED: u32 = hex(0xFFFFFF, 150);
const FAINT: u32 = hex(0xFFFFFF, 100);
const STROKE: u32 = hex(0xFFFFFF, 18);
const CARD: u32 = hex(0xFFFFFF, 10);
const CARD_HOVER: u32 = hex(0xFFFFFF, 19);
const FIELD: u32 = hex(0xFFFFFF, 14);
const SWITCH_OFF: u32 = hex(0xFFFFFF, 38);
const fn accent_alpha(a: u8) -> u32 {
    hex(wisp_theme::glass::ACCENT, a)
}

/// The header (logo, search, Edit HUD) and the tab row under it, in dp.
const HEADER_H: f32 = 60.0;
const TABS_H: f32 = 44.0;
const TILE_W: f32 = 150.0;
const TILE_H: f32 = 158.0;
const GAP: f32 = 12.0;
const ROW_H: f32 = 50.0;
const RADIUS: f32 = 16.0;
/// The menu slides up and the world darkens over this long when it opens, in seconds.
const OPEN_ANIM: f32 = 0.16;

/// What a click does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// A sidebar tab (`None`: All).
    Category(Option<Category>),
    /// A card's or the settings page's switch.
    Toggle(&'static str),
    /// A card: its settings page.
    Open(&'static str),
    Back,
    /// The search field takes the keyboard.
    Search,
    EditHud,
    /// Every setting of the feature back to its default.
    Reset(&'static str),
    Setting(&'static str, &'static str, Control),
    /// The sidebar's Friends tab.
    Friends,
    /// The add-a-friend field takes the keyboard.
    FriendField,
    /// Sends the request to the name in the field.
    FriendAdd,
    /// A friend's or a request's button (`friends_shown`).
    Friend(FriendClick, usize),
    /// Something on the Store, Creator or Admin page.
    Store(StoreClick),
    /// A click outside the panel.
    Close,
    /// Inside the panel, on nothing.
    Nothing,
}
/// What a click on a setting's control does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Control {
    Toggle,
    Choice(usize),
    Color(u32),
    /// The slider's track from `x0` to `x1` in GUI pixels: the value follows the cursor.
    Slider {
        x0: f32,
        x1: f32,
    },
    /// Wait for a key.
    Key,
}

/// An item to draw over the frame: its id, its top left and side in GUI pixels, and the
/// rectangle it is clipped to.
#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    pub item: &'static str,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub clip: [f32; 4],
}

#[derive(Default)]
pub struct Menu {
    /// The sidebar's tab (`None`: All).
    pub category: Option<Category>,
    pub search: String,
    pub search_focused: bool,
    /// The settings page shown instead of the grid.
    pub open: Option<&'static str>,
    pub scroll: f32,
    /// The key setting waiting for a key.
    pub waiting_key: Option<(&'static str, &'static str)>,
    /// The slider being dragged, and its track in GUI pixels.
    pub dragging: Option<(&'static str, &'static str, f32, f32)>,
    /// The Friends tab is shown instead of the modules.
    pub friends_page: bool,
    /// The name typed to add a friend, and whether that field has the keyboard.
    pub friend_input: String,
    pub friend_focused: bool,
    /// The friend whose Remove was clicked once: the next click removes.
    pub confirm_remove: Option<usize>,
    /// The friends list as last drawn.
    friends_shown: Option<wisp_cosmetics::share::Friends>,
    /// The Store, Creator or Admin page is shown instead of the modules.
    pub store_page: Option<StorePage>,
    /// The Store's kind (`None`: everything).
    pub store_kind: Option<wisp_shop::Kind>,
    /// What is typed in the page's field, and which field has the keyboard.
    pub store_input: String,
    pub store_focus: Option<StoreField>,
    /// The creator whose Remove was clicked once: the next click removes.
    pub store_confirm: Option<usize>,
    /// The creators' UUIDs as the Admin page last drew them.
    store_creators: Vec<String>,
    /// When the menu opened, for its slide in.
    pub opened: Option<std::time::Instant>,
    hits: Vec<([f32; 4], Action)>,
    /// The scrolled area's content and visible heights (dp), for the wheel.
    scroll_room: f32,
}

/// What the menu shows of one feature.
struct Card<'a> {
    feature: &'a Feature,
    name: String,
    description: String,
    on: bool,
}

impl Menu {
    /// The features of the tab, filtered by the search.
    fn cards(&self, e: &Extras) -> Vec<Card<'static>> {
        let query = self.search.trim().to_lowercase();
        features::FEATURES
            .iter()
            .filter(|f| self.category.is_none_or(|c| c == f.category))
            .filter_map(|f| {
                let (name, description) = features::texts(f.key);
                let matches = query.is_empty()
                    || name.to_lowercase().contains(&query)
                    || description.to_lowercase().contains(&query);
                matches.then(|| Card {
                    feature: f,
                    name,
                    description,
                    on: e.feature_on(f.key),
                })
            })
            .collect()
    }
    fn hit(&mut self, ui: &Ui, x: f32, y: f32, w: f32, h: f32, action: Action) {
        self.hits.push((ui.gui_rect(x, y, w, h), action));
    }
    /// What a click at `cursor` (GUI pixels) does: the last drawn frame's topmost target.
    pub fn click(&self, cursor: (f32, f32)) -> Option<Action> {
        self.hits.iter().rev().find_map(|([x, y, w, h], a)| {
            (cursor.0 >= *x && cursor.0 < x + w && cursor.1 >= *y && cursor.1 < y + h).then_some(*a)
        })
    }
    /// The UUID of the creator in row `i` of the Admin page as last drawn.
    pub fn store_creator(&self, i: usize) -> Option<&str> {
        self.store_creators.get(i).map(String::as_str)
    }
    /// The wheel scrolls the grid or the settings page.
    pub fn wheel(&mut self, delta: f32) {
        self.scroll = (self.scroll - delta * 40.0).clamp(0.0, self.scroll_room.max(0.0));
    }
    /// Draws the menu over the frame; returns the items to draw on top.
    pub fn draw(
        &mut self,
        ui: &mut Ui,
        e: &Extras,
        key_name: &dyn Fn(&str) -> String,
        friends: FriendsView,
        store: Option<&wisp_cosmetics::share::ShopState>,
    ) -> Vec<Icon> {
        self.hits.clear();
        let mut icons = Vec::new();
        let (w, h) = (ui.w, ui.h);
        let t = self
            .opened
            .map_or(1.0, |at| (at.elapsed().as_secs_f32() / OPEN_ANIM).min(1.0));
        let ease = 1.0 - (1.0 - t).powi(3);
        ui.rounded(0.0, 0.0, w, h, 0.0, hex(SCRIM, (160.0 * ease) as u8));
        self.hit(ui, 0.0, 0.0, w, h, Action::Close);
        let pw = (w - 40.0).clamp(440.0, 1000.0);
        let ph = (h - 40.0).clamp(320.0, 600.0);
        let px = ((w - pw) / 2.0).round();
        let py = ((h - ph) / 2.0).round() + (1.0 - ease) * 16.0;
        // A soft shadow, then the panel.
        ui.rounded(
            px - 2.0,
            py + 4.0,
            pw + 4.0,
            ph + 4.0,
            RADIUS + 2.0,
            hex(0x000000, 70),
        );
        ui.rounded(px, py, pw, ph, RADIUS, PANEL);
        ui.border(px, py, pw, ph, RADIUS, STROKE);
        self.hit(ui, px, py, pw, ph, Action::Nothing);
        self.draw_header(ui, e, (px, py, pw), friends, store);
        let top = HEADER_H + TABS_H + 16.0;
        let (mx, my) = (px + 24.0, py + top);
        let (mw, mh) = (pw - 48.0, ph - top - 18.0);
        if let Some(page) = self.store_page {
            self.draw_store(ui, store, page, (mx, my, mw, mh));
            return icons;
        }
        if self.friends_page {
            self.draw_friends(ui, friends, (mx, my, mw, mh));
            return icons;
        }
        match self.open.and_then(features::feature) {
            Some(f) => self.draw_settings(ui, e, f, (mx, my, mw, mh), key_name, &mut icons),
            None => self.draw_grid(ui, e, (mx, my, mw, mh), &mut icons),
        }
        icons
    }

    /// The header: the mark and name, the search and Edit HUD; under it the tabs.
    fn draw_header(
        &mut self,
        ui: &mut Ui,
        e: &Extras,
        (px, py, pw): (f32, f32, f32),
        friends: FriendsView,
        store: Option<&wisp_cosmetics::share::ShopState>,
    ) {
        let other_page = self.friends_page || self.store_page.is_some();
        // The header's plate keeps the panel's rounded top corners.
        ui.rounded(px, py, pw, HEADER_H + TABS_H, RADIUS, HEADER);
        ui.rounded(px, py + HEADER_H + TABS_H - RADIUS, pw, RADIUS, 0.0, HEADER);
        ui.rounded(px, py + HEADER_H + TABS_H, pw, 1.0, 0.0, STROKE);
        // The mark: Wisp's white feather on a black tile, as the app icon has it.
        let (bx, by, side) = (px + 20.0, py + 14.0, 32.0);
        let tile = hex(0x000000, 255);
        ui.rounded(bx, by, side, side, 9.0, tile);
        ui.border(bx, by, side, side, 9.0, STROKE);
        ui.feather(bx + side / 2.0, by + side / 2.0, side, INK, tile);
        ui.text(Face::Heading, 18.0, bx + 42.0, by - 2.0, INK, "Wisp");
        let client = lang::get_or("wisp.menu.client", "Client");
        ui.text(Face::Regular, 11.0, bx + 42.0, by + 18.0, MUTED, &client);
        // Edit HUD, the main action, at the right.
        let label = lang::get_or("wisp.menu.edit_hud", "Edit HUD");
        let ew = ui.measure(Face::Medium, 13.0, &label) + 30.0;
        let (ex, ey, eh) = (px + pw - 20.0 - ew, py + 13.0, 34.0);
        let fill = if hover(ui, ex, ey, ew, eh) {
            ACCENT_HOVER
        } else {
            ACCENT
        };
        ui.rounded(ex, ey, ew, eh, 10.0, fill);
        ui.text_line(Face::Medium, 13.0, eh, ex + 15.0, ey, ON_ACCENT, &label);
        self.hit(ui, ex, ey, ew, eh, Action::EditHud);
        // The search, for the modules.
        if !other_page {
            let sw = 240.0_f32.min(pw * 0.32);
            let (sx, sy, sh) = (ex - 10.0 - sw, ey, eh);
            ui.rounded(sx, sy, sw, sh, 10.0, FIELD);
            if self.search_focused {
                ui.border(sx, sy, sw, sh, 10.0, ACCENT_SOFT);
            }
            ui.border(sx + 12.0, sy + 11.0, 11.0, 11.0, 5.5, MUTED);
            ui.rounded(sx + 21.0, sy + 20.0, 4.0, 2.0, 1.0, MUTED);
            let shown = if self.search.is_empty() {
                lang::get_or("wisp.menu.search", "Search modules")
            } else {
                self.search.clone()
            };
            let colour = if self.search.is_empty() { FAINT } else { INK };
            let text = fit(ui, Face::Regular, 13.0, &shown, sw - 48.0);
            let tw = ui.text_line(Face::Regular, 13.0, sh, sx + 32.0, sy, colour, &text);
            if self.search_focused && blink() {
                let cx = if self.search.is_empty() {
                    sx + 32.0
                } else {
                    sx + 33.0 + tw
                };
                ui.rounded(cx, sy + 9.0, 1.0, sh - 18.0, 0.0, INK);
            }
            self.hit(ui, sx, sy, sw, sh, Action::Search);
        }
        // The tabs: the categories, then Friends; the chosen one underlined in the accent.
        let ty = py + HEADER_H;
        let mut x = px + 14.0;
        let tabs = std::iter::once(None).chain(Category::ALL.into_iter().map(Some));
        for tab in tabs {
            let name = match tab {
                None => lang::get_or("wisp.menu.category.all", "All"),
                Some(c) => lang::get_or(c.lang_key(), c.name()),
            };
            let count = features::FEATURES
                .iter()
                .filter(|f| tab.is_none_or(|c| c == f.category))
                .filter(|f| e.feature_on(f.key))
                .count();
            let selected = self.category == tab && !other_page;
            let badge = (count > 0).then(|| {
                (
                    count.to_string(),
                    if selected {
                        accent_alpha(110)
                    } else {
                        hex(0xFFFFFF, 22)
                    },
                )
            });
            x = self.draw_tab(ui, x, ty, &name, selected, badge, Action::Category(tab));
        }
        ui.rounded(x + 4.0, ty + 14.0, 1.0, TABS_H - 28.0, 0.0, STROKE);
        x += 10.0;
        let list = friends.state.and_then(|s| s.list.as_ref());
        let waiting = list.map_or(0, |l| l.incoming.len());
        let online = list.map_or(0, |l| l.friends.iter().filter(|f| f.online).count());
        let badge = if waiting > 0 {
            Some((waiting.to_string(), hex(0xFF6B6B, 230)))
        } else {
            (online > 0).then(|| (online.to_string(), hex(0x22C55E, 150)))
        };
        let name = lang::get_or("wisp.menu.friends", "Friends");
        x = self.draw_tab(ui, x, ty, &name, self.friends_page, badge, Action::Friends);
        // The Shop: Store for everyone, Creator for creators, Admin for the owner.
        ui.rounded(x + 4.0, ty + 14.0, 1.0, TABS_H - 28.0, 0.0, STROKE);
        x += 10.0;
        let me = store.map(|s| s.me.clone()).unwrap_or_default();
        let pages = [
            (StorePage::Shop, "wisp.menu.store", "Store", true),
            (
                StorePage::Creator,
                "wisp.menu.creator",
                "Creator",
                me.creator,
            ),
            (StorePage::Admin, "wisp.menu.admin", "Admin", me.admin),
        ];
        for (page, key, fallback, shown) in pages {
            if !shown {
                continue;
            }
            let name = lang::get_or(key, fallback);
            let badge = (page == StorePage::Creator && me.supporters > 0)
                .then(|| (me.supporters.to_string(), hex(0xF2C14E, 150)));
            let selected = self.store_page == Some(page);
            x = self.draw_tab(
                ui,
                x,
                ty,
                &name,
                selected,
                badge,
                Action::Store(StoreClick::Page(page)),
            );
        }
        // How many are on, at the right of the tabs.
        if !other_page && self.open.is_none() {
            let cards = self.cards(e);
            let on = cards.iter().filter(|c| c.on).count();
            let summary = format!(
                "{on} / {} {}",
                cards.len(),
                lang::get_or("wisp.menu.on", "on")
            );
            let sw = ui.measure(Face::Regular, 12.0, &summary);
            ui.text_line(
                Face::Regular,
                12.0,
                TABS_H,
                px + pw - 24.0 - sw,
                ty,
                MUTED,
                &summary,
            );
        }
    }

    /// A tab of the tab row; returns where the next one starts.
    #[allow(clippy::too_many_arguments)]
    fn draw_tab(
        &mut self,
        ui: &mut Ui,
        x: f32,
        ty: f32,
        name: &str,
        selected: bool,
        badge: Option<(String, u32)>,
        action: Action,
    ) -> f32 {
        let face = if selected {
            Face::Medium
        } else {
            Face::Regular
        };
        let nw = ui.measure(face, 13.0, name);
        let badge_w = badge
            .as_ref()
            .map_or(0.0, |(b, _)| ui.measure(Face::Mono, 10.0, b) + 10.0 + 6.0);
        let tw = nw + badge_w + 20.0;
        let (by, bh) = (ty + 7.0, TABS_H - 14.0);
        let hovered = hover(ui, x, by, tw, bh);
        if hovered && !selected {
            ui.rounded(x, by, tw, bh, 8.0, CARD);
        }
        let colour = if selected || hovered { INK } else { MUTED };
        ui.text_line(face, 13.0, TABS_H, x + 10.0, ty, colour, name);
        if let Some((b, fill)) = badge {
            let cw = ui.measure(Face::Mono, 10.0, &b);
            let (bx, bw, bh2) = (x + 10.0 + nw + 6.0, cw + 10.0, 16.0);
            let bty = ty + (TABS_H - bh2) / 2.0;
            ui.rounded(bx, bty, bw, bh2, 8.0, fill);
            ui.text_line(Face::Mono, 10.0, bh2, bx + 5.0, bty, INK, &b);
        }
        if selected {
            ui.rounded(x + 8.0, ty + TABS_H - 3.0, tw - 16.0, 3.0, 1.5, ACCENT);
        }
        self.hit(ui, x, by, tw, bh, action);
        x + tw + 2.0
    }

    fn draw_grid(
        &mut self,
        ui: &mut Ui,
        e: &Extras,
        (mx, top, mw, view): (f32, f32, f32, f32),
        icons: &mut Vec<Icon>,
    ) {
        let cards = self.cards(e);
        let cols = (((mw + GAP) / (TILE_W + GAP)).floor() as usize).max(2);
        let cw = ((mw - GAP * (cols as f32 - 1.0)) / cols as f32).floor();
        let rows = cards.len().div_ceil(cols);
        let content = rows as f32 * (TILE_H + GAP) - GAP;
        self.scroll_room = (content - view).max(0.0);
        self.scroll = self.scroll.min(self.scroll_room);
        if cards.is_empty() {
            let empty = lang::get_or("wisp.menu.empty", "No module matches the search.");
            let ew = ui.measure(Face::Regular, 14.0, &empty);
            ui.text(
                Face::Regular,
                14.0,
                mx + (mw - ew) / 2.0,
                top + 40.0,
                MUTED,
                &empty,
            );
            return;
        }
        let start = ui.mark();
        let clip = ui.gui_rect(mx - 4.0, top, mw + 8.0, view);
        let hits_before = self.hits.len();
        let mut tip = None;
        for (i, card) in cards.iter().enumerate() {
            let x = mx + (i % cols) as f32 * (cw + GAP);
            let y = top + (i / cols) as f32 * (TILE_H + GAP) - self.scroll;
            if y + TILE_H < top || y > top + view {
                continue;
            }
            if self.draw_card(ui, card, x, y, cw, clip, icons) && hover(ui, mx, top, mw, view) {
                tip = Some(card.description.clone());
            }
        }
        ui.clip_since(start, mx - 4.0, top, mw + 8.0, view);
        // Targets scrolled out of the area don't take clicks.
        let [cx, cy, cwg, chg] = clip;
        for (rect, _) in &mut self.hits[hits_before..] {
            let y0 = rect[1].max(cy);
            let y1 = (rect[1] + rect[3]).min(cy + chg);
            rect[1] = y0;
            rect[3] = (y1 - y0).max(0.0);
            rect[0] = rect[0].max(cx);
            rect[2] = rect[2].min(cx + cwg - rect[0]);
        }
        if self.scroll_room > 0.0 {
            let track = view - 8.0;
            let thumb = (view / content * track).max(24.0);
            let ty = top + 4.0 + (track - thumb) * (self.scroll / self.scroll_room);
            ui.rounded(mx + mw + 9.0, ty, 3.0, thumb, 1.5, hex(0xFFFFFF, 40));
        }
        if let Some(text) = tip {
            draw_tip(ui, &text);
        }
    }

    /// A module's tile: its item big in the middle, its name, and the bar that turns it on or
    /// off. A click on the tile opens its settings. Returns whether the cursor is over it.
    #[allow(clippy::too_many_arguments)]
    fn draw_card(
        &mut self,
        ui: &mut Ui,
        card: &Card,
        x: f32,
        y: f32,
        w: f32,
        clip: [f32; 4],
        icons: &mut Vec<Icon>,
    ) -> bool {
        let hovered = hover(ui, x, y, w, TILE_H);
        ui.rounded(
            x,
            y,
            w,
            TILE_H,
            12.0,
            if hovered { CARD_HOVER } else { CARD },
        );
        ui.border(
            x,
            y,
            w,
            TILE_H,
            12.0,
            if card.on { hex(0x22C55E, 70) } else { STROKE },
        );
        self.hit(ui, x, y, w, TILE_H, Action::Open(card.feature.key));
        // The item on a soft disc.
        let (cx, cy) = (x + w / 2.0, y + 50.0);
        let disc = if card.on {
            hex(0x22C55E, 30)
        } else {
            hex(0xFFFFFF, 10)
        };
        ui.circle(cx, cy, 31.0, disc);
        let [gx, gy, gw, _] = ui.gui_rect(cx - 20.0, cy - 20.0, 40.0, 40.0);
        icons.push(Icon {
            item: card.feature.icon,
            x: gx,
            y: gy,
            size: gw,
            clip,
        });
        // Its settings, on hover: three dots in the corner.
        if hovered && (!card.feature.settings.is_empty() || card.feature.widget) {
            for i in 0..3 {
                ui.circle(x + w - 22.0 + i as f32 * 5.0, y + 14.0, 1.6, MUTED);
            }
        }
        let name = fit(ui, Face::Medium, 14.0, &card.name, w - 20.0);
        let nw = ui.measure(Face::Medium, 14.0, &name);
        ui.text(Face::Medium, 14.0, cx - nw / 2.0, y + 88.0, INK, &name);
        let tag = if card.feature.widget {
            lang::get_or("wisp.menu.tag.widget", "HUD widget")
        } else {
            lang::get_or(
                card.feature.category.lang_key(),
                card.feature.category.name(),
            )
        };
        let tag = fit(ui, Face::Regular, 11.0, &tag, w - 20.0);
        let tgw = ui.measure(Face::Regular, 11.0, &tag);
        ui.text(Face::Regular, 11.0, cx - tgw / 2.0, y + 107.0, FAINT, &tag);
        // The bar: green Enabled or grey Disabled.
        let (bx, by, bw, bh) = (x + 8.0, y + TILE_H - 8.0 - 26.0, w - 16.0, 26.0);
        let over = hover(ui, bx, by, bw, bh);
        let fill = match (card.on, over) {
            (true, false) => ON,
            (true, true) => ON_HOVER,
            (false, false) => hex(0xFFFFFF, 16),
            (false, true) => hex(0xFFFFFF, 28),
        };
        ui.rounded(bx, by, bw, bh, 8.0, fill);
        let label = if card.on {
            lang::get_or("wisp.menu.enabled", "Enabled")
        } else {
            lang::get_or("wisp.menu.disabled", "Disabled")
        }
        .to_uppercase();
        let lw = ui.measure(Face::SemiBold, 11.0, &label);
        let colour = if card.on { INK } else { MUTED };
        ui.text_line(
            Face::SemiBold,
            11.0,
            bh,
            bx + (bw - lw) / 2.0,
            by,
            colour,
            &label,
        );
        self.hit(ui, bx, by, bw, bh, Action::Toggle(card.feature.key));
        hovered
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_settings(
        &mut self,
        ui: &mut Ui,
        e: &Extras,
        f: &'static Feature,
        (mx, my, mw, mh): (f32, f32, f32, f32),
        key_name: &dyn Fn(&str) -> String,
        icons: &mut Vec<Icon>,
    ) {
        // Back.
        let back = format!("\u{2039}  {}", lang::get_or("gui.back", "Back"));
        let bw = ui.measure(Face::Medium, 13.0, &back) + 24.0;
        if hover(ui, mx - 8.0, my - 4.0, bw, 30.0) {
            ui.rounded(mx - 8.0, my - 4.0, bw, 30.0, 10.0, CARD_HOVER);
        }
        ui.text_line(
            Face::Medium,
            13.0,
            30.0,
            mx + 4.0,
            my - 4.0,
            INK_SOFT,
            &back,
        );
        self.hit(ui, mx - 8.0, my - 4.0, bw, 30.0, Action::Back);
        // The head: icon, name, description and the switch.
        let (name, description) = features::texts(f.key);
        let on = e.feature_on(f.key);
        let hy = my + 38.0;
        let tile = if on {
            hex(0x22C55E, 40)
        } else {
            hex(0xFFFFFF, 14)
        };
        ui.rounded(mx, hy, 52.0, 52.0, 14.0, tile);
        let whole = ui.gui_rect(0.0, 0.0, ui.w, ui.h);
        let [gx, gy, gw, _] = ui.gui_rect(mx + 10.0, hy + 10.0, 32.0, 32.0);
        icons.push(Icon {
            item: f.icon,
            x: gx,
            y: gy,
            size: gw,
            clip: whole,
        });
        ui.text(Face::Heading, 24.0, mx + 66.0, hy - 2.0, INK, &name);
        let lines = wrap(ui, Face::Regular, 13.0, &description, mw - 66.0 - 70.0, 2);
        for (i, line) in lines.iter().enumerate() {
            ui.text(
                Face::Regular,
                13.0,
                mx + 66.0,
                hy + 30.0 + i as f32 * 18.0,
                MUTED,
                line,
            );
        }
        let (sx, sy) = (mx + mw - 38.0, hy + 4.0);
        draw_switch(ui, sx, sy, on);
        self.hit(ui, sx - 6.0, sy - 6.0, 50.0, 34.0, Action::Toggle(f.key));
        // The rows.
        let top = hy + 76.0;
        let foot = 44.0;
        let view = my + mh - foot - top;
        let mut rows: Vec<Row> = f.settings.iter().map(Row::Setting).collect();
        if f.widget {
            rows.push(Row::Heading(lang::get_or("wisp.menu.look", "Look")));
            rows.extend(WIDGET_SETTINGS.iter().map(Row::Setting));
            rows.push(Row::Position);
        }
        if rows.is_empty() {
            let none = lang::get_or(
                "wisp.menu.no_settings",
                "This module has nothing to set: turn it on or off.",
            );
            ui.text(Face::Regular, 14.0, mx, top + 8.0, MUTED, &none);
        }
        let content: f32 = rows.iter().map(Row::height).sum();
        self.scroll_room = (content - view).max(0.0);
        self.scroll = self.scroll.min(self.scroll_room);
        let start = ui.mark();
        let hits_before = self.hits.len();
        let mut y = top - self.scroll;
        for row in &rows {
            let rh = row.height();
            if y + rh >= top && y <= top + view {
                self.draw_row(ui, e, f, row, mx, y, mw, key_name);
            }
            y += rh;
        }
        ui.clip_since(start, mx - 4.0, top, mw + 8.0, view);
        let clip = ui.gui_rect(mx - 4.0, top, mw + 8.0, view);
        for (rect, _) in &mut self.hits[hits_before..] {
            let y0 = rect[1].max(clip[1]);
            let y1 = (rect[1] + rect[3]).min(clip[1] + clip[3]);
            rect[1] = y0;
            rect[3] = (y1 - y0).max(0.0);
        }
        // Reset, at the foot.
        if !f.settings.is_empty() || f.widget {
            let label = lang::get_or("wisp.menu.reset", "Reset to defaults");
            let rw = ui.measure(Face::Medium, 13.0, &label) + 28.0;
            let (rx, ry) = (mx + mw - rw, my + mh - 34.0);
            let fill = if hover(ui, rx, ry, rw, 32.0) {
                CARD_HOVER
            } else {
                CARD
            };
            ui.rounded(rx, ry, rw, 32.0, 10.0, fill);
            ui.text_line(Face::Medium, 13.0, 32.0, rx + 14.0, ry, INK_SOFT, &label);
            self.hit(ui, rx, ry, rw, 32.0, Action::Reset(f.key));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_row(
        &mut self,
        ui: &mut Ui,
        e: &Extras,
        f: &'static Feature,
        row: &Row,
        x: f32,
        y: f32,
        w: f32,
        key_name: &dyn Fn(&str) -> String,
    ) {
        let s = match row {
            Row::Heading(text) => {
                ui.text(
                    Face::Medium,
                    12.0,
                    x,
                    y + 14.0,
                    ACCENT_SOFT,
                    &text.to_uppercase(),
                );
                return;
            }
            Row::Position => {
                let label = lang::get_or("wisp.menu.position", "Position");
                ui.text_line(Face::Regular, 14.0, ROW_H, x, y, INK_SOFT, &label);
                let button = lang::get_or("wisp.menu.edit_hud", "Edit HUD");
                let bw = ui.measure(Face::Medium, 13.0, &button) + 28.0;
                let (bx, by) = (x + w - bw, y + (ROW_H - 32.0) / 2.0);
                let fill = if hover(ui, bx, by, bw, 32.0) {
                    ACCENT_HOVER
                } else {
                    ACCENT
                };
                ui.rounded(bx, by, bw, 32.0, 10.0, fill);
                ui.text_line(Face::Medium, 13.0, 32.0, bx + 14.0, by, ON_ACCENT, &button);
                self.hit(ui, bx, by, bw, 32.0, Action::EditHud);
                return;
            }
            Row::Setting(s) => *s,
        };
        ui.rounded(x, y + ROW_H - 1.0, w, 1.0, 0.0, hex(0xFFFFFF, 14));
        let label = lang::get_or(&format!("wisp.menu.setting.{}.{}", f.key, s.key), s.label);
        ui.text_line(Face::Regular, 14.0, ROW_H, x, y, INK_SOFT, &label);
        let right = x + w;
        let cy = y + ROW_H / 2.0;
        match s.kind {
            Kind::Toggle(_) => {
                let on = e.opt_bool(f.key, s.key);
                draw_switch(ui, right - 38.0, cy - 11.0, on);
                self.hit(
                    ui,
                    right - 44.0,
                    cy - 17.0,
                    50.0,
                    34.0,
                    Action::Setting(f.key, s.key, Control::Toggle),
                );
            }
            Kind::Choice(names, _) => {
                let current = e.opt_choice(f.key, s.key);
                let widths: Vec<f32> = names
                    .iter()
                    .map(|n| ui.measure(Face::Medium, 12.0, n) + 22.0)
                    .collect();
                let total: f32 = widths.iter().sum::<f32>() + 4.0;
                let (tx, th) = (right - total, 30.0);
                ui.rounded(tx, cy - th / 2.0, total, th, 10.0, FIELD);
                let mut sx = tx + 2.0;
                for (i, (name, sw)) in names.iter().zip(&widths).enumerate() {
                    if i == current {
                        ui.rounded(sx, cy - th / 2.0 + 2.0, *sw, th - 4.0, 8.0, ACCENT);
                    } else if hover(ui, sx, cy - th / 2.0, *sw, th) {
                        ui.rounded(sx, cy - th / 2.0 + 2.0, *sw, th - 4.0, 8.0, CARD_HOVER);
                    }
                    let colour = if i == current { ON_ACCENT } else { MUTED };
                    ui.text_line(
                        Face::Medium,
                        12.0,
                        th,
                        sx + 11.0,
                        cy - th / 2.0,
                        colour,
                        name,
                    );
                    self.hit(
                        ui,
                        sx,
                        cy - th / 2.0,
                        *sw,
                        th,
                        Action::Setting(f.key, s.key, Control::Choice(i)),
                    );
                    sx += sw;
                }
            }
            Kind::Color(_) => {
                let current = e.opt_color(f.key, s.key);
                let (d, gap) = (18.0, 7.0);
                let mut sx = right - SWATCHES.len() as f32 * (d + gap) + gap;
                for c in SWATCHES {
                    let (cx0, cy0) = (sx + d / 2.0, cy);
                    if c == current {
                        ui.border(
                            cx0 - d / 2.0 - 3.0,
                            cy0 - d / 2.0 - 3.0,
                            d + 6.0,
                            d + 6.0,
                            d / 2.0 + 3.0,
                            INK,
                        );
                    }
                    ui.circle(cx0, cy0, d / 2.0, hex(c, 255));
                    if c == 0x1F1B33 {
                        ui.border(
                            cx0 - d / 2.0,
                            cy0 - d / 2.0,
                            d,
                            d,
                            d / 2.0,
                            hex(0xFFFFFF, 60),
                        );
                    }
                    self.hit(
                        ui,
                        sx - 2.0,
                        cy - d / 2.0 - 4.0,
                        d + 4.0,
                        d + 8.0,
                        Action::Setting(f.key, s.key, Control::Color(c)),
                    );
                    sx += d + gap;
                }
            }
            Kind::Slider { min, max, unit, .. } => {
                let v = e.opt_f32(f.key, s.key);
                let tw = 200.0_f32.min(w * 0.4);
                let tx = right - tw - 8.0;
                let t = if max > min {
                    (v - min) / (max - min)
                } else {
                    0.0
                };
                ui.rounded(tx, cy - 2.0, tw, 4.0, 2.0, hex(0xFFFFFF, 34));
                ui.rounded(tx, cy - 2.0, tw * t, 4.0, 2.0, ACCENT);
                let knob = tx + tw * t;
                ui.circle(knob, cy, 8.0, INK);
                ui.circle(knob, cy, 3.0, ACCENT);
                let value = if (max - min) <= 4.0 {
                    format!("{v:.2}{unit}")
                } else {
                    format!("{v:.0}{unit}")
                };
                let vw = ui.measure(Face::Mono, 12.0, &value);
                ui.text_line(Face::Mono, 12.0, ROW_H, tx - 16.0 - vw, y, INK, &value);
                let [x0, _, gw, _] = ui.gui_rect(tx, cy, tw, 1.0);
                self.hit(
                    ui,
                    tx - 10.0,
                    cy - 14.0,
                    tw + 20.0,
                    28.0,
                    Action::Setting(f.key, s.key, Control::Slider { x0, x1: x0 + gw }),
                );
            }
            Kind::Key(_) => {
                let waiting = self.waiting_key == Some((f.key, s.key));
                let label = if waiting {
                    lang::get_or("wisp.menu.press_key", "Press a key\u{2026}")
                } else {
                    key_name(&e.opt_key(f.key, s.key))
                };
                let bw = (ui.measure(Face::Medium, 13.0, &label) + 32.0).max(110.0);
                let (bx, by) = (right - bw, cy - 16.0);
                let fill = if hover(ui, bx, by, bw, 32.0) {
                    CARD_HOVER
                } else {
                    FIELD
                };
                ui.rounded(bx, by, bw, 32.0, 10.0, fill);
                if waiting {
                    ui.border(bx, by, bw, 32.0, 10.0, ACCENT_SOFT);
                }
                let lw = ui.measure(Face::Medium, 13.0, &label);
                ui.text_line(
                    Face::Medium,
                    13.0,
                    32.0,
                    bx + (bw - lw) / 2.0,
                    by,
                    INK,
                    &label,
                );
                self.hit(
                    ui,
                    bx,
                    by,
                    bw,
                    32.0,
                    Action::Setting(f.key, s.key, Control::Key),
                );
            }
        }
    }
}

/// A row of a settings page.
enum Row {
    Setting(&'static Setting),
    Heading(String),
    Position,
}
impl Row {
    fn height(&self) -> f32 {
        match self {
            Row::Heading(_) => 40.0,
            _ => ROW_H,
        }
    }
}

fn hover(ui: &Ui, x: f32, y: f32, w: f32, h: f32) -> bool {
    let (cx, cy) = ui.cursor;
    cx >= x && cx < x + w && cy >= y && cy < y + h
}
/// A switch: a 38x22 pill, the accent when on, with a white knob.
fn draw_switch(ui: &mut Ui, x: f32, y: f32, on: bool) {
    ui.rounded(x, y, 38.0, 22.0, 11.0, if on { ON } else { SWITCH_OFF });
    let kx = if on { x + 27.0 } else { x + 11.0 };
    ui.circle(kx, y + 11.0, 8.0, INK);
}
/// The text cut with an ellipsis to fit `max` dp.
fn fit(ui: &Ui, face: Face, size: f32, text: &str, max: f32) -> String {
    if ui.measure(face, size, text) <= max {
        return text.to_owned();
    }
    let mut s: String = text.to_owned();
    while !s.is_empty() && ui.measure(face, size, &format!("{s}\u{2026}")) > max {
        s.pop();
    }
    format!("{}\u{2026}", s.trim_end())
}
/// The text wrapped to `width` dp, at most `max_lines` (the last cut with an ellipsis).
fn wrap(ui: &Ui, face: Face, size: f32, text: &str, width: f32, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if ui.measure(face, size, &candidate) <= width || line.is_empty() {
            line = candidate;
        } else {
            lines.push(std::mem::take(&mut line));
            line = word.to_owned();
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            *last = fit(ui, face, size, &format!("{last} \u{2026}"), width);
        }
    }
    lines
}
/// A module's description by the cursor.
fn draw_tip(ui: &mut Ui, text: &str) {
    if text.is_empty() {
        return;
    }
    let width = 240.0;
    let lines = wrap(ui, Face::Regular, 12.0, text, width - 20.0, 4);
    let tw = lines
        .iter()
        .map(|l| ui.measure(Face::Regular, 12.0, l))
        .fold(0.0, f32::max)
        + 20.0;
    let th = lines.len() as f32 * 16.0 + 14.0;
    let (cx, cy) = ui.cursor;
    let x = (cx + 14.0).min(ui.w - tw - 6.0);
    let y = if cy + 18.0 + th > ui.h - 6.0 {
        cy - th - 8.0
    } else {
        cy + 18.0
    };
    ui.rounded(x, y, tw, th, 8.0, hex(0x1D1D23, 250));
    ui.border(x, y, tw, th, 8.0, hex(0xFFFFFF, 30));
    for (i, line) in lines.iter().enumerate() {
        ui.text(
            Face::Regular,
            12.0,
            x + 10.0,
            y + 6.0 + i as f32 * 16.0,
            INK_SOFT,
            line,
        );
    }
}

/// The text cursor's blink: on for half of each second.
fn blink() -> bool {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(true, |d| d.subsec_millis() < 500)
}
