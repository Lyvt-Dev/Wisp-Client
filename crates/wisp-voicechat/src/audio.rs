//! The speaker (decode, place and mix the voices that arrive) and the microphone (gate by
//! push-to-talk or voice activation, encode, send).
use crate::connection::MicSender;
use crate::protocol::{Codec, Placement, Sound};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const RATE: u32 = 48_000;
pub const FRAME: usize = 960;
/// Samples a voice gathers before it starts to play, against network jitter (60 ms).
const PRIME: usize = FRAME * 3;
/// The most a voice may lag behind before its oldest audio is dropped (400 ms).
const MAX_QUEUED: usize = RATE as usize * 2 / 5;
/// The sender id of the microphone test, played back to yourself.
pub const SELF_TEST: u128 = 0;

/// An f32 shared between threads.
#[derive(Default)]
pub struct AtomicF32(AtomicU32);
impl AtomicF32 {
    pub fn new(v: f32) -> Self {
        Self(AtomicU32::new(v.to_bits()))
    }
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
    pub fn set(&self, v: f32) {
        self.0.store(v.to_bits(), Ordering::Relaxed);
    }
}

/// What the game sets and the microphone reads, and what the microphone reports back.
#[derive(Default)]
pub struct Controls {
    pub push_to_talk: AtomicBool,
    pub whisper: AtomicBool,
    pub muted: AtomicBool,
    pub disabled: AtomicBool,
    /// Voice activation instead of push-to-talk.
    pub voice_activation: AtomicBool,
    /// The level (dBFS) voice activation opens at.
    pub threshold_db: AtomicF32,
    /// Microphone amplification, as a factor.
    pub mic_gain: AtomicF32,
    /// Hear your own microphone (the settings' test).
    pub testing: AtomicBool,
    /// The last frame's peak level, dBFS.
    pub level_db: AtomicF32,
    pub transmitting: AtomicBool,
}

/// Where the listener is: position and yaw (degrees, Minecraft's: 0 faces +z).
#[derive(Clone, Copy, Debug, Default)]
pub struct Listener {
    pub position: [f64; 3],
    pub yaw: f32,
}

/// Left and right gains for a sound at `at` heard by `listener`, fading linearly to nothing at
/// `distance`; None past `distance + 1`.
pub fn spatial(listener: Listener, at: [f64; 3], distance: f32) -> Option<[f32; 2]> {
    let d = [
        at[0] - listener.position[0],
        at[1] - listener.position[1],
        at[2] - listener.position[2],
    ];
    let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let distance = f64::from(distance.max(1.0));
    if length > distance + 1.0 {
        return None;
    }
    let fade = (1.0 - length.min(distance) / distance) as f32;
    let flat = d[0].hypot(d[2]);
    if flat < 1e-3 {
        return Some([fade, fade]);
    }
    let yaw = f64::from(listener.yaw).to_radians();
    // Facing (-sin, cos); the right hand points (-cos, -sin).
    let pan = ((d[0] * -yaw.cos() + d[2] * -yaw.sin()) / flat) as f32;
    // Sounds overhead or below pan less.
    let pan = pan * (1.0 - (d[1].abs() / 32.0).min(1.0) as f32);
    let left = 1.0 - 0.7 * pan.max(0.0);
    let right = 1.0 - 0.7 * (-pan).max(0.0);
    Some([left * fade, right * fade])
}

struct Voice {
    queue: VecDeque<f32>,
    /// Fraction of a sample played into the head of the queue.
    phase: f64,
    playing: bool,
    gain: [f32; 2],
    placement: Placement,
    sender: u128,
    last: Instant,
    ended: bool,
}

#[derive(Default)]
struct Mixer {
    voices: HashMap<u128, Voice>,
    volume: f32,
    /// Per-player volumes (factors).
    players: HashMap<u128, f32>,
}

