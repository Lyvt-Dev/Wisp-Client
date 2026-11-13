//! Builds the launcher shop's items and their card art (`launcher/assets/shop`).
//!
//! `cargo run -p wisp-cosmetics --example shop_items -- launcher/assets/shop`
//!
//! Every model item is checked by `Item::parse` and its card is drawn with `preview::render`, so a
//! card shows exactly what the game draws.
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

// ---------------------------------------------------------------- capes (HD, animated)

/// HD capes are drawn at 4× Minecraft's cape: a 40×64×4 box at a quarter scale, so it has the
/// size of vanilla's 10×16×1 cape. Its unfolded faces take 88×68 texels: vanilla's 64×32 cape
/// layout at 4×, cropped to the cape.
const CAPE: (i32, i32, i32) = (40, 64, 4);
const CAPE_TEX: (u32, u32) = (88, 68);
/// Frames of an animated cape and the game ticks each shows: a 2.4 s loop.
const CAPE_FRAMES: u32 = 24;
const CAPE_FRAMETIME: u32 = 2;

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

fn luma(c: Rgba) -> f32 {
    (0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32) / 255.0
}

/// How a generated cape moves.
#[derive(Clone, Copy)]
enum CapeMotion {
    /// The nebula drifts, its wisps turning slowly.
    Drift,
    /// The lights ripple down the cape.
    Ripple,
}

struct CapeStyle {
    top: Rgba,
    bottom: Rgba,
    band: Rgba,
    band_glow: Rgba,
    border: Rgba,
    hem: Rgba,
    seed: u32,
    motion: CapeMotion,
}

/// One frame of a generated HD cape, `t` (0..1) through its loop: a night gradient, a band of
/// nebula or aurora across it, twinkling stars, a border and a hem.
fn cape_frame(style: &CapeStyle, t: f32) -> Canvas {
    let mut c = Canvas::new(CAPE_TEX.0, CAPE_TEX.1);
    let (w, h, d) = CAPE;
    let faces = box_faces(0, 0, w, h, d);
    for (x, y, fw, fh) in faces {
        c.rect(x, y, fw, fh, style.border);
    }
    let (fx, fy, _, _) = faces[3];
    let turn = t * std::f32::consts::TAU;
    // A loop: the noise is sampled round a circle, so the last frame meets the first.
    let (lx, ly) = (turn.cos() * 1.3, turn.sin() * 1.3);
    for y in 0..h {
        for x in 0..w {
            // Design units: the 20×32 of the 2× cape.
            let (u, v) = (x as f32 / 2.0, y as f32 / 2.0);
            let mut col = mix(style.top, style.bottom, v / 32.0);
            let wobble = match style.motion {
                CapeMotion::Drift => (fbm(u * 0.18 + lx, v * 0.18 + ly, style.seed) - 0.5) * 9.0,
                CapeMotion::Ripple => {
                    (v * 0.33 - turn).sin() * 2.2
                        + (fbm(u * 0.2, v * 0.2 + ly, style.seed) - 0.5) * 4.0
                }
            };
            let along = (u - 10.0) * 0.55 + (v - 21.0) + wobble;
            let dist = along.abs();
            if dist < 6.5 {
                let k = 1.0 - dist / 6.5;
                col = mix(col, style.band, k * k * 0.9);
                let haze = fbm(u * 0.5 + lx * 0.5, v * 0.5 - ly * 0.5, style.seed + 9);
                if dist < 2.2 && haze > 0.45 {
                    col = mix(
                        col,
                        style.band_glow,
                        (haze - 0.45) * 1.6 * (1.0 - dist / 2.2),
                    );
                }
            }
            // Stars, each twinkling on its own beat.
            let n = noise(x, y, style.seed + 3);
            if n > 0.982 {
                let beat = (turn + noise(x, y, style.seed + 4) * std::f32::consts::TAU).sin();
                col = mix(col, hex(0xf1e8ff), 0.35 + 0.55 * (0.5 + 0.5 * beat));
            }
            c.set(fx + x, fy + y, col);
        }
    }
    // The border two texels in, with a neckline; the hem's two bright rows.
    let neck = |x: i32| match x {
        12..=13 | 26..=27 => 4,
        14..=25 => 6,
        _ => 2,
    };
    for x in 2..w - 2 {
        for k in 0..2 {
            c.set(fx + x, fy + neck(x) + k, style.border);
        }
    }
    for y in 2..h - 6 {
        for k in 0..2 {
            c.set(fx + 2 + k, fy + y, style.border);
            c.set(fx + w - 3 - k, fy + y, style.border);
        }
    }
    for x in 0..w {
        for k in 0..2 {
            c.set(fx + x, fy + h - 6 + k, style.border);
        }
        for k in 2..4 {
            c.set(fx + x, fy + h - 6 + k, style.hem);
        }
        for k in 4..6 {
            c.set(fx + x, fy + h - 6 + k, mix(style.hem, style.border, 0.5));
        }
    }
    // The side toward the wearer: dark.
    let (bx, by, _, _) = faces[5];
    c.rect(bx, by, w, h, hex(0x1b1430));
    c
}

/// The owner's Founders Cape (a 256×128 cape in vanilla's layout at 4×), at its own
/// resolution, animated: a soft sheen sweeping across, its star breathing, the dark stars
/// twinkling. The artwork itself is untouched; the light is added over it.
fn founders_frames(path: &Path) -> Vec<Canvas> {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(
        (info.width, info.height, info.color_type),
        (256, 128, png::ColorType::Rgba),
        "a 256x128 RGBA cape"
    );
    let src = Canvas {
        w: 256,
        h: 128,
        px: buf[..256 * 128 * 4].to_vec(),
    };
    let (w, h, d) = CAPE;
    let (fx, fy, _, _) = box_faces(0, 0, w, h, d)[3];
    // The star: the centre of the brightest texels on the outer face.
    let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
    for y in 0..h {
        for x in 0..w {
            if luma(src.get(fx + x, fy + y)) > 0.7 {
                (sx, sy, n) = (sx + x as f32, sy + y as f32, n + 1.0);
            }
        }
    }
    let star = if n > 0.0 {
        (sx / n, sy / n)
    } else {
        (w as f32 / 2.0, h as f32 / 3.0)
    };
    (0..CAPE_FRAMES)
        .map(|k| {
            let t = k as f32 / CAPE_FRAMES as f32;
            let turn = t * std::f32::consts::TAU;
            let mut c = Canvas::new(CAPE_TEX.0, CAPE_TEX.1);
            for y in 0..CAPE_TEX.1 as i32 {
                for x in 0..CAPE_TEX.0 as i32 {
                    c.set(x, y, src.get(x, y));
                }
            }
            for y in 0..h {
                for x in 0..w {
                    let p = c.get(fx + x, fy + y);
                    if p[3] == 0 {
                        continue;
                    }
                    let l = luma(p);
                    // A diagonal sheen crossing once a loop, starting off the cape.
                    let pos = -40.0 + t * 180.0;
                    let off = (x as f32 * 0.8 + y as f32 * 0.6) - pos;
                    let sheen = (-off * off / 60.0).exp() * 0.42;
                    // The star and the streaks (the bright texels) breathe.
                    let breathe = if l > 0.42 {
                        (0.5 + 0.5 * turn.sin()) * 0.55 * (l - 0.42)
                    } else {
                        0.0
                    };
                    // A soft halo round the star, swelling and easing with its breath.
                    let (dx, dy) = (x as f32 - star.0, y as f32 - star.1);
                    let halo = (-(dx * dx + dy * dy) / 70.0).exp() * (0.22 + 0.18 * turn.sin());
                    let mut q = add_light(p, [190.0, 160.0, 255.0], sheen + breathe + halo);
                    // Small stars in the dark violet, each on its own beat.
                    if l < 0.25 && noise(x, y, 91) > 0.975 {
                        let beat = (turn * 2.0 + noise(x, y, 92) * std::f32::consts::TAU).sin();
                        q = mix(q, hex(0xeadfff), 0.25 + 0.5 * (0.5 + 0.5 * beat));
                    }
                    c.set(fx + x, fy + y, q);
                }
            }
            c
        })
        .collect()
}

