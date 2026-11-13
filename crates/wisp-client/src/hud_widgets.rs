//! The HUD widgets of the Wisp Menu (`docs/extras.md`): small glass plates in the launcher's
//! look, one per widget, drawn with the bundled fonts. The game (`game/hud_widgets.rs`) gathers
//! what each shows into a [`Content`], places it and draws the items and effect icons this
//! returns on top. One function measures and draws, so the editor's outlines fit what is drawn.
use crate::ui_theme::{Face, Ui, color::hex};

/// A widget's look (`features::WIDGET_SETTINGS`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub background: bool,
    /// 0 to 1.
    pub opacity: f32,
    /// `0xRRGGBB`.
    pub text: u32,
    pub accent: u32,
    pub shadow: bool,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            background: true,
            opacity: 0.7,
            text: 0xFFFFFF,
            accent: 0x8B6BFF,
            shadow: true,
        }
    }
}

/// An armor piece or held item: its index in the game's list, and its durability left and most.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub item: usize,
    pub durability: Option<(u32, u32)>,
}
/// How durability shows (`features`' `DURABILITY`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Durability {
    Bar,
    Number,
    Percent,
    BarAndNumber,
}
impl Durability {
    pub fn from_index(i: usize) -> Self {
        [Self::Bar, Self::Number, Self::Percent, Self::BarAndNumber][i.min(3)]
    }
    fn bar(self) -> bool {
        matches!(self, Self::Bar | Self::BarAndNumber)
    }
    fn text(self, left: u32, most: u32) -> Option<String> {
        match self {
            Self::Bar => None,
            Self::Number | Self::BarAndNumber => Some(left.to_string()),
            Self::Percent => Some(format!("{}%", left * 100 / most.max(1))),
        }
    }
}
/// A status effect: its icon sprite, name with level, and time left.
#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    pub sprite: String,
    pub name: String,
    pub time: String,
    /// Under 10 seconds left: the time turns red.
    pub ending: bool,
}
/// The keys Keystrokes shows: how pressed each looks, 1 held, fading to 0 after it is let go.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Keys {
    pub w: f32,
    pub a: f32,
    pub s: f32,
    pub d: f32,
    pub space: f32,
    pub lmb: f32,
    pub rmb: f32,
    /// Left and right clicks per second, when shown.
    pub cps: Option<(u32, u32)>,
    pub mouse: bool,
    pub space_bar: bool,
}

/// What a widget shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    /// A label in the accent and a value: "FPS 144".
    Value {
        label: String,
        value: String,
    },
    /// Label and value rows, stacked.
    Rows(Vec<(String, String)>),
    /// Latency with signal bars (`None`: single player).
    Ping {
        label: String,
        ms: Option<i32>,
        bars: bool,
    },
    Armor {
        pieces: Vec<Piece>,
        horizontal: bool,
        durability: Durability,
    },
    Effects {
        effects: Vec<Effect>,
        horizontal: bool,
        names: bool,
    },
    Keys(Keys),
    /// An item and a count (arrows).
    Count {
        item: usize,
        count: String,
    },
}

/// An item or a sprite to draw over the widget, in GUI pixels.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    Item {
        item: usize,
        x: f32,
        y: f32,
        size: f32,
    },
    Sprite {
        name: String,
        x: f32,
        y: f32,
        size: f32,
    },
}

const PAD_X: f32 = 8.0;
const PAD_Y: f32 = 5.0;
const RADIUS: f32 = 8.0;
const LABEL: f32 = 11.0;
const VALUE: f32 = 13.0;
const LINE: f32 = 18.0;
const ICON: f32 = 16.0;

