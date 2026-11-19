//! Builds the launcher shop's items and their card art (`crates/wisp-shop/assets`).
//!
//! `cargo run -p wisp-cosmetics --example shop_items -- crates/wisp-shop/assets`
//!
//! Every item is checked by `Item::parse` and its card is drawn with `preview::render`, so a card
//! shows exactly what the game draws: the Wisp Founders Cape, and HD items at the format's
//! 256-texel textures (Demon Wings, Angel Wings, the Bamboo Sword and the Stray Head straw hat).
use base64::Engine;
use glam::Mat4;
use serde_json::{Value, json};
use std::path::Path;
use wisp_cosmetics::{
    Item,
    preview::{self, Draw},
};

type Rgba = [u8; 4];

const fn hex(c: u32) -> Rgba {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]
}

struct Canvas {
    w: u32,
    h: u32,
    px: Vec<u8>,
}

impl Canvas {
    fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            px: vec![0; (w * h * 4) as usize],
        }
    }
    fn set(&mut self, x: i32, y: i32, c: Rgba) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        self.px[i..i + 4].copy_from_slice(&c);
    }
    fn get(&self, x: i32, y: i32) -> Rgba {
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        [self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]]
    }
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }
    /// Alpha-blends `c` with `a` (0..1) over a pixel.
    fn blend(&mut self, x: i32, y: i32, c: [f32; 3], a: f32) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        let a = a.clamp(0.0, 1.0);
        for k in 0..3 {
            let v = self.px[i + k] as f32 * (1.0 - a) + c[k] * a;
            self.px[i + k] = v.round().clamp(0.0, 255.0) as u8;
        }
        self.px[i + 3] = self.px[i + 3].max((a * 255.0) as u8);
    }
    fn png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, self.w, self.h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::High);
        let mut writer = enc.write_header().unwrap();
        writer.write_image_data(&self.px).unwrap();
        drop(writer);
        out
    }
}

/// A small deterministic noise, so a rebuild gives the same pictures.
fn noise(x: i32, y: i32, seed: u32) -> f32 {
    let mut n = (x as u32).wrapping_mul(374_761_393)
        ^ (y as u32).wrapping_mul(668_265_263)
        ^ seed.wrapping_mul(2_246_822_519);
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    ((n ^ (n >> 16)) & 0xffff) as f32 / 65535.0
}

fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    let m = |i: usize| (a[i] as f32 * (1.0 - t) + b[i] as f32 * t).round() as u8;
    [m(0), m(1), m(2), 255]
}

/// Smooth value noise in 0..1: hashed lattice values, eased between.
fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let n = |dx: i32, dy: i32| noise(x0 as i32 + dx, y0 as i32 + dy, seed);
    let top = n(0, 0) + (n(1, 0) - n(0, 0)) * sx;
    let bottom = n(0, 1) + (n(1, 1) - n(0, 1)) * sx;
    top + (bottom - top) * sy
}

/// Three octaves of `vnoise`.
fn fbm(x: f32, y: f32, seed: u32) -> f32 {
    (vnoise(x, y, seed) * 0.57
        + vnoise(x * 2.0, y * 2.0, seed + 1) * 0.29
        + vnoise(x * 4.0, y * 4.0, seed + 2) * 0.14)
        .clamp(0.0, 1.0)
}

fn add_light(c: Rgba, light: [f32; 3], k: f32) -> Rgba {
    let a = |i: usize| (c[i] as f32 + light[i] * k).round().clamp(0.0, 255.0) as u8;
    [a(0), a(1), a(2), c[3]]
}

/// Minecraft's box UV for a `w`×`h`×`d` cube at `(u, v)`: (top, bottom, right, front, left, back)
/// as (x, y, w, h).
fn box_faces(u: i32, v: i32, w: i32, h: i32, d: i32) -> [(i32, i32, i32, i32); 6] {
    [
        (u + d, v, w, d),
        (u + d + w, v, w, d),
        (u, v + d, d, h),
        (u + d, v + d, w, h),
        (u + d + w, v + d, d, h),
        (u + 2 * d + w, v + d, w, h),
    ]
}

fn fill_box(c: &mut Canvas, u: i32, v: i32, w: i32, h: i32, d: i32, colour: Rgba) {
    for (x, y, fw, fh) in box_faces(u, v, w, h, d) {
        c.rect(x, y, fw, fh, colour);
    }
}

/// An 8-bit PNG as a canvas (RGB gets an opaque alpha).
fn png_canvas(bytes: &[u8]) -> Canvas {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(info.bit_depth, png::BitDepth::Eight, "an 8-bit PNG");
    let (w, h) = (info.width, info.height);
    let px = match info.color_type {
        png::ColorType::Rgba => buf[..(w * h * 4) as usize].to_vec(),
        png::ColorType::Rgb => buf[..(w * h * 3) as usize]
            .chunks(3)
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        other => panic!("unexpected colour type {other:?}"),
    };
    Canvas { w, h, px }
}

// ---------------------------------------------------------------- the Wisp Founders Cape

/// Texels per pixel of vanilla's cape: 12, the most the format's 256-texel frames hold.
const RES: i32 = 12;
/// The design was drawn at 8×: its sizes grow by this.
const K: f32 = RES as f32 / 8.0;
/// The cape at 12× Minecraft's: a 120×192×8 box at a twelfth of the scale, so it has the size
/// of vanilla's 10×16×1 cape (two thirds as thick) with 144 texels where vanilla has one. Its
/// unfolded faces take 256×200 texels, the format's widest.
const CAPE: (i32, i32, i32) = (10 * RES, 16 * RES, 8);
const CAPE_TEX: (u32, u32) = ((2 * (8 + 10 * RES)) as u32, (8 + 16 * RES) as u32);
/// Frames and the game ticks each shows: a 2.4 s loop at 20 frames a second (every tick).
const CAPE_FRAMES: u32 = 48;
const CAPE_FRAMETIME: u32 = 1;

/// Wisp's own mark (a white feather on black), as the launcher and the game show it.
const WISP_MARK: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../wisp-theme/icons/wisp-256.png"
));

/// The founders' gold: the hem and the sparks that cross it.
const GOLD: Rgba = hex(0xf2c14e);

/// The Wisp Founders Cape: Wisp's feather large on a black night, at 12×, animated: smoke
/// drifting behind, stars twinkling, a ring round the mark with a spark running along it, a
/// halo breathing round the feather, its edge glowing, a sweep of light crossing it, and a gold
/// hem with a glint running along it.
fn founders_frames() -> Vec<Canvas> {
    let src = png_canvas(WISP_MARK);
    // The mark: the white feather (the icon's ground and glow are far darker).
    let ink = |x: i32, y: i32| {
        let p = src.get(x, y);
        p[3] > 128 && p[0] > 115
    };
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, 0, 0);
    for y in 0..src.h as i32 {
        for x in 0..src.w as i32 {
            if ink(x, y) {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    assert!(x0 < x1 && y0 < y1, "the icon has a mark");
    let (w, h, d) = CAPE;
    // The feather 50 texels wide, a little above the middle; the ring round it.
    let mw = 50.0 * K;
    let mh = mw * (y1 - y0) as f32 / (x1 - x0) as f32;
    let centre_y = 54.0 * K;
    let (mx, my) = ((w as f32 - mw) / 2.0, centre_y - mh / 2.0);
    let (rcx, rcy, radius) = (w as f32 / 2.0, centre_y, (mh / 2.0 + 6.0 * K).min(36.0 * K));
    // How much of each texel the feather covers (4×4 samples), and its colour there.
    let mut cover = vec![0.0f32; (w * h) as usize];
    let mut tint = vec![[0.0f32; 3]; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let (mut n, mut col) = (0.0, [0.0; 3]);
            for sy in 0..4 {
                for sx in 0..4 {
                    let u = (x as f32 + (sx as f32 + 0.5) / 4.0 - mx) / mw;
                    let v = (y as f32 + (sy as f32 + 0.5) / 4.0 - my) / mh;
                    if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                        continue;
                    }
                    let px = x0 + (u * (x1 - x0) as f32) as i32;
                    let py = y0 + (v * (y1 - y0) as f32) as i32;
                    if ink(px, py) {
                        let p = src.get(px, py);
                        n += 1.0;
                        for k in 0..3 {
                            col[k] += p[k] as f32;
                        }
                    }
                }
            }
            let i = (y * w + x) as usize;
            cover[i] = n / 16.0;
            if n > 0.0 {
                tint[i] = col.map(|c| c / n);
            }
        }
    }
    let at = |field: &[f32], x: i32, y: i32| {
        if x < 0 || y < 0 || x >= w || y >= h {
            0.0
        } else {
            field[(y * w + x) as usize]
        }
    };
    // A soft halo: the coverage blurred.
    let r = RES;
    let mut halo = vec![0.0f32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let (mut s, mut n) = (0.0, 0.0);
            for dy in -r..=r {
                for dx in -r..=r {
                    let k = (-((dx * dx + dy * dy) as f32) / (24.0 * K * K)).exp();
                    s += at(&cover, x + dx, y + dy) * k;
                    n += k;
                }
            }
            halo[(y * w + x) as usize] = s / n;
        }
    }
    let edge = |x: i32, y: i32| {
        let c = at(&cover, x, y);
        let near = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .map(|(dx, dy)| at(&cover, x + dx, y + dy))
            .fold(0.0f32, |m, n| m.max((n - c).abs()));
        (near * 1.4).min(1.0)
    };
    let faces = box_faces(0, 0, w, h, d);
    let (fx, fy, _, _) = faces[3];
    let deep = hex(0x040405);
    let night = hex(0x16161b);
    let tau = std::f32::consts::TAU;
    (0..CAPE_FRAMES)
        .map(|k| {
            let t = k as f32 / CAPE_FRAMES as f32;
            let turn = t * tau;
            // The smoke is sampled round a circle, so the last frame meets the first.
            let (lx, ly) = (turn.cos() * 1.1, turn.sin() * 1.1);
            let breath = 0.5 + 0.5 * turn.sin();
            let mut c = Canvas::new(CAPE_TEX.0, CAPE_TEX.1);
            // The edges and the inner side: graphite, the inner side darker.
            for (x, y, fw, fh) in faces {
                c.rect(x, y, fw, fh, hex(0x26262b));
            }
            let (bx, by, _, _) = faces[5];
            c.rect(bx, by, w, h, hex(0x0b0b0d));
            for y in 0..h {
                for x in 0..w {
                    // Design units: the 20×32 of a 2× cape, so the smoke keeps its size.
                    let (u, v) = (x as f32 / (RES / 2) as f32, y as f32 / (RES / 2) as f32);
                    let i = (y * w + x) as usize;
                    let mut col = mix(night, deep, v / 32.0 * 0.9);
                    let haze = fbm(u * 0.2 + lx, v * 0.2 + ly, 131);
                    col = add_light(col, [92.0, 92.0, 104.0], (haze - 0.5).max(0.0) * 0.75);
                    // Stars, each on its own beat; none on the halo.
                    let star = RES / 4;
                    if noise(x / star, y / star, 133) > 0.982 && halo[i] < 0.1 {
                        let beat = (turn + noise(x / star, y / star, 134) * tau).sin();
                        col = mix(col, hex(0xf4f4f6), 0.25 + 0.6 * (0.5 + 0.5 * beat));
                    }
                    // The ring, and two sparks running round it once a loop.
                    let (dx, dy) = (x as f32 + 0.5 - rcx, y as f32 + 0.5 - rcy);
                    let off = (dx.hypot(dy) - radius).abs();
                    let ring = (-off * off / (1.2 * K * K)).exp();
                    col = add_light(col, [200.0, 200.0, 212.0], ring * 0.32);
                    let angle = dy.atan2(dx);
                    let spark = |lead: f32| {
                        let da = (angle - lead + tau * 1.5).rem_euclid(tau) - tau / 2.0;
                        (-da * da / 0.03).exp() * ring
                    };
                    col = add_light(
                        col,
                        [255.0, 240.0, 220.0],
                        spark(turn) * 1.3 + spark(turn + tau / 2.0) * 0.5,
                    );
                    col = add_light(col, [175.0, 175.0, 188.0], halo[i] * (0.55 + 0.6 * breath));
                    if cover[i] > 0.0 {
                        let m = tint[i];
                        let lifted = [
                            (m[0] * 1.12 + 22.0).min(255.0) as u8,
                            (m[1] * 1.1 + 18.0).min(255.0) as u8,
                            (m[2] * 1.06 + 16.0).min(255.0) as u8,
                            255,
                        ];
                        col = mix(col, lifted, cover[i]);
                        // A sweep of light crossing the feather once a loop.
                        let pos = (-60.0 + t * 280.0) * K;
                        let off = (x as f32 * 0.9 - y as f32 * 0.45 + 40.0 * K) - pos;
                        let sweep = (-off * off / (90.0 * K * K)).exp() * 0.7;
                        col = add_light(col, [255.0, 255.0, 255.0], sweep * cover[i]);
                    }
                    let e = edge(x, y);
                    if e > 0.0 {
                        col = add_light(col, [230.0, 230.0, 238.0], e * (0.3 + 0.3 * breath));
                    }
                    c.set(fx + x, fy + y, col);
                }
            }
            // A fine silver border; the gold hem with a glint running along it.
            let line = RES / 4;
            let hem = 3 * line;
            for x in 0..w {
                for k in 0..line {
                    c.set(fx + x, fy + k, hex(0xbdbdc2));
                }
                let glint = {
                    let pos = t * (w as f32 + 40.0 * K) - 20.0 * K;
                    let off = x as f32 - pos;
                    (-off * off / (30.0 * K * K)).exp()
                };
                for k in 0..hem {
                    let base = match k / line {
                        0 => hex(0x3a3a40),
                        1 => GOLD,
                        _ => mix(GOLD, hex(0x8a6a1e), 0.5),
                    };
                    c.set(
                        fx + x,
                        fy + h - hem + k,
                        add_light(base, [255.0, 240.0, 200.0], glint * 0.6),
                    );
                }
            }
            for y in 0..h - hem {
                for k in 0..line {
                    c.set(fx + k, fy + y, hex(0xbdbdc2));
                    c.set(fx + w - 1 - k, fy + y, hex(0xbdbdc2));
                }
            }
            c
        })
        .collect()
}

