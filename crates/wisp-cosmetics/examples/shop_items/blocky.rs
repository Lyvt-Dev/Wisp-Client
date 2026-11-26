//! Minecraft-style items built of boxes in pixel art: flat shades with a few dithered texels, no
//! smooth gradients. All are built 2 units a model pixel (half-pixel detail) and drawn at half
//! scale. On the head that makes the head -8..8 across, -16..0 up, and the skin's hat layer
//! -9..9 and -18 at the top: no face here lies on those planes, so nothing flickers against it.

use super::*;
use std::rc::Rc;

/// Units per model pixel.
const UNIT: f32 = 2.0;

/// Paints one box's texels: (face, i, j, face width, face height, seed) -> colour, the faces in
/// `box_faces`' order (top, bottom, right, front, left, back).
type Paint = Rc<dyn Fn(usize, i32, i32, i32, i32, u32) -> Rgba>;

fn paint(f: impl Fn(usize, i32, i32, i32, i32, u32) -> Rgba + 'static) -> Paint {
    Rc::new(f)
}

const CLEAR: Rgba = [0, 0, 0, 0];

/// One of `pal` per texel, `base` its shade: flat, a texel here and there a step lighter or
/// darker, the way Minecraft's own textures are speckled.
fn px(pal: &[u32], base: i32, i: i32, j: i32, seed: u32) -> Rgba {
    let n = noise(i, j, seed);
    let step = if n > 0.92 {
        1
    } else if n < 0.08 {
        -1
    } else {
        0
    };
    hex(pal[(base + step).clamp(0, pal.len() as i32 - 1) as usize])
}

/// A box: origin and size in units, how it is painted.
struct Bx {
    origin: [f32; 3],
    size: [i32; 3],
    paint: Paint,
}

fn bx(origin: [f32; 3], size: [i32; 3], paint: &Paint) -> Bx {
    Bx {
        origin,
        size,
        paint: paint.clone(),
    }
}

/// A part: its boxes turn about `pivot` by `rotation` (degrees), children ride along.
struct Pt {
    name: String,
    pivot: [f32; 3],
    rotation: [f32; 3],
    boxes: Vec<Bx>,
    children: Vec<Pt>,
}

fn pt(name: &str, pivot: [f32; 3], rotation: [f32; 3], boxes: Vec<Bx>, children: Vec<Pt>) -> Pt {
    Pt {
        name: name.to_owned(),
        pivot,
        rotation,
        boxes,
        children,
    }
}

/// What an item is beyond its parts.
struct Spec {
    id: &'static str,
    slot: &'static str,
    bone: &'static str,
    offset: [f32; 3],
    rotation: [f32; 3],
    /// The texture's side (square).
    tex: i32,
    glow: bool,
    sparkles: &'static [&'static str],
    animations: Value,
}

/// The item file and its texture: every box laid out in the texture in the order met, painted,
/// and written as parts.
fn build(spec: Spec, parts: Vec<Pt>) -> (Value, Canvas) {
    let mut canvas = Canvas::new(spec.tex as u32, spec.tex as u32);
    let mut shelf = Shelf::new(spec.tex);
    let mut seed = 0u32;
    fn walk(p: &Pt, c: &mut Canvas, shelf: &mut Shelf, seed: &mut u32, tex: i32) -> Value {
        let cubes: Vec<Value> = p
            .boxes
            .iter()
            .map(|b| {
                let [w, h, d] = b.size;
                let at = shelf.place(b.size);
                assert!(
                    at[0] + 2 * (w + d) <= tex && at[1] + d + h <= tex,
                    "{}: texture full",
                    p.name
                );
                *seed += 97;
                let s = *seed;
                paint_box(c, at[0], at[1], w, h, d, |face, i, j, fw, fh| {
                    (b.paint)(face, i, j, fw, fh, s)
                });
                cube(b.origin, b.size, at)
            })
            .collect();
        let mut v = json!({ "name": p.name, "pivot": p.pivot, "rotation": p.rotation });
        if !cubes.is_empty() {
            v["cubes"] = json!(cubes);
        }
        let children: Vec<Value> = p
            .children
            .iter()
            .map(|ch| walk(ch, c, shelf, seed, tex))
            .collect();
        if !children.is_empty() {
            v["children"] = json!(children);
        }
        v
    }
    let parts: Vec<Value> = parts
        .iter()
        .map(|p| walk(p, &mut canvas, &mut shelf, &mut seed, spec.tex))
        .collect();
    let mut v = json!({
        "format": 1, "id": spec.id, "slot": spec.slot, "bone": spec.bone,
        "attach": { "offset": spec.offset, "rotation": spec.rotation, "scale": 1.0 / UNIT },
        "texture": { "png": b64(&canvas.png()) },
        "parts": parts,
    });
    if spec.glow {
        v["glow"] = json!(true);
    }
    if !spec.sparkles.is_empty() {
        v["sparkles"] = json!(spec.sparkles);
    }
    if spec.animations.as_array().is_some_and(|a| !a.is_empty()) {
        v["animations"] = spec.animations;
    }
    (v, canvas)
}

