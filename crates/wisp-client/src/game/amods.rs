//! Runtime Wisp client modules, loaded from .amod packages.
use super::Game;
use wisp_mods::amod::{self, Definition, ZoomState};

pub(super) struct ClientModule {
    pub definition: Definition,
    pub zoom: ZoomState,
}
impl Game<'_> {
    pub(super) fn load_amods(&mut self) {
        self.mods.client.clear();
        let game = Self::game_dir();
        let mut paths = amod::find(&game);
        paths.extend(self.mods.dev.iter().filter(|p| amod::is_client(p)).cloned());
        paths.sort();
        paths.dedup();
        self.voicechat_module = false;
        let mut max_render = false;
        for path in paths {
            match Definition::read(&path) {
                // Voice chat runs on its own (`game/voicechat.rs`): the zoom state is the rest's.
                Ok(definition) if definition.is_voicechat() => {
                    self.voicechat_module = amod::enabled(&game, &definition.id);
                }
                // Max Render only lifts the render distance cap (`options_file`).
                Ok(definition) if definition.is_max_render() => {
                    max_render |= amod::enabled(&game, &definition.id)
                        && !self.settings.mods.disabled.contains(&definition.id);
                }
                Ok(definition)
                    if !self
                        .mods
                        .client
                        .iter()
                        .any(|m| m.definition.id == definition.id) =>
                {
                    let zoom = ZoomState::new(amod::config(&game, &definition));
                    self.mods.client.push(ClientModule { definition, zoom });
                }
                Ok(_) => log::warn!("Wisp: duplicate client mod skipped: {}", path.display()),
                Err(e) => log::warn!("Wisp: client mod {}: {e}", path.display()),
            }
        }
        wisp_client::options_file::set_unlimited_render_distance(max_render);
        let max = wisp_client::options_file::render_distance_max();
        if self.settings.render_distance > max {
            self.settings.render_distance = max;
            self.apply_settings();
        }
    }
    pub(super) fn amod_key(&mut self, key: &str, pressed: bool) {
        let playing = self.native_playing() && self.native_window();
        for m in &mut self.mods.client {
            let binding = self
                .settings
                .mods
                .keys
                .get(&m.definition.id)
                .and_then(|keys| keys.get("zoom"))
                .unwrap_or(&m.zoom.config.key);
            if key == binding {
                m.zoom.key(
                    &m.zoom.config.key.clone(),
                    pressed,
                    playing && !self.settings.mods.disabled.contains(&m.definition.id),
                );
            }
        }
    }
    pub(super) fn amod_wheel(&mut self, steps: f32) -> bool {
        if !self.native_playing() || self.scoping() {
            return false;
        }
        for m in &mut self.mods.client {
            if !self.settings.mods.disabled.contains(&m.definition.id) && m.zoom.scroll(steps) {
                return true;
            }
        }
        false
    }
    pub(super) fn amod_zoom(&self) -> (f64, bool) {
        let playing = self.native_playing() && self.native_window() && !self.scoping();
        self.mods
            .client
            .iter()
            .map(|m| {
                (
                    m.zoom.factor(playing && self.native_on(&m.definition.id)),
                    m.zoom.config.scale_sensitivity,
                )
            })
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap_or((1., false))
    }
    pub(super) fn amod_settings(&self, id: &str, buttons: &mut Vec<String>) {
        let Some(m) = self.mods.client.iter().find(|m| m.definition.id == id) else {
            return;
        };
        let c = &m.zoom.config;
        let on = |v| {
            crate::lang::get_or(
                if v { "options.on" } else { "options.off" },
                if v { "On" } else { "Off" },
            )
        };
        buttons.push(format!(
            "{}: {}×",
            crate::lang::get_or("wisp.mods.zoom.start", "Starting zoom"),
            c.initial_factor
        ));
        buttons.push(format!(
            "{}: {}",
            crate::lang::get_or("wisp.mods.zoom.smooth", "Smooth zoom"),
            on(c.smooth)
        ));
        buttons.push(format!(
            "{}: {}",
            crate::lang::get_or("wisp.mods.zoom.sensitivity", "Scale sensitivity"),
            on(c.scale_sensitivity)
        ));
    }
    pub(super) fn amod_settings_click(&mut self, id: &str, button: usize) {
        let Some(m) = self.mods.client.iter_mut().find(|m| m.definition.id == id) else {
            return;
        };
        let mut c = m.zoom.config.clone();
        match button {
            0 => {
                c.initial_factor = if c.initial_factor >= amod::MAX_FACTOR {
                    2.
                } else {
                    (c.initial_factor * 2.).min(amod::MAX_FACTOR)
                }
            }
            1 => c.smooth = !c.smooth,
            2 => c.scale_sensitivity = !c.scale_sensitivity,
            _ => return,
        }
        match amod::save_config(&Self::game_dir(), id, &c) {
            Ok(()) => m.zoom = ZoomState::new(c),
            Err(e) => log::warn!("Wisp: mod settings not saved: {e}"),
        }
    }
}
