//! The HUD editor (`docs/extras.md`): every HUD widget that is on, outlined over the world. Drag
//! one to move it (it snaps to the screen's edges and centre and to the other widgets' edges),
//! scroll over it to resize it, right click it for its settings in the Wisp Menu. Places are
//! fractions of the screen (`opt.<widget>.pos`), sizes the widget's `scale` setting.
use super::{Game, Screen};
use wisp_client::{
    extras::features::FEATURES,
    hud::HudBatch,
    lang,
    ui_theme::{Face, Ui, color::hex},
};

/// How near (GUI pixels) an edge has to come to snap to a line.
const SNAP: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Button {
    Reset,
    Settings,
    Done,
}

impl Game<'_> {
    /// Opens the editor over the world (the Wisp Menu's Edit HUD, or the Extras page's).
    pub(super) fn open_hud_editor(&mut self) {
        if self.title.is_some() || self.player.is_none() {
            return;
        }
        self.settings.save(&Self::options_path());
        self.options_to_game = false;
        self.screen = Screen::HudEditor;
        self.extras.hud_drag = None;
        self.capture(false);
    }
    /// Done or Escape: saves; back to the menu when it opened the editor, else to the game.
    pub(super) fn close_hud_editor(&mut self) {
        self.extras.hud_drag = None;
        self.settings.save(&Self::options_path());
        self.apply_settings();
        if std::mem::take(&mut self.extras.editor_from_menu) {
            self.screen = Screen::WispMenu;
        } else {
            self.screen = Screen::Playing;
            self.capture(true);
        }
    }
    pub(super) fn draw_hud_editor(&mut self, batch: &mut HudBatch) {
        let cursor = self.cursor;
        let rects = self.extras.widget_rects.borrow().clone();
        let dragging = self.extras.hud_drag.map(|(k, ..)| k);
        let hovered = dragging.or_else(|| self.widget_at(cursor.0, cursor.1).map(|(k, _)| k));
        let mut hits = Vec::new();
        {
            let atlas = &self.gui.atlas;
            let mut ui = Ui::new(atlas, batch, cursor);
            let per = 1.0 / ui.gui_per_dp();
            let (w, h) = (ui.w, ui.h);
            // The snapping lines while a widget moves: the screen's centre.
            if dragging.is_some() {
                ui.rounded(w / 2.0 - 0.5, 0.0, 1.0, h, 0.0, hex(0x8B6BFF, 70));
                ui.rounded(0.0, h / 2.0 - 0.5, w, 1.0, 0.0, hex(0x8B6BFF, 70));
            }
            for (key, [gx, gy, gw, gh]) in &rects {
                let (x, y, rw, rh) = (gx * per, gy * per, gw * per, gh * per);
                let hot = hovered == Some(*key);
                if hot {
                    ui.rounded(
                        x - 3.0,
                        y - 3.0,
                        rw + 6.0,
                        rh + 6.0,
                        10.0,
                        hex(0xFFFFFF, 28),
                    );
                }
                let ring = if hot {
                    hex(0xFFFFFF, 255)
                } else {
                    hex(0xFFFFFF, 90)
                };
                ui.border(x - 3.0, y - 3.0, rw + 6.0, rh + 6.0, 10.0, ring);
                if hot {
                    let name = Self::widget_name(key);
                    let scale = self.settings.extras.opt_f32(key, "scale");
                    let tag = format!("{name}  \u{b7}  {scale:.2}x");
                    let tw = ui.measure(Face::Medium, 12.0, &tag) + 16.0;
                    let ty = if y > 30.0 { y - 28.0 } else { y + rh + 8.0 };
                    let tx = x.min(w - tw - 4.0).max(4.0);
                    ui.rounded(tx, ty, tw, 22.0, 8.0, hex(0x2A2A30, 245));
                    ui.text_line(
                        Face::Medium,
                        12.0,
                        22.0,
                        tx + 8.0,
                        ty,
                        hex(0xFFFFFF, 255),
                        &tag,
                    );
                }
            }
            // The toolbar: a glass plate at the top centre.
            let title = lang::get_or("wisp.hud_editor.title", "HUD Editor");
            let hint = lang::get_or(
                "wisp.hud_editor.hint",
                "Drag to move, scroll to resize, right click for settings",
            );
            let labels = [
                (
                    Button::Reset,
                    lang::get_or("wisp.hud_editor.reset", "Reset layout"),
                ),
                (
                    Button::Settings,
                    lang::get_or("wisp.hud_editor.settings", "Settings"),
                ),
                (Button::Done, lang::get_or("gui.done", "Done")),
            ];
            let widths: Vec<f32> = labels
                .iter()
                .map(|(_, l)| ui.measure(Face::Medium, 13.0, l) + 28.0)
                .collect();
            let text_w =
                ui.measure(Face::Heading, 17.0, &title)
                    .max(ui.measure(Face::Regular, 12.0, &hint));
            let bar_w = 20.0 + text_w + 20.0 + widths.iter().sum::<f32>() + 8.0 * 2.0 + 14.0;
            let (bx, by, bh) = (((w - bar_w) / 2.0).round(), 14.0, 56.0);
            ui.rounded(bx, by, bar_w, bh, 16.0, hex(0x15112E, 242));
            ui.border(bx, by, bar_w, bh, 16.0, hex(0xFFFFFF, 22));
            ui.text(
                Face::Heading,
                17.0,
                bx + 20.0,
                by + 7.0,
                hex(0xFFFFFF, 255),
                &title,
            );
            ui.text(
                Face::Regular,
                12.0,
                bx + 20.0,
                by + 30.0,
                hex(0xFFFFFF, 150),
                &hint,
            );
            let mut x = bx + 20.0 + text_w + 20.0;
            for ((button, label), bw) in labels.iter().zip(&widths) {
                let (yy, hh) = (by + 12.0, 32.0);
                let over = cursor_in(&ui, x, yy, *bw, hh);
                let fill = match (button, over) {
                    (Button::Done, false) => hex(0xF2F2F2, 255),
                    (Button::Done, true) => hex(0xFFFFFF, 255),
                    (_, false) => hex(0xFFFFFF, 20),
                    (_, true) => hex(0xFFFFFF, 36),
                };
                ui.rounded(x, yy, *bw, hh, 10.0, fill);
                ui.text_line(
                    Face::Medium,
                    13.0,
                    hh,
                    x + 14.0,
                    yy,
                    if *button == Button::Done {
                        hex(0x141414, 255)
                    } else {
                        hex(0xFFFFFF, 255)
                    },
                    label,
                );
                hits.push((ui.gui_rect(x, yy, *bw, hh), *button));
                x += bw + 8.0;
            }
            if rects.is_empty() {
                let none = lang::get_or(
                    "wisp.hud_editor.empty",
                    "No HUD widget is on. Turn some on in the Wisp Menu's HUD tab.",
                );
                let nw = ui.measure(Face::Regular, 14.0, &none);
                ui.text(
                    Face::Regular,
                    14.0,
                    (w - nw) / 2.0,
                    h / 2.0 - 10.0,
                    hex(0xFFFFFF, 220),
                    &none,
                );
            }
            ui.finish();
        }
        *self.extras.editor_hits.borrow_mut() = hits;
    }
    fn editor_button_at(&self, x: f32, y: f32) -> Option<Button> {
        self.extras
            .editor_hits
            .borrow()
            .iter()
            .find(|([bx, by, bw, bh], _)| x >= *bx && x < bx + bw && y >= *by && y < by + bh)
            .map(|(_, b)| *b)
    }
    /// A press: a toolbar button, picking up a widget, or (right) its settings.
    pub(super) fn hud_editor_press(&mut self, right: bool) {
        let (cx, cy) = self.cursor;
        if !right && let Some(button) = self.editor_button_at(cx, cy) {
            self.audio.play_button_click();
            match button {
                Button::Reset => {
                    for f in FEATURES.iter().filter(|f| f.widget) {
                        self.settings.extras.set_widget_pos(f.key, None);
                    }
                    self.settings.save(&Self::options_path());
                }
                Button::Settings => {
                    self.extras.editor_from_menu = true;
                    self.extras.menu.open = None;
                    self.extras.menu.category = Some(wisp_client::extras::features::Category::Hud);
                    self.close_hud_editor();
                }
                Button::Done => self.close_hud_editor(),
            }
            return;
        }
        let Some((key, r)) = self.widget_at(cx, cy) else {
            return;
        };
        if right {
            self.audio.play_button_click();
            self.extras.editor_from_menu = true;
            self.extras.menu.open = Some(key);
            self.extras.menu.scroll = 0.0;
            self.close_hud_editor();
        } else {
            self.extras.hud_drag = Some((key, cx - r[0], cy - r[1]));
        }
    }
    /// The cursor moved with a widget held: it follows, snapping to the screen's edges and
    /// centre and to the other widgets' edges.
    pub(super) fn hud_editor_drag(&mut self) {
        let Some((key, dx, dy)) = self.extras.hud_drag else {
            return;
        };
        let (sw, sh) = self.gui_size();
        if sw <= 0.0 || sh <= 0.0 {
            return;
        }
        let rects = self.extras.widget_rects.borrow().clone();
        let Some(&(_, [_, _, w, h])) = rects.iter().find(|(k, _)| *k == key) else {
            return;
        };
        let others: Vec<[f32; 4]> = rects
            .iter()
            .filter(|(k, _)| *k != key)
            .map(|(_, r)| *r)
            .collect();
        let (cx, cy) = self.cursor;
        let x = snap_axis(cx - dx, w, sw, others.iter().map(|r| (r[0], r[2])));
        let y = snap_axis(cy - dy, h, sh, others.iter().map(|r| (r[1], r[3])));
        // Kept by the edge it sits by, so it grows away from that edge (`anchor_place`).
        let at = (
            wisp_client::hud_widgets::anchor_of(x, w, sw),
            wisp_client::hud_widgets::anchor_of(y, h, sh),
        );
        self.settings.extras.set_widget_anchor(key, at);
    }
    pub(super) fn hud_editor_release(&mut self) {
        if self.extras.hud_drag.take().is_some() {
            self.settings.save(&Self::options_path());
        }
    }
    /// The wheel over a widget resizes it a step.
    pub(super) fn hud_editor_scroll(&mut self, delta: f32) {
        let Some((key, _)) = self.widget_at(self.cursor.0, self.cursor.1) else {
            return;
        };
        let e = &mut self.settings.extras;
        let s = e.opt_f32(key, "scale") + if delta > 0.0 { 0.05 } else { -0.05 };
        e.set_opt(key, "scale", format!("{:.2}", s.clamp(0.5, 2.0)));
        self.settings.save(&Self::options_path());
    }
}

