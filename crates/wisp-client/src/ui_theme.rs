//! The design system's widgets drawn in the game (`crates/wisp-theme`:
//! palette, type scale, radii, control sizes, motion and the bundled fonts;
//! `docs/design-system.md`). Every menu is vanilla's (`ui`); what is left
//! here draws Wisp's own overlays that vanilla has no screen for: the
//! on-screen keyboard a controller types with (`osk`), plus the
//! `wisp.txt` preferences.
//!
//! Build a [`Ui`] over the frame's `HudBatch` with [`Ui::new`], lay out in dp
//! (the launcher's logical pixels) and give every interactive widget a stable
//! `id`. Clicks run the same layout with [`Ui::dry`] and read [`Ui::hit`]: one
//! function draws and hit-tests, so they never disagree.
use crate::hud::{HudAtlas, HudBatch, Rect};
use ab_glyph::{Font as _, FontRef, PxScale, ScaleFont as _};
use std::{cell::RefCell, collections::HashMap, path::Path};

/// A Wisp-only preference from `wisp.txt` beside `options.txt`
/// (`key:value` lines), so vanilla's options file stays exactly as vanilla
/// writes it.
pub fn preference(dir: &Path, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("wisp.txt")).ok()?;
    text.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        (k.trim() == key).then(|| v.trim().to_owned())
    })
}
/// Set (or with None remove) one preference, keeping the others.
pub fn set_preference(dir: &Path, key: &str, value: Option<&str>) {
    let text = std::fs::read_to_string(dir.join("wisp.txt")).unwrap_or_default();
    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| l.split_once(':').is_none_or(|(k, _)| k.trim() != key))
        .map(str::to_owned)
        .collect();
    if let Some(v) = value {
        lines.push(format!("{key}:{v}"));
    }
    let mut out = lines.join("\n");
    out.push('\n');
    let _ = std::fs::write(dir.join("wisp.txt"), out);
}

/// Wisp Christmas, the third Menu style (Wisp Settings): Wisp's own screens (the home screen,
/// the Wisp Menu) in a Christmas look. The game sets it each frame from the setting
/// ([`set`]); the screens read their accent from here and add the decorations: snow falling,
/// a string of fairy lights and a Santa hat on the feather.
pub mod festive {
    use super::{Ui, color::hex};
    use std::sync::atomic::{AtomicBool, Ordering};

    static ON: AtomicBool = AtomicBool::new(false);

    /// Christmas red, the accent while the look is on.
    pub const RED: u32 = 0xD42A35;
    const RED_HOVER: u32 = 0xE94C56;
    const WHITE: u32 = 0xFFFFFF;
    /// The lights' bulbs: red, gold, green, blue.
    const BULBS: [u32; 4] = [0xFF4D4D, 0xFFD24A, 0x3DDC84, 0x5AA9FF];

    pub fn set(on: bool) {
        ON.store(on, Ordering::Relaxed);
    }
    pub fn on() -> bool {
        ON.load(Ordering::Relaxed)
    }
    /// The accent's colour (no alpha): Wisp's near-white, or Christmas red.
    pub fn accent_rgb() -> u32 {
        if on() { RED } else { wisp_theme::glass::ACCENT }
    }
    /// The accent, the hovered accent and the ink on it, in the HUD's encoding.
    pub fn accent() -> u32 {
        hex(accent_rgb(), 255)
    }
    pub fn accent_hover() -> u32 {
        hex(if on() { RED_HOVER } else { 0xFFFFFF }, 255)
    }
    pub fn on_accent() -> u32 {
        hex(
            if on() {
                WHITE
            } else {
                wisp_theme::glass::ON_LIGHT
            },
            255,
        )
    }
    /// The accent at alpha `a`.
    pub fn accent_alpha(a: u8) -> u32 {
        hex(accent_rgb(), a)
    }

    /// Seconds since the first call, for the snow and the lights.
    fn clock() -> f32 {
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_secs_f32()
    }

    /// A number in 0..1 from `i` and `seed`, the same every frame.
    fn rand(i: u32, seed: u32) -> f32 {
        let mut x = i.wrapping_mul(0x9E37_79B9) ^ seed.wrapping_mul(0x85EB_CA6B);
        x ^= x >> 15;
        x = x.wrapping_mul(0x2C1B_3C6D);
        x ^= x >> 12;
        (x & 0xFFFF) as f32 / 65535.0
    }

    /// Snow falling over the rectangle: flakes of three sizes, the big ones nearer and faster,
    /// each swaying as it falls and wrapping round to the top.
    pub fn snow(ui: &mut Ui, (x, y, w, h): (f32, f32, f32, f32), alpha: f32) {
        let t = clock();
        let count = ((w * h) / 9000.0).clamp(30.0, 160.0) as u32;
        for i in 0..count {
            let depth = rand(i, 1);
            let r = 1.0 + depth * 2.2;
            let speed = 14.0 + depth * 34.0;
            let fx = x + rand(i, 2) * w + (t * 0.7 + rand(i, 3) * 6.3).sin() * (4.0 + depth * 8.0);
            let fy = y + (rand(i, 4) * h + t * speed).rem_euclid(h + 10.0) - 5.0;
            let a = ((0.35 + depth * 0.55) * alpha * 255.0) as u8;
            ui.circle(fx, fy, r, hex(0xFFFFFF, a));
        }
    }

