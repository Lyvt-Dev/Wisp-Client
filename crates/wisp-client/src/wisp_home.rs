//! Wisp's home screen, drawn in place of vanilla's title menu (`docs/extras.md`): the player's
//! own picture blurred behind (the launcher's backgrounds folder), the wordmark at the top left
//! with round buttons at the right, the main buttons in the middle and the player on the right:
//! their own skin standing under a name tag (the game draws the figure in [`Home::figure`]),
//! over a card with the account, in the launcher's glass look. Every button runs what vanilla's title button of the same name does
//! (`Title::press_main`). With `titleScreen:vanilla` in `wisp.txt` the game shows vanilla's.
use crate::{
    lang,
    ui_theme::{
        Face, Icon, Ui,
        color::hex,
        festive::{self, accent, accent_hover, on_accent},
    },
};

mod wardrobe;

const VEIL: u32 = hex(0x07080B, 120);
const PLATE: u32 = hex(0x101116, 196);
const PLATE_HOVER: u32 = hex(0x1A1C23, 214);
const STROKE: u32 = hex(0xFFFFFF, 22);
const INK: u32 = hex(0xFFFFFF, 255);
const MUTED: u32 = hex(0xFFFFFF, 150);
const FAINT: u32 = hex(0xFFFFFF, 105);
const DANGER: u32 = hex(0xE5484D, 230);
const DANGER_HOVER: u32 = hex(0xF0575C, 255);

/// What a click on the home screen does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Click {
    /// One of vanilla's title buttons, by its id (`Title::press_main`).
    Button(u8),
    /// The player's figure: it waves.
    Figure,
    /// The wardrobe card: the skin and the cosmetics.
    Wardrobe,
    /// The wardrobe's Back: the home screen again.
    Back,
    /// A wardrobe tab.
    Tab(Tab),
    /// A skin of the wardrobe's library, by its index: tried on.
    Skin(usize),
    /// The model the skin is worn with: slim (`true`) or classic.
    Model(bool),
    /// Wear the skin tried on.
    ApplySkin,
    /// Open the skins folder.
    SkinFolder,
    /// Show my cosmetics, on or off.
    ShowCosmetics,
    /// Wear or take off an item, by its id.
    Item(String),
    /// Inside a panel, on nothing.
    Nothing,
}

/// Which page the home screen shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Main,
    Wardrobe,
}

/// The wardrobe's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Skin,
    Cosmetics,
}

/// A skin in the wardrobe's library.
#[derive(Clone, Debug, PartialEq)]
pub struct SkinChoice {
    pub name: String,
    /// The face as it shows (8x8, the hat over it), in the HUD's colour encoding.
    pub face: [u32; 64],
}

/// An item the player owns, as the wardrobe lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemRow {
    pub id: String,
    pub name: String,
    /// What kind of item it is ("Cape", "Hat"…).
    pub kind: String,
    pub worn: bool,
}

/// What the wardrobe shows.
#[derive(Clone, Debug, Default)]
pub struct WardrobeData {
    pub skins: Vec<SkinChoice>,
    /// The skin tried on.
    pub selected: Option<usize>,
    pub slim: bool,
    pub signed_in: bool,
    /// What the last action did, and whether it failed.
    pub status: Option<(String, bool)>,
    /// A skin is on its way to the account.
    pub busy: bool,
    /// Cosmetics are on in this game (else the tab says so).
    pub cosmetics: bool,
    pub cosmetics_shown: bool,
    pub items: Vec<ItemRow>,
    /// Why no items show (no account, no server…).
    pub items_problem: Option<String>,
}

/// The title buttons' ids, as `Title::buttons` numbers them.
pub mod button {
    pub const SINGLEPLAYER: u8 = 1;
    pub const QUIT: u8 = 2;
    pub const OPTIONS: u8 = 3;
    pub const MULTIPLAYER: u8 = 4;
    pub const FRIENDS: u8 = 5;
    pub const LANGUAGE: u8 = 6;
    pub const ACCESSIBILITY: u8 = 7;
}

/// What the screen shows besides its buttons.
#[derive(Clone, Debug, Default)]
pub struct Data {
    pub player: String,
    /// Signed in with a Microsoft account (else an offline name).
    pub signed_in: bool,
    /// Multiplayer is allowed (`Title::allows_multiplayer`).
    pub multiplayer: bool,
    /// The bottom line: Wisp's and the game's versions.
    pub version: String,
    /// The wardrobe page's contents (only read while it shows).
    pub wardrobe: WardrobeData,
}