/// Frames stacked top to bottom: an animated texture's strip.
fn strip(frames: &[Canvas]) -> Canvas {
    let (w, h) = (frames[0].w, frames[0].h);
    let mut s = Canvas::new(w, h * frames.len() as u32);
    for (k, f) in frames.iter().enumerate() {
        let start = (k as u32 * h * w * 4) as usize;
        s.px[start..start + f.px.len()].copy_from_slice(&f.px);
    }
    s
}

fn cape_item(id: &str, frames: &[Canvas], frametime: u32, glow: bool) -> Value {
    let (w, h, d) = CAPE;
    json!({
        "format": 1,
        "id": id,
        "slot": "back",
        "bone": "body",
        // A night-sky cape stays bright in the dark.
        "glow": glow,
        // Behind the back, turned round like vanilla's cape, at a twelfth of the scale for its
        // 12× texture.
        "attach": { "offset": [0, 0, 2], "rotation": [0, 180, 0], "scale": 1.0 / RES as f32 },
        "texture": { "png": b64(&strip(frames).png()), "frames": frames.len(), "frametime": frametime },
        "parts": [{
            "name": "cape", "pivot": [0, 0, 0], "rotation": [-7, 0, 0],
            "cubes": [{ "origin": [-w / 2, 0, -d], "size": [w, h, d], "uv": [0, 0] }]
        }],
        "animations": [{ "part": "cape", "motion": "swing", "axis": "x", "amplitude": 3, "period": 70, "phase": 0 }]
    })
}

// ---------------------------------------------------------------- Lavender cape

/// Lavender's frames: 48 at two ticks each, a 4.8 s loop (the flowers sway slowly).
const LAVENDER_FRAMES: u32 = 48;
const LAVENDER_FRAMETIME: u32 = 2;

/// Lavender's purples, dark to light.
const LAVENDER: [u32; 6] = [0x4a2a78, 0x6b3fa0, 0x8f5fc4, 0xb57edc, 0xcfa4ec, 0xeedcfb];

/// The Lavender cape at 12×, animated: a green hill under a summer sky, rows of lavender running
/// over it towards the crest, tall lavender stalks in front swaying in the wind, clouds drifting
/// and a white butterfly fluttering across. Every motion turns once a loop, so it never jumps.
fn lavender_frames() -> Vec<Canvas> {
    let (w, h, d) = CAPE;
    let faces = box_faces(0, 0, w, h, d);
    let (fx, fy, _, _) = faces[3];
    let tau = std::f32::consts::TAU;
    let (wf, hf) = (w as f32, h as f32);
    // The hill's crest (texel rows, y down): a broad rounded top a little right of the middle.
    let crest = |x: f32| 74.0 * K / 1.5 + 0.0045 * (x - 0.62 * wf).powi(2) / K * 1.5;
    // Where the rows meet, above the crest.
    let (vx, vy) = (0.62 * wf, crest(0.62 * wf) - 26.0);
    // The front stalks: (base x, base y, height, phase, lean), back to front.
    let mut stalks: Vec<(f32, f32, f32, f32, f32)> = (0..30)
        .map(|i| {
            let n = |s: u32| noise(i, 0, s);
            let base_y = hf * (0.8 + 0.22 * n(301));
            (
                wf * (i as f32 + 0.5) / 30.0 + (n(302) - 0.5) * 6.0,
                base_y,
                (38.0 + 30.0 * n(303)) * (base_y / hf),
                n(304) * 0.6,
                (n(305) - 0.5) * 6.0,
            )
        })
        .collect();
    stalks.sort_by(|a, b| a.1.total_cmp(&b.1));
    (0..LAVENDER_FRAMES)
        .map(|k| {
            let t = k as f32 / LAVENDER_FRAMES as f32;
            let turn = t * tau;
            let mut c = Canvas::new(CAPE_TEX.0, CAPE_TEX.1);
            // The edges a deep green, the inner side darker.
            for (x, y, fw, fh) in faces {
                c.rect(x, y, fw, fh, hex(0x2c4a22));
            }
            let (bx, by, _, _) = faces[5];
            c.rect(bx, by, w, h, hex(0x1c3016));
            // The wind: a gust running over the field once a loop.
            let wind = |x: f32, y: f32| (turn - x * 0.03 - y * 0.012).sin();
            for y in 0..h {
                for x in 0..w {
                    let (xf, yf) = (x as f32 + 0.5, y as f32 + 0.5);
                    let top = crest(xf);
                    let col = if yf < top {
                        // The sky: deep blue up high to a warm haze at the horizon, a sun's glow
                        // to the right, clouds drifting (sampled round a circle so they loop).
                        let v = yf / top;
                        let mut col = mix(hex(0x5b9be0), hex(0xf6dcc0), v.powf(1.6));
                        let (sx, sy) = (xf - 0.8 * wf, yf - 0.22 * hf);
                        let sun = (-(sx * sx + sy * sy) / (110.0 * K * K)).exp();
                        col = add_light(col, [255.0, 240.0, 200.0], sun * 0.75);
                        let (lx, ly) = (turn.cos() * 0.9, turn.sin() * 0.25);
                        let cloud = fbm(xf / (30.0 * K) + lx, yf / (12.0 * K) + ly, 311);
                        let cloud = ((cloud - 0.55) * 4.0).clamp(0.0, 1.0) * (1.0 - v * 0.6);
                        mix(col, hex(0xffffff), cloud * 0.85)
                    } else {
                        // The hill: rows of lavender converging on the crest, green between.
                        let depth = ((yf - top) / (hf - top)).clamp(0.0, 1.0);
                        let across = (xf - vx) / (yf - vy).max(1.0);
                        let rows = across * 9.0;
                        let f = rows - rows.floor();
                        // Grass and lavender alike lighter far off, in the haze.
                        let haze = (1.0 - depth).powi(3) * 0.45;
                        let sway = wind(xf, yf) * 0.08 * depth;
                        let bush = (f - 0.5 + sway).abs() < 0.2 && depth > 0.06;
                        let bump = vnoise(xf / (2.0 + 5.0 * depth), yf / (1.5 + 4.0 * depth), 313);
                        let mut col = if bush {
                            let edge = 1.0 - (f - 0.5 + sway).abs() / 0.2;
                            ramp(&LAVENDER, 0.2 + edge * 0.45 + bump * 0.35)
                        } else {
                            let g = 0.3 + bump * 0.4 + depth * 0.1;
                            ramp(&[0x2f5a1e, 0x3f7426, 0x55912f, 0x6fae3c, 0x8cc552], g)
                        };
                        // The hill's own light: the crest lit, the slope away shaded.
                        col = add_light(col, [40.0, 40.0, 10.0], (1.0 - depth) * 0.6);
                        mix(col, hex(0xe8e0f0), haze)
                    };
                    c.set(fx + x, fy + y, col);
                }
            }
            // The front stalks: a green stem bending with the wind, a spike of lavender buds
            // on its top third, each bud lit from the upper left.
            for &(x0, y0, len, phase, lean) in &stalks {
                let sway = (wind(x0, y0) * 0.75 + (turn * 2.0 + phase * tau).sin() * 0.25) * 7.0;
                let at = |s: f32| [x0 + (lean + sway) * s * s, y0 - len * s];
                let steps = (len * 2.0) as i32;
                for i in 0..steps {
                    let s = i as f32 / steps as f32;
                    let [px, py] = at(s);
                    if s < 0.62 {
                        c.set(fx + px as i32 - 1, fy + py as i32, hex(0x6c9a4a));
                        c.set(fx + px as i32, fy + py as i32, hex(0x4f7a3a));
                        c.set(fx + px as i32 + 1, fy + py as i32, hex(0x37592a));
                    }
                }
                let buds = 9;
                for b in 0..buds {
                    let s = 0.6 + 0.4 * b as f32 / buds as f32;
                    let [px, py] = at(s);
                    let r = 5.0 * (1.0 - 0.45 * b as f32 / buds as f32) * (y0 / hf);
                    let side = if b % 2 == 0 { -1.0 } else { 1.0 };
                    let cx = px + side * r * 0.45;
                    for dy in -7..=7 {
                        for dx in -6..=6 {
                            let (ex, ey) = (dx as f32 / r, dy as f32 / (r * 1.25));
                            let dd = ex * ex + ey * ey;
                            if dd > 1.0 {
                                continue;
                            }
                            let light = 0.75 - (ex + ey) * 0.25 - dd * 0.3;
                            let (xx, yy) = (cx as i32 + dx, py as i32 + dy);
                            if xx >= 0 && yy >= 0 && xx < w && yy < h {
                                c.set(fx + xx, fy + yy, ramp(&LAVENDER, light));
                            }
                        }
                    }
                }
            }
            // A white butterfly fluttering across and back once a loop, its wings beating.
            let (bx, by) = (
                wf * (0.5 + 0.34 * turn.sin()),
                hf * (0.5 + 0.06 * (turn * 2.0).sin()),
            );
            let open = ((turn * 12.0).sin() * 0.5 + 0.5) * 3.5 * K / 1.5 + 1.0;
            for dy in -10..=10 {
                for dx in -12..=12 {
                    let (ex, ey) = (dx as f32 / 2.0, dy as f32 / 2.0);
                    let wing =
                        ex.abs() <= open && ex.abs() >= 0.5 && ey.abs() <= 3.0 + ex.abs() * 0.4;
                    if wing {
                        let col = if ey.abs() > 2.0 + ex.abs() * 0.35 {
                            hex(0xd8d0e8)
                        } else {
                            hex(0xfdfbff)
                        };
                        c.set(fx + bx as i32 + dx, fy + by as i32 + dy, col);
                    }
                }
            }
            for dy in -4..=4 {
                c.set(fx + bx as i32, fy + by as i32 + dy, hex(0x2a2030));
                c.set(fx + bx as i32 + 1, fy + by as i32 + dy, hex(0x2a2030));
            }
            // A fine lavender border and a deep-purple hem.
            let line = RES / 4;
            for x in 0..w {
                for k in 0..line {
                    c.set(fx + x, fy + k, ramp(&LAVENDER, 0.7));
                    c.set(fx + x, fy + h - 1 - k, ramp(&LAVENDER, 0.1));
                    c.set(fx + x, fy + h - 1 - line - k, ramp(&LAVENDER, 0.45));
                }
            }
            for y in 0..h {
                for k in 0..line {
                    c.set(fx + k, fy + y, ramp(&LAVENDER, 0.7));
                    c.set(fx + w - 1 - k, fy + y, ramp(&LAVENDER, 0.7));
                }
            }
            c
        })
        .collect()
}

