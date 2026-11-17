//! Cosmetics on players ([docs/cosmetics.md](../../../../docs/cosmetics.md)): only Wisp
//! clients draw them, off unless `wisp_cosmetics::enabled()`. What another player wears comes
//! from the cosmetics service by UUID (`wisp_cosmetics::Cosmetics`, its own thread); this file
//! puts the items on the avatar's bones, on the GPU skin path when the renderer takes it.

use super::Game;
use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};
use wisp_client::fast_hash::FxHashMap;
use wisp_client::mobs::anim::Poses;
use wisp_client::mobs::{Mob, Part, Polygon};
use wisp_cosmetics::prefs::{self, Others};
use wisp_cosmetics::{Item, Slot};

/// An item as the renderer takes it.
struct Baked {
    item: Arc<Item>,
    /// A model per bone it rides (a shirt's sleeves are on the arms).
    models: Vec<(wisp_cosmetics::Bone, Arc<Part>)>,
    /// Its texture's sprite: a 64×64 skin slot of the atlas, or an HD cosmetic slot (`hd`).
    location: String,
    /// The frame it was last marked drawn in (its slot is kept).
    marked: Cell<u32>,
    /// The frame of an animated texture its slot holds.
    shown: Cell<Option<usize>>,
}

impl Baked {
    fn new(item: &Arc<Item>, frame: u32) -> Self {
        Self {
            item: item.clone(),
            models: item
                .roots()
                .map(|(bone, root)| (bone, Arc::new(part(root))))
                .collect(),
            location: format!("cosmetics/{:x}", Arc::as_ptr(item) as usize),
            marked: Cell::new(frame.wrapping_sub(1)),
            shown: Cell::new(None),
        }
    }
}

/// Whether `item`'s texture is too large for a skin slot and takes an HD cosmetic slot.
fn hd(item: &Item) -> bool {
    item.texture.width > 64 || item.texture.height > 64
}

/// The side of the atlas slot `item`'s texture is in.
fn slot_side(item: &Item) -> f32 {
    if hd(item) {
        wisp_client::hud::HD_SLOT_SIZE as f32
    } else {
        64.0
    }
}

#[derive(Default)]
pub(super) struct Cosmetics {
    client: Option<wisp_cosmetics::Cosmetics>,
    /// By the item's address; each entry holds its item, so the address isn't reused.
    baked: RefCell<FxHashMap<usize, Baked>>,
    /// Items drawn whose texture isn't in the atlas yet.
    uploads: RefCell<Vec<Arc<Item>>>,
    frame: Cell<u32>,
    /// Kept between players so a frame of avatars allocates none: the wearer's body, an item.
    poses: RefCell<(Poses, Poses)>,
    /// Item files from a folder (`WISP_COSMETICS_LOCAL`): mannequins wear them too.
    local: bool,
    /// Show my cosmetics on its way to the service.
    mine: MineSync,
    /// Where each trail wearer was last tick, by entity id (`OWN_TRAIL` for the player's own).
    trail_last: FxHashMap<u32, [f64; 3]>,
    /// Ticks the effects have run: an aura's turn, an animated texture's frame.
    effect_ticks: u32,
    /// Which HD textures hold the atlas's HD cosmetic slots.
    hd_slots: Option<wisp_client::skins::SkinSlots>,
    /// What this player wears and plays, shared with other Wisp players through the service
    /// (`wisp_cosmetics::share`); None without one.
    pub(super) sharing: Option<wisp_cosmetics::share::Sharing>,
    /// "Show my cosmetics" as last published through `sharing`.
    shared_mine: Option<bool>,
    /// What the Wisp server's wallet said is worn when last looked at: a change (a purchase or
    /// a change in the Store) dresses the player again at once.
    worn_seen: Option<Vec<String>>,
}

/// The player's own avatar among the trail wearers (no entity id is this).
const OWN_TRAIL: u32 = u32::MAX;
/// How far (blocks) a wearer moves in a tick before its trail shows: standing still leaves none.
const TRAIL_MOVE: f64 = 0.03;

/// Sending Show my cosmetics: the wish it last looked at, the request on its way, and when to try
/// again after a failure (15 s, doubling to 10 minutes).
#[derive(Default)]
struct MineSync {
    wish: Option<bool>,
    sending: Option<Receiver<Result<(), String>>>,
    retry: Option<(Instant, Duration)>,
}

