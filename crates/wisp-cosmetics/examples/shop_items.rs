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

fn cape_item(id: &str, frames: &[Canvas]) -> Value {
    let (w, h, d) = CAPE;
    json!({
        "format": 1,
        "id": id,
        "slot": "back",
        "bone": "body",
        // A night-sky cape stays bright in the dark.
        "glow": true,
        // Behind the back, turned round like vanilla's cape, at a twelfth of the scale for its
        // 12× texture.
        "attach": { "offset": [0, 0, 2], "rotation": [0, 180, 0], "scale": 1.0 / RES as f32 },
        "texture": { "png": b64(&strip(frames).png()), "frames": frames.len(), "frametime": CAPE_FRAMETIME },
        "parts": [{
            "name": "cape", "pivot": [0, 0, 0], "rotation": [-7, 0, 0],
            "cubes": [{ "origin": [-w / 2, 0, -d], "size": [w, h, d], "uv": [0, 0] }]
        }],
        "animations": [{ "part": "cape", "motion": "swing", "axis": "x", "amplitude": 3, "period": 70, "phase": 0 }]
    })
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

/// Darkens the opaque texels of the `w`×`h` region at (0, 0) that touch a transparent one: the
/// one-texel outline Minecraft's pixel art has.
fn outline(c: &mut Canvas, w: i32, h: i32, colour: Rgba) {
    let solid =
        |c: &Canvas, x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && c.get(x, y)[3] > 0;
    let edge: Vec<(i32, i32)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            solid(c, x, y)
                && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|(dx, dy)| !solid(c, x + dx, y + dy))
        })
        .collect();
    for (x, y) in edge {
        let p = c.get(x, y);
        c.set(x, y, mix([p[0], p[1], p[2], 255], colour, 0.7));
    }
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

/// Texels per design unit: the HD items are drawn 8× finer than Minecraft's pixel, the most the
/// format's 256-texel textures hold.
const HD: i32 = 8;
const HDF: f32 = HD as f32;

/// A wing's design: `WING.0`×`WING.1` units (drawn at `HD` texels each, 256×192), the shoulder on
/// the left (row `WING_ROOT`), the tip on the right. Columns left of `WING_JOINT` are the inner
/// panel, the rest the outer one, which bends as the wing beats.
const WING: (i32, i32) = (32, 24);
const WING_ROOT: f32 = 7.0;
const WING_JOINT: f32 = 13.0;
/// Model pixels a design unit covers: a span of 18 pixels, within the 24-pixel reach.
const WING_TEXEL: f32 = 0.55;

/// The distance from `p` to the polygon's edge.
fn to_edge(poly: &[[f32; 2]], p: [f32; 2]) -> f32 {
    (0..poly.len())
        .map(|i| to_segment(p, poly[i], poly[(i + 1) % poly.len()]))
        .fold(f32::MAX, f32::min)
}

/// Each texel of a `WING`-sized HD canvas, with its point in design units.
fn each_wing_texel(mut f: impl FnMut(i32, i32, [f32; 2])) {
    for y in 0..WING.1 * HD {
        for x in 0..WING.0 * HD {
            f(x, y, [(x as f32 + 0.5) / HDF, (y as f32 + 0.5) / HDF]);
        }
    }
}