// ---------------------------------------------------------------- Minecraft-style pieces

/// Whether `p` lies inside the polygon `poly` (even-odd rule).
fn inside(poly: &[[f32; 2]], p: [f32; 2]) -> bool {
    let mut hit = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            hit = !hit;
        }
        j = i;
    }
    hit
}

/// The distance from `p` to the segment `a`-`b`.
fn to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

fn lerp2(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// Each of the six faces of a box at (`u`, `v`) painted texel by texel: `paint(face, i, j, fw, fh)`
/// with the faces in `box_faces`' order (top, bottom, right, front, left, back).
#[allow(clippy::too_many_arguments)]
fn paint_box(
    c: &mut Canvas,
    u: i32,
    v: i32,
    w: i32,
    h: i32,
    d: i32,
    mut paint: impl FnMut(usize, i32, i32, i32, i32) -> Rgba,
) {
    for (face, (x, y, fw, fh)) in box_faces(u, v, w, h, d).into_iter().enumerate() {
        for j in 0..fh {
            for i in 0..fw {
                c.set(x + i, y + j, paint(face, i, j, fw, fh));
            }
        }
    }
}

/// A box's JSON.
fn cube(origin: [f32; 3], size: [i32; 3], uv: [i32; 2]) -> Value {
    json!({ "origin": origin, "size": size, "uv": uv })
}

/// A quad's JSON: corners (model pixels) with their UVs, in the order given.
fn quad(points: [[f32; 3]; 4], uvs: [[f32; 2]; 4]) -> Value {
    let vertices: Vec<Value> = points
        .iter()
        .zip(uvs)
        .map(|(p, uv)| json!([p[0], p[1], p[2], uv[0], uv[1]]))
        .collect();
    json!({ "vertices": vertices })
}

// ---------------------------------------------------------------- wings

/// Units per model pixel for the HD items: their boxes are built this many times Minecraft's size
/// and drawn at that fraction of the scale, so a 256-texel texture covers them 8 texels a pixel.
const HD: i32 = 8;
const HDF: f32 = HD as f32;

/// A box on the `side` (1 the wearer's left, +x; -1 the right) wing: a left-wing box mirrored in x
/// for the right, texture and all.
fn side_cube(side: f32, origin: [f32; 3], size: [f32; 3], uv: [i32; 2]) -> Value {
    if side > 0.0 {
        json!({ "origin": origin, "size": size, "uv": uv })
    } else {
        json!({
            "origin": [-origin[0] - size[0], origin[1], origin[2]],
            "size": size, "uv": uv, "mirror": true,
        })
    }
}

/// A point `len` along a direction `deg` degrees below +x (y is down), from `from`.
fn ray(from: [f32; 2], deg: f32, len: f32) -> [f32; 2] {
    let a = deg.to_radians();
    [from[0] + a.cos() * len, from[1] + a.sin() * len]
}

/// A scalloped edge from `a` to `b`: a curve bowed `depth` towards `towards`, as points.
fn scallop(a: [f32; 2], b: [f32; 2], towards: [f32; 2], depth: f32) -> Vec<[f32; 2]> {
    let mid = lerp2(a, b, 0.5);
    let (dx, dy) = (towards[0] - mid[0], towards[1] - mid[1]);
    let l = dx.hypot(dy).max(0.001);
    let ctrl = [mid[0] + dx / l * depth * 2.0, mid[1] + dy / l * depth * 2.0];
    (0..=12)
        .map(|i| {
            let t = i as f32 / 12.0;
            let p = lerp2(lerp2(a, ctrl, t), lerp2(ctrl, b, t), t);
            [p[0], p[1]]
        })
        .collect()
}

/// A flat skin stretched over a polygon in a part's plane (z 0): the polygon's bounding box as one
/// quad each way round, its texels at `at` in the texture (one a unit), so the texture's cut-out
/// gives the shape. Returns the quads and the box (x0, y0, w, h).
fn skin_quads(side: f32, poly: &[[f32; 2]], at: [i32; 2], tex: [f32; 2]) -> (Vec<Value>, [i32; 4]) {
    let x0 = poly.iter().map(|p| p[0]).fold(f32::MAX, f32::min).floor();
    let y0 = poly.iter().map(|p| p[1]).fold(f32::MAX, f32::min).floor();
    let x1 = poly.iter().map(|p| p[0]).fold(f32::MIN, f32::max).ceil();
    let y1 = poly.iter().map(|p| p[1]).fold(f32::MIN, f32::max).ceil();
    let p = |x: f32, y: f32| [side * x, y, 0.0];
    let uv = |x: f32, y: f32| {
        [
            (at[0] as f32 + x - x0) / tex[0],
            (at[1] as f32 + y - y0) / tex[1],
        ]
    };
    let pts = [p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)];
    let uvs = [uv(x0, y0), uv(x1, y0), uv(x1, y1), uv(x0, y1)];
    let quads = vec![
        quad(pts, uvs),
        quad(
            [pts[3], pts[2], pts[1], pts[0]],
            [uvs[3], uvs[2], uvs[1], uvs[0]],
        ),
    ];
    (
        quads,
        [x0 as i32, y0 as i32, (x1 - x0) as i32, (y1 - y0) as i32],
    )
}

/// Paints a skin's texels: `paint(point)` for each texel of the box at `at`, transparent outside
/// the polygon, with a dark one-texel outline round it.
fn paint_skin(
    c: &mut Canvas,
    poly: &[[f32; 2]],
    at: [i32; 2],
    bbox: [i32; 4],
    edge: Rgba,
    mut paint: impl FnMut([f32; 2]) -> Rgba,
) {
    let [x0, y0, w, h] = bbox;
    for j in 0..h {
        for i in 0..w {
            let p = [(x0 + i) as f32 + 0.5, (y0 + j) as f32 + 0.5];
            if !inside(poly, p) {
                continue;
            }
            let col = if to_edge(poly, p) < 1.6 {
                edge
            } else {
                paint(p)
            };
            c.set(at[0] + i, at[1] + j, col);
        }
    }
}

/// The distance from `p` to the polygon's edge.
fn to_edge(poly: &[[f32; 2]], p: [f32; 2]) -> f32 {
    (0..poly.len())
        .map(|i| to_segment(p, poly[i], poly[(i + 1) % poly.len()]))
        .fold(f32::MAX, f32::min)
}

/// The distance from `p` to a polyline.
fn to_line(line: &[[f32; 2]], p: [f32; 2]) -> f32 {
    line.windows(2)
        .map(|s| to_segment(p, s[0], s[1]))
        .fold(f32::MAX, f32::min)
}

/// A colour from a ramp of shades by `t` in 0..1, stepped (pixel art's few tones, no smooth blend).
fn ramp(shades: &[u32], t: f32) -> Rgba {
    let i = (t.clamp(0.0, 0.999) * shades.len() as f32) as usize;
    hex(shades[i])
}

/// A round bone along x (box UV, `w` long, `t` thick) painted from a ramp: lit on top, a bright
/// line along it, darker underneath, rings every so often, its ends lighter.
fn paint_bone(c: &mut Canvas, uv: [i32; 2], w: i32, t: i32, shades: &[u32]) {
    paint_box(c, uv[0], uv[1], w, t, t, |face, i, j, fw, fh| {
        let n = noise(i / 2, j / 2, 41) * 0.08;
        match face {
            0 => ramp(shades, 0.78 + n),
            1 => ramp(shades, 0.12 + n),
            2 | 4 => ramp(shades, 0.55 + n),
            _ => {
                let across = (j as f32 + 0.5) / fh as f32;
                let round = (across * std::f32::consts::PI).sin();
                let mut l = 0.18 + 0.62 * round - 0.35 * across;
                if (i % 26) < 2 && i > 4 && i < fw - 4 {
                    l -= 0.25;
                }
                if (across - 0.3).abs() < 0.09 {
                    l += 0.22;
                }
                ramp(shades, l + n)
            }
        }
    });
}

/// A horn-like claw (box UV, pointing along -y): pale at the root, dark at the point.
fn paint_claw(c: &mut Canvas, uv: [i32; 2], size: [i32; 3]) {
    let [w, h, d] = size;
    paint_box(c, uv[0], uv[1], w, h, d, |face, i, j, fw, fh| {
        let along = 1.0 - (j as f32 + 0.5) / fh as f32;
        let across = (i as f32 + 0.5) / fw as f32;
        let side = if face == 3 || face == 5 { 0.15 } else { 0.0 };
        ramp(
            &[0x2a2018, 0x5e4c36, 0x9c8a68, 0xcfc1a0, 0xece2c6],
            0.95 - along * 0.75 + side - across * 0.15,
        )
    });
}

/// The Demon Wings' layout: the wrist, how far the hand reaches, each finger's angle below the
/// hand (degrees) and length, all in HD units in the outer part's frame.
const DEMON_ARM: f32 = 88.0;
const DEMON_HAND: f32 = 78.0;
const DEMON_FINGERS: [(f32, f32); 3] = [(70.0, 88.0), (42.0, 96.0), (16.0, 84.0)];
/// The outer part's rest bend (degrees about z, down-outward from the arm).
const DEMON_BEND: f32 = 30.0;
/// Where the skins, bones and claws sit in the texture.
const DEMON_INNER_AT: [i32; 2] = [0, 0];
const DEMON_OUTER_AT: [i32; 2] = [100, 0];
const DEMON_BONE_AT: [i32; 2] = [0, 120];
const DEMON_KNUCKLE_AT: [i32; 2] = [0, 148];
const DEMON_CLAW_AT: [i32; 2] = [64, 148];
const DEMON_TIP_AT: [i32; 2] = [96, 148];
const DEMON_TEX: (u32, u32) = (256, 184);

/// The outer skin's outline (outer frame): the hand to its tip, then the scalloped edge from
/// fingertip to fingertip back to the innermost finger, and along it to the wrist.
fn demon_outer_poly() -> (Vec<[f32; 2]>, Vec<Vec<[f32; 2]>>) {
    let wrist = [0.0, 0.0];
    let tips: Vec<[f32; 2]> = DEMON_FINGERS
        .iter()
        .rev()
        .map(|&(a, l)| ray(wrist, a, l))
        .collect();
    let hand = [DEMON_HAND, 3.0];
    let mut poly = vec![[0.0, -2.0], [DEMON_HAND - 4.0, -2.0], hand];
    let mut edges = Vec::new();
    let mut from = hand;
    for &tip in &tips {
        let s = scallop(from, tip, wrist, 7.0);
        poly.extend_from_slice(&s[1..]);
        edges.push(s);
        from = tip;
    }
    (poly, edges)
}

/// The inner skin's outline (root frame): along the arm to the wrist, down the innermost finger as
/// it rests, a scalloped edge back to the body.
fn demon_inner_poly() -> (Vec<[f32; 2]>, Vec<Vec<[f32; 2]>>) {
    let wrist = [DEMON_ARM, 0.0];
    let (a, l) = DEMON_FINGERS[0];
    let tip = ray(wrist, a + DEMON_BEND, l);
    let body = [6.0, 70.0];
    let mut poly = vec![[6.0, -2.0], [DEMON_ARM, -2.0], tip];
    let mid = [40.0, 92.0];
    let s1 = scallop(tip, mid, [50.0, 10.0], 6.0);
    let s2 = scallop(mid, body, [50.0, 10.0], 5.0);
    poly.extend_from_slice(&s1[1..]);
    poly.extend_from_slice(&s2[1..]);
    (poly, vec![s1, s2])
}

