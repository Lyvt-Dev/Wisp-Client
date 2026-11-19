//! The home screen's wardrobe: the player's figure large on the left, and beside it the skin
//! (a library from the skins folder, tried on with a click, worn on the account or, offline,
//! in this game) and the cosmetics (Show my cosmetics, and each owned item worn or taken off).
use super::{
    Click, FAINT, Home, INK, ItemRow, MUTED, PLATE, PLATE_HOVER, STROKE, Tab, WardrobeData, fit,
    hover, record,
};
use crate::{
    lang,
    ui_theme::{
        Face, Icon, Ui,
        color::hex,
        festive::{accent, accent_hover, on_accent},
    },
};

const PANEL: u32 = hex(0x0E0F13, 214);
const TILE: u32 = hex(0xFFFFFF, 12);
const TILE_HOVER: u32 = hex(0xFFFFFF, 24);
const GOOD: u32 = hex(0x4ADE80, 255);
const BAD: u32 = hex(0xFF6B6B, 255);
const SWITCH_OFF: u32 = hex(0xFFFFFF, 40);

impl Home {
    pub(super) fn draw_wardrobe(&mut self, ui: &mut Ui, data: &WardrobeData, pad: f32, rise: f32) {
        let (w, h) = (ui.w, ui.h);
        // The top row: the wordmark's place shows the page, Back at the right.
        let by = pad - 4.0;
        ui.text_line(
            Face::Heading,
            26.0,
            40.0,
            pad,
            by - 1.0,
            INK,
            &lang::get_or("wisp.home.wardrobe", "Wardrobe"),
        );
        let back = lang::get_or("gui.back", "Back");
        let bw = ui.measure(Face::SemiBold, 14.0, &back) + 52.0;
        let bx = w - pad - bw;
        let hovered = hover(ui, bx, by, bw, 40.0);
        ui.rounded(
            bx,
            by,
            bw,
            40.0,
            12.0,
            if hovered { PLATE_HOVER } else { PLATE },
        );
        ui.border(bx, by, bw, 40.0, 12.0, STROKE);
        ui.icon(Icon::ChevronLeft, bx + 12.0, by + 11.0, 18.0, INK);
        ui.text_line(Face::SemiBold, 14.0, 40.0, bx + 34.0, by, INK, &back);
        self.hit(ui, bx, by, bw, 40.0, Click::Back);
        record(ui, &back, (bx, by, bw, 40.0), true);

        // The figure, large, on the left.
        let top = by + 64.0 + rise;
        let bottom = h - pad;
        let left_w = (w * 0.38).clamp(260.0, 520.0);
        let fh = (bottom - top - 20.0).max(160.0);
        let fw = (fh * 0.6).min(left_w);
        let fx = pad + (left_w - fw) / 2.0;
        for i in 0..4 {
            let k = i as f32;
            let rw = fw * (0.62 - k * 0.1);
            let rh = 18.0 - k * 3.5;
            ui.rounded(
                pad + (left_w - rw) / 2.0,
                top + fh - 6.0 - rh / 2.0,
                rw,
                rh,
                rh / 2.0,
                hex(0x000000, 36),
            );
        }
        self.figure = Some(ui.gui_rect(fx, top, fw, fh));
        self.hit(ui, fx, top, fw, fh, Click::Figure);

        // The panel on the right.
        let px = pad + left_w + 24.0;
        let pw = w - pad - px;
        let ph = bottom - top;
        ui.rounded(px, top, pw, ph, 22.0, PANEL);
        ui.border(px, top, pw, ph, 22.0, STROKE);
        self.hit(ui, px, top, pw, ph, Click::Nothing);
        let inner = px + 22.0;
        let iw = pw - 44.0;
        let mut y = top + 20.0;
        self.draw_tabs(ui, inner, y);
        y += 56.0;
        match self.tab {
            Tab::Skin => self.draw_skin_tab(ui, data, inner, y, iw, top + ph - 20.0),
            Tab::Cosmetics => self.draw_cosmetics_tab(ui, data, inner, y, iw, top + ph - 20.0),
        }
    }