/// Demon Wings in HD: a bat's wing. Round black bones lit along their length, knuckles at the
/// wrist, an ivory claw on the wrist and at the tip; a leathery membrane, dark by the body and
/// glowing ember-red where it thins towards its scalloped edge, with veins branching through it.
fn demon_wing() -> Canvas {
    let (w, h) = (WING.0 * HD, WING.1 * HD);
    let mut c = Canvas::new(w as u32, h as u32);
    let root = [0.5, 7.5];
    let wrist = [12.5, 2.6];
    let tips = [
        [31.4, 1.2],
        [30.5, 10.5],
        [23.5, 18.0],
        [14.0, 23.0],
        [3.5, 19.5],
    ];
    let mut poly = vec![root, wrist, tips[0]];
    for pair in tips.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let ctrl = lerp2(lerp2(a, b, 0.5), wrist, 0.32);
        for k in 1..=24 {
            let t = k as f32 / 24.0;
            poly.push(lerp2(lerp2(a, ctrl, t), lerp2(ctrl, b, t), t));
        }
    }
    // (from, to, radius at `from`, radius at `to`)
    let bones: [([f32; 2], [f32; 2], f32, f32); 6] = [
        (root, wrist, 0.85, 0.6),
        (wrist, tips[0], 0.5, 0.22),
        (wrist, tips[1], 0.42, 0.18),
        (wrist, tips[2], 0.42, 0.18),
        (wrist, tips[3], 0.42, 0.18),
        ([1.0, 8.0], tips[4], 0.4, 0.18),
    ];
    // How deep into the nearest bone `p` is (0 on its axis, 1 at its surface) and which side.
    let bone_at = |p: [f32; 2]| -> Option<(f32, f32)> {
        let mut best: Option<(f32, f32)> = None;
        for &(a, b, ra, rb) in &bones {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let t =
                (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            let (qx, qy) = (a[0] + t * dx, a[1] + t * dy);
            let r = ra + (rb - ra) * t;
            let d = (p[0] - qx).hypot(p[1] - qy) / r;
            // Which side of the axis: the light comes from above.
            let side = ((p[0] - qx) * -dy + (p[1] - qy) * dx).signum() * dx.signum();
            if d < 1.0 && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, side));
            }
        }
        // A knuckle at the wrist.
        let k = (p[0] - wrist[0]).hypot(p[1] - wrist[1]) / 0.95;
        if k < 1.0 && best.is_none_or(|(bd, _)| k < bd) {
            best = Some((k, (p[1] - wrist[1]).signum()));
        }
        best
    };
    // The distance to the nearest bone axis, for the shadow the bones throw on the skin.
    let near_bone = |p: [f32; 2]| {
        bones
            .iter()
            .map(|&(a, b, ..)| to_segment(p, a, b))
            .fold(f32::MAX, f32::min)
    };
    each_wing_texel(|x, y, p| {
        if let Some((d, side)) = bone_at(p) {
            // A round bone: bright on its upper flank, black at its rim.
            let round = (1.0 - d * d).sqrt();
            let lit = (round * 0.8 + if side < 0.0 { 0.25 } else { -0.1 } * round).clamp(0.0, 1.0);
            let mut col = mix(hex(0x0c0506), hex(0x6e4440), lit);
            if d < 0.25 && side < 0.0 {
                col = add_light(col, [120.0, 90.0, 85.0], (0.25 - d) * 1.6);
            }
            col = add_light(
                col,
                [30.0, 20.0, 20.0],
                (fbm(p[0] * 6.0, p[1] * 6.0, 705) - 0.5) * 1.2,
            );
            c.set(x, y, col);
        } else if inside(&poly, p) {
            let t = ((p[0] - root[0]).hypot(p[1] - root[1]) / 30.0).clamp(0.0, 1.0);
            let skin = fbm(p[0] * 1.1, p[1] * 1.1, 702);
            let mut col = mix(
                hex(0x22040a),
                hex(0xa01c26),
                (t * 0.9 + (skin - 0.5) * 0.35).clamp(0.0, 1.0),
            );
            // Thin skin glows towards the edge, as if lit from behind.
            let edge = to_edge(&poly, p);
            col = add_light(
                col,
                [255.0, 70.0, 40.0],
                (1.0 - edge / 2.2).max(0.0) * 0.35 * t,
            );
            // Veins: the ridges of a noise, darker, branching between the fingers.
            let ridge = 1.0 - (fbm(p[0] * 0.9, p[1] * 0.9, 709) * 2.0 - 1.0).abs();
            if ridge > 0.9 {
                col = mix(col, hex(0x2a0307), (ridge - 0.9) * 7.0);
            }
            // Fine wrinkles of the leather.
            let wrinkle = vnoise(p[0] * 14.0, p[1] * 3.0, 711);
            col = add_light(col, [40.0, 10.0, 10.0], (wrinkle - 0.5) * 0.5);
            // The bones' shadow and the skin pulled taut along them.
            let nb = near_bone(p);
            if nb < 1.4 {
                col = mix(col, hex(0x180205), (1.4 - nb) / 1.4 * 0.55);
            }
            c.set(x, y, col);
        }
    });
    // Claws: an ivory hook on the wrist and a small one at the tip, shaded round.
    for (base, point, r) in [(wrist, [11.7, 0.15], 0.5), (tips[0], [31.9, 0.1], 0.3)] {
        each_wing_texel(|x, y, p| {
            let (dx, dy) = (point[0] - base[0], point[1] - base[1]);
            let t = (((p[0] - base[0]) * dx + (p[1] - base[1]) * dy) / (dx * dx + dy * dy))
                .clamp(0.0, 1.0);
            // A hook: the axis bends forward as it rises.
            let bend = 0.35 * r * (t * std::f32::consts::PI).sin();
            let q = [base[0] + t * dx - bend, base[1] + t * dy];
            let d = (p[0] - q[0]).hypot(p[1] - q[1]) / (r * (1.0 - t * 0.92));
            if d < 1.0 && t > 0.05 {
                let round = (1.0 - d * d).sqrt();
                let col = mix(hex(0x6b5e48), hex(0xf2e9d2), round * (1.0 - t * 0.5));
                c.set(x, y, mix(col, hex(0x2a2218), (t - 0.75).max(0.0) * 3.0));
            }
        });
    }
    outline(&mut c, w, h, hex(0x0a0103));
    c
}