/// Demon Wings' texture: two leathery skins, dark by the bones and glowing ember-red where they
/// thin towards the scalloped edge, veined from the wrist; black bones; ivory claws.
fn demon_texture() -> Canvas {
    let mut c = Canvas::new(DEMON_TEX.0, DEMON_TEX.1);
    let tex = [DEMON_TEX.0 as f32, DEMON_TEX.1 as f32];
    let skin = [
        0x14030a, 0x2b0710, 0x470b16, 0x6a1019, 0x8f1a1f, 0xb82a24, 0xe2482c, 0xff8a3c,
    ];
    let paint = |c: &mut Canvas,
                 poly: &[[f32; 2]],
                 edges: &[Vec<[f32; 2]>],
                 at: [i32; 2],
                 wrist: [f32; 2],
                 veins: &[[f32; 2]]| {
        let (_, bbox) = skin_quads(1.0, poly, at, tex);
        paint_skin(c, poly, at, bbox, hex(0x0c0205), |p| {
            // Chunky shading: worked out per 2×2 texels, like a 4-texel-a-pixel painting.
            let q = [
                (p[0] / 2.0).floor() * 2.0 + 1.0,
                (p[1] / 2.0).floor() * 2.0 + 1.0,
            ];
            let free = edges.iter().map(|e| to_line(e, q)).fold(f32::MAX, f32::min);
            let from_wrist = (q[0] - wrist[0]).hypot(q[1] - wrist[1]);
            let mut t =
                0.22 + (from_wrist / 110.0) * 0.35 + fbm(q[0] / 18.0, q[1] / 18.0, 61) * 0.18;
            t += (1.0 - free / 14.0).max(0.0) * 0.45;
            let vein = veins
                .iter()
                .map(|&v| to_segment(p, wrist, v))
                .fold(f32::MAX, f32::min);
            if vein < 1.3 {
                t -= 0.28;
            } else if vein < 2.6 {
                t += 0.07;
            }
            if free < 2.8 {
                t = 0.97;
            }
            ramp(&skin, t)
        });
    };
    let (poly, edges) = demon_outer_poly();
    let wrist = [0.0, 0.0];
    let veins: Vec<[f32; 2]> = edges.iter().map(|e| e[6]).collect();
    paint(&mut c, &poly, &edges, DEMON_OUTER_AT, wrist, &veins);
    let (poly, edges) = demon_inner_poly();
    let wrist = [DEMON_ARM, 0.0];
    let veins: Vec<[f32; 2]> = edges.iter().map(|e| e[6]).collect();
    paint(&mut c, &poly, &edges, DEMON_INNER_AT, wrist, &veins);
    let bone = [
        0x07050a, 0x120d14, 0x1e1620, 0x2c2230, 0x3c3040, 0x544458, 0x74627a,
    ];
    paint_bone(&mut c, DEMON_BONE_AT, 112, 12, &bone);
    paint_box(
        &mut c,
        DEMON_KNUCKLE_AT[0],
        DEMON_KNUCKLE_AT[1],
        16,
        16,
        16,
        |face, i, j, _, _| {
            let l = [0.7, 0.1, 0.45, 0.5, 0.45, 0.35][face] + noise(i / 2, j / 2, 43) * 0.12;
            ramp(&bone, l)
        },
    );
    paint_claw(&mut c, DEMON_CLAW_AT, [6, 18, 6]);
    paint_claw(&mut c, DEMON_TIP_AT, [6, 14, 6]);
    c
}

/// Demon Wings: each a bat's wing of solid bones (arm, hand, three fingers) with a leathery skin
/// stretched between them, a claw at the wrist and the hand's tip.
fn demon_item(texture: &Canvas) -> Value {
    let tex = [texture.w as f32, texture.h as f32];
    let wing = |name: &str, s: f32| -> Value {
        let (outer, _) = demon_outer_poly();
        let (inner, _) = demon_inner_poly();
        let (outer_quads, _) = skin_quads(s, &outer, DEMON_OUTER_AT, tex);
        let (inner_quads, _) = skin_quads(s, &inner, DEMON_INNER_AT, tex);
        let fingers: Vec<Value> = DEMON_FINGERS
            .iter()
            .enumerate()
            .map(|(i, &(a, l))| {
                json!({
                    "name": format!("{name}_finger{}", i + 1), "pivot": [0, 0, 0],
                    "rotation": [0.0, 0.0, s * a],
                    "cubes": [side_cube(s, [0.0, -4.0, -4.0], [l, 8.0, 8.0], DEMON_BONE_AT)],
                })
            })
            .collect();
        json!({
            "name": name, "pivot": [s * 10.0, 16.0, 20.0], "rotation": [0.0, s * -40.0, s * -24.0],
            "cubes": [
                side_cube(s, [0.0, -6.0, -6.0], [DEMON_ARM, 12.0, 12.0], DEMON_BONE_AT),
                side_cube(s, [DEMON_ARM - 8.0, -8.0, -8.0], [16.0, 16.0, 16.0], DEMON_KNUCKLE_AT),
                side_cube(s, [DEMON_ARM - 3.0, -24.0, -3.0], [6.0, 18.0, 6.0], DEMON_CLAW_AT),
            ],
            "quads": inner_quads,
            "children": [{
                "name": format!("{name}_tip"), "pivot": [s * DEMON_ARM, 0.0, 0.0],
                "rotation": [0.0, s * -10.0, s * DEMON_BEND],
                "cubes": [
                    side_cube(s, [0.0, -5.0, -5.0], [DEMON_HAND, 10.0, 10.0], DEMON_BONE_AT),
                    side_cube(s, [DEMON_HAND - 3.0, -16.0, -3.0], [6.0, 14.0, 6.0], DEMON_TIP_AT),
                ],
                "quads": outer_quads,
                "children": fingers,
            }],
        })
    };
    json!({
        "format": 1, "id": "demon_wings", "slot": "back", "bone": "body",
        "attach": { "offset": [0, 0, 0], "rotation": [0, 0, 0], "scale": 1.0 / HDF },
        "texture": { "png": b64(&texture.png()) },
        "parts": [wing("left_wing", 1.0), wing("right_wing", -1.0)],
        "animations": flap(60.0, 28.0, 7.0, 20.0),
    })
}

/// Wings opening and closing: each wing swings back and forth about y (`open` degrees) and up and
/// down a little (`lift`), its outer half following a beat later (`tip` degrees).
fn flap(period: f32, open: f32, lift: f32, tip: f32) -> Value {
    let mut out = Vec::new();
    for (name, s) in [("left_wing", 1.0), ("right_wing", -1.0)] {
        out.push(json!({ "part": name, "motion": "swing", "axis": "y", "amplitude": s * open, "period": period }));
        out.push(json!({ "part": name, "motion": "swing", "axis": "z", "amplitude": s * lift, "period": period, "phase": 0.25 }));
        out.push(json!({ "part": format!("{name}_tip"), "motion": "swing", "axis": "y", "amplitude": s * tip, "period": period, "phase": 0.9 }));
    }
    json!(out)
}

/// Angel Wings' feathers: five lengths (HD units), each its own place in the texture's top rows.
const FEATHER_W: i32 = 16;
const FEATHER_D: i32 = 4;
const FEATHER_LENGTHS: [i32; 5] = [104, 92, 80, 68, 56];
/// The three sections of a wing, root to tip: bone length, bend from the section before (degrees,
/// down-outward), and the lengths of the feathers hanging from it.
const ANGEL_SECTIONS: [(f32, f32, [i32; 4]); 3] = [
    (68.0, 0.0, [56, 56, 68, 68]),
    (60.0, 26.0, [80, 80, 92, 92]),
    (46.0, 26.0, [104, 104, 92, 0]),
];
/// The coverts: a row of small rounded feathers over each section's bone.
const ANGEL_COVERT: ([i32; 3], [i32; 2]) = ([68, 34, 8], [0, 112]);
const ANGEL_BONE_AT: [i32; 2] = [0, 160];
const ANGEL_TEX: (u32, u32) = (256, 192);

/// Where a feather length's box sits in the texture.
fn feather_uv(len: i32) -> [i32; 2] {
    let i = FEATHER_LENGTHS.iter().position(|&l| l == len).unwrap() as i32;
    [i * 2 * (FEATHER_W + FEATHER_D), 0]
}

/// White feather shades, cool blue-grey in the shadow.
const PLUME: [u32; 7] = [
    0x8792b4, 0xa6b1ce, 0xc2cbe2, 0xd9e0ef, 0xe9eef8, 0xf6f8fd, 0xffffff,
];

/// A feather's face: `along` 0 at the root to 1 at the tip, `across` -1..1; transparent past
/// its rounded tip.
fn feather_texel(along: f32, across: f32, len: f32, back: bool) -> Rgba {
    // The rounded tip: the last few texels narrow to a point.
    let tip = ((1.0 - along) * len / 9.0).min(1.0);
    let half = (1.0 - (1.0 - tip).powi(2)).sqrt();
    if across.abs() > half * 1.02 {
        return [0, 0, 0, 0];
    }
    if across.abs() > half - 0.15 {
        return hex(PLUME[1]);
    }
    let mut t = 0.95 - across.abs() * 0.3 - along * 0.15;
    // Barbs slanting from the shaft to the tip.
    let barb = ((along * len - across.abs() * 6.0) / 5.0).fract();
    if barb < 0.18 {
        t -= 0.12;
    }
    if back {
        t -= 0.15;
    }
    if across.abs() < 0.12 {
        return mix(hex(0xfbf6e6), hex(0xe2cf96), along);
    }
    ramp(&PLUME, t)
}

/// Angel Wings' texture: long feathers with a gold shaft and slanting barbs, a row of small rounded
/// coverts (one band all round its box, so a shorter box shows the same row), and a gold arm.
fn angel_texture() -> Canvas {
    let mut c = Canvas::new(ANGEL_TEX.0, ANGEL_TEX.1);
    for len in FEATHER_LENGTHS {
        let [u, v] = feather_uv(len);
        paint_box(
            &mut c,
            u,
            v,
            FEATHER_W,
            len,
            FEATHER_D,
            |face, i, j, fw, fh| {
                let across = (i as f32 + 0.5) / fw as f32 * 2.0 - 1.0;
                let along = (j as f32 + 0.5) / fh as f32;
                match face {
                    3 | 5 => feather_texel(along, across, len as f32, face == 5),
                    0 | 1 => hex(PLUME[4]),
                    _ => {
                        // The thin sides end where the tip begins to round.
                        if along > 1.0 - 6.0 / len as f32 {
                            [0, 0, 0, 0]
                        } else {
                            hex(PLUME[2])
                        }
                    }
                }
            },
        );
    }
    let ([w, h, d], [u, v]) = ANGEL_COVERT;
    let width = 2 * (w + d) + 64;
    c.rect(u + d, v, 2 * w + 64, d, hex(PLUME[5]));
    for j in 0..h {
        for i in 0..width {
            let (x, y) = (i as f32 + 0.5, j as f32 + 0.5);
            // Two rows of small rounded feathers, the lower row offset by half a feather.
            let row = |off: f32, bottom: f32| {
                let cell = ((x + off) / 16.0).fract() * 2.0 - 1.0;
                bottom - 6.0 + 6.0 * (1.0 - cell * cell).sqrt()
            };
            let low = row(8.0, h as f32);
            let high = row(0.0, h as f32 * 0.55);
            if y > low {
                continue;
            }
            let (edge, cell) = if y > high {
                (low - y, ((x + 8.0) / 16.0).fract())
            } else {
                (high - y, (x / 16.0).fract())
            };
            let col = if edge < 1.2 {
                hex(PLUME[1])
            } else {
                let t = 0.92
                    - (cell - 0.5).abs() * 0.45
                    - (1.0 - edge / 10.0).max(0.0) * 0.25
                    - y / h as f32 * 0.1;
                ramp(&PLUME, t)
            };
            c.set(u + i, v + d + j, col);
        }
    }
    let gold = [
        0x5a3a0c, 0x8a5c14, 0xb8862a, 0xd9a83a, 0xf2c14e, 0xffdf86, 0xfff4c8,
    ];
    paint_bone(&mut c, ANGEL_BONE_AT, 72, 12, &gold);
    c
}

