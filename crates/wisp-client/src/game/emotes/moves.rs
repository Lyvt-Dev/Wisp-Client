//! The emotes that move the player without a lens of their own: Dark Ascension, Bruh, the L
//! Dance and Celestial Wings. Each is a pose over time (`moves_at`), which the player's own
//! model and other players' models take alike, particles and sounds (`moves_effects`), and a
//! camera that frames the player (`moves_camera`).
//!
//! - Dark Ascension: black mist creeps in along the ground, the player rises with arms held out
//!   and head thrown back as dark strands spiral round them, their eyes burning violet; the dark
//!   gathers into them, bursts outward in a ring, and sets them down.
//! - Bruh: the player looks on, then claps a hand to their face, bows their head and shakes it,
//!   sighs, and drags the hand down their face.
//! - L Dance: an L held to the forehead while they bounce to a note-block beat, stepping and
//!   swaying, the free fist pumping; halfway through the hands swap.
//! - Celestial Wings: starlight gathers at the player's back and unfurls into two great wings
//!   of light that beat slowly, lifting them off the ground, then scatter into stardust.
//! - Sit: the player sits down on the ground, arms on their knees, until they move; the mouse
//!   turns the camera round them.
//! - The ragebait ones, page two of the wheel: Crouch Spam (crouching on and off over the spot),
//!   Skill Issue (a slow shrug under a "?" to a sad trombone), Too Easy (a look at the wrist, then
//!   a long yawn), Point and Laugh (pointing at the viewer and doubling over) and Cry About It
//!   (mock crying, fists at the eyes, then a little wave bye-bye).
use super::super::Game;
use super::{Emote, VIOLET, WHITE, direction, smoothstep, wrap, yaw_of};
use std::f64::consts::{PI, TAU};
use wisp_client::hud::{HudAtlas, HudBatch};
use wisp_client::particles::{ParticleOptions, ParticleType};
use wisp_core::geometry::Vec3;
use wisp_core::random::RandomSource;

// Dark Ascension, in seconds: it starts rising, is risen, bursts, falls, ends.
const DARK_RISE: f64 = 0.8;
const DARK_RISEN: f64 = 4.0;
const DARK_BURST: f64 = 6.0;
const DARK_FALL: f64 = 8.0;
pub(super) const DARK_END: f64 = 10.0;
/// How high it lifts the player, in blocks.
const DARK_TOP: f64 = 1.4;
const DARK_DEEP: [f32; 3] = [0.28, 0.06, 0.42];
const DARK_BLACK: [f32; 3] = [0.03, 0.0, 0.05];
const DARK_EYES: [f32; 3] = [0.8, 0.35, 1.0];

// Bruh: the hand reaches the face, then drags down it.
const BRUH_SLAP: f64 = 1.05;
const BRUH_DRAG: f64 = 2.9;
pub(super) const BRUH_END: f64 = 4.2;

// The L Dance: 16 beats at 120 per minute.
const BEAT: f64 = 0.5;
const DANCE_START: f64 = 0.4;
const DANCE_BEATS: u32 = 16;
/// Where the hands swap.
const DANCE_SWITCH: f64 = DANCE_START + 8.0 * BEAT;
pub(super) const DANCE_END: f64 = DANCE_START + DANCE_BEATS as f64 * BEAT + 0.6;
/// The bass line, one note a beat (pitch).
const DANCE_BASS: [f32; 8] = [1.0, 1.0, 1.19, 1.0, 0.89, 0.89, 1.06, 0.94];

// Sit: down, and as long as the player stays (moving stops it).
const SIT_DOWN: f64 = 0.6;
pub(super) const SIT_END: f64 = 600.0;
/// How far the model sinks to sit on the ground: the hips (12 pixels up) to a leg's half width.
pub(super) const SIT_DROP: f64 = 0.62;

// Crouch Spam: on and off every `CROUCH_TOGGLE` seconds.
const CROUCH_START: f64 = 0.3;
const CROUCH_TOGGLE: f64 = 0.16;
pub(super) const CROUCH_END: f64 = 4.0;

// Skill Issue: the shrug, held, and a sad trombone's three falling notes (time, pitch).
pub(super) const SKILL_END: f64 = 3.6;
const SKILL_NOTES: [(f64, f32); 3] = [(0.9, 0.84), (1.35, 0.79), (1.8, 0.63)];

// Too Easy: a look at the wrist, then the yawn.
const EASY_WATCH: f64 = 0.3;
const EASY_YAWN: f64 = 2.4;
pub(super) const EASY_END: f64 = 5.2;

// Point and Laugh: the point, then the laughing.
const LAUGH_START: f64 = 0.9;
pub(super) const LAUGH_END: f64 = 4.8;

// Cry About It: the crying, then the wave.
const CRY_WAVE: f64 = 3.7;
pub(super) const CRY_END: f64 = 5.6;
const TEAR: [f32; 3] = [0.4, 0.7, 1.0];
const GOLD_TEXT: [f32; 3] = [1.0, 0.85, 0.25];

// Celestial Wings: they start unfurling, are open, scatter, and it ends.
const WINGS_OPEN: f64 = 0.6;
const WINGS_OPENED: f64 = 2.6;
const WINGS_SCATTER: f64 = 8.8;
pub(super) const WINGS_END: f64 = 11.0;
/// One wing beat, in seconds.
const WING_BEAT: f64 = 1.6;
/// How high the wings lift the player, in blocks.
const WINGS_TOP: f64 = 0.55;
/// The wings' root: above the feet and behind the back, in blocks.
const WING_ROOT_UP: f64 = 1.3;
const WING_ROOT_BACK: f64 = 0.22;
const WING_EDGE: [f32; 3] = [1.0, 0.92, 0.65];
const WING_CYAN: [f32; 3] = [0.55, 0.88, 1.0];
const WING_VIOLET: [f32; 3] = [0.62, 0.42, 1.0];
const WING_PINK: [f32; 3] = [1.0, 0.58, 0.88];

/// What one of these emotes does to the player's model `t` seconds in.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Moves {
    /// How far the model is lifted off the ground, in blocks.
    pub lift: f64,
    /// The whole body's lean (pitch, roll in degrees; pitch leans back).
    pub tilt: [f32; 2],
    /// The arms (right, left: x, y, z in radians) and how far they hold.
    pub arms: ([[f32; 3]; 2], f32),
    /// The head's pitch (degrees, down positive) and turn from the body (degrees).
    pub head_pitch: f32,
    pub head_turn: f32,
    /// The body's turn from the way the player faced (degrees).
    pub turn: f32,
    /// How fast the legs step (the walk animation's speed), 0 standing.
    pub legs: f32,
    /// The legs (right, left: x, y, z in radians) and how far they hold, as the arms.
    pub leg_pose: ([[f32; 3]; 2], f32),
    /// Crouching (the model's sneaking pose).
    pub crouch: bool,
}

fn lerp3(a: [f64; 3], b: [f64; 3], k: f64) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * k)
}

fn arm(a: [f64; 3]) -> [f32; 3] {
    a.map(|v| v as f32)
}

/// The left arm doing what the right one does, mirrored.
fn mirrored(a: [f64; 3]) -> [f64; 3] {
    [a[0], -a[1], -a[2]]
}

/// `emote`'s pose `t` seconds in, if it is one of these.
pub(super) fn moves_at(emote: Emote, t: f64) -> Option<Moves> {
    match emote {
        Emote::DarkAscension => Some(dark_at(t)),
        Emote::Bruh => Some(bruh_at(t)),
        Emote::LDance => Some(dance_at(t)),
        Emote::CelestialWings => Some(wings_at(t)),
        Emote::Sit => Some(sit_at(t)),
        Emote::CrouchSpam => Some(crouch_at(t)),
        Emote::SkillIssue => Some(skill_at(t)),
        Emote::TooEasy => Some(easy_at(t)),
        Emote::PointAndLaugh => Some(laugh_at(t)),
        Emote::CryAboutIt => Some(cry_at(t)),
        _ => None,
    }
}

/// How far into the sitting pose, 0 to 1.
fn sat(t: f64) -> f64 {
    smoothstep(0.0, SIT_DOWN, t) * (1.0 - smoothstep(SIT_END - SIT_DOWN, SIT_END, t))
}

fn sit_at(t: f64) -> Moves {
    let down = sat(t);
    // Legs out in front, a little apart; hands resting on the knees; breathing slowly.
    let breathe = (t * 1.6).sin();
    let right = [-0.78 + 0.02 * breathe, -0.18, 0.08];
    Moves {
        lift: -SIT_DROP * down,
        tilt: [(3.0 * down) as f32, 0.0],
        arms: ([arm(right), arm(mirrored(right))], down as f32),
        head_pitch: ((6.0 + 2.0 * breathe) * down) as f32,
        leg_pose: ([[-1.45, 0.24, 0.0], [-1.45, -0.24, 0.0]], down as f32),
        ..Moves::default()
    }
}

/// Whether Crouch Spam has the player crouched at `t`.
fn crouched(t: f64) -> bool {
    (CROUCH_START..CROUCH_END - 0.3).contains(&t)
        && ((t - CROUCH_START) / CROUCH_TOGGLE).floor() as i64 % 2 == 0
}

