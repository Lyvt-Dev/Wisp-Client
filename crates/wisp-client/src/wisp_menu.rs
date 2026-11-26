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
pub use store::{
    StoreClick, StoreField, StorePage, coins as store_coins, kind_name as store_kind_name,
};

use crate::{
    extras::{
        Extras,
        features::{self, Category, Feature, Kind, SWATCHES, Setting, TEXT_MAX, WIDGET_GROUPS},
    },
    lang,
    ui_theme::{
        Face, Ui,
        color::hex,
        festive::{self, accent, accent_alpha, accent_hover, on_accent},
    },
};

// The launcher's dark look in the HUD encoding: neutral greys, the violet for what is selected,
// green for what is on.
const SCRIM: u32 = 0x050507;
const PANEL: u32 = hex(0x111217, 238);
const SIDEBAR: u32 = hex(0x0B0C10, 246);
const ACCENT_SOFT: u32 = hex(0xD4D4D8, 255);
// The accent itself (and the text on it) comes from `ui_theme::festive`: near white with dark
// text, or Christmas red with white text.
const INK: u32 = hex(0xFFFFFF, 255);
const INK_SOFT: u32 = hex(0xFFFFFF, 210);
const MUTED: u32 = hex(0xFFFFFF, 150);
const FAINT: u32 = hex(0xFFFFFF, 100);
const STROKE: u32 = hex(0xFFFFFF, 18);
const CARD: u32 = hex(0xFFFFFF, 10);
const CARD_HOVER: u32 = hex(0xFFFFFF, 19);
const FIELD: u32 = hex(0xFFFFFF, 14);
const SWITCH_OFF: u32 = hex(0xFFFFFF, 38);