/// Angel Wings: each wing three sections of gold bone, each bending further down and out, with a
/// row of small coverts in front and long feathers hanging behind, longest at the tip, so the
/// feathers fan out like a bird's.
fn angel_item(texture: &Canvas) -> Value {
    let ([_, ch, cd], cuv) = ANGEL_COVERT;
    let wing = |name: &str, s: f32| -> Value {
        // Built from the tip in, each section the child of the one before.
        let mut child: Option<Value> = None;
        for (k, &(len, bend, feathers)) in ANGEL_SECTIONS.iter().enumerate().rev() {
            let t = 10.0 - k as f32 * 2.0;
            let mut cubes = vec![
                side_cube(
                    s,
                    [0.0, -t / 2.0, -t / 2.0],
                    [len + 4.0, t, t],
                    ANGEL_BONE_AT,
                ),
                side_cube(s, [0.0, 2.0, -9.0], [len, ch as f32, cd as f32], cuv),
            ];
            for (f, flen) in feathers.into_iter().enumerate().filter(|f| f.1 > 0) {
                let x = f as f32 * (len - 16.0) / 3.0;
                let z = if f % 2 == 0 { 0.0 } else { 2.0 };
                cubes.push(side_cube(
                    s,
                    [x, 2.0 + f as f32 * 2.0, z],
                    [FEATHER_W as f32, flen as f32, FEATHER_D as f32],
                    feather_uv(flen),
                ));
            }
            let section_name = match k {
                0 => name.to_owned(),
                1 => format!("{name}_mid"),
                _ => format!("{name}_tip"),
            };
            let (pivot, rotation) = if k == 0 {
                ([s * 10.0, 16.0, 20.0], [0.0, s * -40.0, s * -34.0])
            } else {
                let before = ANGEL_SECTIONS[k - 1].0;
                ([s * before, 0.0, 0.0], [0.0, s * -6.0, s * bend])
            };
            let mut part = json!({
                "name": section_name, "pivot": pivot, "rotation": rotation, "cubes": cubes,
            });
            if let Some(c) = child.take() {
                part["children"] = json!([c]);
            }
            child = Some(part);
        }
        child.unwrap()
    };
    let mut animations = flap(72.0, 26.0, 6.0, 14.0);
    for (name, s) in [("left_wing", 1.0), ("right_wing", -1.0)] {
        animations.as_array_mut().unwrap().push(json!({
            "part": format!("{name}_mid"), "motion": "swing", "axis": "y",
            "amplitude": s * 10.0, "period": 72, "phase": 0.95,
        }));
    }
    json!({
        "format": 1, "id": "angel_wings", "slot": "back", "bone": "body", "glow": true,
        "attach": { "offset": [0, 0, 0], "rotation": [0, 0, 0], "scale": 1.0 / HDF },
        "texture": { "png": b64(&texture.png()) },
        "parts": [wing("left_wing", 1.0), wing("right_wing", -1.0)],
        "animations": animations,
    })
}

// ---------------------------------------------------------------- Bamboo Sword

/// The Bamboo Sword's boxes in HD units (8 a model pixel, drawn at an eighth of the scale):
/// (name, origin, size, uv), the blade's end up (-y), the pommel down.
const SWORD: [(&str, [f32; 3], [i32; 3], [i32; 2]); 4] = [
    ("blade", [-8.0, -96.0, -8.0], [16, 120, 16], [0, 0]),
    ("guard", [-24.0, 24.0, -8.0], [48, 8, 16], [128, 0]),
    ("grip", [-8.0, 32.0, -8.0], [16, 40, 16], [64, 0]),
    ("pommel", [-12.0, 72.0, -12.0], [24, 8, 24], [128, 24]),
];
/// The leaf: a thin plate whose front and back carry the leaf's shape, sprouting from the blade's side.
const LEAF_PIVOT: [f32; 2] = [8.0, -64.0];
const LEAF: ([f32; 3], [i32; 3], [i32; 2]) = ([0.0, -6.0, -1.0], [36, 12, 2], [64, 56]);

/// Light on a round stalk seen face on: `across` 0..1 over the face, lit from the left.
fn stalk_light(across: f32) -> f32 {
    let round = (across * std::f32::consts::PI).sin();
    (0.35 + 0.65 * round - 0.25 * across).clamp(0.0, 1.0)
}

/// A cut end of bamboo: the hollow, the pale wall, the green skin, `r` 0 at the middle, 1 at
/// the rim.
fn bamboo_end(r: f32, n: f32) -> Rgba {
    if r < 0.52 {
        mix(hex(0x2a1c0c), hex(0x4a3418), r / 0.52)
    } else if r < 0.8 {
        let ring = ((r - 0.52) * 40.0).sin() * 0.5 + 0.5;
        mix(hex(0xe9dca6), hex(0xcdb978), ring * 0.4 + n * 0.2)
    } else {
        mix(hex(0x4f8a22), hex(0x2f5a14), (r - 0.8) * 4.0)
    }
}

/// The Bamboo Sword's HD texture: a green stalk with fibres along it, a raised node every five
/// pixels, cut ends showing the hollow and the pale wall, a bamboo guard and pommel, a grip wound
/// with twine, a leaf with its midrib.
fn bamboo_texture() -> Canvas {
    let mut c = Canvas::new(256, 144);
    let green = |light: f32, i: i32, j: i32| {
        let fibre = (vnoise(i as f32 * 0.9, j as f32 * 0.05, 901) - 0.5) * 0.35;
        let speck = if noise(i, j, 903) > 0.995 { -0.3 } else { 0.0 };
        let l = (light + fibre + speck).clamp(0.0, 1.0);
        if l < 0.5 {
            mix(hex(0x1f4209), hex(0x5f9e2b), l * 2.0)
        } else {
            mix(hex(0x5f9e2b), hex(0xb6e06a), (l - 0.5) * 2.0)
        }
    };
    for (name, _, [w, h, d], [u, v]) in SWORD {
        paint_box(&mut c, u, v, w, h, d, |face, i, j, fw, fh| {
            let across = (i as f32 + 0.5) / fw as f32;
            let end_r = || {
                let (cx, cy) = (
                    (i as f32 + 0.5) / fw as f32 - 0.5,
                    (j as f32 + 0.5) / fh as f32 - 0.5,
                );
                cx.hypot(cy) * 2.0
            };
            match name {
                "blade" if face < 2 => bamboo_end(end_r(), noise(i, j, 905)),
                "blade" => {
                    // A node every 40 texels: a raised ring, lit on top, shaded under.
                    let n = j % 40;
                    let mut l = stalk_light(across);
                    if (34..40).contains(&n) {
                        l = l * 0.55 + if n < 36 { 0.35 } else { 0.0 };
                    } else if n < 3 {
                        l *= 0.75;
                    }
                    let col = green(l, i, j);
                    if (36..38).contains(&n) {
                        mix(col, hex(0xd8e6a0), 0.35)
                    } else {
                        col
                    }
                }
                "guard" if face == 2 || face == 4 => bamboo_end(end_r(), noise(i, j, 907)),
                "guard" => {
                    let along = (j as f32 + 0.5) / fh as f32;
                    let col = green(stalk_light(along) * 0.8, j, i);
                    if i % 24 < 2 {
                        mix(col, hex(0x1f4209), 0.5)
                    } else {
                        col
                    }
                }
                "pommel" if face < 2 => bamboo_end(end_r(), noise(i, j, 909)),
                "pommel" => green(stalk_light(across) * 0.65, i, j),
                // Twine: strands wound slantwise, each round, fibrous.
                _ if face < 2 => mix(hex(0x5a3a1e), hex(0x8a6038), noise(i, j, 911)),
                _ => {
                    let s = ((j as f32 + i as f32 * 0.45) / 5.0).fract();
                    let strand = (s * std::f32::consts::PI).sin();
                    let l = (strand * 0.75 + stalk_light(across) * 0.35).clamp(0.0, 1.0);
                    let col = mix(hex(0x3a2412), hex(0xc39561), l);
                    add_light(
                        col,
                        [40.0, 30.0, 20.0],
                        (vnoise(i as f32 * 2.0, j as f32 * 2.0, 913) - 0.5) * 0.8,
                    )
                }
            }
        });
    }
    // The leaf on both broad faces; its thin sides stay clear.
    let (_, [w, h, d], [u, v]) = LEAF;
    paint_box(&mut c, u, v, w, h, d, |face, i, j, fw, fh| {
        // The front and the back (mirrored, so the leaf grows from the stalk on both).
        if face != 3 && face != 5 {
            return [0, 0, 0, 0];
        }
        let along = (i as f32 + 0.5) / fw as f32;
        let along = if face == 5 { 1.0 - along } else { along };
        let across = (j as f32 + 0.5) / fh as f32 * 2.0 - 1.0;
        let half =
            0.95 * (along.powf(0.6) * std::f32::consts::PI).sin().max(0.0) * (1.0 - along * 0.25);
        if across.abs() > half || along < 0.03 {
            return [0, 0, 0, 0];
        }
        let mut col = mix(
            hex(0x6fcf3a),
            hex(0x2f8a1c),
            across.abs() / half.max(0.01) * 0.6 + along * 0.2,
        );
        if face == 5 {
            col = mix(col, hex(0x1f5a12), 0.25);
        }
        if across.abs() < 0.09 {
            col = mix(col, hex(0xc8ee8a), 0.5);
        }
        let vein = ((along * 18.0 - across.abs() * 3.0).fract() < 0.12) as i32 as f32;
        mix(col, hex(0x3d9a24), vein * 0.3)
    });
    c
}

/// The Bamboo Sword across the back like a sword in its scabbard: the grip over the right
/// shoulder, the blade down to the left hip. Its boxes are 8× a model pixel, so the item is
/// drawn at an eighth of 0.8.
fn bamboo_item(texture: &Canvas) -> Value {
    let cubes: Vec<Value> = SWORD
        .iter()
        .map(|&(_, origin, size, uv)| cube(origin, size, uv))
        .collect();
    let (leaf_at, leaf_size, leaf_uv) = LEAF;
    json!({
        "format": 1, "id": "bamboo_sword", "slot": "weapon", "bone": "body",
        "attach": { "offset": [0.0, 6.0, 3.0], "rotation": [0.0, 0.0, 145.0], "scale": 0.8 / HDF },
        "texture": { "png": b64(&texture.png()) },
        "parts": [{
            "name": "sword", "pivot": [0, 0, 0], "cubes": cubes,
            "children": [{
                "name": "leaf", "pivot": [LEAF_PIVOT[0], LEAF_PIVOT[1], 0.0], "rotation": [0.0, 0.0, -30.0],
                "cubes": [cube(leaf_at, leaf_size, leaf_uv)],
            }],
        }],
    })
}

// ---------------------------------------------------------------- Stray Head (a straw hat)

/// The hat is built of thin layers of boxes, 4 units a model pixel (drawn at a quarter scale):
/// each layer a rounded cross of three boxes, stepping in towards the top like a conical straw
/// hat's slope. `HAT_LAYERS` layers, each `HAT_STEP` units tall, the widest `HAT_RADIUS` across
/// (twice the head), the brim's underside at `HAT_BRIM_Y` (the head's top is -32).
const HAT_UNIT: f32 = 4.0;
const HAT_LAYERS: i32 = 11;
const HAT_STEP: f32 = 2.0;
const HAT_RADIUS: f32 = 32.0;
const HAT_TOP_RADIUS: f32 = 5.0;
const HAT_BRIM_Y: f32 = -25.0;
/// A layer's three boxes as (half-width in x, half-width in z), of its radius.
const HAT_CROSS: [(f32, f32); 3] = [(1.0, 0.5), (0.5, 1.0), (0.8, 0.8)];
/// Texture places: the brim's three boxes (copper sides), then one shared by all the others.
const HAT_BRIM_AT: [[i32; 2]; 3] = [[0, 0], [0, 34], [0, 100]];
const HAT_STRAW_AT: [i32; 2] = [0, 154];
const HAT_KNOT_AT: [i32; 2] = [212, 0];