    /// A string of fairy lights hanging across from `x` to `x + w`, its wire at `y`, sagging
    /// between pins; each bulb twinkles on its own.
    pub fn lights(ui: &mut Ui, x: f32, y: f32, w: f32) {
        let t = clock();
        let span = 70.0;
        let spans = (w / span).ceil().max(1.0) as u32;
        let step = w / spans as f32;
        let sag = 9.0;
        let wire_at = |px: f32| {
            let k = ((px - x) / step).fract();
            y + sag * 4.0 * k * (1.0 - k)
        };
        let mut wire = Vec::new();
        let mut px = x;
        while px <= x + w {
            wire.push([px, wire_at(px)]);
            px += 4.0;
        }
        wire.push([x + w, y]);
        ui.polyline(&wire, 1.4, hex(0x1F3A24, 230));
        let bulbs = spans * 3;
        for i in 0..bulbs {
            let bx = x + step * (i as f32 + 0.5) / 3.0;
            let by = wire_at(bx);
            let colour = BULBS[(i % 4) as usize];
            let pulse = 0.55 + 0.45 * (t * (1.3 + rand(i, 7)) + rand(i, 8) * 6.3).sin().abs();
            // The cap on the wire, the glow, then the bulb.
            ui.rounded(bx - 1.5, by - 1.0, 3.0, 3.0, 1.0, hex(0x2A2A2A, 255));
            ui.circle(bx, by + 5.0, 7.0, hex(colour, (60.0 * pulse) as u8));
            ui.circle(
                bx,
                by + 5.0,
                3.2,
                hex(colour, (150.0 + 105.0 * pulse) as u8),
            );
            ui.circle(
                bx - 0.9,
                by + 4.0,
                1.0,
                hex(0xFFFFFF, (140.0 * pulse) as u8),
            );
        }
    }

    /// A Santa hat sitting on a square tile `side` dp across at (`x`, `y`): red, bending to the
    /// right, a white fur brim along the tile's top and a pompom at the tip.
    pub fn hat(ui: &mut Ui, x: f32, y: f32, side: f32) {
        let k = side / 40.0;
        let red = hex(RED, 255);
        let shade = hex(0xA81E28, 255);
        let (l, r, base) = (x + 3.0 * k, x + side - 3.0 * k, y + 4.0 * k);
        let peak = [x + side * 0.62, y - 16.0 * k];
        let tip = [x + side + 5.0 * k, y - 6.0 * k];
        ui.quad([[l, base], peak, [r, base], [r, base]], red);
        ui.quad(
            [
                peak,
                tip,
                [r - 6.0 * k, base - 4.0 * k],
                [r - 6.0 * k, base - 4.0 * k],
            ],
            shade,
        );
        ui.rounded(
            x - 1.0 * k,
            y - 1.0 * k,
            side + 2.0 * k,
            8.0 * k,
            4.0 * k,
            hex(0xF4F4F4, 255),
        );
        ui.circle(tip[0], tip[1], 4.6 * k, hex(0xFFFFFF, 255));
    }
}

/// The dark palette (`wisp_theme::DARK`) in the HUD's colour encoding, plus the
/// translucent plates the overlays draw over the game.
pub mod color {
    use wisp_theme::DARK as P;
    pub const fn hex(c: u32, a: u8) -> u32 {
        (a as u32) << 24 | (c & 0xff) << 16 | (c >> 8 & 0xff) << 8 | (c >> 16 & 0xff)
    }
    /// The page, under every veil.
    pub const BG: u32 = hex(P.page, 255);
    /// Hover plates, secondary buttons, fields, the segmented track: the
    /// dark palette's soft plate, as ink over the veil so the plates read the
    /// same over any backdrop (over the page colour they are `plate_soft`).
    pub const RAISED: u32 = hex(P.ink, 20);
    /// Pressed plates and a switch that is off (`plate`, the same way).
    pub const HOVER: u32 = hex(P.ink, 36);
    /// One-pixel rules between rows, slider tracks.
    pub const BORDER: u32 = hex(P.rule, 255);
    /// Text and selection (`ink`).
    pub const TEXT: u32 = hex(P.ink, 255);
    /// Values and secondary text (`ink_soft`).
    pub const MUTED: u32 = hex(P.ink_soft, 255);
    pub const ACCENT: u32 = hex(P.accent, 255);
    pub const ON_ACCENT: u32 = hex(P.on_accent, 255);
    pub fn with_alpha(c: u32, a: f32) -> u32 {
        let base = (c >> 24) as f32;
        (c & 0x00ff_ffff) | ((base * a.clamp(0.0, 1.0)).round() as u32) << 24
    }
}

