//! Wisp's home screen in place of vanilla's title menu (`wisp_client::wisp_home`): when it
//! shows, its backdrop picture, the player's figure (their own skin, idling, waving hello and
//! when clicked, under a name tag) and its clicks.
mod wardrobe;

use super::*;
use std::time::{Duration, Instant};
use wisp_client::{
    gfx::picture::{self, Picture},
    title::Action,
    ui_theme::Ui,
    wisp_home::{Click, Data},
};

/// When the figure waves on its own: first this long after the screen shows, then again
/// every `WAVE_EVERY` seconds.
const WAVE_FIRST: f32 = 0.6;
const WAVE_EVERY: f32 = 14.0;
/// How long a wave lasts, in seconds.
const WAVE_LENGTH: f32 = 2.4;

/// The signed-in account's own skin (`/minecraft/profile`), fetched once.
#[derive(Default)]
enum OwnSkin {
    #[default]
    Idle,
    Fetching(std::sync::mpsc::Receiver<Option<wisp_client::skins::Textures>>),
    Done(Option<wisp_client::skins::Textures>),
}

/// What the home screen keeps between frames.
#[derive(Default)]
pub(super) struct HomeState {
    pub(super) screen: wisp_client::wisp_home::Home,
    /// The servers and worlds listed, and when they were read.
    data: Option<(Instant, Data)>,
    /// The backdrop picture, once looked for (None inside: the folder has none).
    picture: Option<Option<Arc<Picture>>>,
    /// When the backdrop began to show, for its slow zoom.
    shown_since: Option<Instant>,
    own_skin: OwnSkin,
    /// The player's figure, once the world resources hold the player model.
    figure: Option<Mob>,
    /// When the figure began to show (its clock), and when it was clicked to wave.
    figure_since: Option<Instant>,
    waved: Option<Instant>,
    wardrobe: wardrobe::WardrobeState,
}

impl HomeState {
    /// The account's skin is asked for again (it changed).
    fn reset_own_skin(&mut self) {
        self.own_skin = OwnSkin::Idle;
    }
}

/// The pictures the launcher's backdrop shows: `WISP_BACKGROUNDS` (the launcher sets it), else
/// a `backgrounds` folder in the game folder.
fn backgrounds_dir() -> PathBuf {
    wisp_compat::var_os("WISP_BACKGROUNDS")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("backgrounds"))
}