impl Mixer {
    fn voice(&mut self, channel: u128, sender: u128, placement: Placement) -> &mut Voice {
        let v = self.voices.entry(channel).or_insert_with(|| Voice {
            queue: VecDeque::new(),
            phase: 0.0,
            playing: false,
            gain: if matches!(placement, Placement::Group) {
                [1.0; 2]
            } else {
                [0.0; 2]
            },
            placement,
            sender,
            last: Instant::now(),
            ended: false,
        });
        v.placement = placement;
        v.sender = sender;
        v.last = Instant::now();
        v
    }

    fn push(&mut self, channel: u128, sender: u128, placement: Placement, samples: &[f32]) {
        let v = self.voice(channel, sender, placement);
        v.ended = false;
        v.queue.extend(samples);
        let over = v.queue.len().saturating_sub(MAX_QUEUED);
        v.queue.drain(..over);
    }

    /// One output frame: the sum of every voice, resampled from 48 kHz by `step`.
    fn next(&mut self, step: f64) -> [f32; 2] {
        let mut out = [0.0f32; 2];
        let master = self.volume;
        for v in self.voices.values_mut() {
            if !v.playing {
                if v.queue.len() >= PRIME || (v.ended && !v.queue.is_empty()) {
                    v.playing = true;
                } else {
                    continue;
                }
            }
            if v.queue.len() < 2 {
                v.playing = false;
                continue;
            }
            let t = v.phase as f32;
            let s = v.queue[0] * (1.0 - t) + v.queue[1] * t;
            let volume = master * self.players.get(&v.sender).copied().unwrap_or(1.0);
            out[0] += s * v.gain[0] * volume;
            out[1] += s * v.gain[1] * volume;
            v.phase += step;
            while v.phase >= 1.0 && !v.queue.is_empty() {
                v.phase -= 1.0;
                v.queue.pop_front();
            }
        }
        [out[0].clamp(-1.0, 1.0), out[1].clamp(-1.0, 1.0)]
    }
}

/// An output stream only kept open. Android's AAudio stream is `Send` but not `Sync`; nothing
/// can be called through a shared reference to this, so sharing one is harmless.
struct Playing(#[allow(dead_code)] cpal::Stream);
// SAFETY: the stream is private and never used after it starts, so no `&Playing` reaches it.
unsafe impl Sync for Playing {}

/// The voices: decoded on a thread of their own, mixed into an output stream of their own.
pub struct Speaker {
    mixer: Arc<Mutex<Mixer>>,
    _stream: Option<Playing>,
    decoder: Option<Sender<Sound>>,
}

impl Speaker {
    /// Opens `device` (the default when empty or missing). Without a device it stays silent.
    pub fn open(device: &str, volume: f32) -> Self {
        let mixer = Arc::new(Mutex::new(Mixer {
            volume,
            ..Mixer::default()
        }));
        let stream = match open_output(device, Arc::clone(&mixer)) {
            Ok(s) => Some(s),
            Err(e) => {
                log::warn!("Wisp voice chat: no speaker: {e}");
                None
            }
        };
        Self {
            mixer,
            _stream: stream.map(Playing),
            decoder: None,
        }
    }

    /// A sender of received voice, decoded on a new thread.
    pub fn decoder(&mut self) -> Sender<Sound> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mixer = Arc::clone(&self.mixer);
        let _ = std::thread::Builder::new()
            .name("voicechat-decode".into())
            .spawn(move || decode(&rx, &mixer));
        self.decoder = Some(tx.clone());
        tx
    }

    pub fn set_volume(&self, volume: f32) {
        if let Ok(mut m) = self.mixer.lock() {
            m.volume = volume;
        }
    }

    pub fn set_player_volume(&self, player: u128, volume: f32) {
        if let Ok(mut m) = self.mixer.lock() {
            m.players.insert(player, volume);
        }
    }

    /// Plays the microphone back (the settings' test).
    pub fn push_self(&self, samples: &[f32]) {
        if let Ok(mut m) = self.mixer.lock() {
            m.push(SELF_TEST, SELF_TEST, Placement::Group, samples);
        }
    }

    /// Places every voice for `listener`: a player's at where `position` finds them.
    pub fn spatialize(&self, listener: Listener, position: &dyn Fn(u128) -> Option<[f64; 3]>) {
        let Ok(mut m) = self.mixer.lock() else {
            return;
        };
        m.voices
            .retain(|_, v| v.last.elapsed() < Duration::from_secs(30));
        for v in m.voices.values_mut() {
            v.gain = match v.placement {
                Placement::Group => [1.0; 2],
                Placement::Location { at, distance } => {
                    spatial(listener, at, distance).unwrap_or([0.0; 2])
                }
                Placement::Player { distance, .. } => position(v.sender)
                    .and_then(|at| spatial(listener, at, distance))
                    // The sender may be out of the client's view distance: heard plainly.
                    .unwrap_or(if position(v.sender).is_none() {
                        [0.6; 2]
                    } else {
                        [0.0; 2]
                    }),
            };
        }
    }

    /// The players heard right now, and whether each whispers.
    pub fn talking(&self) -> Vec<(u128, bool)> {
        let Ok(m) = self.mixer.lock() else {
            return Vec::new();
        };
        m.voices
            .values()
            .filter(|v| v.sender != SELF_TEST)
            .filter(|v| {
                !v.queue.is_empty() || (!v.ended && v.last.elapsed() < Duration::from_millis(250))
            })
            .map(|v| {
                let whisper = matches!(
                    v.placement,
                    Placement::Player {
                        whispering: true,
                        ..
                    }
                );
                (v.sender, whisper)
            })
            .collect()
    }
}