/// Makes the service keep `wish` for the signed-in account, unless it already confirmed it
/// (`prefs::confirmed`, in the game folder). An offline account has nothing to send.
fn send_mine(wish: bool) -> Result<(), String> {
    let Some(session) = wisp_client::auth::current() else {
        return Ok(());
    };
    let home = std::env::current_dir().map_err(|e| e.to_string())?;
    let uuid = format!("{:032x}", session.uuid);
    if prefs::confirmed(&home, &uuid).unwrap_or(true) == wish {
        return Ok(());
    }
    let api = wisp_compat::var("WISP_COSMETICS_API").unwrap_or_else(|_| wisp_cosmetics::API.into());
    let http = wisp_cosmetics::Http::new(&api, &format!("Wisp/{}", env!("CARGO_PKG_VERSION")));
    let kept = wisp_cosmetics::account::set_visible(
        &http,
        &session.name,
        |id| wisp_client::auth::join_server(&session, id).map_err(|e| format!("{e:?}")),
        wish,
    )?;
    prefs::set_confirmed(&home, &uuid, kept).map_err(|e| e.to_string())
}

/// The cosmetics service's address (`WISP_COSMETICS_API`, else the built-in one), if any.
fn service() -> Option<String> {
    wisp_compat::var("WISP_COSMETICS_API")
        .unwrap_or_else(|_| wisp_cosmetics::API.to_owned())
        .trim()
        .trim_end_matches('/')
        .to_owned()
        .into()
}

fn service_http(api: &str) -> wisp_cosmetics::Http {
    wisp_cosmetics::Http::new(api, &format!("Wisp/{}", env!("CARGO_PKG_VERSION")))
}

impl Cosmetics {
    /// Starts sharing with other Wisp players, when there is a service: the signed-in account
    /// publishes what it wears and its emotes; without an account it only sees other players'.
    fn start_sharing() -> Option<wisp_cosmetics::share::Sharing> {
        let api = service().filter(|a| !a.is_empty())?;
        let http = service_http(&api);
        let session = wisp_client::auth::current();
        let name = session.as_ref().map(|s| s.name.clone()).unwrap_or_default();
        let join = session.map(|session| -> wisp_cosmetics::share::Join {
            Box::new(move |id: &str| {
                wisp_client::auth::join_server(&session, id).map_err(|e| format!("{e:?}"))
            })
        });
        Some(wisp_cosmetics::share::Sharing::start(http, name, join))
    }

    /// The item files the player wears (the launcher's Shop folder), to publish.
    fn own_files() -> Vec<serde_json::Value> {
        wisp_compat::var_os("WISP_COSMETICS_LOCAL")
            .map(|dir| wisp_cosmetics::Local::new(dir.into()).worn_files())
            .unwrap_or_default()
    }

    fn client() -> (wisp_cosmetics::Cosmetics, bool) {
        let api = service().filter(|a| !a.is_empty());
        if let Some(dir) = wisp_compat::var_os("WISP_COSMETICS_LOCAL") {
            let mut local = wisp_cosmetics::Local::new(dir.into());
            // The launcher's shop: only its signed-in player wears what it bought.
            if let Ok(uuid) = wisp_compat::var("WISP_COSMETICS_LOCAL_PLAYER")
                && !uuid.is_empty()
            {
                local = local.only(&uuid);
                // The player's own items from the folder, everyone else's from the service.
                if let Some(api) = &api {
                    let cache = std::env::current_dir()
                        .unwrap_or_default()
                        .join("assets/cosmetics");
                    let mixed = wisp_cosmetics::Mixed {
                        local,
                        remote: service_http(api),
                    };
                    return (wisp_cosmetics::Cosmetics::new(mixed, Some(cache)), true);
                }
            }
            return (wisp_cosmetics::Cosmetics::new(local, None), true);
        }
        let api = wisp_compat::var("WISP_COSMETICS_API")
            .unwrap_or_else(|_| wisp_cosmetics::API.to_owned());
        let agent = format!("Wisp/{}", env!("CARGO_PKG_VERSION"));
        // Beside the downloaded skins (`assets/skins` in the game folder).
        let cache = std::env::current_dir()
            .unwrap_or_default()
            .join("assets/cosmetics");
        let http = wisp_cosmetics::Http::new(&api, &agent);
        (wisp_cosmetics::Cosmetics::new(http, Some(cache)), false)
    }
}

/// The game's model of an item's part.
fn part(p: &wisp_cosmetics::Part) -> Part {
    Part {
        name: p.name.clone(),
        pose: p.pose,
        polygons: p
            .faces
            .iter()
            .map(|f| Polygon {
                normal: f.normal,
                vertices: f.vertices,
                cull_back: false,
            })
            .collect(),
        children: p.children.iter().map(part).collect(),
    }
}

/// A texture in the top-left corner of a `side`×`side` RGBA slot image.
fn slot_image(t: &wisp_cosmetics::Texture, side: usize) -> Vec<u8> {
    let mut rgba = vec![0; side * side * 4];
    for y in 0..(t.height as usize).min(side) {
        let row = &t.rgba[y * t.width as usize * 4..(y + 1) * t.width as usize * 4];
        rgba[y * side * 4..y * side * 4 + row.len()].copy_from_slice(row);
    }
    rgba
}

/// The frame of `item`'s texture to show `ticks` into its animation.
fn frame_at(item: &Item, ticks: u32) -> usize {
    (ticks / item.frametime.max(1)) as usize % item.frames.len().max(1)
}