impl Game<'_> {
    /// Wisp's home screen shows instead of vanilla's title menu (unless it is off,
    /// [`Self::wisp_home_on`]).
    pub(super) fn wisp_home_shown(&self) -> bool {
        self.title.as_ref().is_some_and(|t| t.shows_main())
            && !matches!(self.screen, Screen::Options | Screen::Win)
            && !self.remote_loading()
            && self.wisp_home_on()
    }

    /// Wisp's home screen is on: Menu style is Wisp or Wisp Christmas (Wisp Settings,
    /// `menuStyle` in `wisp-extras.txt`), `wisp.txt` doesn't say `titleScreen:vanilla` (the older switch) and
    /// `WISP_VANILLA_TITLE` is not set (for a run that compares the title with vanilla's).
    fn wisp_home_on(&self) -> bool {
        self.settings.extras.menu_style != wisp_client::extras::MenuStyle::Minecraft
            && wisp_compat::var_os("WISP_VANILLA_TITLE").is_none()
            && wisp_client::ui_theme::preference(&Self::options_dir(), "titleScreen").as_deref()
                != Some("vanilla")
    }

    fn options_dir() -> PathBuf {
        Self::options_path()
            .parent()
            .map_or_else(|| PathBuf::from("."), std::path::Path::to_path_buf)
    }

    /// The backdrop picture for this frame and its zoom: behind the home screen and every
    /// screen opened from it (worlds, servers, options), in place of the panorama.
    pub(super) fn title_picture(&mut self) -> Option<(Arc<Picture>, f32)> {
        if self.title.is_none() || self.remote_loading() || !self.wisp_home_on() {
            self.home.shown_since = None;
            return None;
        }
        let picture = self
            .home
            .picture
            .get_or_insert_with(|| {
                let found = picture::pictures(&backgrounds_dir());
                if found.is_empty() {
                    return None;
                }
                // A different one each start.
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs() as usize);
                Picture::load(&found[seed % found.len()]).map(Arc::new)
            })
            .clone()?;
        let since = *self.home.shown_since.get_or_insert_with(Instant::now);
        // A slow drift in, over a minute, then held.
        let t = (since.elapsed().as_secs_f32() / 60.0).min(1.0);
        Some((picture, 1.04 + 0.04 * t))
    }

    fn home_data(&mut self) -> Data {
        if let Some((read, data)) = &self.home.data
            && read.elapsed() < Duration::from_secs(5)
        {
            return data.clone();
        }
        let Some(title) = self.title.as_ref() else {
            return Data::default();
        };
        let release = wisp_core::release::RELEASE.unwrap_or("dev");
        let data = Data {
            player: self
                .options
                .player_name
                .clone()
                .unwrap_or_else(|| self.options.player_name()),
            signed_in: self.options.session_file.is_some() || self.options.session_stdin,
            multiplayer: title.allows_multiplayer,
            version: format!(
                "Wisp {release} \u{b7} Minecraft {}",
                wisp_core::VANILLA_VERSION
            ),
            wardrobe: Default::default(),
        };
        self.home.data = Some((Instant::now(), data.clone()));
        data
    }

    pub(super) fn draw_wisp_home(&mut self, batch: &mut HudBatch) {
        self.restore_wardrobe_skin();
        let mut data = self.home_data();
        if self.home.screen.page == wisp_client::wisp_home::Page::Wardrobe {
            data.wardrobe = self.wardrobe_data();
        }
        let cursor = self.cursor;
        let mut ui = Ui::new(&self.gui.atlas, batch, cursor);
        self.home.screen.draw(&mut ui, &data);
        ui.finish();
        match self.home.screen.figure {
            Some(rect) => self.draw_home_figure(batch, rect, &data.player),
            None => self.home.figure_since = None,
        }
    }

    /// The skin the figure wears: the account's own once downloaded, else the launcher's
    /// offline skin, else Steve. Returns the atlas sprite and whether the model is slim.
    fn home_skin(&mut self) -> (String, bool) {
        self.flush_skin_slots();
        match &mut self.home.own_skin {
            OwnSkin::Idle => {
                self.home.own_skin = match self.auth() {
                    Some(auth) => {
                        let (send, recv) = std::sync::mpsc::channel();
                        std::thread::Builder::new()
                            .name("Home skin".into())
                            .spawn(move || {
                                let _ = send.send(wisp_client::auth::own_textures(auth.as_ref()));
                            })
                            .ok();
                        OwnSkin::Fetching(recv)
                    }
                    // Not signed in (yet): asked again next frame.
                    None => OwnSkin::Idle,
                };
            }
            OwnSkin::Fetching(recv) => {
                if let Ok(own) = recv.try_recv() {
                    self.home.own_skin = OwnSkin::Done(own);
                }
            }
            OwnSkin::Done(_) => {}
        }
        if let OwnSkin::Done(Some(textures)) = &self.home.own_skin {
            let textures = textures.clone();
            let found = {
                let mut downloads = self.skin_downloads.borrow_mut();
                wisp_client::skins::lookup(&textures, false, |kind, url| downloads.state(kind, url))
            };
            if let Some(skin) = found
                && self.again_skin(&skin)
            {
                return (skin, textures.slim);
            }
        }
        if let Some(skin) = wisp_client::skins::offline_skin() {
            // The launcher's, or the one picked in the wardrobe (a new sprite each time).
            let sprite = wisp_client::skins::offline_skin_sprite();
            if !self.skin_slots.contains(&sprite) || !self.gui.atlas.sprites.contains_key(&sprite) {
                self.store_skin(sprite.clone(), skin.0.clone());
            }
            if self.gui.atlas.sprites.contains_key(&sprite) {
                return (sprite, skin.1);
            }
        }
        ("entity/player/wide/steve".into(), false)
    }

    /// The player's figure in `rect` (GUI pixels): their skin, breathing and swaying, the
    /// head after the cursor, waving now and then; their name tag over it as in the world.
    fn draw_home_figure(&mut self, batch: &mut HudBatch, rect: [f32; 4], name: &str) {
        let Some(kind) = self
            .res
            .rest
            .get()
            .and_then(|rest| rest.player_type.clone())
        else {
            return;
        };
        let own = self.home_skin();
        let (skin, slim) = self.wardrobe_preview().unwrap_or(own);
        let t = self
            .home
            .figure_since
            .get_or_insert_with(Instant::now)
            .elapsed()
            .as_secs_f32();
        let clicked = self.home.waved.map(|w| w.elapsed().as_secs_f32());
        let [x, y, w, h] = rect;
        let cx = x + w / 2.0;
        // The head and body turn toward the cursor, gently.
        let x_angle = ((cx - self.cursor.0) / 120.0).atan();
        let y_angle = ((y + h * 0.18 - self.cursor.1) / 120.0).atan();
        let pose = figure_pose(t, clicked, x_angle, y_angle);
        let m = self
            .home
            .figure
            .get_or_insert_with(|| Mob::new(0, kind, wisp_core::geometry::Vec3::ZERO, 180.0));
        m.skin = Some(skin);
        m.slim = slim;
        m.extra.model_scale = 0.0;
        let ticks = t * 20.0;
        m.age = ticks as u32;
        let partial = ticks.fract();
        m.body_yaw = pose.body_yaw;
        m.previous_body_yaw = pose.body_yaw;
        m.head_yaw = pose.head_yaw;
        m.previous_head_yaw = pose.head_yaw;
        m.head_pitch = pose.head_pitch;
        m.previous_head_pitch = pose.head_pitch;
        m.extra.emote_arms = pose.arms;
        m.extra.previous_emote_arms = pose.arms;
        m.extra.emote_tilt = pose.tilt;
        m.extra.previous_emote_tilt = pose.tilt;
        m.previous = m.position;
        let m = m.clone();
        let camera = glam::DVec3::new(m.position.x, m.position.y, m.position.z);
        let atlas = &self.gui.atlas;
        let lookup = |name: &str| atlas.sprites.get(name).map(|r| [r.u0, r.v0, r.u1, r.v1]);
        let mut verts = Vec::new();
        wisp_client::mobs::cem_anim::in_gui(|| {
            wisp_client::mobs::render::render_with(
                &m,
                partial,
                camera,
                &lookup,
                240 | 240 << 8,
                self.res.mob_data(),
                &[],
                &mut verts,
            );
            // What the player wears from the Store, while Show my cosmetics is on.
            if wisp_cosmetics::enabled() && self.settings.extras.cosmetics_mine {
                let base = wisp_client::mobs::render::base_transform(&m, partial, camera);
                self.cosmetics_geometry(&m, true, base, partial, 240 | 240 << 8, false, &mut verts);
            }
        });
        // 2.1 blocks fill the box: the model, and the name tag over it.
        let size = h / 2.1;
        let feet = y + h - size * 0.04;
        let gui = glam::Mat4::from_translation(glam::Vec3::new(cx, feet, 0.0))
            * glam::Mat4::from_scale(glam::Vec3::new(size, size, -size))
            * glam::Mat4::from_rotation_z(std::f32::consts::PI);
        let quads = super::perspective::player_preview_quads_no_cull(
            verts,
            gui,
            None,
            &atlas.sprites,
            0..0,
        );
        let start = batch.vertices.len();
        for (_, p, uv, color) in quads {
            batch.quad_3d(p, uv, color);
        }
        batch.depth_since(start, [x - w, y - h, x + 2.0 * w, y + 2.0 * h]);
        // The name tag (`EntityRenderer.renderNameTag`): white text on a quarter-black plate,
        // half a block over the head.
        let scale = (size / 64.0).clamp(0.75, 2.0);
        let text_w = HudBatch::text_width(atlas, name);
        let head_top = feet - size * (1.8 + 0.3) - pose.lift * size;
        let (tw, th) = (text_w * scale, 9.0 * scale);
        let (tx, ty) = (cx - tw / 2.0, head_top - th);
        batch.fill(
            atlas,
            tx - 2.0 * scale,
            ty - 1.0 * scale,
            tw + 3.0 * scale,
            th + scale,
            0x4000_0000,
        );
        let start = batch.vertices.len();
        batch.text(atlas, name, 0.0, 0.0, 0xFFFF_FFFF, false);
        for v in &mut batch.vertices[start..] {
            v.pos[0] = tx + v.pos[0] * scale;
            v.pos[1] = ty + v.pos[1] * scale;
        }
    }

    /// A left press on the home screen; returns whether it took the press.
    pub(super) fn wisp_home_press(&mut self, event_loop: &dyn EventLoopCtl) -> Result<bool> {
        let Some(click) = self.home.screen.click(self.cursor) else {
            return Ok(false);
        };
        if self.wardrobe_click(&click) {
            self.audio.play_button_click();
            return Ok(true);
        }
        let size = self
            .renderer
            .as_ref()
            .map(|r| [r.config.width, r.config.height])
            .unwrap_or([1, 1]);
        let b = HudBatch::new(size);
        let action = match click {
            // The wardrobe's own clicks went to `wardrobe_click`.
            Click::Nothing
            | Click::Wardrobe
            | Click::Back
            | Click::Tab(_)
            | Click::Skin(_)
            | Click::Model(_)
            | Click::ApplySkin
            | Click::SkinFolder
            | Click::ShowCosmetics
            | Click::Item(_) => return Ok(true),
            Click::Button(id) => self
                .title
                .as_mut()
                .map_or(Action::None, |t| t.press_main(id, (b.width, b.height))),
            Click::Figure => {
                // A click waves back (not again while it still waves).
                if self
                    .home
                    .waved
                    .is_none_or(|w| w.elapsed().as_secs_f32() > WAVE_LENGTH * 0.8)
                {
                    self.home.waved = Some(Instant::now());
                }
                return Ok(true);
            }
        };
        self.audio.play_button_click();
        self.home.data = None;
        self.save_title_options();
        self.title_action(event_loop, action)?;
        Ok(true)
    }
}