#[derive(Default)]
pub struct Home {
    hits: Vec<([f32; 4], Click)>,
    /// When the screen first showed, for its fade in.
    opened: Option<std::time::Instant>,
    /// Where the player's figure stands this frame, in GUI pixels (none on a narrow window):
    /// the game draws the model and its name tag there.
    pub figure: Option<[f32; 4]>,
    pub page: Page,
    pub tab: Tab,
}

impl Home {
    /// The screen shows again: it fades in once more.
    pub fn reset(&mut self) {
        self.opened = None;
    }

    /// What a click at `cursor` (GUI pixels) does: the last drawn frame's topmost target.
    pub fn click(&self, cursor: (f32, f32)) -> Option<Click> {
        self.hits.iter().rev().find_map(|([x, y, w, h], c)| {
            (cursor.0 >= *x && cursor.0 < x + w && cursor.1 >= *y && cursor.1 < y + h)
                .then(|| c.clone())
        })
    }

    fn hit(&mut self, ui: &Ui, x: f32, y: f32, w: f32, h: f32, click: Click) {
        self.hits.push((ui.gui_rect(x, y, w, h), click));
    }

    pub fn draw(&mut self, ui: &mut Ui, data: &Data) {
        self.hits.clear();
        self.figure = None;
        let t = self
            .opened
            .get_or_insert_with(std::time::Instant::now)
            .elapsed()
            .as_secs_f32();
        let ease = 1.0 - (1.0 - (t / 0.35).min(1.0)).powi(3);
        let rise = (1.0 - ease) * 10.0;
        let (w, h) = (ui.w, ui.h);
        ui.rounded(0.0, 0.0, w, h, 0.0, VEIL);
        // A darker band at the bottom, where the version line sits.
        for i in 0..6 {
            let a = 10 + i * 9;
            let y = h - 120.0 + i as f32 * 20.0;
            ui.rounded(0.0, y, w, 20.0, 0.0, hex(0x000000, a as u8));
        }
        let pad = (w * 0.035).clamp(20.0, 48.0);
        // Wisp Christmas: snow behind everything, fairy lights along the top.
        if festive::on() {
            festive::snow(ui, (0.0, 0.0, w, h), ease);
            festive::lights(ui, 0.0, 2.0, w);
        }
        if self.page == Page::Wardrobe {
            self.draw_wardrobe(ui, &data.wardrobe, pad, rise);
        } else {
            self.draw_top(ui, pad);
            let side = (w * 0.24).clamp(220.0, 320.0);
            let narrow = w < 820.0;
            self.draw_centre(ui, data, w / 2.0, h * 0.5 + rise, narrow);
            if !narrow {
                self.draw_player(ui, w - pad - side, h * 0.5 + rise, side);
            }
        }
        // The version, at the bottom left; vanilla's notice, at the bottom right.
        let y = h - pad - 14.0;
        ui.text(Face::Medium, 12.0, pad, y, FAINT, &data.version);
        let notice = lang::get_or(
            "wisp.home.notice",
            "Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.",
        );
        let nw = ui.measure(Face::Regular, 11.0, &notice);
        if nw < w - pad * 2.0 - 260.0 {
            ui.text(Face::Regular, 11.0, w - pad - nw, y + 1.0, FAINT, &notice);
        }
    }

    /// The wordmark at the left, the round buttons at the right.
    fn draw_top(&mut self, ui: &mut Ui, pad: f32) {
        let (bx, by, side) = (pad, pad - 4.0, 40.0);
        let tile = hex(0x000000, 255);
        ui.rounded(bx, by, side, side, 12.0, tile);
        ui.border(bx, by, side, side, 12.0, STROKE);
        ui.feather(bx + side / 2.0, by + side / 2.0, side, INK, tile);
        if festive::on() {
            festive::hat(ui, bx, by, side);
        }
        ui.text_line(Face::Heading, 26.0, side, bx + 54.0, by - 1.0, INK, "WISP");
        let buttons = [
            (Icon::Language, button::LANGUAGE, false),
            (Icon::Accessibility, button::ACCESSIBILITY, false),
            (Icon::Friends, button::FRIENDS, false),
            (Icon::Settings, button::OPTIONS, false),
            (Icon::Close, button::QUIT, true),
        ];
        let size = 40.0;
        let mut x = ui.w - pad - size;
        for (icon, id, danger) in buttons.into_iter().rev() {
            let hovered = hover(ui, x, by, size, size);
            let fill = match (danger, hovered) {
                (true, false) => DANGER,
                (true, true) => DANGER_HOVER,
                (false, false) => PLATE,
                (false, true) => PLATE_HOVER,
            };
            ui.rounded(x, by, size, size, 12.0, fill);
            if !danger {
                ui.border(x, by, size, size, 12.0, STROKE);
            }
            draw_icon(ui, icon, x + 12.0, by + 12.0, INK);
            self.hit(ui, x, by, size, size, Click::Button(id));
            record(ui, &vanilla_label(id), (x, by, size, size), true);
            x -= size + 10.0;
        }
    }