fn crouch_at(t: f64) -> Moves {
    let on = smoothstep(0.0, 0.2, t) * (1.0 - smoothstep(CROUCH_END - 0.3, CROUCH_END, t));
    // Looking down at whoever is on the ground there.
    Moves {
        head_pitch: (32.0 * on) as f32,
        crouch: crouched(t),
        ..Moves::default()
    }
}

fn skill_at(t: f64) -> Moves {
    let up = smoothstep(0.3, 0.8, t) * (1.0 - smoothstep(SKILL_END - 0.7, SKILL_END - 0.1, t));
    // Forearms out to the sides, palms up, the shoulders bobbing with each note.
    let bob = SKILL_NOTES
        .iter()
        .map(|&(at, _)| (-((t - at) * 9.0).powi(2)).exp())
        .sum::<f64>()
        * up;
    let right = [-0.55 - 0.1 * bob, 0.0, 0.8 + 0.1 * bob];
    Moves {
        tilt: [0.0, (7.0 * up) as f32],
        arms: ([arm(right), arm(mirrored(right))], up as f32),
        head_pitch: (-4.0 * up) as f32,
        head_turn: (8.0 * up) as f32,
        ..Moves::default()
    }
}

fn easy_at(t: f64) -> Moves {
    let watch = smoothstep(EASY_WATCH, EASY_WATCH + 0.4, t)
        * (1.0 - smoothstep(EASY_YAWN - 0.4, EASY_YAWN, t));
    let yawn = smoothstep(EASY_YAWN, EASY_YAWN + 0.5, t)
        * (1.0 - smoothstep(EASY_END - 0.8, EASY_END - 0.2, t));
    // The left wrist held up in front to look at; then the right hand at the mouth while the
    // left arm stretches up and the head tips back.
    let rest = [0.0, 0.0, -0.06];
    let wrist = [-1.35, 0.75, 0.0];
    let stretch = [-2.9, 0.0, -0.3];
    let left = lerp3(lerp3(rest, wrist, watch), stretch, yawn);
    let right = lerp3([0.0, 0.0, 0.06], [-2.05, -0.75, 0.0], yawn);
    Moves {
        tilt: [(6.0 * yawn) as f32, 0.0],
        arms: ([arm(right), arm(left)], watch.max(yawn) as f32),
        head_pitch: (28.0 * watch - 26.0 * yawn) as f32,
        head_turn: (-14.0 * watch) as f32,
        ..Moves::default()
    }
}

/// How hard Point and Laugh is laughing, 0 to 1, and its shaking.
fn laughing(t: f64) -> (f64, f64) {
    let laugh = smoothstep(LAUGH_START, LAUGH_START + 0.3, t)
        * (1.0 - smoothstep(LAUGH_END - 0.9, LAUGH_END - 0.3, t));
    (laugh, laugh * (t * 18.0).sin())
}

fn laugh_at(t: f64) -> Moves {
    let point = smoothstep(0.15, 0.5, t) * (1.0 - smoothstep(LAUGH_END - 0.6, LAUGH_END - 0.1, t));
    let (laugh, shake) = laughing(t);
    // The right arm points straight at the viewer, the left hand holds the belly.
    let right = [-1.57 + 0.1 * shake, -0.1, 0.0];
    let left = [-0.65, 0.95, 0.0];
    Moves {
        lift: 0.03 * shake.abs(),
        tilt: [(-14.0 * laugh + 3.0 * shake) as f32, 0.0],
        arms: ([arm(right), arm(left)], point as f32),
        head_pitch: (-18.0 * laugh + 8.0 * shake) as f32,
        ..Moves::default()
    }
}

fn cry_at(t: f64) -> Moves {
    let rub = smoothstep(0.3, 0.7, t) * (1.0 - smoothstep(CRY_WAVE - 0.3, CRY_WAVE + 0.1, t));
    let wave = smoothstep(CRY_WAVE - 0.1, CRY_WAVE + 0.3, t);
    let away = smoothstep(CRY_END - 0.6, CRY_END - 0.1, t);
    let wobble = (t * 14.0).sin();
    // Both fists at the eyes, twisting; then the right hand up, waving bye-bye.
    let fist = [-2.25 + 0.08 * wobble, -0.55, 0.0];
    let waving = [-2.8, 0.0, 0.3 + 0.35 * (t * 9.0).sin()];
    let right = lerp3(fist, waving, wave);
    let left = lerp3(mirrored(fist), [0.0, 0.0, -0.06], wave);
    Moves {
        tilt: [(-6.0 * rub) as f32, 0.0],
        arms: (
            [arm(right), arm(left)],
            ((rub + wave) * (1.0 - away)).min(1.0) as f32,
        ),
        head_pitch: (20.0 * rub - 4.0 * wave * (1.0 - away)) as f32,
        head_turn: (7.0 * wobble * rub) as f32,
        ..Moves::default()
    }
}

/// How far the dark has burst, 0 to 1, and how hard it still shakes the player.
fn dark_burst(t: f64) -> (f64, f64) {
    let jolt = if t > DARK_BURST {
        (-(t - DARK_BURST) * 3.0).exp()
    } else {
        0.0
    };
    (smoothstep(DARK_BURST - 0.15, DARK_BURST + 0.1, t), jolt)
}

fn dark_lift(t: f64) -> f64 {
    let down = 1.0 - smoothstep(DARK_FALL, DARK_END - 0.4, t);
    let hover = smoothstep(DARK_RISEN - 1.0, DARK_RISEN, t) * down;
    DARK_TOP * smoothstep(DARK_RISE, DARK_RISEN, t) * down + 0.08 * (t * 1.3).sin() * hover
}

fn dark_at(t: f64) -> Moves {
    let rise = smoothstep(DARK_RISE, DARK_RISEN, t);
    let (burst, jolt) = dark_burst(t);
    let down = 1.0 - smoothstep(DARK_FALL, DARK_END - 0.4, t);
    let bow = smoothstep(0.0, DARK_RISE, t) * (1.0 - rise);
    // Trembling as the dark gathers into them.
    let gather = smoothstep(DARK_RISEN, DARK_BURST, t) * (1.0 - burst);
    let shake = gather * (t * 31.0).sin();
    // The arms held out and down, palms out, as they rise; flung wide as it bursts.
    let low = [0.12 + 0.05 * (t * 1.1).sin(), 0.0, 0.55 + 0.04 * shake];
    let wide = [-0.25, 0.0, 1.45];
    let right = lerp3(low, wide, burst);
    let weight = smoothstep(0.1, DARK_RISE + 0.6, t) * down;
    Moves {
        lift: dark_lift(t),
        tilt: [
            ((8.0 * rise + 14.0 * jolt) * down + 1.5 * shake) as f32,
            (3.0 * (t * 0.7).sin() * rise * down) as f32,
        ],
        arms: ([arm(right), arm(mirrored(right))], weight as f32),
        head_pitch: (22.0 * bow - 38.0 * rise * down) as f32,
        head_turn: 0.0,
        turn: 0.0,
        legs: 0.0,
        ..Moves::default()
    }
}

fn bruh_at(t: f64) -> Moves {
    let up = smoothstep(BRUH_SLAP - 0.5, BRUH_SLAP, t);
    let drag = smoothstep(BRUH_DRAG, BRUH_DRAG + 0.5, t);
    let away = smoothstep(BRUH_DRAG + 0.4, BRUH_END - 0.3, t);
    // The right palm over the eyes, then dragged down to the chin; the left arm hangs limp.
    let face = [-2.3, -0.83, 0.0];
    let chin = [-1.55, -0.7, 0.0];
    let right = lerp3(face, chin, drag);
    let left = [0.05, 0.0, -0.06];
    let hold = up * (1.0 - 0.6 * drag) * (1.0 - away);
    let shake = smoothstep(BRUH_SLAP + 0.2, BRUH_SLAP + 0.5, t) * (1.0 - smoothstep(2.3, 2.8, t));
    // A glance aside before it.
    let glance = smoothstep(0.1, 0.4, t) * (1.0 - up);
    Moves {
        lift: 0.0,
        tilt: [(-6.0 * hold) as f32, 0.0],
        arms: ([arm(right), arm(left)], (up * (1.0 - away)) as f32),
        head_pitch: (28.0 * hold - 6.0 * glance) as f32,
        head_turn: (11.0 * shake * (t * 6.5).sin() + 14.0 * glance) as f32,
        turn: 0.0,
        legs: 0.0,
        ..Moves::default()
    }
}

/// Beats into the L Dance at `t` (0 before it starts).
fn dance_beats(t: f64) -> f64 {
    ((t - DANCE_START) / BEAT).max(0.0)
}