/// Which slots an avatar's gear covers: a helmet or head item hides a hat, an elytra what is
/// worn on the back. A name tag style is never drawn on the body. A back item replaces the
/// player's Mojang cape instead (`Game::cosmetic_on_back`): the player chose it in Wisp.
fn covered(m: &Mob, slot: Slot) -> bool {
    match slot {
        Slot::Head => m.extra.head.is_some(),
        // An elytra folds over the back, a weapon carried there included.
        Slot::Back | Slot::Weapon => m.extra.chest.as_deref() == Some("minecraft:elytra"),
        Slot::Shoulder => false,
        // A chestplate, not an elytra.
        Slot::Torso => m
            .extra
            .chest
            .as_deref()
            .is_some_and(|c| c != "minecraft:elytra"),
        // A name tag style and a trail are never drawn on the body.
        Slot::NameTag | Slot::Trail | Slot::Aura => true,
    }
}

/// An RGB colour as the particle options take it.
fn rgb([r, g, b]: [u8; 3]) -> [f32; 3] {
    [r, g, b].map(|c| f32::from(c) / 255.0)
}

/// One tick of `trail` behind a wearer who went from `from` to `to` (its feet).
fn emit_trail(
    particles: &mut wisp_client::particles::ParticleEngine,
    trail: &wisp_cosmetics::Trail,
    from: [f64; 3],
    to: [f64; 3],
) {
    use wisp_client::particles::ParticleOptions;
    use wisp_core::random::RandomSource;
    let head = rgb(trail.colours[0]);
    let tail = rgb(*trail.colours.last().unwrap_or(&trail.colours[0]));
    let moved = (0..3)
        .map(|i| (to[i] - from[i]).powi(2))
        .sum::<f64>()
        .sqrt();
    // The path, pulled back behind the wearer so the trail follows them instead of covering
    // their body (the camera of the view from behind looks straight through it).
    let flat = (to[0] - from[0]).hypot(to[2] - from[2]);
    let back = if flat > 1e-6 {
        [
            -(to[0] - from[0]) / flat * 0.45,
            0.0,
            -(to[2] - from[2]) / flat * 0.45,
        ]
    } else {
        [0.0; 3]
    };
    let along =
        |t: f64| std::array::from_fn::<f64, 3, _>(|i| from[i] + (to[i] - from[i]) * t + back[i]);
    // A small white mote: the sparkle (the end rod's sprite is far too large this close).
    let sparkle = ParticleOptions::dust([1.0, 1.0, 1.0], 0.45);
    match trail.style {
        wisp_cosmetics::TrailStyle::Stardust => {
            // Soft dust low behind the feet (dust drifts up a little by itself); faster steps
            // leave more of it.
            let count = 2 + (moved * 6.0).min(4.0) as usize;
            for _ in 0..count {
                let r = particles.random();
                let at = along(r.next_double());
                let colour = trail.colours
                    [r.next_int_bound(trail.colours.len() as i32).unwrap_or(0) as usize];
                let pos = [
                    at[0] + (r.next_double() - 0.5) * 0.45,
                    at[1] + 0.05 + r.next_double() * 0.3,
                    at[2] + (r.next_double() - 0.5) * 0.45,
                ];
                let scale = 0.45 + r.next_float() * 0.45;
                let dust = ParticleOptions::dust_transition(rgb(colour), tail, scale);
                particles.add_glowing(&dust, pos, [0.0, 0.0, 0.0]);
            }
            let r = particles.random();
            if r.next_float() < 0.4 {
                let at = along(r.next_double());
                let pos = [
                    at[0] + (r.next_double() - 0.5) * 0.5,
                    at[1] + 0.1 + r.next_double() * 0.5,
                    at[2] + (r.next_double() - 0.5) * 0.5,
                ];
                particles.add_glowing(&sparkle, pos, [0.0, 0.0, 0.0]);
            }
        }
        wisp_cosmetics::TrailStyle::Comet => {
            // An unbroken tail along the path, bright at the head and fading to the tail colour.
            let steps = ((moved / 0.12).ceil() as usize).clamp(1, 12);
            for k in 0..steps {
                let r = particles.random();
                let at = along((k as f64 + r.next_double()) / steps as f64);
                let pos = [
                    at[0] + (r.next_double() - 0.5) * 0.12,
                    at[1] + 0.12 + r.next_double() * 0.12,
                    at[2] + (r.next_double() - 0.5) * 0.12,
                ];
                let scale = 0.9 + r.next_float() * 0.4;
                let dust = ParticleOptions::dust_transition(head, tail, scale);
                particles.add_glowing(&dust, pos, [0.0, 0.0, 0.0]);
            }
            let r = particles.random();
            if r.next_float() < 0.3 {
                let at = along(r.next_double());
                particles.add_glowing(&sparkle, [at[0], at[1] + 0.2, at[2]], [0.0, 0.0, 0.0]);
            }
        }
    }
}