    /// Skin | Cosmetics, as the launcher's segmented control.
    fn draw_tabs(&mut self, ui: &mut Ui, x: f32, y: f32) {
        let tabs = [
            (Tab::Skin, lang::get_or("wisp.wardrobe.skin", "Skin")),
            (
                Tab::Cosmetics,
                lang::get_or("wisp.wardrobe.cosmetics", "Cosmetics"),
            ),
        ];
        let widths: Vec<f32> = tabs
            .iter()
            .map(|(_, l)| ui.measure(Face::SemiBold, 14.0, l) + 36.0)
            .collect();
        let total: f32 = widths.iter().sum::<f32>() + 8.0;
        ui.rounded(x, y, total, 42.0, 14.0, hex(0xFFFFFF, 14));
        let mut tx = x + 4.0;
        for ((tab, label), tw) in tabs.iter().zip(widths) {
            let on = self.tab == *tab;
            if on {
                ui.rounded(tx, y + 4.0, tw, 34.0, 11.0, accent());
            } else if hover(ui, tx, y + 4.0, tw, 34.0) {
                ui.rounded(tx, y + 4.0, tw, 34.0, 11.0, hex(0xFFFFFF, 16));
            }
            let lw = ui.measure(Face::SemiBold, 14.0, label);
            ui.text_line(
                Face::SemiBold,
                14.0,
                34.0,
                tx + (tw - lw) / 2.0,
                y + 4.0,
                if on { on_accent() } else { INK },
                label,
            );
            self.hit(ui, tx, y + 4.0, tw, 34.0, Click::Tab(*tab));
            record(ui, label, (tx, y + 4.0, tw, 34.0), true);
            tx += tw;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_skin_tab(
        &mut self,
        ui: &mut Ui,
        data: &WardrobeData,
        x: f32,
        y: f32,
        w: f32,
        bottom: f32,
    ) {
        // The heading and the folder button.
        let head = lang::get_or("wisp.wardrobe.skins", "Your skins");
        ui.text(Face::SemiBold, 16.0, x, y, INK, &head);
        let folder = lang::get_or("wisp.wardrobe.folder", "Skins folder");
        let fw = ui.measure(Face::Medium, 13.0, &folder) + 46.0;
        let (fx, fy) = (x + w - fw, y - 6.0);
        let hovered = hover(ui, fx, fy, fw, 34.0);
        ui.rounded(
            fx,
            fy,
            fw,
            34.0,
            11.0,
            if hovered { TILE_HOVER } else { TILE },
        );
        ui.border(fx, fy, fw, 34.0, 11.0, STROKE);
        ui.icon(Icon::Folder, fx + 12.0, fy + 9.0, 16.0, INK);
        ui.text_line(Face::Medium, 13.0, 34.0, fx + 34.0, fy, INK, &folder);
        self.hit(ui, fx, fy, fw, 34.0, Click::SkinFolder);
        record(ui, &folder, (fx, fy, fw, 34.0), true);

        // The bottom bar: the model, what happened, and the button.
        let bar_y = bottom - 46.0;
        let grid_top = y + 40.0;
        let grid_bottom = bar_y - 16.0;
        if data.skins.is_empty() {
            let none = lang::get_or(
                "wisp.wardrobe.empty",
                "Put skin files (PNG, 64x64) in the skins folder and they show here.",
            );
            ui.text(
                Face::Regular,
                13.0,
                x,
                grid_top + 6.0,
                MUTED,
                &fit(ui, Face::Regular, 13.0, &none, w),
            );
        }
        // The library: each skin's face, tried on with a click.
        let (tw, th, gap) = (96.0, 116.0, 12.0);
        let cols = ((w + gap) / (tw + gap)).floor().max(1.0) as usize;
        let rows = (((grid_bottom - grid_top) + gap) / (th + gap))
            .floor()
            .max(0.0) as usize;
        for (i, skin) in data.skins.iter().enumerate().take(cols * rows) {
            let (c, r) = (i % cols, i / cols);
            let tx = x + c as f32 * (tw + gap);
            let ty = grid_top + r as f32 * (th + gap);
            let on = data.selected == Some(i);
            let hovered = hover(ui, tx, ty, tw, th);
            ui.rounded(
                tx,
                ty,
                tw,
                th,
                14.0,
                if hovered || on { TILE_HOVER } else { TILE },
            );
            if on {
                ui.border(tx, ty, tw, th, 14.0, accent());
                ui.border(tx + 1.0, ty + 1.0, tw - 2.0, th - 2.0, 13.0, accent());
            } else {
                ui.border(tx, ty, tw, th, 14.0, STROKE);
            }
            // The face, 8x8 pixels at 7 dp.
            let px = 7.0;
            let (ox, oy) = (tx + (tw - 8.0 * px) / 2.0, ty + 14.0);
            for (p, colour) in skin.face.iter().enumerate() {
                let (fx, fy) = ((p % 8) as f32, (p / 8) as f32);
                ui.rounded(ox + fx * px, oy + fy * px, px, px, 0.0, *colour);
            }
            let name = fit(ui, Face::Medium, 12.0, &skin.name, tw - 14.0);
            let nw = ui.measure(Face::Medium, 12.0, &name);
            ui.text(
                Face::Medium,
                12.0,
                tx + (tw - nw) / 2.0,
                ty + th - 30.0,
                INK,
                &name,
            );
            self.hit(ui, tx, ty, tw, th, Click::Skin(i));
            record(ui, &skin.name, (tx, ty, tw, th), true);
        }

        // Classic | Slim.
        let models = [
            (false, lang::get_or("wisp.wardrobe.classic", "Classic")),
            (true, lang::get_or("wisp.wardrobe.slim", "Slim")),
        ];
        let mut mx = x;
        ui.rounded(
            mx,
            bar_y,
            8.0 + models
                .iter()
                .map(|(_, l)| ui.measure(Face::Medium, 13.0, l) + 28.0)
                .sum::<f32>(),
            42.0,
            13.0,
            hex(0xFFFFFF, 14),
        );
        mx += 4.0;
        for (slim, label) in &models {
            let mw = ui.measure(Face::Medium, 13.0, label) + 28.0;
            let on = data.slim == *slim;
            if on {
                ui.rounded(mx, bar_y + 4.0, mw, 34.0, 10.0, hex(0xFFFFFF, 40));
            }
            let lw = ui.measure(Face::Medium, 13.0, label);
            ui.text_line(
                Face::Medium,
                13.0,
                34.0,
                mx + (mw - lw) / 2.0,
                bar_y + 4.0,
                if on { INK } else { MUTED },
                label,
            );
            self.hit(ui, mx, bar_y + 4.0, mw, 34.0, Click::Model(*slim));
            record(ui, label, (mx, bar_y + 4.0, mw, 34.0), true);
            mx += mw;
        }

        // Wear it.
        let apply = if data.busy {
            lang::get_or("wisp.wardrobe.saving", "Saving...")
        } else {
            lang::get_or("wisp.wardrobe.apply", "Wear this skin")
        };
        let enabled = data.selected.is_some() && !data.busy;
        let aw = ui.measure(Face::SemiBold, 14.0, &apply) + 56.0;
        let ax = x + w - aw;
        let hovered = enabled && hover(ui, ax, bar_y, aw, 42.0);
        let fill = match (enabled, hovered) {
            (false, _) => hex(0xFFFFFF, 30),
            (true, false) => accent(),
            (true, true) => accent_hover(),
        };
        ui.rounded(ax, bar_y, aw, 42.0, 13.0, fill);
        let ink = if enabled { on_accent() } else { FAINT };
        ui.icon(Icon::Check, ax + 16.0, bar_y + 12.0, 18.0, ink);
        ui.text_line(Face::SemiBold, 14.0, 42.0, ax + 40.0, bar_y, ink, &apply);
        if enabled {
            self.hit(ui, ax, bar_y, aw, 42.0, Click::ApplySkin);
        }
        record(ui, &apply, (ax, bar_y, aw, 42.0), enabled);

        // What happened, or where the skin goes, between them.
        let note_x = mx + 16.0;
        let room = ax - 16.0 - note_x;
        let (note, colour) = match &data.status {
            Some((text, failed)) => (text.clone(), if *failed { BAD } else { GOOD }),
            None if data.signed_in => (
                lang::get_or("wisp.wardrobe.account", "Saved to your Microsoft account"),
                FAINT,
            ),
            None => (
                lang::get_or("wisp.wardrobe.offline", "Offline: worn in this game"),
                FAINT,
            ),
        };
        if room > 40.0 {
            let note = fit(ui, Face::Regular, 12.0, &note, room);
            ui.text_line(Face::Regular, 12.0, 42.0, note_x, bar_y, colour, &note);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_cosmetics_tab(
        &mut self,
        ui: &mut Ui,
        data: &WardrobeData,
        x: f32,
        y: f32,
        w: f32,
        bottom: f32,
    ) {
        if !data.cosmetics {
            let off = lang::get_or(
                "wisp.wardrobe.cosmetics_off",
                "Cosmetics are off in this game.",
            );
            ui.text(Face::Regular, 13.0, x, y, MUTED, &off);
            return;
        }
        // Show my cosmetics.
        let (cy, ch) = (y, 66.0);
        ui.rounded(x, cy, w, ch, 16.0, TILE);
        ui.border(x, cy, w, ch, 16.0, STROKE);
        let title = lang::get_or("wisp.wardrobe.show", "Show my cosmetics");
        let detail = lang::get_or(
            "wisp.wardrobe.show.detail",
            "Every Wisp player sees what you wear, you included.",
        );
        ui.text(Face::SemiBold, 14.0, x + 18.0, cy + 13.0, INK, &title);
        let detail = fit(ui, Face::Regular, 12.0, &detail, w - 110.0);
        ui.text(Face::Regular, 12.0, x + 18.0, cy + 35.0, MUTED, &detail);
        let (sw, sh) = (46.0, 26.0);
        let (sx, sy) = (x + w - 18.0 - sw, cy + (ch - sh) / 2.0);
        let on = data.cosmetics_shown;
        ui.rounded(
            sx,
            sy,
            sw,
            sh,
            sh / 2.0,
            if on { accent() } else { SWITCH_OFF },
        );
        let knob = if on { sx + sw - sh + 3.0 } else { sx + 3.0 };
        ui.circle(
            knob + (sh - 6.0) / 2.0,
            sy + sh / 2.0,
            (sh - 6.0) / 2.0,
            if on { on_accent() } else { INK },
        );
        self.hit(ui, x, cy, w, ch, Click::ShowCosmetics);
        record(ui, &title, (x, cy, w, ch), true);

        // The items.
        let mut ry = cy + ch + 22.0;
        let head = lang::get_or("wisp.wardrobe.items", "Your items");
        ui.text(Face::SemiBold, 16.0, x, ry, INK, &head);
        ry += 34.0;
        if let Some(problem) = &data.items_problem {
            let problem = fit(ui, Face::Regular, 13.0, problem, w);
            ui.text(Face::Regular, 13.0, x, ry, MUTED, &problem);
            return;
        }
        if data.items.is_empty() {
            let none = lang::get_or(
                "wisp.wardrobe.no_items",
                "No items yet: the Store in the Wisp Menu has them.",
            );
            ui.text(Face::Regular, 13.0, x, ry, MUTED, &none);
            return;
        }
        let row_h = 58.0;
        for item in &data.items {
            if ry + row_h > bottom {
                break;
            }
            self.item_row(ui, item, x, ry, w, row_h);
            ry += row_h + 8.0;
        }
    }

    fn item_row(&mut self, ui: &mut Ui, item: &ItemRow, x: f32, y: f32, w: f32, h: f32) {
        ui.rounded(x, y, w, h, 14.0, TILE);
        ui.border(x, y, w, h, 14.0, STROKE);
        let label = if item.worn {
            lang::get_or("wisp.wardrobe.take_off", "Take off")
        } else {
            lang::get_or("wisp.wardrobe.wear", "Wear")
        };
        let bw = ui.measure(Face::SemiBold, 13.0, &label) + 32.0;
        let (bx, by) = (x + w - 12.0 - bw, y + (h - 34.0) / 2.0);
        let room = bx - 16.0 - (x + 18.0);
        let name = fit(ui, Face::SemiBold, 14.0, &item.name, room);
        ui.text(Face::SemiBold, 14.0, x + 18.0, y + 10.0, INK, &name);
        let kind = fit(ui, Face::Regular, 12.0, &item.kind, room);
        ui.text(Face::Regular, 12.0, x + 18.0, y + 31.0, MUTED, &kind);
        let hovered = hover(ui, bx, by, bw, 34.0);
        let (fill, ink) = match (item.worn, hovered) {
            (true, false) => (hex(0xFFFFFF, 26), INK),
            (true, true) => (hex(0xFFFFFF, 40), INK),
            (false, false) => (accent(), on_accent()),
            (false, true) => (accent_hover(), on_accent()),
        };
        ui.rounded(bx, by, bw, 34.0, 11.0, fill);
        let lw = ui.measure(Face::SemiBold, 13.0, &label);
        ui.text_line(
            Face::SemiBold,
            13.0,
            34.0,
            bx + (bw - lw) / 2.0,
            by,
            ink,
            &label,
        );
        self.hit(ui, bx, by, bw, 34.0, Click::Item(item.id.clone()));
        record(
            ui,
            &format!("{label} {}", item.name),
            (bx, by, bw, 34.0),
            true,
        );
    }
}