fn dance_at(t: f64) -> Moves {
    let on = smoothstep(0.0, DANCE_START, t) * (1.0 - smoothstep(DANCE_END - 0.6, DANCE_END, t));
    let phase = dance_beats(t) * PI;
    let bounce = phase.sin().abs();
    // The L at the forehead (the right hand first), the other fist pumping on every beat.
    let l = [-2.55, -0.75, 0.0];
    let pump = [-0.6 - 0.75 * bounce, 0.0, 0.22];
    let swap = smoothstep(DANCE_SWITCH - 0.25, DANCE_SWITCH + 0.15, t);
    let right = lerp3(l, pump, swap);
    let left = lerp3(mirrored(pump), mirrored(l), swap);
    // Swaying hip to hip on each beat, turning a little; turning further after the swap.
    let turn = (8.0 + 14.0 * swap) * (phase / 2.0).sin();
    Moves {
        lift: 0.07 * bounce * on,
        tilt: [0.0, (7.0 * phase.sin() * on) as f32],
        arms: ([arm(right), arm(left)], on as f32),
        head_pitch: ((8.0 * bounce - 4.0) * on) as f32,
        head_turn: (-0.5 * turn * on) as f32,
        turn: (turn * on) as f32,
        legs: (0.55 * on) as f32,
        ..Moves::default()
    }
}

/// How far the wings are open, 0 to 1.
fn wings_spread(t: f64) -> f64 {
    smoothstep(WINGS_OPEN, WINGS_OPENED, t)
        * (1.0 - smoothstep(WINGS_SCATTER, WINGS_SCATTER + 0.5, t))
}

/// How much the wings beat, 0 to 1, and where in a beat they are (radians).
fn wing_beating(t: f64) -> (f64, f64) {
    let beating = smoothstep(WINGS_OPENED - 0.6, WINGS_OPENED + 0.4, t)
        * (1.0 - smoothstep(WINGS_SCATTER - 0.3, WINGS_SCATTER, t));
    (beating, (t - WINGS_OPENED) / WING_BEAT * TAU)
}

/// The wings' beat: how far their tips are raised (radians), a quick downstroke and a slower
/// upstroke round a pose held a little high.
fn wing_flap(t: f64) -> f64 {
    let (beating, cycle) = wing_beating(t);
    0.15 * wings_spread(t) + 0.5 * (cycle.sin() + 0.2 * (2.0 * cycle).sin()) * beating
}

fn wings_lift(t: f64) -> f64 {
    let (beating, cycle) = wing_beating(t);
    let hover = smoothstep(WINGS_OPEN + 0.5, WINGS_OPENED + 0.5, t)
        * (1.0 - smoothstep(WINGS_SCATTER, WINGS_END - 0.5, t));
    (WINGS_TOP * hover - 0.08 * cycle.cos() * beating).max(0.0)
}

fn wings_at(t: f64) -> Moves {
    let open =
        smoothstep(0.2, WINGS_OPENED, t) * (1.0 - smoothstep(WINGS_SCATTER, WINGS_END - 0.4, t));
    // Arms out and a little back, palms down; chest up, looking up.
    let right = [0.3, 0.0, 0.5 + 0.06 * wing_flap(t)];
    Moves {
        lift: wings_lift(t),
        tilt: [(4.0 * open) as f32, 0.0],
        arms: ([arm(right), arm(mirrored(right))], open as f32),
        head_pitch: (-18.0 * open) as f32,
        head_turn: 0.0,
        turn: 0.0,
        legs: 0.0,
        ..Moves::default()
    }
}

/// A point on a Celestial Wing `t` seconds in: `u` along the wing from the shoulder (0) to the
/// tip (1), `v` down a feather from the wing's edge (0) to its end (1), on the `side` (1 right,
/// -1 left), as (right, up, back) blocks from the wings' root.
fn wing_point(t: f64, side: f64, u: f64, v: f64) -> [f64; 3] {
    let spread = wings_spread(t);
    let flap = wing_flap(t);
    // The edge arches up from the shoulder and down to the tip; the feathers hang from it,
    // longest in the middle, fanning outward toward the tip.
    let length = 0.6 + 0.8 * (u * PI * 0.8).sin();
    let out = 0.12 + 2.0 * u + 0.55 * length * v * u;
    let up = 0.7 * (u * PI * 0.9).sin() - 0.15 * u - length * v * (1.0 - 0.3 * u);
    // Folded, they lie against the back.
    let (out, up) = (out * (0.2 + 0.8 * spread), up * (0.4 + 0.6 * spread));
    let back = 0.1 * u + 0.35 * u * (1.0 - spread);
    let (s, c) = flap.sin_cos();
    [side * (out * c - up * s), out * s + up * c, back]
}

/// Feet and the way the player faces, for placing things round them.
struct Frame {
    feet: [f64; 3],
    forward: [f64; 2],
    right: [f64; 2],
}

impl Frame {
    /// A player at `feet` facing `yaw` (radians, Minecraft's: 0 faces +z).
    fn new(feet: [f64; 3], yaw: f64) -> Self {
        Self {
            feet,
            forward: [-yaw.sin(), yaw.cos()],
            right: [-yaw.cos(), -yaw.sin()],
        }
    }

    /// The direction `side` to the right, `up` and `ahead`.
    fn dir(&self, side: f64, up: f64, ahead: f64) -> [f64; 3] {
        [
            self.right[0] * side + self.forward[0] * ahead,
            up,
            self.right[1] * side + self.forward[1] * ahead,
        ]
    }

    /// The point `side` to the right, `up` above the feet and `ahead` in front.
    fn at(&self, side: f64, up: f64, ahead: f64) -> [f64; 3] {
        let d = self.dir(side, up, ahead);
        [
            self.feet[0] + d[0],
            self.feet[1] + d[1],
            self.feet[2] + d[2],
        ]
    }

    /// A point on a Celestial Wing (see `wing_point`), lifted `lift`.
    fn wing(&self, t: f64, lift: f64, side: f64, u: f64, v: f64) -> [f64; 3] {
        let [x, y, back] = wing_point(t, side, u, v);
        self.at(x, lift + WING_ROOT_UP + y, -WING_ROOT_BACK - back)
    }
}

/// Where the eyes are (right, left) of a player at `f` lifted `lift`, head pitched `pitch`.
fn eyes(f: &Frame, lift: f64, pitch: f32) -> [[f64; 3]; 2] {
    let a = -f64::from(pitch).to_radians();
    // From the neck: forward to the face, up to the eyes.
    let (ahead, up) = (0.27, 0.24);
    let (ahead, up) = (
        ahead * a.cos() - up * a.sin(),
        ahead * a.sin() + up * a.cos(),
    );
    [0.12, -0.12].map(|side| f.at(side, lift + 1.5 + up, ahead))
}

/// How far the walk animation moves a tick while the legs step: a step a beat.
pub(super) const STRIDE: f32 = 0.47;