fn cursor_in(ui: &Ui, x: f32, y: f32, w: f32, h: f32) -> bool {
    let (cx, cy) = ui.cursor;
    cx >= x && cx < x + w && cy >= y && cy < y + h
}

/// One axis of a dragged widget's place (`start` its edge, `size` its length, `screen` the
/// screen's): kept on the screen, snapped to the edges, the centre and the other widgets' edges.
fn snap_axis(start: f32, size: f32, screen: f32, others: impl Iterator<Item = (f32, f32)>) -> f32 {
    let start = start.clamp(0.0, (screen - size).max(0.0));
    let mut lines = vec![0.0, screen / 2.0, screen];
    for (o, len) in others {
        lines.extend([o, o + len]);
    }
    let mut best = start;
    let mut best_d = SNAP;
    for line in lines {
        // The widget's near edge, centre or far edge on the line.
        for candidate in [line, line - size / 2.0, line - size] {
            let d = (candidate - start).abs();
            if d < best_d {
                best_d = d;
                best = candidate;
            }
        }
    }
    best.clamp(0.0, (screen - size).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::snap_axis;

    #[test]
    fn a_dragged_widget_snaps_to_edges_centres_and_neighbours() {
        // Near the left edge: on it.
        assert_eq!(snap_axis(3.0, 40.0, 400.0, std::iter::empty()), 0.0);
        // Its centre near the screen's centre.
        assert_eq!(snap_axis(178.0, 40.0, 400.0, std::iter::empty()), 180.0);
        // Its far edge near the right edge, and never off the screen.
        assert_eq!(snap_axis(357.0, 40.0, 400.0, std::iter::empty()), 360.0);
        assert_eq!(snap_axis(500.0, 40.0, 400.0, std::iter::empty()), 360.0);
        // Its edge on a neighbour's far edge.
        assert_eq!(
            snap_axis(102.0, 40.0, 400.0, [(50.0, 50.0)].into_iter()),
            100.0
        );
        // Far from every line: where it was put.
        assert_eq!(snap_axis(250.0, 40.0, 400.0, std::iter::empty()), 250.0);
    }
}