/// The colour of `fraction` of durability left: green, yellow, orange, red.
pub fn durability_color(fraction: f32) -> u32 {
    match fraction {
        f if f > 0.6 => 0x4ADE80,
        f if f > 0.3 => 0xFFD43B,
        f if f > 0.15 => 0xFFA94D,
        _ => 0xFF6B6B,
    }
}
/// Where a widget of length `size` starts on an axis of length `screen` for its anchor `f` (a
/// fraction of the screen): in the first third `f` is its near edge, in the middle third its
/// centre, in the last third its far edge. So a widget by the right edge grows to the left when
/// its text gets longer, and one in the middle grows both ways, instead of everything growing
/// to the right and off the screen.
pub fn anchor_place(f: f32, size: f32, screen: f32) -> f32 {
    let at = f * screen;
    let start = if f < 1.0 / 3.0 {
        at
    } else if f > 2.0 / 3.0 {
        at - size
    } else {
        at - size / 2.0
    };
    start.clamp(0.0, (screen - size).max(0.0))
}
/// The anchor (`anchor_place`) of a widget starting at `start`: by the third its centre is in.
pub fn anchor_of(start: f32, size: f32, screen: f32) -> f32 {
    if screen <= 0.0 {
        return 0.0;
    }
    let centre = (start + size / 2.0) / screen;
    let edge = if centre < 1.0 / 3.0 {
        start
    } else if centre > 2.0 / 3.0 {
        start + size
    } else {
        start + size / 2.0
    };
    (edge / screen).clamp(0.0, 1.0)
}

/// The colour of a latency: green, yellow, orange, red.
pub fn ping_color(ms: i32) -> u32 {
    match ms {
        ..80 => 0x4ADE80,
        80..150 => 0xFFD43B,
        150..300 => 0xFFA94D,
        _ => 0xFF6B6B,
    }
}

/// Measures (`paint` false) or draws a widget with its top left at `(x, y)` dp: returns its size
/// in dp and, when painting, the items and sprites to draw on top. The plate is at least `min_w`
/// wide, its content centred in it, so a number that briefly gets shorter doesn't shake it.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    ui: &mut Ui,
    x: f32,
    y: f32,
    content: &Content,
    style: &Style,
    paint: bool,
    min_w: f32,
    out: &mut Vec<Draw>,
) -> (f32, f32) {
    let mut p = Painter {
        ui,
        style,
        paint,
        out,
    };
    let (own_w, h) = p.size(content);
    let w = if matches!(content, Content::Keys(_)) {
        own_w
    } else {
        own_w.max(min_w)
    };
    if paint {
        if style.background {
            let a = (style.opacity.clamp(0.0, 1.0) * 255.0) as u8;
            p.ui.rounded(x, y, w, h, RADIUS, hex(0x15112E, a));
            // Glass: a lighter rim, brightest along the top.
            p.ui.border(x, y, w, h, RADIUS, hex(0xFFFFFF, (a / 9).max(4)));
            p.ui.rounded(
                x + RADIUS,
                y + 0.5,
                w - 2.0 * RADIUS,
                1.0,
                0.5,
                hex(0xFFFFFF, (a / 6).max(6)),
            );
        }
        p.draw(x + ((w - own_w) / 2.0).floor(), y, content, own_w);
    }
    (w, h)
}