/// The sidebar's width, the page's head (title, search, close) and the chips under it, in dp.
const SIDEBAR_W: f32 = 228.0;
const HEAD_H: f32 = 62.0;
const CHIPS_H: f32 = 42.0;
const TILE_W: f32 = 168.0;
const TILE_H: f32 = 140.0;
/// A module tile's name bar, which turns it on or off.
const TILE_BAR: f32 = 44.0;
const GAP: f32 = 12.0;
const ROW_H: f32 = 50.0;
const RADIUS: f32 = 16.0;
/// The menu slides up and the world darkens over this long when it opens, in seconds.
const OPEN_ANIM: f32 = 0.16;
/// The live preview's stage on a widget's settings page.
const PREVIEW_H: f32 = 92.0;

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
    /// Type the setting's text.
    Text,
    /// Type a colour setting's hex code.
    Hex,
    /// A colour setting's hue strip from `x0` to `x1` in GUI pixels: the hue follows the cursor.
    Hue {
        x0: f32,
        x1: f32,
    },
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
    /// The slider (or colour's hue strip) being dragged, and its track in GUI pixels.
    pub dragging: Option<(&'static str, &'static str, f32, f32)>,
    /// The text or hex setting being typed, and what is typed so far.
    pub text_edit: Option<(&'static str, &'static str)>,
    pub text_input: String,
    /// Where the open widget's live preview goes (GUI pixels), as last drawn.
    pub preview: Option<[f32; 4]>,
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
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        ui: &mut Ui,
        e: &Extras,
        key_name: &dyn Fn(&str) -> String,
        friends: FriendsView,
        store: Option<&wisp_cosmetics::share::ShopState>,
        player: &str,
    ) -> Vec<Icon> {
        self.hits.clear();
        self.preview = None;
        let mut icons = Vec::new();
        let (w, h) = (ui.w, ui.h);
        let t = self
            .opened
            .map_or(1.0, |at| (at.elapsed().as_secs_f32() / OPEN_ANIM).min(1.0));
        let ease = 1.0 - (1.0 - t).powi(3);
        ui.rounded(0.0, 0.0, w, h, 0.0, hex(SCRIM, (110.0 * ease) as u8));
        // Wisp Christmas: snow falling over the world behind the panel.
        if festive::on() {
            festive::snow(ui, (0.0, 0.0, w, h), ease);
        }
        self.hit(ui, 0.0, 0.0, w, h, Action::Close);
        let pw = (w - 48.0).clamp(520.0, 1120.0);
        let ph = (h - 48.0).clamp(340.0, 660.0);
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
        self.draw_sidebar(ui, e, (px, py, ph), friends, store, player);
        // Fairy lights hung along the panel's top edge.
        if festive::on() {
            festive::lights(ui, px + RADIUS, py - 3.0, pw - 2.0 * RADIUS);
        }
        let (cx, cw) = (px + SIDEBAR_W + 28.0, pw - SIDEBAR_W - 56.0);
        let modules = !self.friends_page && self.store_page.is_none();
        let grid = modules && self.open.is_none();
        self.draw_head(ui, e, (cx, py + 22.0, cw), grid);
        let top = if grid {
            self.draw_chips(ui, e, cx, py + 22.0 + HEAD_H, cw);
            py + 22.0 + HEAD_H + CHIPS_H + 10.0
        } else {
            py + 22.0 + HEAD_H + 4.0
        };
        let (mx, my) = (cx, top);
        let (mw, mh) = (cw, py + ph - top - 20.0);
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

    /// The sidebar: the wordmark, Edit HUD, the pages in their sections, the player at the foot.
    fn draw_sidebar(
        &mut self,
        ui: &mut Ui,
        e: &Extras,
        (px, py, ph): (f32, f32, f32),
        friends: FriendsView,
        store: Option<&wisp_cosmetics::share::ShopState>,
        player: &str,
    ) {
        // Its plate keeps the panel's rounded left corners; a rule parts it from the page.
        ui.rounded(px, py, SIDEBAR_W, ph, RADIUS, SIDEBAR);
        ui.rounded(px + SIDEBAR_W - RADIUS, py, RADIUS, ph, 0.0, SIDEBAR);
        ui.rounded(px + SIDEBAR_W, py, 1.0, ph, 0.0, STROKE);
        // The mark: Wisp's white feather on a black tile, and the wordmark.
        let (bx, by, side) = (px + 22.0, py + 24.0, 34.0);
        let tile = hex(0x000000, 255);
        ui.rounded(bx, by, side, side, 10.0, tile);
        ui.border(bx, by, side, side, 10.0, STROKE);
        ui.feather(bx + side / 2.0, by + side / 2.0, side, INK, tile);
        if festive::on() {
            festive::hat(ui, bx, by, side);
        }
        ui.text_line(Face::Heading, 22.0, side, bx + 46.0, by - 1.0, INK, "WISP");
        let (x, w) = (px + 14.0, SIDEBAR_W - 28.0);
        let mut y = py + 82.0;
        let label = lang::get_or("wisp.menu.edit_hud", "Edit HUD");
        y = self.side_row(
            ui,
            (x, y, w),
            Glyph::Hud,
            &label,
            false,
            None,
            Action::EditHud,
        );
        // Modules.
        let other_page = self.friends_page || self.store_page.is_some();
        let section = lang::get_or("wisp.menu.section.modules", "Mods & options");
        y = side_section(ui, x, y, &section);
        let on = features::FEATURES
            .iter()
            .filter(|f| e.feature_on(f.key))
            .count();
        let badge = (on > 0).then(|| (on.to_string(), hex(0xFFFFFF, 22)));
        let label = lang::get_or("wisp.menu.mods", "Mods");
        y = self.side_row(
            ui,
            (x, y, w),
            Glyph::Mods,
            &label,
            !other_page,
            badge,
            Action::Category(self.category),
        );
        // Social.
        let section = lang::get_or("wisp.menu.section.social", "Social");
        y = side_section(ui, x, y, &section);
        let list = friends.state.and_then(|s| s.list.as_ref());
        let waiting = list.map_or(0, |l| l.incoming.len());
        let online = list.map_or(0, |l| l.friends.iter().filter(|f| f.online).count());
        let badge = if waiting > 0 {
            Some((waiting.to_string(), hex(0xFF6B6B, 230)))
        } else {
            (online > 0).then(|| (online.to_string(), hex(0x22C55E, 150)))
        };
        let label = lang::get_or("wisp.menu.friends", "Friends");
        y = self.side_row(
            ui,
            (x, y, w),
            Glyph::Friends,
            &label,
            self.friends_page,
            badge,
            Action::Friends,
        );
        // The Shop: Store for everyone, Creator for creators, Admin for the owner.
        let section = lang::get_or("wisp.menu.section.shop", "Shop");
        y = side_section(ui, x, y, &section);
        let me = store.map(|s| s.me.clone()).unwrap_or_default();
        let pages = [
            (
                StorePage::Shop,
                Glyph::Store,
                "wisp.menu.store",
                "Store",
                true,
            ),
            (
                StorePage::Creator,
                Glyph::Creator,
                "wisp.menu.creator",
                "Creator",
                me.creator,
            ),
            (
                StorePage::Admin,
                Glyph::Admin,
                "wisp.menu.admin",
                "Admin",
                me.admin,
            ),
        ];
        for (page, glyph, key, fallback, shown) in pages {
            if !shown {
                continue;
            }
            let label = lang::get_or(key, fallback);
            let badge = (page == StorePage::Creator && me.supporters > 0)
                .then(|| (me.supporters.to_string(), hex(0xF2C14E, 150)));
            y = self.side_row(
                ui,
                (x, y, w),
                glyph,
                &label,
                self.store_page == Some(page),
                badge,
                Action::Store(StoreClick::Page(page)),
            );
        }
        // The player, at the foot.
        let fy = py + ph - 64.0;
        if y < fy && !player.is_empty() {
            ui.rounded(px, fy, SIDEBAR_W, 1.0, 0.0, STROKE);
            let (ax, ay, side) = (px + 20.0, fy + 16.0, 32.0);
            ui.rounded(ax, ay, side, side, 9.0, hex(0xFFFFFF, 20));
            let initial = player.chars().take(1).collect::<String>().to_uppercase();
            let iw = ui.measure(Face::SemiBold, 15.0, &initial);
            ui.text_line(
                Face::SemiBold,
                15.0,
                side,
                ax + (side - iw) / 2.0,
                ay,
                INK,
                &initial,
            );
            let name = fit(ui, Face::Medium, 13.0, player, SIDEBAR_W - 76.0);
            ui.text(Face::Medium, 13.0, ax + 44.0, ay - 1.0, INK, &name);
            let client = lang::get_or("wisp.menu.client_name", "Wisp Client");
            ui.text(Face::Regular, 11.0, ax + 44.0, ay + 17.0, MUTED, &client);
        }
    }

    /// One page in the sidebar; returns where the next row starts.
    #[allow(clippy::too_many_arguments)]
    fn side_row(
        &mut self,
        ui: &mut Ui,
        (x, y, w): (f32, f32, f32),
        glyph: Glyph,
        label: &str,
        selected: bool,
        badge: Option<(String, u32)>,
        action: Action,
    ) -> f32 {
        let h = 36.0;
        let hovered = hover(ui, x, y, w, h);
        if selected {
            ui.rounded(x, y, w, h, 10.0, accent());
        } else if hovered {
            ui.rounded(x, y, w, h, 10.0, CARD_HOVER);
        }
        let colour = if selected {
            on_accent()
        } else if hovered {
            INK
        } else {
            INK_SOFT
        };
        draw_glyph(ui, glyph, x + 12.0, y + 10.0, colour);
        let face = if selected {
            Face::Medium
        } else {
            Face::Regular
        };
        ui.text_line(face, 13.0, h, x + 38.0, y, colour, label);
        if let Some((b, fill)) = badge {
            let bw = ui.measure(Face::Mono, 10.0, &b) + 10.0;
            let (bx, bh) = (x + w - 10.0 - bw, 16.0);
            let by = y + (h - bh) / 2.0;
            let (fill, ink) = if selected {
                (hex(0x000000, 40), on_accent())
            } else {
                (fill, INK)
            };
            ui.rounded(bx, by, bw, bh, 8.0, fill);
            ui.text_line(Face::Mono, 10.0, bh, bx + 5.0, by, ink, &b);
        }
        self.hit(ui, x, y, w, h, action);
        y + h + 4.0
    }

    /// The page's head: Back on a module's page, the title, the search over the modules and
    /// the close button.
    fn draw_head(&mut self, ui: &mut Ui, e: &Extras, (x, y, w): (f32, f32, f32), grid: bool) {
        let h = HEAD_H - 16.0;
        // Close, at the right.
        let (qx, qy, qs) = (x + w - 32.0, y + (h - 32.0) / 2.0, 32.0);
        if hover(ui, qx, qy, qs, qs) {
            ui.rounded(qx, qy, qs, qs, 10.0, CARD_HOVER);
        }
        draw_glyph(ui, Glyph::Close, qx + 9.0, qy + 9.0, INK_SOFT);
        self.hit(ui, qx, qy, qs, qs, Action::Close);
        let mut tx = x;
        let module = if self.friends_page || self.store_page.is_some() {
            None
        } else {
            self.open.and_then(features::feature)
        };
        if module.is_some() {
            let (bx, by, bs) = (x - 6.0, y + (h - 32.0) / 2.0, 32.0);
            if hover(ui, bx, by, bs, bs) {
                ui.rounded(bx, by, bs, bs, 10.0, CARD_HOVER);
            }
            draw_glyph(ui, Glyph::Back, bx + 9.0, by + 9.0, INK);
            self.hit(ui, bx, by, bs, bs, Action::Back);
            tx += 34.0;
        }
        let title = if let Some(page) = self.store_page {
            match page {
                StorePage::Shop => lang::get_or("wisp.menu.store", "Store"),
                StorePage::Creator => lang::get_or("wisp.menu.creator", "Creator"),
                StorePage::Admin => lang::get_or("wisp.menu.admin", "Admin"),
            }
        } else if self.friends_page {
            lang::get_or("wisp.menu.friends", "Friends")
        } else if let Some(f) = module {
            features::texts(f.key).0
        } else {
            lang::get_or("wisp.menu.mods", "Mods")
        };
        let title = fit(ui, Face::Heading, 26.0, &title, w * 0.45);
        let tw = ui.measure(Face::Heading, 26.0, &title);
        ui.text_line(Face::Heading, 26.0, h, tx, y - 1.0, INK, &title);
        if !grid {
            return;
        }
        // How many are on, beside the title.
        let cards = self.cards(e);
        let on = cards.iter().filter(|c| c.on).count();
        let summary = format!(
            "{on} / {} {}",
            cards.len(),
            lang::get_or("wisp.menu.on", "on")
        );
        ui.text_line(
            Face::Regular,
            12.0,
            h,
            tx + tw + 12.0,
            y + 3.0,
            MUTED,
            &summary,
        );
        // The search.
        let sw = 260.0_f32.min(w * 0.36);
        let (sx, sy, sh) = (qx - 10.0 - sw, y + (h - 34.0) / 2.0, 34.0);
        ui.rounded(sx, sy, sw, sh, 10.0, FIELD);
        let edge = if self.search_focused {
            ACCENT_SOFT
        } else {
            STROKE
        };
        ui.border(sx, sy, sw, sh, 10.0, edge);
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

    /// The categories as chips over the grid, the chosen one filled in the accent.
    fn draw_chips(&mut self, ui: &mut Ui, e: &Extras, x0: f32, y: f32, w: f32) {
        let mut x = x0;
        let ch = CHIPS_H - 8.0;
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
            let selected = self.category == tab;
            let nw = ui.measure(Face::Medium, 13.0, &name);
            let count = (count > 0).then(|| count.to_string());
            let extra = count
                .as_ref()
                .map_or(0.0, |c| ui.measure(Face::Mono, 10.0, c) + 8.0);
            let cw = nw + extra + 26.0;
            if x + cw > x0 + w {
                break;
            }
            let hovered = hover(ui, x, y, cw, ch);
            let (fill, ink) = if selected {
                (accent(), on_accent())
            } else if hovered {
                (CARD_HOVER, INK)
            } else {
                (CARD, INK_SOFT)
            };
            ui.rounded(x, y, cw, ch, 10.0, fill);
            if !selected {
                ui.border(x, y, cw, ch, 10.0, STROKE);
            }
            ui.text_line(Face::Medium, 13.0, ch, x + 13.0, y, ink, &name);
            if let Some(c) = count {
                let muted = if selected {
                    hex(wisp_theme::glass::ON_LIGHT, 150)
                } else {
                    MUTED
                };
                ui.text_line(Face::Mono, 10.0, ch, x + 21.0 + nw, y + 1.0, muted, &c);
            }
            self.hit(ui, x, y, cw, ch, Action::Category(tab));
            x += cw + 8.0;
        }
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

    /// A module's tile: its item in the middle, its name and the switch that turns it on or off
    /// at the foot. A click on the tile opens its settings. Returns whether the cursor is over it.
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
        // On: a lighter plate and a clear edge; off: the quiet card.
        let plate = match (card.on, hovered) {
            (true, false) => hex(0xFFFFFF, 18),
            (true, true) => hex(0xFFFFFF, 26),
            (false, false) => CARD,
            (false, true) => CARD_HOVER,
        };
        ui.rounded(x, y, w, TILE_H, 12.0, plate);
        let edge = if card.on { hex(0xFFFFFF, 70) } else { STROKE };
        ui.border(x, y, w, TILE_H, 12.0, edge);
        self.hit(ui, x, y, w, TILE_H, Action::Open(card.feature.key));
        // The item on a soft disc, in the middle above the foot.
        let (cx, cy) = (x + w / 2.0, y + (TILE_H - TILE_BAR) / 2.0 + 2.0);
        ui.circle(cx, cy, 32.0, hex(0xFFFFFF, if card.on { 12 } else { 6 }));
        let [gx, gy, gw, _] = ui.gui_rect(cx - 20.0, cy - 20.0, 40.0, 40.0);
        icons.push(Icon {
            item: card.feature.icon,
            x: gx,
            y: gy,
            size: gw,
            clip,
        });
        // Its settings, on hover, as three dots.
        if hovered && (!card.feature.settings.is_empty() || card.feature.widget) {
            for i in 0..3 {
                ui.circle(x + w - 22.0 + i as f32 * 5.0, y + 16.0, 1.6, INK_SOFT);
            }
        }
        // The foot: the name and the switch, which turns the module on or off.
        let (by, bh) = (y + TILE_H - TILE_BAR, TILE_BAR);
        ui.rounded(x + 12.0, by, w - 24.0, 1.0, 0.0, hex(0xFFFFFF, 14));
        let ink = if card.on { INK } else { INK_SOFT };
        let name = fit(ui, Face::Medium, 14.0, &card.name, w - 24.0 - 48.0);
        ui.text_line(Face::Medium, 14.0, bh, x + 14.0, by, ink, &name);
        draw_switch(ui, x + w - 14.0 - 38.0, by + (bh - 22.0) / 2.0, card.on);
        self.hit(ui, x, by, w, bh, Action::Toggle(card.feature.key));
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
        // The first card: the item, Enabled with the module's description, and its switch.
        let (_, description) = features::texts(f.key);
        let on = e.feature_on(f.key);
        let lines = wrap(ui, Face::Regular, 12.0, &description, mw - 66.0 - 80.0, 2);
        let hh = 30.0 + lines.len().max(1) as f32 * 17.0;
        let hh = hh.max(64.0);
        let hy = my;
        ui.rounded(mx, hy, mw, hh, 12.0, CARD);
        ui.border(mx, hy, mw, hh, 12.0, STROKE);
        let tile = hex(0xFFFFFF, 14);
        ui.rounded(mx + 12.0, hy + (hh - 40.0) / 2.0, 40.0, 40.0, 10.0, tile);
        let whole = ui.gui_rect(0.0, 0.0, ui.w, ui.h);
        let [gx, gy, gw, _] = ui.gui_rect(mx + 16.0, hy + (hh - 32.0) / 2.0, 32.0, 32.0);
        icons.push(Icon {
            item: f.icon,
            x: gx,
            y: gy,
            size: gw,
            clip: whole,
        });
        let enabled = lang::get_or("wisp.menu.enabled_title", "Enabled");
        let ty = hy + (hh - 18.0 - lines.len() as f32 * 17.0) / 2.0;
        ui.text(Face::Medium, 14.0, mx + 66.0, ty - 2.0, INK, &enabled);
        for (i, line) in lines.iter().enumerate() {
            ui.text(
                Face::Regular,
                12.0,
                mx + 66.0,
                ty + 18.0 + i as f32 * 17.0,
                MUTED,
                line,
            );
        }
        let (sx, sy) = (mx + mw - 16.0 - 38.0, hy + (hh - 22.0) / 2.0);
        draw_switch(ui, sx, sy, on);
        self.hit(ui, mx, hy, mw, hh, Action::Toggle(f.key));
        // A widget's live preview: a stage the game draws the widget on, as it looks now.
        let mut top = hy + hh + 8.0;
        if f.widget {
            let sh = PREVIEW_H;
            ui.rounded(mx, top, mw, sh, 12.0, hex(0x07070A, 200));
            ui.border(mx, top, mw, sh, 12.0, STROKE);
            // A faint grid, as a design canvas has: the plate's edges read on it.
            let step = 16.0;
            let mut gx = mx + (mw % step) / 2.0;
            while gx < mx + mw {
                ui.rounded(gx, top + 1.0, 1.0, sh - 2.0, 0.0, hex(0xFFFFFF, 7));
                gx += step;
            }
            let mut gy = top + (sh % step) / 2.0;
            while gy < top + sh {
                ui.rounded(mx + 1.0, gy, mw - 2.0, 1.0, 0.0, hex(0xFFFFFF, 7));
                gy += step;
            }
            let tag = lang::get_or("wisp.menu.preview", "Preview");
            ui.text_line(
                Face::Medium,
                10.0,
                18.0,
                mx + 12.0,
                top + 6.0,
                FAINT,
                &tag.to_uppercase(),
            );
            self.preview = Some(ui.gui_rect(mx + 8.0, top + 8.0, mw - 16.0, sh - 16.0));
            top += sh + 8.0;
        }
        // The rows.
        let foot = 44.0;
        let view = my + mh - foot - top;
        let mut rows: Vec<Row> = Vec::new();
        if f.widget && !f.settings.is_empty() {
            rows.push(Row::Heading(lang::get_or(
                "wisp.menu.group.content",
                "Content",
            )));
        }
        rows.extend(f.settings.iter().map(Row::Setting));
        if f.widget {
            for (key, name, settings) in WIDGET_GROUPS {
                rows.push(Row::Heading(lang::get_or(key, name)));
                rows.extend(settings.iter().map(Row::Setting));
            }
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
                ui.rounded(x, y, w, ROW_H, 12.0, CARD);
                ui.border(x, y, w, ROW_H, 12.0, STROKE);
                let (x, w) = (x + 16.0, w - 32.0);
                let label = lang::get_or("wisp.menu.position", "Position");
                ui.text_line(Face::Regular, 14.0, ROW_H, x, y, INK_SOFT, &label);
                let button = lang::get_or("wisp.menu.edit_hud", "Edit HUD");
                let bw = ui.measure(Face::Medium, 13.0, &button) + 28.0;
                let (bx, by) = (x + w - bw, y + (ROW_H - 32.0) / 2.0);
                let fill = if hover(ui, bx, by, bw, 32.0) {
                    accent_hover()
                } else {
                    accent()
                };
                ui.rounded(bx, by, bw, 32.0, 10.0, fill);
                ui.text_line(
                    Face::Medium,
                    13.0,
                    32.0,
                    bx + 14.0,
                    by,
                    on_accent(),
                    &button,
                );
                self.hit(ui, bx, by, bw, 32.0, Action::EditHud);
                return;
            }
            Row::Setting(s) => *s,
        };
        // Each setting on its own card; one that doesn't matter now (its switch is off) greyed
        // out and not clickable.
        let rh = row.height() - 6.0;
        let (card_x, card_w) = (x, w);
        ui.rounded(x, y, w, rh, 12.0, CARD);
        ui.border(x, y, w, rh, 12.0, STROKE);
        let enabled = e.opt_enabled(f.key, s);
        let hits_before = self.hits.len();
        let (x, w) = (x + 16.0, w - 32.0);
        let label = lang::get_or(&format!("wisp.menu.setting.{}.{}", f.key, s.key), s.label);
        ui.text_line(Face::Regular, 14.0, ROW_H, x, y, INK_SOFT, &label);
        let right = x + w;
        let cy = y + ROW_H / 2.0;
        self.draw_control(ui, e, f, *s, (x, y, w, right, cy), key_name);
        if !enabled {
            self.hits.truncate(hits_before);
            ui.rounded(card_x, y, card_w, rh, 12.0, hex(0x111217, 170));
        }
    }

    /// A setting's control at the right of its card (a colour's on the line under its label).
    #[allow(clippy::too_many_arguments)]
    fn draw_control(
        &mut self,
        ui: &mut Ui,
        e: &Extras,
        f: &'static Feature,
        s: Setting,
        (x, y, w, right, cy): (f32, f32, f32, f32, f32),
        key_name: &dyn Fn(&str) -> String,
    ) {
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
                        ui.rounded(sx, cy - th / 2.0 + 2.0, *sw, th - 4.0, 8.0, accent());
                    } else if hover(ui, sx, cy - th / 2.0, *sw, th) {
                        ui.rounded(sx, cy - th / 2.0 + 2.0, *sw, th - 4.0, 8.0, CARD_HOVER);
                    }
                    let colour = if i == current { on_accent() } else { MUTED };
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
                // Under the label: the swatches, a hue strip for any other colour, and the hex
                // code, which a click lets the player type.
                let current = e.opt_color(f.key, s.key);
                let ly = y + ROW_H - 6.0;
                let (d, gap) = (18.0, 7.0);
                let mut sx = x;
                for c in SWATCHES {
                    let (cx0, cy0) = (sx + d / 2.0, ly + d / 2.0);
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
                    if c < 0x303030 {
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
                        ly - 4.0,
                        d + 4.0,
                        d + 8.0,
                        Action::Setting(f.key, s.key, Control::Color(c)),
                    );
                    sx += d + gap;
                }
                // The hex code at the right, in a chip of the colour.
                let editing = self.text_edit == Some((f.key, s.key));
                let code = if editing {
                    format!("#{}", self.text_input)
                } else {
                    format!("#{current:06X}")
                };
                let bw = 92.0;
                let (bx, by) = (right - bw, ly - 5.0);
                let fill = if hover(ui, bx, by, bw, 28.0) {
                    CARD_HOVER
                } else {
                    FIELD
                };
                ui.rounded(bx, by, bw, 28.0, 8.0, fill);
                if editing {
                    ui.border(bx, by, bw, 28.0, 8.0, ACCENT_SOFT);
                }
                ui.rounded(bx + 7.0, by + 7.0, 14.0, 14.0, 4.0, hex(current, 255));
                ui.border(bx + 7.0, by + 7.0, 14.0, 14.0, 4.0, hex(0xFFFFFF, 50));
                let tw = ui.text_line(Face::Mono, 12.0, 28.0, bx + 28.0, by, INK, &code);
                if editing && blink() {
                    ui.rounded(bx + 29.0 + tw, by + 7.0, 1.0, 14.0, 0.0, INK);
                }
                self.hit(
                    ui,
                    bx,
                    by,
                    bw,
                    28.0,
                    Action::Setting(f.key, s.key, Control::Hex),
                );
                // The hue strip between the swatches and the code.
                let (hx0, hx1) = (sx + 6.0, bx - 14.0);
                if hx1 - hx0 > 40.0 {
                    let (hy, hh) = (ly + 3.0, 12.0);
                    // About one band per dp, so the strip reads as one smooth gradient.
                    let n = ((hx1 - hx0) as usize).clamp(24, 400);
                    let seg = (hx1 - hx0) / n as f32;
                    let start = ui.mark();
                    for i in 0..n {
                        let c = crate::hud_widgets::hsv(i as f32 / n as f32, 0.75, 1.0);
                        ui.rounded(hx0 + i as f32 * seg, hy, seg + 0.6, hh, 0.0, hex(c, 255));
                    }
                    ui.clip_since(start, hx0, hy, hx1 - hx0, hh);
                    ui.border(hx0, hy, hx1 - hx0, hh, 4.0, hex(0xFFFFFF, 40));
                    let [x0, _, gw, _] = ui.gui_rect(hx0, hy, hx1 - hx0, 1.0);
                    self.hit(
                        ui,
                        hx0,
                        hy - 6.0,
                        hx1 - hx0,
                        hh + 12.0,
                        Action::Setting(f.key, s.key, Control::Hue { x0, x1: x0 + gw }),
                    );
                }
            }
            Kind::Text(_) => {
                let editing = self.text_edit == Some((f.key, s.key));
                let value = e.opt_text(f.key, s.key);
                let (shown, ink) = if editing {
                    (self.text_input.clone(), INK)
                } else if value.is_empty() {
                    (lang::get_or("wisp.menu.text_default", "Default"), FAINT)
                } else {
                    (value, INK)
                };
                let bw = 200.0_f32.min(w * 0.5);
                let (bx, by) = (right - bw, cy - 16.0);
                let fill = if hover(ui, bx, by, bw, 32.0) {
                    CARD_HOVER
                } else {
                    FIELD
                };
                ui.rounded(bx, by, bw, 32.0, 10.0, fill);
                if editing {
                    ui.border(bx, by, bw, 32.0, 10.0, ACCENT_SOFT);
                }
                let text = fit(ui, Face::Medium, 13.0, &shown, bw - 28.0);
                let tw = ui.text_line(Face::Medium, 13.0, 32.0, bx + 12.0, by, ink, &text);
                if editing && blink() {
                    let cx = if shown.is_empty() {
                        bx + 12.0
                    } else {
                        bx + 13.0 + tw
                    };
                    ui.rounded(cx, by + 9.0, 1.0, 14.0, 0.0, INK);
                }
                if editing {
                    let count = format!("{}/{TEXT_MAX}", self.text_input.chars().count());
                    let cw = ui.measure(Face::Mono, 10.0, &count);
                    ui.text_line(Face::Mono, 10.0, 32.0, bx - 10.0 - cw, by, FAINT, &count);
                }
                self.hit(
                    ui,
                    bx,
                    by,
                    bw,
                    32.0,
                    Action::Setting(f.key, s.key, Control::Text),
                );
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
                ui.rounded(tx, cy - 2.0, tw * t, 4.0, 2.0, accent());
                let knob = tx + tw * t;
                ui.circle(knob, cy, 8.0, INK);
                ui.circle(knob, cy, 3.0, accent());
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
            Row::Setting(s) if matches!(s.kind, Kind::Color(_)) => ROW_H + 30.0 + 6.0,
            _ => ROW_H + 6.0,
        }
    }
}

