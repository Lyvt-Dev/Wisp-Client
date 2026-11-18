//! The HUD widgets of the Wisp Menu (`docs/extras.md`): small plates drawn with the bundled
//! fonts or Minecraft's, one per widget, every part of their look the player's to choose
//! (`features::WIDGET_GROUPS`). The game (`game/hud_widgets.rs`) gathers what each shows into a
//! [`Content`], places it and draws the items and effect icons this returns on top. One
//! function measures and draws, so the editor's outlines fit what is drawn.
use crate::hud::HudBatch;
use crate::ui_theme::{Face, Ui, color::hex};

/// The font a widget writes in (`features::FONTS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Wisp,
    WispBold,
    Mono,
    /// Minecraft's own font, at whole pixels so it stays sharp.
    Minecraft,
}
impl Font {
    pub fn from_index(i: usize) -> Self {
        [Self::Wisp, Self::WispBold, Self::Mono, Self::Minecraft][i.min(3)]
    }
}
/// How a label and its value are written (`features::FORMATS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// `FPS 144`
    LabelValue,
    /// `144 FPS`
    ValueLabel,
    /// `FPS: 144`
    Colon,
    /// `[FPS: 144]`
    Brackets,
    /// `144`
    ValueOnly,
}
impl Format {
    pub fn from_index(i: usize) -> Self {
        [
            Self::LabelValue,
            Self::ValueLabel,
            Self::Colon,
            Self::Brackets,
            Self::ValueOnly,
        ][i.min(4)]
    }
}
/// The plate's edge (`features::BORDERS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Border {
    None,
    Subtle,
    Solid,
    LeftBar,
    Underline,
}
impl Border {
    pub fn from_index(i: usize) -> Self {
        [
            Self::None,
            Self::Subtle,
            Self::Solid,
            Self::LeftBar,
            Self::Underline,
        ][i.min(4)]
    }
}
/// Where the content sits in a plate wider than it (`features::ALIGNS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Centre,
    Right,
}
impl Align {
    pub fn from_index(i: usize) -> Self {
        [Self::Left, Self::Centre, Self::Right][i.min(2)]
    }
}

/// A widget's look (`features::WIDGET_GROUPS`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub font: Font,
    pub format: Format,
    /// The value's colour, `0xRRGGBB`; also a pressed key's.
    pub text: u32,
    /// The label's colour.
    pub label: u32,
    /// Rainbow text cycling at this speed (`None`: the colours above).
    pub chroma: Option<f32>,
    pub uppercase: bool,
    pub shadow: bool,
    pub background: bool,
    /// The plate's colour.
    pub bg: u32,
    /// 0 to 1.
    pub opacity: f32,
    pub plate_shadow: bool,
    pub border: Border,
    pub border_color: u32,
    /// Corner radius in dp.
    pub radius: f32,
    /// Space round the content, in dp.
    pub padding: f32,
    pub align: Align,
    /// Seconds since some start: chroma and blinking move with it.
    pub time: f32,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            font: Font::Wisp,
            format: Format::LabelValue,
            text: 0xFFFFFF,
            label: 0xA1A1AA,
            chroma: None,
            uppercase: false,
            shadow: true,
            background: true,
            bg: 0x0B0B0E,
            opacity: 0.55,
            plate_shadow: false,
            border: Border::Subtle,
            border_color: 0xFFFFFF,
            radius: 4.0,
            padding: 7.0,
            align: Align::Centre,
            time: 0.0,
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
    /// Under 10 seconds left: the time turns red (and blinks, when asked).
    pub ending: bool,
}
/// The keys Keystrokes shows: how pressed each looks, 1 held, fading to 0 after it is let go.
#[derive(Clone, Copy, Debug, PartialEq)]
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
    /// Arrows on the movement keys instead of W A S D.
    pub arrows: bool,
    /// A key's side and the gap between keys, in dp.
    pub size: f32,
    pub gap: f32,
}
impl Default for Keys {
    fn default() -> Self {
        Self {
            w: 0.0,
            a: 0.0,
            s: 0.0,
            d: 0.0,
            space: 0.0,
            lmb: 0.0,
            rmb: 0.0,
            cps: None,
            mouse: true,
            space_bar: true,
            arrows: false,
            size: 24.0,
            gap: 3.0,
        }
    }
}