fn decode(rx: &Receiver<Sound>, mixer: &Mutex<Mixer>) {
    struct State {
        decoder: opus::Decoder,
        next: Option<i64>,
    }
    let mut states: HashMap<u128, State> = HashMap::new();
    let mut pcm = vec![0i16; FRAME * 6];
    while let Ok(sound) = rx.recv() {
        let state = match states.entry(sound.channel) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                match opus::Decoder::new(RATE, opus::Channels::Mono) {
                    Ok(decoder) => e.insert(State {
                        decoder,
                        next: None,
                    }),
                    Err(err) => {
                        log::warn!("Wisp voice chat: no decoder: {err}");
                        continue;
                    }
                }
            }
        };
        let mut samples: Vec<f32> = Vec::new();
        if sound.opus.is_empty() {
            // The end of a stretch of speech.
            let _ = state.decoder.reset_state();
            state.next = None;
            if let Ok(mut m) = mixer.lock() {
                m.voice(sound.channel, sound.sender, sound.placement).ended = true;
            }
            continue;
        }
        if let Some(next) = state.next {
            if sound.sequence < next {
                continue;
            }
            let lost = sound.sequence - next;
            if lost > 5 {
                let _ = state.decoder.reset_state();
            } else {
                for _ in 0..lost {
                    if let Ok(n) = state.decoder.decode(&[], &mut pcm, false) {
                        samples.extend(pcm[..n].iter().map(|&s| f32::from(s) / 32768.0));
                    }
                }
            }
        }
        state.next = Some(sound.sequence + 1);
        match state.decoder.decode(&sound.opus, &mut pcm, false) {
            Ok(n) => samples.extend(pcm[..n].iter().map(|&s| f32::from(s) / 32768.0)),
            Err(e) => log::debug!("Voice chat: undecodable frame: {e}"),
        }
        if let Ok(mut m) = mixer.lock() {
            m.push(sound.channel, sound.sender, sound.placement, &samples);
        }
    }
}

fn find_device(input: bool, name: &str) -> Option<cpal::Device> {
    let host = cpal::default_host();
    if !name.is_empty() {
        let devices = if input {
            host.input_devices()
        } else {
            host.output_devices()
        };
        if let Some(d) = devices
            .ok()
            .and_then(|mut ds| ds.find(|d| d.name().is_ok_and(|n| n == name)))
        {
            return Some(d);
        }
    }
    if input {
        host.default_input_device()
    } else {
        host.default_output_device()
    }
}