    /// The main buttons, around the middle of the screen.
    fn draw_centre(&mut self, ui: &mut Ui, data: &Data, cx: f32, cy: f32, narrow: bool) {
        let bw = if narrow { 260.0 } else { 300.0 };
        let x = (cx - bw / 2.0).round();
        let mut y = (cy - 118.0).round();
        // The big wordmark over them.
        let mark = "Wisp";
        let mw = ui.measure(Face::Heading, 64.0, mark);
        ui.text(Face::Heading, 64.0, cx - mw / 2.0, y - 104.0, INK, mark);
        let tag = if festive::on() {
            lang::get_or("wisp.home.tagline.christmas", "Merry Christmas from Wisp")
        } else {
            lang::get_or("wisp.home.tagline", "Client for Minecraft")
        };
        let tw = ui.measure(Face::Regular, 14.0, &tag);
        ui.text(Face::Regular, 14.0, cx - tw / 2.0, y - 16.0, MUTED, &tag);
        y += 14.0;
        let single = lang::get_or("menu.singleplayer", "Singleplayer");
        self.big_button(
            ui,
            (x, y, bw, 52.0),
            &single,
            Icon::Play,
            true,
            true,
            button::SINGLEPLAYER,
        );
        y += 62.0;
        let multi = lang::get_or("menu.multiplayer", "Multiplayer");
        self.big_button(
            ui,
            (x, y, bw, 52.0),
            &multi,
            Icon::Globe,
            false,
            data.multiplayer,
            button::MULTIPLAYER,
        );
        y += 62.0;
        let half = (bw - 10.0) / 2.0;
        let options = lang::get_or("wisp.home.settings", "Settings");
        self.big_button(
            ui,
            (x, y, half, 46.0),
            &options,
            Icon::Settings,
            false,
            true,
            button::OPTIONS,
        );
        let friends = lang::get_or("gui.friends.open", "Friends");
        self.big_button(
            ui,
            (x + half + 10.0, y, half, 46.0),
            &friends,
            Icon::Friends,
            false,
            true,
            button::FRIENDS,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn big_button(
        &mut self,
        ui: &mut Ui,
        (x, y, w, h): (f32, f32, f32, f32),
        label: &str,
        icon: Icon,
        primary: bool,
        enabled: bool,
        id: u8,
    ) {
        let hovered = enabled && hover(ui, x, y, w, h);
        let (fill, ink) = match (primary, hovered) {
            (true, false) => (accent(), on_accent()),
            (true, true) => (accent_hover(), on_accent()),
            (false, false) => (PLATE, INK),
            (false, true) => (PLATE_HOVER, INK),
        };
        let ink = if enabled { ink } else { FAINT };
        ui.rounded(x, y, w, h, 14.0, fill);
        if !primary {
            ui.border(x, y, w, h, 14.0, STROKE);
        }
        let face = Face::SemiBold;
        let size = if h > 48.0 { 16.0 } else { 14.0 };
        let lw = ui.measure(face, size, label);
        let total = lw + 26.0;
        let lx = x + (w - total) / 2.0;
        draw_icon(ui, icon, lx, y + (h - 16.0) / 2.0, ink);
        ui.text_line(face, size, h, lx + 26.0, y, ink, label);
        if enabled {
            self.hit(ui, x, y, w, h, Click::Button(id));
        }
        // The top row's icons stand for Options and Friends under vanilla's labels.
        if !matches!(id, button::OPTIONS | button::FRIENDS) {
            record(ui, label, (x, y, w, h), enabled);
        }
    }

    /// The player on the right: room for their figure (drawn by the game), and under it the
    /// wardrobe's card.
    fn draw_player(&mut self, ui: &mut Ui, x: f32, cy: f32, w: f32) {
        // The figure: 2.1 blocks of room (the model and its name tag) over the card.
        let fh = (ui.h * 0.5).clamp(240.0, 380.0);
        let card_h = 64.0;
        let top = cy - (fh + card_h) / 2.0 + 10.0;
        let fw = fh * 0.6;
        let fx = x + (w - fw) / 2.0;
        // A soft shadow the figure stands on.
        let foot = top + fh - 6.0;
        for i in 0..4 {
            let k = i as f32;
            let rw = fw * (0.62 - k * 0.1);
            let rh = 16.0 - k * 3.0;
            ui.rounded(
                x + (w - rw) / 2.0,
                foot - rh / 2.0,
                rw,
                rh,
                rh / 2.0,
                hex(0x000000, 34),
            );
        }
        self.figure = Some(ui.gui_rect(fx, top, fw, fh));
        self.hit(ui, fx, top, fw, fh, Click::Figure);
        // The wardrobe: the skin and the cosmetics, a click away.
        let y = top + fh + 10.0;
        let hovered = hover(ui, x, y, w, card_h);
        ui.rounded(
            x,
            y,
            w,
            card_h,
            18.0,
            if hovered { PLATE_HOVER } else { PLATE },
        );
        ui.border(x, y, w, card_h, 18.0, STROKE);
        let (ix, iy, side) = (x + 12.0, y + 12.0, 40.0);
        ui.rounded(ix, iy, side, side, 12.0, accent());
        ui.icon(Icon::Hanger, ix + 9.0, iy + 9.0, 22.0, on_accent());
        let title = lang::get_or("wisp.home.wardrobe", "Wardrobe");
        let detail = lang::get_or("wisp.home.wardrobe.detail", "Skin and cosmetics");
        let tx = ix + side + 14.0;
        let room = x + w - 40.0 - tx;
        let title = fit(ui, Face::SemiBold, 15.0, &title, room);
        ui.text(Face::SemiBold, 15.0, tx, y + 12.0, INK, &title);
        let detail = fit(ui, Face::Regular, 12.0, &detail, room);
        ui.text(Face::Regular, 12.0, tx, y + 33.0, MUTED, &detail);
        let arrow = if hovered { INK } else { MUTED };
        ui.icon(Icon::ChevronRight, x + w - 34.0, y + 22.0, 20.0, arrow);
        self.hit(ui, x, y, w, card_h, Click::Wardrobe);
        record(ui, &title, (x, y, w, card_h), true);
    }
}

/// A button for the narrator and agent control (`wisp-ctl ui` and `click`), as vanilla's
/// buttons record themselves, in GUI pixels.
fn record(ui: &Ui, label: &str, (x, y, w, h): (f32, f32, f32, f32), enabled: bool) {
    crate::widgets::record(label, ui.gui_rect(x, y, w, h), enabled);
}

/// The label of vanilla's title button with this id, which the top row's icons record.
fn vanilla_label(id: u8) -> String {
    match id {
        button::LANGUAGE => lang::get_or("options.language", "Language..."),
        button::ACCESSIBILITY => lang::get_or("options.accessibility", "Accessibility Settings..."),
        button::FRIENDS => lang::get_or("gui.friends.open", "Friends"),
        button::OPTIONS => lang::get_or("menu.options", "Options..."),
        _ => lang::get_or("menu.quit", "Quit Game"),
    }
}

fn hover(ui: &Ui, x: f32, y: f32, w: f32, h: f32) -> bool {
    let (cx, cy) = ui.cursor;
    cx >= x && cx < x + w && cy >= y && cy < y + h
}

/// The text cut with an ellipsis to fit `max` dp.
fn fit(ui: &Ui, face: Face, size: f32, text: &str, max: f32) -> String {
    if ui.measure(face, size, text) <= max {
        return text.to_owned();
    }
    let mut s = text.to_owned();
    while !s.is_empty() && ui.measure(face, size, &format!("{s}\u{2026}")) > max {
        s.pop();
    }
    format!("{}\u{2026}", s.trim_end())
}

/// A line icon, 16 dp square.
fn draw_icon(ui: &mut Ui, icon: Icon, x: f32, y: f32, ink: u32) {
    ui.icon(icon, x, y, 16.0, ink);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_finds_the_topmost_target() {
        let mut home = Home::default();
        home.hits.push(([0.0, 0.0, 100.0, 100.0], Click::Nothing));
        home.hits
            .push(([10.0, 10.0, 20.0, 20.0], Click::Button(button::QUIT)));
        assert_eq!(home.click((15.0, 15.0)), Some(Click::Button(button::QUIT)));
        assert_eq!(home.click((50.0, 50.0)), Some(Click::Nothing));
        assert_eq!(home.click((150.0, 50.0)), None);
    }
}