/// A head item's spec.
fn head(id: &'static str, animations: Value) -> Spec {
    Spec {
        id,
        slot: "head",
        bone: "head",
        offset: [0.0; 3],
        rotation: [0.0; 3],
        tex: 256,
        glow: false,
        sparkles: &[],
        animations,
    }
}

/// The shade step light gives a face: the top a step up, the bottom a step down.
fn lit(face: usize) -> i32 {
    match face {
        0 => 1,
        1 => -1,
        _ => 0,
    }
}

/// A plain material: `pal` dark to light, the faces lit, a few texels dithered.
fn solid(pal: &'static [u32], base: i32) -> Paint {
    paint(move |face, i, j, _, _, s| px(pal, base + lit(face), i, j, s))
}

/// Like `solid`, a darker outline round each face, as Minecraft's blocky hats are drawn.
fn framed(pal: &'static [u32], base: i32) -> Paint {
    paint(move |face, i, j, fw, fh, s| {
        let edge = i == 0 || j == 0 || i == fw - 1 || j == fh - 1;
        px(pal, base + lit(face) - edge as i32, i, j, s)
    })
}

/// A cone of stacked boxes, base to tip, each (width, height, bend back, bend aside) from the one
/// below it, the first standing on `base`; `tip` is added to the last.
fn cone(
    name: &str,
    base: [f32; 3],
    segments: &[(i32, i32, f32, f32)],
    paint: &Paint,
    tip: Vec<Bx>,
) -> Pt {
    let mut tip = Some(tip);
    let mut child: Option<Pt> = None;
    for (k, &(w, h, back, side)) in segments.iter().enumerate().rev() {
        let pivot = if k == 0 {
            base
        } else {
            [0.0, -(segments[k - 1].1 as f32) + 1.0, 0.0]
        };
        let half = w as f32 / 2.0;
        let mut boxes = vec![bx([-half, -h as f32, -half], [w, h, w], paint)];
        if k == segments.len() - 1 {
            boxes.extend(tip.take().unwrap());
        }
        child = Some(pt(
            &format!("{name}_{k}"),
            pivot,
            [back, 0.0, side],
            boxes,
            child.into_iter().collect(),
        ));
    }
    child.unwrap()
}

// ---------------------------------------------------------------- palettes

const GOLD: &[u32] = &[0x7a4e0c, 0xb07818, 0xe0a828, 0xffd050, 0xfff0a0];
const LEATHER: &[u32] = &[0x2e1a0c, 0x45281a, 0x5e3a22, 0x784c2c, 0x926038];
const GLASS: &[u32] = &[0x0c1014, 0x182028, 0x26323e];
const IRON: &[u32] = &[0x3a3e46, 0x5a6068, 0x80868e, 0xa8aeb6, 0xd0d4da];
const WHITE: &[u32] = &[0x9aa0a8, 0xc4c8ce, 0xe2e4e8, 0xf6f7f8, 0xffffff];
const BLACK: &[u32] = &[0x0e0e12, 0x1c1c22, 0x2c2c34, 0x3e3e48];

// ---------------------------------------------------------------- Blossom Shades

/// The flower lens' side in texels.
const LENS: i32 = 12;

/// Whether texel (`i`, `j`) of a flower lens is clear (None), the lens (true) or the frame.
fn flower(i: i32, j: i32) -> Option<bool> {
    let c = LENS as f32 / 2.0;
    let (dx, dy) = (i as f32 + 0.5 - c, j as f32 + 0.5 - c);
    let r = dx.hypot(dy) / (LENS as f32 / 10.0);
    let a = dy.atan2(dx) + std::f32::consts::FRAC_PI_2;
    if r > 3.7 + 1.4 * (5.0 * a).cos().max(-0.2) {
        return None;
    }
    Some(r < 2.4)
}

/// Blossom Shades: two pink flower-shaped frames with rosy lenses, thin arms to the ears.
pub fn glasses() -> (Value, Canvas) {
    const PINK: &[u32] = &[0x8a2e5a, 0xc4588a, 0xee8ab4, 0xffb6d4, 0xffe2ee];
    let lens = paint(|face, i, j, _, _, s| {
        if face != 3 && face != 5 {
            return CLEAR;
        }
        match flower(i, j) {
            None => CLEAR,
            Some(true) if (i == 4 && j == 4) || (i == 5 && j == 4) => hex(0xfff4f8),
            Some(true) => px(&[0x9a2c48, 0xc84a68, 0xe8789a], 1, i, j, s),
            Some(false) => {
                let lower = (j > 5) as i32;
                px(PINK, 3 - lower, i, j, s)
            }
        }
    });
    let frame = solid(PINK, 2);
    let tip = solid(PINK, 1);
    let y = -13.0;
    let glasses = pt(
        "glasses",
        [0.0, 0.0, 0.0],
        [0.0; 3],
        vec![
            bx([-12.5, y, -10.5], [LENS, LENS, 1], &lens),
            bx([0.5, y, -10.5], [LENS, LENS, 1], &lens),
            bx([-1.0, y + 5.0, -10.5], [2, 1, 1], &frame),
            bx([9.5, y + 3.0, -9.5], [1, 1, 17], &frame),
            bx([-10.5, y + 3.0, -9.5], [1, 1, 17], &frame),
            bx([9.5, y + 4.0, 6.5], [1, 3, 1], &tip),
            bx([-10.5, y + 4.0, 6.5], [1, 3, 1], &tip),
        ],
        vec![],
    );
    let mut spec = head("blossom_shades", json!([]));
    spec.tex = 64;
    build(spec, vec![glasses])
}

