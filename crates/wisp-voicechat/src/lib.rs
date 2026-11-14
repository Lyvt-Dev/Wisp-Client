//! Wisp's proximity voice chat, compatible with servers that run Simple Voice Chat
//! (`docs/voicechat-protocol.md`). A clean-room implementation for interoperability: no code or
//! assets of the original mod.
//!
//! The game forwards the `voicechat:*` plugin channel payloads ([`VoiceChat::payload`]), sends
//! what [`VoiceChat::take_outgoing`] gives it, and each tick tells it where the listener and the
//! players are ([`VoiceChat::tick`]).
pub mod audio;
pub mod connection;
pub mod protocol;

use audio::{AtomicF32, Controls, Listener, Mic, Speaker};
use connection::{Connection, Resolve, Status};
use protocol::{PlayerState, Secret};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

/// The player's voice chat settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub voice_activation: bool,
    pub threshold_db: f32,
    /// Microphone amplification, as a factor (1 = unchanged).
    pub mic_gain: f32,
    /// Everyone's voice volume, as a factor.
    pub volume: f32,
    pub muted: bool,
    pub disabled: bool,
    pub microphone: String,
    pub speaker: String,
    /// Per-player volumes.
    pub players: HashMap<u128, f32>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            voice_activation: false,
            threshold_db: -50.0,
            mic_gain: 1.0,
            volume: 1.0,
            muted: false,
            disabled: false,
            microphone: String::new(),
            speaker: String::new(),
            players: HashMap::new(),
        }
    }
}

/// What the HUD shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// The server has no voice chat (or hasn't answered yet).
    Unavailable,
    Connecting,
    Connected,
    /// The connection dropped; it retries.
    Lost,
}

struct Session {
    connection: Connection,
    speaker: Arc<Speaker>,
    _mic: Option<Mic>,
    secret: Secret,
    mic_error: Option<String>,
}