/// A layer's radius, the brim 0.
fn hat_radius(layer: i32) -> f32 {
    let t = layer as f32 / (HAT_LAYERS - 1) as f32;
    HAT_RADIUS + (HAT_TOP_RADIUS - HAT_RADIUS) * t.powf(0.9)
}

/// A layer's three boxes (origin, size), even sizes so they sit centred.
fn hat_boxes(layer: i32) -> [([f32; 3], [f32; 3]); 3] {
    let r = hat_radius(layer);
    let y = HAT_BRIM_Y - HAT_STEP * (layer + 1) as f32;
    HAT_CROSS.map(|(ax, az)| {
        let (w, d) = ((r * ax).round() * 2.0, (r * az).round() * 2.0);
        ([-w / 2.0, y, -d / 2.0], [w, HAT_STEP, d])
    })
}

/// Straw: woven palm strips running round the hat, each its own shade, with fibres along them,
/// stitch rows across, and a few darker tucks.
fn straw(i: i32, j: i32) -> Rgba {
    let shades = [
        0x7a5a2c, 0x9c7a42, 0xb8965a, 0xcdb072, 0xdcc28a, 0xe8d5a4, 0xf3e6c4,
    ];
    let strip = j.div_euclid(3);
    let mut t = 0.55 + (noise(strip, 0, 71) - 0.5) * 0.35;
    t += (vnoise(i as f32 * 0.25, strip as f32, 73) - 0.5) * 0.3;
    if j.rem_euclid(3) == 2 {
        t -= 0.18;
    }
    if (i + strip * 5).rem_euclid(14) == 0 {
        t -= 0.3;
    }
    ramp(&shades, t)
}

/// The Stray Head's texture: straw all over; the brim's sides a copper rim wound with red thread;
/// a white cord knot.
fn hat_texture() -> Canvas {
    let mut c = Canvas::new(256, 256);
    let brim = hat_boxes(0);
    for (k, &(_, [w, h, d])) in brim.iter().enumerate() {
        let [u, v] = HAT_BRIM_AT[k];
        let (w, h, d) = (w as i32, h as i32, d as i32);
        paint_box(&mut c, u, v, w, h, d, |face, i, j, _, _| {
            if face < 2 {
                let s = straw(i, j);
                if face == 1 {
                    mix(s, hex(0x4a3418), 0.35)
                } else {
                    s
                }
            } else if (i + j).rem_euclid(4) == 0 {
                hex(0xc8322a)
            } else {
                ramp(
                    &[0x7a3e18, 0xa8602a, 0xd08a48, 0xeab070],
                    0.35 + 0.4 * (1.0 - j as f32),
                )
            }
        });
    }
    // The other layers share one place, big enough for the brim's widest box layout.
    let (sw, sh) = (2 * (52 + 64), 66);
    for j in 0..sh {
        for i in 0..sw {
            c.set(HAT_STRAW_AT[0] + i, HAT_STRAW_AT[1] + j, straw(i, j + 5));
        }
    }
    paint_box(
        &mut c,
        HAT_KNOT_AT[0],
        HAT_KNOT_AT[1],
        8,
        6,
        8,
        |face, i, j, _, _| {
            let t = 0.6 + (noise(i, j, 77) - 0.5) * 0.3 + if face == 0 { 0.2 } else { 0.0 };
            ramp(
                &[0x9a9488, 0xc8c2b4, 0xe6e2d8, 0xfaf8f2],
                t - ((i + j) % 3 == 0) as i32 as f32 * 0.2,
            )
        },
    );
    c
}

/// The Stray Head: a conical straw hat in stepped layers, twice the head's width, its copper rim
/// wound with red thread and a white knot on top.
fn hat_item(texture: &Canvas) -> Value {
    let mut cubes = Vec::new();
    for layer in 0..HAT_LAYERS {
        for (k, (origin, size)) in hat_boxes(layer).into_iter().enumerate() {
            let uv = if layer == 0 {
                HAT_BRIM_AT[k]
            } else {
                HAT_STRAW_AT
            };
            cubes.push(json!({ "origin": origin, "size": size, "uv": uv }));
        }
    }
    let top = HAT_BRIM_Y - HAT_STEP * (HAT_LAYERS + 1) as f32;
    cubes.push(json!({ "origin": [-4.0, top - 4.0, -4.0], "size": [8, 6, 8], "uv": HAT_KNOT_AT }));
    json!({
        "format": 1, "id": "stray_head", "slot": "head", "bone": "head",
        "attach": { "offset": [0, 0, 0], "rotation": [0, 0, 0], "scale": 1.0 / HAT_UNIT },
        "texture": { "png": b64(&texture.png()) },
        "parts": [{ "name": "hat", "pivot": [0, 0, 0], "cubes": cubes }],
    })
}

// ---------------------------------------------------------------- Devil Horns and Angel Halo

/// A devil horn's segments, base to tip: thickness (HD units) and the bend from the one before
/// (degrees: out towards the side, back). Each is `HORN_LEN` units (2 pixels) long.
const HORN: [(i32, f32, f32); 5] = [
    (18, 38.0, -16.0),
    (15, -16.0, -10.0),
    (12, -20.0, -6.0),
    (9, -22.0, 4.0),
    (6, -24.0, 12.0),
];
const HORN_LEN: i32 = 16;

/// Where a horn segment's box sits in the texture: a column of them.
fn horn_uv(k: usize) -> [i32; 2] {
    [0, HORN[..k].iter().map(|(t, _, _)| t + HORN_LEN + 2).sum()]
}

/// The horns' texture: dark crimson at the base to near black at the tip, ridged rings round
/// each segment, a sheen down the front.
fn horn_texture() -> Canvas {
    let mut c = Canvas::new(80, 160);
    let shades = [
        0x0c0204, 0x1e0508, 0x3a090e, 0x5c0f14, 0x84181a, 0xae2a22, 0xd8502e,
    ];
    for (k, &(t, _, _)) in HORN.iter().enumerate() {
        let [u, v] = horn_uv(k);
        let base = 0.82 - k as f32 * 0.12;
        paint_box(&mut c, u, v, t, HORN_LEN, t, |face, i, j, fw, fh| {
            let across = (i as f32 + 0.5) / fw as f32;
            let along = (j as f32 + 0.5) / fh as f32;
            let mut l = base + (1.0 - along) * 0.08;
            match face {
                0 | 1 => l -= 0.05,
                3 => l += 0.1 - (across - 0.4).abs() * 0.3,
                5 => l -= 0.12,
                _ => l -= (across - 0.5).abs() * 0.2,
            }
            if face >= 2 && j % 4 == 3 {
                l -= 0.14;
            }
            l += (noise(i, j + k as i32 * 31, 501) - 0.5) * 0.06;
            ramp(&shades, l)
        });
    }
    c
}

/// Devil Horns: two curved horns on the head, each five tapering segments bending out, up and
/// back, dark crimson to a black point.
fn horns_item(texture: &Canvas) -> Value {
    let horn = |name: &str, s: f32| -> Value {
        let mut child: Option<Value> = None;
        for (k, &(t, out, back)) in HORN.iter().enumerate().rev() {
            let tf = t as f32;
            let pivot = if k == 0 {
                [s * 20.0, -62.0, -6.0]
            } else {
                [0.0, -(HORN_LEN as f32) + 1.0, 0.0]
            };
            let mut part = json!({
                "name": if k == 0 { name.to_owned() } else { format!("{name}_{k}") },
                "pivot": pivot, "rotation": [back, 0.0, s * out],
                "cubes": [side_cube(s, [-tf / 2.0, -(HORN_LEN as f32), -tf / 2.0], [tf, HORN_LEN as f32, tf], horn_uv(k))],
            });
            if let Some(c) = child.take() {
                part["children"] = json!([c]);
            }
            child = Some(part);
        }
        child.unwrap()
    };
    json!({
        "format": 1, "id": "devil_horns", "slot": "head", "bone": "head",
        "attach": { "offset": [0, 0, 0], "rotation": [0, 0, 0], "scale": 1.0 / HDF },
        "texture": { "png": b64(&texture.png()) },
        "parts": [horn("left_horn", 1.0), horn("right_horn", -1.0)],
    })
}

/// The halo: a ring of `HALO_SEGMENTS` gold bars round a circle `HALO_RADIUS` across (HD units),
/// floating `HALO_Y` above the head's pivot.
const HALO_SEGMENTS: usize = 12;
const HALO_RADIUS: f32 = 30.0;
const HALO_THICK: f32 = 5.0;
const HALO_Y: f32 = -82.0;

/// The halo's texture: bright gold, a white-hot line along the top, warmer underneath.
fn halo_texture() -> Canvas {
    let mut c = Canvas::new(64, 16);
    let gold = [
        0x8a5c14, 0xc08a26, 0xe8b83c, 0xf8d662, 0xffe9a0, 0xfff8e0, 0xffffff,
    ];
    let len = halo_bar() as i32;
    let t = HALO_THICK as i32;
    paint_box(&mut c, 0, 0, len, t, t, |face, i, j, _, fh| {
        let l = match face {
            0 => 0.95,
            1 => 0.3,
            _ => 0.85 - (j as f32 + 0.5) / fh as f32 * 0.45,
        };
        ramp(&gold, l + (noise(i, j, 511) - 0.5) * 0.05)
    });
    c
}

/// A halo bar's length: the chord of its share of the circle, a little longer so they meet.
fn halo_bar() -> f32 {
    (2.0 * HALO_RADIUS * (std::f32::consts::PI / HALO_SEGMENTS as f32).tan() + HALO_THICK).round()
}

/// Angel Halo: a glowing gold ring floating over the head, tilted a little, turning slowly and
/// bobbing.
fn halo_item(texture: &Canvas) -> Value {
    let len = halo_bar();
    let bars: Vec<Value> = (0..HALO_SEGMENTS)
        .map(|i| {
            json!({
                "name": format!("halo_{i}"), "pivot": [0, 0, 0],
                "rotation": [0.0, i as f32 * 360.0 / HALO_SEGMENTS as f32, 0.0],
                "cubes": [{
                    "origin": [-len / 2.0, -HALO_THICK / 2.0, HALO_RADIUS - HALO_THICK / 2.0],
                    "size": [len, HALO_THICK, HALO_THICK], "uv": [0, 0],
                }],
            })
        })
        .collect();
    json!({
        "format": 1, "id": "angel_halo", "slot": "head", "bone": "head", "glow": true,
        "attach": { "offset": [0, 0, 0], "rotation": [0, 0, 0], "scale": 1.0 / HDF },
        "texture": { "png": b64(&texture.png()) },
        "parts": [{
            "name": "halo", "pivot": [0.0, HALO_Y, 0.0], "rotation": [-8, 0, 0],
            "children": bars,
        }],
        "animations": [
            { "part": "halo", "motion": "spin", "axis": "y", "amplitude": 0, "period": 240 },
            { "part": "halo", "motion": "bob", "axis": "y", "amplitude": 4.0, "period": 60 },
        ],
    })
}

// ---------------------------------------------------------------- Christmas: Santa Hat and Santa's Sack

/// Both are built 4 units a model pixel (drawn at a quarter scale), like the Stray Head.
const XMAS_UNIT: f32 = 4.0;

/// Lays boxes out in a texture left to right in rows: each box takes its unwrap's footprint
/// (2 × (w + d) across, d + h down).
struct Shelf {
    x: i32,
    y: i32,
    row: i32,
    width: i32,
}

impl Shelf {
    fn new(width: i32) -> Self {
        Self {
            x: 0,
            y: 0,
            row: 0,
            width,
        }
    }
    fn place(&mut self, [w, h, d]: [i32; 3]) -> [i32; 2] {
        let (fw, fh) = (2 * (w + d), d + h);
        if self.x + fw > self.width {
            self.x = 0;
            self.y += self.row + 1;
            self.row = 0;
        }
        let at = [self.x, self.y];
        self.x += fw + 1;
        self.row = self.row.max(fh);
        assert!(self.y + fh <= 256, "texture full");
        at
    }
}