// ---------------------------------------------------------------- hats

/// The Gearwork Topper: a tall leather top hat with a gold band and brass goggles on the front.
pub fn gearwork_topper() -> (Value, Canvas) {
    let crown = paint(|face, i, j, _, _, s| {
        let seam = face >= 2 && i % 5 == 0;
        px(LEATHER, 2 + lit(face) - seam as i32, i, j, s)
    });
    let brim = framed(LEATHER, 1);
    let band = solid(GOLD, 2);
    let lens = paint(|_, i, j, fw, fh, s| {
        if i == 0 || j == 0 || i == fw - 1 || j == fh - 1 {
            px(GOLD, 3, i, j, s)
        } else if i == 1 && j == 1 {
            hex(0x9ad8f0)
        } else {
            px(GLASS, 1, i, j, s)
        }
    });
    let hat = pt(
        "hat",
        [0.0; 3],
        [0.0; 3],
        vec![
            bx([-15.0, -19.0, -15.0], [30, 2, 30], &brim),
            bx([-10.0, -33.0, -10.0], [20, 14, 20], &crown),
            bx([-10.5, -23.0, -10.5], [21, 3, 21], &band),
            bx([-8.0, -28.0, -12.5], [6, 6, 2], &lens),
            bx([2.0, -28.0, -12.5], [6, 6, 2], &lens),
            bx([-2.0, -26.0, -11.5], [4, 2, 1], &band),
        ],
        vec![],
    );
    build(head("gearwork_topper", json!([])), vec![hat])
}

/// The Spelunker Helmet: a yellow hard hat with a black stripe and a lamp on the front.
pub fn spelunker_helmet() -> (Value, Canvas) {
    const YELLOW: &[u32] = &[0x8a6a08, 0xc49a10, 0xf0c41c, 0xffe050, 0xfff4a0];
    let shell = paint(|face, i, j, fw, _, s| {
        let stripe = (face == 0 || face == 3 || face == 5) && (i - fw / 2).abs() <= 1;
        if stripe && face == 0 {
            px(BLACK, 2, i, j, s)
        } else {
            px(YELLOW, 2 + lit(face), i, j, s)
        }
    });
    let top = paint(|face, i, j, fw, _, s| {
        if (i - fw / 2).abs() <= 1 && face != 1 {
            px(BLACK, 2 + lit(face), i, j, s)
        } else {
            px(YELLOW, 3 + lit(face), i, j, s)
        }
    });
    let lip = framed(YELLOW, 2);
    let housing = framed(IRON, 1);
    let lamp = paint(|_, i, j, fw, fh, _| {
        let (dx, dy) = (i as f32 + 0.5 - fw as f32 / 2.0, j as f32 + 0.5 - fh as f32 / 2.0);
        hex(if dx.hypot(dy) < 1.2 { 0xffffff } else { 0xfff2b0 })
    });
    let hat = pt(
        "hat",
        [0.0; 3],
        [0.0; 3],
        vec![
            bx([-10.0, -24.0, -10.0], [20, 8, 20], &shell),
            bx([-8.0, -26.0, -8.0], [16, 2, 16], &top),
            bx([-12.0, -17.5, -12.0], [24, 2, 24], &lip),
            bx([-3.5, -24.0, -13.0], [7, 7, 3], &housing),
            bx([-2.5, -23.0, -14.0], [5, 5, 1], &lamp),
        ],
        vec![],
    );
    let mut spec = head("spelunker_helmet", json!([]));
    spec.glow = false;
    build(spec, vec![hat])
}

/// The Frost Wizard Hat: a pointed ice-blue hat bending back at the tip, a leather band with a
/// gold buckle, a gold star at the point.
pub fn frost_wizard_hat() -> (Value, Canvas) {
    const ICE: &[u32] = &[0x0f4a5a, 0x167a8a, 0x24a4b4, 0x4cc8d4, 0x8ce8ee];
    let cloth = paint(|face, i, j, _, _, s| {
        let fold = face >= 2 && (i + j / 3) % 6 == 0;
        px(ICE, 2 + lit(face) - fold as i32, i, j, s)
    });
    let brim = framed(ICE, 2);
    let band = solid(LEATHER, 2);
    let buckle = paint(|face, i, j, fw, fh, _| {
        if face != 3 {
            return hex(GOLD[2]);
        }
        let (dx, dy) = (
            (i as f32 + 0.5 - fw as f32 / 2.0).abs(),
            (j as f32 + 0.5 - fh as f32 / 2.0).abs(),
        );
        if dx + dy < 1.1 {
            hex(0xff8a20)
        } else if dx + dy < 2.1 {
            hex(GOLD[3])
        } else {
            CLEAR
        }
    });
    let star = solid(GOLD, 3);
    let segments = [
        (20, 6, 0.0, 0.0),
        (16, 6, -6.0, 0.0),
        (12, 6, -10.0, 4.0),
        (8, 6, -18.0, 6.0),
        (5, 5, -26.0, 10.0),
    ];
    let tip = vec![bx([-1.5, -7.0, -1.5], [3, 3, 3], &star)];
    let hat = pt(
        "hat",
        [0.0; 3],
        [0.0; 3],
        vec![
            bx([-17.0, -19.0, -17.0], [34, 2, 34], &brim),
            bx([-10.5, -22.5, -10.5], [21, 3, 21], &band),
            bx([-2.0, -23.0, -11.5], [4, 4, 1], &buckle),
        ],
        vec![cone("cone", [0.0, -18.5, 0.0], &segments, &cloth, tip)],
    );
    let anim = json!([
        { "part": "cone_4", "motion": "swing", "axis": "z", "amplitude": 6.0, "period": 40 },
    ]);
    build(head("frost_wizard_hat", anim), vec![hat])
}

