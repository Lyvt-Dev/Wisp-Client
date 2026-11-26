//! Proximity voice chat with servers that run Simple Voice Chat (`wisp_voicechat`), on while
//! the instance has the `voicechat` client module. Its keys and settings live on its page in
//! Options > Mods, which V opens in game; the HUD shows the microphone's state and who talks.
use super::Game;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use wisp_client::hud::HudBatch;
use wisp_client::options_ui::Page;
use wisp_voicechat::audio::{self, Listener};
use wisp_voicechat::{Settings, State, VoiceChat};

pub(super) const ID: &str = "voicechat";
const NAME: &str = "Simple Voice Chat";

/// The keys on its page: name, label, default.
const KEYS: [(&str, &str, &str); 5] = [
    ("push_to_talk", "Push to talk", "key.keyboard.caps.lock"),
    ("whisper", "Whisper", "key.keyboard.unknown"),
    ("mute", "Mute microphone", "key.keyboard.m"),
    ("disable", "Disable voice chat", "key.keyboard.n"),
    ("menu", "Voice chat menu", "key.keyboard.v"),
];

const THRESHOLDS: [f32; 13] = [
    -80.0, -75.0, -70.0, -65.0, -60.0, -55.0, -50.0, -45.0, -40.0, -35.0, -30.0, -25.0, -20.0,
];
const GAINS: [f32; 8] = [0.5, 0.75, 1.0, 1.5, 2.0, 2.5, 3.0, 4.0];
const VOLUMES: [f32; 13] = [
    0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0,
];

/// The settings as saved (`wisp-mods/.wisp-mod-data/voicechat/settings.json`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    voice_activation: bool,
    threshold_db: f32,
    mic_gain: f32,
    volume: f32,
    muted: bool,
    disabled: bool,
    microphone: String,
    speaker: String,
    /// Per-player volumes, by UUID in hex.
    players: HashMap<String, f32>,
}

impl Default for Saved {
    fn default() -> Self {
        Saved::from(&Settings::default())
    }
}

impl From<&Settings> for Saved {
    fn from(s: &Settings) -> Self {
        Self {
            voice_activation: s.voice_activation,
            threshold_db: s.threshold_db,
            mic_gain: s.mic_gain,
            volume: s.volume,
            muted: s.muted,
            disabled: s.disabled,
            microphone: s.microphone.clone(),
            speaker: s.speaker.clone(),
            players: s
                .players
                .iter()
                .map(|(k, v)| (format!("{k:032x}"), *v))
                .collect(),
        }
    }
}

impl From<Saved> for Settings {
    fn from(s: Saved) -> Self {
        Self {
            voice_activation: s.voice_activation,
            threshold_db: s.threshold_db.clamp(-127.0, 0.0),
            mic_gain: s.mic_gain.clamp(0.1, 10.0),
            volume: s.volume.clamp(0.0, 3.0),
            muted: s.muted,
            disabled: s.disabled,
            microphone: s.microphone,
            speaker: s.speaker,
            players: s
                .players
                .into_iter()
                .filter_map(|(k, v)| Some((u128::from_str_radix(&k, 16).ok()?, v.clamp(0.0, 3.0))))
                .collect(),
        }
    }
}

#[derive(Default)]
pub(super) struct Voice {
    chat: Option<VoiceChat>,
    /// The server it joined, to notice a new one.
    joined: Option<String>,
    testing: bool,
}

/// The next value of `list` after `current` (wrapping), for a button that cycles.
fn next<T: Copy + PartialOrd>(list: &[T], current: T) -> T {
    list.iter()
        .copied()
        .find(|&v| v > current)
        .unwrap_or(list[0])
}

/// The host of `host[:port]` (or `[v6]:port`).
fn host_of(address: &str) -> String {
    if let Some(rest) = address.strip_prefix('[') {
        return rest.split(']').next().unwrap_or(rest).to_owned();
    }
    match address.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') && p.parse::<u16>().is_ok() => h.to_owned(),
        _ => address.to_owned(),
    }
}

fn settings_path() -> std::path::PathBuf {
    Game::game_dir()
        .join(wisp_mods::amod::DIR)
        .join(".wisp-mod-data")
        .join(ID)
        .join("settings.json")
}

fn load() -> Settings {
    std::fs::read(settings_path())
        .ok()
        .and_then(|b| serde_json::from_slice::<Saved>(&b).ok())
        .unwrap_or_default()
        .into()
}