/// One feather's colour: `base` with its rounded flanks shaded, the quill down its middle, fine
/// barbs angled from it, a dark seam where it meets the next, and the shade of the row above
/// `shadow` units over it.
fn feather(base: Rgba, fx: f32, fy: f32, len: f32, shadow: f32, quill: bool) -> Rgba {
    let edge = (2.0 * fx - 1.0).abs();
    let mut col = mix(base, hex(0xbfc8da), edge.powi(3) * 0.55);
    if quill && (fx - 0.5).abs() < 0.04 && fy > 0.05 && fy < 0.93 {
        col = mix(col, hex(0xe9dcbc), 0.7);
    }
    let barb = (fy * len * 7.0 - edge * 2.5).fract();
    if barb < 0.18 {
        col = mix(col, hex(0xd9dfea), 0.5);
    }
    if fx < 0.045 {
        col = mix(col, hex(0x9da8c0), 0.8);
    }
    // The tip a little cooler.
    col = mix(col, hex(0xdfe5f0), ((fy - 0.75) * 3.0).clamp(0.0, 0.5));
    if shadow < 0.9 {
        col = mix(col, hex(0xa9b3c9), (1.0 - shadow / 0.9) * 0.65);
    }
    col
}

/// Angel Wings in HD: white feathers in three rows hanging from a gold-edged arm. Rounded
/// coverts at the top, a row of short feathers under them, then the long primaries, longest in
/// the middle of the wing; every feather has its quill and barbs, and each row shades the one
/// under it.
fn angel_wing() -> Canvas {
    let (w, h) = (WING.0 * HD, WING.1 * HD);
    let mut c = Canvas::new(w as u32, h as u32);
    let pi = std::f32::consts::PI;
    let top = |x: f32| 6.0 - 5.0 * (pi * x / 36.0).sin();
    const LONG: [f32; 11] = [
        9.0, 11.0, 13.0, 15.0, 16.0, 17.0, 17.0, 16.0, 14.0, 11.0, 7.0,
    ];
    let round = |f: f32| (1.0 - (2.0 * f - 1.0).powi(2)).max(0.0).sqrt();
    each_wing_texel(|x, y, [px, py]| {
        let t = top(px);
        if py < t {
            return;
        }
        let fc = ((px + 2.0) / 4.0).fract();
        let covert = t + 5.0 + round(fc) * 1.1;
        let fm = ((px + 1.0) / 3.0).fract();
        let middle = t + 9.0 + round(fm) * 1.2;
        let k = ((px / 3.0) as usize).min(LONG.len() - 1);
        let fp = (px / 3.0).fract();
        // Pointed tips, the outer side a little longer so each points out.
        let long = t + LONG[k] + (1.0 - (2.0 * fp - 1.0).abs()).powf(0.6) * 1.3 + fp * 0.5;
        let col = if py <= covert {
            let mut col = feather(hex(0xfffaf0), fc, (py - t) / (covert - t), 5.0, 9.0, false);
            // Gold along the arm, fading down.
            let g = (py - t) / 0.9;
            if g < 1.0 {
                col = mix(hex(0xe2b54a), col, g * g);
                col = add_light(col, [255.0, 240.0, 190.0], (1.0 - g) * 0.25);
            }
            col
        } else if py <= middle {
            feather(
                hex(0xf7f8fc),
                fm,
                (py - t) / (middle - t),
                9.0,
                py - covert,
                true,
            )
        } else if py <= long {
            feather(
                hex(0xf0f3f9),
                fp,
                (py - t) / (long - t),
                LONG[k],
                py - middle,
                true,
            )
        } else {
            return;
        };
        c.set(x, y, col);
    });
    outline(&mut c, w, h, hex(0x7a87a3));
    c
}