/// The Tangerine Topper: a tall orange hat with a white stripe, its crown leaning over to one side.
pub fn tangerine_topper() -> (Value, Canvas) {
    const ORANGE: &[u32] = &[0x8a3a06, 0xc45a0a, 0xf07c14, 0xffa040, 0xffc880];
    let crown = paint(|face, i, j, fw, fh, s| {
        if (face == 3 || face == 5 || face == 0) && (i - fw / 2).abs() <= 1 {
            return px(WHITE, 3 + lit(face), i, j, s);
        }
        let edge = i == 0 || i == fw - 1 || (face >= 2 && j == fh - 1);
        px(ORANGE, 2 + lit(face) - edge as i32, i, j, s)
    });
    let brim = framed(ORANGE, 2);
    let segments = [(18, 8, 0.0, 0.0), (18, 8, 0.0, 12.0), (17, 7, -4.0, 18.0)];
    let hat = pt(
        "hat",
        [0.0; 3],
        [0.0; 3],
        vec![bx([-15.0, -19.0, -15.0], [30, 2, 30], &brim)],
        vec![cone("crown", [0.0, -18.5, 0.0], &segments, &crown, vec![])],
    );
    build(head("tangerine_topper", json!([])), vec![hat])
}

/// The Panda Cap: a white cap with a panda's face on the front, round black ears and cheek flaps.
pub fn panda_cap() -> (Value, Canvas) {
    let cap = paint(|face, i, j, fw, _, s| {
        if face == 3 {
            // The face: eye patches with a white glint, a nose, a small smile.
            let eye = |cx: f32| {
                let (dx, dy) = ((i as f32 + 0.5 - cx) / 2.6, (j as f32 + 0.5 - 4.5) / 2.2);
                dx * dx + dy * dy < 1.0
            };
            let (l, r) = (fw as f32 * 0.27, fw as f32 * 0.73);
            if (i == (l as i32) && j == 4) || (i == (r as i32) && j == 4) {
                return hex(0xffffff);
            }
            if eye(l) || eye(r) {
                return px(BLACK, 1, i, j, s);
            }
            let mid = fw / 2;
            let nose = j == 6 && (i == mid - 1 || i == mid);
            let smile = (j == 8 && (i == mid - 2 || i == mid + 1)) || (j == 9 && (i == mid - 1 || i == mid));
            if nose || smile {
                return px(BLACK, 0, i, j, s);
            }
        }
        px(WHITE, 2 + lit(face), i, j, s)
    });
    let ear = framed(BLACK, 2);
    let flap = solid(WHITE, 2);
    let hat = pt(
        "hat",
        [0.0; 3],
        [0.0; 3],
        vec![
            bx([-10.0, -24.0, -10.0], [20, 13, 20], &cap),
            bx([-11.0, -29.0, -2.0], [6, 6, 4], &ear),
            bx([5.0, -29.0, -2.0], [6, 6, 4], &ear),
            bx([-11.5, -12.0, -6.0], [2, 8, 12], &flap),
            bx([9.5, -12.0, -6.0], [2, 8, 12], &flap),
        ],
        vec![],
    );
    build(head("panda_cap", json!([])), vec![hat])
}