/// The figure's pose at a moment: how it stands, sways and waves.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FigurePose {
    body_yaw: f32,
    head_yaw: f32,
    head_pitch: f32,
    /// Both arms (right, left: x, y, z radians) and how far they hold (`emote_arms`).
    arms: ([[f32; 3]; 2], f32),
    /// The body's lean (pitch, roll degrees).
    tilt: [f32; 2],
    /// How far the head rises with a breath, in blocks.
    lift: f32,
}

fn smooth(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far into a wave the figure is (0..1, the arm up) and the wave's own clock: one
/// `WAVE_FIRST` in and every `WAVE_EVERY` after, or one `clicked` seconds ago.
fn waving(t: f32, clicked: Option<f32>) -> (f32, f32) {
    let auto = (t >= WAVE_FIRST).then(|| (t - WAVE_FIRST) % WAVE_EVERY);
    let phase = match (auto, clicked) {
        (_, Some(c)) if c < WAVE_LENGTH => c,
        (Some(a), _) if a < WAVE_LENGTH => a,
        _ => return (0.0, 0.0),
    };
    let up = smooth(0.0, 0.35, phase) * (1.0 - smooth(WAVE_LENGTH - 0.45, WAVE_LENGTH, phase));
    (up, phase)
}

/// The figure at `t` seconds: breathing, a slow sway and the head after the cursor (`x_angle`
/// and `y_angle`, as the inventory's preview turns), the right hand raised to wave.
fn figure_pose(t: f32, clicked: Option<f32>, x_angle: f32, y_angle: f32) -> FigurePose {
    let sway = (t * 0.7).sin();
    let breath = (t * 1.6).sin();
    let (wave, phase) = waving(t, clicked);
    let body_yaw = 180.0 + x_angle * 18.0 + 5.0 * sway - 6.0 * wave;
    // The idle arms as vanilla bobs them (`AnimationUtils.bobModelPart`), so the arm the wave
    // leaves alone hangs as it would have.
    let age = t * 20.0;
    let bob_x = (age * 0.067).sin() * 0.05;
    let bob_z = (age * 0.09).cos() * 0.05 + 0.05;
    let rest_right = [bob_x, 0.0, bob_z];
    let left = [-bob_x, 0.0, -bob_z];
    // The hand up and out beside the head, waving side to side (with the arm raised, a
    // negative roll takes the hand away from the head).
    let swing = (phase * 9.0).sin();
    let hand = [-2.6 + 0.05 * swing, 0.0, -0.5 - 0.3 * swing];
    let right: [f32; 3] = std::array::from_fn(|i| rest_right[i] + (hand[i] - rest_right[i]) * wave);
    FigurePose {
        body_yaw,
        head_yaw: body_yaw + x_angle * 22.0 + 4.0 * (t * 0.45).sin() + 8.0 * wave,
        head_pitch: -y_angle * 22.0 + 2.0 * breath - 6.0 * wave,
        // The arms hold the pose only while it waves; at rest vanilla's bob is the pose.
        arms: ([right, left], wave),
        tilt: [1.2 * breath, 1.5 * sway + 2.5 * wave],
        lift: 0.004 * breath,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_figure_waves_hello_then_rests_until_the_next_wave() {
        assert_eq!(waving(0.2, None).0, 0.0);
        assert!(waving(WAVE_FIRST + 1.0, None).0 > 0.99);
        assert_eq!(waving(WAVE_FIRST + WAVE_LENGTH + 1.0, None).0, 0.0);
        assert!(waving(WAVE_FIRST + WAVE_EVERY + 1.0, None).0 > 0.99);
    }

    #[test]
    fn a_click_waves_at_once() {
        let t = WAVE_FIRST + 6.0;
        assert_eq!(waving(t, None).0, 0.0);
        assert!(waving(t, Some(1.0)).0 > 0.99);
    }

    #[test]
    fn at_rest_the_arms_are_left_to_vanilla() {
        assert_eq!(figure_pose(WAVE_FIRST + 6.0, None, 0.0, 0.0).arms.1, 0.0);
        let up = figure_pose(WAVE_FIRST + 1.0, None, 0.0, 0.0);
        assert!(up.arms.0[0][0] < -2.5, "the right arm is up");
    }
}