fn hover(ui: &Ui, x: f32, y: f32, w: f32, h: f32) -> bool {
    let (cx, cy) = ui.cursor;
    cx >= x && cx < x + w && cy >= y && cy < y + h
}
/// A switch: a 38x22 pill, the accent with a dark knob when on, grey with a white one when off.
fn draw_switch(ui: &mut Ui, x: f32, y: f32, on: bool) {
    ui.rounded(
        x,
        y,
        38.0,
        22.0,
        11.0,
        if on { accent() } else { SWITCH_OFF },
    );
    let kx = if on { x + 27.0 } else { x + 11.0 };
    ui.circle(kx, y + 11.0, 8.0, if on { on_accent() } else { INK });
}

/// A section's name in the sidebar; returns where its first row starts.
fn side_section(ui: &mut Ui, x: f32, y: f32, name: &str) -> f32 {
    ui.text(
        Face::SemiBold,
        10.0,
        x + 10.0,
        y + 14.0,
        FAINT,
        &name.to_uppercase(),
    );
    y + 34.0
}

/// The small line icons of the sidebar and the head, 16 dp square.
#[derive(Clone, Copy)]
enum Glyph {
    Hud,
    Mods,
    Friends,
    Store,
    Creator,
    Admin,
    Close,
    Back,
}

/// The launcher's line icons, smooth at any scale (`ui_theme::Icon`).
fn draw_glyph(ui: &mut Ui, glyph: Glyph, x: f32, y: f32, ink: u32) {
    use crate::ui_theme::Icon;
    let icon = match glyph {
        Glyph::Hud => Icon::Hud,
        Glyph::Mods => Icon::Mods,
        Glyph::Friends => Icon::Friends,
        Glyph::Store => Icon::Cart,
        Glyph::Creator => Icon::Spark,
        Glyph::Admin => Icon::ShieldCheck,
        Glyph::Close => Icon::Close,
        Glyph::Back => Icon::ChevronLeft,
    };
    ui.icon(icon, x, y, 16.0, ink);
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