/// The Buccaneer Tricorn: a brown pirate hat, its brim turned up all round and trimmed in gold.
pub fn buccaneer_tricorn() -> (Value, Canvas) {
    let wood = paint(|face, i, j, _, _, s| {
        let grain = (j % 3 == 2) as i32;
        px(LEATHER, 2 + lit(face) - grain, i, j, s)
    });
    let wall = paint(|face, i, j, fw, _, s| {
        if face >= 2 && j <= 1 {
            // A gold trim with a notched crest along it.
            if j == 0 && i % 4 != 1 {
                return CLEAR;
            }
            return px(GOLD, 3, i, j, s);
        }
        if face == 0 {
            return px(GOLD, 2, i, j, s);
        }
        let _ = fw;
        px(LEATHER, 1 + (face == 3 || face == 5) as i32, i, j, s)
    });
    let skull = paint(|face, i, j, _, _, _| {
        if face != 3 {
            return hex(WHITE[2]);
        }
        let eye = j == 1 && (i == 1 || i == 3);
        hex(if eye { BLACK[0] } else { WHITE[3] })
    });
    let walls = |name: &str, pivot: [f32; 3], rot: [f32; 3], origin: [f32; 3], size: [i32; 3]| {
        pt(name, pivot, rot, vec![bx(origin, size, &wall)], vec![])
    };
    let hat = pt(
        "hat",
        [0.0; 3],
        [0.0; 3],
        vec![
            bx([-8.5, -28.0, -8.5], [17, 10, 17], &wood),
            bx([-15.0, -19.0, -15.0], [30, 2, 30], &wood),
        ],
        vec![
            walls(
                "front",
                [0.0, -18.0, -14.0],
                [-30.0, 0.0, 0.0],
                [-15.0, -9.0, -1.0],
                [30, 9, 2],
            ),
            walls(
                "back",
                [0.0, -18.0, 14.0],
                [30.0, 0.0, 0.0],
                [-15.0, -10.0, -1.0],
                [30, 10, 2],
            ),
            walls(
                "left",
                [14.0, -18.0, 0.0],
                [0.0, 0.0, -30.0],
                [-1.0, -8.0, -13.0],
                [2, 8, 26],
            ),
            walls(
                "right",
                [-14.0, -18.0, 0.0],
                [0.0, 0.0, 30.0],
                [-1.0, -8.0, -13.0],
                [2, 8, 26],
            ),
            pt(
                "skull",
                [0.0, -18.0, -14.0],
                [-30.0, 0.0, 0.0],
                vec![bx([-2.5, -7.0, -2.0], [5, 4, 1], &skull)],
                vec![],
            ),
        ],
    );
    build(head("buccaneer_tricorn", json!([])), vec![hat])
}

/// The Orbit Cap: a deep-blue dome with a tilted ring round it and three little planets
/// circling.
pub fn orbit_cap() -> (Value, Canvas) {
    const SPACE: &[u32] = &[0x0e2a5a, 0x16448a, 0x2266b8, 0x3e8ee0, 0x86c4ff];
    let dome = paint(|face, i, j, _, _, s| {
        if noise(i, j, s + 5) > 0.93 {
            return hex(0xeaf4ff);
        }
        px(SPACE, 2 + lit(face), i, j, s)
    });
    let ring = paint(|face, i, j, fw, fh, _| {
        if face > 1 {
            return CLEAR;
        }
        let (dx, dy) = (i as f32 + 0.5 - fw as f32 / 2.0, j as f32 + 0.5 - fh as f32 / 2.0);
        let r = dx.hypot(dy);
        if !(11.0..17.0).contains(&r) {
            return CLEAR;
        }
        let band = ((r - 11.0) / 2.0) as usize % 3;
        hex([0xc8a868, 0xf0dca8, 0xa88a50][band])
    });
    let planet = |pal: &'static [u32]| {
        paint(move |face, i, j, _, _, s| {
            let band = (j % 3 == 1) as i32;
            px(pal, 2 + lit(face) - band, i, j, s)
        })
    };
    let sun = planet(&[0x8a6a08, 0xd8a818, 0xffd040, 0xfff090]);
    let green = planet(&[0x1a5a1e, 0x2e8a30, 0x56c04a, 0x9ae878]);
    let purple = planet(&[0x3a145a, 0x5e2490, 0x8a42c8, 0xc28af0]);
    let tilt = [6.0, 0.0, -8.0];
    let hat = pt(
        "hat",
        [0.0; 3],
        [0.0; 3],
        vec![
            bx([-10.0, -24.0, -10.0], [20, 8, 20], &dome),
            bx([-7.0, -26.0, -7.0], [14, 2, 14], &dome),
        ],
        vec![
            pt(
                "ring",
                [0.0, -20.0, 0.0],
                tilt,
                vec![bx([-17.0, -0.5, -17.0], [34, 1, 34], &ring)],
                vec![],
            ),
            pt(
                "orbit",
                [0.0, -21.0, 0.0],
                tilt,
                vec![
                    bx([12.0, -2.0, -2.0], [4, 4, 4], &sun),
                    bx([-12.0, -1.5, 7.0], [3, 3, 3], &green),
                    bx([-9.0, -2.5, -14.0], [5, 5, 5], &purple),
                ],
                vec![],
            ),
        ],
    );
    let anim = json!([
        { "part": "orbit", "motion": "spin", "axis": "y", "amplitude": 0, "period": 160 },
    ]);
    build(head("orbit_cap", anim), vec![hat])
}

