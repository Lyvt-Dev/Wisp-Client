//! Wisp emotes: B opens a wheel, the mouse picks a slot, a click plays it. The first emote
//! is the Black Hole: a hole opens above the player, who floats up weightless, lying back, and
//! drifts slowly round it while the world bends around it (`black_hole.wgsl`); then it fades
//! away with a soft ripple and sets the player down. The second is the Supernova Crown: a star
//! gathers above the player's head, swells, collapses and bursts in a flash and a shock wave;
//! the shell it throws off glows round the player, a pulsar at its centre, and its light
//! settles on their head as a turning crown. The same shader draws the star, the shell and
//! the crown; particles only add the matter streaming in and flung out. The third is the
//! Purple Hollow: the player gathers a blue orb that pulls the world in over one hand and a red
//! one that pushes it away over the other, brings them together into a purple sphere crawling
//! with lightning, and fires it: it tears away ahead of them, leaving a wake of violet light.
//! The fourth is False Dreams: the player rises off the ground as if possessed, head thrown
//! back and arms hanging, while its song plays; eyes drift round them, each turned to whoever
//! watches, and the player's own view swims, blurred and dark at its edges.
//! The fifth is The Rook: the player points at the sky, a giant ivory chess rook falls and
//! slams down in front of them, a Brilliant "!!" badge pops up over it, and the rook explodes
//! forward in fire and smoke, then crumbles away into teal sparks.
//! Dark Ascension, Bruh, the L Dance, Celestial Wings, Sit and the ragebait emotes (Crouch Spam,
//! Skill Issue, Too Easy, Point and Laugh, Cry About It) have no lens: they are a pose, particles
//! and sounds (`moves.rs`). The wheel has pages of ten: scrolling, or Q and E, turns them.
//! With the cosmetics service, other Wisp players see it too: the game tells the service
//! which emote started (`wisp_cosmetics::share`), and plays the emotes of the players in view
//! on them (`tick_remote_emotes`): their particles, sounds and the lens, at their place.
//! Nothing goes through the game's server. Emotes are bought in the launcher's Shop, which
//! lists the ones in the wheel in `emotes.txt` beside the worn items
//! (`WISP_COSMETICS_LOCAL`); the others show locked.
use super::Game;
use super::perspective::CameraType;
use std::f64::consts::{PI, TAU};
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use winit::{event::MouseButton, keyboard::KeyCode};
use wisp_client::fast_hash::FxHashMap;
use wisp_client::gfx::{BlackHole, FalseDreams as DreamLens, HollowPurple, Rook, Supernova};
use wisp_client::hud::HudBatch;
use wisp_client::particles::{ParticleOptions, ParticleType};
use wisp_core::geometry::Vec3;
use wisp_core::random::RandomSource;

/// The emotes there are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Emote {
    BlackHole,
    SupernovaCrown,
    PurpleHollow,
    FalseDreams,
    TheRook,
    DarkAscension,
    Bruh,
    LDance,
    CelestialWings,
    Sit,
    CrouchSpam,
    SkillIssue,
    TooEasy,
    PointAndLaugh,
    CryAboutIt,
}

impl Emote {
    const ALL: [Emote; 15] = [
        Emote::BlackHole,
        Emote::SupernovaCrown,
        Emote::PurpleHollow,
        Emote::FalseDreams,
        Emote::TheRook,
        Emote::DarkAscension,
        Emote::Bruh,
        Emote::LDance,
        Emote::CelestialWings,
        Emote::Sit,
        Emote::CrouchSpam,
        Emote::SkillIssue,
        Emote::TooEasy,
        Emote::PointAndLaugh,
        Emote::CryAboutIt,
    ];

    /// The emote with the Shop's id `id`.
    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.id() == id)
    }

    /// Whether it draws through the lens pass (the others are a pose and particles,
    /// `moves.rs`).
    fn has_lens(self) -> bool {
        moves::moves_at(self, 0.0).is_none()
    }

    /// Whether the shop sells it: only those are offered in the wheel.
    fn sold(self) -> bool {
        wisp_shop::offer(self.id()).is_some()
    }

    /// The Shop's id for it.
    fn id(self) -> &'static str {
        match self {
            Emote::BlackHole => "black_hole",
            Emote::SupernovaCrown => "supernova_crown",
            Emote::PurpleHollow => "purple_hollow",
            Emote::FalseDreams => "false_dreams",
            Emote::TheRook => "the_rook",
            Emote::DarkAscension => "dark_ascension",
            Emote::Bruh => "bruh",
            Emote::LDance => "l_dance",
            Emote::CelestialWings => "celestial_wings",
            Emote::Sit => "sit",
            Emote::CrouchSpam => "crouch_spam",
            Emote::SkillIssue => "skill_issue",
            Emote::TooEasy => "too_easy",
            Emote::PointAndLaugh => "point_and_laugh",
            Emote::CryAboutIt => "cry_about_it",
        }
    }

    fn name(self) -> String {
        match self {
            Emote::BlackHole => crate::lang::get_or("wisp.emote.black_hole", "Black Hole"),
            Emote::SupernovaCrown => {
                crate::lang::get_or("wisp.emote.supernova_crown", "Supernova Crown")
            }
            Emote::PurpleHollow => crate::lang::get_or("wisp.emote.purple_hollow", "Purple Hollow"),
            Emote::FalseDreams => crate::lang::get_or("wisp.emote.false_dreams", "False Dreams"),
            Emote::TheRook => crate::lang::get_or("wisp.emote.the_rook", "The Rook"),
            Emote::DarkAscension => {
                crate::lang::get_or("wisp.emote.dark_ascension", "Dark Ascension")
            }
            Emote::Bruh => crate::lang::get_or("wisp.emote.bruh", "Bruh"),
            Emote::LDance => crate::lang::get_or("wisp.emote.l_dance", "L Dance"),
            Emote::CelestialWings => {
                crate::lang::get_or("wisp.emote.celestial_wings", "Celestial Wings")
            }
            Emote::Sit => crate::lang::get_or("wisp.emote.sit", "Sit"),
            Emote::CrouchSpam => crate::lang::get_or("wisp.emote.crouch_spam", "Crouch Spam"),
            Emote::SkillIssue => crate::lang::get_or("wisp.emote.skill_issue", "Skill Issue"),
            Emote::TooEasy => crate::lang::get_or("wisp.emote.too_easy", "Too Easy"),
            Emote::PointAndLaugh => {
                crate::lang::get_or("wisp.emote.point_and_laugh", "Point and Laugh")
            }
            Emote::CryAboutIt => crate::lang::get_or("wisp.emote.cry_about_it", "Cry About It"),
        }
    }

    /// How long it plays, in seconds.
    fn length(self) -> f64 {
        match self {
            Emote::BlackHole => END,
            Emote::SupernovaCrown => NOVA_END,
            Emote::PurpleHollow => HOLLOW_END,
            Emote::FalseDreams => DREAMS_END,
            Emote::TheRook => ROOK_END,
            Emote::DarkAscension => moves::DARK_END,
            Emote::Bruh => moves::BRUH_END,
            Emote::LDance => moves::DANCE_END,
            Emote::CelestialWings => moves::WINGS_END,
            Emote::Sit => moves::SIT_END,
            Emote::CrouchSpam => moves::CROUCH_END,
            Emote::SkillIssue => moves::SKILL_END,
            Emote::TooEasy => moves::EASY_END,
            Emote::PointAndLaugh => moves::LAUGH_END,
            Emote::CryAboutIt => moves::CRY_END,
        }
    }

    /// What it costs in the launcher's Shop, in coins.
    fn price(self) -> u32 {
        match self {
            Emote::BlackHole => 600,
            Emote::SupernovaCrown => 800,
            Emote::PurpleHollow => 1200,
            Emote::FalseDreams => 500,
            Emote::TheRook => 700,
            Emote::DarkAscension => 1400,
            Emote::Bruh => 300,
            Emote::LDance => 450,
            Emote::CelestialWings => 1800,
            Emote::Sit => 300,
            Emote::CrouchSpam => 250,
            Emote::SkillIssue => 350,
            Emote::TooEasy => 400,
            Emote::PointAndLaugh => 450,
            Emote::CryAboutIt => 500,
        }
    }
}

mod moves;

/// The wheel's pages, each with its slots clockwise from the top.
const PAGES: [[Option<Emote>; 10]; 2] = [
    [
        Some(Emote::BlackHole),
        Some(Emote::SupernovaCrown),
        Some(Emote::PurpleHollow),
        Some(Emote::FalseDreams),
        Some(Emote::TheRook),
        Some(Emote::DarkAscension),
        Some(Emote::CelestialWings),
        Some(Emote::Bruh),
        Some(Emote::LDance),
        None,
    ],
    // Sit and the ragebait ones.
    [
        Some(Emote::Sit),
        Some(Emote::CrouchSpam),
        Some(Emote::SkillIssue),
        Some(Emote::TooEasy),
        Some(Emote::PointAndLaugh),
        Some(Emote::CryAboutIt),
        None,
        None,
        None,
        None,
    ],
];
/// Slots on a page.
const SLOTS: usize = 10;

// The Rook's timeline, in seconds: it starts falling, lands, its badge pops up, it blasts,
// it crumbles.
const ROOK_DROP: f64 = 0.7;
const ROOK_LAND: f64 = 1.25;
const ROOK_BADGE: f64 = 1.75;
const ROOK_BLAST: f64 = 2.9;
const ROOK_LEAVE: f64 = 6.6;
const ROOK_END: f64 = 8.0;
/// The rook's height, how far ahead of the player it lands and how far it falls, in blocks.
const ROOK_HEIGHT: f64 = 2.6;
const ROOK_AHEAD: f64 = 3.4;
const ROOK_FALL: f64 = 16.0;
const TEAL: [f32; 3] = [0.15, 0.85, 0.72];
const FIRE: [f32; 3] = [1.0, 0.45, 0.08];

// The Black Hole's timeline, in seconds: slow and calm, the player drifting weightless.
/// The hole has grown to its size.
const SUMMONED: f64 = 2.5;
/// It starts to fade away and set the player down.
const FADE: f64 = 13.0;
/// It is gone.
const GONE: f64 = 14.4;
/// The emote is over.
const END: f64 = 15.5;
// The Supernova Crown's timeline, in seconds.
/// The star has gathered and starts to swell.
const NOVA_FORMED: f64 = 2.6;
/// Its core gives way: it falls in on itself.
const NOVA_COLLAPSE: f64 = 3.6;
/// It bursts: flash, shock wave, the shell flying out.
const NOVA_BURST: f64 = 3.9;
/// The remnant and the crown start to fade.
const NOVA_FADE: f64 = 10.8;
const NOVA_END: f64 = 12.6;
/// How high above the feet the star gathers, and the crown's ring above them.
const NOVA_HEIGHT: f64 = 3.4;
const CROWN_HEIGHT: f64 = 1.86;
/// The crown's radius (`black_hole.wgsl`'s).
const CROWN_RADIUS: f64 = 0.4;
/// How far in front of the player the camera watches the star from, and the remnant.
const NOVA_CAMERA: f64 = 4.6;
const NOVA_CAMERA_FAR: f64 = 7.5;
const GOLD: [f32; 3] = [1.0, 0.68, 0.12];
const NOVA_PINK: [f32; 3] = [1.0, 0.38, 0.75];
const NOVA_CYAN: [f32; 3] = [0.45, 0.85, 1.0];
// The Purple Hollow's timeline, in seconds.
/// The blue orb starts to gather over the right hand, then the red one over the left.
const HOLLOW_BLUE: f64 = 0.7;
const HOLLOW_RED: f64 = 2.5;
/// The hands bring the two orbs together.
const HOLLOW_JOIN: f64 = 4.7;
/// They meet: the purple sphere is born and charges.
const HOLLOW_MERGE: f64 = 6.3;
/// It is fired.
const HOLLOW_FIRE: f64 = 8.6;
/// It has torn away into the distance; its wake fades.
const HOLLOW_AWAY: f64 = 10.7;
const HOLLOW_END: f64 = 13.2;
/// How high above the feet the hands hold the orbs, how far to each side, and how far ahead
/// of the player the orbs meet.
const HOLLOW_HANDS: f64 = 1.75;
const HOLLOW_APART: f64 = 1.15;
const HOLLOW_AHEAD: f64 = 1.5;
const HOLLOW_BLUE_LIGHT: [f32; 3] = [0.3, 0.62, 1.0];
const HOLLOW_RED_LIGHT: [f32; 3] = [1.0, 0.2, 0.14];
// False Dreams' timeline, in seconds: as long as its song.
/// The player has risen to their height.
const DREAMS_RISEN: f64 = 6.0;
/// The eyes start to come, one after another.
const DREAMS_EYES: f64 = 2.5;
/// The player starts to come down, and is on the ground again.
const DREAMS_FALL: f64 = 81.0;
const DREAMS_LANDED: f64 = 84.5;
const DREAMS_END: f64 = 86.0;
/// The highest the player floats over where they stood, in blocks.
const DREAMS_TOP: f64 = 1.8;
/// How many eyes there are, their radius and how far from the player they drift.
const DREAMS_EYE_COUNT: f32 = 9.0;
const DREAMS_EYE_SIZE: f32 = 0.17;
const DREAMS_EYE_ORBIT: f32 = 2.2;
/// How far the song is heard from another player playing it, in blocks.
const DREAMS_HEARD: f32 = 32.0;
/// The song, decoded once, the first time it is wanted (`dream_song`).
const DREAMS_SONG: &[u8] = include_bytes!("../../assets/emotes/false_dreams.mp3");
static DREAMS_DECODED: OnceLock<Option<(Arc<Vec<f32>>, u32)>> = OnceLock::new();
static DREAMS_DECODING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// False Dreams' song, once decoded: the first call starts decoding it on a thread of its own
/// and answers `None` until it is done.
fn dream_song() -> Option<(Arc<Vec<f32>>, u32)> {
    if let Some(song) = DREAMS_DECODED.get() {
        return song.clone();
    }
    if !DREAMS_DECODING.swap(true, std::sync::atomic::Ordering::Relaxed) {
        std::thread::Builder::new()
            .name("emote song".into())
            .spawn(|| {
                let song = wisp_client::audio::decode_bytes(DREAMS_SONG, "mp3")
                    .map(|(samples, rate)| (Arc::new(samples), rate));
                if song.is_none() {
                    log::warn!("Wisp: the False Dreams song did not decode");
                }
                let _ = DREAMS_DECODED.set(song);
            })
            .ok();
    }
    None
}

/// How loud False Dreams' song is `t` seconds in, 0 to 1: it comes in and fades at the end.
fn dream_volume(t: f64) -> f32 {
    (smoothstep(0.0, 0.8, t) * (1.0 - smoothstep(DREAMS_END - 2.5, DREAMS_END, t))) as f32
}

/// How far False Dreams has lifted the player, 0 to 1.
fn dream_float(t: f64) -> f64 {
    smoothstep(0.4, DREAMS_RISEN, t) * (1.0 - smoothstep(DREAMS_FALL, DREAMS_LANDED, t))
}

/// False Dreams' pose `t` seconds in, for a player who floats `top` blocks up at most.
struct Dream {
    /// How high they float over where they stood.
    lift: f64,
    /// Their lean back and roll (degrees).
    tilt: [f32; 2],
    /// Their arms (as `Extra::emote_arms`).
    arms: ([[f32; 3]; 2], f32),
    /// Their head's pitch (thrown back) and how far it turns from the body (degrees).
    head_pitch: f32,
    head_turn: f32,
}

fn dream_at(t: f64, top: f64) -> Dream {
    let float = dream_float(t);
    // Every nine seconds or so a jerk runs through them, as if something pulled.
    let k = (t * 0.11 + 0.3).fract();
    let jerk = if k < 0.025 {
        (k / 0.025 * PI).sin()
    } else {
        0.0
    };
    let hang = |phase: f64, side: f64| -> [f32; 3] {
        [
            (0.2 + 0.06 * (t * 0.8 + phase).sin() - 0.35 * jerk) as f32,
            0.0,
            ((0.32 + 0.05 * (t * 0.6 + phase * 2.0).sin() + 0.2 * jerk) * side) as f32,
        ]
    };
    Dream {
        lift: (top + 0.07 * (t * 1.1).sin()) * float,
        tilt: [
            ((9.0 + 4.0 * (t * 0.37).sin() + 9.0 * jerk) * float) as f32,
            ((5.0 * (t * 0.29).sin() - 6.0 * jerk) * float) as f32,
        ],
        arms: ([hang(0.0, 1.0), hang(1.0, -1.0)], float as f32),
        head_pitch: ((-55.0 + 6.0 * (t * 0.5).sin() - 15.0 * jerk) * float) as f32,
        head_turn: (10.0 * (t * 0.23).sin() * float) as f32,
    }
}

/// How many of False Dreams' eyes show, 0 to 1.
fn dream_eyes(t: f64) -> f64 {
    smoothstep(DREAMS_EYES, DREAMS_EYES + 9.0, t)
        * (1.0 - smoothstep(DREAMS_FALL - 1.5, DREAMS_LANDED - 0.5, t))
}

/// The horizon's radius, in blocks.
const RADIUS: f64 = 0.75;
/// How far the player orbits from its centre.
const ORBIT: f64 = 3.8;
/// How far above the disk the player's feet fly, so the disk never buries them.
const ABOVE_DISK: f64 = 0.8;
/// How high above the player's feet the hole opens.
const HOVER: f64 = 3.0;
/// How far the camera stays from the hole, and how far above its disk.
const CAMERA_DISTANCE: f64 = 10.0;
const CAMERA_HEIGHT: f64 = 2.4;

#[derive(Default)]
struct Wheel {
    /// Where the mouse has pushed the selector, in wheel radii (screen y down).
    aim: (f64, f64),
    /// The page shown (`PAGES`).
    page: usize,
    /// The Shop ids of the emotes the player has in the wheel.
    owned: Vec<String>,
}

impl Wheel {
    fn open() -> Self {
        let owned = wisp_compat::var_os("WISP_COSMETICS_LOCAL")
            .and_then(|dir| {
                std::fs::read_to_string(std::path::Path::new(&dir).join("emotes.txt")).ok()
            })
            .map(|text| parse_owned(&text))
            .unwrap_or_default();
        Self {
            aim: (0.0, 0.0),
            page: 0,
            owned,
        }
    }

    fn owns(&self, emote: Emote) -> bool {
        self.owned.iter().any(|id| id == emote.id())
    }
}