/// Red velvet: deep in the folds, bright where the light catches the nap.
fn velvet(l: f32, i: i32, j: i32, seed: u32) -> Rgba {
    let nap = (vnoise(i as f32 * 0.35, j as f32 * 0.35, seed) - 0.5) * 0.22;
    ramp(
        &[0x5a0a12, 0x8a121c, 0xb31b26, 0xd42a35, 0xea4a52, 0xf47a7e],
        l + nap + (noise(i, j, seed + 1) - 0.5) * 0.06,
    )
}

/// White fur in soft clumps, greyer in the hollows between them.
fn fur(l: f32, i: i32, j: i32, seed: u32) -> Rgba {
    let clump = vnoise(i as f32 * 0.5, j as f32 * 0.5, seed);
    let strand = (noise(i, j, seed + 3) - 0.5) * 0.2;
    ramp(
        &[0xa9adb8, 0xcfd3dc, 0xe8eaef, 0xf6f7f9, 0xffffff],
        l * 0.5 + clump * 0.55 + strand,
    )
}

/// Light on a box face: the top brightest, the front lit, the sides and back darker.
fn face_light(face: usize) -> f32 {
    match face {
        0 => 0.82,
        1 => 0.28,
        3 => 0.66,
        5 => 0.46,
        _ => 0.54,
    }
}

/// The hat's cone, base to tip: width, height and its bend from the one below (degrees: x
/// tips it back, z to the wearer's left), so the tip flops back and over to one side.
const SANTA_CONE: [(i32, i32, f32, f32); 6] = [
    (32, 10, -2.0, 0.0),
    (26, 10, -4.0, 6.0),
    (20, 9, -4.0, 14.0),
    (15, 9, -4.0, 24.0),
    (11, 8, -3.0, 30.0),
    (8, 8, -2.0, 34.0),
];
const SANTA_BRIM: [i32; 3] = [36, 7, 36];
const SANTA_POMPOM: [i32; 3] = [10, 10, 10];

/// Where each box of the hat sits in the texture: the brim, the cone's segments, the pompom.
fn santa_hat_uvs() -> (Vec<[i32; 2]>, [i32; 2], [i32; 2]) {
    let mut shelf = Shelf::new(256);
    let brim = shelf.place(SANTA_BRIM);
    let cone = SANTA_CONE
        .iter()
        .map(|&(w, h, _, _)| shelf.place([w, h, w]))
        .collect();
    let pompom = shelf.place(SANTA_POMPOM);
    (cone, brim, pompom)
}

/// The Santa Hat's texture: red velvet, darker towards the tip; white fur brim and pompom.
fn santa_hat_texture() -> Canvas {
    let mut c = Canvas::new(256, 256);
    let (cone, brim, pompom) = santa_hat_uvs();
    let [w, h, d] = SANTA_BRIM;
    paint_box(&mut c, brim[0], brim[1], w, h, d, |face, i, j, _, fh| {
        let roll = if face >= 2 {
            // The rolled fur is round: lit along its middle.
            let t = (j as f32 + 0.5) / fh as f32;
            0.3 + 0.7 * (t * std::f32::consts::PI).sin()
        } else {
            face_light(face)
        };
        fur(roll, i, j, 601)
    });
    for (k, &(w, h, _, _)) in SANTA_CONE.iter().enumerate() {
        let [u, v] = cone[k];
        let tip = k as f32 / (SANTA_CONE.len() - 1) as f32;
        paint_box(&mut c, u, v, w, h, w, |face, i, j, fw, _| {
            let across = (i as f32 + 0.5) / fw as f32;
            let mut l = face_light(face) - tip * 0.12;
            if face >= 2 {
                l += 0.12 * (across * std::f32::consts::PI).sin() - 0.06;
            }
            velvet(l, i + k as i32 * 17, j, 611)
        });
    }
    let [w, h, d] = SANTA_POMPOM;
    paint_box(&mut c, pompom[0], pompom[1], w, h, d, |face, i, j, _, _| {
        fur(face_light(face) + 0.15, i, j, 621)
    });
    c
}

/// The Santa Hat: a white fur brim round the head, a red velvet cone flopping back and over to
/// one side, and a white pompom at its tip that swings as you move.
fn santa_hat_item(texture: &Canvas) -> Value {
    let (cone, brim_uv, pompom_uv) = santa_hat_uvs();
    let mut child: Option<Value> = None;
    for (k, &(w, h, back, side)) in SANTA_CONE.iter().enumerate().rev() {
        let (wf, hf) = (w as f32, h as f32);
        let pivot = if k == 0 {
            [0.0, -33.0, 0.0]
        } else {
            [0.0, -(SANTA_CONE[k - 1].1 as f32) + 1.0, 0.0]
        };
        let mut part = json!({
            "name": format!("cone_{k}"),
            "pivot": pivot, "rotation": [back, 0.0, side],
            "cubes": [cube([-wf / 2.0, -hf, -wf / 2.0], [w, h, w], cone[k])],
        });
        let mut children = Vec::new();
        if k == SANTA_CONE.len() - 1 {
            let [pw, ph, pd] = SANTA_POMPOM;
            children.push(json!({
                "name": "pompom", "pivot": [0.0, -hf + 2.0, 0.0],
                "cubes": [cube([-(pw as f32) / 2.0, -(ph as f32), -(pd as f32) / 2.0], SANTA_POMPOM, pompom_uv)],
            }));
        }
        if let Some(c) = child.take() {
            children.push(c);
        }
        if !children.is_empty() {
            part["children"] = json!(children);
        }
        child = Some(part);
    }
    let [bw, bh, bd] = SANTA_BRIM;
    json!({
        "format": 1, "id": "santa_hat", "slot": "head", "bone": "head",
        "attach": { "offset": [0, 0, 0], "rotation": [0, 0, 0], "scale": 1.0 / XMAS_UNIT },
        "texture": { "png": b64(&texture.png()) },
        "parts": [
            {
                "name": "brim", "pivot": [0, 0, 0],
                "cubes": [cube([-(bw as f32) / 2.0, -34.0, -(bd as f32) / 2.0], [bw, bh, bd], brim_uv)],
            },
            child.unwrap(),
        ],
        "animations": [
            { "part": "cone_3", "motion": "swing", "axis": "x", "amplitude": 5.0, "period": 46 },
            { "part": "cone_5", "motion": "swing", "axis": "z", "amplitude": 7.0, "period": 34, "phase": 0.3 },
        ],
    })
}

/// What Santa's Sack is made of, for its texture.
#[derive(Clone, Copy, PartialEq)]
enum Stuff {
    Sack,
    Ruffle,
    Rope,
    Gift,
    Cane,
}

/// The sack's boxes in 4-unit pixels on the body's back (+z away from it, y down from the
/// shoulders): (part, origin, size, what it is made of). The sack is a few overlapping boxes so
/// it bulges; the neck is gathered and tied, the ruffle spills over the rope.
const SACK: [(&str, [f32; 3], [i32; 3], Stuff); 12] = [
    ("sack", [-15.0, 6.0, 0.0], [30, 38, 24], Stuff::Sack),
    ("sack", [-18.0, 11.0, 1.0], [36, 28, 22], Stuff::Sack),
    ("sack", [-20.0, 16.0, 2.0], [40, 18, 18], Stuff::Sack),
    ("sack", [-12.0, 40.0, 3.0], [24, 6, 18], Stuff::Sack),
    ("sack", [-7.0, 0.0, 6.0], [14, 8, 12], Stuff::Sack),
    ("sack", [-8.0, 2.0, 5.0], [16, 3, 14], Stuff::Rope),
    ("sack", [-11.0, -5.0, 3.0], [22, 6, 18], Stuff::Ruffle),
    ("sack", [-8.0, -8.0, 5.0], [16, 3, 14], Stuff::Ruffle),
    ("gift", [-6.0, -12.0, -6.0], [12, 12, 12], Stuff::Gift),
    ("cane", [-1.5, -22.0, -1.5], [3, 22, 3], Stuff::Cane),
    ("cane", [-1.5, -25.0, -1.5], [10, 3, 3], Stuff::Cane),
    ("cane", [5.5, -22.0, -1.5], [3, 6, 3], Stuff::Cane),
];

fn sack_uvs() -> Vec<[i32; 2]> {
    let mut shelf = Shelf::new(256);
    SACK.iter()
        .map(|&(_, _, size, _)| shelf.place(size))
        .collect()
}

/// Burlap: a weave of threads across and down, each a little different, with darker flecks.
fn burlap(l: f32, i: i32, j: i32) -> Rgba {
    let weave = if (i + j) % 2 == 0 { 0.06 } else { -0.06 };
    let thread = (noise(i, 0, 631) - 0.5) * 0.14 + (noise(0, j, 633) - 0.5) * 0.14;
    let fleck = if noise(i, j, 635) > 0.985 { -0.25 } else { 0.0 };
    ramp(
        &[0x3e2412, 0x5e3a1e, 0x80542c, 0x9f6e3c, 0xbd8c52, 0xd8ac72],
        l + weave + thread + fleck + (vnoise(i as f32 * 0.12, j as f32 * 0.12, 637) - 0.5) * 0.2,
    )
}

/// Santa's Sack's texture: brown burlap, a gold rope with a twist, a green present with a red
/// ribbon, a red and white candy cane.
fn sack_texture() -> Canvas {
    let mut c = Canvas::new(256, 256);
    for (k, &(_, _, [w, h, d], stuff)) in SACK.iter().enumerate() {
        let [u, v] = sack_uvs()[k];
        paint_box(&mut c, u, v, w, h, d, |face, i, j, fw, fh| {
            let across = (i as f32 + 0.5) / fw as f32;
            let down = (j as f32 + 0.5) / fh as f32;
            let light = face_light(face);
            match stuff {
                Stuff::Sack => {
                    // Bulging cloth: lit across its middle, a few creases running down.
                    let bulge = (across * std::f32::consts::PI).sin() * 0.16;
                    let crease = if (i * 7 + k as i32 * 13) % 23 == 0 {
                        -0.12
                    } else {
                        0.0
                    };
                    burlap(light - 0.1 + bulge + crease, i, j)
                }
                Stuff::Ruffle => {
                    let fold = ((i as f32 * 0.9).sin() * 0.5 + 0.5) * 0.25;
                    burlap(light + fold - 0.05, i, j + 40)
                }
                Stuff::Rope => {
                    let twist = (((i + j) as f32) / 3.0).fract();
                    let l = 0.35 + 0.55 * (twist * std::f32::consts::PI).sin();
                    ramp(
                        &[0x6a4a10, 0xa8781c, 0xd8a838, 0xf2cc60, 0xfff0b0],
                        l * light * 1.3,
                    )
                }
                Stuff::Gift => {
                    // A red ribbon across both ways over green paper; a bow's knot on top.
                    let ribbon = (across - 0.5).abs() < 0.14
                        || (face >= 2 && (down - 0.5).abs() < 0.14)
                        || (face < 2 && (down - 0.5).abs() < 0.14);
                    if ribbon {
                        ramp(&[0x7a0e16, 0xc01e2a, 0xee4a52, 0xff8a8e], light + 0.1)
                    } else {
                        let dot = (i % 4 == 1 && j % 4 == 1) as i32 as f32 * 0.2;
                        ramp(&[0x0c3a1c, 0x16602e, 0x1f8a40, 0x3cb862], light + dot)
                    }
                }
                Stuff::Cane => {
                    let stripe = (((i + j) as f32) / 4.0).floor() as i32 % 2 == 0;
                    let base = if stripe { 0xd42a35 } else { 0xf7f3ee };
                    add_light(hex(base), [40.0, 40.0, 40.0], light - 0.6)
                }
            }
        });
    }
    c
}