fn save(settings: &Settings) {
    let bytes = serde_json::to_vec_pretty(&Saved::from(settings)).unwrap_or_default();
    if let Err(e) = wisp_mods::amod::atomic_write(&settings_path(), &bytes) {
        log::warn!("Wisp voice chat: settings not saved: {e}");
    }
}

impl Game<'_> {
    fn voice_on(&self) -> bool {
        self.voicechat_module && !self.settings.mods.disabled.iter().any(|d| d == ID)
    }

    fn voice_chat(&mut self) -> &mut VoiceChat {
        self.voicechat
            .chat
            .get_or_insert_with(|| VoiceChat::new(load()))
    }

    /// Once a tick: join a new server's voice chat, place the voices, send what it queued.
    pub(super) fn tick_voicechat(&mut self) {
        let server = self
            .remote
            .as_ref()
            .filter(|r| r.logged_in)
            .map(|r| r.session.address.clone());
        if !self.voice_on() || server.is_none() {
            if self.voicechat.joined.take().is_some()
                && let Some(chat) = self.voicechat.chat.as_mut()
            {
                chat.leave();
            }
            return;
        }
        let server = server.expect("checked");
        if self.voicechat.joined.as_deref() != Some(server.as_str()) {
            let host = host_of(&server);
            let resolve: wisp_voicechat::connection::Resolve =
                Arc::new(|host: &str| wisp_protocol::server_redirect(host, 25565).0);
            self.voice_chat().joined(&host, resolve);
            self.voicechat.joined = Some(server);
        }
        let listener = self
            .player
            .as_ref()
            .map(|p| Listener {
                position: [p.position.x, p.position.y + 1.62, p.position.z],
                yaw: self.yaw,
            })
            .unwrap_or_default();
        let positions: HashMap<u128, [f64; 3]> = self
            .remote
            .as_ref()
            .map(|r| {
                r.level_players
                    .iter()
                    .filter_map(|(&id, &uuid)| {
                        let m = self.mobs.iter().find(|m| m.id == id as u32)?;
                        Some((uuid, [m.position.x, m.position.y + 1.62, m.position.z]))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let chat = self.voice_chat();
        chat.tick(listener, &|uuid| positions.get(&uuid).copied());
        for (channel, data) in chat.take_outgoing() {
            self.send(wisp_protocol::packets::play::Serverbound::CustomPayload(
                wisp_protocol::packets::configuration::CustomPayload {
                    channel: channel.into(),
                    data,
                },
            ));
        }
    }

    /// A custom payload from the server: true when it was voice chat's.
    pub(super) fn voicechat_payload(&mut self, channel: &str, data: &[u8]) -> bool {
        if !channel.starts_with("voicechat:") || !self.voice_on() {
            return false;
        }
        self.voice_chat().payload(channel, data)
    }

    /// A key went down or up, by name (`key.keyboard.v`).
    pub(super) fn voicechat_key(&mut self, key: &str, pressed: bool) {
        if !self.voice_on() || key == "key.keyboard.unknown" {
            return;
        }
        let playing = self.native_playing();
        let bound = |name: &str| {
            let default = KEYS.iter().find(|k| k.0 == name).map_or("", |k| k.2);
            self.native_mod_key(ID, name, default) == key
        };
        let (ptt, whisper, mute, disable, menu) = (
            bound("push_to_talk"),
            bound("whisper"),
            bound("mute"),
            bound("disable"),
            bound("menu"),
        );
        // Talking keys hold only while playing; letting go always counts.
        if ptt {
            self.voice_chat().set_push_to_talk(pressed && playing);
        }
        if whisper {
            self.voice_chat().set_whisper(pressed && playing);
        }
        if !pressed || !playing {
            return;
        }
        if mute {
            self.change_voice(|s| s.muted = !s.muted);
        }
        if disable {
            self.change_voice(|s| s.disabled = !s.disabled);
        }
        if menu {
            self.open_voice_menu();
        }
    }

    fn change_voice(&mut self, change: impl FnOnce(&mut Settings)) {
        let chat = self.voice_chat();
        let mut s = chat.settings().clone();
        change(&mut s);
        chat.apply(s.clone());
        save(&s);
    }

    /// Opens its page in Options > Mods.
    fn open_voice_menu(&mut self) {
        self.list_native_mods();
        let Some(i) = self.settings.mods.installed.iter().position(|m| m.id == ID) else {
            return;
        };
        self.open_options_at(false, Page::NativeMod(i));
        self.options_to_game = true;
        self.native_settings_refresh(i);
    }

    /// Its row in Options > Mods.
    pub(super) fn voicechat_mod(&self, mods: &mut Vec<crate::mod_api::NativeMod>) {
        if !self.voicechat_module {
            return;
        }
        mods.push(crate::mod_api::NativeMod {
            id: ID.into(),
            title: format!("{NAME} 1.0.0"),
            description: crate::lang::get_or(
                "wisp.voicechat.description",
                "Proximity voice chat with players on servers that run Simple Voice Chat.",
            ),
            keys: KEYS
                .iter()
                .map(|(name, label, default)| {
                    let label = crate::lang::get_or(&format!("wisp.voicechat.key.{name}"), label);
                    [(*name).into(), label, (*default).into()]
                })
                .collect(),
        });
    }

    /// The buttons of its page.
    pub(super) fn voicechat_settings(&mut self, id: &str, buttons: &mut Vec<String>) {
        if id != ID {
            return;
        }
        let testing = self.voicechat.testing;
        let chat = self.voice_chat();
        let s = chat.settings().clone();
        let tr = |key: &str, fallback: &str| crate::lang::get_or(key, fallback);
        let on_off = |v: bool| {
            if v {
                tr("options.on", "On")
            } else {
                tr("options.off", "Off")
            }
        };
        let status = match chat.state() {
            State::Connected => format!(
                "{} ({} {:.0})",
                tr("wisp.voicechat.connected", "Connected"),
                tr("wisp.voicechat.distance", "range"),
                chat.distance().unwrap_or(0.0)
            ),
            State::Connecting => tr("wisp.voicechat.connecting", "Connecting…"),
            State::Lost => tr("wisp.voicechat.lost", "Connection lost, retrying"),
            State::Unavailable => tr("wisp.voicechat.unavailable", "Not available on this server"),
        };
        let device = |name: &str| {
            if name.is_empty() {
                tr("wisp.voicechat.default_device", "System default")
            } else {
                name.to_owned()
            }
        };
        buttons.push(format!(
            "{}: {status}",
            tr("wisp.voicechat.status", "Status")
        ));
        buttons.push(format!(
            "{}: {}",
            tr("wisp.voicechat.activation", "Activation"),
            if s.voice_activation {
                tr("wisp.voicechat.voice", "Voice activation")
            } else {
                tr("wisp.voicechat.ptt", "Push to talk")
            }
        ));
        buttons.push(format!(
            "{}: {:.0} dB",
            tr("wisp.voicechat.threshold", "Voice activation threshold"),
            s.threshold_db
        ));
        buttons.push(format!(
            "{}: {:.0}%",
            tr("wisp.voicechat.mic_gain", "Microphone amplification"),
            s.mic_gain * 100.0
        ));
        buttons.push(format!(
            "{}: {:.0}%",
            tr("wisp.voicechat.volume", "Voice chat volume"),
            s.volume * 100.0
        ));
        buttons.push(format!(
            "{}: {}",
            tr("wisp.voicechat.microphone", "Microphone"),
            device(&s.microphone)
        ));
        buttons.push(format!(
            "{}: {}",
            tr("wisp.voicechat.speaker", "Speaker"),
            device(&s.speaker)
        ));
        let level = if testing {
            format!(" ({:.0} dB)", chat.level_db().max(-99.0))
        } else {
            String::new()
        };
        buttons.push(format!(
            "{}: {}{level}",
            tr("wisp.voicechat.test", "Test microphone"),
            on_off(testing)
        ));
        buttons.push(format!(
            "{}: {}",
            tr("wisp.voicechat.muted", "Microphone muted"),
            on_off(s.muted)
        ));
        buttons.push(format!(
            "{}: {}",
            tr("wisp.voicechat.disabled", "Voice chat disabled"),
            on_off(s.disabled)
        ));
        if let Some(e) = chat.mic_error() {
            buttons.push(format!(
                "{}: {e}",
                tr("wisp.voicechat.mic_problem", "Microphone problem")
            ));
        }
    }

    /// A click on button `button` of its page.
    pub(super) fn voicechat_settings_click(&mut self, id: &str, button: usize) {
        if id != ID {
            return;
        }
        let cycle_device = |input: bool, current: &str| -> String {
            let mut names = vec![String::new()];
            names.extend(audio::devices(input));
            let at = names.iter().position(|n| n == current).unwrap_or(0);
            names[(at + 1) % names.len()].clone()
        };
        match button {
            1 => self.change_voice(|s| s.voice_activation = !s.voice_activation),
            2 => self.change_voice(|s| s.threshold_db = next(&THRESHOLDS, s.threshold_db)),
            3 => self.change_voice(|s| s.mic_gain = next(&GAINS, s.mic_gain)),
            4 => self.change_voice(|s| s.volume = next(&VOLUMES, s.volume)),
            5 => self.change_voice(|s| s.microphone = cycle_device(true, &s.microphone)),
            6 => self.change_voice(|s| s.speaker = cycle_device(false, &s.speaker)),
            7 => {
                self.voicechat.testing = !self.voicechat.testing;
                let on = self.voicechat.testing;
                self.voice_chat().set_testing(on);
            }
            8 => self.change_voice(|s| s.muted = !s.muted),
            9 => self.change_voice(|s| s.disabled = !s.disabled),
            _ => {}
        }
    }

    /// The options screen closed: the microphone test ends.
    pub(super) fn voicechat_menu_closed(&mut self) {
        if self.voicechat.testing {
            self.voicechat.testing = false;
            self.voice_chat().set_testing(false);
        }
    }

    /// The microphone's state at the bottom left, and who is talking above it.
    pub(super) fn draw_voicechat(&self, batch: &mut HudBatch) {
        let Some(chat) = self.voicechat.chat.as_ref() else {
            return;
        };
        if !self.voice_on() || self.voicechat.joined.is_none() || chat.state() == State::Unavailable
        {
            return;
        }
        let atlas = &self.gui.atlas;
        let s = chat.settings();
        let tr = |key: &str, fallback: &str| crate::lang::get_or(key, fallback);
        let (label, colour) = match chat.state() {
            State::Connecting => (
                tr("wisp.voicechat.hud.connecting", "Voice: connecting"),
                0xFFAAAAAA,
            ),
            State::Lost => (
                tr("wisp.voicechat.hud.lost", "Voice: disconnected"),
                0xFFFF6B6B,
            ),
            _ if s.disabled => (tr("wisp.voicechat.hud.disabled", "Voice off"), 0xFFFF6B6B),
            _ if chat.mic_error().is_some() => {
                (tr("wisp.voicechat.hud.no_mic", "No microphone"), 0xFFFFB347)
            }
            _ if s.voice_activation && s.muted => {
                (tr("wisp.voicechat.hud.muted", "Muted"), 0xFFFF6B6B)
            }
            _ if chat.transmitting() => (tr("wisp.voicechat.hud.talking", "Talking"), 0xFF5CFF8A),
            _ => (String::new(), 0),
        };
        let x = 4.0;
        let mut y = batch.height - 70.0;
        if !label.is_empty() {
            let w = HudBatch::text_width(atlas, &label);
            batch.fill(atlas, x - 2.0, y - 2.0, w + 14.0, 12.0, 0x80000000);
            batch.fill(atlas, x, y + 1.0, 6.0, 6.0, colour);
            batch.text(atlas, &label, x + 9.0, y, colour, true);
            y -= 14.0;
        }
        let names = self.remote.as_ref().map(|r| &r.players);
        for (uuid, whisper) in chat.talking() {
            let name = names
                .and_then(|n| n.get(&uuid))
                .cloned()
                .or_else(|| chat.player(uuid).map(|p| p.name.clone()))
                .unwrap_or_else(|| tr("wisp.voicechat.someone", "Someone"));
            let colour = if whisper { 0xFFB9A6FF } else { 0xFFFFFFFF };
            let w = HudBatch::text_width(atlas, &name);
            batch.fill(atlas, x - 2.0, y - 2.0, w + 14.0, 12.0, 0x80000000);
            batch.fill(atlas, x, y + 1.0, 6.0, 6.0, 0xFF5CFF8A);
            batch.text(atlas, &name, x + 9.0, y, colour, true);
            y -= 14.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_cycle_and_hosts_lose_their_port() {
        assert_eq!(next(&THRESHOLDS, -50.0), -45.0);
        assert_eq!(next(&THRESHOLDS, -20.0), -80.0);
        assert_eq!(host_of("mc.example.net:25566"), "mc.example.net");
        assert_eq!(host_of("mc.example.net"), "mc.example.net");
        assert_eq!(host_of("[::1]:25565"), "::1");
    }

    #[test]
    fn settings_survive_saving() {
        let mut s = Settings {
            voice_activation: true,
            threshold_db: -40.0,
            ..Settings::default()
        };
        s.players.insert(0xABCD, 0.5);
        let back: Settings =
            serde_json::from_slice::<Saved>(&serde_json::to_vec(&Saved::from(&s)).unwrap())
                .unwrap()
                .into();
        assert_eq!(back, s);
    }
}
