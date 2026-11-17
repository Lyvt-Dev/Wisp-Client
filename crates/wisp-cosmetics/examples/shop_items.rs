//! Builds the launcher shop's items and their card art (`crates/wisp-shop/assets`).
//!
//! `cargo run -p wisp-cosmetics --example shop_items -- crates/wisp-shop/assets`
//!
//! Every item is checked by `Item::parse` and its card is drawn with `preview::render`, so a card
//! shows exactly what the game draws. The shop has one item for now: the Wisp Founders Cape.
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

/// The cape at 8× Minecraft's: an 80×128×8 box at an eighth of the scale, so it has the size
/// of vanilla's 10×16×1 cape with 64 texels where vanilla has one. Its unfolded faces take
/// 176×136 texels (vanilla's cape layout at 8×, cropped to the cape), under the format's 256.
const CAPE: (i32, i32, i32) = (80, 128, 8);
const CAPE_TEX: (u32, u32) = (176, 136);
/// Frames and the game ticks each shows: a 2.4 s loop.
const CAPE_FRAMES: u32 = 24;
const CAPE_FRAMETIME: u32 = 2;

/// Wisp's own mark (a white feather on black), as the launcher and the game show it.
const WISP_MARK: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../wisp-theme/icons/wisp-256.png"
));

/// The founders' gold: the hem and the sparks that cross it.
const GOLD: Rgba = hex(0xf2c14e);

/// The Wisp Founders Cape: Wisp's feather large on a black night, at 8×, animated: smoke
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
    let mw = 50.0;
    let mh = mw * (y1 - y0) as f32 / (x1 - x0) as f32;
    let centre_y = 54.0;
    let (mx, my) = ((w as f32 - mw) / 2.0, centre_y - mh / 2.0);
    let (rcx, rcy, radius) = (w as f32 / 2.0, centre_y, (mh / 2.0 + 6.0).min(36.0));
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
    let r = 8;
    let mut halo = vec![0.0f32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let (mut s, mut n) = (0.0, 0.0);
            for dy in -r..=r {
                for dx in -r..=r {
                    let k = (-((dx * dx + dy * dy) as f32) / 24.0).exp();
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
                    let (u, v) = (x as f32 / 4.0, y as f32 / 4.0);
                    let i = (y * w + x) as usize;
                    let mut col = mix(night, deep, v / 32.0 * 0.9);
                    let haze = fbm(u * 0.2 + lx, v * 0.2 + ly, 131);
                    col = add_light(col, [92.0, 92.0, 104.0], (haze - 0.5).max(0.0) * 0.75);
                    // Stars, each on its own beat; none on the halo.
                    if noise(x / 2, y / 2, 133) > 0.982 && halo[i] < 0.1 {
                        let beat = (turn + noise(x / 2, y / 2, 134) * tau).sin();
                        col = mix(col, hex(0xf4f4f6), 0.25 + 0.6 * (0.5 + 0.5 * beat));
                    }
                    // The ring, and two sparks running round it once a loop.
                    let (dx, dy) = (x as f32 + 0.5 - rcx, y as f32 + 0.5 - rcy);
                    let off = (dx.hypot(dy) - radius).abs();
                    let ring = (-off * off / 1.2).exp();
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
                        let pos = -60.0 + t * 280.0;
                        let off = (x as f32 * 0.9 - y as f32 * 0.45 + 40.0) - pos;
                        let sweep = (-off * off / 90.0).exp() * 0.7;
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
            for x in 0..w {
                for k in 0..2 {
                    c.set(fx + x, fy + k, hex(0xbdbdc2));
                }
                let glint = {
                    let pos = t * (w as f32 + 40.0) - 20.0;
                    let off = x as f32 - pos;
                    (-off * off / 30.0).exp()
                };
                for k in 0..6 {
                    let base = match k {
                        0 | 1 => hex(0x3a3a40),
                        2 | 3 => GOLD,
                        _ => mix(GOLD, hex(0x8a6a1e), 0.5),
                    };
                    c.set(
                        fx + x,
                        fy + h - 6 + k,
                        add_light(base, [255.0, 240.0, 200.0], glint * 0.6),
                    );
                }
            }
            for y in 0..h - 6 {
                for k in 0..2 {
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
        // Behind the back, turned round like vanilla's cape, at an eighth of the scale for its
        // 8× texture.
        "attach": { "offset": [0, 0, 2], "rotation": [0, 180, 0], "scale": 0.125 },
        "texture": { "png": b64(&strip(frames).png()), "frames": frames.len(), "frametime": CAPE_FRAMETIME },
        "parts": [{
            "name": "cape", "pivot": [0, 0, 0], "rotation": [-7, 0, 0],
            "cubes": [{ "origin": [-w / 2, 0, -d], "size": [w, h, d], "uv": [0, 0] }]
        }],
        "animations": [{ "part": "cape", "motion": "swing", "axis": "x", "amplitude": 3, "period": 70, "phase": 0 }]
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
}