/// A random colour of `colours`.
fn pick(r: &mut wisp_core::random::LegacyRandom, colours: &[[u8; 3]]) -> [f32; 3] {
    use wisp_core::random::RandomSource;
    rgb(colours[r.next_int_bound(colours.len() as i32).unwrap_or(0) as usize])
}

/// One tick of `aura` around a wearer whose feet are at `at`; `ticks` counts the game's ticks.
fn emit_aura(
    particles: &mut wisp_client::particles::ParticleEngine,
    aura: &wisp_cosmetics::Aura,
    at: [f64; 3],
    ticks: u32,
) {
    use std::f64::consts::TAU;
    use wisp_client::particles::ParticleOptions;
    use wisp_core::random::RandomSource;
    let around = |angle: f64, radius: f64, height: f64| {
        [
            at[0] + angle.cos() * radius,
            at[1] + height,
            at[2] + angle.sin() * radius,
        ]
    };
    let t = f64::from(ticks);
    match aura.style {
        wisp_cosmetics::AuraStyle::Storm => {
            // A bolt now and then: a zigzag from a point beside the body, drawn in small motes,
            // its core the first colour.
            let r = particles.random();
            if r.next_float() < 0.75 {
                let mut angle = r.next_double() * TAU;
                let mut radius = 0.55 + r.next_double() * 0.25;
                let mut height = 0.5 + r.next_double() * 1.4;
                let down = if r.next_boolean() { -1.0 } else { 1.0 };
                let mut points = vec![around(angle, radius, height)];
                for _ in 0..4 {
                    angle += (r.next_double() - 0.5) * 0.6;
                    radius = (radius + (r.next_double() - 0.5) * 0.2).clamp(0.45, 0.9);
                    height += down * (0.12 + r.next_double() * 0.14);
                    points.push(around(angle, radius, height));
                }
                let colours = aura.colours.clone();
                for pair in points.windows(2) {
                    let (a, b) = (pair[0], pair[1]);
                    let length = (0..3).map(|i| (b[i] - a[i]).powi(2)).sum::<f64>().sqrt();
                    let steps = ((length / 0.04).ceil() as usize).max(1);
                    for k in 0..steps {
                        let f = k as f64 / steps as f64;
                        let pos = std::array::from_fn(|i| a[i] + (b[i] - a[i]) * f);
                        let r = particles.random();
                        let colour = if r.next_float() < 0.7 {
                            rgb(colours[0])
                        } else {
                            pick(r, &colours)
                        };
                        let scale = 0.55 + r.next_float() * 0.25;
                        particles.add_glowing(&ParticleOptions::dust(colour, scale), pos, [0.0; 3]);
                    }
                }
            }
            // Loose sparks crackling close to the body.
            let r = particles.random();
            if r.next_float() < 0.35 {
                let pos = around(
                    r.next_double() * TAU,
                    0.4 + r.next_double() * 0.4,
                    0.2 + r.next_double() * 1.7,
                );
                let colour = pick(r, &aura.colours);
                particles.add_glowing(&ParticleOptions::dust(colour, 0.5), pos, [0.0; 3]);
            }
        }
        wisp_cosmetics::AuraStyle::Orbit => {
            // Three stars circling the waist, gently rising and falling, each its own colour;
            // the dust they leave makes their path a ring.
            for k in 0..3 {
                let phase = f64::from(k) * TAU / 3.0;
                let angle = t * 0.16 + phase;
                let height = 1.0 + (t * 0.05 + phase).sin() * 0.18;
                let colour = rgb(aura.colours[k as usize % aura.colours.len()]);
                particles.add_glowing(
                    &ParticleOptions::dust(colour, 0.85),
                    around(angle, 0.8, height),
                    [0.0; 3],
                );
            }
            let r = particles.random();
            if r.next_float() < 0.25 {
                let pos = around(r.next_double() * TAU, 0.8, 0.9 + r.next_double() * 0.2);
                particles.add_glowing(&ParticleOptions::dust([1.0; 3], 0.4), pos, [0.0; 3]);
            }
        }
        wisp_cosmetics::AuraStyle::Nebula => {
            // Three strands of stardust spiralling from the feet to above the head, their colour
            // running through the colours as they climb; two motes a tick keep each unbroken.
            let n = aura.colours.len();
            for k in 0..3 {
                for half in [0.0, 0.5] {
                    let t = t - half;
                    let strand = f64::from(k) / 3.0;
                    let climb = (t * 0.03 + strand).rem_euclid(1.0);
                    let angle = t * 0.22 + strand * TAU;
                    let colour = rgb(aura.colours[((climb * n as f64) as usize).min(n - 1)]);
                    particles.add_glowing(
                        &ParticleOptions::dust(colour, 0.75),
                        around(angle, 0.7 - climb * 0.2, climb * 2.1),
                        [0.0; 3],
                    );
                }
            }
            // A faint mist of the last colour.
            let r = particles.random();
            if r.next_float() < 0.5 {
                let pos = around(
                    r.next_double() * TAU,
                    0.35 + r.next_double() * 0.35,
                    r.next_double() * 1.9,
                );
                let colour = rgb(aura.colours[n - 1]);
                particles.add_glowing(&ParticleOptions::dust(colour, 0.9), pos, [0.0; 3]);
            }
        }
    }
}