/// The Propeller Cap: a patchwork cap in red, green and yellow, a blue grid visor and a
/// spinning propeller on top.
pub fn propeller_cap() -> (Value, Canvas) {
    let patch = paint(|face, i, j, fw, _, s| {
        let third = if face == 0 || face == 1 {
            (j * 3 / 20).clamp(0, 2)
        } else {
            (i * 3 / fw.max(1)).clamp(0, 2)
        };
        let pal: &[u32] = match third {
            0 => &[0x7a0e14, 0xb81e28, 0xe8323c, 0xff6a6a],
            1 => &[0x2a5a10, 0x4a8a1c, 0x78c030, 0xaae860],
            _ => &[0x8a6a08, 0xd0a010, 0xf8d030, 0xfff080],
        };
        let plaid = (i % 5 == 0 || j % 5 == 0) as i32;
        px(pal, 2 + lit(face) - plaid, i, j, s)
    });
    const BLUE: &[u32] = &[0x0a2a6a, 0x1448a8, 0x2a6ee0, 0x5a9cff];
    let visor = paint(|face, i, j, _, _, s| {
        let grid = (i % 4 == 0 || j % 4 == 0) as i32;
        px(BLUE, 2 + lit(face) + grid, i, j, s)
    });
    let stem = solid(IRON, 2);
    let blades = paint(|face, i, j, fw, _, s| {
        let end = i < 3 || i >= fw - 3;
        if end {
            px(&[0x8a6a08, 0xd0a010, 0xf8d030, 0xfff080], 2 + lit(face), i, j, s)
        } else {
            px(&[0x7a0e14, 0xb81e28, 0xe8323c, 0xff6a6a], 2 + lit(face), i, j, s)
        }
    });
    let hat = pt(
        "hat",
        [0.0; 3],
        [0.0; 3],
        vec![
            bx([-10.0, -24.0, -10.0], [20, 8, 20], &patch),
            bx([-10.0, -17.0, -22.0], [20, 1, 12], &visor),
            bx([-1.0, -28.0, -1.0], [2, 4, 2], &stem),
        ],
        vec![pt(
            "propeller",
            [0.0, -28.0, 0.0],
            [0.0; 3],
            vec![bx([-9.0, -1.0, -1.5], [18, 1, 3], &blades)],
            vec![],
        )],
    );
    let anim = json!([
        { "part": "propeller", "motion": "spin", "axis": "y", "amplitude": 0, "period": 10 },
    ]);
    build(head("propeller_cap", anim), vec![hat])
}

/// The Neon Headset: a cyan visor wrapped round the eyes, dark ear cups, a band over the top and
/// two antennas.
pub fn neon_headset() -> (Value, Canvas) {
    const CYAN: &[u32] = &[0x0c5a6e, 0x1290a8, 0x2cc4d8, 0x7aeaf4, 0xc8fcff];
    let visor = paint(|face, i, j, _, fh, s| {
        if face >= 2 && j > 0 && j < fh - 1 && face == 3 {
            return if (i + j) % 7 == 0 {
                hex(0x6af0ff)
            } else {
                px(GLASS, 1, i, j, s)
            };
        }
        px(CYAN, 2 + lit(face), i, j, s)
    });
    let band = framed(CYAN, 2);
    let cup = paint(|face, i, j, fw, fh, s| {
        let ring = face == 2 || face == 4;
        let (dx, dy) = (i as f32 + 0.5 - fw as f32 / 2.0, j as f32 + 0.5 - fh as f32 / 2.0);
        if ring && dx.hypot(dy) < 2.0 {
            return hex(0x6af0ff);
        }
        px(BLACK, 2 + lit(face), i, j, s)
    });
    let antenna = solid(BLACK, 2);
    let bulb = solid(CYAN, 3);
    let aerial = |name: &str, x: f32, lean: f32| {
        pt(
            name,
            [x, -19.0, 0.0],
            [0.0, 0.0, lean],
            vec![
                bx([-0.5, -6.0, -0.5], [1, 6, 1], &antenna),
                bx([-1.0, -8.0, -1.0], [2, 2, 2], &bulb),
            ],
            vec![],
        )
    };
    let hat = pt(
        "hat",
        [0.0; 3],
        [0.0; 3],
        vec![
            bx([-11.5, -12.0, -11.5], [23, 6, 2], &visor),
            bx([-11.5, -12.0, 9.5], [23, 6, 2], &band),
            bx([9.5, -12.0, -9.5], [2, 6, 19], &band),
            bx([-11.5, -12.0, -9.5], [2, 6, 19], &band),
            bx([11.5, -13.0, -4.0], [3, 8, 8], &cup),
            bx([-14.5, -13.0, -4.0], [3, 8, 8], &cup),
            bx([-12.0, -20.0, -1.5], [24, 2, 3], &band),
            bx([10.0, -18.0, -1.5], [2, 5, 3], &band),
            bx([-12.0, -18.0, -1.5], [2, 5, 3], &band),
        ],
        vec![aerial("aerial_l", 6.0, 18.0), aerial("aerial_r", -6.0, -18.0)],
    );
    let anim = json!([
        { "part": "aerial_l", "motion": "swing", "axis": "z", "amplitude": 5.0, "period": 36 },
        { "part": "aerial_r", "motion": "swing", "axis": "z", "amplitude": -5.0, "period": 36, "phase": 0.5 },
    ]);
    build(head("neon_headset", anim), vec![hat])
}

// ---------------------------------------------------------------- Crimson Katana