impl Game<'_> {
    /// Once a tick: the player's own model takes the pose of the emote playing (one of these),
    /// lifted where it started.
    pub(super) fn moves_pose(&mut self, t: f64) {
        let Some(p) = self.emotes.playing.as_mut() else {
            return;
        };
        let Some(mv) = moves_at(p.emote, t) else {
            return;
        };
        let real = self.player.as_ref().map(|pl| pl.position);
        let (Some(real), Some(m)) = (real, self.player_model.as_mut()) else {
            return;
        };
        let offset = [
            p.feet[0] - real.x,
            p.feet[1] + mv.lift - real.y,
            p.feet[2] - real.z,
        ];
        m.previous = Vec3::new(
            m.previous.x + p.offset[0],
            m.previous.y + p.offset[1],
            m.previous.z + p.offset[2],
        );
        m.position = Vec3::new(
            m.position.x + offset[0],
            m.position.y + offset[1],
            m.position.z + offset[2],
        );
        p.offset = offset;
        m.extra.previous_emote_tilt = m.extra.emote_tilt;
        m.extra.emote_tilt = mv.tilt;
        m.extra.previous_emote_arms = m.extra.emote_arms;
        m.extra.emote_arms = mv.arms;
        m.extra.previous_emote_legs = m.extra.emote_legs;
        m.extra.emote_legs = mv.leg_pose;
        if mv.crouch {
            m.extra.crouching = true;
        }
        m.previous_body_yaw = m.body_yaw;
        m.body_yaw = p.body_yaw + mv.turn;
        m.yaw = m.body_yaw + mv.head_turn;
        m.head_yaw = m.body_yaw + mv.head_turn;
        m.head_pitch = mv.head_pitch;
        m.previous_limb_speed = m.limb_speed;
        m.limb_speed = mv.legs;
        if mv.legs > 0.0 {
            m.limb_position += STRIDE;
        }
    }

    /// Once a tick: the particles and sounds of `emote` (if it is one of these), `t` seconds in
    /// and `previous` at the last tick, for a player whose feet were at `feet` facing `yaw`.
    pub(super) fn moves_effects(
        &mut self,
        emote: Emote,
        previous: f64,
        t: f64,
        ticks: u32,
        feet: [f64; 3],
        yaw: f64,
    ) {
        let f = Frame::new(feet, yaw);
        match emote {
            Emote::DarkAscension => self.dark_effects(previous, t, ticks, &f),
            Emote::Bruh => self.bruh_effects(previous, t, ticks, &f),
            Emote::LDance => self.dance_effects(previous, t, &f),
            Emote::CelestialWings => self.wings_effects(previous, t, ticks, &f),
            Emote::Sit => self.sit_effects(previous, t, &f),
            Emote::CrouchSpam => self.crouch_effects(previous, t, &f),
            Emote::SkillIssue => self.skill_effects(previous, t, &f),
            Emote::TooEasy => self.easy_effects(previous, t, ticks, &f),
            Emote::PointAndLaugh => self.laugh_effects(previous, t, ticks, &f),
            Emote::CryAboutIt => self.cry_effects(previous, t, ticks, &f),
            _ => {}
        }
    }

    /// `rows` (`#` lit) as glowing dots over the player, read from in front: `cell` blocks a
    /// dot, its middle `side` to the player's right and `up` above the feet.
    #[allow(clippy::too_many_arguments)]
    fn glyph(&mut self, f: &Frame, rows: &[&str], side: f64, up: f64, cell: f64, colour: [f32; 3]) {
        let width = rows.iter().map(|r| r.len()).max().unwrap_or(0) as f64;
        let height = rows.len() as f64;
        for (y, row) in rows.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                if c == '#' {
                    // Seen from in front, the player's right is the viewer's left.
                    let across = side - (x as f64 - (width - 1.0) / 2.0) * cell;
                    let at = f.at(across, up + ((height - 1.0) / 2.0 - y as f64) * cell, 0.0);
                    self.particles
                        .add_glowing(&ParticleOptions::dust(colour, 0.5), at, [0.0; 3]);
                }
            }
        }
    }

    fn sit_effects(&mut self, previous: f64, t: f64, f: &Frame) {
        let crossed = |at: f64| previous < at && t >= at;
        if crossed(SIT_DOWN * 0.8) {
            // Dust where they sit.
            for k in 0..10 {
                let a = f64::from(k) * TAU / 10.0;
                self.particles.add(
                    &ParticleOptions::dust([0.72, 0.68, 0.62], 0.9),
                    f.at(a.cos() * 0.3, 0.05, a.sin() * 0.3 + 0.2),
                    f.dir(a.cos() * 0.6, 0.1, a.sin() * 0.6),
                );
            }
            self.audio.play(
                "minecraft:block.wool.place",
                Some(f.at(0.0, 0.2, 0.0)),
                0.6,
                0.8,
            );
        }
    }

    fn crouch_effects(&mut self, previous: f64, t: f64, f: &Frame) {
        // A soft thump at every crouch, and a puff from the ground.
        if crouched(t) && !crouched(previous) {
            let at = f.at(0.0, 0.1, 0.0);
            self.audio
                .play("minecraft:block.wool.step", Some(at), 0.5, 1.4);
            let r = self.particles.random();
            let side = (r.next_double() - 0.5) * 0.4;
            self.particles.add(
                &ParticleOptions::simple(ParticleType::Poof),
                f.at(side, 0.1, 0.35),
                [0.0, 0.02, 0.0],
            );
        }
        if previous < CROUCH_END - 0.4 && t >= CROUCH_END - 0.4 {
            self.audio.play(
                "minecraft:entity.player.levelup",
                Some(f.at(0.0, 1.6, 0.0)),
                0.4,
                1.8,
            );
        }
    }

    fn skill_effects(&mut self, previous: f64, t: f64, f: &Frame) {
        let crossed = |at: f64| previous < at && t >= at;
        if (0.6..SKILL_END - 0.4).contains(&t) {
            self.glyph(
                f,
                &[
                    ".###.", "#...#", "....#", "...#.", "..#..", ".....", "..#..",
                ],
                0.0,
                2.55,
                0.07,
                GOLD_TEXT,
            );
        }
        // A sad trombone: three falling notes, the last the loudest.
        let at = Some(f.at(0.0, 1.6, 0.0));
        for (k, &(when, pitch)) in SKILL_NOTES.iter().enumerate() {
            if crossed(when) {
                let volume = if k == 2 { 0.9 } else { 0.7 };
                self.audio
                    .play("minecraft:block.note_block.didgeridoo", at, volume, pitch);
                self.audio
                    .play("minecraft:block.note_block.bass", at, 0.5, pitch);
            }
        }
    }

    fn easy_effects(&mut self, previous: f64, t: f64, ticks: u32, f: &Frame) {
        if (EASY_WATCH + 0.4..EASY_YAWN - 0.4).contains(&t) {
            // The watch glinting and ticking.
            if ticks.is_multiple_of(4) {
                self.particles.add_glowing(
                    &ParticleOptions::dust(GOLD_TEXT, 0.5),
                    f.at(-0.15, 1.25, 0.45),
                    [0.0; 3],
                );
            }
            if ticks.is_multiple_of(10) {
                self.audio.play(
                    "minecraft:block.note_block.hat",
                    Some(f.at(0.0, 1.3, 0.3)),
                    0.35,
                    1.9,
                );
            }
        }
        if (EASY_YAWN + 0.4..EASY_END - 0.5).contains(&t) {
            // "z z z" drifting up over the head, one after another.
            for k in 0..3 {
                let since = t - (EASY_YAWN + 0.4 + 0.5 * f64::from(k));
                if since > 0.0 {
                    let k = f64::from(k);
                    self.glyph(
                        f,
                        &["####", "..#.", ".#..", "####"],
                        -0.3 * k,
                        2.25 + 0.25 * k + (since * 0.25).min(0.4),
                        0.05,
                        WHITE,
                    );
                }
            }
        }
        if previous < EASY_YAWN + 0.2 && t >= EASY_YAWN + 0.2 {
            let at = Some(f.at(0.0, 1.6, 0.0));
            self.audio.play("minecraft:entity.fox.sleep", at, 0.9, 0.7);
            self.audio
                .play("minecraft:entity.villager.ambient", at, 0.5, 0.55);
        }
    }

    fn laugh_effects(&mut self, previous: f64, t: f64, ticks: u32, f: &Frame) {
        let at = Some(f.at(0.0, 1.6, 0.0));
        if previous < 0.45 && t >= 0.45 {
            // The point lands: a spark off the fingertip.
            let tip = f.at(-0.35, 1.4, 0.75);
            for _ in 0..5 {
                let r = self.particles.random();
                let d = direction(r);
                self.particles.add(
                    &ParticleOptions::simple(ParticleType::Crit),
                    tip,
                    d.map(|c| c * 0.2),
                );
            }
            self.audio
                .play("minecraft:entity.experience_orb.pickup", at, 0.5, 1.6);
        }
        if laughing(t).0 > 0.5 {
            // "HA" after "HA", and tears of joy.
            let (since, last) = (t - LAUGH_START, previous - LAUGH_START);
            if (since / 0.45).floor() > (last / 0.45).floor() {
                let r = self.particles.random();
                let pitch = 1.3 + 0.3 * r.next_float();
                self.audio
                    .play("minecraft:entity.witch.celebrate", at, 0.8, pitch);
            }
            if ticks.is_multiple_of(5) {
                for side in [0.12, -0.12] {
                    self.particles.add(
                        &ParticleOptions::dust(TEAR, 0.5),
                        f.at(side, 1.65, 0.28),
                        f.dir(side * 2.0, 0.15, 0.05),
                    );
                }
            }
            if ticks.is_multiple_of(9) {
                let side = if ticks.is_multiple_of(18) {
                    0.55
                } else {
                    -0.55
                };
                self.glyph(
                    f,
                    &["#.#.###", "#.#.#.#", "###.###", "#.#.#.#", "#.#.#.#"],
                    side,
                    2.45,
                    0.045,
                    GOLD_TEXT,
                );
            }
        }
    }

    fn cry_effects(&mut self, previous: f64, t: f64, ticks: u32, f: &Frame) {
        let crossed = |at: f64| previous < at && t >= at;
        let at = Some(f.at(0.0, 1.6, 0.0));
        if (0.6..CRY_WAVE - 0.2).contains(&t) {
            // Fake tears streaming from both eyes.
            if ticks.is_multiple_of(2) {
                for side in [0.13, -0.13] {
                    self.particles.add(
                        &ParticleOptions::dust(TEAR, 0.6),
                        f.at(side, 1.6, 0.3),
                        f.dir(side * 1.5, -0.4, 0.1),
                    );
                }
            }
            if ticks.is_multiple_of(16) {
                self.audio.play("minecraft:entity.wolf.whine", at, 0.6, 1.3);
            }
        }
        for sob in [0.8, 1.6, 2.4] {
            if crossed(sob) {
                self.audio
                    .play("minecraft:entity.villager.hurt", at, 0.5, 1.7);
            }
        }
        if crossed(CRY_WAVE + 0.2) {
            self.audio
                .play("minecraft:block.note_block.bell", at, 0.6, 1.5);
            self.audio
                .play("minecraft:block.note_block.bell", at, 0.4, 1.19);
        }
    }

    fn dark_effects(&mut self, previous: f64, t: f64, ticks: u32, f: &Frame) {
        let crossed = |at: f64| previous < at && t >= at;
        let m = dark_at(t);
        let lift = m.lift;
        let body = f.at(0.0, lift + 1.0, 0.0);
        let particles = &mut self.particles;
        if t > 0.1 && t < DARK_RISEN + 0.5 {
            // Black mist creeping in along the ground.
            for _ in 0..3 {
                let r = particles.random();
                let a = r.next_double() * TAU;
                let far = 1.2 + r.next_double() * 1.8;
                let at = [
                    f.feet[0] + a.cos() * far,
                    f.feet[1] + 0.1,
                    f.feet[2] + a.sin() * far,
                ];
                let v = [-a.cos() * 0.45, 0.02, -a.sin() * 0.45];
                particles.add(
                    &ParticleOptions::dust_transition(DARK_DEEP, DARK_BLACK, 1.8),
                    at,
                    v,
                );
            }
        }
        if t > DARK_RISE && t < DARK_FALL + 0.5 {
            // Dark strands spiralling up round the body, with violet sparks among them.
            let strength = smoothstep(DARK_RISE, DARK_RISE + 1.0, t);
            for k in 0..3 {
                for j in 0..2 {
                    let climb = (t * 0.9 + f64::from(j) * 0.5 + f64::from(k) * 0.17).fract();
                    let a = t * 2.4 + f64::from(k) * TAU / 3.0 + climb * 2.0;
                    let radius = (0.95 - 0.35 * climb) * (0.6 + 0.4 * strength);
                    let at = [
                        f.feet[0] + a.cos() * radius,
                        f.feet[1] + lift - 0.3 + climb * 2.7,
                        f.feet[2] + a.sin() * radius,
                    ];
                    particles.add(
                        &ParticleOptions::dust_transition(DARK_DEEP, DARK_BLACK, 1.3),
                        at,
                        [0.0; 3],
                    );
                }
            }
            let r = particles.random();
            if r.next_int_bound(3).unwrap_or(0) == 0 {
                let d = direction(r);
                let at = [
                    body[0] + d[0] * 0.9,
                    body[1] + d[1] * 1.1,
                    body[2] + d[2] * 0.9,
                ];
                particles.add_glowing(
                    &ParticleOptions::dust_transition(VIOLET, DARK_DEEP, 0.7),
                    at,
                    [0.0, 0.3, 0.0],
                );
            }
        }
        if t > DARK_RISEN && t < DARK_BURST {
            // The dark drawn into them, faster as it gathers.
            let gather = smoothstep(DARK_RISEN, DARK_BURST, t);
            for _ in 0..(2.0 + 6.0 * gather) as usize {
                let r = particles.random();
                let d = direction(r);
                let far = 1.6 + r.next_double() * 1.2;
                particles.add(
                    &ParticleOptions::simple(ParticleType::Portal),
                    body,
                    d.map(|c| c * far),
                );
            }
        }
        if t > DARK_RISE + 1.0 && t < DARK_FALL + 0.3 {
            // Burning violet eyes.
            for at in eyes(f, lift, m.head_pitch) {
                particles.add_glowing(&ParticleOptions::dust(DARK_EYES, 0.35), at, [0.0; 3]);
            }
        }
        if crossed(DARK_BURST) {
            // The burst: a ring of dark thrown outward, a shock wave, sparks.
            particles.add(
                &ParticleOptions::simple(ParticleType::SonicBoom),
                body,
                [0.0; 3],
            );
            for k in 0..72 {
                let r = particles.random();
                let a = f64::from(k) * TAU / 72.0 + r.next_double() * 0.05;
                let speed = 2.5 + r.next_double() * 1.0;
                let v = [
                    a.cos() * speed,
                    (r.next_double() - 0.5) * 0.4,
                    a.sin() * speed,
                ];
                particles.add(
                    &ParticleOptions::dust_transition(DARK_DEEP, DARK_BLACK, 2.2),
                    body,
                    v,
                );
            }
            for _ in 0..40 {
                let r = particles.random();
                let d = direction(r);
                let speed = 0.4 + r.next_double() * 0.5;
                particles.add(
                    &ParticleOptions::simple(ParticleType::ReversePortal),
                    body,
                    d.map(|c| c * speed * 0.3),
                );
                particles.add(
                    &ParticleOptions::simple(ParticleType::Witch),
                    body,
                    d.map(|c| c * speed),
                );
            }
        }
        if t > DARK_BURST && t < DARK_FALL + 0.6 {
            // The dark still smouldering round them.
            for _ in 0..2 {
                let r = particles.random();
                let d = direction(r);
                let at = [
                    body[0] + d[0] * 0.6,
                    body[1] + d[1] * 0.9,
                    body[2] + d[2] * 0.6,
                ];
                particles.add(
                    &ParticleOptions::simple(ParticleType::Smoke),
                    at,
                    [0.0, 0.03, 0.0],
                );
            }
        }
        if crossed(DARK_END - 0.6) {
            // Set down: dust thrown out round the feet.
            for k in 0..24 {
                let a = f64::from(k) * TAU / 24.0;
                let at = [
                    f.feet[0] + a.cos() * 0.4,
                    f.feet[1] + 0.1,
                    f.feet[2] + a.sin() * 0.4,
                ];
                let v = [a.cos() * 1.5, 0.05, a.sin() * 1.5];
                particles.add(
                    &ParticleOptions::dust_transition(DARK_DEEP, DARK_BLACK, 1.4),
                    at,
                    v,
                );
            }
        }
        let audio = &mut self.audio;
        let at = Some(body);
        if crossed(0.05) {
            audio.play("minecraft:entity.wither.ambient", at, 0.5, 0.5);
        }
        if crossed(DARK_RISE) {
            audio.play("minecraft:block.respawn_anchor.charge", at, 0.8, 0.5);
        }
        if t > DARK_RISEN && t < DARK_BURST && ticks.is_multiple_of(14) {
            let gather = smoothstep(DARK_RISEN, DARK_BURST, t) as f32;
            audio.play(
                "minecraft:entity.warden.heartbeat",
                at,
                0.5 + 0.5 * gather,
                0.8 + 0.3 * gather,
            );
        }
        if crossed(DARK_BURST) {
            audio.play("minecraft:entity.warden.sonic_boom", at, 0.7, 0.8);
            audio.play("minecraft:entity.generic.explode", at, 0.5, 0.5);
        }
        if crossed(DARK_FALL) {
            audio.play("minecraft:block.beacon.deactivate", at, 0.7, 0.5);
        }
    }

    fn bruh_effects(&mut self, previous: f64, t: f64, ticks: u32, f: &Frame) {
        let crossed = |at: f64| previous < at && t >= at;
        let face = f.at(0.0, 1.75, 0.35);
        let particles = &mut self.particles;
        if crossed(BRUH_SLAP) {
            // The slap.
            for _ in 0..6 {
                let r = particles.random();
                let d = direction(r);
                let at = [
                    face[0] + d[0] * 0.15,
                    face[1] + d[1] * 0.15,
                    face[2] + d[2] * 0.15,
                ];
                particles.add(
                    &ParticleOptions::simple(ParticleType::Crit),
                    at,
                    d.map(|c| c * 0.15),
                );
            }
        }
        if (BRUH_SLAP + 0.4..BRUH_DRAG).contains(&t) {
            // "..." over the head, one dot at a time.
            for k in 0..3 {
                if t >= BRUH_SLAP + 0.4 + 0.35 * f64::from(k) {
                    let at = f.at(-0.25 + 0.25 * f64::from(k), 2.35, 0.0);
                    particles.add_glowing(&ParticleOptions::dust(WHITE, 0.55), at, [0.0; 3]);
                }
            }
        }
        if (BRUH_SLAP + 0.2..BRUH_DRAG).contains(&t) && ticks.is_multiple_of(12) {
            // A drop of sweat running off the temple.
            let at = f.at(0.3, 1.85, 0.05);
            particles.add(
                &ParticleOptions::dust([0.45, 0.72, 1.0], 0.7),
                at,
                [0.0, -0.6, 0.0],
            );
        }
        if crossed(2.2) {
            // The sigh: a puff of breath, down and out.
            let mouth = f.at(0.0, 1.55, 0.4);
            for _ in 0..8 {
                let r = particles.random();
                let d = direction(r);
                let v = f.dir(d[0] * 0.03, -0.02, 0.06 + r.next_double() * 0.04);
                particles.add(&ParticleOptions::simple(ParticleType::WhiteSmoke), mouth, v);
            }
        }
        let audio = &mut self.audio;
        let at = Some(face);
        if crossed(BRUH_SLAP) {
            audio.play("minecraft:entity.slime.squish_small", at, 0.7, 1.6);
            audio.play("minecraft:entity.player.attack.nodamage", at, 0.5, 1.4);
        }
        if crossed(BRUH_SLAP + 0.45) {
            audio.play("minecraft:entity.villager.no", at, 0.8, 0.85);
        }
        if crossed(2.2) {
            audio.play("minecraft:entity.villager.ambient", at, 0.35, 0.6);
        }
    }

    fn dance_effects(&mut self, previous: f64, t: f64, f: &Frame) {
        let crossed = |at: f64| previous < at && t >= at;
        let lift = dance_at(t).lift;
        for k in 0..DANCE_BEATS {
            let beat = DANCE_START + f64::from(k) * BEAT;
            let at = Some(f.at(0.0, 1.0, 0.0));
            if crossed(beat) {
                let drum = if k % 2 == 0 {
                    "minecraft:block.note_block.basedrum"
                } else {
                    "minecraft:block.note_block.snare"
                };
                self.audio.play(drum, at, 0.7, 1.0);
                let bass = DANCE_BASS[k as usize % DANCE_BASS.len()];
                self.audio
                    .play("minecraft:block.note_block.bass", at, 0.6, bass);
                // A note over the head, its colour walking round the wheel.
                let particles = &mut self.particles;
                let r = particles.random();
                let note = f.at(
                    (r.next_double() - 0.5) * 0.6,
                    lift + 2.2,
                    (r.next_double() - 0.5) * 0.3,
                );
                particles.add(
                    &ParticleOptions::simple(ParticleType::Note),
                    note,
                    [f64::from(k) / 16.0, 0.0, 0.0],
                );
                // A puff kicked up by the step.
                let side = if k % 2 == 0 { 0.15 } else { -0.15 };
                let step = f.at(side, 0.05, 0.0);
                particles.add(
                    &ParticleOptions::dust([0.75, 0.72, 0.8], 0.8),
                    step,
                    f.dir(side * 3.0, 0.3, 0.0),
                );
            }
            if crossed(beat + BEAT / 2.0) {
                self.audio
                    .play("minecraft:block.note_block.hat", at, 0.45, 1.2);
            }
        }
        if crossed(DANCE_SWITCH) {
            self.audio.play(
                "minecraft:block.note_block.bell",
                Some(f.at(0.0, 1.6, 0.0)),
                0.6,
                1.19,
            );
        }
    }

    fn wings_effects(&mut self, previous: f64, t: f64, ticks: u32, f: &Frame) {
        let crossed = |at: f64| previous < at && t >= at;
        let lift = wings_lift(t);
        let root = f.wing(t, lift, 1.0, 0.0, 0.0);
        let spread = wings_spread(t);
        let particles = &mut self.particles;
        if t < WINGS_OPEN + 0.3 {
            // Starlight drawn to the back.
            for k in 0..3 {
                let r = particles.random();
                let d = direction(r);
                let from = [
                    root[0] + d[0] * 2.2,
                    root[1] + d[1] * 1.6,
                    root[2] + d[2] * 2.2,
                ];
                let colour = [WHITE, WING_CYAN, WING_VIOLET][k];
                let trail = ParticleOptions {
                    color: [colour[0], colour[1], colour[2], 1.0],
                    trail: Some((root, 10)),
                    ..ParticleOptions::simple(ParticleType::Trail)
                };
                particles.add_glowing(&trail, from, [0.0; 3]);
            }
        }
        if spread > 0.02 {
            // The wings themselves: a bright edge and feathers of light, cyan to violet to pink.
            for side in [1.0, -1.0] {
                for k in 0..(16.0 * spread.max(0.4)) as u32 {
                    let r = particles.random();
                    let u = (f64::from(k) + r.next_double()) / 16.0;
                    let at = f.wing(t, lift, side, u, 0.0);
                    particles.add_glowing(
                        &ParticleOptions::dust_transition(WHITE, WING_EDGE, 0.9),
                        at,
                        [0.0; 3],
                    );
                }
                for _ in 0..(30.0 * spread) as u32 {
                    let r = particles.random();
                    let (u, v) = (r.next_double(), r.next_double());
                    let at = f.wing(t, lift, side, u, v);
                    let (from, to) = if v < 0.5 {
                        (WING_CYAN, WING_VIOLET)
                    } else {
                        (WING_VIOLET, WING_PINK)
                    };
                    let scale = 0.55 + 0.35 * r.next_float();
                    particles.add_glowing(
                        &ParticleOptions::dust_transition(from, to, scale),
                        at,
                        [0.0; 3],
                    );
                }
                // Stardust falling from the feather tips.
                let r = particles.random();
                if r.next_int_bound(3).unwrap_or(0) == 0 {
                    let at = f.wing(t, lift, side, r.next_double(), 1.0);
                    particles.add(
                        &ParticleOptions::simple(ParticleType::EndRod),
                        at,
                        [0.0, -0.03, 0.0],
                    );
                }
            }
        }
        if crossed(WINGS_SCATTER) {
            // They scatter into stardust.
            for side in [1.0, -1.0] {
                for _ in 0..60 {
                    let r = particles.random();
                    let at = f.wing(t, lift, side, r.next_double(), r.next_double());
                    let d = direction(r);
                    let colour = [WHITE, WING_CYAN, WING_VIOLET, WING_PINK]
                        [r.next_int_bound(4).unwrap_or(0) as usize];
                    particles.add_glowing(
                        &ParticleOptions::dust_transition(colour, WHITE, 1.0),
                        at,
                        d.map(|c| c * 1.2),
                    );
                }
            }
        }
        if t > WINGS_SCATTER && t < WINGS_END - 0.5 {
            // Feathers of light drifting down.
            for _ in 0..3 {
                let r = particles.random();
                let at = f.at(
                    (r.next_double() - 0.5) * 4.0,
                    1.0 + r.next_double() * 2.0,
                    -0.6 + (r.next_double() - 0.5) * 1.5,
                );
                particles.add_glowing(
                    &ParticleOptions::dust_transition(WHITE, WING_VIOLET, 0.8),
                    at,
                    [0.0, -0.4, 0.0],
                );
            }
        }
        let audio = &mut self.audio;
        let at = Some(root);
        if crossed(WINGS_OPEN) {
            audio.play("minecraft:block.beacon.activate", at, 0.7, 1.4);
            audio.play("minecraft:block.amethyst_block.resonate", at, 0.8, 1.2);
        }
        // Each downstroke: from the top of the beat (a quarter of the way through it).
        let (beating, cycle) = wing_beating(t);
        let last = (previous - WINGS_OPENED) / WING_BEAT * TAU;
        let top = |c: f64| ((c - PI / 2.0) / TAU).floor();
        if beating > 0.3 && top(cycle) > top(last) {
            audio.play("minecraft:entity.ender_dragon.flap", at, 0.4, 1.7);
            audio.play(
                "minecraft:block.amethyst_block.chime",
                at,
                0.5,
                1.2 + 0.2 * (ticks % 3) as f32,
            );
        }
        if crossed(WINGS_SCATTER) {
            audio.play("minecraft:block.amethyst_cluster.break", at, 1.0, 1.0);
            audio.play("minecraft:entity.firework_rocket.twinkle", at, 0.8, 1.2);
        }
    }

    /// The camera for `emote` (one of these) in place of `view`: framing a player whose feet
    /// were at `feet` facing `yaw`, from in front (Celestial Wings from behind at first; Sit
    /// turned `look` degrees round by the mouse), closer where something is in the way,
    /// blended in and out of the player's own.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn moves_camera(
        &self,
        emote: Emote,
        t: f64,
        feet: [f64; 3],
        yaw: f64,
        look: f64,
        view: (glam::DVec3, f32, f32),
    ) -> (glam::DVec3, f32, f32) {
        let Some(m) = moves_at(emote, t) else {
            return view;
        };
        // Round the player (0 is straight ahead of them), how far, how high above the focus,
        // and the focus's height above the feet.
        let (round, far, above, focus_up, end) = match emote {
            Emote::Bruh => (
                0.4 + 0.05 * (t * 0.5).sin(),
                3.4 - 0.6 * smoothstep(0.5, 2.5, t),
                0.05,
                1.5,
                BRUH_END,
            ),
            Emote::LDance => (0.3 + 0.25 * (t * 0.35).sin(), 4.4, 0.1, 1.05, DANCE_END),
            Emote::DarkAscension => (
                0.35 + 0.1 * (t * 0.3).sin(),
                5.2 + 0.8 * smoothstep(DARK_RISEN, DARK_BURST + 0.5, t),
                -0.7,
                1.3,
                DARK_END,
            ),
            Emote::CelestialWings => (
                2.6 - 1.3 * smoothstep(WINGS_OPEN, WINGS_SCATTER, t),
                6.4,
                0.2,
                1.5,
                WINGS_END,
            ),
            // The mouse turns the camera round the sitting player.
            Emote::Sit => (0.5 + look.to_radians(), 3.6, 0.6, 1.0, SIT_END),
            Emote::CrouchSpam => (0.5, 3.8, 0.9, 0.9, CROUCH_END),
            Emote::SkillIssue => (0.3, 3.6, 0.2, 1.4, SKILL_END),
            Emote::TooEasy => (0.45, 3.4, 0.1, 1.4, EASY_END),
            // Straight in front: it points at whoever watches.
            Emote::PointAndLaugh => (0.05, 3.6, 0.1, 1.35, LAUGH_END),
            Emote::CryAboutIt => (0.2, 3.2, 0.05, 1.45, CRY_END),
            _ => return view,
        };
        let a = yaw + round;
        let out = [-a.sin(), a.cos()];
        let focus = [feet[0], feet[1] + m.lift + focus_up, feet[2]];
        let distance = [far, far * 0.75, far * 0.5, far * 0.3]
            .into_iter()
            .find(|&d| {
                let at = [
                    focus[0] + out[0] * d,
                    focus[1] + above,
                    focus[2] + out[1] * d,
                ];
                self.sees(at, focus)
            })
            .unwrap_or(far * 0.3);
        let position = glam::DVec3::new(
            focus[0] + out[0] * distance,
            focus[1] + above,
            focus[2] + out[1] * distance,
        );
        let look = glam::DVec3::from_array(focus) - position;
        let mut yaw = yaw_of(look.x, look.z);
        let mut pitch = (-look.y.atan2(look.x.hypot(look.z)).to_degrees()) as f32;
        if emote == Emote::DarkAscension {
            // It shakes as the dark gathers and as it bursts.
            let (burst, jolt) = dark_burst(t);
            let rumble =
                (0.25 * smoothstep(DARK_RISEN, DARK_BURST, t) * (1.0 - burst) + 2.4 * jolt) as f32;
            yaw += ((t * 37.0).sin() * 0.6 + (t * 23.0).sin() * 0.4) as f32 * rumble;
            pitch += ((t * 41.0).cos() * 0.6 + (t * 29.0).sin() * 0.4) as f32 * rumble;
        }
        let blend = smoothstep(0.0, 1.0, t) * (1.0 - smoothstep(end - 1.0, end, t));
        let (from, from_yaw, from_pitch) = view;
        (
            from.lerp(position, blend),
            from_yaw + wrap(yaw - from_yaw) * blend as f32,
            from_pitch + (pitch - from_pitch) * blend as f32,
        )
    }
}