/// The Zedix Cape: the owner's purple Z (`zedix_cape_logo.png`, any size, its letter opaque or
/// on white) on a dark violet cape, animated: violet smoke drifting behind, a halo breathing round
/// the letter, a neon edge pulsing, sparks running along it and a bright sweep across the Z.
fn zedix_frames(path: &Path) -> Vec<Canvas> {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(
        (info.color_type, info.bit_depth),
        (png::ColorType::Rgba, png::BitDepth::Eight),
        "an 8-bit RGBA logo"
    );
    let src = Canvas {
        w: info.width,
        h: info.height,
        px: buf[..(info.width * info.height * 4) as usize].to_vec(),
    };
    // The letter: opaque texels that aren't the white page.
    let ink = |x: i32, y: i32| {
        let p = src.get(x, y);
        p[3] > 128 && (p[0] as u32 + p[1] as u32 + p[2] as u32) < 600
    };
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, 0, 0);
    for y in 0..src.h as i32 {
        for x in 0..src.w as i32 {
            if ink(x, y) {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    assert!(x0 < x1 && y0 < y1, "the logo has a letter");

    let (w, h, d) = CAPE;
    // The letter 30 texels wide on the outer face, centred a little above the middle.
    let zw = 30.0;
    let zh = zw * (y1 - y0) as f32 / (x1 - x0) as f32;
    let (zx, zy) = ((w as f32 - zw) / 2.0, 27.0 - zh / 2.0);
    // Each texel's share of letter (4×4 samples) and the letter's own colour there.
    let mut cover = vec![0.0f32; (w * h) as usize];
    let mut tint = vec![[0.0f32; 3]; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let (mut n, mut col) = (0.0, [0.0; 3]);
            for sy in 0..4 {
                for sx in 0..4 {
                    let u = (x as f32 + (sx as f32 + 0.5) / 4.0 - zx) / zw;
                    let v = (y as f32 + (sy as f32 + 0.5) / 4.0 - zy) / zh;
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
    // The halo: the letter blurred wide.
    let r = 5;
    let mut halo = vec![0.0f32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let (mut s, mut n) = (0.0, 0.0);
            for dy in -r..=r {
                for dx in -r..=r {
                    let k = (-((dx * dx + dy * dy) as f32) / 10.0).exp();
                    s += at(&cover, x + dx, y + dy) * k;
                    n += k;
                }
            }
            halo[(y * w + x) as usize] = s / n;
        }
    }
    // The edge: letter texels beside open cape, and open texels beside the letter.
    let edge = |x: i32, y: i32| {
        let c = at(&cover, x, y);
        let near = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .map(|(dx, dy)| at(&cover, x + dx, y + dy))
            .fold(0.0f32, |m, n| m.max((n - c).abs()));
        (near * 1.4)
            .min(1.0)
            .max(if c > 0.05 && c < 0.95 { 1.0 } else { 0.0 })
    };

    let faces = box_faces(0, 0, w, h, d);
    let (fx, fy, _, _) = faces[3];
    let deep = hex(0x07030f);
    let night = hex(0x1a0b33);
    let neon = [205.0, 150.0, 255.0];
    (0..CAPE_FRAMES)
        .map(|k| {
            let t = k as f32 / CAPE_FRAMES as f32;
            let turn = t * std::f32::consts::TAU;
            let (lx, ly) = (turn.cos() * 1.2, turn.sin() * 1.2);
            let mut c = Canvas::new(CAPE_TEX.0, CAPE_TEX.1);
            for (x, y, fw, fh) in faces {
                c.rect(x, y, fw, fh, hex(0x6a2fd0));
            }
            for y in 0..h {
                for x in 0..w {
                    let (u, v) = (x as f32 / 2.0, y as f32 / 2.0);
                    let i = (y * w + x) as usize;
                    // Dark violet, lighter toward the letter, with smoke drifting up.
                    let mut col = mix(night, deep, (v / 32.0 - 0.2).abs() * 1.4);
                    let smoke = fbm(u * 0.22 + lx, v * 0.16 + ly + t * 2.0, 71);
                    col = add_light(col, [90.0, 30.0, 170.0], (smoke - 0.45).max(0.0) * 0.9);
                    // Stars, each twinkling on its own beat.
                    if noise(x, y, 73) > 0.985 && halo[i] < 0.15 {
                        let beat = (turn + noise(x, y, 74) * std::f32::consts::TAU).sin();
                        col = mix(col, hex(0xeadfff), 0.3 + 0.5 * (0.5 + 0.5 * beat));
                    }
                    let breath = 0.5 + 0.5 * turn.sin();
                    col = add_light(col, [150.0, 70.0, 255.0], halo[i] * (0.7 + 0.6 * breath));
                    // The letter: its own gradient lifted toward violet so it reads in the dark.
                    if cover[i] > 0.0 {
                        let z = tint[i];
                        let lifted = [
                            (z[0] * 1.5 + 40.0).min(255.0) as u8,
                            (z[1] * 1.3 + 12.0).min(255.0) as u8,
                            (z[2] * 1.6 + 60.0).min(255.0) as u8,
                            255,
                        ];
                        col = mix(col, lifted, cover[i]);
                        // A diagonal sweep of light crossing once a loop.
                        let pos = -30.0 + t * 140.0;
                        let off = (x as f32 * 0.9 - y as f32 * 0.45 + 20.0) - pos;
                        let sweep = (-off * off / 30.0).exp() * 0.75;
                        col = add_light(col, [230.0, 200.0, 255.0], sweep * cover[i]);
                    }
                    // The neon edge, pulsing, with sparks running round it.
                    let e = edge(x, y);
                    if e > 0.0 {
                        let spark_phase = noise(x, y, 75) * std::f32::consts::TAU;
                        let spark = ((turn * 2.0 + spark_phase).sin() - 0.8).max(0.0) * 5.0;
                        let glow = e * (0.45 + 0.35 * breath + spark);
                        col = add_light(col, neon, glow);
                    }
                    c.set(fx + x, fy + y, col);
                }
            }
            // A fine glowing border and a bright hem.
            for x in 0..w {
                c.set(fx + x, fy, hex(0x8a4dff));
                c.set(fx + x, fy + h - 1, hex(0xb98cff));
                c.set(fx + x, fy + h - 2, hex(0x6a2fd0));
            }
            for y in 0..h {
                c.set(fx, fy + y, hex(0x8a4dff));
                c.set(fx + w - 1, fy + y, hex(0x8a4dff));
            }
            // The side toward the wearer: dark.
            let (bx, by, _, _) = faces[5];
            c.rect(bx, by, w, h, hex(0x140a26));
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
        // Behind the back, turned round like vanilla's cape, at a quarter scale for its 4× texture.
        "attach": { "offset": [0, 0, 2], "rotation": [0, 180, 0], "scale": 0.25 },
        "texture": { "png": b64(&strip(frames).png()), "frames": frames.len(), "frametime": CAPE_FRAMETIME },
        "parts": [{
            "name": "cape", "pivot": [0, 0, 0], "rotation": [-7, 0, 0],
            "cubes": [{ "origin": [-w / 2, 0, -d], "size": [w, h, d], "uv": [0, 0] }]
        }],
        "animations": [{ "part": "cape", "motion": "swing", "axis": "x", "amplitude": 3, "period": 70, "phase": 0 }]
    })
}

// ---------------------------------------------------------------- badges

/// Wisp's badge: the app icon (a white feather on a black tile) shrunk to a name tag's mark,
/// each pixel the average of the icon's pixels it covers (alpha-weighted).
fn wisp_badge_texture() -> Canvas {
    let src = png_canvas(WISP_MARK);
    let side = wisp_cosmetics::MAX_MARK;
    // The tile's rim is left out, so the feather fills more of the small mark.
    let rim = src.w / 8;
    let step = (src.w - 2 * rim) / side;
    let mut c = Canvas::new(side, side);
    for y in 0..side {
        for x in 0..side {
            let mut sum = [0u32; 4];
            for sy in 0..step {
                for sx in 0..step {
                    let p = src.get((rim + x * step + sx) as i32, (rim + y * step + sy) as i32);
                    let a = u32::from(p[3]);
                    for i in 0..3 {
                        sum[i] += u32::from(p[i]) * a;
                    }
                    sum[3] += a;
                }
            }
            if sum[3] > 0 {
                let n = step * step;
                let rgb = |i: usize| (sum[i] / sum[3]) as u8;
                c.set(
                    x as i32,
                    y as i32,
                    [rgb(0), rgb(1), rgb(2), (sum[3] / n) as u8],
                );
            }
        }
    }
    c
}

fn badge_item(id: &str, texture: &Canvas, line: &str) -> Value {
    json!({
        "format": 1, "id": id, "slot": "name_tag",
        "texture": { "png": b64(&texture.png()) },
        "parts": [], "tag": { "line": line }
    })
}

// ---------------------------------------------------------------- pets

/// Paints every face of a `w`×`h`×`d` box at `(u, v)`: `paint(face, x, y, fw, fh)`, the faces in
/// `box_faces`' order (top, bottom, right, front, left, back), `x`, `y` within the face.
#[allow(clippy::too_many_arguments)]
fn paint_box(
    c: &mut Canvas,
    u: i32,
    v: i32,
    w: i32,
    h: i32,
    d: i32,
    paint: impl Fn(usize, i32, i32, i32, i32) -> Rgba,
) {
    for (face, (fx, fy, fw, fh)) in box_faces(u, v, w, h, d).into_iter().enumerate() {
        for y in 0..fh {
            for x in 0..fw {
                c.set(fx + x, fy + y, paint(face, x, y, fw, fh));
            }
        }
    }
}

/// A patch of night sky for a pet's fur or feathers: a smooth deep ground, faint nebula wisps
/// and a few bright stars, so it reads as sky rather than camouflage at a few texels a face.
#[derive(Clone, Copy)]
struct Sky {
    deep: Rgba,
    mid: Rgba,
    nebula: Rgba,
    /// The share of texels that are a bright star.
    stars: f32,
}

impl Sky {
    /// The sky at texel `(x, y)` of a face; `seed` tells faces and boxes apart.
    fn at(&self, x: i32, y: i32, seed: u32) -> Rgba {
        let n = noise(x / 3, y / 3, seed);
        let mut c = mix(self.deep, self.mid, 0.2 + n * 0.45);
        let wisp = noise(x / 3, y / 2, seed + 2);
        if wisp > 0.8 {
            c = mix(c, self.nebula, (wisp - 0.8) * 2.2);
        }
        if noise(x, y, seed + 4) > 1.0 - self.stars {
            c = mix(c, hex(0xffffff), 0.85);
        }
        c
    }

    /// `at` for a box face, each face its own patch of sky.
    fn face(&self, face: usize, x: i32, y: i32, seed: u32) -> Rgba {
        self.at(x + face as i32 * 7, y + face as i32 * 5, seed)
    }
}

const OWL_SKY: Sky = Sky {
    deep: hex(0x1f1554),
    mid: hex(0x3f2a92),
    nebula: hex(0xb257d6),
    stars: 0.06,
};
const OWL_WING: Rgba = hex(0x2c1d6e);
const OWL_PALE: Rgba = hex(0xd8ccff);
const GOLD: Rgba = hex(0xf2b544);

/// Nova, a star owl: night-sky feathers, a pale breast and face disc, amber eyes, gold beak and
/// talons.
fn owl_texture() -> Canvas {
    let mut c = Canvas::new(64, 64);
    // Body 5×5×4 at (0, 0): a pale breast with soft scallops and a small star.
    paint_box(&mut c, 0, 0, 5, 5, 4, |face, x, y, _, _| {
        if face != 3 {
            return OWL_SKY.face(face, x, y, 11);
        }
        if (x, y) == (2, 1) {
            return hex(0xffffff);
        }
        let base = mix(OWL_PALE, hex(0xa592ea), y as f32 / 4.0);
        if y % 2 == 0 && y > 0 && x % 2 == 1 {
            mix(base, hex(0x8670d8), 0.35)
        } else {
            base
        }
    });
    // Head 6×5×5 at (0, 10): the face disc, dark brows, eyes with a glint, the beak's base.
    paint_box(&mut c, 0, 10, 6, 5, 5, |face, x, y, _, _| {
        if face != 3 {
            return OWL_SKY.face(face, x, y, 23);
        }
        match (x, y) {
            (0 | 1, 0) | (4 | 5, 0) => hex(0x2a1c5c),
            (0, 1) | (5, 1) => hex(0xfff6d6),
            (1, 1) | (0, 2) | (4, 1) | (5, 2) => hex(0xffc64d),
            (1, 2) | (4, 2) => hex(0x1a0f2e),
            (_, 4) => OWL_PALE,
            _ => hex(0xb8a6f5),
        }
    });
    // Ear tufts 1×2×1, a bright tip on top.
    for u in [24, 28] {
        paint_box(&mut c, u, 0, 1, 2, 1, |face, _, y, _, _| {
            if face == 0 || y == 0 {
                hex(0xe9dcff)
            } else {
                OWL_WING
            }
        });
    }
    // Beak 2×1×1 and talons 1×1×1.
    paint_box(&mut c, 32, 0, 2, 1, 1, |face, _, _, _, _| {
        if face == 1 { hex(0xb57d22) } else { GOLD }
    });
    for u in [36, 40] {
        paint_box(&mut c, u, 0, 1, 1, 1, |_, _, _, _, _| GOLD);
    }
    // Wings 1×4×4: darker than the body, pale-edged flight feathers and a star.
    for u in [0, 10] {
        paint_box(&mut c, u, 20, 1, 4, 4, move |face, x, y, fw, fh| {
            if face != 2 && face != 4 {
                return OWL_WING;
            }
            if y == fh - 1 || (y == fh - 2 && x % 2 == 0) {
                hex(0xa592ea)
            } else if (x, y) == (fw / 2, 1) {
                hex(0xffffff)
            } else {
                mix(OWL_WING, hex(0x4a33a6), y as f32 / fh as f32)
            }
        });
    }
    // Tail 3×1×2: pale feather tips.
    paint_box(&mut c, 20, 20, 3, 1, 2, |face, x, y, _, _| {
        if face == 3 || face == 5 {
            hex(0xa592ea)
        } else {
            OWL_SKY.face(face, x, y, 41)
        }
    });
    c
}

fn owl_item(texture: &Canvas) -> Value {
    json!({
        "format": 1, "id": "nova_owl", "slot": "shoulder", "bone": "body", "glow": true,
        "attach": { "offset": [7, 0.5, 0], "scale": 0.9 },
        "sparkles": ["#ffffff", "#ffd36b", "#c7a3ff"],
        "texture": { "png": b64(&texture.png()) },
        // A part's cubes are placed from its own pivot.
        "parts": [{
            "name": "owl", "pivot": [0, 0, 0],
            "cubes": [
                { "origin": [-2.5, -5, -2], "size": [5, 5, 4], "uv": [0, 0] },
                { "origin": [-2, -1, -2.6], "size": [1, 1, 1], "uv": [36, 0] },
                { "origin": [1, -1, -2.6], "size": [1, 1, 1], "uv": [40, 0] }
            ],
            "children": [
                { "name": "owl_head", "pivot": [0, -5, 0],
                  "cubes": [
                    { "origin": [-3, -5, -2.5], "size": [6, 5, 5], "uv": [0, 10] },
                    { "origin": [-1, -2, -3.5], "size": [2, 1, 1], "uv": [32, 0] }
                  ],
                  "children": [
                    { "name": "owl_tuft_left", "pivot": [2.5, -5, 0], "rotation": [0, 0, 20],
                      "cubes": [{ "origin": [-0.5, -2, -0.5], "size": [1, 2, 1], "uv": [24, 0] }] },
                    { "name": "owl_tuft_right", "pivot": [-2.5, -5, 0], "rotation": [0, 0, -20],
                      "cubes": [{ "origin": [-0.5, -2, -0.5], "size": [1, 2, 1], "uv": [28, 0] }] }
                  ] },
                { "name": "owl_wing_left", "pivot": [2.5, -4.5, 0],
                  "cubes": [{ "origin": [0, 0, -2], "size": [1, 4, 4], "uv": [0, 20], "inflate": 0.1 }] },
                { "name": "owl_wing_right", "pivot": [-2.5, -4.5, 0],
                  "cubes": [{ "origin": [-1, 0, -2], "size": [1, 4, 4], "uv": [10, 20], "inflate": 0.1 }] },
                { "name": "owl_tail", "pivot": [0, -1, 2], "rotation": [-25, 0, 0],
                  "cubes": [{ "origin": [-1.5, 0, 0], "size": [3, 1, 2], "uv": [20, 20] }] }
            ]
        }],
        "animations": [
            { "part": "owl", "motion": "bob", "axis": "y", "amplitude": 0.35, "period": 60, "phase": 0 },
            { "part": "owl_head", "motion": "swing", "axis": "y", "amplitude": 25, "period": 180, "phase": 0.2 },
            { "part": "owl_wing_left", "motion": "swing", "axis": "z", "amplitude": 6, "period": 40, "phase": 0 },
            { "part": "owl_wing_right", "motion": "swing", "axis": "z", "amplitude": 6, "period": 40, "phase": 0.5 },
            { "part": "owl_tail", "motion": "swing", "axis": "x", "amplitude": 8, "period": 60, "phase": 0.3 }
        ]
    })
}

const FOX_SKY: Sky = Sky {
    deep: hex(0x141a52),
    mid: hex(0x2c2c86),
    nebula: hex(0xc25ad0),
    stars: 0.05,
};
/// The fox's starlight fur (chest, cheeks, paws), its shade, and the glow of its tips.
const FOX_LIGHT: Rgba = hex(0xe8e2ff);
const FOX_SHADE: Rgba = hex(0xb9acf0);
const FOX_GLOW: Rgba = hex(0xc8fbff);
const FOX_EYE: Rgba = hex(0x7af3ff);

/// Astra, a cosmic fox: fur of deep space, starlight on the chest and cheeks, cyan eyes, pink
/// nebula in the ears, and a tail full of stars that ends in a glowing one.
fn fox_texture() -> Canvas {
    let mut c = Canvas::new(64, 64);
    // Chest 4×6×4 at (0, 0): a starlight bib widening down the front.
    paint_box(&mut c, 0, 0, 4, 6, 4, |face, x, y, _, _| {
        if face == 3 && (y >= 2 || (1..=2).contains(&x)) {
            mix(FOX_LIGHT, FOX_SHADE, y as f32 / 5.0)
        } else {
            FOX_SKY.face(face, x, y, 13)
        }
    });
    // Haunch 4×3×5 at (16, 0).
    paint_box(&mut c, 16, 0, 4, 3, 5, |face, x, y, _, _| {
        FOX_SKY.face(face, x, y, 17)
    });
    // Head 6×5×5 at (0, 10): a smooth brow, eyes, starlit cheeks and chin, a star on top.
    paint_box(&mut c, 0, 10, 6, 5, 5, |face, x, y, _, _| match face {
        3 => match (x, y) {
            (1, 2) | (4, 2) => FOX_EYE,
            (0 | 1 | 4 | 5, 3) | (_, 4) => FOX_LIGHT,
            _ => mix(FOX_SKY.deep, FOX_SKY.mid, 0.35 + y as f32 * 0.1),
        },
        0 if (2..=3).contains(&x) && (1..=2).contains(&y) => hex(0xffffff),
        1 => FOX_SHADE,
        2 | 4 if y >= 3 => FOX_LIGHT,
        _ => FOX_SKY.face(face, x, y, 29),
    });
    // Snout 3×2×2 at (22, 10): pale, a dark nose on its tip.
    paint_box(&mut c, 22, 10, 3, 2, 2, |face, x, y, _, _| {
        match (face, x, y) {
            (3, 1, 0) => hex(0x0b0a1d),
            (0, _, 0) | (1, _, _) => FOX_SHADE,
            _ => FOX_LIGHT,
        }
    });
    // Ears 2×3×1: glowing tips, nebula pink inside.
    for (u, seed) in [(36, 43), (42, 47)] {
        paint_box(&mut c, u, 0, 2, 3, 1, move |face, x, y, _, _| {
            match (face, y) {
                (0, _) | (_, 0) => FOX_GLOW,
                (3, _) => hex(0xff79d4),
                _ => FOX_SKY.face(face, x, y, seed),
            }
        });
    }
    // Legs 1×3×1, starlight paws.
    for (u, seed) in [(48, 53), (52, 59)] {
        paint_box(&mut c, u, 0, 1, 3, 1, move |face, x, y, _, fh| {
            if face == 1 || y == fh - 1 {
                FOX_LIGHT
            } else {
                FOX_SKY.face(face, x, y, seed)
            }
        });
    }
    // Tail 3×3×6 at (0, 20): the most stars and nebula.
    let dense = Sky {
        stars: 0.11,
        nebula: hex(0x4fc8e8),
        ..FOX_SKY
    };
    paint_box(&mut c, 0, 20, 3, 3, 6, |face, x, y, _, _| {
        dense.face(face, x, y, 61)
    });
    // Tail tip 2×2×3 at (18, 20): a glowing star, white at its heart.
    paint_box(&mut c, 18, 20, 2, 2, 3, |face, x, y, _, _| {
        mix(FOX_GLOW, hex(0xffffff), noise(x + face as i32 * 3, y, 67))
    });
    c
}

fn fox_item(texture: &Canvas) -> Value {
    json!({
        "format": 1, "id": "cosmic_fox", "slot": "shoulder", "bone": "body", "glow": true,
        "attach": { "offset": [7, 0.5, 0], "scale": 0.8 },
        "sparkles": ["#ffffff", "#7af3ff", "#ff79d4"],
        "texture": { "png": b64(&texture.png()) },
        "parts": [{
            "name": "fox", "pivot": [0, 0, 0],
            "cubes": [
                { "origin": [-2, -8, -2], "size": [4, 6, 4], "uv": [0, 0] },
                { "origin": [-2, -3, -1], "size": [4, 3, 5], "uv": [16, 0], "inflate": 0.05 },
                { "origin": [0.5, -3, -2.5], "size": [1, 3, 1], "uv": [48, 0] },
                { "origin": [-1.5, -3, -2.5], "size": [1, 3, 1], "uv": [52, 0] }
            ],
            "children": [
                { "name": "fox_head", "pivot": [0, -8, -0.5],
                  "cubes": [
                    { "origin": [-3, -5, -3], "size": [6, 5, 5], "uv": [0, 10] },
                    { "origin": [-1.5, -2, -5], "size": [3, 2, 2], "uv": [22, 10] },
                    { "origin": [-3, -8, -1.5], "size": [2, 3, 1], "uv": [36, 0] },
                    { "origin": [1, -8, -1.5], "size": [2, 3, 1], "uv": [42, 0] }
                  ] },
                { "name": "fox_tail", "pivot": [0, -1.2, 4.1], "rotation": [40, 0, 0],
                  "cubes": [{ "origin": [-1.5, -1.5, 0], "size": [3, 3, 6], "uv": [0, 20] }],
                  "children": [
                    { "name": "fox_tail_tip", "pivot": [0, 0, 6],
                      "cubes": [{ "origin": [-1, -1, 0], "size": [2, 2, 3], "uv": [18, 20] }] }
                  ] }
            ]
        }],
        "animations": [
            { "part": "fox", "motion": "bob", "axis": "y", "amplitude": 0.3, "period": 80, "phase": 0 },
            { "part": "fox_head", "motion": "swing", "axis": "y", "amplitude": 18, "period": 150, "phase": 0.1 },
            { "part": "fox_tail", "motion": "swing", "axis": "y", "amplitude": 18, "period": 44, "phase": 0 },
            { "part": "fox_tail_tip", "motion": "swing", "axis": "y", "amplitude": 14, "period": 44, "phase": 0.15 }
        ]
    })
}

// ---------------------------------------------------------------- trails

fn trail_item(id: &str, style: &str, colours: &[&str]) -> Value {
    json!({ "format": 1, "id": id, "slot": "trail", "trail": { "style": style, "colours": colours } })
}

// ---------------------------------------------------------------- auras

fn aura_item(id: &str, style: &str, colours: &[&str]) -> Value {
    json!({ "format": 1, "id": id, "slot": "aura", "aura": { "style": style, "colours": colours } })
}

/// `#rrggbb` as the card painter's colour.
fn rgbf(hex_text: &str) -> [f32; 3] {
    let v = u32::from_str_radix(hex_text.trim_start_matches('#'), 16).unwrap();
    [(v >> 16) as f32, (v >> 8 & 0xff) as f32, (v & 0xff) as f32]
}

/// A glowing pixel dot: a soft halo of `glow` around a bright core `size` pixels across.
fn dot(card: &mut Canvas, x: f32, y: f32, size: i32, colour: [f32; 3], glow: f32) {
    let halo = size * 3;
    for dy in -halo..=halo {
        for dx in -halo..=halo {
            let d = ((dx * dx + dy * dy) as f32).sqrt() / halo as f32;
            if d <= 1.0 {
                card.blend(
                    x as i32 + dx,
                    y as i32 + dy,
                    colour,
                    (1.0 - d) * glow * 0.35,
                );
            }
        }
    }
    let half = size / 2;
    for dy in 0..size {
        for dx in 0..size {
            card.blend(x as i32 + dx - half, y as i32 + dy - half, colour, 0.95);
        }
    }
}

/// An aura's card: the wearer, `centre` of the way across, and the aura drawn around them as
/// the game emits it.
fn aura_card(w: u32, h: u32, centre: f32, style: &str, colours: &[&str], seed: u32) -> Canvas {
    let skin = wearer_skin();
    let body = preview::player_model(false);
    let draws = vec![Draw {
        root: &body,
        base: Mat4::IDENTITY,
        texture: &skin,
        poses: Vec::new(),
    }];
    let size = (h as f32 * 1.02) as u32;
    let figure = preview::render(&draws, size, size, 20.0, 12.0);
    let glow = rgbf(colours[colours.len() / 2]);
    let mut card = backdrop(w, h, glow, seed);
    let (ox, oy) = (
        (w as f32 * centre) as i32 - size as i32 / 2,
        h as i32 - size as i32 + size as i32 / 12,
    );
    composite(&mut card, &figure, ox, oy);
    // The figure's bounds on the card.
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, 0, 0);
    for y in 0..figure.height as i32 {
        for x in 0..figure.width as i32 {
            if figure.rgba[((y as u32 * figure.width + x as u32) * 4 + 3) as usize] > 0 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    let (x0, y0, x1, y1) = (
        (x0 + ox) as f32,
        (y0 + oy) as f32,
        (x1 + ox) as f32,
        (y1 + oy) as f32,
    );
    let (cx, tall) = ((x0 + x1) / 2.0, y1 - y0);
    let cell = (h as i32 / 60).max(2);
    let c = |i: usize| rgbf(colours[i % colours.len()]);
    match style {
        "storm" => {
            // Bolts beside the body: zigzags from the shoulders and hips, white cores.
            for k in 0..6 {
                let side = if k % 2 == 0 { -1.0 } else { 1.0 };
                let mut x = cx + side * (x1 - x0) * (0.55 + noise(k, 1, seed) * 0.35);
                let mut y = y0 + tall * (0.1 + noise(k, 2, seed) * 0.6);
                for s in 0..5 {
                    let jag = if s % 2 == 0 { 1.0 } else { -1.0 };
                    let nx = x + jag * (0.5 + noise(k, s + 10, seed)) * tall * 0.06;
                    let ny = y + tall * (0.05 + noise(k, s + 20, seed) * 0.06);
                    let steps = 8;
                    for i in 0..steps {
                        let f = i as f32 / steps as f32;
                        let colour = if i % 3 == 0 { c(1) } else { c(0) };
                        dot(
                            &mut card,
                            x + (nx - x) * f,
                            y + (ny - y) * f,
                            cell,
                            colour,
                            0.8,
                        );
                    }
                    (x, y) = (nx, ny);
                }
            }
        }
        "orbit" => {
            // A tilted ring of star dust round the waist, three bright stars on it.
            let (rx, ry, cy) = ((x1 - x0) * 1.05, tall * 0.07, y0 + tall * 0.5);
            for i in 0..90 {
                let a = i as f32 / 90.0 * std::f32::consts::TAU;
                let colour = c(i / 30);
                dot(
                    &mut card,
                    cx + a.cos() * rx,
                    cy + a.sin() * ry,
                    cell - 1,
                    colour,
                    0.4,
                );
            }
            for k in 0..3 {
                let a = 0.4 + k as f32 * std::f32::consts::TAU / 3.0;
                dot(
                    &mut card,
                    cx + a.cos() * rx,
                    cy + a.sin() * ry,
                    cell * 2,
                    c(k),
                    1.0,
                );
            }
        }
        _ => {
            // Two strands of stardust spiralling up from the feet past the head.
            for k in 0..2 {
                for i in 0..80 {
                    let t = i as f32 / 80.0;
                    let a = t * std::f32::consts::TAU * 2.2 + k as f32 * std::f32::consts::PI;
                    let rx = (x1 - x0) * (0.95 - t * 0.3);
                    let colour = c((t * colours.len() as f32) as usize);
                    let front = a.sin() > 0.0;
                    dot(
                        &mut card,
                        cx + a.cos() * rx,
                        y1 - t * tall * 1.1,
                        if front { cell } else { cell - 1 },
                        colour,
                        if front { 0.7 } else { 0.35 },
                    );
                }
            }
        }
    }
    card
}

// ---------------------------------------------------------------- cards

/// A wearer in dark clothes on a 64×64 skin, so the items stand out.
fn wearer_skin() -> wisp_cosmetics::Texture {
    let mut c = Canvas::new(64, 64);
    let cloth = hex(0x231b33);
    let trim = hex(0x6a46c4);
    let hair = hex(0x171221);
    let face = hex(0xd9a183);
    fill_box(&mut c, 0, 0, 8, 8, 8, hair);
    let (x, y, _, _) = box_faces(0, 0, 8, 8, 8)[3];
    c.rect(x, y + 3, 8, 5, face);
    c.rect(x + 1, y + 4, 2, 1, hex(0xf2f0ff));
    c.rect(x + 5, y + 4, 2, 1, hex(0xf2f0ff));
    c.set(x + 2, y + 4, hex(0x5b3aa8));
    c.set(x + 5, y + 4, hex(0x5b3aa8));
    fill_box(&mut c, 16, 16, 8, 12, 4, cloth);
    for (u, v) in [(40, 16), (32, 48)] {
        fill_box(&mut c, u, v, 4, 12, 4, cloth);
        for (x, y, w, _) in box_faces(u, v, 4, 12, 4).into_iter().skip(2) {
            c.rect(x, y + 3, w, 1, trim);
            c.rect(x, y + 10, w, 2, face);
        }
    }
    for (u, v) in [(0, 16), (16, 48)] {
        fill_box(&mut c, u, v, 4, 12, 4, hex(0x16111f));
    }
    wisp_cosmetics::Texture {
        width: 64,
        height: 64,
        rgba: c.px,
    }
}

/// The card's background: a night gradient, a glow behind the subject, stars.
fn backdrop(w: u32, h: u32, glow: [f32; 3], seed: u32) -> Canvas {
    let mut c = Canvas::new(w, h);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let t = y as f32 / h as f32;
            let col = mix(hex(0x120b24), hex(0x2a1446), t);
            c.set(x, y, col);
            let (dx, dy) = (
                (x as f32 - w as f32 * 0.5) / w as f32,
                (y as f32 - h as f32 * 0.55) / h as f32,
            );
            let g = (1.0 - (dx * dx * 2.2 + dy * dy * 2.8).sqrt() * 1.6).max(0.0);
            c.blend(x, y, glow, g * 0.55);
            let n = noise(x, y, seed);
            if n > 0.9975 {
                c.blend(x, y, [255.0, 255.0, 255.0], 0.5 + (n - 0.9975) * 200.0);
            }
        }
    }
    // Pixel ground: a dark hill line at the bottom.
    for x in 0..w as i32 {
        let top = h as f32 * 0.86 + (noise(x / 12, 0, seed + 1) * 10.0);
        for y in top as i32..h as i32 {
            c.blend(x, y, [24.0, 14.0, 40.0], 0.9);
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
/// `drop` of the figure's size below the card's bottom (a close-up crops the legs), its middle
/// `centre` of the way across.
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

/// The cape's art laid flat beside the model: its outer face (40×64), `height` pixels tall.
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

/// A particle trail sweeping across the card: `colours` from head to tail.
fn trail_card(w: u32, h: u32, colours: &[[f32; 3]], seed: u32) -> Canvas {
    let mut card = backdrop(w, h, colours[0], seed);
    let cell = (h / 40).max(3) as i32;
    for k in 0..220 {
        let t = k as f32 / 220.0;
        let x = w as f32 * (0.1 + 0.8 * t);
        let y = h as f32 * (0.68 - 0.36 * t) + (t * 9.0 + seed as f32).sin() * h as f32 * 0.06;
        let spread = (1.0 - t) * h as f32 * 0.09;
        let jx = (noise(k, 1, seed) - 0.5) * spread * 2.0;
        let jy = (noise(k, 2, seed) - 0.5) * spread * 2.0;
        let col = colours[((1.0 - t) * (colours.len() - 1) as f32).round() as usize];
        let size = cell + (noise(k, 3, seed) * 2.0) as i32 * cell / 2;
        let (px, py) = (
            ((x + jx) as i32 / cell) * cell,
            ((y + jy) as i32 / cell) * cell,
        );
        for yy in 0..size {
            for xx in 0..size {
                card.blend(px + xx, py + yy, col, 0.35 + t * 0.6);
            }
        }
    }
    card
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn write(dir: &Path, name: &str, bytes: &[u8]) {
    std::fs::write(dir.join(name), bytes).unwrap();
    println!("wrote {name}");
}

/// `item` alone, turned to show its face, filling most of the card, its middle `centre` of the
/// way across.
fn close_up(item: &Item, w: u32, h: u32, fill: f32, centre: f32, glow: [f32; 3]) -> Canvas {
    let body = preview::player_model(false);
    let draws = Draw::item(item, &body, 0.0);
    let big = preview::render(&draws, 2400, 2400, 25.0, 8.0);
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..big.height {
        for x in 0..big.width {
            if big.rgba[((y * big.width + x) * 4 + 3) as usize] > 0 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    let scale = (h as f32 * fill / (y1 - y0 + 1) as f32).min(w as f32 * 0.8 / (x1 - x0 + 1) as f32);
    let mut card = backdrop(w, h, glow, 17);
    let (fw, fh) = (
        ((x1 - x0 + 1) as f32 * scale) as i32,
        ((y1 - y0 + 1) as f32 * scale) as i32,
    );
    let (ox, oy) = ((w as f32 * centre) as i32 - fw / 2, (h as i32 - fh) / 2);
    for y in 0..fh {
        for x in 0..fw {
            let sx = x0 + (x as f32 / scale) as u32;
            let sy = y0 + (y as f32 / scale) as u32;
            let i = ((sy * big.width + sx) * 4) as usize;
            let a = big.rgba[i + 3] as f32 / 255.0;
            if a > 0.0 {
                let c = [
                    big.rgba[i] as f32,
                    big.rgba[i + 1] as f32,
                    big.rgba[i + 2] as f32,
                ];
                card.blend(ox + x, oy + y, c, a);
            }
        }
    }
    card
}

/// A cape's card: the wearer's back on the left, the cape laid flat on the right.
fn cape_card(item: &Item, texture: &Canvas, w: u32, h: u32, glow: [f32; 3]) -> Canvas {
    let mut card = framed_card(item, w, h, 162.0, 1.9, 0.45, 0.33, glow);
    let tall = h as i32 - 24;
    flat_cape(&mut card, texture, (w as f32 * 0.66) as i32, 12, tall);
    card
}

/// Nebula Wings: the side of one frame of the wing's texture and how many frames.
const WING_SIZE: u32 = 256;
const WING_FRAMES: usize = 8;
/// The wing's shoulder in its texture (0..1 across and down): the arm starts here.
const WING_ROOT: [f32; 2] = [0.05, 0.44];
/// The two panels a wing is made of, as corners in its texture (top left, top right, bottom
/// right, bottom left): the arm's half and the hand's, which bends at their shared edge. Their
/// lower edges follow the feather tips, so the panels stay within an item's reach.
const WING_INNER: [[f32; 2]; 4] = [[0.0, 0.0], [0.5, 0.0], [0.5, 0.76], [0.0, 0.97]];
const WING_OUTER: [[f32; 2]; 4] = [[0.5, 0.0], [1.0, 0.0], [1.0, 0.4], [0.5, 0.76]];

/// A smooth value noise in 0..1 at `(x, y)` (one unit a cell), `seed` choosing the pattern.
fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let cell = |ix: i32, iy: i32| -> f32 {
        let mut h = (ix as u32).wrapping_mul(0x8da6_b343)
            ^ (iy as u32).wrapping_mul(0xd816_3841)
            ^ seed.wrapping_mul(0xcb1a_b31f);
        h ^= h >> 13;
        h = h.wrapping_mul(0x5bd1_e995);
        h ^= h >> 15;
        (h & 0xffff) as f32 / 65535.0
    };
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (s(fx), s(fy));
    let top = cell(ix, iy) + (cell(ix + 1, iy) - cell(ix, iy)) * sx;
    let bottom = cell(ix, iy + 1) + (cell(ix + 1, iy + 1) - cell(ix, iy + 1)) * sx;
    top + (bottom - top) * sy
}

fn wing_fbm(x: f32, y: f32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut freq) = (0.0, 0.5, 1.0);
    for octave in 0..5 {
        sum += amp * value_noise(x * freq, y * freq, seed + octave);
        amp *= 0.5;
        freq *= 2.0;
    }
    sum
}

/// One feather of the wing, in its texture's 0..1 space.
struct Feather {
    /// Where its quill leaves the arm, the way it points and how long it is.
    base: [f32; 2],
    dir: [f32; 2],
    length: f32,
    /// Its half width at its widest.
    width: f32,
    /// 0: the long flight feathers; 1 and 2: the coverts lying over them.
    layer: u8,
    /// Its own shade, 0..1.
    tint: f32,
}

/// The wing's arm, `s` of the way from the shoulder to the hand: up to the wrist, then out.
fn wing_arm(s: f32) -> [f32; 2] {
    let (p0, p1, p2) = (WING_ROOT, [0.3, 0.1], [0.58, 0.085]);
    let t = 1.0 - s;
    [
        t * t * p0[0] + 2.0 * t * s * p1[0] + s * s * p2[0],
        t * t * p0[1] + 2.0 * t * s * p1[1] + s * s * p2[1],
    ]
}

/// A bird's wing, spread: three rows of feathers along the arm, hanging down at the shoulder and
/// fanning out to the tip; the topmost first (short coverts over longer ones over the flight
/// feathers, each feather over its outer neighbour).
fn wing_feathers() -> Vec<Feather> {
    let mut feathers = Vec::new();
    for (layer, count, long, width) in [
        (2u8, 13, 0.27, 0.043),
        (1, 16, 0.56, 0.04),
        (0, 19, 1.0, 0.04),
    ] {
        for i in 0..count {
            let s = i as f32 / (count - 1) as f32;
            let h = value_noise(i as f32, f32::from(layer), 5);
            let h2 = value_noise(i as f32, f32::from(layer), 6);
            let angle = (88.0 - 80.0 * s.powf(1.3) + (h - 0.5) * 5.0).to_radians();
            let length = (0.36 + 0.04 * s + 0.56 * s * (1.0 - s)) * long * (0.96 + 0.08 * h2);
            feathers.push(Feather {
                base: wing_arm(s),
                dir: [angle.cos(), angle.sin()],
                length,
                width: width * (1.0 - 0.25 * s),
                layer,
                tint: h,
            });
        }
    }
    feathers
}

/// The topmost feather over `p`: which, how far along it (0 at the quill, 1 at the tip) and how
/// far across (-1 to 1, 0 on the shaft).
fn feather_at(feathers: &[Feather], p: [f32; 2]) -> Option<(&Feather, f32, f32)> {
    feathers.iter().find_map(|f| {
        let d = [p[0] - f.base[0], p[1] - f.base[1]];
        let a = (d[0] * f.dir[0] + d[1] * f.dir[1]) / f.length;
        if !(-0.04..=1.0).contains(&a) {
            return None;
        }
        let along = a.max(0.0);
        let half = f.width * (along + 0.05).powf(0.3) * (1.0 - along.powi(4)).sqrt();
        let b = (d[1] * f.dir[0] - d[0] * f.dir[1]) / half.max(1e-4);
        (b.abs() <= 1.0).then_some((f, along, b))
    })
}

/// Whether `p` is inside the convex quad `q` (corners in order).
fn in_quad(q: &[[f32; 2]; 4], p: [f32; 2]) -> bool {
    (0..4).all(|i| {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]) >= 0.0
    })
}

/// One frame of the wings' texture, `phase` of the way through the loop: a feathered wing made
/// of night sky. Each feather is shaded (its shaft, its vanes, the shadow of the row above),
/// dark blue at the arm, through violet and pink gas, to tips that glow cyan white; the gas
/// drifts, a shimmer runs out along the feathers and stars twinkle.
fn wing_frame(phase: f32, feathers: &[Feather]) -> Canvas {
    let n = WING_SIZE;
    let mut c = Canvas::new(n, n);
    let deep = [16.0, 8.0, 52.0];
    let violet = [118.0, 54.0, 232.0];
    let pink = [255.0, 104.0, 212.0];
    let cyan = [130.0, 232.0, 255.0];
    let white = [245.0, 240.0, 255.0];
    let mix = |a: [f32; 3], b: [f32; 3], t: f32| -> [f32; 3] {
        let t = t.clamp(0.0, 1.0);
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    };
    let step = |a: f32, b: f32, x: f32| {
        let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let drift = phase * std::f32::consts::TAU;
    let (ox, oy) = (drift.cos() * 0.35, drift.sin() * 0.35);
    let arm: Vec<[f32; 2]> = (0..=40).map(|k| wing_arm(k as f32 / 40.0)).collect();
    for y in 0..n {
        for x in 0..n {
            let p = [(x as f32 + 0.5) / n as f32, (y as f32 + 0.5) / n as f32];
            if !in_quad(&WING_INNER, p) && !in_quad(&WING_OUTER, p) {
                continue;
            }
            // The arm: a rounded ridge of down along the leading edge, over every feather.
            let (ridge, ridge_s) = arm
                .iter()
                .enumerate()
                .map(|(k, q)| ((p[0] - q[0]).hypot(p[1] - q[1]), k as f32 / 40.0))
                .fold((f32::MAX, 0.0), |m, v| if v.0 < m.0 { v } else { m });
            let thick = 0.034 * (1.0 - 0.45 * ridge_s);
            // How far down the wing's colours this texel is, its shade, and how much it glows.
            let (depth, shade, glow) = if ridge < thick {
                let r = ridge / thick;
                let fluff = value_noise(p[0] * 90.0, p[1] * 90.0, 41);
                (
                    0.12 + 0.1 * fluff,
                    (1.0 - 0.45 * r * r) * (0.9 + 0.2 * fluff),
                    0.0,
                )
            } else if let Some((f, a, b)) = feather_at(feathers, p) {
                // Vanes darker towards their edges (where the next feather lies over them), one
                // lighter than the other, a pale shaft and fine barbs.
                let mut shade = 0.48 + 0.52 * (1.0 - b.abs().powf(3.0));
                shade *= 1.0 - 0.14 * b;
                if b.abs() < 0.1 {
                    shade += 0.25 * (1.0 - a);
                }
                shade *= 0.94 + 0.06 * (a * f.length * 240.0 + b.abs() * 5.0).sin();
                shade *= 0.92 + 0.16 * f.tint;
                // The shadow of the row lying over this one.
                let (depth, lit) = match f.layer {
                    0 => (0.25 + 0.75 * a, 0.58 + 0.42 * step(0.42, 0.68, a)),
                    1 => (0.15 + 0.6 * a, 0.68 + 0.32 * step(0.3, 0.6, a)),
                    _ => (0.08 + 0.42 * a, 1.0),
                };
                let glow = if f.layer == 0 {
                    step(0.82, 1.0, a)
                } else {
                    step(0.9, 1.0, a) * 0.5
                };
                (depth, shade * lit, glow)
            } else {
                continue;
            };
            let gas = wing_fbm(p[0] * 5.5 + ox, p[1] * 5.5 + oy, 11);
            let swirl = wing_fbm(p[0] * 13.0 - oy, p[1] * 13.0 + ox, 29);
            let mut col = mix(deep, violet, depth * 1.9 + gas * 0.9 - 0.5);
            col = mix(col, pink, (gas - 0.4) * 3.4 * swirl + (depth - 0.6) * 1.1);
            col = mix(col, cyan, (swirl - 0.66) * 4.0 + (depth - 0.84) * 3.0);
            col = col.map(|v| v * shade);
            // Light running out along the feathers, and tips that shine.
            let wave = (drift - depth * 5.0 - p[0] * 3.0).sin().max(0.0).powi(6);
            col = mix(col, cyan, wave * 0.22);
            col = mix(col, white, glow * 0.7);
            // Stars that twinkle as the loop turns.
            let h = value_noise(x as f32 * 7.31, y as f32 * 5.17, 97);
            if h > 0.955 && glow < 0.3 {
                let twinkle = (0.5 + 0.5 * (drift * 2.0 + h * 400.0).sin()).powf(2.0);
                col = mix(col, [255.0, 255.0, 255.0], 0.35 + 0.65 * twinkle);
            }
            c.set(
                x as i32,
                y as i32,
                [
                    col[0].min(255.0) as u8,
                    col[1].min(255.0) as u8,
                    col[2].min(255.0) as u8,
                    255,
                ],
            );
        }
    }
    c
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

/// Nebula Wings on the back: two wings at the shoulder blades, each an arm and a hand that
/// bends after it as the wing beats, swept back and cupped a little, drawn at full brightness
/// and shedding stardust.
fn nebula_wings_item(frames: &[Canvas]) -> Value {
    // A wing's texture covers this many model pixels each way.
    let size = 21.0;
    // One panel of the wing reaching to +x or -x, seen from behind and from in front; `origin`
    // is the texture point at the part's pivot.
    let panel = |side: f32, uv: [[f32; 2]; 4], origin: [f32; 2]| -> Vec<Value> {
        let [a, b, c, d] =
            uv.map(|[u, v]| [side * (u - origin[0]) * size, (v - origin[1]) * size, 0.0]);
        let back = quad([a, b, c, d], uv);
        let front = quad([d, c, b, a], [uv[3], uv[2], uv[1], uv[0]]);
        if side > 0.0 {
            vec![back, front]
        } else {
            vec![front, back]
        }
    };
    let joint = [0.5, WING_ROOT[1]];
    let wing = |name: &str, side: f32| -> Value {
        json!({
            "name": name, "pivot": [side * 1.5, 3.0, 2.6], "rotation": [0.0, side * -24.0, 0.0],
            "quads": panel(side, WING_INNER, WING_ROOT),
            "children": [{
                "name": format!("{name}_tip"),
                "pivot": [side * (joint[0] - WING_ROOT[0]) * size, 0.0, 0.0],
                "rotation": [0.0, side * 12.0, 0.0],
                "quads": panel(side, WING_OUTER, joint),
            }],
        })
    };
    json!({
        "format": 1, "id": "nebula_wings", "slot": "back", "bone": "body", "glow": true,
        "texture": { "png": b64(&strip(frames).png()), "frames": frames.len(), "frametime": 3 },
        "sparkles": ["#ffffff", "#ff8ae0", "#8fe8ff", "#b48cff"],
        "parts": [wing("left_wing", 1.0), wing("right_wing", -1.0)],
        "animations": [
            { "part": "left_wing", "motion": "swing", "axis": "y", "amplitude": 13, "period": 64 },
            { "part": "right_wing", "motion": "swing", "axis": "y", "amplitude": -13, "period": 64 },
            { "part": "left_wing_tip", "motion": "swing", "axis": "y", "amplitude": 11, "period": 64, "phase": 0.14 },
            { "part": "right_wing_tip", "motion": "swing", "axis": "y", "amplitude": -11, "period": 64, "phase": 0.14 },
            { "part": "left_wing", "motion": "swing", "axis": "z", "amplitude": 4, "period": 64, "phase": 0.25 },
            { "part": "right_wing", "motion": "swing", "axis": "z", "amplitude": -4, "period": 64, "phase": 0.25 },
        ],
    })
}

/// A Blockbench model (`.bbmodel`, free format) as quads for an item part, and its texture's PNG
/// (base64). Each cube turns about its origin (Blockbench's order: x, then y, then z), its faces
/// keep their own UVs, and the model is moved so that `centre` (Blockbench pixels) is the part's
/// origin. Blockbench is y up; a model part is y down and x mirrored (the game draws models
/// turned half round about z), so x and y change sign.
fn bbmodel_quads(model: &Value, centre: [f32; 3]) -> (Vec<Value>, String) {
    let f = |v: &Value| v.as_f64().unwrap_or(0.0) as f32;
    let v3 = |v: &Value| [f(&v[0]), f(&v[1]), f(&v[2])];
    let (uw, uh) = (
        f(&model["resolution"]["width"]),
        f(&model["resolution"]["height"]),
    );
    let source = model["textures"][0]["source"].as_str().expect("a texture");
    let png = source
        .split_once("base64,")
        .map_or(source, |(_, b)| b)
        .to_owned();
    let mut quads = Vec::new();
    for el in model["elements"].as_array().expect("elements") {
        let (a, b) = (v3(&el["from"]), v3(&el["to"]));
        let origin = v3(&el["origin"]);
        let [rx, ry, rz] = if el["rotation"].is_array() {
            v3(&el["rotation"])
        } else {
            [0.0; 3]
        };
        let turn = glam::Mat3::from_rotation_z(rz.to_radians())
            * glam::Mat3::from_rotation_y(ry.to_radians())
            * glam::Mat3::from_rotation_x(rx.to_radians());
        // Each face's corners as seen from outside: top left, top right, bottom right, bottom left.
        let corners = |face: &str| -> [[f32; 3]; 4] {
            let ([x0, y0, z0], [x1, y1, z1]) = (a, b);
            match face {
                "north" => [[x1, y1, z0], [x0, y1, z0], [x0, y0, z0], [x1, y0, z0]],
                "south" => [[x0, y1, z1], [x1, y1, z1], [x1, y0, z1], [x0, y0, z1]],
                "east" => [[x1, y1, z1], [x1, y1, z0], [x1, y0, z0], [x1, y0, z1]],
                "west" => [[x0, y1, z0], [x0, y1, z1], [x0, y0, z1], [x0, y0, z0]],
                "up" => [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]],
                _ => [[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]],
            }
        };
        let faces = el["faces"].as_object().expect("faces");
        for name in ["north", "east", "south", "west", "up", "down"] {
            let Some(face) = faces.get(name).filter(|fc| !fc["texture"].is_null()) else {
                continue;
            };
            let uv = &face["uv"];
            let (u1, v1, u2, v2) = (
                f(&uv[0]) / uw,
                f(&uv[1]) / uh,
                f(&uv[2]) / uw,
                f(&uv[3]) / uh,
            );
            let [tl, tr, br, bl] = corners(name);
            // Vanilla's corner order: (u1, v0), (u0, v0), (u0, v1), (u1, v1).
            let vertices: Vec<Value> = [(tr, u2, v1), (tl, u1, v1), (bl, u1, v2), (br, u2, v2)]
                .into_iter()
                .map(|(p, u, v)| {
                    let o = glam::Vec3::from(origin);
                    let p = turn * (glam::Vec3::from(p) - o) + o - glam::Vec3::from(centre);
                    let r = |x: f32| (x * 10_000.0).round() / 10_000.0;
                    json!([
                        r(-p.x),
                        r(-p.y),
                        r(p.z),
                        r(u.clamp(0.0, 1.0)),
                        r(v.clamp(0.0, 1.0))
                    ])
                })
                .collect();
            quads.push(json!({ "vertices": vertices }));
        }
    }
    (quads, png)
}

/// Where Daylight's model is centred (Blockbench pixels): the middle of the sword.
const DAYLIGHT_CENTRE: [f32; 3] = [0.0, 10.5, -1.5];
/// The card looks at the wearer's back.
const DAYLIGHT_YAW: f32 = 200.0;

/// Daylight on the body bone: behind the back (+z), turned 145° about z so the hilt rises over
/// the right shoulder and the blade crosses the back to the left hip. The `DAYLIGHT_*`
/// variables try other placements.
fn daylight_attach() -> Value {
    let env = |name: &str, default: [f32; 3]| -> [f32; 3] {
        std::env::var(name)
            .ok()
            .and_then(|s| {
                let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                (v.len() == 3).then(|| [v[0], v[1], v[2]])
            })
            .unwrap_or(default)
    };
    let scale = std::env::var("DAYLIGHT_SCALE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.7);
    json!({
        "offset": env("DAYLIGHT_OFFSET", [0.0, 5.0, 3.5]),
        "rotation": env("DAYLIGHT_ROTATION", [0.0, 0.0, 145.0]),
        "scale": scale,
    })
}

/// Daylight, a sword by @gold3nz, carried across the back: hilt over the right shoulder, the
/// blade down to the left hip, flat against the back.
fn daylight_item(dir: &Path) -> Value {
    let text = std::fs::read_to_string(dir.join("daylight.bbmodel")).expect("daylight.bbmodel");
    let model: Value = serde_json::from_str(&text).expect("a bbmodel");
    let (quads, png) = bbmodel_quads(&model, DAYLIGHT_CENTRE);
    json!({
        "format": 1, "id": "daylight", "slot": "weapon", "bone": "body",
        "attach": daylight_attach(),
        "texture": { "png": png },
        "parts": [{ "name": "daylight", "quads": quads }],
    })
}

// ---------------------------------------------------------------- Wisp Original Cape

/// Wisp's own mark (a white feather on black), as the launcher and the game show it.
const WISP_MARK: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../wisp-theme/icons/wisp-256.png"
));

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

/// The Wisp Original Cape: Wisp's feather on a black night, animated: a starfield
/// twinkling, a faint ring round the mark with a spark running along it, a halo breathing round
/// the mark, its edge glowing and a sweep of light crossing it.
fn original_frames() -> Vec<Canvas> {
    let src = png_canvas(WISP_MARK);
    // The mark: the white feather (the icon's ground and glow are far darker, its shaft black).
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
    // The mark 24 texels wide, centred a little above the middle; the ring round it.
    let mw = 24.0;
    let mh = mw * (y1 - y0) as f32 / (x1 - x0) as f32;
    let (mx, my) = ((w as f32 - mw) / 2.0, 26.0 - mh / 2.0);
    let (rcx, rcy, radius) = (w as f32 / 2.0, 26.0, (mh / 2.0 + 3.0).min(17.0));
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
    let r = 5;
    let mut halo = vec![0.0f32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let (mut s, mut n) = (0.0, 0.0);
            for dy in -r..=r {
                for dx in -r..=r {
                    let k = (-((dx * dx + dy * dy) as f32) / 10.0).exp();
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
    let deep = hex(0x030303);
    let night = hex(0x18181c);
    let tau = std::f32::consts::TAU;
    (0..CAPE_FRAMES)
        .map(|k| {
            let t = k as f32 / CAPE_FRAMES as f32;
            let turn = t * tau;
            let (lx, ly) = (turn.cos() * 1.1, turn.sin() * 1.1);
            let mut c = Canvas::new(CAPE_TEX.0, CAPE_TEX.1);
            for (x, y, fw, fh) in faces {
                c.rect(x, y, fw, fh, hex(0x2a2a2e));
            }
            for y in 0..h {
                for x in 0..w {
                    let (u, v) = (x as f32 / 2.0, y as f32 / 2.0);
                    let i = (y * w + x) as usize;
                    // Night, darker toward the hem, with a faint nebula drifting.
                    let mut col = mix(night, deep, v / 32.0 * 0.9);
                    let haze = fbm(u * 0.2 + lx, v * 0.2 + ly, 131);
                    col = add_light(col, [90.0, 90.0, 100.0], (haze - 0.5).max(0.0) * 0.8);
                    // A starfield, each star on its own beat.
                    if noise(x, y, 133) > 0.976 && halo[i] < 0.12 {
                        let beat = (turn + noise(x, y, 134) * tau).sin();
                        col = mix(col, hex(0xf4f4f6), 0.3 + 0.55 * (0.5 + 0.5 * beat));
                    }
                    // The ring, and a spark running once round it a loop, a fainter one after.
                    let (dx, dy) = (x as f32 + 0.5 - rcx, y as f32 + 0.5 - rcy);
                    let off = (dx.hypot(dy) - radius).abs();
                    let ring = (-off * off / 0.5).exp();
                    col = add_light(col, [200.0, 200.0, 210.0], ring * 0.35);
                    let angle = dy.atan2(dx);
                    let spark = |lead: f32| {
                        let da = (angle - lead + tau * 1.5).rem_euclid(tau) - tau / 2.0;
                        (-da * da / 0.04).exp() * ring
                    };
                    col = add_light(
                        col,
                        [255.0, 235.0, 255.0],
                        spark(turn) * 1.3 + spark(turn + tau / 2.0) * 0.5,
                    );
                    let breath = 0.5 + 0.5 * turn.sin();
                    col = add_light(col, [170.0, 170.0, 180.0], halo[i] * (0.6 + 0.6 * breath));
                    if cover[i] > 0.0 {
                        let m = tint[i];
                        let lifted = [
                            (m[0] * 1.15 + 20.0).min(255.0) as u8,
                            (m[1] * 1.1 + 10.0).min(255.0) as u8,
                            (m[2] * 1.05 + 10.0).min(255.0) as u8,
                            255,
                        ];
                        col = mix(col, lifted, cover[i]);
                        let pos = -30.0 + t * 140.0;
                        let off = (x as f32 * 0.9 - y as f32 * 0.45 + 20.0) - pos;
                        let sweep = (-off * off / 30.0).exp() * 0.7;
                        col = add_light(col, [255.0, 255.0, 255.0], sweep * cover[i]);
                    }
                    let e = edge(x, y);
                    if e > 0.0 {
                        col = add_light(col, [230.0, 230.0, 235.0], e * (0.35 + 0.3 * breath));
                    }
                    c.set(fx + x, fy + y, col);
                }
            }
            // A fine border and a bright hem.
            for x in 0..w {
                c.set(fx + x, fy, hex(0xbdbdc2));
                c.set(fx + x, fy + h - 1, hex(0xffffff));
                c.set(fx + x, fy + h - 2, hex(0x9a9aa0));
                c.set(fx + x, fy + h - 3, hex(0x3a3a40));
            }
            for y in 0..h {
                c.set(fx, fy + y, hex(0xbdbdc2));
                c.set(fx + w - 1, fy + y, hex(0xbdbdc2));
            }
            let (bx, by, _, _) = faces[5];
            c.rect(bx, by, w, h, hex(0x0c0c0e));
            c
        })
        .collect()
}

// ---------------------------------------------------------------- Stray Head (a straw hat)

/// The hat's cone: segments round it, the rim's radius and height (model pixels, y down: the
/// head's top is -8), the copper band's depth, and the apexes outside and underneath.
const HAT_SEGMENTS: usize = 20;
const HAT_RIM: f32 = 12.0;
const HAT_RIM_Y: f32 = -7.0;
const HAT_BAND: f32 = 0.6;
const HAT_APEX_Y: f32 = -12.5;
const HAT_UNDER_APEX_Y: f32 = -11.0;
/// The tiny ring the cone closes to at its top, under the knot.
const HAT_TOP: f32 = 0.3;

/// The hat's 64×64 texture: rows 0-31 the woven outside (each column a direction round it,
/// apex at the top), 32-35 the copper band bound with red thread, 36-63 the darker underside.
fn straw_texture() -> Canvas {
    let mut c = Canvas::new(64, 64);
    for y in 0..64 {
        for x in 0..64 {
            let strip = x / 2;
            let col = if y < 32 {
                // Strips of straw from the apex down, each its own shade, lighter up top where
                // the sun falls, a row of stitching every five texels.
                let v = y as f32 / 31.0;
                let mut col = mix(hex(0xf0dfba), hex(0xd2b582), v * 0.8);
                col = mix(col, hex(0xc4a46f), noise(strip, 1, 7) * 0.45);
                if x % 2 == 1 && noise(x, y / 3, 8) > 0.55 {
                    col = mix(col, hex(0xfaf0d8), 0.35);
                }
                if y % 5 == 4 {
                    col = if x % 3 == 0 {
                        hex(0xf6eedc)
                    } else {
                        mix(col, hex(0x9c7f52), 0.5)
                    };
                }
                if y < 3 {
                    col = mix(hex(0xf6f1e6), hex(0xe4dccb), noise(x, y, 9));
                }
                col
            } else if y < 36 {
                // Copper, bright along its top edge, wound with red thread.
                let mut col = match y {
                    32 => hex(0xe09a5a),
                    35 => hex(0x7a3d1c),
                    _ => hex(0xb8672f),
                };
                if x % 3 == 0 && (33..35).contains(&y) {
                    col = hex(0xc8402c);
                }
                col
            } else {
                // The inside: darker straw, strips running in to the crown, a ring every six rows.
                let mut col = mix(hex(0xa8875a), hex(0x8a6c44), (y - 36) as f32 / 27.0);
                col = mix(col, hex(0x6f5535), noise(strip, 2, 7) * 0.4);
                if (y - 36) % 6 == 5 {
                    col = mix(col, hex(0x5c4428), 0.5);
                }
                col
            };
            c.set(x, y, col);
        }
    }
    c
}

/// Stray Head: a wide conical straw hat on the head, double-sided so it reads from above and
/// below, with a little knot of straw at its top.
fn stray_item(texture: &Canvas) -> Value {
    let n = HAT_SEGMENTS;
    let texel = |t: f32| t / 64.0;
    let mut quads = Vec::new();
    // Each face both ways, the same texels on each side.
    let mut both = |p: [[f32; 3]; 4], uv: [[f32; 2]; 4]| {
        quads.push(quad(p, uv));
        quads.push(quad([p[3], p[2], p[1], p[0]], [uv[3], uv[2], uv[1], uv[0]]));
    };
    let ring = |a: f32, r: f32, y: f32| [a.cos() * r, y, a.sin() * r];
    for i in 0..n {
        let (s0, s1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
        let (a0, a1) = (s0 * std::f32::consts::TAU, s1 * std::f32::consts::TAU);
        let (u0, u1) = (s0, s1);
        let band_y = HAT_RIM_Y + HAT_BAND;
        both(
            [
                ring(a1, HAT_TOP, HAT_APEX_Y),
                ring(a0, HAT_TOP, HAT_APEX_Y),
                ring(a0, HAT_RIM, HAT_RIM_Y),
                ring(a1, HAT_RIM, HAT_RIM_Y),
            ],
            [
                [u1, texel(0.5)],
                [u0, texel(0.5)],
                [u0, texel(31.5)],
                [u1, texel(31.5)],
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
                [u1, texel(32.2)],
                [u0, texel(32.2)],
                [u0, texel(35.8)],
                [u1, texel(35.8)],
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
                [u1, texel(63.5)],
                [u0, texel(63.5)],
                [u0, texel(36.5)],
                [u1, texel(36.5)],
            ],
        );
    }
    json!({
        "format": 1, "id": "stray_head", "slot": "head", "bone": "head",
        "texture": { "png": b64(&texture.png()) },
        "parts": [{
            "name": "hat", "pivot": [0, 0, 0], "rotation": [0, 0, 0],
            // The knot at the top (its box texels are the cream of the apex rows).
            "cubes": [{ "origin": [-1, HAT_APEX_Y - 0.8, -1], "size": [2, 1, 2], "uv": [56, 0] }],
            "quads": quads,
        }],
    })
}

// ---------------------------------------------------------------- emote cards

/// An emote's card: the wearer posed (part name → turn about x, y, z in radians), `yaw` degrees
/// round and framed like `framed_card`, and where points round the wearer land on it.
struct Posed {
    card: Canvas,
    figure: preview::Image,
    size: u32,
    ox: i32,
    oy: i32,
    yaw: f32,
}

/// The card painter's pitch for every figure (`framed_card`'s).
const CARD_PITCH: f32 = 12.0;

impl Posed {
    fn new(
        turns: &[(&str, [f32; 3])],
        w: u32,
        h: u32,
        yaw: f32,
        zoom: f32,
        drop: f32,
        glow: [f32; 3],
        seed: u32,
    ) -> Self {
        let skin = wearer_skin();
        let body = preview::player_model(false);
        let poses = turns
            .iter()
            .map(|(name, [x, y, z])| {
                let part = body
                    .children
                    .iter()
                    .find(|p| p.name == *name)
                    .expect("a body part");
                let mut pose = part.pose;
                (pose[3], pose[4], pose[5]) = (*x, *y, *z);
                (name.to_string(), pose)
            })
            .collect();
        let draws = vec![Draw {
            root: &body,
            base: Mat4::IDENTITY,
            texture: &skin,
            poses,
        }];
        let size = (h as f32 * zoom) as u32;
        let figure = preview::render(&draws, size, size, yaw, CARD_PITCH);
        let mut card = backdrop(w, h, glow, seed);
        let (ox, oy) = (
            w as i32 / 2 - size as i32 / 2,
            h as i32 - size as i32 + (size as f32 * drop) as i32,
        );
        composite(&mut card, &figure, ox, oy);
        Self {
            card,
            figure,
            size,
            ox,
            oy,
            yaw,
        }
    }

    /// Where the point `side` blocks to the wearer's right, `up` above their feet and `back`
    /// behind them lands on the card, and whether the wearer hides it there.
    fn project(&self, side: f32, up: f32, back: f32) -> (f32, f32, bool) {
        let s = self.size as f32;
        let view = Mat4::from_translation(glam::Vec3::new(s / 2.0, s * 0.1, 0.0))
            * Mat4::from_scale(glam::Vec3::splat(s / 2.5))
            * Mat4::from_rotation_x(-CARD_PITCH.to_radians())
            * Mat4::from_rotation_y(self.yaw.to_radians())
            * Mat4::from_translation(glam::Vec3::new(0.0, 0.5, 0.0));
        // Model space: the right arm is at -x, the feet 1.5 blocks below the neck, the back +z.
        let model = glam::Vec3::new(-side, 1.5 - up, back);
        let q = view.transform_point3(model);
        let centre = view.transform_point3(glam::Vec3::new(0.0, 0.75, 0.0));
        let (fx, fy) = (q.x as i32, q.y as i32);
        let covered = fx >= 0
            && fy >= 0
            && (fx as u32) < self.figure.width
            && (fy as u32) < self.figure.height
            && self.figure.rgba[((fy as u32 * self.figure.width + fx as u32) * 4 + 3) as usize] > 0;
        (
            self.ox as f32 + q.x,
            self.oy as f32 + q.y,
            covered && q.z > centre.z,
        )
    }

    /// A glowing dot at a point round the wearer, left out where the wearer stands in front.
    fn dot(&mut self, side: f32, up: f32, back: f32, size: i32, colour: [f32; 3], glow: f32) {
        let (x, y, hidden) = self.project(side, up, back);
        if !hidden {
            dot(&mut self.card, x, y, size, colour, glow);
        }
    }
}

fn dark_ascension_card(w: u32, h: u32) -> Canvas {
    let mut p = Posed::new(
        &[
            ("right_arm", [-0.1, 0.0, 1.0]),
            ("left_arm", [-0.1, 0.0, -1.0]),
            ("head", [-0.55, 0.0, 0.0]),
        ],
        w,
        h,
        18.0,
        1.0,
        0.0,
        [110.0, 30.0, 190.0],
        41,
    );
    // Three dark strands spiralling up round the body, violet sparks among them.
    for k in 0..3 {
        for j in 0..70 {
            let c = j as f32 / 70.0;
            let a = c * std::f32::consts::TAU * 1.4 + k as f32 * std::f32::consts::TAU / 3.0;
            let r = 0.85 - 0.3 * c;
            let colour = if j % 9 == 0 {
                [190.0, 110.0, 255.0]
            } else {
                [60.0, 12.0, 100.0]
            };
            p.dot(a.cos() * r, -0.1 + c * 2.4, a.sin() * r, 3, colour, 0.5);
        }
    }
    // Black mist on the ground.
    for k in 0..40 {
        let a = noise(k, 1, 3) * std::f32::consts::TAU;
        let r = 0.5 + noise(k, 2, 3) * 1.4;
        p.dot(a.cos() * r, 0.05, a.sin() * r, 4, [40.0, 8.0, 70.0], 0.4);
    }
    // Burning violet eyes, on the head turned up.
    let turn = glam::Mat3::from_rotation_x(-0.55);
    for side in [2.0f32, -2.0] {
        let e = turn * glam::Vec3::new(-side, -4.5, -4.3) / 16.0;
        p.dot(-e.x, 1.5 - e.y, e.z - 0.02, 2, [220.0, 140.0, 255.0], 1.0);
    }
    p.card
}

fn bruh_card(w: u32, h: u32) -> Canvas {
    let mut p = Posed::new(
        &[
            ("right_arm", [-2.3, -0.83, 0.0]),
            ("left_arm", [0.05, 0.0, -0.06]),
            ("head", [0.49, 0.15, 0.0]),
        ],
        w,
        h,
        22.0,
        1.9,
        0.52,
        [170.0, 130.0, 230.0],
        43,
    );
    // "..." over the head, and a drop of sweat at the temple.
    for k in 0..3 {
        p.dot(
            -0.22 + 0.22 * k as f32,
            2.2,
            0.0,
            6,
            [255.0, 255.0, 255.0],
            0.6,
        );
    }
    p.dot(0.32, 1.85, -0.1, 5, [120.0, 190.0, 255.0], 0.8);
    p.dot(0.33, 1.78, -0.1, 4, [120.0, 190.0, 255.0], 0.5);
    p.card
}

/// A music note in pixels at `(x, y)` (its head's centre), `cell` pixels a texel.
fn note_glyph(card: &mut Canvas, x: f32, y: f32, cell: i32, colour: [f32; 3]) {
    let (x, y) = (x as i32, y as i32);
    let mut px = |dx: i32, dy: i32| {
        for yy in 0..cell {
            for xx in 0..cell {
                card.blend(x + dx * cell + xx, y + dy * cell + yy, colour, 0.95);
            }
        }
    };
    for (dx, dy) in [(-1, 0), (0, 0), (-1, 1), (0, 1), (-2, 0), (-2, 1)] {
        px(dx, dy);
    }
    for dy in -5..1 {
        px(1, dy);
    }
    for (dx, dy) in [(2, -5), (3, -4), (3, -3)] {
        px(dx, dy);
    }
}

fn l_dance_card(w: u32, h: u32) -> Canvas {
    let mut p = Posed::new(
        &[
            ("right_arm", [-2.55, -0.75, 0.0]),
            ("left_arm", [-1.3, 0.0, -0.22]),
            ("right_leg", [0.45, 0.0, 0.0]),
            ("left_leg", [-0.45, 0.0, 0.0]),
            ("head", [0.1, -0.2, 0.0]),
        ],
        w,
        h,
        25.0,
        1.0,
        0.0,
        [70.0, 210.0, 130.0],
        47,
    );
    let colours = [
        [255.0, 110.0, 110.0],
        [255.0, 205.0, 70.0],
        [100.0, 235.0, 130.0],
        [100.0, 175.0, 255.0],
        [215.0, 120.0, 255.0],
    ];
    let cell = (h as i32 / 50).max(2);
    for (k, (side, up)) in [
        (-0.95, 1.85),
        (0.95, 1.6),
        (-1.6, 1.15),
        (1.6, 0.95),
        (0.55, 1.88),
    ]
    .into_iter()
    .enumerate()
    {
        let (x, y, _) = p.project(side, up, 0.0);
        let colour = colours[k];
        dot(&mut p.card, x, y, cell, colour, 0.6);
        note_glyph(&mut p.card, x, y, cell, colour);
    }
    // Dust kicked up by the steps.
    for k in 0..10 {
        let side = (noise(k, 1, 5) - 0.5) * 0.9;
        p.dot(
            side,
            0.05 + noise(k, 2, 5) * 0.15,
            (noise(k, 3, 5) - 0.5) * 0.4,
            3,
            [210.0, 200.0, 225.0],
            0.3,
        );
    }
    p.card
}

/// A Celestial Wing's point as the game draws it, open and held a little high: `u` along the
/// wing, `v` down a feather, on the `side` (1 right), as (right, up, back) from its root.
fn wing_point(side: f32, u: f32, v: f32) -> [f32; 3] {
    let pi = std::f32::consts::PI;
    let flap = 0.25f32;
    let length = 0.6 + 0.8 * (u * pi * 0.8).sin();
    let out = 0.12 + 2.0 * u + 0.55 * length * v * u;
    let up = 0.7 * (u * pi * 0.9).sin() - 0.15 * u - length * v * (1.0 - 0.3 * u);
    let (s, c) = flap.sin_cos();
    [side * (out * c - up * s), out * s + up * c, 0.1 * u]
}

fn celestial_wings_card(w: u32, h: u32) -> Canvas {
    let mut p = Posed::new(
        &[
            ("right_arm", [0.3, 0.0, 0.5]),
            ("left_arm", [0.3, 0.0, -0.5]),
            ("head", [-0.3, 0.0, 0.0]),
        ],
        w,
        h,
        200.0,
        0.8,
        0.0,
        [110.0, 170.0, 255.0],
        53,
    );
    let (cyan, violet, pink) = (
        [140.0, 225.0, 255.0],
        [160.0, 110.0, 255.0],
        [255.0, 150.0, 225.0],
    );
    let lerp = |a: [f32; 3], b: [f32; 3], k: f32| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * k);
    for side in [1.0f32, -1.0] {
        for i in 0..36 {
            for j in 1..14 {
                let (u, v) = (i as f32 / 35.0, j as f32 / 13.0);
                let [x, y, back] = wing_point(side, u, v);
                let colour = if v < 0.5 {
                    lerp(cyan, violet, v * 2.0)
                } else {
                    lerp(violet, pink, v * 2.0 - 1.0)
                };
                p.dot(x, 1.3 + y, 0.22 + back, 2, colour, 0.35);
            }
        }
        for i in 0..90 {
            let [x, y, back] = wing_point(side, i as f32 / 89.0, 0.0);
            p.dot(x, 1.3 + y, 0.22 + back, 3, [255.0, 240.0, 200.0], 0.6);
        }
    }
    // Stardust falling from the feathers.
    for k in 0..24 {
        let side = (noise(k, 1, 11) - 0.5) * 4.2;
        p.dot(
            side,
            0.2 + noise(k, 2, 11) * 0.9,
            0.4,
            2,
            [255.0, 255.0, 255.0],
            0.8,
        );
    }
    p.card
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .expect("usage: shop_items <output folder>");
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir).unwrap();
    // The card picture's shape in the launcher (292×90 at its reference size), and the banner's.
    let (cw, ch) = (480, 156);
    let (bw, bh) = (1380, 240);
    let violet = [150.0, 80.0, 255.0];

    let founders = founders_frames(&dir.join("founders_cape_texture.png"));
    let animated = |style: CapeStyle| -> Vec<Canvas> {
        (0..CAPE_FRAMES)
            .map(|k| cape_frame(&style, k as f32 / CAPE_FRAMES as f32))
            .collect()
    };
    let capes = [
        ("founders_cape", founders, violet),
        (
            "nebula_cape",
            animated(CapeStyle {
                top: hex(0x170a24),
                bottom: hex(0x3a0f45),
                band: hex(0xd94fb0),
                band_glow: hex(0xffb3e6),
                border: hex(0xf08ad6),
                hem: hex(0xb43c94),
                seed: 23,
                motion: CapeMotion::Drift,
            }),
            [230.0, 90.0, 190.0],
        ),
        (
            "aurora_cape",
            animated(CapeStyle {
                top: hex(0x07131f),
                bottom: hex(0x0d2a3a),
                band: hex(0x33d6a6),
                band_glow: hex(0xb7ffe9),
                border: hex(0x79e8ff),
                hem: hex(0x2a9cc0),
                seed: 37,
                motion: CapeMotion::Ripple,
            }),
            [60.0, 210.0, 200.0],
        ),
        (
            "zedix_cape",
            zedix_frames(&dir.join("zedix_cape_logo.png")),
            [140.0, 60.0, 255.0],
        ),
        (
            "wisp_original_cape",
            original_frames(),
            [200.0, 200.0, 215.0],
        ),
    ];
    for (id, frames, glow) in &capes {
        let texture = &frames[0];
        let file = serde_json::to_vec_pretty(&cape_item(id, frames)).unwrap();
        let item = Item::parse(&file).unwrap_or_else(|e| panic!("{id}: {e}"));
        write(dir, &format!("{id}.json"), &file);
        write(
            dir,
            &format!("{id}.png"),
            &cape_card(&item, texture, cw, ch, *glow).png(),
        );
        if !["aurora_cape", "zedix_cape", "wisp_original_cape"].contains(id) {
            // A banner slide: text on the left, the model right of centre, the art at the edge.
            let mut banner = framed_card(&item, bw, bh, 155.0, 1.05, 0.05, 0.7, *glow);
            flat_cape(&mut banner, texture, bw as i32 - 300, 24, bh as i32 - 48);
            write(dir, &format!("banner_{id}.png"), &banner.png());
        }
    }

    // Wisp's badge: every player wears it (the launcher always writes it), so it has no card.
    let file =
        serde_json::to_vec_pretty(&badge_item("wisp_badge", &wisp_badge_texture(), "#ffffff"))
            .unwrap();
    Item::parse(&file).unwrap_or_else(|e| panic!("wisp_badge: {e}"));
    write(dir, "wisp_badge.json", &file);

    let cosmic = [90.0, 140.0, 255.0];
    for (id, texture, value, glow) in [
        (
            "nova_owl",
            owl_texture(),
            owl_item as fn(&Canvas) -> Value,
            violet,
        ),
        ("cosmic_fox", fox_texture(), fox_item, cosmic),
    ] {
        let file = serde_json::to_vec_pretty(&value(&texture)).unwrap();
        let item = Item::parse(&file).unwrap_or_else(|e| panic!("{id}: {e}"));
        write(dir, &format!("{id}.json"), &file);
        write(
            dir,
            &format!("{id}.png"),
            &close_up(&item, cw, ch, 0.8, 0.5, glow).png(),
        );
        if id == "cosmic_fox" {
            // The Featured tab's tall highlight card (458×210 at reference size), and a banner
            // slide with the fox right of centre.
            write(
                dir,
                "highlight.png",
                &close_up(&item, 458, 210, 0.82, 0.5, glow).png(),
            );
            write(
                dir,
                "banner_cosmic_fox.png",
                &close_up(&item, bw, bh, 0.86, 0.7, glow).png(),
            );
        }
    }

    for (id, style, hexes, colours, seed) in [
        (
            "stardust_trail",
            "stardust",
            ["#f6eeff", "#c7a3ff", "#8a4dff"],
            vec![
                [250.0, 240.0, 255.0],
                [200.0, 160.0, 255.0],
                [140.0, 80.0, 240.0],
            ],
            5,
        ),
        (
            "comet_trail",
            "comet",
            ["#bff3ff", "#5fb4ff", "#3d4dff"],
            vec![
                [255.0, 250.0, 220.0],
                [120.0, 220.0, 255.0],
                [60.0, 90.0, 230.0],
            ],
            9,
        ),
    ] {
        let file = serde_json::to_vec_pretty(&trail_item(id, style, &hexes)).unwrap();
        Item::parse(&file).unwrap_or_else(|e| panic!("{id}: {e}"));
        write(dir, &format!("{id}.json"), &file);
        write(
            dir,
            &format!("{id}.png"),
            &trail_card(cw, ch, &colours, seed).png(),
        );
        if id == "stardust_trail" {
            write(
                dir,
                "banner_stardust_trail.png",
                &trail_card(bw, bh, &colours, seed).png(),
            );
        }
    }
    for (id, style, colours, seed) in [
        ("storm_aura", "storm", ["#ffffff", "#9fe8ff", "#5b8cff"], 71),
        (
            "starlight_orbit",
            "orbit",
            ["#fff4c2", "#ffd36b", "#c7a3ff"],
            73,
        ),
        (
            "nebula_aura",
            "nebula",
            ["#ff9bf0", "#c26bff", "#6b4dff"],
            79,
        ),
    ] {
        let file = serde_json::to_vec_pretty(&aura_item(id, style, &colours)).unwrap();
        Item::parse(&file).unwrap_or_else(|e| panic!("{id}: {e}"));
        write(dir, &format!("{id}.json"), &file);
        write(
            dir,
            &format!("{id}.png"),
            &aura_card(cw, ch, 0.5, style, &colours, seed).png(),
        );
        if id == "storm_aura" {
            write(
                dir,
                "banner_storm_aura.png",
                &aura_card(bw, bh, 0.7, style, &colours, seed).png(),
            );
        }
    }

    let daylight = serde_json::to_vec_pretty(&daylight_item(dir)).unwrap();
    let item = Item::parse(&daylight).unwrap_or_else(|e| panic!("daylight: {e}"));
    write(dir, "daylight.json", &daylight);
    if std::env::var_os("DAYLIGHT_VIEWS").is_some() {
        for yaw in [20.0, 110.0, 160.0, 200.0, 290.0] {
            let view = framed_card(&item, 420, 420, yaw, 1.0, 0.05, 0.5, [255.0, 200.0, 90.0]);
            write(dir, &format!("daylight_view_{yaw}.png"), &view.png());
        }
    }
    write(
        dir,
        "daylight.png",
        &framed_card(
            &item,
            cw,
            ch,
            DAYLIGHT_YAW,
            1.25,
            0.3,
            0.5,
            [255.0, 200.0, 90.0],
        )
        .png(),
    );

    let straw = straw_texture();
    let hat = serde_json::to_vec_pretty(&stray_item(&straw)).unwrap();
    let item = Item::parse(&hat).unwrap_or_else(|e| panic!("stray_head: {e}"));
    write(dir, "stray_head.json", &hat);
    write(
        dir,
        "stray_head.png",
        &framed_card(&item, cw, ch, 25.0, 1.9, 0.52, 0.5, [240.0, 190.0, 110.0]).png(),
    );
    if std::env::var_os("HAT_VIEWS").is_some() {
        for yaw in [25.0, 110.0, 200.0] {
            let view = framed_card(&item, 420, 420, yaw, 1.0, 0.05, 0.5, [240.0, 190.0, 110.0]);
            write(dir, &format!("hat_view_{yaw}.png"), &view.png());
        }
    }

    // The emotes' cards: the wearer in the emote's pose, its effects drawn round them.
    for (id, card) in [
        ("dark_ascension", dark_ascension_card(cw, ch)),
        ("bruh", bruh_card(cw, ch)),
        ("l_dance", l_dance_card(cw, ch)),
        ("celestial_wings", celestial_wings_card(cw, ch)),
    ] {
        write(dir, &format!("{id}.png"), &card.png());
    }

    let feathers = wing_feathers();
    let frames: Vec<Canvas> = (0..WING_FRAMES)
        .map(|k| wing_frame(k as f32 / WING_FRAMES as f32, &feathers))
        .collect();
    let wings = serde_json::to_vec_pretty(&nebula_wings_item(&frames)).unwrap();
    let item = Item::parse(&wings).unwrap_or_else(|e| panic!("nebula_wings: {e}"));
    write(dir, "nebula_wings.json", &wings);
    write(
        dir,
        "nebula_wings.png",
        &framed_card(&item, cw, ch, 200.0, 1.15, 0.22, 0.5, [200.0, 90.0, 255.0]).png(),
    );
    if std::env::var_os("WING_VIEWS").is_some() {
        for yaw in [20.0, 110.0, 160.0, 200.0] {
            let view = framed_card(&item, 420, 420, yaw, 1.0, 0.05, 0.5, [200.0, 90.0, 255.0]);
            write(dir, &format!("wings_view_{yaw}.png"), &view.png());
        }
        write(dir, "wings_frame0.png", &frames[0].png());
    }
}