/// What a widget shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    /// A label and a value, "FPS 144"; `tint` colours the value (a status colour).
    Value {
        label: String,
        value: String,
        tint: Option<u32>,
    },
    /// Label and value rows, stacked.
    Rows(Vec<(String, String)>),
    /// Latency with signal bars (`None`: single player); `colors` tints the value by latency.
    Ping {
        label: String,
        ms: Option<i32>,
        bars: bool,
        colors: bool,
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
        blink: bool,
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

const LABEL: f32 = 11.0;
const VALUE: f32 = 13.0;
const SMALL: f32 = 11.0;
const LINE: f32 = 17.0;
const ICON: f32 = 16.0;
/// The gap between a label and its value.
const GAP: f32 = 5.0;

/// The colour of `fraction` of durability left: green, yellow, orange, red.
pub fn durability_color(fraction: f32) -> u32 {
    match fraction {
        f if f > 0.6 => 0x4ADE80,
        f if f > 0.3 => 0xFFD43B,
        f if f > 0.15 => 0xFF9F43,
        _ => 0xFF5C5C,
    }
}
/// The colour of a frame rate: green from 60, yellow from 30, red below.
pub fn fps_color(fps: u32) -> u32 {
    match fps {
        60.. => 0x4ADE80,
        30..60 => 0xFFD43B,
        _ => 0xFF5C5C,
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
        150..300 => 0xFF9F43,
        _ => 0xFF5C5C,
    }
}
/// A colour of the rainbow, `0xRRGGBB`, at `hue` turns (0 to 1, wrapping).
pub fn rainbow(hue: f32) -> u32 {
    hsv(hue, 0.62, 1.0)
}
/// `0xRRGGBB` from hue (turns), saturation and value (0 to 1).
pub fn hsv(hue: f32, s: f32, v: f32) -> u32 {
    let h = hue.rem_euclid(1.0) * 6.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let byte = |f: f32| ((f + m).clamp(0.0, 1.0) * 255.0).round() as u32;
    byte(r) << 16 | byte(g) << 8 | byte(b)
}
/// Black or white, whichever reads on `0xRRGGBB`.
pub fn ink_on(c: u32) -> u32 {
    let [r, g, b] = [c >> 16 & 0xff, c >> 8 & 0xff, c & 0xff].map(|v| v as f32 / 255.0);
    if 0.299 * r + 0.587 * g + 0.114 * b > 0.6 {
        0x0B0B0E
    } else {
        0xFFFFFF
    }
}

/// What a piece of text is, for its font, size and colour.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    Label,
    Value,
    /// The value in a status colour.
    Tinted(u32),
    /// Brackets and separators: the label's colour, dimmer.
    Muted,
}

/// A label and value as the format writes them: the pieces in order.
fn value_pieces(style: &Style, label: &str, value: &str, tint: Option<u32>) -> Vec<(String, Role)> {
    let value_role = tint.map_or(Role::Value, Role::Tinted);
    let label = if style.uppercase {
        label.to_uppercase()
    } else {
        label.to_owned()
    };
    if label.is_empty() || style.format == Format::ValueOnly {
        return vec![(value.to_owned(), value_role)];
    }
    match style.format {
        Format::LabelValue => vec![(label, Role::Label), (value.to_owned(), value_role)],
        Format::ValueLabel => vec![(value.to_owned(), value_role), (label, Role::Label)],
        Format::Colon => vec![
            (format!("{label}:"), Role::Label),
            (value.to_owned(), value_role),
        ],
        Format::Brackets => vec![
            ("[".to_owned(), Role::Muted),
            (format!("{label}:"), Role::Label),
            (value.to_owned(), value_role),
            ("]".to_owned(), Role::Muted),
        ],
        Format::ValueOnly => unreachable!(),
    }
}