/// The ids in `emotes.txt`: one a line.
fn parse_owned(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

impl Wheel {
    fn hovered(&self) -> Option<usize> {
        let (x, y) = self.aim;
        if x.hypot(y) < 0.35 {
            return None;
        }
        // Slot 0 is at the top; the slots go clockwise.
        let angle = y.atan2(x) + PI / 2.0;
        let n = SLOTS as i64;
        Some(((angle / (TAU / n as f64)).round() as i64).rem_euclid(n) as usize)
    }

    /// The emote in `slot` of the page shown.
    /// The emote in `slot` of the page shown, if the shop sells it.
    fn at(&self, slot: usize) -> Option<Emote> {
        PAGES[self.page][slot].filter(|e| e.sold())
    }

    /// Turns `steps` pages on (back when negative), round from the last to the first.
    fn turn(&mut self, steps: i32) {
        self.page = (self.page as i32 + steps).rem_euclid(PAGES.len() as i32) as usize;
    }
}

struct Playing {
    emote: Emote,
    started: Instant,
    /// Ticks since it started.
    ticks: u32,
    /// The hole's centre.
    anchor: [f64; 3],
    /// The player's feet when it started.
    feet: [f64; 3],
    /// The orbit's starting angle.
    orbit_angle: f64,
    /// The camera mode to go back to.
    camera_before: CameraType,
    /// How far the model was moved from the player last tick.
    offset: [f64; 3],
    /// The model's last yaws, to keep turning the short way.
    body_yaw: f32,
    head_yaw: f32,
    /// The time at the last tick, to catch the moments it passes.
    last: f64,
    /// Its song's channel, once playing.
    song: Option<u64>,
}

impl Playing {
    fn time(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }
}

/// An emote another player is playing, seen here.
struct Remote {
    emote: Emote,
    started: Instant,
    /// Where it plays: the hole or the star, and the player's feet when it started.
    anchor: [f64; 3],
    feet: [f64; 3],
    ticks: u32,
    last: f64,
    /// The player's entity, and how far its model was moved from where the server has it.
    entity: u32,
    offset: [f64; 3],
    body_yaw: f32,
    /// The way they faced when it started (radians).
    yaw: f64,
    /// Its song's channel, once playing.
    song: Option<u64>,
}

#[derive(Default)]
pub(super) struct Emotes {
    wheel: Option<Wheel>,
    playing: Option<Playing>,
    /// Other players' emotes, by their UUID.
    remote: FxHashMap<u128, Remote>,
    /// How far the mouse has turned the camera round a sitting player, in degrees.
    look: f64,
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The yaw (degrees, Minecraft's: 0 faces +z) looking along `dx, dz`.
fn yaw_of(dx: f64, dz: f64) -> f32 {
    (-dx).atan2(dz).to_degrees() as f32
}

fn wrap(degrees: f32) -> f32 {
    (degrees + 180.0).rem_euclid(360.0) - 180.0
}

/// The horizon radius at `t` (0 once gone).
fn hole_radius(t: f64) -> f64 {
    if t < SUMMONED {
        RADIUS * smoothstep(0.0, SUMMONED, t)
    } else if t < FADE {
        // A slow breath.
        RADIUS * (1.0 + 0.03 * ((t - SUMMONED) * 1.1).sin())
    } else if t < GONE {
        RADIUS * (1.0 - smoothstep(FADE, GONE, t))
    } else {
        0.0
    }
}

/// How far the Supernova Crown lifts the player off the ground (blocks).
fn nova_lift(t: f64) -> f64 {
    0.35 * smoothstep(0.4, 2.4, t) * (1.0 - smoothstep(NOVA_FADE, NOVA_END - 0.2, t))
}

/// How much is left of the supernova's remnant and the crown, 1 to 0.
fn nova_life(t: f64) -> f64 {
    1.0 - smoothstep(NOVA_FADE, NOVA_END - 0.3, t)
}

/// The star's radius in blocks: it gathers, swells and throbs, then falls in on itself; once
/// burst, the size of the glare it leaves.
fn nova_star(t: f64) -> f64 {
    if t < NOVA_BURST {
        let swell = smoothstep(NOVA_FORMED, NOVA_COLLAPSE, t);
        let size = 0.42 * smoothstep(0.0, NOVA_FORMED, t) + 0.2 * swell;
        let throb = 1.0 + 0.06 * swell * (t * 38.0).sin();
        size * throb * (1.0 - 0.8 * smoothstep(NOVA_COLLAPSE, NOVA_BURST, t))
    } else {
        0.2 + 0.45 * (-(t - NOVA_BURST) * 1.5).exp()
    }
}

/// How hard the star shines: brighter as it swells, hardest as it collapses and bursts, then
/// the pulsar's faint glow.
fn nova_power(t: f64) -> f64 {
    if t < NOVA_BURST {
        0.2 + 0.6 * smoothstep(0.0, NOVA_FORMED, t)
            + 0.5 * smoothstep(NOVA_FORMED, NOVA_COLLAPSE, t)
            + 1.2 * smoothstep(NOVA_COLLAPSE, NOVA_BURST, t)
    } else {
        (3.0 * (-(t - NOVA_BURST) * 2.2).exp() + 0.3) * nova_life(t)
    }
}

/// Where the Purple Hollow's orbs and its sphere are `t` seconds in, for a player whose feet
/// were at `feet`, facing `yaw` (radians, Minecraft's).
struct Hollow {
    blue: [f64; 3],
    red: [f64; 3],
    /// The blue and the red orb's radii.
    orbs: [f64; 2],
    /// The purple sphere, and where it was fired from.
    sphere: [f64; 3],
    origin: [f64; 3],
    radius: f64,
    /// The way the player faces, level.
    forward: [f64; 2],
}

fn hollow_at(t: f64, feet: [f64; 3], yaw: f64) -> Hollow {
    let forward = [-yaw.sin(), yaw.cos()];
    let right = [-yaw.cos(), -yaw.sin()];
    let at = |ahead: f64, aside: f64, up: f64| {
        [
            feet[0] + forward[0] * ahead + right[0] * aside,
            feet[1] + up,
            feet[2] + forward[1] * ahead + right[1] * aside,
        ]
    };
    let origin = at(HOLLOW_AHEAD, 0.0, 1.5);
    // The orbs hang over the hands, bobbing, then circle in on each other.
    let join = smoothstep(HOLLOW_JOIN, HOLLOW_MERGE, t);
    let whirl = (t - HOLLOW_JOIN).max(0.0) * 5.0 * join;
    let apart = HOLLOW_APART * (1.0 - join);
    let orb = |side: f64, phase: f64| {
        let held = at(
            0.45 + apart * 0.35 * whirl.sin() * side,
            apart * whirl.cos() * side,
            HOLLOW_HANDS
                + 0.05 * (t * 1.7 + phase).sin()
                + 0.25 * apart * (whirl * 0.5).sin() * side,
        );
        std::array::from_fn(|i| held[i] + (origin[i] - held[i]) * join)
    };
    let gone = 1.0 - smoothstep(HOLLOW_MERGE - 0.12, HOLLOW_MERGE + 0.12, t);
    let orbs = [
        0.27 * smoothstep(HOLLOW_BLUE, HOLLOW_BLUE + 1.5, t) * gone,
        0.27 * smoothstep(HOLLOW_RED, HOLLOW_RED + 1.5, t) * gone,
    ];
    // The sphere charges where they met, throbbing harder and harder, then tears away.
    let age = t - HOLLOW_FIRE;
    let (radius, travelled) = if t < HOLLOW_MERGE {
        (0.0, 0.0)
    } else if age < 0.0 {
        let charge = smoothstep(HOLLOW_MERGE, HOLLOW_FIRE, t);
        let throb = 1.0 + 0.05 * charge * (t * 30.0).sin();
        (
            (0.22 + 0.5 * charge) * throb * smoothstep(HOLLOW_MERGE, HOLLOW_MERGE + 0.25, t),
            0.0,
        )
    } else {
        let size = 0.72 + 0.9 * smoothstep(0.0, 1.3, age);
        (
            size * (1.0 - smoothstep(HOLLOW_AWAY - 0.3, HOLLOW_AWAY + 0.4, t)),
            3.0 * age + 13.0 * age * age,
        )
    };
    let travelled = travelled.min(64.0);
    Hollow {
        blue: orb(1.0, 0.0),
        red: orb(-1.0, 2.0),
        orbs,
        sphere: [
            origin[0] + forward[0] * travelled,
            origin[1],
            origin[2] + forward[1] * travelled,
        ],
        origin,
        radius,
        forward,
    }
}

/// How hard the Purple Hollow's sphere shines.
fn hollow_power(t: f64) -> f64 {
    if t < HOLLOW_FIRE {
        0.7 + 0.9 * smoothstep(HOLLOW_MERGE, HOLLOW_FIRE, t)
    } else {
        1.3 + 0.8 * (-(t - HOLLOW_FIRE) * 2.0).exp()
    }
}

/// How much is left of the sphere's wake, 1 to 0.
fn hollow_wake(t: f64) -> f64 {
    1.0 - smoothstep(HOLLOW_AWAY - 0.4, HOLLOW_END - 0.5, t)
}

/// The Purple Hollow's lean (pitch, roll in degrees): back as the sphere charges, a recoil as
/// it is fired.
fn hollow_tilt(t: f64) -> [f32; 2] {
    let brace = smoothstep(HOLLOW_MERGE, HOLLOW_FIRE, t)
        * (1.0 - smoothstep(HOLLOW_FIRE, HOLLOW_FIRE + 0.2, t));
    let recoil = if t > HOLLOW_FIRE {
        (-(t - HOLLOW_FIRE) * 4.0).exp() * 11.0
    } else {
        0.0
    };
    [(-5.0 * brace + recoil) as f32, 0.0]
}

/// The Purple Hollow's arms (right, left: x, y, z in radians) and how far they hold: each hand
/// lifts its orb out to its side, both come together in front, the right one fires.
fn hollow_arms(t: f64) -> ([[f32; 3]; 2], f32) {
    let lift = |from: f64| smoothstep(from - 0.5, from + 0.7, t);
    let join = smoothstep(HOLLOW_JOIN, HOLLOW_MERGE, t);
    let fire = smoothstep(HOLLOW_FIRE - 0.12, HOLLOW_FIRE + 0.1, t);
    let sway = 0.04 * (t * 1.7).sin();
    // Out to the side, palm up; then both hands in front of the chest; then the right arm
    // thrown straight out and the left swept back.
    let arm = |raised: f64, side: f64, fired: [f64; 3]| -> [f32; 3] {
        let held = [-1.25 + sway, 0.85 * side, 0.12 * side];
        let joined = [-1.42, -0.32 * side, 0.0];
        std::array::from_fn(|i| {
            let before = held[i] + (joined[i] - held[i]) * join;
            ((before + (fired[i] - before) * fire) * raised) as f32
        })
    };
    let weight = smoothstep(HOLLOW_BLUE - 0.5, HOLLOW_BLUE + 0.3, t)
        * (1.0 - smoothstep(HOLLOW_AWAY, HOLLOW_END - 0.6, t));
    (
        [
            arm(lift(HOLLOW_BLUE), 1.0, [-1.6, -0.08, 0.0]),
            arm(lift(HOLLOW_RED), -1.0, [0.55, 0.0, -0.25]),
        ],
        weight as f32,
    )
}

/// Where The Rook's pieces are.
struct RookAt {
    /// The rook's base (falling from the sky until it lands), how far it leans back (radians)
    /// and how much of it is left, 1 to 0.
    base: [f64; 3],
    tilt: f64,
    shown: f64,
    /// The badge over it and its size (0: not there).
    badge: [f64; 3],
    badge_size: f64,
    /// Where the blast starts, and the way it goes (level).
    origin: [f64; 3],
    forward: [f64; 2],
}

/// The Rook `t` seconds in, landing at `ground` for a player facing `yaw` (radians).
fn rook_at(t: f64, ground: [f64; 3], yaw: f64) -> RookAt {
    let forward = [-yaw.sin(), yaw.cos()];
    // It falls faster and faster, then hops once or twice where it lands.
    let drop = if t < ROOK_LAND {
        let u = ((t - ROOK_DROP) / (ROOK_LAND - ROOK_DROP)).clamp(0.0, 1.0);
        ROOK_FALL * (1.0 - u * u)
    } else {
        let a = t - ROOK_LAND;
        0.2 * (a * 9.0).sin().abs() * (-a * 7.0).exp()
    };
    let shown = if t < ROOK_DROP {
        0.0
    } else {
        1.0 - smoothstep(ROOK_LEAVE, ROOK_END - 0.5, t)
    };
    // A wobble as it lands; knocked back by its own blast, then settling.
    let landed = t - ROOK_LAND;
    let wobble = if landed > 0.0 {
        0.07 * (landed * 14.0).sin() * (-landed * 5.0).exp()
    } else {
        0.0
    };
    let blasted = t - ROOK_BLAST;
    let recoil = if blasted > 0.0 {
        0.2 * (blasted * 10.0).min(1.0) * (-blasted * 2.2).exp()
    } else {
        0.0
    };
    // The badge pops up with a springy overshoot, bobs, and shrinks away before the rook goes.
    let popped = t - ROOK_BADGE;
    let badge_size = if popped > 0.0 {
        smoothstep(0.0, 0.16, popped)
            * (1.0 + 0.3 * (-popped * 5.0).exp() * (popped * 16.0).sin())
            * (1.0 - smoothstep(ROOK_LEAVE - 0.3, ROOK_LEAVE + 0.4, t))
    } else {
        0.0
    };
    let base = [ground[0], ground[1] + drop, ground[2]];
    RookAt {
        base,
        tilt: wobble + recoil,
        shown,
        badge: [
            ground[0],
            base[1] + ROOK_HEIGHT + 0.8 + 0.06 * (t * 2.2).sin(),
            ground[2],
        ],
        badge_size,
        origin: [
            ground[0] + forward[0] * 0.75,
            ground[1] + 1.25,
            ground[2] + forward[1] * 0.75,
        ],
        forward,
    }
}

/// How hard The Rook's blast burns, `age` seconds after it.
fn rook_blast_power(age: f64) -> f64 {
    0.4 + 1.4 * (-age * 0.9).exp()
}

/// The Rook's arms (right, left: x, y, z in radians) and how far they hold: the right hand
/// calls the rook down from the sky, then points at it; the left fist goes up as it blasts.
fn rook_arms(t: f64) -> ([[f32; 3]; 2], f32) {
    let point = smoothstep(ROOK_LAND - 0.25, ROOK_LAND + 0.15, t);
    let cheer = smoothstep(ROOK_BLAST - 0.1, ROOK_BLAST + 0.25, t);
    let up = [-2.9, -0.1, 0.1];
    let at = [-1.5, -0.12, 0.0];
    let right: [f32; 3] = std::array::from_fn(|i| (up[i] + (at[i] - up[i]) * point) as f32);
    let fist = [-2.7, 0.0, -0.3];
    let left: [f32; 3] = std::array::from_fn(|i| (fist[i] * cheer) as f32);
    let weight = smoothstep(0.0, 0.3, t) * (1.0 - smoothstep(ROOK_LEAVE, ROOK_END - 0.3, t));
    ([right, left], weight as f32)
}

/// The Rook's lean (pitch, roll in degrees): rocked back by the blast.
fn rook_tilt(t: f64) -> [f32; 2] {
    let blasted = t - ROOK_BLAST;
    if blasted > 0.0 {
        [(7.0 * (-blasted * 3.0).exp()) as f32, 0.0]
    } else {
        [0.0, 0.0]
    }
}

/// How far the player has floated up and out, 0 to 1.
fn rise(t: f64) -> f64 {
    smoothstep(0.3, 3.6, t) * (1.0 - smoothstep(FADE, END - 0.3, t))
}

/// How far the player has turned onto their back, 0 to 1.
fn lying(t: f64) -> f64 {
    smoothstep(0.8, 3.8, t) * (1.0 - smoothstep(FADE, END - 0.5, t))
}

/// Where the model's feet are at `t`, as a fraction of the way from the player's feet to the
/// orbit (`rise`), and the orbit's angle.
fn orbit(p: &Playing, t: f64) -> ([f64; 3], f64) {
    orbit_from(p.anchor, p.feet, p.orbit_angle, t)
}

/// The orbit of a player whose feet were at `feet` round the hole at `anchor`, starting
/// `orbit_angle` round it.
fn orbit_from(anchor: [f64; 3], feet: [f64; 3], orbit_angle: f64, t: f64) -> ([f64; 3], f64) {
    let rise = rise(t);
    // A slow, even drift round the hole, the distance and height breathing gently.
    let angle = orbit_angle + 0.2 * t;
    let radius = ORBIT + 0.3 * (t * 0.45).sin();
    let around = [
        anchor[0] + angle.cos() * radius,
        anchor[1] + ABOVE_DISK + 0.35 * (t * 0.8).sin() + 0.15 * (t * 0.31).sin(),
        anchor[2] + angle.sin() * radius,
    ];
    let at = std::array::from_fn(|i| feet[i] + (around[i] - feet[i]) * rise);
    (at, angle)
}

/// The Black Hole's lying pose at `t` (pitch, roll in degrees).
fn floating_tilt(t: f64) -> [f32; 2] {
    let lie = lying(t);
    [
        ((82.0 + 8.0 * (t * 0.55).sin()) * lie) as f32,
        (16.0 * (t * 0.4).sin() * lie) as f32,
    ]
}

/// The Supernova Crown's lean at `t`: back to watch the star form, a jolt at the burst.
fn nova_tilt(t: f64) -> [f32; 2] {
    let watch = smoothstep(0.3, 1.8, t) * (1.0 - smoothstep(NOVA_BURST, NOVA_BURST + 1.2, t));
    let jolt = if t > NOVA_BURST {
        (-(t - NOVA_BURST) * 5.0).exp() * 14.0
    } else {
        0.0
    };
    let fade = 1.0 - smoothstep(NOVA_FADE, NOVA_END - 0.3, t);
    [((12.0 * watch + jolt) * fade) as f32, 0.0]
}

/// Where the camera is, `distance` from the hole at `angle` round it.
fn camera_at(anchor: [f64; 3], angle: f64, distance: f64) -> [f64; 3] {
    [
        anchor[0] + angle.cos() * distance,
        anchor[1] + CAMERA_HEIGHT,
        anchor[2] + angle.sin() * distance,
    ]
}

/// A random unit vector.
fn direction(r: &mut wisp_core::random::LegacyRandom) -> [f64; 3] {
    let y = r.next_double() * 2.0 - 1.0;
    let a = r.next_double() * TAU;
    let s = (1.0 - y * y).sqrt();
    [a.cos() * s, y, a.sin() * s]
}

const VIOLET: [f32; 3] = [0.72, 0.36, 1.0];
const ORANGE: [f32; 3] = [1.0, 0.55, 0.15];
const WHITE: [f32; 3] = [1.0, 0.95, 0.9];

/// The lens `emote` draws `t` seconds in, at `anchor` (the hole or the star) for a player whose
/// feet were at `feet`, facing `yaw` (radians) when it started; none for an emote without one.
fn lens_at(emote: Emote, t: f64, anchor: [f64; 3], feet: [f64; 3], yaw: f64) -> Option<BlackHole> {
    emote
        .has_lens()
        .then(|| lens_of(emote, t, anchor, feet, yaw))
}

fn lens_of(emote: Emote, t: f64, anchor: [f64; 3], feet: [f64; 3], yaw: f64) -> BlackHole {
    if emote == Emote::TheRook {
        let r = rook_at(t, anchor, yaw);
        let landed = t - ROOK_LAND;
        let blasted = t - ROOK_BLAST;
        let flash = if blasted > 0.0 {
            0.85 * (-blasted * 4.5).exp()
        } else if landed > 0.0 {
            0.22 * (-landed * 8.0).exp()
        } else {
            0.0
        };
        let shock = if (0.0..1.2).contains(&blasted) {
            blasted * 30.0
        } else if (0.0..0.6).contains(&landed) {
            landed * 14.0
        } else {
            0.0
        };
        return BlackHole {
            centre: glam::DVec3::from_array(r.base),
            radius: ROOK_HEIGHT as f32,
            disk_normal: [r.forward[0] as f32, 0.0, r.forward[1] as f32],
            time: t as f32,
            strength: r.shown as f32,
            flash: flash as f32,
            shock: shock as f32,
            nova: None,
            hollow: None,
            dreams: None,
            rook: Some(Rook {
                tilt: r.tilt as f32,
                blast_age: blasted as f32,
                blast_power: rook_blast_power(blasted.max(0.0)) as f32,
                badge: glam::DVec3::from_array(r.badge),
                badge_shown: r.badge_size as f32,
                origin: glam::DVec3::from_array(r.origin),
            }),
        };
    }
    if emote == Emote::FalseDreams {
        let lift = dream_at(t, anchor[1] - feet[1]).lift;
        let blur = smoothstep(0.5, 4.0, t) * (1.0 - smoothstep(DREAMS_FALL, DREAMS_END - 0.3, t));
        return BlackHole {
            rook: None,
            centre: glam::DVec3::new(feet[0], feet[1] + lift + 1.0, feet[2]),
            radius: DREAMS_EYE_SIZE,
            disk_normal: [0.0, 1.0, 0.0],
            time: t as f32,
            strength: 0.0,
            flash: 0.0,
            shock: 0.0,
            nova: None,
            hollow: None,
            dreams: Some(DreamLens {
                eyes: dream_eyes(t) as f32,
                blur: (0.8 * blur) as f32,
                orbit: DREAMS_EYE_ORBIT,
                count: DREAMS_EYE_COUNT,
            }),
        };
    }
    if emote == Emote::PurpleHollow {
        let h = hollow_at(t, feet, yaw);
        // A flash as the orbs meet, a harder one as the sphere is fired.
        let flash = if t >= HOLLOW_FIRE {
            0.75 * (-(t - HOLLOW_FIRE) * 4.5).exp()
        } else if t >= HOLLOW_MERGE {
            0.45 * (-(t - HOLLOW_MERGE) * 6.0).exp()
        } else {
            0.0
        };
        let shock = if (HOLLOW_FIRE..HOLLOW_FIRE + 1.2).contains(&t) {
            (t - HOLLOW_FIRE) * 50.0
        } else {
            0.0
        };
        let dim = 0.25 * smoothstep(HOLLOW_BLUE, HOLLOW_JOIN, t)
            + 0.55 * smoothstep(HOLLOW_JOIN, HOLLOW_FIRE, t);
        return BlackHole {
            rook: None,
            centre: glam::DVec3::from_array(h.sphere),
            radius: h.radius as f32,
            disk_normal: [0.0, 1.0, 0.0],
            time: t as f32,
            strength: (dim * hollow_wake(t)) as f32,
            flash: flash as f32,
            shock: shock as f32,
            nova: None,
            dreams: None,
            hollow: Some(HollowPurple {
                blue: glam::DVec3::from_array(h.blue),
                red: glam::DVec3::from_array(h.red),
                orbs: [h.orbs[0] as f32, h.orbs[1] as f32],
                origin: glam::DVec3::from_array(h.origin),
                age: (t - HOLLOW_FIRE) as f32,
                power: hollow_power(t) as f32,
                wake: hollow_wake(t) as f32,
            }),
        };
    }
    if emote == Emote::SupernovaCrown {
        let flash = if t >= NOVA_BURST {
            0.85 * (-(t - NOVA_BURST) * 4.0).exp()
        } else {
            0.05 * smoothstep(NOVA_COLLAPSE, NOVA_BURST, t)
        };
        let shock = if (NOVA_BURST..NOVA_BURST + 1.4).contains(&t) {
            (t - NOVA_BURST) * 55.0
        } else {
            0.0
        };
        return BlackHole {
            rook: None,
            centre: glam::DVec3::from_array(anchor),
            radius: 0.5,
            disk_normal: [0.0, 1.0, 0.0],
            time: t as f32,
            // The star outshines the world round it.
            strength: (0.8 * smoothstep(0.3, NOVA_FORMED, t) * nova_life(t)) as f32,
            flash: flash as f32,
            shock: shock as f32,
            nova: Some(Supernova {
                star: nova_star(t) as f32,
                age: (t - NOVA_BURST) as f32,
                life: nova_life(t) as f32,
                power: nova_power(t) as f32,
                crown: glam::DVec3::new(feet[0], feet[1] + CROWN_HEIGHT + nova_lift(t), feet[2]),
                crown_shown: (smoothstep(NOVA_BURST + 0.7, NOVA_BURST + 2.0, t) * nova_life(t))
                    as f32,
            }),
            hollow: None,
            dreams: None,
        };
    }
    let radius = hole_radius(t);
    // A faint glow as it opens and as it closes; a soft ripple once it's gone.
    let flash = if t < 1.0 {
        0.12 * (1.0 - t)
    } else if t >= GONE {
        0.2 * (-(t - GONE) * 3.0).exp()
    } else {
        0.0
    };
    let shock = if (GONE..GONE + 1.5).contains(&t) {
        (t - GONE) * 18.0
    } else {
        0.0
    };
    BlackHole {
        rook: None,
        centre: glam::DVec3::from_array(anchor),
        // The ripple is measured in horizon radii: keep the size once it's gone.
        radius: if radius > 0.0 { radius } else { RADIUS } as f32,
        disk_normal: [0.12, 1.0, 0.22],
        time: t as f32,
        strength: if radius > 0.0 {
            smoothstep(0.0, 1.0, t) as f32
        } else {
            0.0
        },
        flash: flash as f32,
        shock: shock as f32,
        nova: None,
        hollow: None,
        dreams: None,
    }
}

impl Game<'_> {
    /// Whether the block at `at` is air.
    fn air(&self, at: [f64; 3]) -> bool {
        let state = self.world.block(
            at[0].floor() as i32,
            at[1].floor() as i32,
            at[2].floor() as i32,
        );
        self.res.facts().get(state).name.ends_with("air")
    }

    /// Whether everything within `radius` of `at` is air.
    fn open_around(&self, at: [f64; 3], radius: f64) -> bool {
        let steps = 2;
        (-steps..=steps).all(|x| {
            (-steps..=steps).all(|y| {
                (-steps..=steps).all(|z| {
                    let d = [x, y, z].map(|v| f64::from(v) * radius / f64::from(steps));
                    d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > radius * radius
                        || self.air([at[0] + d[0], at[1] + d[1], at[2] + d[2]])
                })
            })
        })
    }

    /// Whether the line from `from` to `to` is clear.
    fn sees(&self, from: [f64; 3], to: [f64; 3]) -> bool {
        let length = (0..3)
            .map(|i| (to[i] - from[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        let steps = (length / 0.5).ceil() as usize;
        (0..steps).all(|k| {
            let f = k as f64 / steps as f64;
            self.air(std::array::from_fn(|i| from[i] + (to[i] - from[i]) * f))
        })
    }

    /// Where The Rook lands for a player at `feet` facing `yaw` (radians): on the ground a few
    /// blocks ahead, nearer when that's no place for it, on the player's own level at worst.
    fn rook_spot(&self, feet: [f64; 3], yaw: f64) -> [f64; 3] {
        let forward = [-yaw.sin(), yaw.cos()];
        for ahead in [ROOK_AHEAD, 2.7, 2.0] {
            let (x, z) = (feet[0] + forward[0] * ahead, feet[2] + forward[1] * ahead);
            // A full block to stand on, and nothing solid where it stands (grass and flowers
            // are no matter).
            let solid = |y: f64| {
                let state = self
                    .world
                    .block(x.floor() as i32, y.floor() as i32, z.floor() as i32);
                self.res.facts().get(state).full_collision
            };
            let ground = (-4..=2)
                .rev()
                .map(|dy| feet[1].floor() + f64::from(dy))
                .find(|&y| solid(y - 0.5) && (0..3).all(|up| !solid(y + 0.5 + f64::from(up))));
            if let Some(y) = ground {
                return [x, y, z];
            }
        }
        [
            feet[0] + forward[0] * ROOK_AHEAD,
            feet[1],
            feet[2] + forward[1] * ROOK_AHEAD,
        ]
    }

    /// B opens or closes the wheel; Escape closes it; moving stops an emote. True when the
    /// key was the wheel's.
    pub(super) fn emote_key(&mut self, key: KeyCode, raw: KeyCode) -> bool {
        if self.emotes.wheel.is_some() && key == KeyCode::Escape {
            self.emotes.wheel = None;
            return true;
        }
        if let Some(wheel) = &mut self.emotes.wheel
            && matches!(key, KeyCode::KeyQ | KeyCode::KeyE)
        {
            wheel.turn(if key == KeyCode::KeyE { 1 } else { -1 });
            self.audio
                .play("minecraft:item.book.page_turn", None, 0.6, 1.2);
            return true;
        }
        if self.emotes.playing.is_some()
            && matches!(
                key,
                KeyCode::KeyW | KeyCode::KeyA | KeyCode::KeyS | KeyCode::KeyD | KeyCode::Space
            )
        {
            self.stop_emote();
            return false;
        }
        if key != KeyCode::KeyB || raw != KeyCode::KeyB || self.keys.contains(&KeyCode::F3) {
            return false;
        }
        if self.emotes.wheel.take().is_none() {
            // No emote in the shop: B is an ordinary key.
            if self.player.is_none() || !Emote::ALL.into_iter().any(Emote::sold) {
                return false;
            }
            let wheel = Wheel::open();
            if wheel.owns(Emote::FalseDreams) {
                dream_song();
            }
            self.emotes.wheel = Some(wheel);
            self.audio.play("minecraft:ui.button.click", None, 0.4, 1.6);
        }
        true
    }

    /// A mouse button while playing: the wheel's while it is open.
    pub(super) fn emote_mouse(&mut self, button: MouseButton, pressed: bool) -> bool {
        let Some(wheel) = &self.emotes.wheel else {
            return false;
        };
        if pressed {
            match button {
                MouseButton::Left => {
                    let chosen = wheel
                        .hovered()
                        .and_then(|slot| wheel.at(slot))
                        .filter(|&e| wheel.owns(e));
                    if let Some(emote) = chosen {
                        self.emotes.wheel = None;
                        self.start_emote(emote);
                    }
                }
                MouseButton::Right => self.emotes.wheel = None,
                _ => {}
            }
        }
        true
    }

    /// Whether the emote wheel or a playing emote has the camera (freelook waits for them).
    pub(super) fn emote_holds_camera(&self) -> bool {
        self.emotes.wheel.is_some() || self.emotes.playing.is_some()
    }

    /// Wheel steps while playing: the open emote wheel turns its pages with them. True when
    /// they were taken.
    pub(super) fn emote_scroll(&mut self, steps: f32) -> bool {
        let Some(wheel) = &mut self.emotes.wheel else {
            return false;
        };
        if steps.abs() >= 0.5 {
            // Down (negative) goes on, as a list scrolls.
            wheel.turn(if steps < 0.0 { 1 } else { -1 });
            self.audio
                .play("minecraft:item.book.page_turn", None, 0.6, 1.2);
        }
        true
    }

    /// Mouse movement while playing: it steers the wheel, and an emote holds the view still
    /// (Sit's turns the camera round the player). True when it was taken.
    pub(super) fn emote_look(&mut self, x: f64, y: f64) -> bool {
        if self
            .emotes
            .playing
            .as_ref()
            .is_some_and(|p| p.emote == Emote::Sit)
        {
            self.emotes.look = (self.emotes.look + x * 0.15).rem_euclid(360.0);
            return true;
        }
        if let Some(wheel) = &mut self.emotes.wheel {
            let (ax, ay) = (wheel.aim.0 + x * 0.02, wheel.aim.1 + y * 0.02);
            let length = ax.hypot(ay);
            wheel.aim = if length > 1.0 {
                (ax / length, ay / length)
            } else {
                (ax, ay)
            };
            return true;
        }
        self.emotes.playing.is_some()
    }

    fn start_emote(&mut self, emote: Emote) {
        self.stop_emote();
        self.emotes.look = 0.0;
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let feet = [p.position.x, p.position.y, p.position.z];
        let yaw = f64::from(self.yaw).to_radians();
        if !emote.has_lens() {
            // A pose and particles where the player stands, facing the way they look.
            let camera_before = self.camera_type;
            self.camera_type = CameraType::ThirdPersonBack;
            self.emotes.playing = Some(Playing {
                emote,
                started: Instant::now(),
                ticks: 0,
                anchor: feet,
                feet,
                orbit_angle: yaw,
                camera_before,
                offset: [0.0; 3],
                body_yaw: self.yaw,
                head_yaw: self.yaw,
                last: 0.0,
                song: None,
            });
            self.share_emote(Some(emote));
            return;
        }
        if emote == Emote::FalseDreams {
            let top = self.dream_height(feet);
            let camera_before = self.camera_type;
            self.camera_type = CameraType::ThirdPersonBack;
            let body = self.player_model.as_ref().map_or(self.yaw, |m| m.body_yaw);
            self.emotes.playing = Some(Playing {
                emote,
                started: Instant::now(),
                ticks: 0,
                anchor: [feet[0], feet[1] + top, feet[2]],
                feet,
                orbit_angle: yaw,
                camera_before,
                offset: [0.0; 3],
                body_yaw: body,
                head_yaw: self.yaw,
                last: 0.0,
                song: None,
            });
            self.share_emote(Some(emote));
            dream_song();
            return;
        }
        if emote == Emote::TheRook {
            let camera_before = self.camera_type;
            self.camera_type = CameraType::ThirdPersonBack;
            // It lands where the player looks.
            let ground = self.rook_spot(feet, yaw);
            self.emotes.playing = Some(Playing {
                emote,
                started: Instant::now(),
                ticks: 0,
                anchor: ground,
                feet,
                orbit_angle: yaw,
                camera_before,
                offset: [0.0; 3],
                body_yaw: self.yaw,
                head_yaw: self.yaw,
                last: 0.0,
                song: None,
            });
            self.share_emote(Some(emote));
            self.audio.play(
                "minecraft:entity.evoker.prepare_summon",
                Some(feet),
                0.8,
                1.2,
            );
            return;
        }
        if emote == Emote::PurpleHollow {
            let camera_before = self.camera_type;
            self.camera_type = CameraType::ThirdPersonBack;
            // The player turns to face the way they look: that is where it is fired.
            self.emotes.playing = Some(Playing {
                emote,
                started: Instant::now(),
                ticks: 0,
                anchor: [feet[0], feet[1] + 1.5, feet[2]],
                feet,
                orbit_angle: yaw,
                camera_before,
                offset: [0.0; 3],
                body_yaw: self.yaw,
                head_yaw: self.yaw,
                last: 0.0,
                song: None,
            });
            self.share_emote(Some(emote));
            let at = Some(hollow_at(0.0, feet, yaw).blue);
            self.audio
                .play("minecraft:block.beacon.power_select", at, 0.8, 0.7);
            return;
        }
        if emote == Emote::SupernovaCrown {
            let anchor = [feet[0], feet[1] + NOVA_HEIGHT, feet[2]];
            let camera_before = self.camera_type;
            self.camera_type = CameraType::ThirdPersonBack;
            let body = self.player_model.as_ref().map_or(self.yaw, |m| m.body_yaw);
            self.emotes.playing = Some(Playing {
                emote,
                started: Instant::now(),
                ticks: 0,
                anchor,
                feet,
                orbit_angle: yaw,
                camera_before,
                offset: [0.0; 3],
                body_yaw: body,
                head_yaw: self.yaw,
                last: 0.0,
                song: None,
            });
            self.share_emote(Some(emote));
            let at = Some(anchor);
            self.audio
                .play("minecraft:block.beacon.power_select", at, 0.9, 0.5);
            self.audio
                .play("minecraft:block.respawn_anchor.charge", at, 0.6, 1.4);
            return;
        }
        // The camera starts behind the player, looking the way they look.
        let forward = (-yaw.sin(), yaw.cos());
        // The hole opens in open air: higher where the player stands under a hill or a tree.
        let anchor = (0..=8)
            .map(|lift| {
                [
                    feet[0] + forward.0 * 1.2,
                    feet[1] + HOVER + f64::from(lift),
                    feet[2] + forward.1 * 1.2,
                ]
            })
            .find(|&at| self.open_around(at, 2.5))
            .unwrap_or([feet[0], feet[1] + HOVER + 8.0, feet[2]]);
        let orbit_angle = (feet[2] - anchor[2]).atan2(feet[0] - anchor[0]);
        let camera_before = self.camera_type;
        self.camera_type = CameraType::ThirdPersonBack;
        let body = self.player_model.as_ref().map_or(self.yaw, |m| m.body_yaw);
        self.emotes.playing = Some(Playing {
            emote,
            started: Instant::now(),
            ticks: 0,
            anchor,
            feet,
            orbit_angle,
            camera_before,
            offset: [0.0; 3],
            body_yaw: body,
            head_yaw: self.yaw,
            last: 0.0,
            song: None,
        });
        self.share_emote(Some(emote));
        let at = Some(anchor);
        self.audio
            .play("minecraft:block.beacon.activate", at, 0.8, 0.6);
        self.audio
            .play("minecraft:block.amethyst_block.resonate", at, 0.7, 0.7);
    }

    /// Tells other Wisp players (through the service) the emote this player started, or that
    /// it stopped, when sharing emotes is on.
    fn share_emote(&self, emote: Option<Emote>) {
        if let Some(sharing) = &self.cosmetics.sharing
            && self.settings.extras.emotes_shared
        {
            sharing.emote(emote.map(Emote::id));
        }
    }

    /// Once a tick: plays the emotes of the other players in view, as the service last
    /// answered, at their places; tells the sharing thread who is in view.
    pub(super) fn tick_remote_emotes(&mut self) {
        let Some(sharing) = &self.cosmetics.sharing else {
            return;
        };
        let camera = self.player.as_ref().map(|p| p.position);
        let mut near: Vec<(u128, u32, [f64; 3])> = Vec::new();
        for m in self.mobs.iter() {
            if &*m.kind.id != "minecraft:player" {
                continue;
            }
            let Some(uuid) = self.remote_player_uuid(m.id) else {
                continue;
            };
            // Where the server has them: a floating model was moved by its emote.
            let o = self.emotes.remote.get(&uuid).map_or([0.0; 3], |r| r.offset);
            let p = m.position;
            let p = [p.x - o[0], p.y - o[1], p.z - o[2]];
            if camera.is_some_and(|c| (c.x - p[0]).hypot(c.z - p[2]) > 64.0) {
                continue;
            }
            near.push((uuid, m.id, p));
        }
        sharing.asking(near.iter().map(|(u, _, _)| *u));
        let playing = sharing.playing();
        // New emotes start where the player stands; ended ones and players gone are dropped,
        // and their models set back where the server has them.
        let ended: Vec<u128> = self
            .emotes
            .remote
            .iter()
            .filter(|(uuid, r)| {
                !playing
                    .get(uuid)
                    .is_some_and(|p| Emote::from_id(&p.id) == Some(r.emote))
                    || r.started.elapsed().as_secs_f64() >= r.emote.length()
                    || !near.iter().any(|(u, _, _)| u == *uuid)
            })
            .map(|(uuid, _)| *uuid)
            .collect();
        for uuid in ended {
            let Some(r) = self.emotes.remote.remove(&uuid) else {
                continue;
            };
            if let Some(song) = r.song {
                self.audio.stop_samples(song);
            }
            if let Some(m) = self.mobs.iter_mut().find(|m| m.id == r.entity) {
                let o = r.offset;
                m.position = Vec3::new(
                    m.position.x - o[0],
                    m.position.y - o[1],
                    m.position.z - o[2],
                );
                m.previous = Vec3::new(
                    m.previous.x - o[0],
                    m.previous.y - o[1],
                    m.previous.z - o[2],
                );
                m.extra.emote_tilt = [0.0; 2];
                m.extra.previous_emote_tilt = [0.0; 2];
                m.extra.emote_arms = Default::default();
                m.extra.previous_emote_arms = Default::default();
                m.extra.emote_legs = Default::default();
                m.extra.previous_emote_legs = Default::default();
                m.extra.crouching = false;
            }
        }
        for (uuid, entity, feet) in &near {
            let Some(p) = playing.get(uuid) else {
                continue;
            };
            let Some(emote) = Emote::from_id(&p.id) else {
                continue;
            };
            if self.emotes.remote.contains_key(uuid)
                || p.started.elapsed().as_secs_f64() >= emote.length()
            {
                continue;
            }
            let body_yaw = self
                .mobs
                .iter()
                .find(|m| m.id == *entity)
                .map_or(0.0, |m| m.body_yaw);
            let anchor = match emote {
                Emote::BlackHole => [feet[0], feet[1] + HOVER, feet[2]],
                Emote::SupernovaCrown => [feet[0], feet[1] + NOVA_HEIGHT, feet[2]],
                Emote::PurpleHollow => [feet[0], feet[1] + 1.5, feet[2]],
                Emote::FalseDreams => [feet[0], feet[1] + self.dream_height(*feet), feet[2]],
                Emote::TheRook => self.rook_spot(*feet, f64::from(body_yaw).to_radians()),
                Emote::DarkAscension
                | Emote::Bruh
                | Emote::LDance
                | Emote::CelestialWings
                | Emote::Sit
                | Emote::CrouchSpam
                | Emote::SkillIssue
                | Emote::TooEasy
                | Emote::PointAndLaugh
                | Emote::CryAboutIt => *feet,
            };
            self.emotes.remote.insert(
                *uuid,
                Remote {
                    emote,
                    started: p.started,
                    anchor,
                    feet: *feet,
                    ticks: 0,
                    last: p.started.elapsed().as_secs_f64(),
                    entity: *entity,
                    offset: [0.0; 3],
                    body_yaw,
                    yaw: f64::from(body_yaw).to_radians(),
                    song: None,
                },
            );
        }
        // Their models float as the player's own does on their screen.
        for r in self.emotes.remote.values_mut() {
            let Some(m) = self.mobs.iter_mut().find(|m| m.id == r.entity) else {
                continue;
            };
            let t = r.started.elapsed().as_secs_f64();
            let server = [
                m.position.x - r.offset[0],
                m.position.y - r.offset[1],
                m.position.z - r.offset[2],
            ];
            let (at, tilt, along) = match r.emote {
                Emote::BlackHole => {
                    let (at, angle) = orbit_from(r.anchor, r.feet, 0.0, t);
                    (
                        at,
                        floating_tilt(t),
                        (rise(t) > 0.05).then(|| yaw_of(-angle.sin(), angle.cos())),
                    )
                }
                Emote::SupernovaCrown => (
                    [server[0], server[1] + nova_lift(t), server[2]],
                    nova_tilt(t),
                    None,
                ),
                Emote::PurpleHollow => (server, hollow_tilt(t), None),
                Emote::TheRook => {
                    m.extra.previous_emote_arms = m.extra.emote_arms;
                    m.extra.emote_arms = rook_arms(t);
                    m.previous_body_yaw = m.body_yaw;
                    m.body_yaw = r.body_yaw;
                    (server, rook_tilt(t), None)
                }
                Emote::FalseDreams => {
                    let d = dream_at(t, r.anchor[1] - r.feet[1]);
                    m.extra.previous_emote_arms = m.extra.emote_arms;
                    m.extra.emote_arms = d.arms;
                    m.previous_body_yaw = m.body_yaw;
                    m.body_yaw = r.body_yaw;
                    m.head_yaw = r.body_yaw + d.head_turn;
                    m.head_pitch = d.head_pitch;
                    ([server[0], server[1] + d.lift, server[2]], d.tilt, None)
                }
                e @ (Emote::DarkAscension
                | Emote::Bruh
                | Emote::LDance
                | Emote::CelestialWings
                | Emote::Sit
                | Emote::CrouchSpam
                | Emote::SkillIssue
                | Emote::TooEasy
                | Emote::PointAndLaugh
                | Emote::CryAboutIt) => {
                    let mv = moves::moves_at(e, t).unwrap_or_default();
                    m.extra.previous_emote_arms = m.extra.emote_arms;
                    m.extra.emote_arms = mv.arms;
                    m.extra.previous_emote_legs = m.extra.emote_legs;
                    m.extra.emote_legs = mv.leg_pose;
                    m.extra.crouching = mv.crouch;
                    m.previous_body_yaw = m.body_yaw;
                    m.body_yaw = r.body_yaw + mv.turn;
                    m.head_yaw = m.body_yaw + mv.head_turn;
                    m.head_pitch = mv.head_pitch;
                    if mv.legs > 0.0 {
                        m.previous_limb_speed = m.limb_speed;
                        m.limb_speed = mv.legs;
                        m.limb_position += moves::STRIDE;
                    }
                    ([server[0], server[1] + mv.lift, server[2]], mv.tilt, None)
                }
            };
            if r.emote == Emote::PurpleHollow {
                m.extra.previous_emote_arms = m.extra.emote_arms;
                m.extra.emote_arms = hollow_arms(t);
                m.previous_body_yaw = m.body_yaw;
                m.body_yaw = r.body_yaw;
            }
            let offset = [at[0] - server[0], at[1] - server[1], at[2] - server[2]];
            m.position = Vec3::new(at[0], at[1], at[2]);
            m.previous = Vec3::new(
                m.previous.x - r.offset[0] + offset[0],
                m.previous.y - r.offset[1] + offset[1],
                m.previous.z - r.offset[2] + offset[2],
            );
            r.offset = offset;
            m.extra.previous_emote_tilt = m.extra.emote_tilt;
            m.extra.emote_tilt = tilt;
            if let Some(along) = along {
                let drift = (35.0 * (t * 0.3).sin() + 12.0 * (t * 0.13).cos()) as f32;
                r.body_yaw += wrap(along + drift - r.body_yaw) * 0.15;
                m.previous_body_yaw = m.body_yaw;
                m.body_yaw = r.body_yaw;
                m.head_yaw = r.body_yaw + (20.0 * (t * 0.5).sin()) as f32;
            }
        }
        // Their song, heard from where they float.
        for r in self.emotes.remote.values_mut() {
            if r.emote != Emote::FalseDreams {
                continue;
            }
            let t = r.started.elapsed().as_secs_f64();
            let at = [r.feet[0], r.feet[1] + 1.0, r.feet[2]];
            let volume = 0.8 * dream_volume(t);
            match r.song {
                Some(song) => self.audio.set_samples_volume(song, volume),
                None => {
                    if let Some((samples, rate)) = dream_song() {
                        r.song = self.audio.play_samples(
                            samples,
                            rate,
                            t,
                            Some(at),
                            DREAMS_HEARD,
                            volume,
                        );
                    }
                }
            }
        }
        let mut runs = Vec::new();
        for r in self.emotes.remote.values_mut() {
            r.ticks += 1;
            let t = r.started.elapsed().as_secs_f64();
            let previous = std::mem::replace(&mut r.last, t);
            runs.push((r.emote, previous, t, r.ticks, r.anchor, r.feet, r.yaw));
        }
        for (emote, previous, t, ticks, anchor, feet, yaw) in runs {
            match emote {
                Emote::BlackHole => {
                    let body = [feet[0], feet[1] + 1.0, feet[2]];
                    self.black_hole_effects(previous, t, ticks, anchor, body);
                }
                Emote::SupernovaCrown => {
                    let crown = [feet[0], feet[1] + CROWN_HEIGHT + nova_lift(t), feet[2]];
                    self.supernova_effects(previous, t, ticks, anchor, crown);
                }
                Emote::PurpleHollow => self.hollow_effects(previous, t, ticks, feet, yaw),
                Emote::FalseDreams => self.dreams_effects(previous, t, feet, anchor[1] - feet[1]),
                Emote::TheRook => self.rook_effects(previous, t, ticks, anchor, yaw),
                Emote::DarkAscension
                | Emote::Bruh
                | Emote::LDance
                | Emote::CelestialWings
                | Emote::Sit
                | Emote::CrouchSpam
                | Emote::SkillIssue
                | Emote::TooEasy
                | Emote::PointAndLaugh
                | Emote::CryAboutIt => {
                    self.moves_effects(emote, previous, t, ticks, feet, yaw);
                }
            }
        }
    }

    /// The lens of the nearest other player's emote, when this player plays none.
    fn remote_lens(&self) -> Option<BlackHole> {
        let camera = self.player.as_ref().map(|p| p.position)?;
        let r = self
            .emotes
            .remote
            .values()
            .filter(|r| r.emote.has_lens())
            .min_by(|a, b| {
                let d = |r: &Remote| (r.anchor[0] - camera.x).hypot(r.anchor[2] - camera.z);
                d(a).total_cmp(&d(b))
            })?;
        lens_at(
            r.emote,
            r.started.elapsed().as_secs_f64(),
            r.anchor,
            r.feet,
            r.yaw,
        )
    }

    fn stop_emote(&mut self) {
        if self.emotes.playing.is_some() {
            self.share_emote(None);
        }
        if let Some(p) = self.emotes.playing.take() {
            self.camera_type = p.camera_before;
            if let Some(song) = p.song {
                self.audio.stop_samples(song);
            }
            if let Some(m) = self.player_model.as_mut() {
                m.extra.emote_tilt = [0.0; 2];
                m.extra.previous_emote_tilt = [0.0; 2];
                m.extra.emote_arms = Default::default();
                m.extra.previous_emote_arms = Default::default();
                m.extra.emote_legs = Default::default();
                m.extra.previous_emote_legs = Default::default();
                m.extra.crouching = false;
            }
        }
    }

    /// Once a tick, after the player's model: the emote's particles and sounds, and the
    /// model floating round the hole.
    pub(super) fn tick_emote(&mut self) {
        let Some(p) = self.emotes.playing.as_mut() else {
            return;
        };
        let t = p.time();
        if t >= p.emote.length() || self.player.is_none() {
            self.stop_emote();
            return;
        }
        p.ticks += 1;
        let ticks = p.ticks;
        let previous = std::mem::replace(&mut p.last, t);
        let anchor = p.anchor;
        if !p.emote.has_lens() {
            let (emote, feet, yaw) = (p.emote, p.feet, p.orbit_angle);
            self.moves_pose(t);
            self.moves_effects(emote, previous, t, ticks, feet, yaw);
            return;
        }
        if p.emote == Emote::FalseDreams {
            let (feet, top) = (p.feet, p.anchor[1] - p.feet[1]);
            let volume = dream_volume(t);
            match p.song {
                Some(song) => self.audio.set_samples_volume(song, volume),
                None => {
                    if let Some((samples, rate)) = dream_song() {
                        p.song = self.audio.play_samples(samples, rate, t, None, 0.0, volume);
                    }
                }
            }
            self.dreams_pose(t);
            self.dreams_effects(previous, t, feet, top);
            return;
        }
        if p.emote == Emote::TheRook {
            let yaw = p.orbit_angle;
            self.rook_pose(t);
            self.rook_effects(previous, t, ticks, anchor, yaw);
            return;
        }
        if p.emote == Emote::PurpleHollow {
            let (feet, yaw) = (p.feet, p.orbit_angle);
            self.hollow_pose(t);
            self.hollow_effects(previous, t, ticks, feet, yaw);
            return;
        }
        if p.emote == Emote::SupernovaCrown {
            let crown = [
                p.feet[0],
                p.feet[1] + CROWN_HEIGHT + nova_lift(t),
                p.feet[2],
            ];
            self.supernova_pose(t);
            self.supernova_effects(previous, t, ticks, anchor, crown);
            return;
        }
        let (feet, angle) = orbit(p, t);
        let real = self.player.as_ref().map(|pl| pl.position);
        if let (Some(real), Some(m)) = (real, self.player_model.as_mut()) {
            let offset = [feet[0] - real.x, feet[1] - real.y, feet[2] - real.z];
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
            // Weightless: lying back along the orbit, slowly turning, rolling and swaying, the
            // limbs drifting as if in water.
            let lie = lying(t);
            m.extra.previous_emote_tilt = m.extra.emote_tilt;
            m.extra.emote_tilt = [
                ((82.0 + 8.0 * (t * 0.55).sin()) * lie) as f32,
                (16.0 * (t * 0.4).sin() * lie) as f32,
            ];
            if rise(t) > 0.05 {
                let along = yaw_of(-angle.sin(), angle.cos());
                let drift = (35.0 * (t * 0.3).sin() + 12.0 * (t * 0.13).cos()) as f32;
                let target = along + drift;
                p.body_yaw += wrap(target - p.body_yaw) * 0.15;
                let head = p.body_yaw + (20.0 * (t * 0.5).sin()) as f32;
                p.head_yaw += wrap(head - p.head_yaw) * 0.15;
                m.previous_body_yaw = m.body_yaw;
                m.body_yaw = p.body_yaw;
                m.yaw = p.head_yaw;
                m.head_yaw = p.head_yaw;
                m.head_pitch = (-10.0 + 12.0 * (t * 0.37).sin()) as f32;
                m.previous_limb_speed = m.limb_speed;
                m.limb_speed = (0.35 * lie) as f32;
                m.limb_position += 0.12;
            }
        }
        let body = [feet[0], feet[1] + 1.0, feet[2]];
        if p.emote == Emote::BlackHole {
            self.black_hole_effects(previous, t, ticks, anchor, body);
        }
    }

    /// How high False Dreams can float a player standing at `feet`: as high as it likes under
    /// open sky, lower under a roof, not at all in a tunnel.
    fn dream_height(&self, feet: [f64; 3]) -> f64 {
        [DREAMS_TOP, 1.2, 0.8, 0.4]
            .into_iter()
            .find(|lift| {
                [0.3, 1.2, 2.1]
                    .iter()
                    .all(|up| self.air([feet[0], feet[1] + lift + up, feet[2]]))
            })
            .unwrap_or(0.0)
    }

    /// False Dreams' pose: floating up where the emote started, head thrown back, arms hanging,
    /// now and then jerking as if pulled.
    fn dreams_pose(&mut self, t: f64) {
        let Some(p) = self.emotes.playing.as_mut() else {
            return;
        };
        let d = dream_at(t, p.anchor[1] - p.feet[1]);
        let real = self.player.as_ref().map(|pl| pl.position);
        let (Some(real), Some(m)) = (real, self.player_model.as_mut()) else {
            return;
        };
        let offset = [
            p.feet[0] - real.x,
            p.feet[1] + d.lift - real.y,
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
        m.extra.emote_tilt = d.tilt;
        m.extra.previous_emote_arms = m.extra.emote_arms;
        m.extra.emote_arms = d.arms;
        m.previous_body_yaw = m.body_yaw;
        m.body_yaw = p.body_yaw;
        m.yaw = p.body_yaw + d.head_turn;
        m.head_yaw = p.body_yaw + d.head_turn;
        m.head_pitch = d.head_pitch;
        m.previous_limb_speed = m.limb_speed;
        m.limb_speed = 0.0;
    }

    /// False Dreams' particles and sounds (the eyes and the blur are the shader's, the song
    /// plays apart): dark motes drifting up round the floating player, dust kicked up as they
    /// leave the ground, a heartbeat as it takes them and as it lets them go.
    fn dreams_effects(&mut self, previous: f64, t: f64, feet: [f64; 3], top: f64) {
        let lift = dream_at(t, top).lift;
        let body = [feet[0], feet[1] + lift + 1.0, feet[2]];
        let particles = &mut self.particles;
        let motes = if dream_float(t) > 0.1 { 2 } else { 0 };
        for _ in 0..motes {
            let r = particles.random();
            let d = direction(r);
            let far = 0.8 + r.next_double() * 2.6;
            let at = [
                body[0] + d[0] * far,
                body[1] + d[1] * far * 0.8,
                body[2] + d[2] * far,
            ];
            let scale = 0.5 + r.next_float() * 0.6;
            let up = 0.008 + r.next_double() * 0.014;
            particles.add(
                &ParticleOptions::dust_transition([0.2, 0.15, 0.18], [0.04, 0.02, 0.04], scale),
                at,
                [0.0, up, 0.0],
            );
        }
        if previous < 0.6 && t >= 0.6 {
            for k in 0..24 {
                let r = particles.random();
                let a = f64::from(k) * TAU / 24.0;
                let s = 0.05 + r.next_double() * 0.05;
                particles.add(
                    &ParticleOptions::dust_transition([0.3, 0.26, 0.28], [0.08, 0.06, 0.08], 1.1),
                    [
                        feet[0] + a.cos() * 0.4,
                        feet[1] + 0.1,
                        feet[2] + a.sin() * 0.4,
                    ],
                    [a.cos() * s, 0.02, a.sin() * s],
                );
            }
        }
        let audio = &mut self.audio;
        if previous < 0.4 && t >= 0.4 {
            audio.play("minecraft:entity.warden.heartbeat", Some(body), 1.0, 0.7);
        }
        if previous < DREAMS_LANDED && t >= DREAMS_LANDED {
            audio.play("minecraft:entity.warden.heartbeat", Some(body), 0.8, 0.6);
        }
    }

    /// The Purple Hollow's pose: standing where the emote started, facing the way it is fired,
    /// each hand lifting its orb, both brought together, the right arm thrown out as it fires;
    /// the head follows the orbs, then the sphere.
    fn hollow_pose(&mut self, t: f64) {
        let Some(p) = self.emotes.playing.as_mut() else {
            return;
        };
        let real = self.player.as_ref().map(|pl| pl.position);
        let (Some(real), Some(m)) = (real, self.player_model.as_mut()) else {
            return;
        };
        let offset = [p.feet[0] - real.x, p.feet[1] - real.y, p.feet[2] - real.z];
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
        m.extra.emote_tilt = hollow_tilt(t);
        m.extra.previous_emote_arms = m.extra.emote_arms;
        m.extra.emote_arms = hollow_arms(t);
        m.previous_body_yaw = m.body_yaw;
        m.body_yaw = p.body_yaw;
        // A glance at the blue orb, then the red one, then straight at where they meet.
        let blue = smoothstep(HOLLOW_BLUE, HOLLOW_BLUE + 0.8, t)
            * (1.0 - smoothstep(HOLLOW_RED, HOLLOW_RED + 0.8, t));
        let red = smoothstep(HOLLOW_RED, HOLLOW_RED + 0.8, t)
            * (1.0 - smoothstep(HOLLOW_JOIN - 0.4, HOLLOW_JOIN + 0.6, t));
        let head = p.body_yaw + (38.0 * blue - 38.0 * red) as f32;
        m.yaw = head;
        m.head_yaw = head;
        m.head_pitch =
            (-6.0 * (blue + red) + 4.0 * smoothstep(HOLLOW_JOIN, HOLLOW_MERGE, t)) as f32;
        m.previous_limb_speed = m.limb_speed;
        m.limb_speed = 0.0;
    }

    /// The Purple Hollow's particles and sounds (the orbs, the sphere and its wake are the
    /// shader's): air streaming into the blue orb and sparks thrown off the red one, light
    /// arcing between them as they close, matter torn into the sphere as it charges, and what
    /// it rips loose along its way once fired.
    fn hollow_effects(&mut self, previous: f64, t: f64, ticks: u32, feet: [f64; 3], yaw: f64) {
        let h = hollow_at(t, feet, yaw);
        let particles = &mut self.particles;
        let trail = |colour: [f32; 3], to: [f64; 3], ticks: i32| ParticleOptions {
            color: [colour[0], colour[1], colour[2], 1.0],
            trail: Some((to, ticks)),
            ..ParticleOptions::simple(ParticleType::Trail)
        };
        if h.orbs[0] > 0.02 && t < HOLLOW_MERGE {
            // The blue orb pulls: streaks of air falling into it from all round.
            for _ in 0..2 {
                let r = particles.random();
                let d = direction(r);
                let far = 1.2 + r.next_double() * 1.6;
                let from = [
                    h.blue[0] + d[0] * far,
                    h.blue[1] + d[1] * far,
                    h.blue[2] + d[2] * far,
                ];
                let colour = if r.next_int_bound(3).unwrap_or(0) == 0 {
                    WHITE
                } else {
                    HOLLOW_BLUE_LIGHT
                };
                let ticks = (8.0 + r.next_double() * 6.0) as i32;
                particles.add_glowing(&trail(colour, h.blue, ticks), from, [0.0; 3]);
            }
        }
        if h.orbs[1] > 0.02 && t < HOLLOW_MERGE {
            // The red one pushes: sparks thrown off it, hard.
            for _ in 0..2 {
                let r = particles.random();
                let d = direction(r);
                let s = 0.12 + r.next_double() * 0.14;
                let from = [
                    h.red[0] + d[0] * h.orbs[1],
                    h.red[1] + d[1] * h.orbs[1],
                    h.red[2] + d[2] * h.orbs[1],
                ];
                let scale = 0.5 + r.next_float() * 0.4;
                particles.add_glowing(
                    &ParticleOptions::dust_transition(HOLLOW_RED_LIGHT, ORANGE, scale),
                    from,
                    [d[0] * s, d[1] * s, d[2] * s],
                );
            }
        }
        if (HOLLOW_JOIN..HOLLOW_MERGE).contains(&t) {
            // Closing on each other, light arcs across from one to the other.
            let r = particles.random();
            let (from, to, colour) = if r.next_int_bound(2).unwrap_or(0) == 0 {
                (h.blue, h.red, HOLLOW_BLUE_LIGHT)
            } else {
                (h.red, h.blue, HOLLOW_RED_LIGHT)
            };
            let bend = direction(r);
            let from = [
                from[0] + bend[0] * 0.2,
                from[1] + bend[1] * 0.2,
                from[2] + bend[2] * 0.2,
            ];
            particles.add_glowing(&trail(colour, to, 4), from, [0.0; 3]);
        }
        if previous < HOLLOW_MERGE && t >= HOLLOW_MERGE {
            // They meet: a ring of violet light thrown out sideways.
            for k in 0..36 {
                let a = f64::from(k) * TAU / 36.0;
                let out = [
                    -h.forward[1] * a.cos() * 1.6,
                    a.sin() * 1.6,
                    h.forward[0] * a.cos() * 1.6,
                ];
                let to = [
                    h.sphere[0] + out[0],
                    h.sphere[1] + out[1],
                    h.sphere[2] + out[2],
                ];
                let colour = if k % 3 == 0 { WHITE } else { VIOLET };
                particles.add_glowing(&trail(colour, to, 7 + k % 4), h.sphere, [0.0; 3]);
            }
        }
        if (HOLLOW_MERGE..HOLLOW_FIRE).contains(&t) {
            // Charging: everything near is torn into the sphere, faster and faster.
            let charge = smoothstep(HOLLOW_MERGE, HOLLOW_FIRE, t);
            for _ in 0..(2.0 + 5.0 * charge) as usize {
                let r = particles.random();
                let d = direction(r);
                let far = 1.5 + r.next_double() * 2.5;
                let from = [
                    h.sphere[0] + d[0] * far,
                    h.sphere[1] + d[1] * far * 0.8,
                    h.sphere[2] + d[2] * far,
                ];
                let colour = match r.next_int_bound(4).unwrap_or(0) {
                    0 => HOLLOW_BLUE_LIGHT,
                    1 => HOLLOW_RED_LIGHT,
                    2 => WHITE,
                    _ => VIOLET,
                };
                let ticks = (12.0 - 7.0 * charge) as i32;
                particles.add_glowing(&trail(colour, h.sphere, ticks.max(4)), from, [0.0; 3]);
            }
        }
        if previous < HOLLOW_FIRE && t >= HOLLOW_FIRE {
            // Fired: the air it leaves is blown back and out.
            for i in 0..70 {
                let r = particles.random();
                let d = direction(r);
                let far = 1.5 + r.next_double() * 2.0;
                let to = [
                    h.origin[0] + d[0] * far - h.forward[0] * 1.2,
                    h.origin[1] + d[1] * far * 0.7,
                    h.origin[2] + d[2] * far - h.forward[1] * 1.2,
                ];
                let colour = if i % 3 == 0 { WHITE } else { VIOLET };
                let ticks = (7.0 + r.next_double() * 8.0) as i32;
                particles.add_glowing(&trail(colour, to, ticks), h.origin, [0.0; 3]);
            }
        }
        if t >= HOLLOW_FIRE && h.radius > 0.05 {
            // On its way: violet dust ripped loose round it and left hanging in its wake.
            for _ in 0..6 {
                let r = particles.random();
                let d = direction(r);
                let pos = [
                    h.sphere[0] + d[0] * h.radius,
                    h.sphere[1] + d[1] * h.radius,
                    h.sphere[2] + d[2] * h.radius,
                ];
                let scale = 0.8 + r.next_float() * 0.9;
                particles.add_glowing(
                    &ParticleOptions::dust_transition(VIOLET, WHITE, scale),
                    pos,
                    [d[0] * 0.05, d[1] * 0.05 + 0.01, d[2] * 0.05],
                );
            }
        }
        // Sounds.
        let audio = &mut self.audio;
        let (blue, red, sphere) = (Some(h.blue), Some(h.red), Some(h.sphere));
        if t > HOLLOW_BLUE && t < HOLLOW_MERGE && ticks.is_multiple_of(14) {
            audio.play("minecraft:block.beacon.ambient", blue, 0.7, 1.5);
        }
        if previous < HOLLOW_RED && t >= HOLLOW_RED {
            audio.play("minecraft:block.respawn_anchor.charge", red, 0.8, 0.8);
        }
        if t > HOLLOW_RED && t < HOLLOW_MERGE && (ticks + 7).is_multiple_of(14) {
            audio.play("minecraft:block.fire.ambient", red, 0.9, 0.6);
        }
        if previous < HOLLOW_JOIN && t >= HOLLOW_JOIN {
            audio.play("minecraft:entity.warden.sonic_charge", sphere, 0.9, 0.9);
        }
        if previous < HOLLOW_MERGE && t >= HOLLOW_MERGE {
            audio.play("minecraft:block.end_portal.spawn", sphere, 0.6, 1.7);
            audio.play("minecraft:block.conduit.activate", sphere, 1.0, 0.6);
        }
        if (HOLLOW_MERGE..HOLLOW_FIRE).contains(&t) && ticks.is_multiple_of(6) {
            let rise = 0.6 + 1.3 * smoothstep(HOLLOW_MERGE, HOLLOW_FIRE, t);
            audio.play(
                "minecraft:block.amethyst_block.resonate",
                sphere,
                0.7,
                rise as f32,
            );
        }
        if previous < HOLLOW_FIRE && t >= HOLLOW_FIRE {
            let from = Some(h.origin);
            audio.play("minecraft:entity.warden.sonic_boom", from, 1.3, 0.7);
            audio.play("minecraft:entity.generic.explode", from, 1.0, 0.5);
            audio.play("minecraft:entity.lightning_bolt.thunder", from, 0.8, 1.3);
        }
        if previous < HOLLOW_AWAY && t >= HOLLOW_AWAY {
            audio.play(
                "minecraft:block.beacon.deactivate",
                Some(h.origin),
                0.7,
                0.8,
            );
        }
    }

    /// The Rook's pose: standing where it started, facing the rook, calling it down, watching
    /// it fall, pointing at it, rocked back by its blast.
    fn rook_pose(&mut self, t: f64) {
        let Some(p) = self.emotes.playing.as_mut() else {
            return;
        };
        let real = self.player.as_ref().map(|pl| pl.position);
        let (Some(real), Some(m)) = (real, self.player_model.as_mut()) else {
            return;
        };
        let offset = [p.feet[0] - real.x, p.feet[1] - real.y, p.feet[2] - real.z];
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
        m.extra.emote_tilt = rook_tilt(t);
        m.extra.previous_emote_arms = m.extra.emote_arms;
        m.extra.emote_arms = rook_arms(t);
        m.previous_body_yaw = m.body_yaw;
        m.body_yaw = p.body_yaw;
        m.yaw = p.body_yaw;
        m.head_yaw = p.body_yaw;
        // Up at the sky as it falls, down at it once landed, up at the badge.
        let sky =
            smoothstep(0.2, 0.7, t) * (1.0 - smoothstep(ROOK_LAND - 0.1, ROOK_LAND + 0.25, t));
        let badge = smoothstep(ROOK_BADGE, ROOK_BADGE + 0.4, t)
            * (1.0 - smoothstep(ROOK_BLAST - 0.3, ROOK_BLAST, t));
        m.head_pitch = (-40.0 * sky - 22.0 * badge) as f32;
        m.previous_limb_speed = m.limb_speed;
        m.limb_speed = 0.0;
    }

    /// The Rook's particles and sounds (the rook, its badge and the blast's fire are the
    /// shader's): a streak behind it as it falls, dust thrown out as it lands, sparkles as the
    /// badge pops up, a fuse, the explosion thrown forward, and teal sparks as it crumbles.
    fn rook_effects(&mut self, previous: f64, t: f64, ticks: u32, ground: [f64; 3], yaw: f64) {
        let r = rook_at(t, ground, yaw);
        let crossed = |at: f64| previous < at && t >= at;
        let f = r.forward;
        let particles = &mut self.particles;
        if (ROOK_DROP..ROOK_LAND).contains(&t) {
            // A bright streak above it as it falls.
            for _ in 0..4 {
                let rr = particles.random();
                let d = direction(rr);
                let pos = [
                    r.base[0] + d[0] * 0.5,
                    r.base[1] + ROOK_HEIGHT * (0.5 + rr.next_double()) + 1.0,
                    r.base[2] + d[2] * 0.5,
                ];
                let colour = if rr.next_int_bound(3).unwrap_or(0) == 0 {
                    TEAL
                } else {
                    WHITE
                };
                particles.add_glowing(
                    &ParticleOptions::dust_transition(colour, WHITE, 1.2),
                    pos,
                    [0.0, 0.3, 0.0],
                );
            }
        }
        if crossed(ROOK_LAND) {
            // It lands: dust and smoke thrown out in a ring.
            for k in 0..48 {
                let a = f64::from(k) * TAU / 48.0;
                let rr = particles.random();
                let speed = 0.25 + rr.next_double() * 0.2;
                let pos = [
                    ground[0] + a.cos() * 0.9,
                    ground[1] + 0.1,
                    ground[2] + a.sin() * 0.9,
                ];
                let ty = if k % 3 == 0 {
                    ParticleType::LargeSmoke
                } else {
                    ParticleType::Cloud
                };
                particles.add(
                    &ParticleOptions::simple(ty),
                    pos,
                    [a.cos() * speed, 0.03, a.sin() * speed],
                );
            }
            for _ in 0..16 {
                let rr = particles.random();
                let d = direction(rr);
                let pos = [
                    ground[0] + d[0] * 0.6,
                    ground[1] + 0.3,
                    ground[2] + d[2] * 0.6,
                ];
                particles.add(
                    &ParticleOptions::simple(ParticleType::Crit),
                    pos,
                    [d[0] * 0.5, 0.4 + d[1].abs() * 0.3, d[2] * 0.5],
                );
            }
        }
        if crossed(ROOK_BADGE) {
            // The badge pops up: teal and white sparkles thrown off it.
            for k in 0..30 {
                let rr = particles.random();
                let d = direction(rr);
                let to = [
                    r.badge[0] + d[0] * 1.4,
                    r.badge[1] + d[1] * 1.4,
                    r.badge[2] + d[2] * 1.4,
                ];
                let colour = if k % 3 == 0 { WHITE } else { TEAL };
                let trail = ParticleOptions {
                    color: [colour[0], colour[1], colour[2], 1.0],
                    trail: Some((to, 6 + k % 5)),
                    ..ParticleOptions::simple(ParticleType::Trail)
                };
                particles.add_glowing(&trail, r.badge, [0.0; 3]);
            }
        }
        if (ROOK_BLAST - 1.0..ROOK_BLAST).contains(&t) {
            // The fuse: sparks spitting from its front, faster and faster.
            let charge = smoothstep(ROOK_BLAST - 1.0, ROOK_BLAST, t);
            for _ in 0..(1.0 + 4.0 * charge) as usize {
                let rr = particles.random();
                let d = direction(rr);
                let pos = [
                    r.origin[0] + d[0] * 0.3,
                    r.origin[1] + d[1] * 0.3,
                    r.origin[2] + d[2] * 0.3,
                ];
                particles.add_glowing(
                    &ParticleOptions::simple(ParticleType::Flame),
                    pos,
                    [d[0] * 0.04, 0.03, d[2] * 0.04],
                );
            }
        }
        if crossed(ROOK_BLAST) {
            // The blast, thrown forward.
            let along = |k: f64, up: f64| {
                [
                    r.origin[0] + f[0] * k,
                    r.origin[1] + up,
                    r.origin[2] + f[1] * k,
                ]
            };
            for k in [1.5, 4.0, 6.5] {
                particles.add(
                    &ParticleOptions::simple(ParticleType::ExplosionEmitter),
                    along(k, 0.0),
                    [0.0; 3],
                );
            }
            for _ in 0..90 {
                let rr = particles.random();
                let d = direction(rr);
                let speed = 0.5 + rr.next_double() * 0.9;
                let v = [
                    (f[0] * 1.2 + d[0] * 0.55) * speed,
                    (d[1].abs() * 0.5 + 0.08) * speed,
                    (f[1] * 1.2 + d[2] * 0.55) * speed,
                ];
                let ty = match rr.next_int_bound(5).unwrap_or(0) {
                    0 => ParticleType::Lava,
                    1 | 2 => ParticleType::LargeSmoke,
                    _ => ParticleType::Flame,
                };
                let pos = [
                    r.origin[0] + d[0] * 0.3,
                    r.origin[1] + d[1] * 0.3,
                    r.origin[2] + d[2] * 0.3,
                ];
                if ty == ParticleType::Flame {
                    particles.add_glowing(&ParticleOptions::simple(ty), pos, v);
                } else {
                    particles.add(&ParticleOptions::simple(ty), pos, v);
                }
            }
            for _ in 0..40 {
                let rr = particles.random();
                let d = direction(rr);
                let pos = [
                    r.origin[0] + d[0] * 0.4,
                    r.origin[1] + d[1] * 0.4,
                    r.origin[2] + d[2] * 0.4,
                ];
                let v = [
                    f[0] * 0.6 + d[0] * 0.3,
                    d[1].abs() * 0.3,
                    f[1] * 0.6 + d[2] * 0.3,
                ];
                particles.add_glowing(&ParticleOptions::dust_transition(FIRE, WHITE, 2.0), pos, v);
            }
        }
        if (ROOK_BLAST..ROOK_BLAST + 2.0).contains(&t) {
            // Burning and smoking along where it went.
            let left = 1.0 - (t - ROOK_BLAST) / 2.0;
            for _ in 0..(5.0 * left) as usize + 1 {
                let rr = particles.random();
                let k = 1.0 + rr.next_double() * 7.5;
                let d = direction(rr);
                let pos = [
                    r.origin[0] + f[0] * k + d[0],
                    r.origin[1] + d[1] * 0.7,
                    r.origin[2] + f[1] * k + d[2],
                ];
                let ty = if rr.next_int_bound(3).unwrap_or(0) == 0 {
                    ParticleType::Flame
                } else {
                    ParticleType::CampfireCosySmoke
                };
                particles.add(&ParticleOptions::simple(ty), pos, [0.0, 0.04, 0.0]);
            }
        }
        if (ROOK_LEAVE..ROOK_END).contains(&t) {
            // Crumbling into teal sparks that drift up.
            for _ in 0..6 {
                let rr = particles.random();
                let a = rr.next_double() * TAU;
                let up = rr.next_double() * ROOK_HEIGHT;
                let out = 0.55 + 0.25 * (1.0 - up / ROOK_HEIGHT);
                let pos = [
                    r.base[0] + a.cos() * out,
                    r.base[1] + up,
                    r.base[2] + a.sin() * out,
                ];
                let colour = if rr.next_int_bound(4).unwrap_or(0) == 0 {
                    WHITE
                } else {
                    TEAL
                };
                particles.add_glowing(
                    &ParticleOptions::dust_transition(colour, WHITE, 0.9),
                    pos,
                    [a.cos() * 0.02, 0.06, a.sin() * 0.02],
                );
            }
        }
        // Sounds.
        let audio = &mut self.audio;
        let (rook, badge, origin) = (Some(r.base), Some(r.badge), Some(r.origin));
        if crossed(ROOK_DROP) {
            audio.play("minecraft:entity.phantom.swoop", Some(ground), 1.0, 0.6);
        }
        if crossed(ROOK_LAND) {
            audio.play("minecraft:block.anvil.land", Some(ground), 1.0, 0.5);
            audio.play("minecraft:entity.generic.explode", Some(ground), 0.6, 1.5);
            audio.play("minecraft:block.stone.break", Some(ground), 1.0, 0.6);
        }
        if crossed(ROOK_BADGE) {
            audio.play("minecraft:entity.player.levelup", badge, 0.9, 1.3);
            audio.play("minecraft:block.amethyst_block.chime", badge, 1.0, 1.4);
            audio.play("minecraft:block.note_block.chime", badge, 1.0, 1.2);
        }
        if crossed(ROOK_BLAST - 1.0) {
            audio.play("minecraft:entity.tnt.primed", origin, 1.0, 1.0);
        }
        if crossed(ROOK_BLAST) {
            audio.play("minecraft:entity.generic.explode", origin, 1.4, 0.6);
            audio.play("minecraft:entity.generic.explode", origin, 1.0, 0.9);
            audio.play(
                "minecraft:entity.firework_rocket.large_blast",
                origin,
                1.0,
                0.7,
            );
            audio.play("minecraft:entity.lightning_bolt.thunder", origin, 0.6, 1.5);
        }
        if (ROOK_BLAST..ROOK_BLAST + 2.0).contains(&t) && ticks.is_multiple_of(10) {
            audio.play("minecraft:block.fire.ambient", origin, 1.0, 0.8);
        }
        if crossed(ROOK_LEAVE) {
            audio.play("minecraft:block.amethyst_cluster.break", rook, 1.0, 0.7);
            audio.play("minecraft:block.beacon.deactivate", rook, 0.5, 1.5);
        }
    }

    /// The Supernova Crown's pose: lifted a little off the ground, leaning back to watch the
    /// star, then standing tall under the crown; arms drifting out.
    fn supernova_pose(&mut self, t: f64) {
        let Some(p) = self.emotes.playing.as_mut() else {
            return;
        };
        let feet = [p.feet[0], p.feet[1] + nova_lift(t), p.feet[2]];
        let real = self.player.as_ref().map(|pl| pl.position);
        let (Some(real), Some(m)) = (real, self.player_model.as_mut()) else {
            return;
        };
        let offset = [feet[0] - real.x, feet[1] - real.y, feet[2] - real.z];
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
        // Leaning back to watch the star form, a jolt at the burst, upright under the crown.
        let watch = smoothstep(0.3, 1.8, t) * (1.0 - smoothstep(NOVA_BURST, NOVA_BURST + 1.2, t));
        let jolt = if t > NOVA_BURST {
            (-(t - NOVA_BURST) * 5.0).exp() * 14.0
        } else {
            0.0
        };
        let fade = 1.0 - smoothstep(NOVA_FADE, NOVA_END - 0.3, t);
        m.extra.previous_emote_tilt = m.extra.emote_tilt;
        m.extra.emote_tilt = [((12.0 * watch + jolt) * fade) as f32, 0.0];
        m.previous_body_yaw = m.body_yaw;
        m.body_yaw = p.body_yaw;
        m.yaw = p.body_yaw;
        m.head_yaw = p.body_yaw;
        // The head follows the star up, then looks ahead under the crown.
        let look_up =
            -55.0 * watch - 10.0 * (1.0 - watch) * smoothstep(NOVA_BURST, NOVA_BURST + 1.0, t);
        m.head_pitch = (look_up * fade) as f32;
        m.previous_limb_speed = m.limb_speed;
        m.limb_speed = (0.25 * smoothstep(0.5, 2.0, t) * fade) as f32;
        m.limb_position += 0.06;
    }

    /// The Supernova Crown's particles and sounds: matter streaming into the star, flung out
    /// when it bursts, and light falling onto the head where the crown forms (the star, its
    /// shell and the crown themselves are the shader's). `star` is where the star gathers,
    /// `crown` the middle of the crown's band.
    fn supernova_effects(
        &mut self,
        previous: f64,
        t: f64,
        ticks: u32,
        star: [f64; 3],
        crown: [f64; 3],
    ) {
        let at = Some(star);
        let particles = &mut self.particles;
        if t < NOVA_COLLAPSE {
            let gather = smoothstep(0.0, NOVA_FORMED, t);
            let swell = smoothstep(NOVA_FORMED, NOVA_COLLAPSE, t);
            // Matter streaming in from all round, faster as the star swells.
            for _ in 0..(1.0 + 3.0 * gather + 4.0 * swell) as usize {
                let r = particles.random();
                let d = direction(r);
                let far = 2.5 + r.next_double() * 2.5;
                let from = [
                    star[0] + d[0] * far,
                    star[1] + d[1] * far * 0.7,
                    star[2] + d[2] * far,
                ];
                let colour = match r.next_int_bound(3).unwrap_or(0) {
                    0 => GOLD,
                    1 => ORANGE,
                    _ => WHITE,
                };
                let duration = (18.0 - 10.0 * swell) as i32;
                particles.add_glowing(
                    &ParticleOptions {
                        color: [colour[0], colour[1], colour[2], 1.0],
                        trail: Some((star, duration.max(5))),
                        ..ParticleOptions::simple(ParticleType::Trail)
                    },
                    from,
                    [0.0; 3],
                );
            }
        }
        if previous < NOVA_BURST && t >= NOVA_BURST {
            // The burst: stellar matter streaking out as far as the shell will reach, and sparks
            // flung past it.
            for i in 0..110 {
                let r = particles.random();
                let d = direction(r);
                let far = 1.8 + r.next_double() * 0.9;
                let to = [
                    star[0] + d[0] * far,
                    star[1] + d[1] * far,
                    star[2] + d[2] * far,
                ];
                let colour = match i % 4 {
                    0 => WHITE,
                    1 => ORANGE,
                    2 => NOVA_PINK,
                    _ => NOVA_CYAN,
                };
                let duration = (8.0 + r.next_double() * 10.0) as i32;
                particles.add_glowing(
                    &ParticleOptions {
                        color: [colour[0], colour[1], colour[2], 1.0],
                        trail: Some((to, duration)),
                        ..ParticleOptions::simple(ParticleType::Trail)
                    },
                    star,
                    [0.0; 3],
                );
            }
            for _ in 0..30 {
                let r = particles.random();
                let d = direction(r);
                let s = 0.3 + r.next_double() * 0.4;
                particles.add_glowing(
                    &ParticleOptions::simple(ParticleType::EndRod),
                    star,
                    [d[0] * s, d[1] * s * 0.8, d[2] * s],
                );
            }
        }
        let crowning = NOVA_BURST + 0.6;
        if previous < crowning && t >= crowning {
            // Light falls from what is left of the star onto the head, where the crown forms.
            for k in 0..28 {
                let a = f64::from(k) * TAU / 28.0;
                let to = [
                    crown[0] + a.cos() * CROWN_RADIUS,
                    crown[1],
                    crown[2] + a.sin() * CROWN_RADIUS,
                ];
                particles.add_glowing(
                    &ParticleOptions {
                        color: [GOLD[0], GOLD[1], GOLD[2], 1.0],
                        trail: Some((to, 12 + k % 6)),
                        ..ParticleOptions::simple(ParticleType::Trail)
                    },
                    star,
                    [0.0; 3],
                );
            }
        }
        // Sounds.
        let audio = &mut self.audio;
        if t < NOVA_BURST && ticks.is_multiple_of(10) {
            let rise = 0.5 + 1.2 * smoothstep(0.0, NOVA_BURST, t);
            audio.play(
                "minecraft:block.amethyst_block.resonate",
                at,
                0.6,
                rise as f32,
            );
        }
        if previous < NOVA_FORMED && t >= NOVA_FORMED {
            audio.play("minecraft:entity.warden.sonic_charge", at, 0.8, 1.6);
        }
        if previous < NOVA_BURST && t >= NOVA_BURST {
            audio.play("minecraft:entity.generic.explode", at, 1.2, 0.6);
            audio.play("minecraft:entity.firework_rocket.large_blast", at, 1.4, 0.7);
            audio.play("minecraft:block.end_portal.spawn", at, 0.7, 1.5);
            audio.play("minecraft:block.beacon.activate", at, 1.0, 1.2);
        }
        if previous < crowning && t >= crowning {
            audio.play(
                "minecraft:block.amethyst_block.chime",
                Some(crown),
                1.0,
                0.8,
            );
        }
        if t > NOVA_BURST + 0.8 && t < NOVA_FADE && (ticks + 7).is_multiple_of(35) {
            audio.play("minecraft:block.amethyst_block.chime", at, 0.9, 1.2);
        }
        if previous < NOVA_FADE && t >= NOVA_FADE {
            audio.play("minecraft:block.beacon.deactivate", at, 0.7, 1.3);
        }
    }

    fn black_hole_effects(
        &mut self,
        previous: f64,
        t: f64,
        ticks: u32,
        anchor: [f64; 3],
        body: [f64; 3],
    ) {
        let at = Some(anchor);
        let particles = &mut self.particles;
        if ticks == 1 {
            // It opens softly: a slow puff of stardust.
            for _ in 0..24 {
                let d = direction(particles.random());
                let s = 0.05 + particles.random().next_double() * 0.08;
                particles.add_glowing(
                    &ParticleOptions::simple(ParticleType::EndRod),
                    anchor,
                    [d[0] * s, d[1] * s, d[2] * s],
                );
            }
        }
        let alive = t > 0.3 && t < GONE;
        if alive {
            let calm = 1.0 - smoothstep(FADE, GONE, t);
            // A thin, slow trickle of matter drifting in.
            if ticks.is_multiple_of(2) {
                let d = direction(particles.random());
                let s = 3.0 + particles.random().next_double() * 4.0;
                particles.add_glowing(
                    &ParticleOptions::simple(ParticleType::Portal),
                    anchor,
                    [d[0] * s, d[1] * s, d[2] * s],
                );
            }
            // Long, lazy streaks spiralling into the disk.
            if ticks.is_multiple_of(4) && calm > 0.2 {
                let r = particles.random();
                let a = r.next_double() * TAU;
                let radius = 7.0 + r.next_double() * 4.0;
                let from = [
                    anchor[0] + a.cos() * radius,
                    anchor[1] + (r.next_double() - 0.5) * 1.5,
                    anchor[2] + a.sin() * radius,
                ];
                let colour = match r.next_int_bound(3).unwrap_or(0) {
                    0 => VIOLET,
                    1 => ORANGE,
                    _ => WHITE,
                };
                let duration = (40.0 + r.next_double() * 30.0) as i32;
                particles.add_glowing(
                    &ParticleOptions {
                        color: [colour[0], colour[1], colour[2], 0.8],
                        trail: Some((anchor, duration)),
                        ..ParticleOptions::simple(ParticleType::Trail)
                    },
                    from,
                    [0.0; 3],
                );
            }
            // Warm dust gliding round the disk.
            if ticks.is_multiple_of(2) {
                let r = particles.random();
                let a = r.next_double() * TAU;
                let radius = 2.4 + r.next_double() * 2.0;
                let pos = [
                    anchor[0] + a.cos() * radius,
                    anchor[1] + (r.next_double() - 0.5) * 0.2,
                    anchor[2] + a.sin() * radius,
                ];
                let speed = 0.08 / radius.sqrt();
                let scale = 0.5 + r.next_float() * 0.5;
                particles.add_glowing(
                    &ParticleOptions::dust_transition(ORANGE, VIOLET, scale),
                    pos,
                    [-a.sin() * speed, 0.0, a.cos() * speed],
                );
            }
            // Stars hanging in the air round the floating player, barely moving.
            if t > 1.5 && ticks.is_multiple_of(5) {
                let r = particles.random();
                let d = direction(r);
                let far = 1.0 + r.next_double() * 2.5;
                let pos = [
                    body[0] + d[0] * far,
                    body[1] + d[1] * far * 0.6,
                    body[2] + d[2] * far,
                ];
                let drift = [
                    (r.next_double() - 0.5) * 0.01,
                    0.004 + r.next_double() * 0.008,
                    (r.next_double() - 0.5) * 0.01,
                ];
                particles.add_glowing(&ParticleOptions::simple(ParticleType::EndRod), pos, drift);
            }
            // A faint violet wake behind the player.
            if t > 1.0 && ticks.is_multiple_of(3) {
                let r = particles.random();
                let pos = [
                    body[0] + (r.next_double() - 0.5) * 0.8,
                    body[1] + (r.next_double() - 0.5) * 0.5,
                    body[2] + (r.next_double() - 0.5) * 0.8,
                ];
                particles.add_glowing(&ParticleOptions::dust(VIOLET, 0.6), pos, [0.0; 3]);
            }
        }
        if previous < GONE && t >= GONE {
            // It closes like a soap bubble: a last soft breath of stardust.
            for i in 0..40 {
                let d = direction(particles.random());
                let s = 0.06 + particles.random().next_double() * 0.12;
                let options = if i % 2 == 0 {
                    ParticleOptions::simple(ParticleType::EndRod)
                } else {
                    ParticleOptions::dust_transition(WHITE, VIOLET, 1.0)
                };
                particles.add_glowing(&options, anchor, [d[0] * s, d[1] * s, d[2] * s]);
            }
        }
        // Sounds: a low hum and the odd chime.
        let audio = &mut self.audio;
        if t > SUMMONED && t < FADE && ticks.is_multiple_of(60) {
            audio.play("minecraft:block.beacon.ambient", at, 0.9, 0.5);
        }
        if t > SUMMONED && t < FADE && (ticks + 30).is_multiple_of(70) {
            audio.play("minecraft:block.amethyst_block.chime", at, 0.8, 0.6);
        }
        if previous < FADE && t >= FADE {
            audio.play("minecraft:block.beacon.deactivate", at, 0.8, 0.6);
        }
        if previous < GONE && t >= GONE {
            audio.play("minecraft:block.amethyst_cluster.break", at, 0.6, 0.6);
        }
    }

    /// The emote's camera in place of `view` (position, yaw, pitch): drifting slowly round the
    /// hole with the player, bobbing gently, blended in and out of the player's own.
    pub(super) fn emote_camera(&self, view: (glam::DVec3, f32, f32)) -> (glam::DVec3, f32, f32) {
        let Some(p) = &self.emotes.playing else {
            return view;
        };
        let t = p.time();
        if !p.emote.has_lens() {
            return self.moves_camera(p.emote, t, p.feet, p.orbit_angle, self.emotes.look, view);
        }
        if p.emote == Emote::SupernovaCrown {
            return self.supernova_camera(p, t, view);
        }
        if p.emote == Emote::PurpleHollow {
            return self.hollow_camera(p, t, view);
        }
        if p.emote == Emote::FalseDreams {
            return self.dreams_camera(p, t, view);
        }
        if p.emote == Emote::TheRook {
            return self.rook_camera(p, t, view);
        }
        // The camera follows the player round, a little behind them, the hole beside them;
        // it comes closer where the ground or a tree is in the way.
        let (_, angle) = orbit(p, t);
        let a = angle - 0.5 + 0.08 * (t * 0.21).sin();
        let far = CAMERA_DISTANCE - 0.8 * (t * 0.17).sin().abs();
        let distance = [far, far * 0.8, far * 0.62, far * 0.45]
            .into_iter()
            .find(|&d| self.sees(camera_at(p.anchor, a, d), p.anchor))
            .unwrap_or(far * 0.45);
        let anchor = glam::DVec3::from_array(p.anchor);
        let mut position = glam::DVec3::from_array(camera_at(p.anchor, a, distance));
        // Floating too: a slow bob.
        position.y += 0.25 * (t * 0.6).sin();
        let look = anchor - position;
        let yaw = yaw_of(look.x, look.z) + (0.6 * (t * 0.27).sin()) as f32;
        let pitch = (-look.y.atan2(look.x.hypot(look.z)).to_degrees()) as f32
            + (0.5 * (t * 0.33).cos()) as f32;
        // In from the player's view and back to it, slowly.
        let blend = smoothstep(0.0, 2.0, t) * (1.0 - smoothstep(END - 1.5, END, t));
        let (from, from_yaw, from_pitch) = view;
        (
            from.lerp(position, blend),
            from_yaw + wrap(yaw - from_yaw) * blend as f32,
            from_pitch + (pitch - from_pitch) * blend as f32,
        )
    }

    /// The Supernova Crown's camera: in front of the player, a little below the star, swinging
    /// slowly round; a jolt at the burst, then further back for the remnant.
    fn supernova_camera(
        &self,
        p: &Playing,
        t: f64,
        view: (glam::DVec3, f32, f32),
    ) -> (glam::DVec3, f32, f32) {
        // The way the player faced when it started, then a slow swing round to one side.
        let a =
            p.orbit_angle + 0.5 * (t * 0.18).sin() + 0.25 * smoothstep(NOVA_BURST, NOVA_FADE, t);
        let forward = [-a.sin(), a.cos()];
        let focus = [p.feet[0], p.feet[1] + 2.3 + nova_lift(t), p.feet[2]];
        // It pulls back as the star bursts, to take in the whole shell.
        let far = NOVA_CAMERA
            + (NOVA_CAMERA_FAR - NOVA_CAMERA) * smoothstep(NOVA_BURST - 0.2, NOVA_BURST + 2.2, t);
        let distance = [far, far * 0.75, far * 0.5]
            .into_iter()
            .find(|&d| {
                let at = [
                    focus[0] + forward[0] * d,
                    focus[1] - 0.3,
                    focus[2] + forward[1] * d,
                ];
                self.sees(at, focus)
            })
            .unwrap_or(far * 0.5);
        let position = glam::DVec3::new(
            focus[0] + forward[0] * distance,
            focus[1] - 0.3,
            focus[2] + forward[1] * distance,
        );
        let look = glam::DVec3::from_array(focus) - position;
        let mut yaw = yaw_of(look.x, look.z);
        let mut pitch = (-look.y.atan2(look.x.hypot(look.z)).to_degrees()) as f32;
        let jolt = if t > NOVA_BURST {
            (-(t - NOVA_BURST) * 4.0).exp() * 2.2
        } else {
            0.0
        };
        let rumble = 0.2 * smoothstep(NOVA_FORMED, NOVA_BURST, t) + jolt;
        yaw += ((t * 37.0).sin() * 0.6 + (t * 23.0).sin() * 0.4) as f32 * rumble as f32;
        pitch += ((t * 41.0).cos() * 0.6 + (t * 29.0).sin() * 0.4) as f32 * rumble as f32;
        let blend = smoothstep(0.0, 1.2, t) * (1.0 - smoothstep(NOVA_END - 1.2, NOVA_END, t));
        let (from, from_yaw, from_pitch) = view;
        (
            from.lerp(position, blend),
            from_yaw + wrap(yaw - from_yaw) * blend as f32,
            from_pitch + (pitch - from_pitch) * blend as f32,
        )
    }

    /// The Purple Hollow's camera: in front of the player and a little to their side while
    /// the orbs gather, swinging round behind their shoulder as the sphere charges, to watch it
    /// tear away; it shakes as it charges and jolts as it is fired.
    fn hollow_camera(
        &self,
        p: &Playing,
        t: f64,
        view: (glam::DVec3, f32, f32),
    ) -> (glam::DVec3, f32, f32) {
        let h = hollow_at(t, p.feet, p.orbit_angle);
        // Round the player: 0 is straight ahead of them, pi straight behind.
        let round = 0.45
            + 0.12 * (t * 0.5).sin()
            + 2.25 * smoothstep(HOLLOW_JOIN - 0.3, HOLLOW_FIRE - 0.5, t);
        let a = p.orbit_angle + round;
        let out = [-a.sin(), a.cos()];
        // It looks at the player, then past them to where the sphere goes.
        let ahead = 5.0 * smoothstep(HOLLOW_FIRE - 0.6, HOLLOW_FIRE + 0.8, t);
        let focus = [
            p.feet[0] + h.forward[0] * (0.6 + ahead),
            p.feet[1] + 1.55,
            p.feet[2] + h.forward[1] * (0.6 + ahead),
        ];
        let from = [p.feet[0], p.feet[1] + 1.9, p.feet[2]];
        let far = 4.4 + 1.2 * smoothstep(HOLLOW_MERGE, HOLLOW_FIRE, t);
        let distance = [far, far * 0.75, far * 0.5]
            .into_iter()
            .find(|&d| {
                let at = [from[0] + out[0] * d, from[1] + 0.3, from[2] + out[1] * d];
                self.sees(at, from)
            })
            .unwrap_or(far * 0.5);
        let position = glam::DVec3::new(
            from[0] + out[0] * distance,
            from[1] + 0.3,
            from[2] + out[1] * distance,
        );
        let look = glam::DVec3::from_array(focus) - position;
        let mut yaw = yaw_of(look.x, look.z);
        let mut pitch = (-look.y.atan2(look.x.hypot(look.z)).to_degrees()) as f32;
        let jolt = if t > HOLLOW_FIRE {
            (-(t - HOLLOW_FIRE) * 3.5).exp() * 2.4
        } else {
            0.0
        };
        let rumble = (0.3
            * smoothstep(HOLLOW_MERGE, HOLLOW_FIRE, t)
            * (1.0 - smoothstep(HOLLOW_FIRE, HOLLOW_FIRE + 0.1, t))
            + jolt) as f32;
        yaw += ((t * 37.0).sin() * 0.6 + (t * 23.0).sin() * 0.4) as f32 * rumble;
        pitch += ((t * 41.0).cos() * 0.6 + (t * 29.0).sin() * 0.4) as f32 * rumble;
        let blend = smoothstep(0.0, 1.2, t) * (1.0 - smoothstep(HOLLOW_END - 1.2, HOLLOW_END, t));
        let (from, from_yaw, from_pitch) = view;
        (
            from.lerp(position, blend),
            from_yaw + wrap(yaw - from_yaw) * blend as f32,
            from_pitch + (pitch - from_pitch) * blend as f32,
        )
    }

    /// The Rook's camera: off to the player's side, a little behind, framing them and the rook;
    /// tilting up as it falls, jolted as it lands and blasts, then pulling back to follow the
    /// blast forward.
    fn rook_camera(
        &self,
        p: &Playing,
        t: f64,
        view: (glam::DVec3, f32, f32),
    ) -> (glam::DVec3, f32, f32) {
        let r = rook_at(t, p.anchor, p.orbit_angle);
        let f = r.forward;
        // The angle round the player with the clearest view of both them and the rook: the
        // same all through, as it is judged from where things end up, not from the moment.
        let clear = |turn: f64| {
            let a = p.orbit_angle + turn;
            let middle = [
                p.feet[0] + f[0] * 2.5,
                p.feet[1] + 1.5,
                p.feet[2] + f[1] * 2.5,
            ];
            [8.0, 6.0, 4.5, 3.0, 2.0]
                .into_iter()
                .find(|&d| {
                    let at = [
                        middle[0] - a.sin() * d,
                        middle[1] + 1.6,
                        middle[2] + a.cos() * d,
                    ];
                    self.sees(at, middle)
                        && self.sees(at, [p.feet[0], p.feet[1] + 1.6, p.feet[2]])
                        && self.sees(at, [p.anchor[0], p.anchor[1] + ROOK_HEIGHT, p.anchor[2]])
                })
                .unwrap_or(0.0)
        };
        let turn = [1.95, -1.95, 2.5, -2.5, 1.4, -1.4, PI]
            .into_iter()
            .fold((1.95, -1.0), |best, turn| {
                let d = clear(turn);
                if d > best.1 { (turn, d) } else { best }
            })
            .0;
        let a = p.orbit_angle + turn + 0.08 * (t * 0.4).sin();
        let out = [-a.sin(), a.cos()];
        let blast = smoothstep(ROOK_BLAST - 0.3, ROOK_BLAST + 1.0, t);
        let falling = smoothstep(ROOK_DROP - 0.3, ROOK_DROP + 0.1, t)
            * (1.0 - smoothstep(ROOK_LAND - 0.1, ROOK_LAND + 0.3, t));
        let ahead = ROOK_AHEAD * 0.5 + 2.5 * blast;
        let focus = [
            p.feet[0] + f[0] * ahead,
            p.feet[1] + 1.5 + 2.2 * falling,
            p.feet[2] + f[1] * ahead,
        ];
        let far = 7.0 + 2.0 * blast;
        let distance = [far, far * 0.75, far * 0.55, far * 0.4, far * 0.28]
            .into_iter()
            .find(|&d| {
                let at = [focus[0] + out[0] * d, focus[1] + 1.6, focus[2] + out[1] * d];
                self.sees(at, focus) && self.sees(at, [p.feet[0], p.feet[1] + 1.6, p.feet[2]])
            })
            .unwrap_or(1.5);
        let position = glam::DVec3::new(
            focus[0] + out[0] * distance,
            focus[1] + 1.6,
            focus[2] + out[1] * distance,
        );
        let look = glam::DVec3::from_array(focus) - position;
        let mut yaw = yaw_of(look.x, look.z);
        let mut pitch = (-look.y.atan2(look.x.hypot(look.z)).to_degrees()) as f32;
        let jolt = |since: f64, size: f64| {
            if since > 0.0 {
                (-since * 4.0).exp() * size
            } else {
                0.0
            }
        };
        let rumble = (jolt(t - ROOK_LAND, 1.6) + jolt(t - ROOK_BLAST, 3.0)) as f32;
        yaw += ((t * 37.0).sin() * 0.6 + (t * 23.0).sin() * 0.4) as f32 * rumble;
        pitch += ((t * 41.0).cos() * 0.6 + (t * 29.0).sin() * 0.4) as f32 * rumble;
        let blend = smoothstep(0.0, 0.9, t) * (1.0 - smoothstep(ROOK_END - 1.0, ROOK_END, t));
        let (from, from_yaw, from_pitch) = view;
        (
            from.lerp(position, blend),
            from_yaw + wrap(yaw - from_yaw) * blend as f32,
            from_pitch + (pitch - from_pitch) * blend as f32,
        )
    }

    /// False Dreams' camera: in front of the player and a little below, looking up at them as
    /// they float, circling slowly round them, swaying as if held by an unsteady hand.
    fn dreams_camera(
        &self,
        p: &Playing,
        t: f64,
        view: (glam::DVec3, f32, f32),
    ) -> (glam::DVec3, f32, f32) {
        let d = dream_at(t, p.anchor[1] - p.feet[1]);
        let body = [p.feet[0], p.feet[1] + d.lift + 1.1, p.feet[2]];
        let a = p.orbit_angle + 0.35 + t * 0.045;
        let out = [-a.sin(), a.cos()];
        let far = 5.6 - 0.6 * (t * 0.11).sin();
        let height = 0.2 - 0.5 * smoothstep(0.0, DREAMS_RISEN, t);
        let distance = [far, far * 0.75, far * 0.5]
            .into_iter()
            .find(|&r| {
                let at = [body[0] + out[0] * r, body[1] + height, body[2] + out[1] * r];
                self.sees(at, body)
            })
            .unwrap_or(far * 0.5);
        let position = glam::DVec3::new(
            body[0] + out[0] * distance,
            body[1] + height,
            body[2] + out[1] * distance,
        );
        let look = glam::DVec3::new(body[0], body[1] - 0.25, body[2]) - position;
        let sway = ((t * 0.6).sin() * 0.8 + (t * 0.23).sin() * 0.6) as f32;
        let yaw = yaw_of(look.x, look.z) + sway;
        let pitch = (-look.y.atan2(look.x.hypot(look.z)).to_degrees()) as f32
            + ((t * 0.45).sin() * 0.6) as f32;
        let blend = smoothstep(0.0, 1.5, t) * (1.0 - smoothstep(DREAMS_END - 1.5, DREAMS_END, t));
        let (from, from_yaw, from_pitch) = view;
        (
            from.lerp(position, blend),
            from_yaw + wrap(yaw - from_yaw) * blend as f32,
            from_pitch + (pitch - from_pitch) * blend as f32,
        )
    }

    /// What the renderer draws of the Black Hole this frame, or of the Supernova Crown (which
    /// borrows its pass, flash and shock wave): this player's emote, else the nearest other
    /// player's (its flash dimmed: it is theirs).
    pub(super) fn emote_black_hole(&self) -> Option<BlackHole> {
        if let Some(p) = self.emotes.playing.as_ref()
            && p.emote.has_lens()
        {
            return lens_at(p.emote, p.time(), p.anchor, p.feet, p.orbit_angle);
        }
        self.remote_lens().map(|mut lens| {
            lens.flash *= 0.3;
            // Their view swims, not this one.
            if let Some(dreams) = lens.dreams.as_mut() {
                dreams.blur = 0.0;
            }
            lens
        })
    }

    /// The wheel over the HUD while it is open.
    pub(super) fn draw_emote_wheel(&self, batch: &mut HudBatch) {
        let Some(wheel) = &self.emotes.wheel else {
            return;
        };
        let atlas = &self.gui.atlas;
        let (cx, cy) = (batch.width / 2.0, batch.height / 2.0);
        let (inner, outer) = (28.0f32, 80.0f32);
        let hovered = wheel.hovered();
        let point = |angle: f32, r: f32| [cx + angle.cos() * r, cy + angle.sin() * r];
        let slice = TAU as f32 / SLOTS as f32;
        // A dark veil behind it.
        batch.fill(atlas, 0.0, 0.0, batch.width, batch.height, 0x60000000);
        for slot in 0..SLOTS {
            let centre = -PI as f32 / 2.0 + slot as f32 * slice;
            let (from, to) = (centre - slice / 2.0 + 0.03, centre + slice / 2.0 - 0.03);
            let on = hovered == Some(slot);
            let filled = wheel.at(slot).is_some_and(|e| wheel.owns(e));
            let colour = match (on, filled) {
                (true, true) => 0xE08A3CF0,
                (true, false) => 0xC0402A58,
                (false, true) => 0xC8241634,
                (false, false) => 0xB0140E1E,
            };
            let reach = if on { outer + 4.0 } else { outer };
            let steps = 8;
            for k in 0..steps {
                let a0 = from + (to - from) * k as f32 / steps as f32;
                let a1 = from + (to - from) * (k + 1) as f32 / steps as f32;
                batch.fill_quad(
                    atlas,
                    [
                        point(a0, inner),
                        point(a0, reach),
                        point(a1, reach),
                        point(a1, inner),
                    ],
                    colour,
                );
                // A bright rim on the outside.
                let rim = if on { 0xFFE6B4FF } else { 0x806A4C8A };
                batch.fill_quad(
                    atlas,
                    [
                        point(a0, reach - 1.5),
                        point(a0, reach),
                        point(a1, reach),
                        point(a1, reach - 1.5),
                    ],
                    rim,
                );
            }
            let [ix, iy] = point(centre, (inner + reach) / 2.0);
            match wheel.at(slot) {
                Some(Emote::BlackHole) => {
                    let lit = wheel.owns(Emote::BlackHole);
                    let dim = |c: u32| {
                        if lit {
                            c
                        } else {
                            (c & 0x00FFFFFF) | 0x50000000
                        }
                    };
                    let ring = |r0: f32, r1: f32, colour: u32, batch: &mut HudBatch| {
                        for k in 0..24 {
                            let a0 = k as f32 / 24.0 * TAU as f32;
                            let a1 = (k + 1) as f32 / 24.0 * TAU as f32;
                            let at = |a: f32, r: f32| [ix + a.cos() * r, iy + a.sin() * r * 0.55];
                            batch.fill_quad(
                                atlas,
                                [at(a0, r0), at(a0, r1), at(a1, r1), at(a1, r0)],
                                colour,
                            );
                        }
                    };
                    let disc = |r: f32, colour: u32, batch: &mut HudBatch| {
                        for k in 0..24 {
                            let a0 = k as f32 / 24.0 * TAU as f32;
                            let a1 = (k + 1) as f32 / 24.0 * TAU as f32;
                            let at = |a: f32| [ix + a.cos() * r, iy + a.sin() * r];
                            batch.fill_quad(atlas, [[ix, iy], at(a0), at(a1), at(a1)], colour);
                        }
                    };
                    disc(11.0, dim(0x40B45CFF), batch);
                    ring(6.5, 11.0, dim(0xFFFF9A3C), batch);
                    ring(9.0, 12.0, dim(0xC0B45CFF), batch);
                    disc(5.5, dim(0xFFFFE0B0), batch);
                    disc(4.8, 0xFF000000, batch);
                }
                Some(Emote::SupernovaCrown) => {
                    let lit = wheel.owns(Emote::SupernovaCrown);
                    let dim = |c: u32| {
                        if lit {
                            c
                        } else {
                            (c & 0x00FFFFFF) | 0x50000000
                        }
                    };
                    let disc = |r: f32, colour: u32, batch: &mut HudBatch| {
                        for k in 0..20 {
                            let a0 = k as f32 / 20.0 * TAU as f32;
                            let a1 = (k + 1) as f32 / 20.0 * TAU as f32;
                            let at = |a: f32| [ix + a.cos() * r, iy + 3.0 + a.sin() * r];
                            batch.fill_quad(
                                atlas,
                                [[ix, iy + 3.0], at(a0), at(a1), at(a1)],
                                colour,
                            );
                        }
                    };
                    // A glow, the star, and a crown of five points over it.
                    disc(10.0, dim(0x40FFB347), batch);
                    disc(5.0, dim(0xFFFFE9A8), batch);
                    disc(2.8, dim(0xFFFFFFFF), batch);
                    for k in 0..5 {
                        let x = ix - 8.0 + k as f32 * 4.0;
                        let h = if k == 2 {
                            7.0
                        } else if k % 2 == 1 {
                            4.5
                        } else {
                            5.5
                        };
                        batch.fill_quad(
                            atlas,
                            [
                                [x - 1.6, iy - 3.0],
                                [x, iy - 3.0 - h],
                                [x + 1.6, iy - 3.0],
                                [x, iy - 3.0],
                            ],
                            dim(0xFFFFC34D),
                        );
                    }
                    batch.fill(atlas, ix - 9.0, iy - 3.0, 18.0, 2.0, dim(0xFFFFC34D));
                }
                Some(Emote::PurpleHollow) => {
                    let lit = wheel.owns(Emote::PurpleHollow);
                    let dim = |c: u32| {
                        if lit {
                            c
                        } else {
                            (c & 0x00FFFFFF) | 0x50000000
                        }
                    };
                    let disc = |x: f32, y: f32, r: f32, colour: u32, batch: &mut HudBatch| {
                        for k in 0..20 {
                            let a0 = k as f32 / 20.0 * TAU as f32;
                            let a1 = (k + 1) as f32 / 20.0 * TAU as f32;
                            let at = |a: f32| [x + a.cos() * r, y + a.sin() * r];
                            batch.fill_quad(atlas, [[x, y], at(a0), at(a1), at(a1)], colour);
                        }
                    };
                    // The blue orb and the red one, and the purple sphere between them.
                    disc(ix - 8.5, iy + 4.0, 3.6, dim(0xFF3C8CFF), batch);
                    disc(ix - 8.5, iy + 4.0, 1.8, dim(0xFF081440), batch);
                    disc(ix + 8.5, iy + 4.0, 3.6, dim(0xFFFF3A2A), batch);
                    disc(ix + 8.5, iy + 4.0, 1.6, dim(0xFFFFB080), batch);
                    disc(ix, iy - 2.0, 10.0, dim(0x40B45CFF), batch);
                    disc(ix, iy - 2.0, 6.5, dim(0xFF8A3CF0), batch);
                    disc(ix, iy - 2.0, 3.4, dim(0xFFE8C8FF), batch);
                }
                Some(Emote::FalseDreams) => {
                    let lit = wheel.owns(Emote::FalseDreams);
                    let dim = |c: u32| {
                        if lit {
                            c
                        } else {
                            (c & 0x00FFFFFF) | 0x50000000
                        }
                    };
                    let oval = |rx: f32, ry: f32, colour: u32, batch: &mut HudBatch| {
                        for k in 0..24 {
                            let a0 = k as f32 / 24.0 * TAU as f32;
                            let a1 = (k + 1) as f32 / 24.0 * TAU as f32;
                            let at = |a: f32| [ix + a.cos() * rx, iy + a.sin() * ry];
                            batch.fill_quad(atlas, [[ix, iy], at(a0), at(a1), at(a1)], colour);
                        }
                    };
                    // An eye: a red glow, the white, a burning iris and its pupil.
                    // (The HUD takes its colours with red and blue swapped.)
                    oval(13.0, 9.0, dim(0x402A2AFF), batch);
                    oval(10.5, 5.2, dim(0xFFD4DCE8), batch);
                    oval(4.4, 4.4, dim(0xFF1C34D8), batch);
                    oval(3.0, 3.0, dim(0xFF2E8AFF), batch);
                    oval(1.7, 1.7, 0xFF080000, batch);
                    batch.fill(atlas, ix - 2.6, iy - 2.8, 1.2, 1.2, dim(0xFFFFFFFF));
                }
                Some(Emote::TheRook) => {
                    let lit = wheel.owns(Emote::TheRook);
                    let dim = |c: u32| {
                        if lit {
                            c
                        } else {
                            (c & 0x00FFFFFF) | 0x50000000
                        }
                    };
                    // An ivory rook, and the teal "!!" badge over its shoulder.
                    // (The HUD takes its colours with red and blue swapped.)
                    let (x, y) = (ix - 3.0, iy + 1.0);
                    let ivory = dim(0xFFE0ECF2);
                    let shade = dim(0xFFB4C4CC);
                    batch.fill(atlas, x - 7.0, y + 8.0, 14.0, 3.0, ivory);
                    batch.fill(atlas, x - 5.5, y + 6.0, 11.0, 2.0, shade);
                    batch.fill(atlas, x - 4.0, y - 3.0, 8.0, 9.0, ivory);
                    batch.fill(atlas, x - 5.5, y - 5.0, 11.0, 2.0, shade);
                    batch.fill(atlas, x - 6.0, y - 9.0, 12.0, 4.0, ivory);
                    for k in 0..3 {
                        batch.fill(atlas, x - 6.0 + k as f32 * 4.5, y - 11.5, 3.0, 2.5, ivory);
                    }
                    let (bx, by) = (ix + 8.0, iy - 8.0);
                    for k in 0..20 {
                        let a0 = k as f32 / 20.0 * TAU as f32;
                        let a1 = (k + 1) as f32 / 20.0 * TAU as f32;
                        let at = |a: f32, r: f32| [bx + a.cos() * r, by + a.sin() * r];
                        batch.fill_quad(
                            atlas,
                            [[bx, by], at(a0, 7.0), at(a1, 7.0), at(a1, 7.0)],
                            dim(0x409AB31F),
                        );
                        batch.fill_quad(
                            atlas,
                            [[bx, by], at(a0, 5.0), at(a1, 5.0), at(a1, 5.0)],
                            dim(0xFF9AB31F),
                        );
                    }
                    for dx in [-2.2f32, 1.0] {
                        batch.fill(atlas, bx + dx, by - 3.2, 1.3, 4.0, dim(0xFFFFFFFF));
                        batch.fill(atlas, bx + dx, by + 1.6, 1.3, 1.3, dim(0xFFFFFFFF));
                    }
                }
                Some(
                    e @ (Emote::DarkAscension
                    | Emote::Bruh
                    | Emote::LDance
                    | Emote::CelestialWings
                    | Emote::Sit
                    | Emote::CrouchSpam
                    | Emote::SkillIssue
                    | Emote::TooEasy
                    | Emote::PointAndLaugh
                    | Emote::CryAboutIt),
                ) => moves::draw_moves_icon(batch, atlas, e, ix, iy, wheel.owns(e)),
                None => batch.fill(atlas, ix - 1.0, iy - 1.0, 2.0, 2.0, 0x60FFFFFF),
            }
        }
        // The centre: what's picked and how to play it.
        for k in 0..32 {
            let a0 = k as f32 / 32.0 * TAU as f32;
            let a1 = (k + 1) as f32 / 32.0 * TAU as f32;
            batch.fill_quad(
                atlas,
                [
                    [cx, cy],
                    point(a0, inner - 3.0),
                    point(a1, inner - 3.0),
                    point(a1, inner - 3.0),
                ],
                0xE00C0814,
            );
        }
        let chosen = hovered.and_then(|slot| wheel.at(slot));
        let title = chosen.map_or_else(
            || crate::lang::get_or("wisp.emote.title", "Emotes"),
            Emote::name,
        );
        let hint = match chosen {
            Some(e) if wheel.owns(e) => crate::lang::get_or("wisp.emote.play", "Click to play"),
            Some(e) => crate::lang::get_or(
                "wisp.emote.locked",
                "Buy it in the launcher's Shop (%s coins)",
            )
            .replace("%s", &e.price().to_string()),
            None => crate::lang::get_or("wisp.emote.pick", "Move the mouse to pick"),
        };
        let tw = HudBatch::text_width(atlas, &title);
        batch.text(
            atlas,
            &title,
            (cx - tw / 2.0).floor(),
            cy - outer - 18.0,
            0xFFFFFFFF,
            true,
        );
        let hw = HudBatch::text_width(atlas, &hint);
        batch.text(
            atlas,
            &hint,
            (cx - hw / 2.0).floor(),
            cy + outer + 8.0,
            0xFFB9A6CC,
            true,
        );
        // The pages: a dot each under the hint, the shown one lit, and how to turn them.
        let pages = PAGES.len() as f32;
        for k in 0..PAGES.len() {
            let x = cx - (pages - 1.0) * 5.0 + k as f32 * 10.0;
            let colour = if k == wheel.page {
                0xFFE6B4FF
            } else {
                0x80806A8A
            };
            batch.fill(atlas, x - 2.0, cy + outer + 22.0, 4.0, 4.0, colour);
        }
        let turn = crate::lang::get_or("wisp.emote.pages", "Scroll or Q / E for more emotes");
        let pw = HudBatch::text_width(atlas, &turn);
        batch.text(
            atlas,
            &turn,
            (cx - pw / 2.0).floor(),
            cy + outer + 30.0,
            0xFF8C7AA0,
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wheel_picks_the_slot_the_mouse_points_at() {
        let mut wheel = Wheel::default();
        assert_eq!(wheel.hovered(), None);
        // Ten slots of 36 degrees, clockwise from the top (screen y points down).
        wheel.aim = (0.0, -1.0);
        assert_eq!(wheel.hovered(), Some(0));
        wheel.aim = (0.0, 1.0);
        assert_eq!(wheel.hovered(), Some(5));
        // 72 degrees clockwise from the top: up and to the right.
        wheel.aim = (0.951, -0.309);
        assert_eq!(wheel.hovered(), Some(2));
        wheel.aim = (-0.951, -0.309);
        assert_eq!(wheel.hovered(), Some(8));
    }

    #[test]
    fn the_wheel_turns_its_pages_round() {
        let mut wheel = Wheel::default();
        assert_eq!(PAGES[wheel.page][0], Some(Emote::BlackHole));
        // The shop sells no emote now: the wheel offers none.
        assert_eq!(wheel.at(0), None);
        wheel.turn(1);
        assert_eq!(PAGES[wheel.page][0], Some(Emote::Sit));
        wheel.turn(1);
        assert_eq!(wheel.page, 0, "past the last page comes the first");
        wheel.turn(-1);
        assert_eq!(wheel.page, PAGES.len() - 1);
    }

    #[test]
    fn every_emote_has_a_slot_and_a_shop_id() {
        for emote in Emote::ALL {
            assert_eq!(
                PAGES
                    .iter()
                    .flatten()
                    .filter(|s| **s == Some(emote))
                    .count(),
                1,
                "{emote:?}"
            );
            assert_eq!(Emote::from_id(emote.id()), Some(emote));
        }
        assert_eq!(
            Emote::from_id("celestial_wings"),
            Some(Emote::CelestialWings)
        );
        assert!(Emote::from_id("nothing").is_none());
        // Only the emotes drawn through the lens pass have a lens.
        assert!(lens_at(Emote::Bruh, 1.0, [0.0; 3], [0.0; 3], 0.0).is_none());
        assert!(lens_at(Emote::BlackHole, 1.0, [0.0; 3], [0.0; 3], 0.0).is_some());
    }

    #[test]
    fn only_emotes_from_the_shop_unlock() {
        let wheel = Wheel {
            owned: parse_owned("black_hole\n\n"),
            ..Wheel::default()
        };
        assert!(wheel.owns(Emote::BlackHole));
        assert!(!Wheel::default().owns(Emote::BlackHole));
    }

    #[test]
    fn the_hole_grows_lives_and_fades() {
        assert!(hole_radius(0.0).abs() < 1e-9);
        assert!((hole_radius(SUMMONED) - RADIUS).abs() < 0.1);
        assert!(hole_radius(FADE + 0.5) < RADIUS && hole_radius(FADE + 0.5) > 0.0);
        assert_eq!(hole_radius(GONE + 0.01), 0.0);
    }

    #[test]
    fn the_purple_hollow_gathers_merges_and_fires() {
        assert_eq!(PAGES[0][2], Some(Emote::PurpleHollow));
        assert_eq!(Emote::from_id("purple_hollow"), Some(Emote::PurpleHollow));
        assert_eq!(Emote::PurpleHollow.price(), 1200);
        const {
            assert!(HOLLOW_BLUE < HOLLOW_RED && HOLLOW_RED < HOLLOW_JOIN);
            assert!(HOLLOW_JOIN < HOLLOW_MERGE && HOLLOW_MERGE < HOLLOW_FIRE);
            assert!(HOLLOW_FIRE < HOLLOW_AWAY && HOLLOW_AWAY < HOLLOW_END);
        };
        let feet = [10.0, 64.0, -3.0];
        let at = |t: f64| hollow_at(t, feet, 0.0);
        let apart = |h: &Hollow| (h.blue[0] - h.red[0]).hypot(h.blue[2] - h.red[2]);
        // The blue orb comes first, on the player's right (yaw 0 faces +z: right is -x).
        let early = at(HOLLOW_RED - 0.1);
        assert!(early.orbs[0] > 0.2 && early.orbs[1] == 0.0 && early.radius == 0.0);
        assert!(early.blue[0] < feet[0]);
        // Both held apart, then brought together where the sphere is born.
        let held = at(HOLLOW_JOIN);
        assert!(held.orbs[1] > 0.2 && apart(&held) > 2.0);
        let met = at(HOLLOW_MERGE);
        assert!(apart(&met) < 1e-6 && (met.blue[2] - met.sphere[2]).abs() < 1e-6);
        // The sphere charges in place, then flies ahead of the player and is gone.
        let charged = at(HOLLOW_FIRE - 0.01);
        assert!(charged.orbs == [0.0, 0.0] && charged.radius > 0.6);
        assert!((charged.sphere[2] - (feet[2] + HOLLOW_AHEAD)).abs() < 1e-6);
        let flying = at(HOLLOW_FIRE + 1.5);
        assert!(flying.sphere[2] > feet[2] + 30.0 && flying.radius > 1.0);
        assert_eq!(at(HOLLOW_END).radius, 0.0);
        assert!(hollow_wake(HOLLOW_FIRE) == 1.0 && hollow_wake(HOLLOW_END) == 0.0);
        // The arms hold only while it plays.
        assert_eq!(hollow_arms(0.0).1, 0.0);
        assert_eq!(hollow_arms(HOLLOW_MERGE).1, 1.0);
        assert_eq!(hollow_arms(HOLLOW_END).1, 0.0);
    }

    #[test]
    fn false_dreams_floats_the_player_for_as_long_as_its_song() {
        assert_eq!(PAGES[0][3], Some(Emote::FalseDreams));
        assert_eq!(Emote::from_id("false_dreams"), Some(Emote::FalseDreams));
        assert_eq!(Emote::FalseDreams.price(), 500);
        // The emote ends with its song.
        let (samples, rate) = wisp_client::audio::decode_bytes(DREAMS_SONG, "mp3").unwrap();
        let song = samples.len() as f64 / f64::from(rate);
        assert!(
            (song - DREAMS_END).abs() < 1.0,
            "song {song} s, emote {DREAMS_END} s"
        );
        // Standing, then floating at the top, then down again.
        assert!(dream_at(0.0, DREAMS_TOP).lift.abs() < 1e-9);
        let up = dream_at(30.0, DREAMS_TOP);
        assert!((up.lift - DREAMS_TOP).abs() < 0.1 && up.head_pitch < -40.0 && up.arms.1 == 1.0);
        let down = dream_at(DREAMS_LANDED, DREAMS_TOP);
        assert!(down.lift.abs() < 1e-9 && down.arms.1 == 0.0 && down.tilt == [0.0; 2]);
        // Under a roof they keep to the ground.
        assert!(dream_at(30.0, 0.0).lift.abs() < 0.1);
        // The eyes come after it starts and are gone before it ends; so is the song.
        assert_eq!(dream_eyes(0.0), 0.0);
        assert_eq!(dream_eyes(30.0), 1.0);
        assert_eq!(dream_eyes(DREAMS_END), 0.0);
        assert_eq!(dream_volume(DREAMS_END), 0.0);
        assert_eq!(dream_volume(30.0), 1.0);
    }

    #[test]
    fn the_supernova_gathers_bursts_and_crowns() {
        assert_eq!(PAGES[0][1], Some(Emote::SupernovaCrown));
        assert_eq!(Emote::SupernovaCrown.id(), "supernova_crown");
        const {
            assert!(NOVA_FORMED < NOVA_COLLAPSE && NOVA_COLLAPSE < NOVA_BURST);
            assert!(NOVA_BURST < NOVA_FADE && NOVA_FADE < NOVA_END);
        };
        // The star grows, then falls in on itself before it bursts.
        assert!(nova_star(NOVA_FORMED) > 0.4 && nova_star(NOVA_BURST - 0.01) < 0.2);
        assert!(nova_life(NOVA_BURST) == 1.0 && nova_life(NOVA_END) == 0.0);
        assert!(
            nova_lift(0.0).abs() < 1e-9 && nova_lift(5.0) > 0.3 && nova_lift(NOVA_END).abs() < 1e-6
        );
    }

    #[test]
    fn the_rook_falls_lands_pops_its_badge_blasts_and_crumbles() {
        let ground = [10.0, 64.0, -3.0];
        // Nothing until it starts falling; high in the sky as it does; on the ground once landed.
        assert_eq!(rook_at(0.2, ground, 0.0).shown, 0.0);
        let falling = rook_at(ROOK_DROP + 0.01, ground, 0.0);
        assert!(falling.shown > 0.99 && falling.base[1] > ground[1] + ROOK_FALL * 0.9);
        let landed = rook_at(ROOK_LAND + 1.0, ground, 0.0);
        assert!((landed.base[1] - ground[1]).abs() < 0.01);
        // The badge only after it landed, over its top.
        assert_eq!(rook_at(ROOK_BADGE - 0.01, ground, 0.0).badge_size, 0.0);
        let popped = rook_at(ROOK_BADGE + 1.0, ground, 0.0);
        assert!((popped.badge_size - 1.0).abs() < 0.05);
        assert!(popped.badge[1] > ground[1] + ROOK_HEIGHT);
        // The blast goes the way the player faces: yaw 0 is south (+z).
        let lens = lens_at(Emote::TheRook, ROOK_BLAST + 0.2, ground, ground, 0.0).unwrap();
        let rook = lens.rook.expect("the rook's lens");
        assert!(rook.blast_age > 0.0 && lens.flash > 0.1);
        assert!(lens.disk_normal[2] > 0.99);
        assert!(rook.origin.z > ground[2]);
        assert!(
            lens_at(Emote::TheRook, 1.0, ground, ground, 0.0)
                .unwrap()
                .rook
                .unwrap()
                .blast_age
                < 0.0
        );
        // Gone at the end.
        assert!(rook_at(ROOK_END - 0.1, ground, 0.0).shown < 0.01);
        assert_eq!(rook_at(ROOK_END - 0.1, ground, 0.0).badge_size, 0.0);
    }

    #[test]
    fn yaw_matches_minecrafts() {
        // Yaw 0 faces +z (south), 90 faces -x (west).
        assert!((yaw_of(0.0, 1.0)).abs() < 1e-4);
        assert!((yaw_of(-1.0, 0.0) - 90.0).abs() < 1e-4);
    }
}