/// The launcher's type scale and shapes (`wisp_theme`), in dp.
pub(crate) const SMALL: f32 = wisp_theme::CAPTION.px;
pub(crate) const BODY: f32 = wisp_theme::LABEL.px;
/// The boot screen's wordmark (`DISPLAY`) and console lines (`MICRO`).
pub(crate) const DISPLAY: f32 = wisp_theme::DISPLAY.px;
pub(crate) const MICRO: f32 = wisp_theme::MICRO.px;
/// Fields; plates (hover rows, selection); pills (buttons, segments).
pub(crate) const RADIUS: f32 = wisp_theme::RADIUS;
pub(crate) const RADIUS_PLATE: f32 = wisp_theme::RADIUS_PLATE;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    Regular,
    Medium,
    /// The wordmark and page titles (the design system's 500).
    SemiBold,
    /// Counts, timestamps and the boot screen's console lines.
    Mono,
    /// Glass's headings: Bricolage Grotesque Heading SemiBold (the Wisp Menu's titles).
    Heading,
}
struct Fonts {
    faces: [FontRef<'static>; 5],
}
impl Fonts {
    fn load() -> Self {
        let f = |b: &'static [u8]| FontRef::try_from_slice(b).expect("bundled font");
        Self {
            faces: [
                f(wisp_theme::fonts::GEIST_REGULAR),
                f(wisp_theme::fonts::GEIST_MEDIUM),
                f(wisp_theme::fonts::GEIST_SEMIBOLD),
                f(wisp_theme::fonts::GEIST_MONO),
                f(wisp_theme::fonts::BRICOLAGE_HEADING_SEMIBOLD),
            ],
        }
    }
    fn face(&self, f: Face) -> &FontRef<'static> {
        &self.faces[f as usize]
    }
    /// The ab_glyph scale for an em size in pixels.
    fn scale(&self, f: Face, px: f32) -> PxScale {
        let font = self.face(f);
        let upem = font.units_per_em().unwrap_or(1000.0);
        PxScale::from(px * font.height_unscaled() / upem)
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Glyph(Face, u16, u16),
    /// A quarter disc (`false`) or a 1px quarter ring of this radius in pixels.
    Corner(u16, bool),
    /// A line icon at this size in pixels.
    Icon(Icon, u16),
}

/// The line icons the home screen and the Wisp Menu draw: 24-unit SVGs in the launcher's
/// style (`assets/wisp/icons`, some the launcher's own), rasterized smooth at the size drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Play,
    Globe,
    Settings,
    Friends,
    Language,
    Accessibility,
    Close,
    Hanger,
    ChevronRight,
    ChevronLeft,
    Hud,
    Mods,
    Cart,
    Spark,
    ShieldCheck,
    Folder,
    Check,
}
impl Icon {
    fn svg(self) -> &'static [u8] {
        macro_rules! icon {
            ($name:literal) => {
                include_bytes!(concat!("../assets/wisp/icons/", $name, ".svg"))
            };
        }
        match self {
            Self::Play => icon!("play"),
            Self::Globe => icon!("globe"),
            Self::Settings => icon!("settings"),
            Self::Friends => icon!("friends"),
            Self::Language => icon!("language"),
            Self::Accessibility => icon!("accessibility"),
            Self::Close => icon!("close"),
            Self::Hanger => icon!("hanger"),
            Self::ChevronRight => icon!("chevron-right"),
            Self::ChevronLeft => icon!("chevron-left"),
            Self::Hud => icon!("hud"),
            Self::Mods => icon!("mods"),
            Self::Cart => icon!("cart"),
            Self::Spark => icon!("spark"),
            Self::ShieldCheck => icon!("shield-check"),
            Self::Folder => icon!("folder"),
            Self::Check => icon!("check"),
        }
    }
    /// The icon's coverage, `px` pixels square, 0..1 per pixel.
    fn coverage(self, px: u32) -> Option<Vec<f32>> {
        let tree =
            resvg::usvg::Tree::from_data(self.svg(), &resvg::usvg::Options::default()).ok()?;
        let mut pixmap = resvg::tiny_skia::Pixmap::new(px, px)?;
        let k = px as f32 / tree.size().width();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(k, k),
            &mut pixmap.as_mut(),
        );
        Some(
            pixmap
                .pixels()
                .iter()
                .map(|p| f32::from(p.alpha()) / 255.0)
                .collect(),
        )
    }
}
#[derive(Clone, Copy)]
struct Cached {
    rect: Option<Rect>,
    /// Offset of the bitmap from the pen (glyphs) in pixels.
    left: f32,
    top: f32,
}
/// Glyphs and corner masks rasterized into the atlas region `wisp/ui-text`.
struct Cache {
    fonts: Fonts,
    entries: HashMap<Key, Cached>,
    shelf: (u32, u32, u32),
    pending: Vec<crate::font::Patch>,
    /// The atlas the entries were uploaded to (a new atlas starts over).
    atlas: usize,
}
thread_local! {
    static CACHE: RefCell<Option<Cache>> = const { RefCell::new(None) };
}
/// Reloaded atlases can occupy the same address with empty run-time slots.
pub fn reset_atlas() {
    CACHE.with(|cache| *cache.borrow_mut() = None);
}
fn with_cache<R>(atlas: &HudAtlas, f: impl FnOnce(&mut Cache) -> R) -> R {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let id = atlas as *const HudAtlas as usize;
        let cache = c.get_or_insert_with(|| Cache {
            fonts: Fonts::load(),
            entries: HashMap::new(),
            shelf: (0, 0, 0),
            pending: Vec::new(),
            atlas: id,
        });
        if cache.atlas != id {
            cache.atlas = id;
            cache.entries.clear();
            cache.shelf = (0, 0, 0);
            cache.pending.clear();
        }
        f(cache)
    })
}
impl Cache {
    fn region(atlas: &HudAtlas) -> Option<Rect> {
        atlas.sprites.get("wisp/ui-text").copied()
    }
    /// Reserve `w x h` pixels; on overflow forget everything and start over.
    fn place(&mut self, atlas: &HudAtlas, w: u32, h: u32) -> Option<(u32, u32, Rect)> {
        let region = Self::region(atlas)?;
        let (mut x, mut y, mut row) = self.shelf;
        if x + w + 1 > region.w {
            x = 0;
            y += row + 1;
            row = 0;
        }
        if y + h > region.h || w > region.w {
            self.entries.clear();
            self.shelf = (0, 0, 0);
            return None;
        }
        self.shelf = (x + w + 1, y, row.max(h));
        Some((x, y, region))
    }
    fn upload(&mut self, atlas: &HudAtlas, w: u32, h: u32, alpha: &[f32]) -> Option<Rect> {
        let (x, y, region) = self.place(atlas, w, h)?;
        let (ax, ay) = (
            (region.u0 * atlas.width as f32).round() as u32 + x,
            (region.v0 * atlas.height as f32).round() as u32 + y,
        );
        let rgba = alpha
            .iter()
            .flat_map(|a| [255, 255, 255, (a.clamp(0.0, 1.0) * 255.0).round() as u8])
            .collect();
        self.pending.push(crate::font::Patch {
            x: ax,
            y: ay,
            width: w,
            height: h,
            rgba,
        });
        let (aw, ah) = (atlas.width as f32, atlas.height as f32);
        Some(Rect {
            u0: ax as f32 / aw,
            v0: ay as f32 / ah,
            u1: (ax + w) as f32 / aw,
            v1: (ay + h) as f32 / ah,
            w,
            h,
        })
    }
    fn glyph(&mut self, atlas: &HudAtlas, face: Face, id: ab_glyph::GlyphId, px: f32) -> Cached {
        let size = (px * 4.0).round() as u16;
        let key = Key::Glyph(face, id.0, size);
        if let Some(c) = self.entries.get(&key) {
            return *c;
        }
        let scale = self.fonts.scale(face, f32::from(size) / 4.0);
        let glyph = id.with_scale_and_position(scale, ab_glyph::point(0.0, 0.0));
        let made = match self.fonts.face(face).outline_glyph(glyph) {
            Some(outline) => {
                let b = outline.px_bounds();
                let (w, h) = (b.width() as u32, b.height() as u32);
                let mut alpha = vec![0.0; (w * h) as usize];
                outline.draw(|x, y, c| {
                    if x < w && y < h {
                        alpha[(y * w + x) as usize] = c;
                    }
                });
                // Light text on dark backgrounds reads thin: a gentle boost.
                for a in &mut alpha {
                    *a = a.powf(0.85);
                }
                Cached {
                    rect: self.upload(atlas, w, h, &alpha),
                    left: b.min.x,
                    top: b.min.y,
                }
            }
            None => Cached {
                rect: None,
                left: 0.0,
                top: 0.0,
            },
        };
        if made.rect.is_some() || made.left == 0.0 {
            self.entries.insert(key, made);
        }
        made
    }
    fn icon(&mut self, atlas: &HudAtlas, icon: Icon, px: u16) -> Option<Rect> {
        let key = Key::Icon(icon, px);
        if let Some(c) = self.entries.get(&key) {
            return c.rect;
        }
        let alpha = icon.coverage(u32::from(px))?;
        let rect = self.upload(atlas, u32::from(px), u32::from(px), &alpha);
        if rect.is_some() {
            self.entries.insert(
                key,
                Cached {
                    rect,
                    left: 0.0,
                    top: 0.0,
                },
            );
        }
        rect
    }
    fn corner(&mut self, atlas: &HudAtlas, r: u16, ring: bool) -> Option<Rect> {
        let key = Key::Corner(r, ring);
        if let Some(c) = self.entries.get(&key) {
            return c.rect;
        }
        // The top-left quadrant of a circle centred at (r, r), 4x4 supersampled.
        let n = u32::from(r);
        let rf = f32::from(r);
        let mut alpha = vec![0.0; (n * n) as usize];
        for y in 0..n {
            for x in 0..n {
                let mut hits = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let px = x as f32 + (sx as f32 + 0.5) / 4.0;
                        let py = y as f32 + (sy as f32 + 0.5) / 4.0;
                        let d = ((px - rf).powi(2) + (py - rf).powi(2)).sqrt();
                        if d <= rf && (!ring || d >= rf - 1.0) {
                            hits += 1;
                        }
                    }
                }
                alpha[(y * n + x) as usize] = hits as f32 / 16.0;
            }
        }
        let rect = self.upload(atlas, n, n, &alpha);
        if rect.is_some() {
            self.entries.insert(
                key,
                Cached {
                    rect,
                    left: 0.0,
                    top: 0.0,
                },
            );
        }
        rect
    }
}