/// Measures (`paint` false) or draws a widget with its top left at `(x, y)` dp: returns its size
/// in dp and, when painting, the items and sprites to draw on top. The plate is at least `min_w`
/// wide, its content placed in it by the style's alignment, so a number that briefly gets
/// shorter doesn't shake it.
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
    let keys = matches!(content, Content::Keys(_));
    let w = if keys { own_w } else { own_w.max(min_w) };
    if paint {
        if !keys {
            p.plate(x, y, w, h);
        }
        let room = w - own_w;
        let dx = match style.align {
            Align::Left => 0.0,
            Align::Centre => (room / 2.0).floor(),
            Align::Right => room,
        };
        p.draw(x + dx, y, content, own_w);
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
    fn pad_x(&self) -> f32 {
        self.style.padding.max(0.0)
    }
    fn pad_y(&self) -> f32 {
        (self.style.padding * 0.55).clamp(1.0, 10.0)
    }
    fn radius(&self, w: f32, h: f32) -> f32 {
        self.style.radius.clamp(0.0, w.min(h) / 2.0)
    }
    /// The plate: its shadow, fill and edge.
    fn plate(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let s = self.style;
        let r = self.radius(w, h);
        let a = (s.opacity.clamp(0.0, 1.0) * 255.0) as u8;
        if s.background && a > 0 {
            if s.plate_shadow {
                let sa = (f32::from(a) * 0.45) as u8;
                self.ui.rounded(
                    x - 0.5,
                    y + 1.0,
                    w + 1.0,
                    h + 1.5,
                    r + 1.0,
                    hex(0x000000, sa / 2),
                );
                self.ui.rounded(x, y + 1.0, w, h, r, hex(0x000000, sa));
            }
            self.ui.rounded(x, y, w, h, r, hex(s.bg, a));
        }
        let c = s.border_color;
        match s.border {
            Border::None => {}
            Border::Subtle => self.ui.border(x, y, w, h, r, hex(c, 34)),
            Border::Solid => self.ui.border(x, y, w, h, r, hex(c, 235)),
            Border::LeftBar => {
                let inset = (r * 0.6).min(h / 3.0);
                self.ui
                    .rounded(x, y + inset, 2.0, h - 2.0 * inset, 1.0, hex(c, 255));
            }
            Border::Underline => {
                let inset = r.max(2.0).min(w / 3.0);
                self.ui.rounded(
                    x + inset,
                    y + h - 1.5,
                    w - 2.0 * inset,
                    1.5,
                    0.75,
                    hex(c, 255),
                );
            }
        }
    }
    fn face(&self, role: Role) -> Option<Face> {
        let label = matches!(role, Role::Label | Role::Muted);
        match self.style.font {
            Font::Wisp => Some(if label { Face::Medium } else { Face::SemiBold }),
            Font::WispBold => Some(if label { Face::SemiBold } else { Face::Heading }),
            Font::Mono => Some(Face::Mono),
            Font::Minecraft => None,
        }
    }
    fn size_of(role: Role) -> f32 {
        if matches!(role, Role::Label | Role::Muted) {
            LABEL
        } else {
            VALUE
        }
    }
    fn color(&self, role: Role) -> u32 {
        match role {
            Role::Label => hex(self.style.label, 255),
            Role::Value => hex(self.style.text, 255),
            Role::Tinted(c) => hex(c, 255),
            Role::Muted => hex(self.style.label, 150),
        }
    }
    /// Minecraft's font: GUI pixels per font pixel for text of `size` dp, whole physical pixels.
    fn mc_scale(&self, size: f32) -> f32 {
        let s = self.ui.batch.scale.max(1.0);
        let want = size * self.ui.gui_per_dp() / 8.0 * s;
        want.round().max(1.0) / s
    }
    fn measure(&self, face: Option<Face>, size: f32, s: &str) -> f32 {
        match face {
            Some(f) => self.ui.measure(f, size, s),
            None => {
                HudBatch::text_width(self.ui.atlas, s) * self.mc_scale(size) / self.ui.gui_per_dp()
            }
        }
    }
    fn measure_role(&self, role: Role, s: &str) -> f32 {
        self.measure(self.face(role), Self::size_of(role), s)
    }
    /// The gap between pieces of text: a space of Minecraft's font, else [`GAP`].
    fn gap(&self) -> f32 {
        if self.style.font == Font::Minecraft {
            self.measure(None, VALUE, " ")
        } else {
            GAP
        }
    }
    /// Text in a `line`-tall box at `y`, with the shadow when it is on, in the rainbow when
    /// chroma is; returns its width.
    #[allow(clippy::too_many_arguments)]
    fn text_in(
        &mut self,
        face: Option<Face>,
        size: f32,
        line: f32,
        x: f32,
        y: f32,
        color: u32,
        s: &str,
    ) -> f32 {
        if !self.paint {
            return self.measure(face, size, s);
        }
        let Some(speed) = self.style.chroma else {
            return self.run(face, size, line, x, y, color, s);
        };
        // Each character a step further round the rainbow, the whole moving with time.
        let alpha = color & 0xff00_0000;
        let mut pen = x;
        let mut buf = [0u8; 4];
        for ch in s.chars() {
            let c = ch.encode_utf8(&mut buf);
            let (gx, _) = self.ui.to_gui(pen, y);
            let hue = gx * 0.006 - self.style.time * 0.35 * speed;
            let rgb = hex(rainbow(hue), 255) & 0x00ff_ffff;
            pen += self.run(face, size, line, pen, y, alpha | rgb, c);
        }
        pen - x
    }
    #[allow(clippy::too_many_arguments)]
    fn run(
        &mut self,
        face: Option<Face>,
        size: f32,
        line: f32,
        x: f32,
        y: f32,
        color: u32,
        s: &str,
    ) -> f32 {
        let Some(face) = face else {
            let f = self.mc_scale(size);
            let gpd = self.ui.gui_per_dp();
            let (gx, gy) = self.ui.to_gui(x, y + (line - 8.0 * f / gpd) / 2.0);
            let batch = &mut *self.ui.batch;
            let start = batch.vertices.len();
            let advance = batch.text(self.ui.atlas, s, gx, gy, color, self.style.shadow);
            batch.scale_since(start, gx, gy, f, f);
            return advance * f / gpd;
        };
        if self.style.shadow {
            let off = (1.0 / self.ui.density).max(0.5);
            let a = ((color >> 24) as f32 * 0.6) as u8;
            self.ui
                .text_line(face, size, line, x + off, y + off, hex(0x000000, a), s);
        }
        self.ui.text_line(face, size, line, x, y, color, s)
    }
    fn text_role(&mut self, role: Role, x: f32, y: f32, s: &str) -> f32 {
        let c = self.color(role);
        self.text_in(self.face(role), Self::size_of(role), LINE, x, y, c, s)
    }
    /// Small numbers (durability, times, clicks per second) in `color`.
    fn small(&mut self, size: f32, line: f32, x: f32, y: f32, color: u32, s: &str) -> f32 {
        let face = match self.style.font {
            Font::Minecraft => None,
            Font::Mono | Font::Wisp => Some(Face::Mono),
            Font::WispBold => Some(Face::SemiBold),
        };
        self.text_in(face, size, line, x, y, color, s)
    }
    fn small_w(&self, size: f32, s: &str) -> f32 {
        let face = match self.style.font {
            Font::Minecraft => None,
            Font::Mono | Font::Wisp => Some(Face::Mono),
            Font::WispBold => Some(Face::SemiBold),
        };
        self.measure(face, size, s)
    }
    fn pieces_w(&self, pieces: &[(String, Role)]) -> f32 {
        let gap = self.gap();
        let mut w = 0.0;
        for (i, (s, role)) in pieces.iter().enumerate() {
            // No gap inside the brackets.
            if i > 0 && !matches!(role, Role::Muted) && pieces[i - 1].1 != Role::Muted {
                w += gap;
            }
            w += self.measure_role(*role, s);
        }
        w
    }
    fn draw_pieces(&mut self, x: f32, y: f32, pieces: &[(String, Role)]) -> f32 {
        let gap = self.gap();
        let mut pen = x;
        for (i, (s, role)) in pieces.iter().enumerate() {
            if i > 0 && !matches!(role, Role::Muted) && pieces[i - 1].1 != Role::Muted {
                pen += gap;
            }
            pen += self.text_role(*role, pen, y, s);
        }
        pen
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
    fn rows_label(&self, l: &str) -> String {
        let l = if self.style.uppercase {
            l.to_uppercase()
        } else {
            l.to_owned()
        };
        match self.style.format {
            Format::Colon | Format::Brackets => format!("{l}:"),
            _ => l,
        }
    }
    fn ping_value(ms: Option<i32>) -> String {
        ms.map_or_else(|| "local".to_owned(), |m| format!("{m} ms"))
    }
    fn ping_pieces(&self, label: &str, ms: Option<i32>, colors: bool) -> Vec<(String, Role)> {
        let tint = ms.filter(|_| colors).map(ping_color);
        value_pieces(self.style, label, &Self::ping_value(ms), tint)
    }
    fn size(&mut self, content: &Content) -> (f32, f32) {
        let (px, py) = (self.pad_x(), self.pad_y());
        match content {
            Content::Value { label, value, tint } => {
                let pieces = value_pieces(self.style, label, value, *tint);
                (self.pieces_w(&pieces) + 2.0 * px, LINE + 2.0 * py)
            }
            Content::Rows(rows) => {
                let labels = self.style.format != Format::ValueOnly;
                let lw = if labels {
                    rows.iter()
                        .map(|(l, _)| self.measure_role(Role::Label, &self.rows_label(l)))
                        .fold(0.0, f32::max)
                } else {
                    0.0
                };
                let vw = rows
                    .iter()
                    .map(|(_, v)| self.measure_role(Role::Value, v))
                    .fold(0.0, f32::max);
                let gap = if labels { self.gap() + 2.0 } else { 0.0 };
                (
                    lw + gap + vw + 2.0 * px,
                    rows.len() as f32 * LINE + 2.0 * py,
                )
            }
            Content::Ping {
                label,
                ms,
                bars,
                colors,
            } => {
                let pieces = self.ping_pieces(label, *ms, *colors);
                let extra = if *bars { 20.0 } else { 0.0 };
                (self.pieces_w(&pieces) + extra + 2.0 * px, LINE + 2.0 * py)
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
                    .map(|t| self.small_w(SMALL, &t))
                    .fold(0.0, f32::max);
                if *horizontal {
                    let cell = (ICON + 6.0).max(text_w + 4.0);
                    let below = if durability.bar() { 5.0 } else { 0.0 }
                        + if text_w > 0.0 { 14.0 } else { 0.0 };
                    (n * cell + 2.0 * px - 6.0, ICON + below + 2.0 * py + 2.0)
                } else {
                    let bar = if durability.bar() { 40.0 } else { 0.0 };
                    let gap = if text_w > 0.0 && bar > 0.0 { 6.0 } else { 0.0 };
                    (
                        ICON + 8.0 + bar + gap + text_w + 2.0 * px,
                        n * (ICON + 4.0) - 4.0 + 2.0 * py + 2.0,
                    )
                }
            }
            Content::Effects {
                effects,
                horizontal,
                names,
                ..
            } => {
                let n = effects.len().max(1) as f32;
                if *horizontal {
                    let cell = 34.0;
                    (n * cell + 2.0 * px - 4.0, 18.0 + 14.0 + 2.0 * py)
                } else {
                    let tw = effects
                        .iter()
                        .map(|e| {
                            let name = if *names {
                                self.measure_role(Role::Label, &e.name) + 8.0
                            } else {
                                0.0
                            };
                            name + self.small_w(SMALL, &e.time)
                        })
                        .fold(0.0, f32::max);
                    (18.0 + 8.0 + tw + 2.0 * px, n * 22.0 - 4.0 + 2.0 * py)
                }
            }
            Content::Keys(k) => {
                let (key, gap) = (k.size, k.gap);
                let w = 3.0 * key + 2.0 * gap;
                let mut h = 2.0 * key + gap;
                if k.mouse {
                    h += gap + key * 0.85;
                }
                if k.space_bar {
                    h += gap + key * 0.5;
                }
                (w, h)
            }
            Content::Count { count, .. } => (
                ICON + 6.0 + self.measure_role(Role::Value, count) + 2.0 * px,
                LINE + 2.0 * py,
            ),
        }
    }
    fn draw(&mut self, x: f32, y: f32, content: &Content, w: f32) {
        let (ix, iy) = (x + self.pad_x(), y + self.pad_y());
        match content {
            Content::Value { label, value, tint } => {
                let pieces = value_pieces(self.style, label, value, *tint);
                self.draw_pieces(ix, iy, &pieces);
            }
            Content::Rows(rows) => {
                let labels = self.style.format != Format::ValueOnly;
                let lw = if labels {
                    rows.iter()
                        .map(|(l, _)| self.measure_role(Role::Label, &self.rows_label(l)))
                        .fold(0.0, f32::max)
                } else {
                    0.0
                };
                let gap = if labels { self.gap() + 2.0 } else { 0.0 };
                for (i, (l, v)) in rows.iter().enumerate() {
                    let ry = iy + i as f32 * LINE;
                    if labels {
                        let l = self.rows_label(l);
                        self.text_role(Role::Label, ix, ry, &l);
                    }
                    self.text_role(Role::Value, ix + lw + gap, ry, v);
                }
            }
            Content::Ping {
                label,
                ms,
                bars,
                colors,
            } => {
                let pieces = self.ping_pieces(label, *ms, *colors);
                let end = self.draw_pieces(ix, iy, &pieces);
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
                    let colour = if *colors {
                        ms.map_or(0x4ADE80, ping_color)
                    } else {
                        self.style.text
                    };
                    for b in 0..5 {
                        let bh = 3.0 + b as f32 * 2.0;
                        let bx = end + 7.0 + b as f32 * 2.6;
                        let by = iy + LINE - 3.5 - bh;
                        let c = if b < lit {
                            hex(colour, 255)
                        } else {
                            hex(self.style.text, 45)
                        };
                        self.ui.rounded(bx, by, 1.6, bh, 0.5, c);
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
                    .map(|t| self.small_w(SMALL, &t))
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
                            let tw = self.small_w(SMALL, &t);
                            self.small(
                                SMALL,
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
                            self.small(SMALL, ICON, tx, py, hex(colour, 255), &t);
                        }
                    }
                }
            }
            Content::Effects {
                effects,
                horizontal,
                names,
                blink,
            } => {
                let flash = *blink && (self.style.time * 2.5).fract() < 0.5;
                for (i, e) in effects.iter().enumerate() {
                    let time_c = match (e.ending, flash) {
                        (true, true) => hex(0xFF5C5C, 90),
                        (true, false) => hex(0xFF5C5C, 255),
                        _ => hex(self.style.label, 255),
                    };
                    if *horizontal {
                        let ex = ix + i as f32 * 34.0;
                        self.sprite(&e.sprite, ex + 6.0, iy, 18.0);
                        let tw = self.small_w(SMALL, &e.time);
                        self.small(
                            SMALL,
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
                            let c = hex(self.style.text, 255);
                            tx += self.text_in(
                                self.face(Role::Label),
                                12.0,
                                18.0,
                                tx,
                                ey,
                                c,
                                &e.name,
                            ) + 8.0;
                        }
                        self.small(SMALL, 18.0, tx, ey, time_c, &e.time);
                    }
                }
            }
            Content::Keys(k) => self.keys(x, y, k, w),
            Content::Count { item, count } => {
                self.item(*item, ix, iy, ICON);
                self.text_role(Role::Value, ix + ICON + 6.0, iy, count);
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
    /// One key of Keystrokes: its plate in the widget's look, lit by how pressed it is.
    #[allow(clippy::too_many_arguments)]
    fn key_cell(
        &mut self,
        (cx, cy, cw, ch): (f32, f32, f32, f32),
        level: f32,
        label: KeyLabel,
        sub: Option<String>,
    ) {
        let s = *self.style;
        let level = level.clamp(0.0, 1.0);
        let r = self.radius(cw, ch);
        let a = (s.opacity.clamp(0.0, 1.0) * 255.0) as u8;
        if s.background && a > 0 {
            if s.plate_shadow {
                self.ui.rounded(
                    cx,
                    cy + 1.0,
                    cw,
                    ch,
                    r,
                    hex(0x000000, (f32::from(a) * 0.45) as u8),
                );
            }
            self.ui.rounded(cx, cy, cw, ch, r, hex(s.bg, a));
        }
        // A pressed key fills with the value's colour at once and fades out when let go.
        if level > 0.01 {
            self.ui
                .rounded(cx, cy, cw, ch, r, hex(s.text, (235.0 * level) as u8));
        }
        match s.border {
            Border::None => {}
            Border::Subtle => self.ui.border(cx, cy, cw, ch, r, hex(s.border_color, 34)),
            Border::Solid => self.ui.border(cx, cy, cw, ch, r, hex(s.border_color, 235)),
            Border::LeftBar | Border::Underline => {
                let inset = r.max(2.0).min(cw / 3.0);
                self.ui.rounded(
                    cx + inset,
                    cy + ch - 1.5,
                    cw - 2.0 * inset,
                    1.5,
                    0.75,
                    hex(s.border_color, 255),
                );
            }
        }
        // The label turns dark on a light pressed key.
        let ink = mix(s.text, ink_on(s.text), level);
        let colour = hex(ink, 255);
        let size = (ch * 0.46).clamp(8.0, 18.0);
        match label {
            KeyLabel::Text(t) => {
                let small = sub.is_some();
                let size = if small { size * 0.82 } else { size };
                let face = self.face(Role::Value);
                let lw = self.measure(face, size, t);
                let line = if small { ch * 0.55 } else { ch };
                let ly = if small { cy + ch * 0.06 } else { cy };
                let lx = cx + (cw - lw) / 2.0;
                if level > 0.5 {
                    // Pressed: the ink that reads on the fill, never the rainbow.
                    self.run(face, size, line, lx, ly, colour, t);
                } else {
                    self.text_in(face, size, line, lx, ly, colour, t);
                }
                if let Some(sub) = sub {
                    let ss = (size * 0.62).max(6.5);
                    let sw = self.small_w(ss, &sub);
                    let c = hex(ink, 170);
                    self.small(ss, ch * 0.4, cx + (cw - sw) / 2.0, cy + ch * 0.56, c, &sub);
                }
            }
            KeyLabel::Arrow(turns) => {
                // A chevron pointing `turns` of a full turn from up.
                let (mx, my) = (cx + cw / 2.0, cy + ch / 2.0);
                let arm = (ch * 0.16).max(2.5);
                let angle = turns * std::f32::consts::TAU;
                let rot = |dx: f32, dy: f32| {
                    let (s, c) = angle.sin_cos();
                    [mx + dx * c - dy * s, my + dx * s + dy * c]
                };
                let pts = [
                    rot(-arm, arm * 0.5),
                    rot(0.0, -arm * 0.5),
                    rot(arm, arm * 0.5),
                ];
                if s.shadow && level < 0.5 {
                    let off = (1.0 / self.ui.density).max(0.5);
                    let shade: Vec<[f32; 2]> =
                        pts.iter().map(|[x, y]| [x + off, y + off]).collect();
                    self.ui.polyline(&shade, 1.6, hex(0x000000, 150));
                }
                self.ui.polyline(&pts, 1.6, colour);
            }
        }
    }
    fn keys(&mut self, x: f32, y: f32, k: &Keys, w: f32) {
        let (key, gap) = (k.size, k.gap);
        let label = |letter: &'static str, turns: f32| {
            if k.arrows {
                KeyLabel::Arrow(turns)
            } else {
                KeyLabel::Text(letter)
            }
        };
        self.key_cell((x + key + gap, y, key, key), k.w, label("W", 0.0), None);
        let row = y + key + gap;
        self.key_cell((x, row, key, key), k.a, label("A", 0.75), None);
        self.key_cell((x + key + gap, row, key, key), k.s, label("S", 0.5), None);
        self.key_cell(
            (x + 2.0 * (key + gap), row, key, key),
            k.d,
            label("D", 0.25),
            None,
        );
        let mut next = row + key + gap;
        if k.mouse {
            let half = (w - gap) / 2.0;
            let mh = key * 0.85;
            let (l, r) = k.cps.map_or((None, None), |(l, r)| {
                (Some(format!("{l} CPS")), Some(format!("{r} CPS")))
            });
            self.key_cell((x, next, half, mh), k.lmb, KeyLabel::Text("LMB"), l);
            self.key_cell(
                (x + half + gap, next, half, mh),
                k.rmb,
                KeyLabel::Text("RMB"),
                r,
            );
            next += mh + gap;
        }
        if k.space_bar {
            let sh = key * 0.5;
            self.key_cell((x, next, w, sh), k.space, KeyLabel::Text(""), None);
            let ink = mix(
                self.style.text,
                ink_on(self.style.text),
                k.space.clamp(0.0, 1.0),
            );
            let bar = (w * 0.22).min(28.0);
            self.ui.rounded(
                x + (w - bar) / 2.0,
                next + sh / 2.0 - 0.75,
                bar,
                1.5,
                0.75,
                hex(ink, 200),
            );
        }
    }
}

/// What a key of Keystrokes shows.
#[derive(Clone, Copy)]
enum KeyLabel {
    Text(&'static str),
    /// A chevron, turned from up by this part of a turn.
    Arrow(f32),
}

/// `a` towards `b` by `t` (0 to 1), per channel of `0xRRGGBB`.
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |shift: u32| {
        let (x, y) = ((a >> shift & 0xff) as f32, (b >> shift & 0xff) as f32);
        ((x + (y - x) * t).round() as u32) << shift
    };
    ch(16) | ch(8) | ch(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durability_ping_and_frame_rate_colours_step_down() {
        assert_eq!(durability_color(1.0), 0x4ADE80);
        assert_eq!(durability_color(0.5), 0xFFD43B);
        assert_eq!(durability_color(0.2), 0xFF9F43);
        assert_eq!(durability_color(0.05), 0xFF5C5C);
        assert_eq!(ping_color(20), 0x4ADE80);
        assert_eq!(ping_color(400), 0xFF5C5C);
        assert_eq!(fps_color(144), 0x4ADE80);
        assert_eq!(fps_color(45), 0xFFD43B);
        assert_eq!(fps_color(12), 0xFF5C5C);
        assert_eq!(Durability::from_index(2).text(50, 200), Some("25%".into()));
        assert_eq!(Durability::from_index(0).text(50, 200), None);
        assert_eq!(Durability::from_index(9), Durability::BarAndNumber);
    }

    #[test]
    fn each_format_writes_label_and_value_in_its_order() {
        let text = |format: Format, label: &str| {
            let style = Style {
                format,
                ..Style::default()
            };
            value_pieces(&style, label, "144", None)
                .into_iter()
                .map(|(s, _)| s)
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert_eq!(text(Format::LabelValue, "FPS"), "FPS 144");
        assert_eq!(text(Format::ValueLabel, "FPS"), "144 FPS");
        assert_eq!(text(Format::Colon, "FPS"), "FPS: 144");
        assert_eq!(text(Format::Brackets, "FPS"), "[ FPS: 144 ]");
        assert_eq!(text(Format::ValueOnly, "FPS"), "144");
        // No label: only the value, whatever the format.
        assert_eq!(text(Format::Brackets, ""), "144");
        let upper = Style {
            uppercase: true,
            ..Style::default()
        };
        assert_eq!(value_pieces(&upper, "Ping", "9 ms", None)[0].0, "PING");
    }

    #[test]
    fn colours_from_hue_and_the_ink_that_reads_on_them() {
        assert_eq!(hsv(0.0, 1.0, 1.0), 0xFF0000);
        assert_eq!(hsv(1.0 / 3.0, 1.0, 1.0), 0x00FF00);
        assert_eq!(hsv(2.0 / 3.0, 1.0, 1.0), 0x0000FF);
        // Hue wraps.
        assert_eq!(hsv(1.0, 1.0, 1.0), hsv(0.0, 1.0, 1.0));
        assert_eq!(hsv(0.5, 0.0, 1.0), 0xFFFFFF);
        assert_eq!(ink_on(0xFFFFFF), 0x0B0B0E);
        assert_eq!(ink_on(0x000000), 0xFFFFFF);
        assert_eq!(mix(0x000000, 0xFFFFFF, 0.5), 0x808080);
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