pub struct VoiceChat {
    settings: Settings,
    controls: Arc<Controls>,
    server_host: String,
    resolve: Resolve,
    session: Option<Session>,
    outgoing: Vec<(&'static str, Vec<u8>)>,
    /// When to ask for a new secret after the connection was lost.
    retry_at: Option<Instant>,
    states: HashMap<u128, PlayerState>,
    /// Whether the server answered the request: it runs voice chat.
    available: bool,
}

impl VoiceChat {
    pub fn new(settings: Settings) -> Self {
        let controls = Arc::new(Controls {
            threshold_db: AtomicF32::new(settings.threshold_db),
            mic_gain: AtomicF32::new(settings.mic_gain),
            ..Controls::default()
        });
        let mut v = Self {
            settings: Settings::default(),
            controls,
            server_host: String::new(),
            resolve: Arc::new(|h: &str| h.to_owned()),
            session: None,
            outgoing: Vec::new(),
            retry_at: None,
            states: HashMap::new(),
            available: false,
        };
        v.apply(settings);
        v
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn controls(&self) -> &Arc<Controls> {
        &self.controls
    }

    /// Takes new settings; a changed device reopens it, a changed mute state reaches the server.
    pub fn apply(&mut self, settings: Settings) {
        let c = &self.controls;
        c.voice_activation
            .store(settings.voice_activation, Ordering::Relaxed);
        c.threshold_db.set(settings.threshold_db);
        c.mic_gain.set(settings.mic_gain);
        c.muted.store(settings.muted, Ordering::Relaxed);
        c.disabled.store(settings.disabled, Ordering::Relaxed);
        let state_changed =
            settings.muted != self.settings.muted || settings.disabled != self.settings.disabled;
        let devices_changed = settings.microphone != self.settings.microphone
            || settings.speaker != self.settings.speaker;
        if let Some(s) = &self.session {
            s.speaker.set_volume(if settings.disabled {
                0.0
            } else {
                settings.volume
            });
            for (&p, &v) in &settings.players {
                s.speaker.set_player_volume(p, v);
            }
        }
        self.settings = settings;
        if state_changed && self.available {
            self.send_state();
        }
        if devices_changed && let Some(s) = self.session.take() {
            let secret = s.secret.clone();
            drop(s);
            self.start(secret);
        }
    }

    fn send_state(&mut self) {
        let payload = protocol::client_state(self.settings.muted, self.settings.disabled);
        self.outgoing.push((protocol::CLIENT_STATE, payload));
    }

    /// The player joined a server (`host` as they typed it, without the port; `resolve` follows
    /// it through an SRV redirect): announce the channels and ask for the secret.
    pub fn joined(&mut self, host: &str, resolve: Resolve) {
        self.leave();
        self.server_host = host.to_owned();
        self.resolve = resolve;
        self.outgoing
            .push(("minecraft:register", protocol::register()));
        self.outgoing
            .push((protocol::REQUEST_SECRET, protocol::request_secret()));
    }

    /// The player left the server.
    pub fn leave(&mut self) {
        self.session = None;
        self.available = false;
        self.retry_at = None;
        self.states.clear();
        self.controls.transmitting.store(false, Ordering::Relaxed);
    }

    /// The custom payloads to send: (channel, data).
    pub fn take_outgoing(&mut self) -> Vec<(&'static str, Vec<u8>)> {
        std::mem::take(&mut self.outgoing)
    }

    /// A custom payload from the server; true when it was voice chat's.
    pub fn payload(&mut self, channel: &str, data: &[u8]) -> bool {
        match channel {
            protocol::SECRET => match Secret::read(data) {
                Ok(secret) => {
                    self.available = true;
                    self.send_state();
                    self.start(secret);
                }
                Err(e) => log::warn!("Wisp voice chat: bad secret: {e}"),
            },
            protocol::STATE => {
                if let Ok(s) = PlayerState::read_one(data) {
                    self.states.insert(s.player, s);
                }
            }
            protocol::STATES => {
                if let Ok(all) = PlayerState::read_all(data) {
                    self.states = all.into_iter().map(|s| (s.player, s)).collect();
                }
            }
            protocol::REMOVE_STATE => {
                if let Ok(player) = protocol::read_removed_state(data) {
                    self.states.remove(&player);
                }
            }
            protocol::JOINED_GROUP => {}
            _ => return false,
        }
        true
    }

    fn start(&mut self, secret: Secret) {
        self.session = None;
        let mut speaker = Speaker::open(
            &self.settings.speaker,
            if self.settings.disabled {
                0.0
            } else {
                self.settings.volume
            },
        );
        for (&p, &v) in &self.settings.players {
            speaker.set_player_volume(p, v);
        }
        let sounds = speaker.decoder();
        let connection = match Connection::open(
            &secret,
            &self.server_host,
            Arc::clone(&self.resolve),
            sounds,
        ) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Wisp voice chat: can't reach the voice server: {e}");
                self.retry_at = Some(Instant::now() + Duration::from_secs(10));
                return;
            }
        };
        let speaker = Arc::new(speaker);
        let (mic, mic_error) = match Mic::open(
            &self.settings.microphone,
            secret.codec,
            Arc::clone(&self.controls),
            connection.mic(),
            Some(Arc::clone(&speaker)),
        ) {
            Ok(m) => (Some(m), None),
            Err(e) => {
                log::warn!("Wisp voice chat: no microphone: {e}");
                (None, Some(e))
            }
        };
        self.session = Some(Session {
            connection,
            speaker,
            _mic: mic,
            secret,
            mic_error,
        });
    }