/// The names of the microphones, or the speakers.
pub fn devices(input: bool) -> Vec<String> {
    let host = cpal::default_host();
    let devices = if input {
        host.input_devices()
    } else {
        host.output_devices()
    };
    devices
        .map(|ds| ds.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

fn open_output(device: &str, mixer: Arc<Mutex<Mixer>>) -> Result<cpal::Stream, String> {
    let device = find_device(false, device).ok_or("no output device")?;
    let config = device.default_output_config().map_err(|e| e.to_string())?;
    let channels = usize::from(config.channels());
    let step = f64::from(RATE) / f64::from(config.sample_rate().0);
    let stream_config: cpal::StreamConfig = config.clone().into();
    let err = |e| log::warn!("Wisp voice chat: speaker: {e}");
    macro_rules! build {
        ($t:ty) => {
            device.build_output_stream(
                &stream_config,
                move |data: &mut [$t], _| {
                    let Ok(mut m) = mixer.try_lock() else {
                        data.fill(<$t as cpal::Sample>::EQUILIBRIUM);
                        return;
                    };
                    for frame in data.chunks_mut(channels) {
                        let [l, r] = m.next(step);
                        for (i, s) in frame.iter_mut().enumerate() {
                            let v = match (channels, i) {
                                (1, _) => (l + r) * 0.5,
                                (_, 0) => l,
                                (_, 1) => r,
                                _ => 0.0,
                            };
                            *s = <$t as cpal::FromSample<f32>>::from_sample_(v);
                        }
                    }
                },
                err,
                None,
            )
        };
    }
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build!(f32),
        cpal::SampleFormat::I16 => build!(i16),
        cpal::SampleFormat::U16 => build!(u16),
        cpal::SampleFormat::I32 => build!(i32),
        other => return Err(format!("unsupported sample format {other}")),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

/// The microphone: captured on its device's stream, processed on a thread of its own.
pub struct Mic {
    _stream: cpal::Stream,
}

impl Mic {
    /// Opens `device` (the default when empty or missing) and starts sending to `sender`.
    pub fn open(
        device: &str,
        codec: Codec,
        controls: Arc<Controls>,
        sender: MicSender,
        echo: Option<Arc<Speaker>>,
    ) -> Result<Self, String> {
        let device = find_device(true, device).ok_or("no microphone")?;
        let config = device.default_input_config().map_err(|e| e.to_string())?;
        let channels = usize::from(config.channels());
        let rate = config.sample_rate().0;
        let stream_config: cpal::StreamConfig = config.clone().into();
        let (tx, rx) = std::sync::mpsc::channel::<Vec<f32>>();
        let err = |e| log::warn!("Wisp voice chat: microphone: {e}");
        macro_rules! build {
            ($t:ty) => {
                device.build_input_stream(
                    &stream_config,
                    move |data: &[$t], _| {
                        let mono = data
                            .chunks(channels)
                            .map(|f| {
                                f.iter()
                                    .map(|&s| <f32 as cpal::FromSample<$t>>::from_sample_(s))
                                    .sum::<f32>()
                                    / channels as f32
                            })
                            .collect();
                        let _ = tx.send(mono);
                    },
                    err,
                    None,
                )
            };
        }
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => build!(f32),
            cpal::SampleFormat::I16 => build!(i16),
            cpal::SampleFormat::U16 => build!(u16),
            cpal::SampleFormat::I32 => build!(i32),
            other => return Err(format!("unsupported sample format {other}")),
        }
        .map_err(|e| e.to_string())?;
        let application = match codec {
            Codec::Voip => opus::Application::Voip,
            Codec::Audio => opus::Application::Audio,
            Codec::LowDelay => opus::Application::LowDelay,
        };
        let encoder = opus::Encoder::new(RATE, opus::Channels::Mono, application)
            .map_err(|e| e.to_string())?;
        std::thread::Builder::new()
            .name("voicechat-mic".into())
            .spawn(move || process(&rx, rate, encoder, &controls, &sender, echo.as_deref()))
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        log::info!("Wisp voice chat: microphone open at {rate} Hz");
        Ok(Self { _stream: stream })
    }
}

/// Frames voice activation keeps sending after the voice drops (500 ms), and push-to-talk after
/// the key is let go (100 ms).
const VOICE_HOLD: u32 = 25;
const PTT_HOLD: u32 = 5;

/// Whether a frame goes out: push-to-talk or voice activation, held a little after it stops.
#[derive(Default)]
pub struct Gate {
    hold: u32,
}

impl Gate {
    pub fn open(&mut self, controls: &Controls, level_db: f32) -> bool {
        // Muted or off sends nothing, in either mode and while whispering, as the original does.
        if controls.disabled.load(Ordering::Relaxed) || controls.muted.load(Ordering::Relaxed) {
            self.hold = 0;
            return false;
        }
        let voice = controls.voice_activation.load(Ordering::Relaxed);
        let whisper = controls.whisper.load(Ordering::Relaxed);
        let (on, hold) = if voice {
            (level_db > controls.threshold_db.get(), VOICE_HOLD)
        } else {
            (controls.push_to_talk.load(Ordering::Relaxed), PTT_HOLD)
        };
        if on || whisper {
            self.hold = hold;
            return true;
        }
        if self.hold > 0 {
            self.hold -= 1;
            return true;
        }
        false
    }
}

fn process(
    rx: &Receiver<Vec<f32>>,
    rate: u32,
    mut encoder: opus::Encoder,
    controls: &Controls,
    sender: &MicSender,
    echo: Option<&Speaker>,
) {
    let step = f64::from(rate) / f64::from(RATE);
    let mut input: VecDeque<f32> = VecDeque::new();
    let mut phase = 0.0f64;
    let mut frame: Vec<f32> = Vec::with_capacity(FRAME);
    let mut gate = Gate::default();
    let mut sending = false;
    let mut sequence = 0i64;
    let mut out = vec![0u8; crate::protocol::MAX_OPUS];
    while let Ok(chunk) = rx.recv() {
        if sender.closed() {
            return;
        }
        input.extend(chunk);
        // Resample to 48 kHz.
        while input.len() >= 2 {
            let t = phase as f32;
            frame.push(input[0] * (1.0 - t) + input[1] * t);
            phase += step;
            while phase >= 1.0 && input.len() >= 2 {
                phase -= 1.0;
                input.pop_front();
            }
            if frame.len() < FRAME {
                continue;
            }
            let gain = controls.mic_gain.get();
            let pcm: Vec<i16> = frame
                .iter()
                .map(|&s| ((s * gain).clamp(-1.0, 1.0) * 32767.0) as i16)
                .collect();
            frame.clear();
            let peak = pcm.iter().map(|&s| i32::from(s).abs()).max().unwrap_or(0);
            let level = if peak == 0 {
                -127.0
            } else {
                20.0 * (peak as f32 / 32767.0).log10()
            };
            controls.level_db.set(level);
            if controls.testing.load(Ordering::Relaxed)
                && let Some(speaker) = echo
            {
                let samples: Vec<f32> = pcm.iter().map(|&s| f32::from(s) / 32768.0).collect();
                speaker.push_self(&samples);
            }
            let open = gate.open(controls, level);
            controls.transmitting.store(open, Ordering::Relaxed);
            if open {
                match encoder.encode(&pcm, &mut out) {
                    Ok(n) => {
                        let whisper = controls.whisper.load(Ordering::Relaxed);
                        sender.send(out[..n].to_vec(), sequence, whisper);
                        sequence += 1;
                        sending = true;
                    }
                    Err(e) => log::debug!("Voice chat: encode failed: {e}"),
                }
            } else if sending {
                // The end of speech: an empty frame, and a fresh encoder for the next.
                sender.send(Vec::new(), sequence, false);
                sequence += 1;
                sending = false;
                let _ = encoder.reset_state();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sounds_fade_with_distance_and_pan_to_their_side() {
        let me = Listener {
            position: [0.0, 64.0, 0.0],
            yaw: 0.0, // facing +z
        };
        let ahead = spatial(me, [0.0, 64.0, 10.0], 48.0).unwrap();
        assert!((ahead[0] - ahead[1]).abs() < 1e-4);
        assert!(spatial(me, [0.0, 64.0, 50.0], 48.0).is_none());
        let near = spatial(me, [0.0, 64.0, 2.0], 48.0).unwrap();
        assert!(near[0] > ahead[0]);
        // Facing +z, -x is on the right.
        let right = spatial(me, [-5.0, 64.0, 0.0], 48.0).unwrap();
        assert!(right[1] > right[0]);
        let left = spatial(me, [5.0, 64.0, 0.0], 48.0).unwrap();
        assert!(left[0] > left[1]);
    }

    #[test]
    fn the_gate_follows_the_activation_mode() {
        let c = Controls {
            threshold_db: AtomicF32::new(-50.0),
            ..Controls::default()
        };
        let mut gate = Gate::default();
        assert!(!gate.open(&c, 0.0));
        c.push_to_talk.store(true, Ordering::Relaxed);
        assert!(gate.open(&c, -127.0));
        c.push_to_talk.store(false, Ordering::Relaxed);
        for _ in 0..PTT_HOLD {
            assert!(gate.open(&c, -127.0));
        }
        assert!(!gate.open(&c, -127.0));
        c.voice_activation.store(true, Ordering::Relaxed);
        assert!(gate.open(&c, -20.0));
        assert!(!{
            let mut g = Gate::default();
            g.open(&c, -60.0)
        });
        c.muted.store(true, Ordering::Relaxed);
        let mut g = Gate::default();
        assert!(!g.open(&c, -20.0));
        c.disabled.store(true, Ordering::Relaxed);
        c.whisper.store(true, Ordering::Relaxed);
        assert!(!g.open(&c, -20.0));
    }

    #[test]
    fn muted_sends_nothing_by_push_to_talk_or_whisper() {
        let c = Controls::default();
        c.muted.store(true, Ordering::Relaxed);
        c.push_to_talk.store(true, Ordering::Relaxed);
        let mut g = Gate::default();
        assert!(!g.open(&c, 0.0));
        c.whisper.store(true, Ordering::Relaxed);
        assert!(!g.open(&c, 0.0));
        // Muting while talking cuts off at once, without the hold.
        c.muted.store(false, Ordering::Relaxed);
        assert!(g.open(&c, 0.0));
        c.muted.store(true, Ordering::Relaxed);
        assert!(!g.open(&c, 0.0));
    }

    #[test]
    fn opus_round_trips_a_frame() {
        let mut enc =
            opus::Encoder::new(RATE, opus::Channels::Mono, opus::Application::Voip).unwrap();
        let mut dec = opus::Decoder::new(RATE, opus::Channels::Mono).unwrap();
        let pcm: Vec<i16> = (0..FRAME)
            .map(|i| ((i as f32 * 0.05).sin() * 8000.0) as i16)
            .collect();
        let mut packet = vec![0u8; 1275];
        let n = enc.encode(&pcm, &mut packet).unwrap();
        assert!(n > 0 && n <= 1275);
        let mut out = vec![0i16; FRAME];
        assert_eq!(dec.decode(&packet[..n], &mut out, false).unwrap(), FRAME);
    }

    #[test]
    fn the_mixer_primes_then_plays_and_drops_old_audio() {
        let mut m = Mixer {
            volume: 1.0,
            ..Mixer::default()
        };
        m.push(1, 1, Placement::Group, &vec![0.5; FRAME]);
        assert_eq!(m.next(1.0), [0.0, 0.0]);
        m.push(1, 1, Placement::Group, &vec![0.5; PRIME]);
        let out = m.next(1.0);
        assert!((out[0] - 0.5).abs() < 1e-4);
        m.push(1, 1, Placement::Group, &vec![0.0; MAX_QUEUED * 2]);
        assert_eq!(m.voices[&1].queue.len(), MAX_QUEUED);
    }
}