/// One tick of a shoulder or back item's sparkles: now and then a glowing mote around the pet,
/// which sits `item`'s attachment offset to the side of a wearer whose feet are at `at`, facing
/// `yaw`, or across the wingspan behind their back.
fn emit_sparkles(
    particles: &mut wisp_client::particles::ParticleEngine,
    item: &Item,
    at: [f64; 3],
    yaw: f32,
) {
    use wisp_client::particles::ParticleOptions;
    use wisp_core::random::RandomSource;
    let r = particles.random();
    if r.next_float() >= 0.3 {
        return;
    }
    // The model's +x is the wearer's left (pets sit on the left shoulder): (cos yaw, sin yaw)
    // in the world.
    let yaw = f64::from(yaw).to_radians();
    let side = f64::from(item.root.pose[0]) / 16.0;
    let scale = f64::from(item.root.pose[6]);
    let centre = [
        at[0] + yaw.cos() * side,
        at[1] + 1.5 + 0.35 * scale,
        at[2] + yaw.sin() * side,
    ];
    let pos = [
        centre[0] + (r.next_double() - 0.5) * 0.6,
        centre[1] + (r.next_double() - 0.5) * 0.5,
        centre[2] + (r.next_double() - 0.5) * 0.6,
    ];
    let pos = if item.slot == wisp_cosmetics::Slot::Back {
        // Behind the back, across both wings: (sin yaw, -cos yaw) is behind the wearer.
        let across = (r.next_double() - 0.5) * 2.2;
        let behind = 0.25 + r.next_double() * 0.25;
        [
            at[0] + yaw.sin() * behind + yaw.cos() * across,
            at[1] + 1.1 + r.next_double() * 0.9,
            at[2] - yaw.cos() * behind + yaw.sin() * across,
        ]
    } else {
        pos
    };
    let colour = pick(r, &item.sparkles);
    let size = 0.3 + r.next_float() * 0.25;
    particles.add_glowing(&ParticleOptions::dust(colour, size), pos, [0.0; 3]);
}