/// `rgb` (0xRRGGBB) at `alpha` in the HUD's colour order (red and blue swapped).
fn hud(rgb: u32, alpha: u32) -> u32 {
    (alpha << 24) | ((rgb & 0xFF) << 16) | (rgb & 0xFF00) | ((rgb >> 16) & 0xFF)
}

fn disc(batch: &mut HudBatch, atlas: &HudAtlas, x: f32, y: f32, r: f32, colour: u32) {
    for k in 0..20 {
        let a0 = k as f32 / 20.0 * TAU as f32;
        let a1 = (k + 1) as f32 / 20.0 * TAU as f32;
        let at = |a: f32| [x + a.cos() * r, y + a.sin() * r];
        batch.fill_quad(atlas, [[x, y], at(a0), at(a1), at(a1)], colour);
    }
}

/// The wheel's picture of `emote` (one of these) at `(ix, iy)`, dimmed when not owned.
pub(super) fn draw_moves_icon(
    batch: &mut HudBatch,
    atlas: &HudAtlas,
    emote: Emote,
    ix: f32,
    iy: f32,
    lit: bool,
) {
    let c = |rgb: u32| hud(rgb, if lit { 0xFF } else { 0x50 });
    let glow = |rgb: u32| hud(rgb, if lit { 0x48 } else { 0x20 });
    match emote {
        Emote::DarkAscension => {
            // A dark core with violet eyes, the dark rising off it.
            disc(batch, atlas, ix, iy + 2.0, 11.0, glow(0x8a3cf0));
            disc(batch, atlas, ix, iy + 2.0, 6.5, c(0x3a1060));
            disc(batch, atlas, ix, iy + 2.0, 4.6, c(0x050008));
            batch.fill(atlas, ix - 3.0, iy + 1.0, 1.8, 1.2, c(0xcc66ff));
            batch.fill(atlas, ix + 1.2, iy + 1.0, 1.8, 1.2, c(0xcc66ff));
            for k in 0..2 {
                let y = iy - 6.0 - k as f32 * 3.5;
                batch.fill_quad(
                    atlas,
                    [
                        [ix - 5.0, y + 2.5],
                        [ix, y - 1.0],
                        [ix, y + 0.8],
                        [ix - 5.0, y + 4.0],
                    ],
                    c(0x8a3cf0),
                );
                batch.fill_quad(
                    atlas,
                    [
                        [ix, y - 1.0],
                        [ix + 5.0, y + 2.5],
                        [ix + 5.0, y + 4.0],
                        [ix, y + 0.8],
                    ],
                    c(0x8a3cf0),
                );
            }
        }
        Emote::Bruh => {
            // A face with a palm over its eyes.
            disc(batch, atlas, ix, iy, 9.0, c(0xe0b090));
            batch.fill(atlas, ix - 3.0, iy + 4.0, 6.0, 1.2, c(0x5a3a2a));
            batch.fill_quad(
                atlas,
                [
                    [ix - 9.0, iy - 5.0],
                    [ix + 6.0, iy - 3.5],
                    [ix + 6.0, iy + 0.5],
                    [ix - 9.0, iy - 1.0],
                ],
                c(0xc08060),
            );
            for k in 0..3 {
                let x = ix + 6.0 + k as f32 * 1.2;
                batch.fill(atlas, x, iy - 4.5 + k as f32 * 1.4, 3.0, 1.0, c(0xc08060));
            }
        }
        Emote::LDance => {
            // An L and a note.
            batch.fill(atlas, ix - 8.0, iy - 9.0, 4.5, 15.0, c(0x6ee87c));
            batch.fill(atlas, ix - 8.0, iy + 2.0, 11.0, 4.0, c(0x6ee87c));
            batch.fill(atlas, ix + 7.5, iy - 10.0, 1.5, 9.0, c(0xffffff));
            batch.fill(atlas, ix + 7.5, iy - 10.0, 3.5, 1.5, c(0xffffff));
            disc(batch, atlas, ix + 6.0, iy - 1.0, 2.2, c(0xffffff));
        }
        Emote::CelestialWings => {
            // Two wings of light round a star.
            disc(batch, atlas, ix, iy, 12.0, glow(0x9fe8ff));
            for side in [-1.0f32, 1.0] {
                let p = |x: f32, y: f32| [ix + side * x, iy + y];
                batch.fill_quad(
                    atlas,
                    [p(1.5, -1.0), p(12.0, -9.0), p(11.0, 1.0), p(2.0, 3.0)],
                    c(0x9fe8ff),
                );
                batch.fill_quad(
                    atlas,
                    [p(2.5, 0.0), p(10.0, -6.0), p(9.0, 1.0), p(2.5, 2.5)],
                    c(0xb48cff),
                );
                batch.fill_quad(
                    atlas,
                    [p(1.5, -1.0), p(12.0, -9.0), p(12.0, -8.0), p(1.5, 0.0)],
                    c(0xfff2c0),
                );
            }
            disc(batch, atlas, ix, iy, 2.6, c(0xffffff));
        }
        Emote::Sit => {
            // A figure sitting on the ground: head, back, thighs out in front, shins down.
            disc(batch, atlas, ix - 3.0, iy - 8.0, 3.2, c(0xe0b090));
            batch.fill(atlas, ix - 5.0, iy - 4.5, 4.0, 8.0, c(0x4aa3df));
            batch.fill(atlas, ix - 5.0, iy + 3.0, 11.0, 3.5, c(0x3b4a8c));
            batch.fill(atlas, ix + 3.0, iy + 3.0, 3.5, 7.0, c(0x3b4a8c));
            batch.fill(atlas, ix - 10.0, iy + 10.0, 20.0, 1.5, c(0x9a8f80));
        }
        Emote::CrouchSpam => {
            // A squat figure between an up and a down arrow.
            disc(batch, atlas, ix, iy - 1.0, 3.0, c(0xe0b090));
            batch.fill(atlas, ix - 3.0, iy + 2.0, 6.0, 5.0, c(0x4aa3df));
            batch.fill(atlas, ix - 3.0, iy + 7.0, 6.0, 3.0, c(0x3b4a8c));
            let gold = c(0xffd84a);
            let (lx, rx) = (ix - 9.0, ix + 9.0);
            batch.fill_quad(
                atlas,
                [
                    [lx, iy - 9.0],
                    [lx + 3.5, iy - 4.0],
                    [lx - 3.5, iy - 4.0],
                    [lx, iy - 9.0],
                ],
                gold,
            );
            batch.fill(atlas, lx - 1.0, iy - 4.0, 2.0, 5.0, gold);
            batch.fill_quad(
                atlas,
                [
                    [rx, iy + 9.0],
                    [rx - 3.5, iy + 4.0],
                    [rx + 3.5, iy + 4.0],
                    [rx, iy + 9.0],
                ],
                gold,
            );
            batch.fill(atlas, rx - 1.0, iy - 1.0, 2.0, 5.0, gold);
        }
        Emote::SkillIssue => {
            // A big gold "?".
            let gold = c(0xffd84a);
            batch.fill(atlas, ix - 4.0, iy - 10.0, 8.0, 2.5, gold);
            batch.fill(atlas, ix - 5.5, iy - 9.0, 2.5, 3.0, gold);
            batch.fill(atlas, ix + 3.0, iy - 9.0, 2.5, 6.0, gold);
            batch.fill(atlas, ix - 1.0, iy - 4.0, 4.5, 2.5, gold);
            batch.fill(atlas, ix - 1.0, iy - 2.0, 2.5, 4.0, gold);
            batch.fill(atlas, ix - 1.0, iy + 4.0, 2.5, 2.5, gold);
        }
        Emote::TooEasy => {
            // A watch face and a "z".
            disc(batch, atlas, ix - 3.0, iy + 2.0, 7.0, c(0xffd84a));
            disc(batch, atlas, ix - 3.0, iy + 2.0, 5.5, c(0xfff6dc));
            batch.fill(atlas, ix - 3.5, iy - 2.0, 1.2, 4.5, c(0x302030));
            batch.fill(atlas, ix - 3.5, iy + 1.8, 3.5, 1.2, c(0x302030));
            let white = c(0xffffff);
            batch.fill(atlas, ix + 4.0, iy - 10.0, 6.0, 1.5, white);
            batch.fill_quad(
                atlas,
                [
                    [ix + 8.5, iy - 8.5],
                    [ix + 10.0, iy - 8.5],
                    [ix + 5.5, iy - 5.0],
                    [ix + 4.0, iy - 5.0],
                ],
                white,
            );
            batch.fill(atlas, ix + 4.0, iy - 5.0, 6.0, 1.5, white);
        }
        Emote::PointAndLaugh => {
            // A laughing face with tears of joy.
            disc(batch, atlas, ix, iy, 9.0, c(0xffcc4d));
            batch.fill(atlas, ix - 5.0, iy - 3.0, 3.0, 1.2, c(0x5a3a2a));
            batch.fill(atlas, ix + 2.0, iy - 3.0, 3.0, 1.2, c(0x5a3a2a));
            batch.fill_quad(
                atlas,
                [
                    [ix - 5.5, iy + 1.5],
                    [ix + 5.5, iy + 1.5],
                    [ix + 3.0, iy + 6.0],
                    [ix - 3.0, iy + 6.0],
                ],
                c(0x5a2a2a),
            );
            disc(batch, atlas, ix - 7.5, iy - 0.5, 1.8, c(0x66b3ff));
            disc(batch, atlas, ix + 7.5, iy - 0.5, 1.8, c(0x66b3ff));
        }
        Emote::CryAboutIt => {
            // A face with fists at its eyes, tears running down.
            disc(batch, atlas, ix, iy, 9.0, c(0xe0b090));
            disc(batch, atlas, ix - 4.0, iy - 2.0, 3.0, c(0xc08060));
            disc(batch, atlas, ix + 4.0, iy - 2.0, 3.0, c(0xc08060));
            batch.fill(atlas, ix - 3.0, iy + 4.5, 6.0, 1.2, c(0x5a3a2a));
            batch.fill(atlas, ix - 7.5, iy + 0.5, 1.6, 7.0, c(0x66b3ff));
            batch.fill(atlas, ix + 6.0, iy + 0.5, 1.6, 7.0, c(0x66b3ff));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_move_starts_and_ends_standing_on_the_ground() {
        for (emote, end) in [
            (Emote::DarkAscension, DARK_END),
            (Emote::Bruh, BRUH_END),
            (Emote::LDance, DANCE_END),
            (Emote::CelestialWings, WINGS_END),
            (Emote::Sit, SIT_END),
            (Emote::CrouchSpam, CROUCH_END),
            (Emote::SkillIssue, SKILL_END),
            (Emote::TooEasy, EASY_END),
            (Emote::PointAndLaugh, LAUGH_END),
            (Emote::CryAboutIt, CRY_END),
        ] {
            assert_eq!(emote.length(), end);
            for t in [0.0, end] {
                let m = moves_at(emote, t).unwrap();
                assert!(m.lift.abs() < 1e-6, "{emote:?} at {t}: lift {}", m.lift);
                assert!(
                    m.arms.1.abs() < 1e-6,
                    "{emote:?} at {t}: arms held {}",
                    m.arms.1
                );
                assert!(
                    m.tilt[0].abs() < 1e-3 && m.tilt[1].abs() < 1e-3,
                    "{emote:?} at {t}"
                );
                assert!(m.leg_pose.1.abs() < 1e-6 && !m.crouch, "{emote:?} at {t}");
            }
            // Never under the ground on the way (Sit only as far as sitting on it).
            let (floor, step) = if emote == Emote::Sit {
                (-SIT_DROP, 1.7)
            } else {
                (0.0, 0.05)
            };
            let mut t = 0.0;
            while t < end {
                let lift = moves_at(emote, t).unwrap().lift;
                assert!(lift >= floor - 1e-9, "{emote:?} at {t}: {lift}");
                t += step;
            }
        }
        assert_eq!(moves_at(Emote::BlackHole, 1.0), None);
    }

    #[test]
    fn dark_ascension_lifts_the_player_and_flings_the_arms_wide_at_the_burst() {
        let risen = dark_at(DARK_RISEN + 0.5);
        assert!(risen.lift > DARK_TOP * 0.9 && risen.head_pitch < -30.0);
        let before = dark_at(DARK_BURST - 0.5).arms.0[0][2];
        let after = dark_at(DARK_BURST + 0.5).arms.0[0][2];
        assert!(
            before < 0.7 && after > 1.3,
            "the arms go from {before} to {after}"
        );
        // The arms mirror each other.
        let arms = dark_at(DARK_BURST + 0.5).arms.0;
        assert_eq!(arms[1][2], -arms[0][2]);
    }

    #[test]
    fn bruh_brings_the_right_hand_to_the_face_and_bows_the_head() {
        let m = bruh_at(BRUH_SLAP + 0.5);
        assert!(m.arms.1 > 0.99 && m.arms.0[0][0] < -2.0, "{m:?}");
        assert!(m.head_pitch > 20.0);
        assert_eq!(bruh_at(0.0).arms.1, 0.0);
    }

    #[test]
    fn the_l_dance_swaps_hands_halfway() {
        // The hand at the forehead is raised high (x near -2.55), the pumping one lower.
        let first = dance_at(DANCE_START + 2.0 * BEAT).arms.0;
        assert!(first[0][0] < -2.4 && first[1][0] > -1.5);
        let second = dance_at(DANCE_SWITCH + 2.0 * BEAT).arms.0;
        assert!(second[1][0] < -2.4 && second[0][0] > -1.5);
        // It bounces on the beat and stands still between the hops.
        assert!(dance_at(DANCE_START + 2.5 * BEAT).lift > 0.06);
        assert!(dance_at(DANCE_START + 3.0 * BEAT).lift < 0.01);
    }

    #[test]
    fn the_wings_unfurl_from_the_back_beat_and_scatter() {
        // Folded at first: close to the back; open: their tips well out to each side.
        let folded = wing_point(0.0, 1.0, 1.0, 0.0);
        let open = wing_point(WINGS_OPENED + 1.0, 1.0, 1.0, 0.0);
        assert!(
            folded[0].abs() < 0.8 && open[0] > 1.5,
            "{folded:?} {open:?}"
        );
        // Mirrored left and right.
        let left = wing_point(WINGS_OPENED + 1.0, -1.0, 1.0, 0.0);
        assert!((left[0] + open[0]).abs() < 1e-9 && (left[1] - open[1]).abs() < 1e-9);
        // The tips rise and fall as they beat.
        let tip = |t: f64| wing_point(t, 1.0, 1.0, 0.0)[1];
        let heights: Vec<f64> = (0..8)
            .map(|k| tip(WINGS_OPENED + 1.0 + f64::from(k) * WING_BEAT / 8.0))
            .collect();
        let (lo, hi) = heights
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), &h| (a.min(h), b.max(h)));
        assert!(hi - lo > 0.8, "the tip moves {lo} to {hi}");
        assert_eq!(wings_spread(WINGS_END), 0.0);
        assert!(wings_lift(WINGS_OPENED + 1.0) > 0.3);
    }

    #[test]
    fn sit_lowers_the_player_onto_the_ground_with_the_legs_out_in_front() {
        let m = sit_at(SIT_DOWN + 5.0);
        assert!((m.lift + SIT_DROP).abs() < 1e-9);
        let (legs, held) = m.leg_pose;
        assert_eq!(held, 1.0);
        assert!(legs[0][0] < -1.3 && legs[1][0] < -1.3, "{legs:?}");
        // Still sitting minutes later.
        assert_eq!(sit_at(300.0).lift, m.lift);
    }

    #[test]
    fn crouch_spam_crouches_on_and_off_many_times() {
        let (mut toggles, mut was, mut t) = (0, false, 0.0);
        while t < CROUCH_END {
            let now = crouch_at(t).crouch;
            toggles += usize::from(now != was);
            was = now;
            t += 0.01;
        }
        assert!(toggles >= 16, "{toggles} toggles");
    }

    #[test]
    fn point_and_laugh_points_straight_ahead_and_doubles_over() {
        let m = laugh_at(2.0);
        assert!(m.arms.1 > 0.99);
        assert!((m.arms.0[0][0] + 1.57).abs() < 0.15, "{:?}", m.arms.0[0]);
        assert!(m.tilt[0] < -8.0, "{:?}", m.tilt);
    }

    #[test]
    fn cry_about_it_rubs_the_eyes_then_waves() {
        let crying = cry_at(1.5).arms;
        assert!(crying.0[0][0] < -2.0 && crying.0[1][0] < -2.0);
        let waving = cry_at(CRY_WAVE + 0.8).arms;
        assert!(waving.0[0][0] < -2.6 && waving.0[1][0] > -0.2);
    }

    #[test]
    fn the_wheel_colours_swap_red_and_blue() {
        assert_eq!(hud(0x112233, 0xFF), 0xFF332211);
    }
}