/// Santa's Sack: a big burlap sack over your back, leaning a little, gathered and tied at the
/// neck with gold rope, a present and a candy cane poking out of the top. It sways as you walk.
fn sack_item(texture: &Canvas) -> Value {
    let uvs = sack_uvs();
    let cubes = |part: &str| -> Vec<Value> {
        SACK.iter()
            .zip(&uvs)
            .filter(|((p, _, _, _), _)| *p == part)
            .map(|(&(_, origin, size, _), &uv)| cube(origin, size, uv))
            .collect()
    };
    json!({
        "format": 1, "id": "santa_sack", "slot": "weapon", "bone": "body",
        "attach": { "offset": [0.5, 1.0, 2.1], "rotation": [0.0, 0.0, -12.0], "scale": 1.0 / XMAS_UNIT },
        "texture": { "png": b64(&texture.png()) },
        "parts": [{
            "name": "sack", "pivot": [0, 0, 0], "cubes": cubes("sack"),
            "children": [
                { "name": "gift", "pivot": [-6.0, -4.0, 12.0], "rotation": [8.0, 24.0, -14.0], "cubes": cubes("gift") },
                { "name": "cane", "pivot": [7.0, -3.0, 13.0], "rotation": [-6.0, 0.0, 18.0], "cubes": cubes("cane") },
            ],
        }],
        "animations": [
            { "part": "sack", "motion": "swing", "axis": "z", "amplitude": 2.5, "period": 52 },
        ],
    })
}

// ---------------------------------------------------------------- cards

/// A wearer in dark clothes on a 64×64 skin, so the item stands out.
fn wearer_skin() -> wisp_cosmetics::Texture {
    let mut c = Canvas::new(64, 64);
    let cloth = hex(0x1d1d22);
    let trim = hex(0x5a5a64);
    let hair = hex(0x141417);
    let face = hex(0xd9a183);
    fill_box(&mut c, 0, 0, 8, 8, 8, hair);
    let (x, y, _, _) = box_faces(0, 0, 8, 8, 8)[3];
    c.rect(x, y + 3, 8, 5, face);
    c.rect(x + 1, y + 4, 2, 1, hex(0xf2f0ff));
    c.rect(x + 5, y + 4, 2, 1, hex(0xf2f0ff));
    c.set(x + 2, y + 4, hex(0x3a3a48));
    c.set(x + 5, y + 4, hex(0x3a3a48));
    fill_box(&mut c, 16, 16, 8, 12, 4, cloth);
    for (u, v) in [(40, 16), (32, 48)] {
        fill_box(&mut c, u, v, 4, 12, 4, cloth);
        for (x, y, w, _) in box_faces(u, v, 4, 12, 4).into_iter().skip(2) {
            c.rect(x, y + 3, w, 1, trim);
            c.rect(x, y + 10, w, 2, face);
        }
    }
    for (u, v) in [(0, 16), (16, 48)] {
        fill_box(&mut c, u, v, 4, 12, 4, hex(0x121215));
    }
    wisp_cosmetics::Texture {
        width: 64,
        height: 64,
        rgba: c.px,
    }
}

/// The card's background: a graphite night, a glow behind the subject, a few stars.
fn backdrop(w: u32, h: u32, glow: [f32; 3], seed: u32) -> Canvas {
    let mut c = Canvas::new(w, h);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let t = y as f32 / h as f32;
            c.set(x, y, mix(hex(0x0c0c0f), hex(0x1b1b21), t));
            let (dx, dy) = (
                (x as f32 - w as f32 * 0.5) / w as f32,
                (y as f32 - h as f32 * 0.55) / h as f32,
            );
            let g = (1.0 - (dx * dx * 2.2 + dy * dy * 2.8).sqrt() * 1.6).max(0.0);
            c.blend(x, y, glow, g * 0.4);
            let n = noise(x, y, seed);
            if n > 0.9975 {
                c.blend(x, y, [255.0, 255.0, 255.0], 0.5 + (n - 0.9975) * 200.0);
            }
        }
    }
    c
}

fn composite(card: &mut Canvas, layer: &preview::Image, ox: i32, oy: i32) {
    for y in 0..layer.height as i32 {
        for x in 0..layer.width as i32 {
            let i = ((y as u32 * layer.width + x as u32) * 4) as usize;
            let a = layer.rgba[i + 3] as f32 / 255.0;
            if a > 0.0 {
                let c = [
                    layer.rgba[i] as f32,
                    layer.rgba[i + 1] as f32,
                    layer.rgba[i + 2] as f32,
                ];
                card.blend(ox + x, oy + y, c, a);
            }
        }
    }
}

/// `item` on the wearer, `yaw` degrees round (180 shows the back), `zoom` times the card's height:
/// `drop` of the figure's size below the card's bottom, its middle `centre` of the way across.
#[allow(clippy::too_many_arguments)]
fn framed_card(
    item: &Item,
    w: u32,
    h: u32,
    yaw: f32,
    zoom: f32,
    drop: f32,
    centre: f32,
    glow: [f32; 3],
) -> Canvas {
    let skin = wearer_skin();
    let body = preview::player_model(false);
    // Head items reach above the head's top, where the picture ends: the figure is lowered
    // a quarter of a block for them.
    let lower = if item.slot == wisp_cosmetics::Slot::Head {
        Mat4::from_translation(glam::Vec3::new(0.0, 0.25, 0.0))
    } else {
        Mat4::IDENTITY
    };
    let mut draws = vec![Draw {
        root: &body,
        base: lower,
        texture: &skin,
        poses: Vec::new(),
    }];
    draws.extend(Draw::item(item, &body, 0.0).into_iter().map(|mut d| {
        d.base = lower * d.base;
        d
    }));
    let size = (h as f32 * zoom) as u32;
    let figure = preview::render(&draws, size, size, yaw, 12.0);
    let mut card = backdrop(w, h, glow, item.id.len() as u32 * 31);
    composite(
        &mut card,
        &figure,
        (w as f32 * centre) as i32 - size as i32 / 2,
        h as i32 - size as i32 + (size as f32 * drop) as i32,
    );
    card
}

/// The cape's art laid flat beside the model: its outer face, `height` pixels tall.
fn flat_cape(card: &mut Canvas, texture: &Canvas, ox: i32, oy: i32, height: i32) {
    let (cw, ch, cd) = CAPE;
    let (fx, fy, w, h) = box_faces(0, 0, cw, ch, cd)[3];
    let scale = height as f32 / h as f32;
    for y in 0..height {
        for x in 0..(w as f32 * scale) as i32 {
            let p = texture.get(
                fx + (x as f32 / scale) as i32,
                fy + (y as f32 / scale) as i32,
            );
            card.set(ox + x, oy + y, p);
        }
    }
}

/// A cape's card: the wearer's back on the left, the cape laid flat on the right.
fn cape_card(item: &Item, texture: &Canvas, w: u32, h: u32, glow: [f32; 3]) -> Canvas {
    let mut card = framed_card(item, w, h, 162.0, 1.9, 0.45, 0.33, glow);
    let tall = h as i32 - 24;
    flat_cape(&mut card, texture, (w as f32 * 0.66) as i32, 12, tall);
    card
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn write(dir: &Path, name: &str, bytes: &[u8]) {
    std::fs::write(dir.join(name), bytes).unwrap();
    println!("wrote {name} ({} KiB)", bytes.len() / 1024);
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .expect("usage: shop_items <output folder>");
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir).unwrap();
    // The card picture's shape in the launcher, and the Store's backdrop (the window's 16:9).
    let (cw, ch) = (480, 156);
    let (bw, bh) = (1600, 900);
    let glow = [200.0, 200.0, 215.0];

    let id = "founders_cape";
    let frames = founders_frames();
    let texture = &frames[0];
    let file = serde_json::to_vec_pretty(&cape_item(id, &frames, CAPE_FRAMETIME, true)).unwrap();
    let item = Item::parse(&file).unwrap_or_else(|e| panic!("{id}: {e}"));
    write(dir, &format!("{id}.json"), &file);
    write(
        dir,
        &format!("{id}.png"),
        &cape_card(&item, texture, cw, ch, glow).png(),
    );
    // The Store's backdrop: the wearer's back in the middle, between the item's name on the
    // left and the list on the right.
    let banner = framed_card(&item, bw, bh, 160.0, 0.82, 0.04, 0.55, glow);
    write(dir, &format!("banner_{id}.png"), &banner.png());

    let id = "lavender_cape";
    let frames = lavender_frames();
    let file =
        serde_json::to_vec_pretty(&cape_item(id, &frames, LAVENDER_FRAMETIME, false)).unwrap();
    let item = Item::parse(&file).unwrap_or_else(|e| panic!("{id}: {e}"));
    write(dir, &format!("{id}.json"), &file);
    write(
        dir,
        &format!("{id}.png"),
        &cape_card(&item, &frames[0], cw, ch, [180.0, 130.0, 220.0]).png(),
    );
    if let Ok(out) = std::env::var("WISP_SHOP_PREVIEW") {
        let out = Path::new(&out);
        std::fs::create_dir_all(out).unwrap();
        for k in [0, 12, 24, 36] {
            write(out, &format!("{id}_frame{k}.png"), &frames[k].png());
        }
    }

    // The Minecraft-style items: (id, item file, yaw, zoom, drop, glow behind).
    let demon = demon_texture();
    let angel = angel_texture();
    let bamboo = bamboo_texture();
    let straw = hat_texture();
    let items = [
        (
            "demon_wings",
            demon_item(&demon),
            198.0,
            0.95,
            0.1,
            [190.0, 40.0, 50.0],
        ),
        (
            "angel_wings",
            angel_item(&angel),
            198.0,
            0.95,
            0.1,
            [235.0, 225.0, 180.0],
        ),
        (
            "bamboo_sword",
            bamboo_item(&bamboo),
            195.0,
            1.6,
            0.35,
            [110.0, 200.0, 90.0],
        ),
        (
            "stray_head",
            hat_item(&straw),
            25.0,
            2.6,
            0.5,
            [230.0, 200.0, 140.0],
        ),
        (
            "devil_horns",
            horns_item(&horn_texture()),
            25.0,
            2.4,
            0.52,
            [200.0, 50.0, 50.0],
        ),
        (
            "angel_halo",
            halo_item(&halo_texture()),
            25.0,
            2.6,
            0.5,
            [240.0, 210.0, 120.0],
        ),
        (
            "santa_hat",
            santa_hat_item(&santa_hat_texture()),
            25.0,
            2.2,
            0.62,
            [220.0, 60.0, 70.0],
        ),
        (
            "santa_sack",
            sack_item(&sack_texture()),
            200.0,
            1.5,
            0.3,
            [230.0, 180.0, 90.0],
        ),
    ];
    for (id, value, yaw, zoom, drop, glow) in items {
        let file = serde_json::to_vec_pretty(&value).unwrap();
        let item = Item::parse(&file).unwrap_or_else(|e| panic!("{id}: {e}"));
        write(dir, &format!("{id}.json"), &file);
        let card = framed_card(&item, cw, ch, yaw, zoom, drop, 0.5, glow);
        write(dir, &format!("{id}.png"), &card.png());
        if let Ok(out) = std::env::var("WISP_SHOP_PREVIEW") {
            preview_sheet(&item, Path::new(&out), id);
        }
    }
}

/// With `WISP_SHOP_PREVIEW` set: a sheet of the item from four sides at four moments of its
/// animations, to look the art over.
fn preview_sheet(item: &Item, dir: &Path, id: &str) {
    let size = 360;
    let mut sheet = backdrop(size * 4, size * 4, [120.0, 120.0, 140.0], 7);
    let skin = wearer_skin();
    let body = preview::player_model(false);
    for (row, ticks) in [0.0f32, 15.0, 30.0, 45.0].into_iter().enumerate() {
        for (col, yaw) in [200.0f32, 150.0, 90.0, 30.0].into_iter().enumerate() {
            let mut draws = vec![Draw {
                root: &body,
                base: Mat4::IDENTITY,
                texture: &skin,
                poses: Vec::new(),
            }];
            draws.extend(Draw::item(item, &body, ticks));
            let figure = preview::render(&draws, size, size, yaw, 12.0);
            composite(
                &mut sheet,
                &figure,
                col as i32 * size as i32,
                row as i32 * size as i32,
            );
        }
    }
    std::fs::create_dir_all(dir).unwrap();
    write(dir, &format!("{id}_sheet.png"), &sheet.png());
}