/// A pair of wings on the back from one wing's HD art: each wing two panels, the outer one
/// bending after the inner as they beat, slowly, swept back and raised a little.
fn wings_item(id: &str, texture: &Canvas, glow: bool, beat: u32) -> Value {
    let (tw, th) = (texture.w as f32, texture.h as f32);
    // A panel from design column `u0` to `u1` reaching to +x (`side` 1) or -x, both faces,
    // with column `pivot` and row `WING_ROOT` at the part's pivot.
    let panel = |side: f32, u0: f32, u1: f32, pivot: f32| -> Vec<Value> {
        let corner = |u: f32, v: f32| {
            [
                side * (u - pivot) * WING_TEXEL,
                (v - WING_ROOT) * WING_TEXEL,
                0.0,
            ]
        };
        let uv = |u: f32, v: f32| [u * HDF / tw, v * HDF / th];
        let h = WING.1 as f32;
        let pts = [
            corner(u0, 0.0),
            corner(u1, 0.0),
            corner(u1, h),
            corner(u0, h),
        ];
        let uvs = [uv(u0, 0.0), uv(u1, 0.0), uv(u1, h), uv(u0, h)];
        let one = quad(pts, uvs);
        let other = quad(
            [pts[3], pts[2], pts[1], pts[0]],
            [uvs[3], uvs[2], uvs[1], uvs[0]],
        );
        vec![one, other]
    };
    let wing = |name: &str, side: f32| -> Value {
        json!({
            "name": name, "pivot": [side * 1.5, 1.5, 2.6], "rotation": [0.0, side * -22.0, side * -10.0],
            "quads": panel(side, 0.0, WING_JOINT, 0.0),
            "children": [{
                "name": format!("{name}_tip"),
                "pivot": [side * WING_JOINT * WING_TEXEL, 0.0, 0.0],
                "rotation": [0.0, side * 10.0, 0.0],
                "quads": panel(side, WING_JOINT, WING.0 as f32, WING_JOINT),
            }],
        })
    };
    json!({
        "format": 1, "id": id, "slot": "back", "bone": "body", "glow": glow,
        "texture": { "png": b64(&texture.png()) },
        "parts": [wing("left_wing", 1.0), wing("right_wing", -1.0)],
        "animations": [
            { "part": "left_wing", "motion": "swing", "axis": "y", "amplitude": 12, "period": beat },
            { "part": "right_wing", "motion": "swing", "axis": "y", "amplitude": -12, "period": beat },
            { "part": "left_wing_tip", "motion": "swing", "axis": "y", "amplitude": 9, "period": beat, "phase": 0.14 },
            { "part": "right_wing_tip", "motion": "swing", "axis": "y", "amplitude": -9, "period": beat, "phase": 0.14 },
        ],
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

/// The hat's cone: segments round it, the rim's radius and height (model pixels, y down: the
/// head's top is -8), the copper band's depth, and the apexes outside and underneath. The brim
/// is twice the head's width and the cone clears the head's corners.
const HAT_SEGMENTS: usize = 32;
const HAT_RIM: f32 = 8.0;
const HAT_RIM_Y: f32 = -8.0;
const HAT_BAND: f32 = 0.45;
const HAT_APEX_Y: f32 = -13.0;
const HAT_UNDER_APEX_Y: f32 = -12.2;
/// The tiny ring the cone closes to at its top, under the knot.
const HAT_TOP: f32 = 0.25;
/// The texture's rows: the outside (apex to rim), the band, the underside (rim to apex).
const HAT_OUT: f32 = 160.0;
const HAT_BAND_END: f32 = 176.0;

/// The hat's 256×256 texture: woven palm-leaf strips running from the apex to the rim, each its
/// own shade with fibres along it, rows of thread stitched round it, a sheen where the light
/// falls; a copper rim wound with red thread; the darker underside with its bamboo rings; a
/// white cord knot at the apex.
fn straw_texture() -> Canvas {
    let mut c = Canvas::new(256, 256);
    let (out, band) = (HAT_OUT as i32, HAT_BAND_END as i32);
    for y in 0..256 {
        for x in 0..256 {
            let strip = x / 4;
            let col = if y < 10 {
                mix(hex(0xf7f2e8), hex(0xd9d0be), noise(x / 2, y / 2, 921))
            } else if y < out {
                let v = y as f32 / out as f32;
                let mut col = mix(hex(0xf4e4bf), hex(0xcaa872), v * 0.75);
                col = mix(col, hex(0xb8935c), noise(strip, 0, 923) * 0.35);
                let fibre = vnoise(x as f32 * 1.1, y as f32 * 0.07, 925);
                col = add_light(col, [60.0, 50.0, 30.0], (fibre - 0.5) * 0.9);
                if x % 4 == 0 {
                    col = mix(col, hex(0x8f6e42), 0.4);
                }
                // A sheen a third of the way down.
                col = add_light(
                    col,
                    [255.0, 250.0, 230.0],
                    (-((v - 0.33) / 0.12).powi(2)).exp() * 0.18,
                );
                // Rows of stitching: pale thread over the strips, a groove beside it.
                let row = y % 20;
                if row < 2 {
                    col = if (x + (y / 20) * 3) % 6 < 3 {
                        mix(hex(0xfdf7e6), col, row as f32 * 0.3)
                    } else {
                        mix(col, hex(0x7d5f38), 0.35)
                    };
                } else if row == 2 {
                    col = mix(col, hex(0x8c6c40), 0.2);
                }
                col
            } else if y < band {
                // Copper, round, wound with red thread.
                let k = (y - out) as f32 / (band - out - 1) as f32;
                let shade = (k * std::f32::consts::PI).sin();
                let thread = (x + (y - out)) % 10 < 3;
                if thread {
                    mix(hex(0x7a1a10), hex(0xe2604a), shade)
                } else {
                    mix(hex(0x5e2a12), hex(0xeaa36a), shade)
                }
            } else {
                // Underneath: darker straw, the strips running in to the crown, a bamboo ring
                // every 16 rows.
                let v = (y - band) as f32 / (255 - band) as f32;
                let mut col = mix(hex(0xb08f60), hex(0x6e5233), v);
                col = mix(col, hex(0x5e4428), noise(strip, 2, 927) * 0.35);
                col = add_light(
                    col,
                    [40.0, 30.0, 20.0],
                    (vnoise(x as f32 * 1.1, y as f32 * 0.1, 929) - 0.5) * 0.8,
                );
                let ring = (y - band) % 16;
                if ring < 2 {
                    col = mix(col, hex(0x4a3418), 0.6);
                } else if ring == 2 {
                    col = add_light(col, [60.0, 50.0, 30.0], 0.4);
                }
                col
            };
            c.set(x, y, col);
        }
    }
    c
}

/// The Stray Head: a smooth conical straw hat sized for Minecraft's head, double-sided so it
/// reads from above and below, with a knot of white cord at its top.
fn stray_item(texture: &Canvas) -> Value {
    let n = HAT_SEGMENTS;
    let row = |t: f32| t / 256.0;
    let mut quads = Vec::new();
    let mut both = |p: [[f32; 3]; 4], uv: [[f32; 2]; 4]| {
        quads.push(quad(p, uv));
        quads.push(quad([p[3], p[2], p[1], p[0]], [uv[3], uv[2], uv[1], uv[0]]));
    };
    let ring = |a: f32, r: f32, y: f32| [a.cos() * r, y, a.sin() * r];
    let band_y = HAT_RIM_Y + HAT_BAND;
    for i in 0..n {
        let (u0, u1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
        let (a0, a1) = (u0 * std::f32::consts::TAU, u1 * std::f32::consts::TAU);
        both(
            [
                ring(a1, HAT_TOP, HAT_APEX_Y),
                ring(a0, HAT_TOP, HAT_APEX_Y),
                ring(a0, HAT_RIM, HAT_RIM_Y),
                ring(a1, HAT_RIM, HAT_RIM_Y),
            ],
            [
                [u1, row(0.5)],
                [u0, row(0.5)],
                [u0, row(HAT_OUT - 0.5)],
                [u1, row(HAT_OUT - 0.5)],
            ],
        );
        both(
            [
                ring(a1, HAT_RIM, HAT_RIM_Y),
                ring(a0, HAT_RIM, HAT_RIM_Y),
                ring(a0, HAT_RIM, band_y),
                ring(a1, HAT_RIM, band_y),
            ],
            [
                [u1, row(HAT_OUT + 0.5)],
                [u0, row(HAT_OUT + 0.5)],
                [u0, row(HAT_BAND_END - 0.5)],
                [u1, row(HAT_BAND_END - 0.5)],
            ],
        );
        both(
            [
                ring(a1, HAT_RIM, band_y),
                ring(a0, HAT_RIM, band_y),
                ring(a0, HAT_TOP, HAT_UNDER_APEX_Y),
                ring(a1, HAT_TOP, HAT_UNDER_APEX_Y),
            ],
            [
                [u1, row(HAT_BAND_END + 0.5)],
                [u0, row(HAT_BAND_END + 0.5)],
                [u0, row(255.5)],
                [u1, row(255.5)],
            ],
        );
    }
    json!({
        "format": 1, "id": "stray_head", "slot": "head", "bone": "head",
        "texture": { "png": b64(&texture.png()) },
        "parts": [{
            "name": "hat", "pivot": [0, 0, 0],
            // The knot at the top (its texels are the cord's cream of the apex rows).
            "cubes": [{ "origin": [-1.0, HAT_APEX_Y - 0.8, -1.0], "size": [2, 1, 2], "uv": [248, 0] }],
            "quads": quads,
        }],
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
    let mut draws = vec![Draw {
        root: &body,
        base: Mat4::IDENTITY,
        texture: &skin,
        poses: Vec::new(),
    }];
    draws.extend(Draw::item(item, &body, 0.0));
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
    let file = serde_json::to_vec_pretty(&cape_item(id, &frames)).unwrap();
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

    // The Minecraft-style items: (id, item file, yaw, zoom, drop, glow behind).
    let demon = demon_wing();
    let angel = angel_wing();
    let bamboo = bamboo_texture();
    let straw = straw_texture();
    let items = [
        (
            "demon_wings",
            wings_item("demon_wings", &demon, false, 80),
            200.0,
            1.25,
            0.2,
            [190.0, 40.0, 50.0],
        ),
        (
            "angel_wings",
            wings_item("angel_wings", &angel, true, 64),
            200.0,
            1.25,
            0.2,
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
            stray_item(&straw),
            25.0,
            2.6,
            0.6,
            [230.0, 200.0, 140.0],
        ),
    ];
    for (id, value, yaw, zoom, drop, glow) in items {
        let file = serde_json::to_vec_pretty(&value).unwrap();
        let item = Item::parse(&file).unwrap_or_else(|e| panic!("{id}: {e}"));
        write(dir, &format!("{id}.json"), &file);
        let card = framed_card(&item, cw, ch, yaw, zoom, drop, 0.5, glow);
        write(dir, &format!("{id}.png"), &card.png());
    }
}