struct Painter<'u, 'a, 'o> {
    ui: &'u mut Ui<'a>,
    style: &'u Style,
    paint: bool,
    out: &'o mut Vec<Draw>,
}
impl Painter<'_, '_, '_> {
    fn text_c(&self) -> u32 {
        hex(self.style.text, 255)
    }
    fn accent_c(&self) -> u32 {
        hex(self.style.accent, 255)
    }
    fn muted_c(&self) -> u32 {
        hex(self.style.text, 170)
    }
    /// Text in a `LINE`-tall box at `y`, with the shadow when it is on; returns its width.
    fn text(&mut self, face: Face, size: f32, x: f32, y: f32, color: u32, s: &str) -> f32 {
        if !self.paint {
            return self.ui.measure(face, size, s);
        }
        if self.style.shadow {
            let gpd = self.ui.gui_per_dp();
            let off = (0.5 / gpd).max(0.6);
            self.ui
                .text_line(face, size, LINE, x + off, y + off, hex(0x000000, 150), s);
        }
        self.ui.text_line(face, size, LINE, x, y, color, s)
    }
    fn measure(&self, face: Face, size: f32, s: &str) -> f32 {
        self.ui.measure(face, size, s)
    }
    fn label_value_w(&self, label: &str, value: &str) -> f32 {
        let lw = if label.is_empty() {
            0.0
        } else {
            self.measure(Face::Medium, LABEL, label) + 6.0
        };
        lw + self.measure(Face::SemiBold, VALUE, value)
    }
    fn item(&mut self, item: usize, x: f32, y: f32, size: f32) {
        if self.paint {
            let [gx, gy, gw, _] = self.ui.gui_rect(x, y, size, size);
            self.out.push(Draw::Item {
                item,
                x: gx,
                y: gy,
                size: gw,
            });
        }
    }
    fn sprite(&mut self, name: &str, x: f32, y: f32, size: f32) {
        if self.paint {
            let [gx, gy, gw, _] = self.ui.gui_rect(x, y, size, size);
            self.out.push(Draw::Sprite {
                name: name.to_owned(),
                x: gx,
                y: gy,
                size: gw,
            });
        }
    }
    fn size(&mut self, content: &Content) -> (f32, f32) {
        match content {
            Content::Value { label, value } => (
                self.label_value_w(label, value) + 2.0 * PAD_X,
                LINE + 2.0 * PAD_Y,
            ),
            Content::Rows(rows) => {
                let lw = rows
                    .iter()
                    .map(|(l, _)| self.measure(Face::Medium, LABEL, l))
                    .fold(0.0, f32::max);
                let vw = rows
                    .iter()
                    .map(|(_, v)| self.measure(Face::SemiBold, VALUE, v))
                    .fold(0.0, f32::max);
                (
                    lw + 8.0 + vw + 2.0 * PAD_X,
                    rows.len() as f32 * LINE + 2.0 * PAD_Y,
                )
            }
            Content::Ping { label, ms, bars } => {
                let value = ms.map_or_else(|| "local".to_owned(), |m| format!("{m} ms"));
                let extra = if *bars { 22.0 } else { 0.0 };
                (
                    self.label_value_w(label, &value) + extra + 2.0 * PAD_X,
                    LINE + 2.0 * PAD_Y,
                )
            }
            Content::Armor {
                pieces,
                horizontal,
                durability,
            } => {
                let n = pieces.len().max(1) as f32;
                let text_w = pieces
                    .iter()
                    .filter_map(|p| p.durability.and_then(|(l, m)| durability.text(l, m)))
                    .map(|t| self.measure(Face::Mono, LABEL, &t))
                    .fold(0.0, f32::max);
                if *horizontal {
                    let cell = (ICON + 6.0).max(text_w + 4.0);
                    let below = if durability.bar() { 5.0 } else { 0.0 }
                        + if text_w > 0.0 { 14.0 } else { 0.0 };
                    (
                        n * cell + 2.0 * PAD_X - 6.0,
                        ICON + below + 2.0 * PAD_Y + 2.0,
                    )
                } else {
                    let bar = if durability.bar() { 40.0 } else { 0.0 };
                    let gap = if text_w > 0.0 && bar > 0.0 { 6.0 } else { 0.0 };
                    (
                        ICON + 8.0 + bar + gap + text_w + 2.0 * PAD_X,
                        n * (ICON + 4.0) - 4.0 + 2.0 * PAD_Y + 2.0,
                    )
                }
            }
            Content::Effects {
                effects,
                horizontal,
                names,
            } => {
                let n = effects.len().max(1) as f32;
                if *horizontal {
                    let cell = 34.0;
                    (n * cell + 2.0 * PAD_X - 4.0, 18.0 + 14.0 + 2.0 * PAD_Y)
                } else {
                    let tw = effects
                        .iter()
                        .map(|e| {
                            let name = if *names {
                                self.measure(Face::Medium, 12.0, &e.name) + 8.0
                            } else {
                                0.0
                            };
                            name + self.measure(Face::Mono, LABEL, &e.time)
                        })
                        .fold(0.0, f32::max);
                    (18.0 + 8.0 + tw + 2.0 * PAD_X, n * 22.0 - 4.0 + 2.0 * PAD_Y)
                }
            }
            Content::Keys(k) => {
                let (key, gap) = (24.0, 3.0);
                let w = 3.0 * key + 2.0 * gap;
                let mut h = 2.0 * key + gap;
                if k.mouse {
                    h += gap + key;
                }
                if k.space_bar {
                    h += gap + 12.0;
                }
                (w, h)
            }
            Content::Count { count, .. } => (
                ICON + 6.0 + self.measure(Face::SemiBold, VALUE, count) + 2.0 * PAD_X,
                LINE + 2.0 * PAD_Y,
            ),
        }
    }
    fn label_value(&mut self, x: f32, y: f32, label: &str, value: &str) -> f32 {
        let mut x = x;
        if !label.is_empty() {
            x += self.text(Face::Medium, LABEL, x, y, self.accent_c(), label) + 6.0;
        }
        x + self.text(Face::SemiBold, VALUE, x, y, self.text_c(), value)
    }
    fn draw(&mut self, x: f32, y: f32, content: &Content, w: f32) {
        let (ix, iy) = (x + PAD_X, y + PAD_Y);
        match content {
            Content::Value { label, value } => {
                self.label_value(ix, iy, label, value);
            }
            Content::Rows(rows) => {
                let lw = rows
                    .iter()
                    .map(|(l, _)| self.measure(Face::Medium, LABEL, l))
                    .fold(0.0, f32::max);
                for (i, (l, v)) in rows.iter().enumerate() {
                    let ry = iy + i as f32 * LINE;
                    self.text(Face::Medium, LABEL, ix, ry, self.accent_c(), l);
                    self.text(Face::SemiBold, VALUE, ix + lw + 8.0, ry, self.text_c(), v);
                }
            }
            Content::Ping { label, ms, bars } => {
                let value = ms.map_or_else(|| "local".to_owned(), |m| format!("{m} ms"));
                // The value in the latency's colour when there are no bars to say it.
                let end = match ms {
                    Some(m) if !*bars => {
                        let mut x = ix;
                        if !label.is_empty() {
                            x +=
                                self.text(Face::Medium, LABEL, x, iy, self.accent_c(), label) + 6.0;
                        }
                        x + self.text(
                            Face::SemiBold,
                            VALUE,
                            x,
                            iy,
                            hex(ping_color(*m), 255),
                            &value,
                        )
                    }
                    _ => self.label_value(ix, iy, label, &value),
                };
                if *bars {
                    let lit = match ms {
                        None => 5,
                        Some(m) if *m < 0 => 0,
                        Some(m) if *m < 150 => 5,
                        Some(m) if *m < 300 => 4,
                        Some(m) if *m < 600 => 3,
                        Some(m) if *m < 1000 => 2,
                        Some(_) => 1,
                    };
                    let colour = ms.map_or(0x4ADE80, ping_color);
                    for b in 0..5 {
                        let bh = 3.0 + b as f32 * 2.2;
                        let bx = end + 8.0 + b as f32 * 3.0;
                        let by = iy + LINE - 3.0 - bh;
                        let c = if b < lit {
                            hex(colour, 255)
                        } else {
                            hex(0xFFFFFF, 50)
                        };
                        self.ui.rounded(bx, by, 2.0, bh, 1.0, c);
                    }
                }
            }
            Content::Armor {
                pieces,
                horizontal,
                durability,
            } => {
                let text_w = pieces
                    .iter()
                    .filter_map(|p| p.durability.and_then(|(l, m)| durability.text(l, m)))
                    .map(|t| self.measure(Face::Mono, LABEL, &t))
                    .fold(0.0, f32::max);
                for (i, p) in pieces.iter().enumerate() {
                    let (px, py) = if *horizontal {
                        let cell = (ICON + 6.0).max(text_w + 4.0);
                        (ix + i as f32 * cell + (cell - 6.0 - ICON) / 2.0, iy + 1.0)
                    } else {
                        (ix, iy + 1.0 + i as f32 * (ICON + 4.0))
                    };
                    self.item(p.item, px, py, ICON);
                    let Some((left, most)) = p.durability else {
                        continue;
                    };
                    let f = left as f32 / most.max(1) as f32;
                    let colour = durability_color(f);
                    let text = durability.text(left, most);
                    if *horizontal {
                        let mut by = py + ICON + 2.0;
                        if durability.bar() {
                            self.bar(px, by, ICON, f, colour);
                            by += 5.0;
                        }
                        if let Some(t) = text {
                            let tw = self.measure(Face::Mono, LABEL, &t);
                            self.ui.text_line(
                                Face::Mono,
                                LABEL,
                                14.0,
                                px + (ICON - tw) / 2.0,
                                by,
                                hex(colour, 255),
                                &t,
                            );
                        }
                    } else {
                        let mut tx = px + ICON + 8.0;
                        if durability.bar() {
                            self.bar(tx, py + ICON / 2.0 - 1.5, 40.0, f, colour);
                            tx += 46.0;
                        }
                        if let Some(t) = text {
                            self.ui.text_line(
                                Face::Mono,
                                LABEL,
                                ICON,
                                tx,
                                py,
                                hex(colour, 255),
                                &t,
                            );
                        }
                    }
                }
            }
            Content::Effects {
                effects,
                horizontal,
                names,
            } => {
                for (i, e) in effects.iter().enumerate() {
                    let time_c = if e.ending {
                        hex(0xFF6B6B, 255)
                    } else {
                        self.muted_c()
                    };
                    if *horizontal {
                        let ex = ix + i as f32 * 34.0;
                        self.sprite(&e.sprite, ex + 6.0, iy, 18.0);
                        let tw = self.measure(Face::Mono, LABEL, &e.time);
                        self.ui.text_line(
                            Face::Mono,
                            LABEL,
                            14.0,
                            ex + 15.0 - tw / 2.0,
                            iy + 18.0,
                            time_c,
                            &e.time,
                        );
                    } else {
                        let ey = iy + i as f32 * 22.0;
                        self.sprite(&e.sprite, ix, ey, 18.0);
                        let mut tx = ix + 26.0;
                        if *names {
                            tx +=
                                self.text(Face::Medium, 12.0, tx, ey, self.text_c(), &e.name) + 8.0;
                        }
                        self.text(Face::Mono, LABEL, tx, ey, time_c, &e.time);
                    }
                }
            }
            Content::Keys(k) => self.keys(x, y, k, w),
            Content::Count { item, count } => {
                self.item(*item, ix, iy + 1.0, ICON);
                self.text(
                    Face::SemiBold,
                    VALUE,
                    ix + ICON + 6.0,
                    iy,
                    self.text_c(),
                    count,
                );
            }
        }
    }
    /// A durability bar: a track and the part left in `colour`.
    fn bar(&mut self, x: f32, y: f32, w: f32, fraction: f32, colour: u32) {
        self.ui.rounded(x, y, w, 3.0, 1.5, hex(0x000000, 120));
        self.ui.rounded(
            x,
            y,
            (w * fraction.clamp(0.0, 1.0)).max(1.5),
            3.0,
            1.5,
            hex(colour, 255),
        );
    }
    fn keys(&mut self, x: f32, y: f32, k: &Keys, w: f32) {
        let (key, gap) = (24.0, 3.0);
        let a = (self.style.opacity.clamp(0.0, 1.0) * 255.0) as u8;
        let up = if self.style.background {
            hex(0x15112E, a.max(40))
        } else {
            hex(0x15112E, 0)
        };
        let accent = self.style.accent;
        let down = |level: f32| hex(accent, (235.0 * level.clamp(0.0, 1.0)) as u8);
        let cell = |p: &mut Self,
                    cx: f32,
                    cy: f32,
                    cw: f32,
                    ch: f32,
                    level: f32,
                    label: &str,
                    sub: Option<String>| {
            // A pressed key lights up at once and fades out when let go.
            p.ui.rounded(cx, cy, cw, ch, 6.0, up);
            if level > 0.01 {
                p.ui.rounded(cx, cy, cw, ch, 6.0, down(level));
            }
            if p.style.background || level > 0.01 {
                let rim = 22.0 + 38.0 * level.clamp(0.0, 1.0);
                p.ui.border(cx, cy, cw, ch, 6.0, hex(0xFFFFFF, rim as u8));
            }
            let size = if sub.is_some() { 10.0 } else { 12.0 };
            let lw = p.measure(Face::Medium, size, label);
            let ly = if sub.is_some() {
                cy + 1.0
            } else {
                cy + (ch - LINE) / 2.0
            };
            p.text(
                Face::Medium,
                size,
                cx + (cw - lw) / 2.0,
                ly,
                p.text_c(),
                label,
            );
            if let Some(s) = sub {
                let sw = p.measure(Face::Mono, 9.0, &s);
                p.ui.text_line(
                    Face::Mono,
                    9.0,
                    10.0,
                    cx + (cw - sw) / 2.0,
                    cy + ch - 12.0,
                    p.muted_c(),
                    &s,
                );
            }
        };
        cell(self, x + key + gap, y, key, key, k.w, "W", None);
        let row = y + key + gap;
        cell(self, x, row, key, key, k.a, "A", None);
        cell(self, x + key + gap, row, key, key, k.s, "S", None);
        cell(self, x + 2.0 * (key + gap), row, key, key, k.d, "D", None);
        let mut next = row + key + gap;
        if k.mouse {
            let half = (w - gap) / 2.0;
            let (l, r) = k.cps.map_or((None, None), |(l, r)| {
                (Some(format!("{l} CPS")), Some(format!("{r} CPS")))
            });
            cell(self, x, next, half, key, k.lmb, "LMB", l);
            cell(self, x + half + gap, next, half, key, k.rmb, "RMB", r);
            next += key + gap;
        }
        if k.space_bar {
            self.ui.rounded(x, next, w, 12.0, 6.0, up);
            if k.space > 0.01 {
                self.ui.rounded(x, next, w, 12.0, 6.0, down(k.space));
            }
            if self.style.background || k.space > 0.01 {
                self.ui.border(x, next, w, 12.0, 6.0, hex(0xFFFFFF, 22));
            }
            self.ui.rounded(
                x + w / 2.0 - 12.0,
                next + 5.0,
                24.0,
                2.0,
                1.0,
                self.muted_c(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durability_and_ping_colours_step_down() {
        assert_eq!(durability_color(1.0), 0x4ADE80);
        assert_eq!(durability_color(0.5), 0xFFD43B);
        assert_eq!(durability_color(0.2), 0xFFA94D);
        assert_eq!(durability_color(0.05), 0xFF6B6B);
        assert_eq!(ping_color(20), 0x4ADE80);
        assert_eq!(ping_color(400), 0xFF6B6B);
        assert_eq!(Durability::from_index(2).text(50, 200), Some("25%".into()));
        assert_eq!(Durability::from_index(0).text(50, 200), None);
        assert_eq!(Durability::from_index(9), Durability::BarAndNumber);
    }

    #[test]
    fn widgets_grow_away_from_the_edge_they_sit_by() {
        // By the left edge: the left edge stays.
        assert_eq!(anchor_place(0.0, 50.0, 400.0), 0.0);
        assert_eq!(anchor_of(10.0, 50.0, 400.0), 10.0 / 400.0);
        // By the right edge: the right edge stays as it grows.
        let f = anchor_of(350.0, 50.0, 400.0);
        assert_eq!(f, 1.0);
        assert_eq!(anchor_place(f, 80.0, 400.0), 320.0);
        // In the middle: the centre stays.
        let f = anchor_of(175.0, 50.0, 400.0);
        assert_eq!(anchor_place(f, 50.0, 400.0), 175.0);
        assert_eq!(anchor_place(f, 70.0, 400.0), 165.0);
        // Every place comes back where it was put.
        for start in [0.0, 37.0, 120.0, 200.0, 290.0, 330.0] {
            let f = anchor_of(start, 70.0, 400.0);
            assert!(
                (anchor_place(f, 70.0, 400.0) - start).abs() < 0.01,
                "{start}"
            );
        }
    }
}
