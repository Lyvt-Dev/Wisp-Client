//! The HUD widgets in the game (`docs/extras.md`): what each shows, where it sits and its items
//! and effect icons; the plates themselves are `wisp_client::hud_widgets`. A widget without a
//! place yet stacks in the top left. In the HUD editor every widget shows, an empty one with
//! an example, so it can be placed.
use super::{Game, Screen};
use std::sync::Arc;
use wisp_client::{
    extras::features::{self, FEATURES},
    hud::HudBatch,
    hud_widgets::{self, Content, Draw, Durability, Effect, Keys, Piece, Style},
    items::ItemStack,
    lang,
    ui_theme::Ui,
};

/// The gap between auto-placed widgets and from the screen's edge, in GUI pixels.
const GAP: f32 = 3.0;
/// How long a widget keeps its widest plate after its text got shorter.
const STEADY: std::time::Duration = std::time::Duration::from_millis(1500);
/// How long a key of Keystrokes takes to fade after it is let go, in seconds.
const KEY_FADE: f32 = 0.18;

/// What the widgets remember between frames: each one's widest plate lately, and when each key
/// of Keystrokes was let go.
#[derive(Default)]
pub(super) struct WidgetMemory {
    widths: std::collections::HashMap<&'static str, (f32, std::time::Instant)>,
    keys: [(bool, Option<std::time::Instant>); 7],
}
impl WidgetMemory {
    /// The plate's width for `key`: its widest lately, so a number that gets shorter for a moment
    /// doesn't shake it.
    fn steady(&mut self, key: &'static str, w: f32) -> f32 {
        let now = std::time::Instant::now();
        match self.widths.get(key) {
            Some(&(kept, at)) if kept > w && now.duration_since(at) < STEADY => kept,
            _ => {
                self.widths.insert(key, (w, now));
                w
            }
        }
    }
    /// How pressed key `i` looks: 1 held, fading out after.
    fn key(&mut self, i: usize, held: bool) -> f32 {
        let (was, released) = &mut self.keys[i];
        if *was && !held {
            *released = Some(std::time::Instant::now());
        }
        *was = held;
        if held {
            return 1.0;
        }
        released.map_or(0.0, |at| {
            (1.0 - at.elapsed().as_secs_f32() / KEY_FADE).max(0.0)
        })
    }
}

fn stack(id: &str) -> ItemStack {
    ItemStack {
        item: Arc::from(id),
        count: 1,
        damage: 0,
        extra: None,
    }
}