impl Game<'_> {
    /// Once a frame, before drawing: starts the service's client on first use and puts the
    /// textures the last frame wanted into the atlas.
    pub(super) fn cosmetics_frame(&mut self) {
        if !wisp_cosmetics::enabled() {
            return;
        }
        let c = &mut self.cosmetics;
        if c.client.is_none() {
            let (client, local) = Cosmetics::client();
            c.client = Some(client);
            c.local = local;
            c.sharing = Cosmetics::start_sharing();
        }
        // What this player wears, published again whenever Show my cosmetics changes (hidden:
        // nothing).
        let mine = self.settings.extras.cosmetics_mine;
        if let Some(sharing) = &c.sharing
            && c.shared_mine != Some(mine)
        {
            c.shared_mine = Some(mine);
            sharing.wear(if mine {
                Cosmetics::own_files()
            } else {
                Vec::new()
            });
        }
        // Bought or changed in the Store: the worn folder is written again, the player's own items
        // are read from it and published, without waiting for the next start.
        let worn_now = c
            .sharing
            .as_ref()
            .and_then(|s| s.shop_state().wallet)
            .map(|w| w.equipped);
        if let Some(worn) = worn_now
            && c.worn_seen.as_ref() != Some(&worn)
        {
            let first = c.worn_seen.is_none();
            c.worn_seen = Some(worn.clone());
            if !first && let Some(dir) = wisp_compat::var_os("WISP_COSMETICS_LOCAL") {
                let wallet = wisp_shop::Wallet {
                    equipped: worn,
                    ..Default::default()
                };
                if wallet.dress(std::path::Path::new(&dir)).is_ok() {
                    let (client, local) = Cosmetics::client();
                    c.client = Some(client);
                    c.local = local;
                    c.baked.borrow_mut().clear();
                    c.shared_mine = None;
                }
            }
        }
        c.frame.set(c.frame.get().wrapping_add(1));
        // Show my cosmetics: sent once per change of the wish, then retried until confirmed.
        if !c.local {
            let wish = self.settings.extras.cosmetics_mine;
            let m = &mut c.mine;
            if let Some(done) = m.sending.as_ref().and_then(|rx| rx.try_recv().ok()) {
                m.sending = None;
                m.retry = match done {
                    Ok(()) => None,
                    Err(_) => {
                        let wait = m.retry.map_or(Duration::from_secs(15), |(_, w)| {
                            (w * 2).min(Duration::from_secs(600))
                        });
                        Some((Instant::now() + wait, wait))
                    }
                };
            }
            let due = m.wish != Some(wish) || m.retry.is_some_and(|(at, _)| Instant::now() >= at);
            if due && m.sending.is_none() {
                m.wish = Some(wish);
                let (tx, rx) = channel();
                m.sending = Some(rx);
                std::thread::Builder::new()
                    .name("Cosmetics account".into())
                    .spawn(move || {
                        let _ = tx.send(send_mine(wish));
                    })
                    .ok();
            }
        }
        let ticks = c.effect_ticks;
        let uploads = std::mem::take(&mut *c.uploads.borrow_mut());
        for item in uploads {
            let location = format!("cosmetics/{:x}", Arc::as_ptr(&item) as usize);
            if !self.gui.atlas.sprites.contains_key(&location) {
                let frame = frame_at(&item, ticks);
                self.store_cosmetic(location, &item, frame);
                if let Some(b) = self
                    .cosmetics
                    .baked
                    .borrow()
                    .get(&(Arc::as_ptr(&item) as usize))
                {
                    b.shown.set(Some(frame));
                }
            }
        }
        // Animated textures drawn lately: their slot takes the frame that is due.
        let now = self.cosmetics.frame.get();
        let due: Vec<(String, Arc<Item>, usize)> = self
            .cosmetics
            .baked
            .borrow()
            .values()
            .filter(|b| b.item.frames.len() > 1 && now.wrapping_sub(b.marked.get()) <= 1)
            .filter_map(|b| {
                let frame = frame_at(&b.item, ticks);
                (b.shown.get() != Some(frame) && self.gui.atlas.sprites.contains_key(&b.location))
                    .then(|| {
                        b.shown.set(Some(frame));
                        (b.location.clone(), b.item.clone(), frame)
                    })
            })
            .collect();
        for (location, item, frame) in due {
            self.store_cosmetic(location, &item, frame);
        }
        // Items no player wears any more.
        self.cosmetics
            .baked
            .borrow_mut()
            .retain(|_, b| Arc::strong_count(&b.item) > 1);
    }

    /// Puts frame `frame` of `item`'s texture into its atlas slot: a skin slot, or for an HD
    /// texture one of the HD cosmetic slots (`hud::HD_COSMETIC_SLOTS`).
    fn store_cosmetic(&mut self, location: String, item: &Item, frame: usize) {
        let texture = &item.frames[frame.min(item.frames.len() - 1)];
        if !hd(item) {
            self.store_skin(location, slot_image(texture, 64));
            return;
        }
        let side = wisp_client::hud::HD_SLOT_SIZE;
        let slots = self.cosmetics.hd_slots.get_or_insert_with(|| {
            wisp_client::skins::SkinSlots::new(wisp_client::hud::HD_COSMETIC_SLOTS)
        });
        let Some((slot, evicted)) = slots.place(location.clone()) else {
            return;
        };
        if let Some(old) = evicted {
            self.gui.atlas.sprites.remove(&old);
        }
        let Some(rect) = self
            .gui
            .atlas
            .sprites
            .get(&format!("wisp/cosmetic_hd{slot}"))
            .copied()
        else {
            return;
        };
        let (w, h) = (self.gui.atlas.width as f32, self.gui.atlas.height as f32);
        self.map_patches.push(wisp_client::font::Patch {
            x: (rect.u0 * w).round() as u32,
            y: (rect.v0 * h).round() as u32,
            width: side,
            height: side,
            rgba: slot_image(texture, side as usize),
        });
        self.gui.atlas.sprites.insert(location, rect);
    }

    /// Once a tick: the trails, auras and pet sparkles of the player and the players in view.
    pub(super) fn tick_cosmetic_trails(&mut self) {
        if !wisp_cosmetics::enabled() || self.cosmetics.client.is_none() {
            return;
        }
        self.cosmetics.effect_ticks = self.cosmetics.effect_ticks.wrapping_add(1);
        // Who wears a trail, an aura or a sparkling pet: id, feet, body yaw and the items.
        let effects = |worn: wisp_cosmetics::Worn| {
            worn.iter()
                .any(|i| i.trail.is_some() || i.aura.is_some() || !i.sparkles.is_empty())
                .then_some(worn)
        };
        let mut wearers: Vec<(u32, [f64; 3], f32, wisp_cosmetics::Worn)> = Vec::new();
        if let Some(m) = self.player_model.as_ref()
            && !self.own_invisible()
            && let Some(worn) = self.worn(m, true).and_then(effects)
        {
            let p = m.position;
            wearers.push((OWN_TRAIL, [p.x, p.y, p.z], m.body_yaw, worn));
        }
        // Other players' effects only with "Others' auras and trails" on (off by default).
        let extras = &self.settings.extras;
        if extras.cosmetics_others == Others::Shown && extras.cosmetics_auras {
            let own = self.remote.as_ref().map(|r| r.entity_id as u32);
            // Mannequins wear the local item files (`worn`), so their effects show too.
            let local = self.cosmetics.local;
            for m in &self.mobs {
                let wearer = &*m.kind.id == "minecraft:player"
                    || (local && &*m.kind.id == "minecraft:mannequin");
                if !wearer || Some(m.id) == own || wisp_client::mobs::trader::is_invisible(m) {
                    continue;
                }
                if let Some(worn) = self.worn(m, false).and_then(effects) {
                    let p = m.position;
                    wearers.push((m.id, [p.x, p.y, p.z], m.body_yaw, worn));
                }
            }
        }
        let ticks = self.cosmetics.effect_ticks;
        let last = std::mem::take(&mut self.cosmetics.trail_last);
        for (id, at, yaw, worn) in &wearers {
            for item in worn.iter() {
                if let Some(trail) = item.trail.as_ref()
                    && let Some(from) = last.get(id).copied()
                {
                    let moved = (0..3)
                        .map(|i| (at[i] - from[i]).powi(2))
                        .sum::<f64>()
                        .sqrt();
                    // A teleport isn't a step: no trail across it.
                    if moved > TRAIL_MOVE && moved < 4.0 {
                        emit_trail(&mut self.particles, trail, from, *at);
                    }
                }
                if let Some(aura) = item.aura.as_ref() {
                    emit_aura(&mut self.particles, aura, *at, ticks);
                }
                if !item.sparkles.is_empty() {
                    emit_sparkles(&mut self.particles, item, *at, *yaw);
                }
            }
            self.cosmetics.trail_last.insert(*id, *at);
        }
    }

    /// The name tag style `m` wears, if cosmetics are shown for it (name tags only counts): its
    /// mark's place in the atlas and its line's colour. None until the mark is in the atlas.
    pub(super) fn cosmetics_tag(&self, m: &Mob, own: bool) -> Option<super::names::TagStyle> {
        let shown = self.cosmetics_shown(own);
        let worn = self.worn_by(m, own)?;
        let item = worn
            .iter()
            .find(|i| shown && i.slot == Slot::NameTag)?;
        let [r, g, b] = item.tag?.line;
        let frame = self.cosmetics.frame.get();
        let mut baked = self.cosmetics.baked.borrow_mut();
        let entry = baked
            .entry(Arc::as_ptr(item) as usize)
            .or_insert_with(|| Baked::new(item, frame));
        let Some(slot) = self.gui.atlas.sprites.get(&entry.location) else {
            self.cosmetics.uploads.borrow_mut().push(item.clone());
            return None;
        };
        if entry.marked.replace(frame) != frame {
            self.skin_downloads.borrow_mut().drawn(&entry.location);
        }
        // The mark sits in the slot's top left corner.
        let mut mark = *slot;
        let side = slot_side(item);
        mark.u1 = mark.u0 + (slot.u1 - slot.u0) * item.texture.width as f32 / side;
        mark.v1 = mark.v0 + (slot.v1 - slot.v0) * item.texture.height as f32 / side;
        Some(super::names::TagStyle {
            mark,
            line: u32::from_be_bytes([0, r, g, b]),
        })
    }

    /// What `m` wears, if cosmetics are shown for it: `own` for the player's own model.
    fn worn(&self, m: &Mob, own: bool) -> Option<wisp_cosmetics::Worn> {
        self.cosmetics_shown(own)
            .then(|| self.worn_by(m, own))
            .flatten()
    }

    /// Whether the settings show the player's own cosmetics (`own`) or others'.
    fn cosmetics_shown(&self, own: bool) -> bool {
        let extras = &self.settings.extras;
        if own {
            extras.cosmetics_mine
        } else {
            extras.cosmetics_others != Others::Hidden
        }
    }

    /// What `m` wears, whatever the settings show.
    fn worn_by(&self, m: &Mob, own: bool) -> Option<wisp_cosmetics::Worn> {
        let client = self.cosmetics.client.as_ref()?;
        let uuid = if own {
            // Cosmetics belong to the Minecraft account. In singleplayer the integrated server
            // names its own player by an offline UUID, so the signed-in account's comes first.
            match wisp_client::auth::current() {
                Some(session) => session.uuid,
                None => self.remote.as_ref().map(|r| r.profile())?,
            }
        } else if &*m.kind.id == "minecraft:player" {
            self.remote_player_uuid(m.id)?
        } else if self.cosmetics.local && &*m.kind.id == "minecraft:mannequin" {
            u128::from(m.id)
        } else {
            return None;
        };
        client.frame().worn(uuid)
    }

    /// `m` (the player's own avatar when `own`) has a back item drawn this frame, which takes the
    /// place of its Mojang cape.
    pub(super) fn cosmetic_on_back(&self, m: &Mob, own: bool) -> bool {
        let bodies = own || self.settings.extras.cosmetics_others == Others::Shown;
        bodies
            && !covered(m, Slot::Back)
            && self
                .worn(m, own)
                .is_some_and(|worn| worn.iter().any(|item| item.slot == Slot::Back))
    }

    /// The cosmetics `m` wears, on its bones. `gpu`: posed on the GPU (the mob pass's skin
    /// frame, begun); the bounds of what went there come back, for culling. Else the vertices
    /// go to `out`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn cosmetics_geometry(
        &self,
        m: &Mob,
        own: bool,
        base: glam::Mat4,
        alpha: f32,
        light: u32,
        gpu: bool,
        out: &mut Vec<wisp_client::mesher::Vertex>,
    ) -> Option<(glam::Vec3, glam::Vec3)> {
        use wisp_client::mobs::{anim, render};
        let worn = self.worn(m, own)?;
        if wisp_client::mobs::trader::is_invisible(m) {
            return None;
        }
        // The avatar's body, posed as it is drawn.
        let (key, _) = render::body(m);
        let body = m.kind.layer(&key).unwrap_or(&m.kind.model);
        let mut scratch = self.cosmetics.poses.borrow_mut();
        let (body_poses, poses) = &mut *scratch;
        body_poses.reset(body);
        let data = self.res.mob_data();
        anim::setup(m, &anim::state(m, alpha), body_poses, &data.animations);
        let ticks = m.age as f32 + alpha;
        let frame = self.cosmetics.frame.get();
        let mut bounds = None;
        // Items on one bone (a back item, a shoulder pet and a shirt ride the body) share its
        // matrix: the head, the body and the two arms at most.
        let mut bones: [Option<(wisp_cosmetics::Bone, glam::Mat4)>; 4] = [None; 4];
        // "Name tags only": other players' styles beside their names, nothing on their bodies.
        let bodies = own || self.settings.extras.cosmetics_others == Others::Shown;
        for item in worn.iter() {
            if !bodies || covered(m, item.slot) {
                continue;
            }
            let mut baked = self.cosmetics.baked.borrow_mut();
            let b = baked
                .entry(Arc::as_ptr(item) as usize)
                .or_insert_with(|| Baked::new(item, frame));
            let Some(rect) = self.gui.atlas.sprites.get(&b.location) else {
                self.cosmetics.uploads.borrow_mut().push(item.clone());
                continue;
            };
            if b.marked.replace(frame) != frame {
                self.skin_downloads.borrow_mut().drawn(&b.location);
            }
            // A glowing item (a star pet) is lit as if by full block and sky light.
            let light = if item.glow { 240 | 240 << 8 } else { light };
            // The texture fills the top-left of its 64×64 slot.
            let side = slot_side(item);
            let (w, h) = (
                item.texture.width as f32 / side,
                item.texture.height as f32 / side,
            );
            let uv = [
                rect.u0,
                rect.v0,
                rect.u0 + (rect.u1 - rect.u0) * w,
                rect.v0 + (rect.v1 - rect.v0) * h,
            ];
            for (bone_of, model) in &b.models {
                let known = bones.iter().flatten().find(|(b, _)| b == bone_of);
                let bone = match known {
                    Some(&(_, matrix)) => matrix,
                    None => {
                        let Some(matrix) =
                            render::part_matrix(body, base, body_poses, bone_of.part(), 0.0)
                        else {
                            continue;
                        };
                        if let Some(free) = bones.iter_mut().find(|b| b.is_none()) {
                            *free = Some((*bone_of, matrix));
                        }
                        matrix
                    }
                };
                poses.reset(model);
                item.animate(ticks, |name, pose| *poses.get(name) = pose);
                // Wavy Capes: a cape on the back is bent anew each frame, so it is drawn here
                // and not through the GPU path's cached model.
                let wavy = (item.slot == Slot::Back && self.settings.extras.module("wavy_capes"))
                    .then(|| render::wavy_worn_cape(model, m, alpha))
                    .flatten();
                if let Some(bent) = &wavy {
                    wisp_client::mobs::render_model(
                        bent,
                        bone,
                        &|p: &Part| poses.pose(p),
                        uv,
                        light,
                        0,
                        out,
                    );
                    continue;
                }
                if gpu {
                    let mut models = self.skin_models.borrow_mut();
                    let mut frame = self.skin_frame.borrow_mut();
                    let mut skin = wisp_client::mobs::skin::Skin::new(&mut models, &mut frame);
                    skin.bounds = bounds;
                    let paint = wisp_client::mobs::skin::Paint {
                        uv,
                        scroll: 0.0,
                        color: 0xFFFF_FFFF,
                        light,
                        overlay: 0,
                    };
                    if skin.draw(model, bone, poses, paint) {
                        bounds = skin.bounds;
                        continue;
                    }
                }
                wisp_client::mobs::render_model(
                    model,
                    bone,
                    &|p: &Part| poses.pose(p),
                    uv,
                    light,
                    0,
                    out,
                );
            }
        }
        bounds
    }
}