/// One frame of design-system drawing, in dp: the
/// launcher's logical pixels, scaled so the screen reads like the launcher's
/// 1040x700 window at any size.
pub struct Ui<'a> {
    pub atlas: &'a HudAtlas,
    pub batch: &'a mut HudBatch,
    /// The screen in dp.
    pub w: f32,
    pub h: f32,
    /// The cursor in dp.
    pub cursor: (f32, f32),
    /// Physical pixels per dp.
    pub density: f32,
    scale: f32,
}
impl<'a> Ui<'a> {
    /// `cursor` is in GUI pixels, like every screen's input.
    pub fn new(atlas: &'a HudAtlas, batch: &'a mut HudBatch, cursor: (f32, f32)) -> Self {
        let scale = batch.scale;
        let (pw, ph) = (batch.width * scale, batch.height * scale);
        let density = (pw / 1040.0).min(ph / 700.0).clamp(1.0, 4.0);
        Self {
            atlas,
            w: pw / density,
            h: ph / density,
            cursor: (cursor.0 * scale / density, cursor.1 * scale / density),
            density,
            scale,
            batch,
        }
    }
    /// A Ui at a chosen density (physical pixels per dp) instead of the window's: the HUD's
    /// widgets size with the GUI scale.
    pub fn with_density(
        atlas: &'a HudAtlas,
        batch: &'a mut HudBatch,
        cursor: (f32, f32),
        density: f32,
    ) -> Self {
        let scale = batch.scale;
        let (pw, ph) = (batch.width * scale, batch.height * scale);
        let density = density.max(0.25);
        Self {
            atlas,
            w: pw / density,
            h: ph / density,
            cursor: (cursor.0 * scale / density, cursor.1 * scale / density),
            density,
            scale,
            batch,
        }
    }
    /// A point in dp as GUI pixels (where items, sprites and input live).
    pub fn to_gui(&self, x: f32, y: f32) -> (f32, f32) {
        (
            (x * self.density).round() / self.scale,
            (y * self.density).round() / self.scale,
        )
    }
    /// A rectangle in dp as GUI pixels: `[x, y, w, h]`.
    pub fn gui_rect(&self, x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        let (x0, y0) = self.to_gui(x, y);
        let (x1, y1) = self.to_gui(x + w, y + h);
        [x0, y0, x1 - x0, y1 - y0]
    }
    /// GUI pixels per dp.
    pub fn gui_per_dp(&self) -> f32 {
        self.density / self.scale
    }
    /// A sprite of the GUI atlas stretched over the rectangle (dp); nothing when it isn't there.
    pub fn sprite(&mut self, name: &str, x: f32, y: f32, w: f32, h: f32) {
        let [gx, gy, gw, gh] = self.gui_rect(x, y, w, h);
        self.batch
            .sprite_scaled(self.atlas, name, gx, gy, gw, gh, u32::MAX);
    }
    /// Whether the GUI atlas has the sprite.
    pub fn has_sprite(&self, name: &str) -> bool {
        self.atlas.sprites.contains_key(name)
    }
    /// Where the next quad goes, to clip what follows with [`Ui::clip_since`].
    pub fn mark(&self) -> usize {
        self.batch.vertices.len()
    }
    /// Cut what was drawn since `start` to the rectangle (dp).
    pub fn clip_since(&mut self, start: usize, x: f32, y: f32, w: f32, h: f32) {
        let [gx, gy, gw, gh] = self.gui_rect(x, y, w, h);
        self.batch.clip_since(start, gx, gy, gx + gw, gy + gh);
    }
    /// A filled circle.
    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, color: u32) {
        self.rounded(cx - r, cy - r, 2.0 * r, 2.0 * r, r, color);
    }
    /// A filled quad, its corners in dp in order round it (a triangle repeats one).
    pub fn quad(&mut self, corners: [[f32; 2]; 4], color: u32) {
        if color >> 24 == 0 {
            return;
        }
        let k = self.density / self.scale;
        self.batch
            .fill_quad(self.atlas, corners.map(|[x, y]| [x * k, y * k]), color);
    }
    /// A line through `points` (dp), `width` dp across.
    pub fn polyline(&mut self, points: &[[f32; 2]], width: f32, color: u32) {
        for pair in points.windows(2) {
            let ([x0, y0], [x1, y1]) = (pair[0], pair[1]);
            let len = (x1 - x0).hypot(y1 - y0).max(1e-4);
            let (nx, ny) = (
                -(y1 - y0) / len * width / 2.0,
                (x1 - x0) / len * width / 2.0,
            );
            self.quad(
                [
                    [x0 + nx, y0 + ny],
                    [x1 + nx, y1 + ny],
                    [x1 - nx, y1 - ny],
                    [x0 - nx, y0 - ny],
                ],
                color,
            );
        }
    }
    /// Wisp's feather (`packaging/windows/draw-feather.ps1`'s shapes) centred on `(cx, cy)`,
    /// fitting a square `size` dp across: drawn in `ink`, its splits and shaft in `ground` (the
    /// colour behind it).
    pub fn feather(&mut self, cx: f32, cy: f32, size: f32, ink: u32, ground: u32) {
        let k = size / 990.0 * 1.02;
        let (s, c) = 40f32.to_radians().sin_cos();
        let at = |[x, y]: [f32; 2]| [cx + (x * c - y * s) * k, cy + (x * s + y * c) * k];
        // Cubic Béziers given as a start, then three points a curve, sampled `steps` times
        // each, and placed.
        let curve = |pts: &[[f32; 2]], steps: usize| -> Vec<[f32; 2]> {
            let mut out = vec![at(pts[0])];
            let mut p0 = pts[0];
            for &[p1, p2, p3] in pts[1..].as_chunks::<3>().0 {
                for i in 1..=steps {
                    let t = i as f32 / steps as f32;
                    let u = 1.0 - t;
                    let b = |a: f32, b: f32, cc: f32, d: f32| {
                        u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * cc + t * t * t * d
                    };
                    out.push(at([
                        b(p0[0], p1[0], p2[0], p3[0]),
                        b(p0[1], p1[1], p2[1], p3[1]),
                    ]));
                }
                p0 = p3;
            }
            out
        };
        const VANE: [[f32; 2]; 16] = [
            [0.0, -430.0],
            [58.0, -380.0],
            [96.0, -250.0],
            [98.0, -90.0],
            [100.0, 40.0],
            [72.0, 170.0],
            [18.0, 232.0],
            [4.0, 236.0],
            [-6.0, 236.0],
            [-12.0, 232.0],
            [-82.0, 200.0],
            [-128.0, 80.0],
            [-125.0, -60.0],
            [-121.0, -230.0],
            [-62.0, -372.0],
            [0.0, -430.0],
        ];
        let vane = curve(&VANE, 10);
        let n = vane.len() as f32;
        let centre = vane
            .iter()
            .fold([0.0, 0.0], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
        for pair in vane.windows(2) {
            self.quad([centre, pair[0], pair[1], pair[1]], ink);
        }
        // The splits: beyond the vane they fall on the ground and don't show.
        for [a, b, d] in [
            [[140.0, -215.0], [12.0, -128.0], [140.0, -178.0]],
            [[140.0, 0.0], [9.0, 82.0], [140.0, 30.0]],
            [[-150.0, -150.0], [-10.0, -52.0], [-150.0, -112.0]],
            [[-150.0, 70.0], [-8.0, 160.0], [-150.0, 102.0]],
        ] {
            self.quad([at(a), at(b), at(d), at(d)], ground);
        }
        let shaft = curve(
            &[[0.0, -410.0], [10.0, -150.0], [5.0, 120.0], [1.0, 234.0]],
            12,
        );
        self.polyline(&shaft, (11.0 * k).max(0.6), ground);
        let quill = curve(
            &[[1.0, 234.0], [0.0, 300.0], [-6.0, 360.0], [-14.0, 415.0]],
            8,
        );
        self.polyline(&quill, (16.0 * k).max(0.8), ink);
        for wisp in [
            [[0.0, 222.0], [-40.0, 216.0], [-72.0, 236.0], [-88.0, 268.0]],
            [[0.0, 246.0], [-30.0, 246.0], [-52.0, 266.0], [-60.0, 294.0]],
            [[2.0, 228.0], [36.0, 228.0], [58.0, 248.0], [68.0, 274.0]],
            [[1.0, 252.0], [26.0, 254.0], [40.0, 272.0], [46.0, 294.0]],
        ] {
            let line = curve(&wisp, 6);
            self.polyline(&line, (7.0 * k).max(0.5), ink);
        }
    }
    /// Move rasterized glyphs into the batch's atlas uploads (real frames only).
    pub fn finish(self) {
        let pending = with_cache(self.atlas, |c| std::mem::take(&mut c.pending));
        self.batch.patches.extend(pending);
    }

    // ---- geometry, snapped to physical pixels ----
    fn px_x(&self, v: f32) -> f32 {
        (v * self.density).round()
    }
    fn px_y(&self, v: f32) -> f32 {
        (v * self.density).round()
    }
    fn raw_fill(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: u32) {
        if x1 > x0 && y1 > y0 {
            let s = self.scale;
            self.batch.fill(
                self.atlas,
                x0 / s,
                y0 / s,
                (x1 - x0) / s,
                (y1 - y0) / s,
                color,
            );
        }
    }
    fn corner_quad(&mut self, rect: Rect, x: f32, y: f32, r: f32, flip: (bool, bool), color: u32) {
        let (u0, u1) = if flip.0 {
            (rect.u1, rect.u0)
        } else {
            (rect.u0, rect.u1)
        };
        let (v0, v1) = if flip.1 {
            (rect.v1, rect.v0)
        } else {
            (rect.v0, rect.v1)
        };
        let s = self.scale;
        let (gx, gy, gr) = (x / s, y / s, r / s);
        self.batch.quad(
            [[gx, gy], [gx + gr, gy], [gx + gr, gy + gr], [gx, gy + gr]],
            [[u0, v0], [u1, v0], [u1, v1], [u0, v1]],
            color,
        );
    }
    fn corners(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, ring: bool, color: u32) {
        if r < 1.0 {
            return;
        }
        let atlas = self.atlas;
        if let Some(rect) = with_cache(atlas, |c| c.corner(atlas, r as u16, ring)) {
            self.corner_quad(rect, x0, y0, r, (false, false), color);
            self.corner_quad(rect, x1 - r, y0, r, (true, false), color);
            self.corner_quad(rect, x1 - r, y1 - r, r, (true, true), color);
            self.corner_quad(rect, x0, y1 - r, r, (false, true), color);
        }
    }
    /// A filled rectangle with rounded corners.
    pub fn rounded(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, color: u32) {
        if color >> 24 == 0 {
            return;
        }
        let (x0, y0, x1, y1) = (
            self.px_x(x),
            self.px_y(y),
            self.px_x(x + w),
            self.px_y(y + h),
        );
        let r = (radius * self.density)
            .round()
            .min(((x1 - x0) / 2.0).floor())
            .min(((y1 - y0) / 2.0).floor())
            .max(0.0);
        if r < 1.0 {
            self.raw_fill(x0, y0, x1, y1, color);
            return;
        }
        self.raw_fill(x0 + r, y0, x1 - r, y1, color);
        self.raw_fill(x0, y0 + r, x0 + r, y1 - r, color);
        self.raw_fill(x1 - r, y0 + r, x1, y1 - r, color);
        self.corners(x0, y0, x1, y1, r, false, color);
    }
    /// A 1px rounded border.
    pub fn border(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, color: u32) {
        if color >> 24 == 0 {
            return;
        }
        let (x0, y0, x1, y1) = (
            self.px_x(x),
            self.px_y(y),
            self.px_x(x + w),
            self.px_y(y + h),
        );
        let r = (radius * self.density)
            .round()
            .min(((x1 - x0) / 2.0).floor())
            .min(((y1 - y0) / 2.0).floor())
            .max(0.0);
        self.raw_fill(x0 + r, y0, x1 - r, y0 + 1.0, color);
        self.raw_fill(x0 + r, y1 - 1.0, x1 - r, y1, color);
        self.raw_fill(x0, y0 + r, x0 + 1.0, y1 - r, color);
        self.raw_fill(x1 - 1.0, y0 + r, x1, y1 - r, color);
        self.corners(x0, y0, x1, y1, r, true, color);
    }

    // ---- text ----
    /// The advance of `text` in dp.
    pub fn measure(&self, face: Face, size: f32, text: &str) -> f32 {
        let px = (size * self.density * 4.0).round() / 4.0;
        with_cache(self.atlas, |c| {
            let font = c.fonts.face(face);
            let scaled = font.as_scaled(c.fonts.scale(face, px));
            let mut w = 0.0;
            let mut prev = None;
            for ch in text.chars() {
                let id = font.glyph_id(ch);
                if let Some(p) = prev {
                    w += scaled.kern(p, id);
                }
                w += scaled.h_advance(id);
                prev = Some(id);
            }
            w / self.density
        })
    }
    /// Draw a line of text in a `line`-tall box whose top is `y` (the
    /// launcher's line heights: about 1.45 × size for the interface);
    /// returns its width in dp.
    pub fn text_line(
        &mut self,
        face: Face,
        size: f32,
        line: f32,
        x: f32,
        y: f32,
        color: u32,
        text: &str,
    ) -> f32 {
        let px = (size * self.density * 4.0).round() / 4.0;
        let real = color >> 24 != 0;
        let (x0, y0) = (self.px_x(x), self.px_y(y));
        let box_px = line * self.density;
        let atlas = self.atlas;
        let density = self.density;
        let mut quads = Vec::new();
        let width = with_cache(atlas, |c| {
            let scale = c.fonts.scale(face, px);
            let (ascent, descent) = {
                let f = c.fonts.face(face).as_scaled(scale);
                (f.ascent(), f.descent())
            };
            let baseline = y0 + ((box_px - (ascent - descent)) / 2.0 + ascent).round();
            let mut pen = x0;
            let mut prev = None;
            for ch in text.chars() {
                let (id, advance, kern) = {
                    let font = c.fonts.face(face);
                    let id = font.glyph_id(ch);
                    let f = font.as_scaled(scale);
                    (id, f.h_advance(id), prev.map_or(0.0, |p| f.kern(p, id)))
                };
                pen += kern;
                if real && !ch.is_whitespace() {
                    let g = c.glyph(atlas, face, id, px);
                    if let Some(rect) = g.rect {
                        quads.push(((pen + g.left).round(), baseline + g.top, rect));
                    }
                }
                pen += advance;
                prev = Some(id);
            }
            (pen - x0) / density
        });
        let s = self.scale;
        for (gx, gy, rect) in quads {
            self.batch.rect_uv(
                gx / s,
                gy / s,
                rect.w as f32 / s,
                rect.h as f32 / s,
                rect,
                color,
            );
        }
        width
    }
    /// Text in a `1.45 × size` line box whose top is `y`.
    pub fn text(&mut self, face: Face, size: f32, x: f32, y: f32, color: u32, text: &str) -> f32 {
        self.text_line(face, size, (size * 1.45).round(), x, y, color, text)
    }
    pub fn text_centered(
        &mut self,
        face: Face,
        size: f32,
        cx: f32,
        y: f32,
        color: u32,
        text: &str,
    ) {
        let w = self.measure(face, size, text);
        self.text(face, size, cx - w / 2.0, y, color, text);
    }

    /// A line icon `size` dp square with its top left at `(x, y)`, smooth at any scale.
    pub fn icon(&mut self, icon: Icon, x: f32, y: f32, size: f32, color: u32) {
        if color >> 24 == 0 {
            return;
        }
        let px = (size * self.density).round().clamp(4.0, 256.0) as u16;
        let atlas = self.atlas;
        let Some(rect) = with_cache(atlas, |c| c.icon(atlas, icon, px)) else {
            return;
        };
        let s = self.scale;
        let (x0, y0) = (self.px_x(x), self.px_y(y));
        let side = f32::from(px) / s;
        self.batch.rect_uv(x0 / s, y0 / s, side, side, rect, color);
    }

    // ---- the launcher's components ----
}