    /// Once a tick: place the voices and notice a lost connection.
    pub fn tick(&mut self, listener: Listener, position: &dyn Fn(u128) -> Option<[f64; 3]>) {
        if let Some(at) = self.retry_at
            && Instant::now() >= at
        {
            self.retry_at = None;
            self.outgoing
                .push((protocol::REQUEST_SECRET, protocol::request_secret()));
        }
        let Some(s) = &self.session else {
            return;
        };
        s.speaker.spatialize(listener, position);
        if s.connection.status() == Status::Lost {
            log::warn!("Wisp voice chat: connection lost; retrying in 10 s");
            self.session = None;
            self.controls.transmitting.store(false, Ordering::Relaxed);
            self.retry_at = Some(Instant::now() + Duration::from_secs(10));
        }
    }

    pub fn state(&self) -> State {
        match &self.session {
            Some(s) => match s.connection.status() {
                Status::Connected => State::Connected,
                Status::Connecting => State::Connecting,
                Status::Lost => State::Lost,
            },
            None if self.retry_at.is_some() => State::Lost,
            None => State::Unavailable,
        }
    }

    /// Why the microphone isn't working, if it isn't.
    pub fn mic_error(&self) -> Option<&str> {
        self.session.as_ref()?.mic_error.as_deref()
    }

    pub fn set_push_to_talk(&self, down: bool) {
        self.controls.push_to_talk.store(down, Ordering::Relaxed);
    }

    pub fn set_whisper(&self, down: bool) {
        self.controls.whisper.store(down, Ordering::Relaxed);
    }

    pub fn set_testing(&self, on: bool) {
        self.controls.testing.store(on, Ordering::Relaxed);
    }

    pub fn transmitting(&self) -> bool {
        self.controls.transmitting.load(Ordering::Relaxed)
    }

    pub fn level_db(&self) -> f32 {
        self.controls.level_db.get()
    }

    /// The players heard right now, and whether each whispers.
    pub fn talking(&self) -> Vec<(u128, bool)> {
        self.session
            .as_ref()
            .map(|s| s.speaker.talking())
            .unwrap_or_default()
    }

    /// A player's voice chat state, as the server last sent it.
    pub fn player(&self, uuid: u128) -> Option<&PlayerState> {
        self.states.get(&uuid)
    }

    /// The voice distance the server set.
    pub fn distance(&self) -> Option<f64> {
        self.session.as_ref().map(|s| s.secret.distance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joining_announces_the_channels_and_asks_for_the_secret() {
        let mut v = VoiceChat::new(Settings::default());
        v.joined("mc.example.net", Arc::new(|h: &str| h.to_owned()));
        let out = v.take_outgoing();
        assert_eq!(out[0].0, "minecraft:register");
        assert!(
            String::from_utf8(out[0].1.clone())
                .unwrap()
                .contains("voicechat:secret")
        );
        assert_eq!(
            out[1],
            (protocol::REQUEST_SECRET, 21i32.to_be_bytes().to_vec())
        );
        assert_eq!(v.state(), State::Unavailable);
        assert!(!v.payload("other:thing", &[]));
    }

    #[test]
    fn mute_changes_reach_the_server_once_it_runs_voice_chat() {
        let mut v = VoiceChat::new(Settings::default());
        v.apply(Settings {
            muted: true,
            ..Settings::default()
        });
        assert!(v.take_outgoing().is_empty());
        v.available = true;
        v.apply(Settings::default());
        assert_eq!(v.take_outgoing(), [(protocol::CLIENT_STATE, vec![0, 0])]);
    }

    #[test]
    fn a_removed_state_forgets_the_player() {
        let mut v = VoiceChat::new(Settings::default());
        let mut w = protocol::Writer::default();
        w.u8(0).uuid(42).utf("Alex");
        assert!(v.payload(protocol::STATE, &w.data));
        assert!(v.player(42).is_some());
        assert!(v.payload(protocol::REMOVE_STATE, &42u128.to_be_bytes()));
        assert!(v.player(42).is_none());
        // A short payload is ignored, not a crash.
        assert!(v.payload(protocol::REMOVE_STATE, &[1, 2]));
    }
}