/// The Crimson Katana: a red lacquered scabbard with gold fittings, a gold guard and a grip wound
/// in black over red, carried across the back.
pub fn crimson_katana() -> (Value, Canvas) {
    const LACQUER: &[u32] = &[0x4a0a12, 0x7a1220, 0xa81c2c, 0xd03a46, 0xf06a70];
    let saya = paint(|face, i, j, _, fh, s| {
        if j == 3 || j == 4 || j == fh - 6 {
            return px(GOLD, 2 + lit(face), i, j, s);
        }
        let shine = (face == 3 && i == 0) as i32;
        px(LACQUER, 2 + lit(face) + shine, i, j, s)
    });
    let cap = solid(BLACK, 2);
    let guard = paint(|face, i, j, fw, fh, s| {
        let rim = i == 0 || j == 0 || i == fw - 1 || j == fh - 1;
        px(GOLD, 2 + lit(face) - rim as i32, i, j, s)
    });
    let grip = paint(|face, i, j, _, _, s| {
        // Black cord wound in diamonds over the red underneath.
        let k = (j + if i % 2 == 0 { 0 } else { 2 }) % 4;
        if face < 2 {
            px(BLACK, 1, i, j, s)
        } else if k == 0 {
            px(LACQUER, 3, i, j, s)
        } else {
            px(BLACK, 1 + (k == 2) as i32, i, j, s)
        }
    });
    let pommel = solid(GOLD, 2);
    let sword = pt(
        "katana",
        [0.0; 3],
        [0.0; 3],
        vec![
            bx([-1.5, -26.0, -1.0], [3, 30, 2], &saya),
            bx([-1.5, -28.0, -1.0], [3, 2, 2], &cap),
            bx([-3.0, 4.0, -2.5], [6, 1, 5], &guard),
            bx([-1.0, 5.0, -1.0], [2, 12, 2], &grip),
            bx([-1.5, 17.0, -1.5], [3, 1, 3], &pommel),
        ],
        vec![],
    );
    let spec = Spec {
        id: "crimson_katana",
        slot: "weapon",
        bone: "body",
        offset: [0.0, 6.0, 3.0],
        rotation: [0.0, 0.0, 145.0],
        tex: 64,
        glow: false,
        sparkles: &[],
        animations: json!([]),
    };
    build(spec, vec![sword])
}

// ---------------------------------------------------------------- Butterfly Swarm

/// A butterfly's wing, the body at column 0, the front at row 0: outline, colour, light spot.
const WING: [&str; 9] = [
    "......KK",
    "....KKOK",
    "..KKOOLK",
    "KKOOLWOK",
    "KOOOOOK.",
    "KOLOKK..",
    "KOOOOK..",
    "KKOLOK..",
    ".KKKK...",
];

/// A wing (top and bottom of a thin plate) in `pal` (dark, colour, light), mirrored for the
/// wearer's right with `flip`.
fn wing_paint(pal: [u32; 3], flip: bool) -> Paint {
    paint(move |face, i, j, fw, _, _| {
        if face > 1 {
            return CLEAR;
        }
        let i = if flip { fw - 1 - i } else { i };
        match WING[j as usize % 9].as_bytes()[i as usize % 8] {
            b'K' => hex(pal[0]),
            b'O' => hex(pal[1]),
            b'L' => hex(pal[2]),
            b'W' => hex(0xffffff),
            _ => CLEAR,
        }
    })
}

/// The Butterfly Swarm: six pixel butterflies, blue and orange, fluttering round you on three
/// rings at different heights and speeds.
pub fn butterfly_swarm() -> (Value, Canvas) {
    let blue = [0x0a1c3a, 0x2a8ce8, 0x8ad8ff];
    let orange = [0x1a0e06, 0xf07c14, 0xffc860];
    let body = solid(BLACK, 1);
    let rings = [(-4.0, 27.0, 150.0), (8.0, 28.0, 190.0), (20.0, 26.0, 130.0)];
    let mut parts = Vec::new();
    let mut anims = Vec::new();
    for (k, &(y, r, period)) in rings.iter().enumerate() {
        let mut flies = Vec::new();
        for (n, side) in [1.0f32, -1.0].into_iter().enumerate() {
            let pal = if (k + n) % 2 == 0 { blue } else { orange };
            let left = wing_paint(pal, false);
            let right = wing_paint(pal, true);
            let x = side * r;
            let name = format!("fly{k}{n}");
            flies.push(pt(
                &format!("{name}_l"),
                [x, 0.0, 0.0],
                [0.0, 0.0, -25.0],
                vec![
                    bx([0.0, -0.5, -4.5], [8, 1, 9], &left),
                    bx([-0.5, -0.5, -3.0], [1, 1, 6], &body),
                ],
                vec![],
            ));
            flies.push(pt(
                &format!("{name}_r"),
                [x, 0.0, 0.0],
                [0.0, 0.0, 25.0],
                vec![bx([-8.0, -0.5, -4.5], [8, 1, 9], &right)],
                vec![],
            ));
            let phase = (k * 2 + n) as f32 * 0.17;
            anims.push(json!({ "part": format!("{name}_l"), "motion": "swing", "axis": "z", "amplitude": 35.0, "period": 8, "phase": phase }));
            anims.push(json!({ "part": format!("{name}_r"), "motion": "swing", "axis": "z", "amplitude": -35.0, "period": 8, "phase": phase }));
        }
        let ring = format!("ring{k}");
        anims.push(json!({ "part": ring, "motion": "spin", "axis": "y", "amplitude": 0, "period": period }));
        anims.push(json!({ "part": ring, "motion": "bob", "axis": "y", "amplitude": 2.0, "period": 50 + k * 13 }));
        parts.push(pt(&ring, [0.0, y, 0.0], [0.0, k as f32 * 60.0, 0.0], vec![], flies));
    }
    let spec = Spec {
        id: "butterfly_swarm",
        slot: "shoulder",
        bone: "body",
        offset: [0.0; 3],
        rotation: [0.0; 3],
        tex: 128,
        glow: false,
        sparkles: &[],
        animations: json!(anims),
    };
    build(spec, parts)
}