// ---- the boot screen ----

/// The boot screen's palette on the pure black: a white wordmark and dimmed
/// console lines (the theme's ink is for light pages).
const BOOT_TEXT: u32 = color::hex(0xFFFFFF, 255);
const BOOT_LOG: u32 = color::hex(0x9BA1A6, 255);

/// The boot screen while the menu is not ready yet: the wordmark centred on
/// pure black and the console's last lines at the bottom right, as the
/// terminal prints them.
pub fn draw_boot(atlas: &HudAtlas, batch: &mut HudBatch, logs: &[String]) {
    // Pure black under everything: the frame's panorama (the title artwork
    // once the bundle is in) must not leak through.
    batch.fill(
        atlas,
        0.0,
        0.0,
        batch.width,
        batch.height,
        color::hex(0x000000, 255),
    );
    wordmark(atlas, batch);
    draw_boot_log(atlas, batch, logs);
}

/// The wordmark with its tagline, centred.
fn wordmark(atlas: &HudAtlas, batch: &mut HudBatch) {
    let mut ui = Ui::new(atlas, batch, (0.0, 0.0));
    let size = (ui.w / 16.0).clamp(DISPLAY, 96.0);
    let small = (size * 0.25).clamp(MICRO, 18.0);
    let mark = size * 1.45;
    let mark_width = mark * 54.0 / 64.0;
    let name = crate::lang::get_or("wisp.brand.name", wisp_theme::brand::NAME);
    let gap = size * 0.24;
    let width = mark_width + gap + ui.measure(Face::SemiBold, size, &name);
    let left = (ui.w - width) / 2.0;
    let block = mark + small * 0.6 + small * 1.5;
    let top = (ui.h - block) / 2.0;
    for (quad, rgb) in wisp_theme::brand::MARK {
        let corners = quad.map(|[x, y]| {
            [
                ui.px_x(left + (x - 5.0) / 64.0 * mark) / ui.scale,
                ui.px_y(top + y / 64.0 * mark) / ui.scale,
            ]
        });
        ui.batch.fill_quad(ui.atlas, corners, color::hex(rgb, 255));
    }
    ui.text(
        Face::SemiBold,
        size,
        left + mark_width + gap,
        top,
        BOOT_TEXT,
        &name,
    );
    ui.text_centered(
        Face::Mono,
        small,
        ui.w / 2.0,
        top + size * 1.45 + small * 0.6,
        BOOT_LOG,
        &crate::lang::get_or("wisp.brand.tagline", "Minecraft. In Rust."),
    );
    ui.finish();
}