impl Game<'_> {
    fn widget_style(&self, key: &str) -> Style {
        let e = &self.settings.extras;
        Style {
            background: e.opt_bool(key, "background"),
            opacity: e.opt_f32(key, "opacity") / 100.0,
            text: e.opt_color(key, "text"),
            accent: e.opt_color(key, "accent"),
            shadow: e.opt_bool(key, "shadow"),
        }
    }
    fn durability_of(&self, s: &ItemStack) -> Option<(u32, u32)> {
        let default_max = self.items().get(&s.item).map_or(0, |i| i.max_damage);
        let max = wisp_client::item_render::cit::stack_max_damage(s, default_max);
        (max > 0).then(|| (max.saturating_sub(s.damage), max))
    }
    /// What widget `key` shows now (None: nothing, outside the editor), and the items it names.
    fn widget_content(&mut self, key: &str, editing: bool) -> Option<(Content, Vec<ItemStack>)> {
        let e = self.settings.extras.clone();
        let value = |label: &str, value: String| Content::Value {
            label: label.to_owned(),
            value,
        };
        let t = |k: &str, fallback: &str| lang::get_or(k, fallback);
        let content = match key {
            "hud_fps" => {
                let (fps, _, low) = self.frame_reading();
                let v = if e.opt_bool(key, "low") {
                    format!("{fps}  \u{b7}  {low} low")
                } else {
                    fps.to_string()
                };
                value("FPS", v)
            }
            "hud_cps" => {
                let ((l, r), _) = self.mouse_reading();
                let v = if e.opt_bool(key, "right") {
                    format!("{l} | {r}")
                } else {
                    l.to_string()
                };
                value("CPS", v)
            }
            "hud_keystrokes" => {
                use winit::keyboard::KeyCode;
                let ((l, r), mouse) = self.mouse_reading();
                let held = [
                    self.keys.contains(&KeyCode::KeyW),
                    self.keys.contains(&KeyCode::KeyA),
                    self.keys.contains(&KeyCode::KeyS),
                    self.keys.contains(&KeyCode::KeyD),
                    self.keys.contains(&KeyCode::Space),
                    mouse[0],
                    mouse[1],
                ];
                let m = &mut self.extras.widget_memory;
                let [w, a, s, d, space, lmb, rmb] = std::array::from_fn(|i| m.key(i, held[i]));
                Content::Keys(Keys {
                    w,
                    a,
                    s,
                    d,
                    space,
                    lmb,
                    rmb,
                    cps: e.opt_bool(key, "cps").then_some((l, r)),
                    mouse: e.opt_bool(key, "mouse"),
                    space_bar: e.opt_bool(key, "space"),
                })
            }
            "hud_ping" => Content::Ping {
                label: t("wisp.widget.ping", "Ping"),
                ms: (!self.has_singleplayer_server())
                    .then(|| self.own_latency())
                    .flatten(),
                bars: e.opt_bool(key, "bars"),
            },
            "hud_coords" => {
                let p = self.player.as_ref()?.position;
                let [x, y, z] = [p.x, p.y, p.z].map(|v| v.floor() as i64);
                let facing = e.opt_bool(key, "direction").then(|| self.facing_text());
                if e.opt_choice(key, "layout") == 0 {
                    let mut v = format!("{x}, {y}, {z}");
                    if let Some(f) = facing {
                        v.push_str(&format!("  {f}"));
                    }
                    value("XYZ", v)
                } else {
                    let mut rows = vec![
                        ("X".to_owned(), x.to_string()),
                        ("Y".to_owned(), y.to_string()),
                        ("Z".to_owned(), z.to_string()),
                    ];
                    if let Some(f) = facing {
                        rows.push(("F".to_owned(), f));
                    }
                    Content::Rows(rows)
                }
            }
            "hud_direction" => value(&t("wisp.widget.facing", "Facing"), self.facing_text()),
            "hud_biome" => value(&t("wisp.widget.biome", "Biome"), self.biome_text()?),
            "hud_speed" => {
                let p = self.player.as_ref()?;
                let bps = p.velocity.x.hypot(p.velocity.z) * 20.0;
                let v = if e.opt_choice(key, "unit") == 1 {
                    format!("{:.1} km/h", bps * 3.6)
                } else {
                    format!("{bps:.2} b/s")
                };
                value(&t("wisp.widget.speed", "Speed"), v)
            }
            "hud_clock" => {
                let fmt = match (e.opt_choice(key, "format"), e.opt_bool(key, "seconds")) {
                    (1, true) => "%I:%M:%S %p",
                    (1, false) => "%I:%M %p",
                    (_, true) => "%H:%M:%S",
                    (_, false) => "%H:%M",
                };
                value(
                    &t("wisp.widget.time", "Time"),
                    chrono::Local::now().format(fmt).to_string(),
                )
            }
            "hud_day_time" => {
                let ticks = self.world.day_time;
                let of_day = (ticks.rem_euclid(24000) + 6000) % 24000;
                let mut v = format!("{:02}:{:02}", of_day / 1000, of_day % 1000 * 60 / 1000);
                if e.opt_bool(key, "day") {
                    v.push_str(&format!(
                        "  \u{b7}  {} {}",
                        t("wisp.widget.day", "Day"),
                        ticks.div_euclid(24000)
                    ));
                }
                value(&t("wisp.widget.world", "World"), v)
            }
            "hud_weather" => {
                let w = &self.world.weather;
                let v = if w.thunder_level.1 > 0.9 && w.rain_level.1 > 0.2 {
                    t("wisp.widget.thunder", "Thunder")
                } else if w.rain_level.1 > 0.2 {
                    t("wisp.widget.rain", "Rain")
                } else {
                    t("wisp.widget.clear", "Clear")
                };
                value(&t("wisp.widget.weather", "Weather"), v)
            }
            "hud_server" => {
                let v = match (self.has_singleplayer_server(), self.server_address()) {
                    (false, Some(a)) => wisp_client::net::shown_address(a).to_string(),
                    _ => t("wisp.module.line.singleplayer", "Singleplayer"),
                };
                value(&t("wisp.widget.server", "Server"), v)
            }
            "hud_effects" => {
                let mut effects: Vec<Effect> = self
                    .potions
                    .effects
                    .list
                    .iter()
                    .filter(|x| x.visible)
                    .map(|x| {
                        let name = lang::get(&wisp_client::brewing::description_id(x.id));
                        let level = x.amplifier + 1;
                        let name = if level > 1 {
                            format!("{name} {}", roman(level))
                        } else {
                            name
                        };
                        let (time, ending) = if x.duration < 0 {
                            ("\u{221e}".to_owned(), false)
                        } else {
                            let s = x.duration / 20;
                            (format!("{}:{:02}", s / 60, s % 60), x.duration < 200)
                        };
                        Effect {
                            sprite: format!(
                                "mob_effect/{}",
                                x.id.rsplit(':').next().unwrap_or(x.id)
                            ),
                            name,
                            time,
                            ending,
                        }
                    })
                    .collect();
                if effects.is_empty() {
                    if !editing {
                        return None;
                    }
                    effects.push(Effect {
                        sprite: "mob_effect/speed".into(),
                        name: lang::get("effect.minecraft.speed"),
                        time: "1:30".into(),
                        ending: false,
                    });
                }
                Content::Effects {
                    effects,
                    horizontal: e.opt_choice(key, "layout") == 1,
                    names: e.opt_bool(key, "names"),
                }
            }
            "armor_status" => {
                let mut stacks: Vec<ItemStack> = self.armor.iter().flatten().cloned().collect();
                if e.opt_bool(key, "mainhand")
                    && let Some(s) = self.inventory.get(self.selected).and_then(Option::as_ref)
                {
                    stacks.push(s.clone());
                }
                if e.opt_bool(key, "offhand")
                    && let Some(s) = &self.offhand
                {
                    stacks.push(s.clone());
                }
                if stacks.is_empty() {
                    if !editing {
                        return None;
                    }
                    stacks = ["helmet", "chestplate", "leggings", "boots"]
                        .iter()
                        .map(|p| stack(&format!("minecraft:diamond_{p}")))
                        .collect();
                }
                let pieces = stacks
                    .iter()
                    .enumerate()
                    .map(|(i, s)| Piece {
                        item: i,
                        durability: self.durability_of(s),
                    })
                    .collect();
                let content = Content::Armor {
                    pieces,
                    horizontal: e.opt_choice(key, "layout") == 1,
                    durability: Durability::from_index(e.opt_choice(key, "durability")),
                };
                return Some((content, stacks));
            }
            "hud_durability" => {
                let held = self
                    .inventory
                    .get(self.selected)
                    .and_then(Option::as_ref)
                    .filter(|s| self.durability_of(s).is_some())
                    .cloned();
                let s = match held {
                    Some(s) => s,
                    None if editing => stack("minecraft:iron_pickaxe"),
                    None => return None,
                };
                let content = Content::Armor {
                    pieces: vec![Piece {
                        item: 0,
                        durability: self.durability_of(&s),
                    }],
                    horizontal: false,
                    durability: Durability::from_index(e.opt_choice(key, "durability")),
                };
                return Some((content, vec![s]));
            }
            "hud_arrows" => {
                let held = self.inventory.get(self.selected).and_then(Option::as_ref);
                let bow = held
                    .is_some_and(|s| matches!(&*s.item, "minecraft:bow" | "minecraft:crossbow"));
                if e.opt_bool(key, "bow") && !bow && !editing {
                    return None;
                }
                let count: u32 = self
                    .inventory
                    .iter()
                    .chain(std::iter::once(&self.offhand))
                    .flatten()
                    .filter(|s| s.item.ends_with("arrow"))
                    .map(|s| s.count)
                    .sum();
                return Some((
                    Content::Count {
                        item: 0,
                        count: count.to_string(),
                    },
                    vec![stack("minecraft:arrow")],
                ));
            }
            "hit_info" => {
                let (reach, combo) = self.hit_reading();
                let mut rows = Vec::new();
                if e.opt_bool(key, "reach") {
                    let r = reach.map_or_else(|| "-".to_owned(), |r| format!("{r:.2}"));
                    rows.push((t("wisp.widget.reach", "Reach"), r));
                }
                if e.opt_bool(key, "combo") {
                    rows.push((t("wisp.widget.combo", "Combo"), combo.to_string()));
                }
                if rows.is_empty() {
                    return None;
                }
                Content::Rows(rows)
            }
            _ => return None,
        };
        Some((content, Vec::new()))
    }
    /// The facing as a compass point and the yaw.
    pub(super) fn facing_text(&self) -> String {
        let yaw = self.yaw.rem_euclid(360.0);
        let names = ["S", "SW", "W", "NW", "N", "NE", "E", "SE"];
        let i = ((f64::from(yaw) / 45.0 + 0.5).floor() as usize) % 8;
        format!("{} ({yaw:.0}\u{b0})", names[i])
    }
    /// The biome at the feet, its id made readable ("Dark Forest").
    pub(super) fn biome_text(&self) -> Option<String> {
        let p = self.player.as_ref()?;
        let [x, y, z] = [p.position.x, p.position.y, p.position.z].map(|v| v.floor() as i32);
        if y < self.world.bottom_y() || y >= self.world.top_y() {
            return None;
        }
        let chunk = self.world.chunks.get(&(x >> 4, z >> 4))?;
        let biome = self.res.biomes().list.get(chunk.biome(x, y, z) as usize)?;
        let name = biome.name.rsplit(':').next().unwrap_or(&biome.name);
        Some(
            name.split('_')
                .map(|w| {
                    let mut c = w.chars();
                    c.next()
                        .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
                })
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
    /// Draws every widget that is on (all of them, examples included, in the editor) and keeps
    /// where each went for the editor.
    pub(super) fn draw_hud_widgets(&mut self, batch: &mut HudBatch) {
        let editing = self.screen == Screen::HudEditor;
        let mut rects = Vec::new();
        let mut auto_y = GAP;
        for f in FEATURES.iter().filter(|f| f.widget) {
            if !self.settings.extras.feature_on(f.key) {
                continue;
            }
            let Some((content, stacks)) = self.widget_content(f.key, editing) else {
                continue;
            };
            let style = self.widget_style(f.key);
            let scale = self.settings.extras.opt_f32(f.key, "scale");
            // A widget's dp follows the GUI scale: 2 dp to a GUI pixel at size 1.
            let density = batch.scale * 0.5 * scale;
            let cursor = self.cursor;
            let anchor = self.settings.extras.widget_anchor(f.key);
            let pos = self.settings.extras.widget_pos(f.key);
            let memory = &mut self.extras.widget_memory;
            let atlas = &self.gui.atlas;
            let (draws, rect) = {
                let mut ui = Ui::with_density(atlas, batch, cursor, density);
                let mut out = Vec::new();
                let (w, h) =
                    hud_widgets::paint(&mut ui, 0.0, 0.0, &content, &style, false, 0.0, &mut out);
                let w = memory.steady(f.key, w);
                let gpd = ui.gui_per_dp();
                let (gw, gh) = (w * gpd, h * gpd);
                let (sw, sh) = (ui.w * gpd, ui.h * gpd);
                let (gx, gy) = match (anchor, pos) {
                    (Some((fx, fy)), _) => (
                        hud_widgets::anchor_place(fx, gw, sw),
                        hud_widgets::anchor_place(fy, gh, sh),
                    ),
                    (None, Some((fx, fy))) => (
                        (fx * sw).clamp(0.0, (sw - gw).max(0.0)),
                        (fy * sh).clamp(0.0, (sh - gh).max(0.0)),
                    ),
                    (None, None) => {
                        let at = (GAP, auto_y);
                        auto_y += gh + GAP;
                        at
                    }
                };
                hud_widgets::paint(
                    &mut ui,
                    gx / gpd,
                    gy / gpd,
                    &content,
                    &style,
                    true,
                    w,
                    &mut out,
                );
                ui.finish();
                (out, [gx, gy, gw, gh])
            };
            for d in draws {
                let start = batch.vertices.len();
                match d {
                    Draw::Item { item, x, y, size } => {
                        if let Some(s) = stacks.get(item) {
                            self.draw_item_model(batch, s, x, y);
                            batch.scale_since(start, x, y, size / 16.0, size / 16.0);
                        }
                    }
                    Draw::Sprite { name, x, y, size } => {
                        batch.sprite(&self.gui.atlas, &name, x, y);
                        batch.scale_since(start, x, y, size / 18.0, size / 18.0);
                    }
                }
            }
            rects.push((f.key, rect));
        }
        *self.extras.widget_rects.borrow_mut() = rects;
    }
    /// The widget under `(x, y)` (GUI pixels) and its rectangle, the topmost first.
    pub(super) fn widget_at(&self, x: f32, y: f32) -> Option<(&'static str, [f32; 4])> {
        self.extras
            .widget_rects
            .borrow()
            .iter()
            .rev()
            .find(|(_, [rx, ry, rw, rh])| x >= *rx && x < rx + rw && y >= *ry && y < ry + rh)
            .copied()
    }
    /// The widget's name, for the editor.
    pub(super) fn widget_name(key: &str) -> String {
        features::texts(key).0
    }
}

/// Levels as Roman numerals, as vanilla writes them (`enchantment.level.N` up to 10).
fn roman(n: i32) -> String {
    const R: [&str; 10] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    usize::try_from(n - 1)
        .ok()
        .and_then(|i| R.get(i))
        .map_or_else(|| n.to_string(), |s| (*s).to_owned())
}