// ---------------------------------------------------------------- Astro Buddy

/// Astro Buddy: a little astronaut in a black suit floating by your shoulder, a gold-rimmed visor
/// with a happy face, a jetpack puffing sparks.
pub fn astro_buddy() -> (Value, Canvas) {
    const SUIT: &[u32] = &[0x1c1c24, 0x2a2a34, 0x3a3a46, 0x4e4e5c, 0x6a6a7a];
    let suit = solid(SUIT, 3);
    let helmet = paint(|face, i, j, fw, fh, s| {
        if face == 3 {
            let (x0, x1, y0, y1) = (1, fw - 2, 2, fh - 2);
            if (x0..=x1).contains(&i) && (y0..=y1).contains(&j) {
                let rim = i == x0 || i == x1 || j == y0 || j == y1;
                if rim {
                    return px(GOLD, 3, i, j, s);
                }
                // ^ ^ eyes and a blush.
                let eye = (j == y0 + 2 && (i == x0 + 2 || i == x1 - 2))
                    || (j == y0 + 3 && (i == x0 + 1 || i == x0 + 3 || i == x1 - 1 || i == x1 - 3));
                if eye {
                    return hex(0x1a1a22);
                }
                if j == y1 - 1 && (i == x0 + 1 || i == x1 - 1) {
                    return hex(0xf08aa0);
                }
                return hex(0xf6e0d0);
            }
        }
        if face >= 2 && j == fh - 1 {
            return px(GOLD, 2, i, j, s);
        }
        px(SUIT, 3 + lit(face), i, j, s)
    });
    let belt = paint(|face, i, j, _, fh, s| {
        if face >= 2 && j == fh - 2 {
            px(GOLD, 2, i, j, s)
        } else {
            px(SUIT, 3 + lit(face), i, j, s)
        }
    });
    let pack = paint(|face, i, j, _, _, s| {
        if face == 5 && j == 1 && i % 2 == 1 {
            hex(0xff8a20)
        } else {
            px(IRON, 1 + lit(face), i, j, s)
        }
    });
    let flame = paint(|_, i, j, _, fh, s| {
        let t = j as f32 / fh as f32;
        px(&[0xff4a10, 0xff9a20, 0xffe060], 2 - (t * 2.5) as i32, i, j, s)
    });
    let tip = paint(|_, _, _, _, _, _| hex(0xff8a20));
    let arm = |name: &str, x: f32| {
        pt(
            name,
            [x, -4.0, 0.0],
            [0.0; 3],
            vec![bx([-1.0, 0.0, -1.0], [2, 5, 2], &suit)],
            vec![],
        )
    };
    let buddy = pt(
        "buddy",
        [-24.0, -10.0, 2.0],
        [0.0, 20.0, 0.0],
        vec![
            bx([-6.0, -16.0, -6.0], [12, 11, 12], &helmet),
            bx([-3.5, -5.0, -2.5], [7, 6, 5], &belt),
            bx([-3.0, 1.0, -1.0], [2, 4, 2], &suit),
            bx([1.0, 1.0, -1.0], [2, 4, 2], &suit),
            bx([-3.0, -5.0, 2.5], [6, 6, 3], &pack),
            bx([-2.0, 1.0, 3.0], [4, 3, 2], &flame),
            bx([-0.5, -19.0, -0.5], [1, 3, 1], &suit),
            bx([-1.0, -21.0, -1.0], [2, 2, 2], &tip),
        ],
        vec![arm("arm_l", 4.5), arm("arm_r", -4.5)],
    );
    let spec = Spec {
        id: "astro_buddy",
        slot: "shoulder",
        bone: "body",
        offset: [0.0; 3],
        rotation: [0.0; 3],
        tex: 64,
        glow: false,
        sparkles: &["#ffb040", "#ff6a20", "#fff0a0"],
        animations: json!([
            { "part": "buddy", "motion": "bob", "axis": "y", "amplitude": 1.5, "period": 50 },
            { "part": "buddy", "motion": "swing", "axis": "z", "amplitude": 6.0, "period": 80 },
            { "part": "arm_l", "motion": "swing", "axis": "z", "amplitude": 20.0, "period": 30 },
            { "part": "arm_r", "motion": "swing", "axis": "x", "amplitude": 25.0, "period": 26, "phase": 0.5 },
        ]),
    };
    build(spec, vec![buddy])
}