/// The console's last lines at the bottom left, over any loading screen: long
/// lines wrap word-wise, the block grows upwards.
pub fn draw_boot_log(atlas: &HudAtlas, batch: &mut HudBatch, logs: &[String]) {
    if logs.is_empty() {
        return;
    }
    let mut ui = Ui::new(atlas, batch, (0.0, 0.0));
    let (size, line, pad) = (MICRO, MICRO * 1.5, 16.0);
    let max_w = ui.w / 2.0;
    let max_rows = (((ui.h - pad) / line) as usize).clamp(1, 24);
    let mut y = ui.h - pad - line;
    let mut drawn = 0;
    'lines: for text in logs.iter().rev() {
        let chunks = wrapping_lines(&ui, Face::Mono, size, text, max_w);
        for chunk in chunks.iter().rev() {
            if drawn == max_rows {
                break 'lines;
            }
            ui.text(Face::Mono, size, pad, y, BOOT_LOG, chunk);
            y -= line;
            drawn += 1;
        }
    }
    ui.finish();
}

/// The lines `text` wraps into at `max_w`, word-wise; a word wider than
/// `max_w` on its own breaks at the edge.
fn wrapping_lines(ui: &Ui, face: Face, size: f32, text: &str, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let mut word = word;
        loop {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if ui.measure(face, size, &candidate) <= max_w {
                line = candidate;
                break;
            }
            if line.is_empty() {
                // The word alone does not fit: hard-break it at the edge.
                let mut end = 0;
                for (i, ch) in word.char_indices() {
                    if i > 0 && ui.measure(face, size, &word[..i]) > max_w {
                        break;
                    }
                    end = i + ch.len_utf8();
                }
                lines.push(word[..end].to_owned());
                word = &word[end..];
            } else {
                lines.push(std::mem::take(&mut line));
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_reload_regenerates_text_and_corners_at_the_same_address() {
        let atlas = HudAtlas::build(Path::new("/nonexistent/wisp-test-assets")).unwrap();
        reset_atlas();
        let draw = |cache: &mut Cache| {
            let glyph = cache.fonts.face(Face::Regular).glyph_id('A');
            assert!(
                cache
                    .glyph(&atlas, Face::Regular, glyph, 16.0)
                    .rect
                    .is_some()
            );
            assert!(cache.corner(&atlas, 8, false).is_some());
            std::mem::take(&mut cache.pending)
        };
        let before = with_cache(&atlas, draw);
        assert_eq!(before.len(), 2);
        assert!(with_cache(&atlas, draw).is_empty());
        reset_atlas();
        let after = with_cache(&atlas, draw);
        assert_eq!(after.len(), 2);
        for (a, b) in before.iter().zip(&after) {
            assert_eq!((a.x, a.y, a.width, a.height), (b.x, b.y, b.width, b.height));
            assert_eq!(a.rgba, b.rgba);
        }
        reset_atlas();
    }

    #[test]
    fn the_boot_atlas_holds_the_run_time_glyph_region_without_a_pack() {
        let atlas = HudAtlas::boot();
        assert_eq!((atlas.width, atlas.height), (1024, 1024));
        assert_eq!(
            atlas.sprites["wisp/ui-text"].texels((atlas.width, atlas.height)),
            (1024, 512)
        );
        assert_eq!(
            atlas.sprites["wisp/white"].texels((atlas.width, atlas.height)),
            (4, 4)
        );
    }

    #[test]
    fn the_boot_screen_centres_the_wordmark_and_left_aligns_the_console() {
        reset_atlas();
        let atlas = HudAtlas::boot();
        let mut batch = HudBatch::new([1280, 720]);
        draw_boot(&atlas, &mut batch, &[]);
        // The first quad is the pure black backdrop over the frame's panorama.
        let (w, h) = (batch.width, batch.height);
        let bg = &batch.vertices[..6];
        assert!(bg.iter().all(|v| v.color == 0xFF00_0000));
        assert!(bg.iter().all(|v| v.pos[0] == 0.0 || v.pos[0] == w));
        assert!(bg.iter().all(|v| v.pos[1] == 0.0 || v.pos[1] == h));
        assert!(
            !batch.patches.is_empty(),
            "the glyphs are queued for the atlas"
        );

        let mut batch = HudBatch::new([1280, 720]);
        wordmark(&atlas, &mut batch);
        let (min, max) = batch
            .vertices
            .iter()
            .map(|v| v.pos[0])
            .fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
        assert!(
            min < batch.width / 2.0 && max > batch.width / 2.0,
            "the wordmark straddles the centre"
        );
        assert!(((min + max) / 2.0 - batch.width / 2.0).abs() < 1.0);

        let long = "[12:34:56] [Render thread/INFO]: Wisp: resources loaded in 1.0s; 1240 block states, 1500 items, 62 biomes; 8 worker threads".to_owned();
        let mut batch = HudBatch::new([1280, 720]);
        draw_boot_log(&atlas, &mut batch, &[long]);
        let (min, max) = batch
            .vertices
            .iter()
            .map(|v| v.pos[0])
            .fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
        assert!(min >= 4.0, "the line stays inside the left inset");
        assert!(
            max - min <= batch.width / 2.0 + 1.0,
            "a too wide line is cut to half the screen"
        );

        let mut batch = HudBatch::new([1280, 720]);
        let ui = Ui::new(&atlas, &mut batch, (0.0, 0.0));
        let lines = wrapping_lines(
            &ui,
            Face::Mono,
            MICRO,
            "the quick brown fox jumps over the lazy dog",
            90.0,
        );
        assert!(lines.len() > 1, "a long line wraps");
        assert!(
            lines
                .iter()
                .all(|l| ui.measure(Face::Mono, MICRO, l) <= 90.0),
            "every wrapped line fits"
        );
        assert_eq!(
            lines
                .iter()
                .map(|l| l.split_whitespace().count())
                .sum::<usize>(),
            9,
            "the wrap keeps every word"
        );

        let mut batch = HudBatch::new([1280, 720]);
        draw_boot_log(&atlas, &mut batch, &[]);
        assert!(batch.vertices.is_empty() && batch.patches.is_empty());
        reset_atlas();
    }
}
