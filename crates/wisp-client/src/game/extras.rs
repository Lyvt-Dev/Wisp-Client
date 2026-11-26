//! Wisp's extras in the game (`docs/extras.md`); a native mod's minimap
//! (`native_minimap`: its tiles, image and HUD) and zoom (`native_zoom`) on the camera; clicks
//! and frame times for mods (`native_clicks`, `native_frame_rate`).
//! Each hook does nothing while its extra is off and no mod uses it.
use super::{Dim, Game, Screen, native_hooks::NativeMinimap};
use std::time::Duration;
use winit::event::MouseButton;
use wisp_client::{
    extras::{Corner, Cps, FpsCounter, HudPanel, fov_target},
    extras_hud::{self, Corners},
    hud::{HudBatch, MINIMAP_SPRITE},
    minimap::{self, Layer, Minimap, View},
    multiplayer,
    reconnect::{Retry, Tracker},
    title::Mode,
};

#[derive(Default)]
pub(super) struct ExtrasState {
    fps: FpsCounter,
    /// Left and right mouse buttons: held, and their clicks.
    mouse: [bool; 2],
    cps: [Cps; 2],
    minimap: Minimap,
    /// The dimension the minimap's tiles belong to.
    minimap_dim: Option<Dim>,
    /// Auto-reconnect's attempts at the current server.
    reconnect: Tracker,
    /// Freelook while its key is held.
    freelook: Option<Freelook>,
    /// Damage Numbers: each mob's health when last seen, and the numbers rising.
    last_health: std::collections::HashMap<u32, f32>,
    pub(super) damage_numbers: Vec<DamageNumber>,
    /// Hit Info: the reach of the last hit, the combo and when it last grew.
    last_reach: Option<f64>,
    combo: u32,
    combo_at: f64,
    /// Chat Highlights: the mentions already pinged for.
    mentions_heard: u64,
    /// Where the HUD editor's panels were drawn last frame (x, y, width, height in GUI pixels).
    pub(super) hud_rects: std::cell::Cell<[Option<[f32; 4]>; 2]>,
    /// The HUD editor's drag: the widget and the cursor's offset into it.
    pub(super) hud_drag: Option<(&'static str, f32, f32)>,
    /// Where each HUD widget was drawn last frame (GUI pixels), for the editor.
    pub(super) widget_rects: std::cell::RefCell<Vec<(&'static str, [f32; 4])>>,
    /// The widgets' steady widths and Keystrokes' fading keys (`hud_widgets.rs`).
    pub(super) widget_memory: super::hud_widgets::WidgetMemory,
    /// The Wisp Menu (`wisp_client::wisp_menu`).
    pub(super) menu: wisp_client::wisp_menu::Menu,
    /// The HUD editor's toolbar buttons last frame (GUI pixels).
    pub(super) editor_hits: std::cell::RefCell<Vec<([f32; 4], super::hud_editor::Button)>>,
    /// The HUD editor was opened from the menu: Done goes back to it.
    pub(super) editor_from_menu: bool,
    /// Freelook's key last frame, and its toggle (the Toggle mode).
    freelook_key_down: bool,
    freelook_latched: bool,
    /// Inventory Search's box while an inventory screen is open (`inv_search.rs`).
    pub(super) inv_search: Option<wisp_client::ui::EditBox>,
    /// Waypoints, once read from their file (`waypoints.rs`).
    pub(super) waypoints: Option<Vec<super::waypoints::Waypoint>>,
}
/// A number rising from a mob that lost (or gained) health.
#[derive(Clone, Copy)]
pub(super) struct DamageNumber {
    pub at: glam::DVec3,
    /// Health lost; negative for health gained.
    pub amount: f32,
    pub age: u32,
}
/// Ticks a damage number shows for, and how far it rises in that time.
pub(super) const DAMAGE_NUMBER_TICKS: u32 = 24;
pub(super) const DAMAGE_NUMBER_RISE: f64 = 0.9;
/// Seconds after a hit within which the next one continues the combo.
const COMBO_WINDOW: f64 = 2.0;
/// Freelook (`freelook` module): the camera's own look while the player keeps facing ahead.
#[derive(Clone, Copy)]
struct Freelook {
    yaw: f32,
    pitch: f32,
    /// The camera before freelook moved it behind the player.
    camera_before: super::perspective::CameraType,
}
/// CPU time the minimap's tiles may take per frame.
const MINIMAP_BUDGET: Duration = Duration::from_micros(300);

impl Game<'_> {
    fn extras_now(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }
    /// Mouse buttons while playing, for mods' clicks.
    pub(super) fn extras_mouse(&mut self, button: MouseButton, pressed: bool) {
        self.native_mouse_event(button, pressed);
        let i = match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            _ => return,
        };
        self.extras.mouse[i] = pressed;
        // For mods (`input.clicks-per-second`, `native_clicks`).
        if pressed {
            let now = self.extras_now();
            self.extras.cps[i].click(now);
        }
    }
    /// Once per frame: the frame's time, and keys let go behind screens.
    pub(super) fn extras_frame(&mut self, elapsed: Duration) {
        // Under agent control the window may lack focus without pausing.
        let focused = self.focused || self.control.is_some();
        let playing = self.screen == Screen::Playing && self.title.is_none() && focused;
        if !playing {
            self.extras.mouse = [false; 2];
        }
        self.update_minimap();
        self.update_freelook(playing);
        self.waypoints_ready();
        self.inv_search_frame();
        self.friends_frame();
        // For mods (`render.frame-stats`, `native_frame_rate`).
        let now = self.extras_now();
        self.extras.fps.record(now, elapsed.as_secs_f32() * 1000.0);
        self.native_frame(elapsed);
        self.recipes_received_now();
    }
    /// Freelook runs while its key is held (or, in Toggle mode, from one press to the next) in a
    /// world, and ends when a screen opens, putting the camera back as it was.
    fn update_freelook(&mut self, playing: bool) {
        use super::perspective::CameraType;
        let e = &self.settings.extras;
        let on = e.module("freelook");
        let down = on && self.key_name_held(&e.opt_key("freelook", "key"));
        let toggle = e.opt_choice("freelook", "mode") == 1;
        if toggle && down && !self.extras.freelook_key_down {
            self.extras.freelook_latched = !self.extras.freelook_latched;
        }
        self.extras.freelook_key_down = down;
        if !playing || !on || !toggle {
            self.extras.freelook_latched = false;
        }
        let pressed = if toggle {
            self.extras.freelook_latched
        } else {
            down
        };
        let held = playing && on && self.player.is_some() && !self.emote_holds_camera() && pressed;
        match (held, self.extras.freelook) {
            (true, None) => {
                self.extras.freelook = Some(Freelook {
                    yaw: self.yaw,
                    pitch: self.pitch,
                    camera_before: self.camera_type,
                });
                if self.camera_type == CameraType::FirstPerson {
                    self.camera_type = CameraType::ThirdPersonBack;
                }
            }
            (false, Some(f)) => {
                self.extras.freelook = None;
                self.camera_type = f.camera_before;
            }
            _ => {}
        }
    }
    /// The mouse turns freelook's camera instead of the player while it is on. True when taken.
    pub(super) fn freelook_turn(&mut self, xo: f64, yo: f64) -> bool {
        let Some(f) = self.extras.freelook.as_mut() else {
            return false;
        };
        // Entity.turn's scale: 0.15 degrees per unit.
        f.yaw += (xo * 0.15) as f32;
        f.pitch = (f.pitch + (yo * 0.15) as f32).clamp(-90.0, 90.0);
        true
    }
    /// Freelook's camera look (yaw, pitch), while it is on.
    pub(super) fn freelook_angles(&self) -> Option<(f32, f32)> {
        self.extras.freelook.map(|f| (f.yaw, f.pitch))
    }
    /// Chat Highlights: tells the chat the player's name (or none while the module is off) and
    /// pings once for each new message that named the player.
    pub(super) fn chat_highlights(&mut self) {
        let on = self.settings.extras.module("chat_highlights");
        let name = on.then(|| self.own_profile_name().to_lowercase());
        if self.ui.chat.highlight != name {
            self.ui.chat.highlight = name;
        }
        self.ui.chat.highlight_color = self.settings.extras.opt_color("chat_highlights", "color");
        let mentions = self.ui.chat.mentions;
        if mentions > self.extras.mentions_heard {
            self.extras.mentions_heard = mentions;
            if on && self.settings.extras.opt_bool("chat_highlights", "sound") {
                self.audio
                    .play("minecraft:block.note_block.pling", None, 0.8, 1.5);
            }
        }
    }
    /// Hit Info: the last hit's reach and the combo (0 once it ran out).
    pub(super) fn hit_reading(&self) -> (Option<f64>, u32) {
        let alive = self.extras_now() - self.extras.combo_at <= COMBO_WINDOW;
        (
            self.extras.last_reach,
            if alive { self.extras.combo } else { 0 },
        )
    }
    /// Whether the key named `name` (`key.keyboard.left.alt`) is held.
    pub(super) fn key_name_held(&self, name: &str) -> bool {
        self.physical_keys
            .iter()
            .any(|k| wisp_client::keybinds::key_name(*k) == Some(name))
    }
    /// Hit Info: the player hit a mob `distance` blocks from the eye.
    pub(super) fn record_hit(&mut self, distance: f64) {
        if !self.settings.extras.module("hit_info") {
            return;
        }
        let now = self.extras_now();
        let e = &mut self.extras;
        e.combo = if now - e.combo_at <= COMBO_WINDOW {
            e.combo + 1
        } else {
            1
        };
        e.combo_at = now;
        e.last_reach = Some(distance);
    }
    /// Once a client tick: Damage Numbers compares each nearby mob's health with the last
    /// tick's and starts a number for a change; the numbers age and rise.
    pub(super) fn tick_damage_numbers(&mut self) {
        if !self.settings.extras.module("damage_numbers") {
            self.extras.last_health.clear();
            self.extras.damage_numbers.clear();
            return;
        }
        let Some(eye) = self.eye_position() else {
            return;
        };
        let eye = glam::DVec3::from_array(eye);
        let mut seen = std::collections::HashMap::new();
        let mut new = Vec::new();
        for m in &self.mobs {
            let at = glam::DVec3::new(m.position.x, m.position.y, m.position.z);
            if at.distance_squared(eye) > 32.0 * 32.0 {
                continue;
            }
            seen.insert(m.id, m.health);
            if let Some(&before) = self.extras.last_health.get(&m.id) {
                let lost = before - m.health;
                if lost.abs() >= 0.05 {
                    // Starts over the name tag line (and a Health Indicator) and rises from there.
                    let top = at + glam::DVec3::new(0.0, f64::from(m.height()) + 0.75, 0.0);
                    new.push(DamageNumber {
                        at: top,
                        amount: lost,
                        age: 0,
                    });
                }
            }
        }
        self.extras.last_health = seen;
        let numbers = &mut self.extras.damage_numbers;
        for n in numbers.iter_mut() {
            n.age += 1;
            n.at.y += DAMAGE_NUMBER_RISE / f64::from(DAMAGE_NUMBER_TICKS);
        }
        numbers.retain(|n| n.age < DAMAGE_NUMBER_TICKS);
        numbers.extend(new);
    }
    /// The frame counter for mods: fps, min, 1% low.
    pub(super) fn frame_reading(&self) -> (u32, u32, u32) {
        let r = self.extras.fps.reading();
        (r.fps, r.min, r.low)
    }
    /// Attack and use clicks in the last second, and whether each is held.
    pub(super) fn mouse_reading(&mut self) -> ((u32, u32), [bool; 2]) {
        let now = self.extras_now();
        let [l, r] = &mut self.extras.cps;
        let cps = (l.per_second(now) as u32, r.per_second(now) as u32);
        (cps, self.extras.mouse)
    }
    /// Auto-reconnect: a connection to `address` as `name` starts.
    pub(super) fn extras_connecting(&mut self, address: &str, name: &str, lan: bool) {
        self.extras.reconnect.connecting(address, name, lan);
    }
    /// The server this game connected to last.
    pub(super) fn server_address(&self) -> Option<&str> {
        self.extras.reconnect.address()
    }
    /// Auto-reconnect: the server let the player in.
    pub(super) fn extras_joined(&mut self) {
        self.extras.reconnect.joined(std::time::Instant::now());
    }
    /// Auto-reconnect: the player left the server on purpose.
    pub(super) fn extras_left(&mut self) {
        self.extras.reconnect.reset();
    }
    /// Auto-reconnect's next attempt after a disconnect, when it is on and
    /// the reason allows one.
    pub(super) fn reconnect_after(
        &mut self,
        reason: &str,
        key: Option<&str>,
        from_server: bool,
    ) -> Option<Retry> {
        if !self.settings.extras.auto_reconnect {
            self.extras.reconnect.reset();
            return None;
        }
        let now = std::time::Instant::now();
        self.extras.reconnect.ended(reason, key, from_server, now)
    }
    /// Show the attempt's countdown on the Disconnected screen.
    pub(super) fn offer_reconnect(&mut self, retry: Option<Retry>) {
        let Some(retry) = retry else {
            return;
        };
        if let Some(title) = self.title.as_mut()
            && let Mode::Multiplayer(multiplayer::Screen::Disconnected { retry: slot, .. }) =
                &mut title.mode
        {
            log::info!(
                "Wisp: reconnecting to {} in {} s",
                wisp_client::net::shown_address(&retry.address),
                retry.seconds_left(std::time::Instant::now())
            );
            *slot = Some(retry);
        }
    }
    /// Auto-reconnect: connect again once the countdown is over. An
    /// online-mode server gets the signed-in account again: `connect_server`
    /// takes it from `auth::current()`, held in memory for the whole run
    /// (the launcher's session file is gone after the first read).
    pub(super) fn extras_retry(&mut self) -> wisp_client::Result<()> {
        if let Some(retry) = self.title.as_mut().and_then(|t| t.due_retry()) {
            self.connect_server_with_options(&retry.address, &retry.name, None, retry.lan)?;
        }
        Ok(())
    }
    /// The minimap a mod shows (`native_minimap`): its corner, its side
    /// in GUI pixels and its view of the player.
    fn minimap_shown(&self) -> Option<(Corner, f32, View)> {
        let p = self.player.as_ref()?;
        if self.title.is_some() || self.ui.hide_gui {
            return None;
        }
        let m = self.minimap_wanted()?;
        let view = View {
            x: p.position.x,
            z: p.position.z,
            yaw: self.yaw,
            rotate: m.rotate,
            blocks_per_pixel: m.blocks_per_pixel,
            round: m.round,
        };
        Some((m.corner, m.size, view))
    }
    /// The minimap shown: a native mod's (`native_minimap`).
    fn minimap_wanted(&self) -> Option<NativeMinimap> {
        self.native_minimap_now()
    }
    /// Refresh the minimap's tiles within the frame budget, recompose its
    /// image when the view or a tile changed, and queue the upload.
    fn update_minimap(&mut self) {
        let Some((_, _, view)) = self.minimap_shown() else {
            if self.minimap_wanted().is_none() || self.player.is_none() {
                self.extras.minimap.clear();
                self.extras.minimap_dim = None;
            }
            return;
        };
        if self.extras.minimap_dim != Some(self.dim) {
            self.extras.minimap.clear();
            self.extras.minimap_dim = Some(self.dim);
        }
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let head = (p.position.y + f64::from(p.dimensions.eye_height)).floor() as i32;
        let (bx, bz) = (view.x.floor() as i32, view.z.floor() as i32);
        // Underground, under a roof or in the Nether: the floor around the player.
        let covered = self.world.light(bx, head, bz) >> 4 == 0;
        let layer = if self.dim == Dim::Nether || covered {
            Layer::Below(head)
        } else {
            Layer::Surface
        };
        // The chunks the image can reach (its corners when it turns).
        let reach = minimap::SIZE as f32 / 2.0 * view.blocks_per_pixel * std::f32::consts::SQRT_2;
        let radius = ((reach / 16.0).ceil() as i32 + 1).min(self.settings.render_distance.max(2));
        self.extras.minimap.update(
            &self.world,
            self.res.facts(),
            (bx >> 4, bz >> 4),
            radius,
            layer,
            MINIMAP_BUDGET,
        );
        if !self.extras.minimap.compose(view) {
            return;
        }
        let Some(rect) = self.gui.atlas.sprites.get(MINIMAP_SPRITE).copied() else {
            return;
        };
        let (w, h) = (self.gui.atlas.width as f32, self.gui.atlas.height as f32);
        self.map_patches.push(wisp_client::font::Patch {
            x: (rect.u0 * w).round() as u32,
            y: (rect.v0 * h).round() as u32,
            width: minimap::SIZE as u32,
            height: minimap::SIZE as u32,
            rgba: self.extras.minimap.image.clone(),
        });
    }
    /// A mod's zoom in effect (`native_zoom`): (factor, smooth, turning
    /// scale); none is 1.
    fn zoom_now(&self) -> (f32, bool, f64) {
        self.native_zoom_now().map_or((1.0, true, 1.0), |z| {
            (z.factor, z.smooth, f64::from(z.turn))
        })
    }
    /// The field-of-view modifier target, divided by a smooth zoom.
    pub(super) fn zoomed_fov_target(&self, vanilla: f32) -> f32 {
        match self.zoom_now() {
            (factor, true, _) => fov_target(vanilla, factor),
            _ => vanilla,
        }
    }
    /// The field of view in degrees, divided by an instant zoom.
    pub(super) fn zoomed_fov_degrees(&self, fov: f32) -> f32 {
        match self.zoom_now() {
            (factor, false, _) => fov / factor,
            _ => fov,
        }
    }
    /// Mouse turning while zoomed.
    pub(super) fn zoom_turn_scale(&self) -> f64 {
        let (factor, scale) = self.amod_zoom();
        self.zoom_now().2 * if scale { 1. / factor } else { 1. }
    }
    /// The minimap and mods' corner panels, in the HUD layer (hidden with F1
    /// by the caller). The top left corner gives way to the debug screen.
    pub(super) fn draw_extras(&mut self, batch: &mut HudBatch) {
        let map = self.minimap_shown();
        let visible = |corner| !(self.debug && corner == Corner::TopLeft);
        let mut corners = Corners::new(batch.width, batch.height, self.effect_rows_height());
        let atlas = &self.gui.atlas;
        if let Some((map_corner, map_size, view)) = map
            && visible(map_corner)
            && !self.extras.minimap.image.is_empty()
            && let Some(image) = atlas.sprites.get(MINIMAP_SPRITE).copied()
        {
            let (arrow, north) = if view.rotate {
                (0.0, Some(view.north()))
            } else {
                (180.0 + view.yaw, None)
            };
            let look = extras_hud::MinimapLook {
                corner: map_corner,
                size: map_size,
                image,
                arrow,
                north,
            };
            extras_hud::draw_minimap(batch, atlas, &mut corners, &look);
        }
        self.draw_wisp_hud(batch, &mut corners);
        // The F3 screen takes the corner the widgets stack in, as vanilla's HUD gives way to it.
        let e = &self.settings.extras;
        let smart = e.smart_f3 && self.debug && !self.reduced_debug_info();
        if !self.debug || smart || self.screen == Screen::HudEditor {
            self.draw_hud_widgets(batch);
        }
        self.draw_friend_toasts(batch);
        self.native_hud(batch, &mut corners);
        self.draw_touch(batch);
    }
    fn draw_wisp_hud(&self, batch: &mut HudBatch, corners: &mut Corners) {
        use wisp_client::lang;
        let e = &self.settings.extras;
        let smart = e.smart_f3 && self.debug && !self.reduced_debug_info();
        let mut lines: Vec<(String, u32)> = Vec::new();
        if e.wisp_hud {
            lines.push((lang::get("wisp.hud_title"), 0xf4b895));
        }
        if e.fps || smart {
            let (fps, _, low) = self.frame_reading();
            lines.push((lang::tr("wisp.fps_line", &[&fps, &low]), 0xffffff));
        }
        if (e.wisp_hud || smart)
            && !self.reduced_debug_info()
            && let Some(player) = &self.player
        {
            let p = player.position;
            lines.push((
                lang::tr(
                    "wisp.position_line",
                    &[&p.x.floor(), &p.y.floor(), &p.z.floor()],
                ),
                0xddd4ef,
            ));
        }
        if e.apple_skin {
            if let Some(p) = &self.player {
                let f = &p.health.food;
                lines.push((
                    lang::tr(
                        "wisp.food_line",
                        &[&f.food, &format!("{:.1}", f.saturation)],
                    ),
                    0xffcc88,
                ));
            }
            if let Some(stack) = self.inventory.get(self.selected).and_then(Option::as_ref)
                && let Some((nutrition, saturation)) =
                    self.items().get(&stack.item).and_then(|i| i.food)
            {
                lines.push((
                    lang::tr(
                        "wisp.restore_line",
                        &[&nutrition, &format!("{saturation:.1}")],
                    ),
                    0xa8e6a0,
                ));
            }
        }
        let refs: Vec<_> = lines.iter().map(|(s, c)| (s.as_str(), *c)).collect();
        let corner = if self.debug && !smart {
            Corner::TopRight
        } else {
            Corner::TopLeft
        };
        self.draw_hud_panel(batch, corners, corner, &refs, HudPanel::Info);
    }
    /// One of the HUD editor's panels: in its corner, or where the editor put it, at its scale,
    /// colour and background. In the editor an empty panel shows its name so it can be moved.
    fn draw_hud_panel(
        &self,
        batch: &mut HudBatch,
        corners: &mut Corners,
        corner: Corner,
        lines: &[(&str, u32)],
        panel: HudPanel,
    ) {
        let e = &self.settings.extras;
        let name;
        let lines = if lines.is_empty() && false {
            name = match panel {
                HudPanel::Info => {
                    wisp_client::lang::get_or("wisp.hud_editor.info", "Information HUD")
                }
                HudPanel::Armor => wisp_client::lang::get_or("wisp.armor_status", "Armor Status"),
            };
            vec![(name.as_str(), 0xaaaaaa)]
        } else {
            lines.to_vec()
        };
        let mut rects = self.extras.hud_rects.get();
        rects[panel as usize] = None;
        if !lines.is_empty() {
            let atlas = &self.gui.atlas;
            let style = extras_hud::PanelStyle {
                color: e.hud_color,
                background: e.hud_background,
                glass: e.wisp_hud,
            };
            let (w, h) = extras_hud::panel_size(atlas, &lines, style.glass);
            let s = e.hud_scale;
            let (sw, sh) = (w * s, h * s);
            let pos = match panel {
                HudPanel::Info => e.hud_info_pos,
                HudPanel::Armor => e.hud_armor_pos,
            };
            let (x, y) = match pos {
                Some((fx, fy)) => (
                    (fx * batch.width).clamp(0.0, (batch.width - sw).max(0.0)),
                    (fy * batch.height).clamp(0.0, (batch.height - sh).max(0.0)),
                ),
                None => corners.place(corner, sw, sh),
            };
            let start = batch.vertices.len();
            extras_hud::draw_panel_at(batch, atlas, x, y, &lines, &style);
            if s != 1.0 {
                batch.scale_since(start, x, y, s, s);
            }
            rects[panel as usize] = Some([x, y, sw, sh]);
        }
        self.extras.hud_rects.set(rects);
    }
    /// The height the status effect icons take at the top right
    /// (`Hud.extractEffects`: beneficial at y 1, harmful at y 26).
    fn effect_rows_height(&self) -> f32 {
        let effects = &self.potions.effects.list;
        if effects.is_empty() || self.screen != Screen::Playing {
            return 0.0;
        }
        let harmful = effects.iter().any(|e| {
            wisp_client::brewing::effect(e.id)
                .is_some_and(|d| d.category != wisp_client::brewing::Category::Beneficial)
        });
        if harmful { 50.0 } else { 25.0 }
    }
}
