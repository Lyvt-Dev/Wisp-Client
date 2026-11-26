//! The wire formats of `docs/voicechat-protocol.md`: the plugin channel payloads and the
//! encrypted UDP packets.
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes128Gcm, Nonce};

/// The protocol versions this client speaks: 20 is every released Simple Voice Chat 2.6.x, 21 the
/// 2.7 prereleases. A server only answers a request for its own version, so the client asks for
/// 20 first and falls back to 21.
pub const VERSION_RELEASE: i32 = 20;
pub const VERSION_PRERELEASE: i32 = 21;
/// The plugin channels.
pub const REQUEST_SECRET: &str = "voicechat:request_secret";
pub const SECRET: &str = "voicechat:secret";
/// Version 21's mute and disable state.
pub const CLIENT_STATE: &str = "voicechat:client_state";
/// Version 20's: only disabled.
pub const UPDATE_STATE: &str = "voicechat:update_state";
pub const STATE: &str = "voicechat:state";
pub const STATES: &str = "voicechat:states";
pub const JOINED_GROUP: &str = "voicechat:joined_group";
/// A player left: their uuid.
pub const REMOVE_STATE: &str = "voicechat:remove_state";
/// Every channel, for `minecraft:register`.
pub const CHANNELS: [&str; 8] = [
    REQUEST_SECRET,
    SECRET,
    CLIENT_STATE,
    UPDATE_STATE,
    STATE,
    STATES,
    REMOVE_STATE,
    JOINED_GROUP,
];

/// `voicechat:remove_state`: the player whose state goes.
pub fn read_removed_state(data: &[u8]) -> Result<u128, Error> {
    Reader::new(data).uuid()
}

/// The largest Opus frame a packet carries.
pub const MAX_OPUS: usize = 1275;
const MAGIC: u8 = 0xFF;
const IV: usize = 12;
const MAX_DATAGRAM: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
fn err<T>(what: &str) -> Result<T, Error> {
    Err(Error(what.to_owned()))
}

/// A reader over Minecraft's buffer encodings.
pub struct Reader<'a> {
    data: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }
    pub fn remaining(&self) -> usize {
        self.data.len()
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        if self.data.len() < n {
            return err("packet too short");
        }
        let (head, rest) = self.data.split_at(n);
        self.data = rest;
        Ok(head)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        Ok(self.take(N)?.try_into().expect("length checked"))
    }
    pub fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> Result<bool, Error> {
        Ok(self.u8()? != 0)
    }
    pub fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_be_bytes(self.array()?))
    }
    pub fn i64(&mut self) -> Result<i64, Error> {
        Ok(i64::from_be_bytes(self.array()?))
    }
    pub fn f32(&mut self) -> Result<f32, Error> {
        Ok(f32::from_be_bytes(self.array()?))
    }
    pub fn f64(&mut self) -> Result<f64, Error> {
        Ok(f64::from_be_bytes(self.array()?))
    }
    pub fn uuid(&mut self) -> Result<u128, Error> {
        Ok(u128::from_be_bytes(self.array()?))
    }
    pub fn raw16(&mut self) -> Result<[u8; 16], Error> {
        self.array()
    }
    pub fn varint(&mut self) -> Result<i32, Error> {
        let mut value = 0u32;
        for i in 0..5 {
            let b = self.u8()?;
            value |= u32::from(b & 0x7F) << (7 * i);
            if b & 0x80 == 0 {
                return Ok(value as i32);
            }
        }
        err("varint too long")
    }
    pub fn bytes(&mut self, max: usize) -> Result<&'a [u8], Error> {
        let n = self.varint()?;
        let n = usize::try_from(n).map_err(|_| Error("negative length".into()))?;
        if n > max {
            return err("byte array too long");
        }
        self.take(n)
    }
    pub fn utf(&mut self, max_chars: usize) -> Result<String, Error> {
        let bytes = self.bytes(max_chars * 3)?;
        let s = std::str::from_utf8(bytes).map_err(|_| Error("bad UTF-8".into()))?;
        if s.chars().count() > max_chars {
            return err("string too long");
        }
        Ok(s.to_owned())
    }
}

/// A writer of Minecraft's buffer encodings.
#[derive(Default)]
pub struct Writer {
    pub data: Vec<u8>,
}

impl Writer {
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.data.push(v);
        self
    }
    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.u8(u8::from(v))
    }
    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.data.extend(v.to_be_bytes());
        self
    }
    pub fn i64(&mut self, v: i64) -> &mut Self {
        self.data.extend(v.to_be_bytes());
        self
    }
    pub fn f32(&mut self, v: f32) -> &mut Self {
        self.data.extend(v.to_be_bytes());
        self
    }
    pub fn f64(&mut self, v: f64) -> &mut Self {
        self.data.extend(v.to_be_bytes());
        self
    }
    pub fn uuid(&mut self, v: u128) -> &mut Self {
        self.data.extend(v.to_be_bytes());
        self
    }
    pub fn raw(&mut self, v: &[u8]) -> &mut Self {
        self.data.extend_from_slice(v);
        self
    }
    pub fn varint(&mut self, v: i32) -> &mut Self {
        let mut v = v as u32;
        loop {
            if v & !0x7F == 0 {
                self.data.push(v as u8);
                return self;
            }
            self.data.push((v as u8 & 0x7F) | 0x80);
            v >>= 7;
        }
    }
    pub fn bytes(&mut self, v: &[u8]) -> &mut Self {
        self.varint(v.len() as i32).raw(v)
    }
    pub fn utf(&mut self, v: &str) -> &mut Self {
        self.bytes(v.as_bytes())
    }
}

/// The Opus application the server asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Voip,
    Audio,
    LowDelay,
}

/// `voicechat:secret`: what the client needs to open the UDP connection.
#[derive(Clone, Debug, PartialEq)]
pub struct Secret {
    /// The protocol version the layout showed (20 or 21).
    pub version: i32,
    pub secret: [u8; 16],
    /// What UDP packets to the server carry: version 20 uses the player's uuid.
    pub connection_id: u128,
    pub port: i32,
    pub player: u128,
    pub codec: Codec,
    pub mtu: i32,
    pub distance: f64,
    pub keep_alive_ms: i32,
    pub groups: bool,
    pub voice_host: String,
    pub recording: bool,
}

impl Secret {
    /// Either version's layout (21 adds the connection id after the secret): whichever reads
    /// the payload exactly.
    pub fn read(data: &[u8]) -> Result<Self, Error> {
        let exact = |version| {
            let mut r = Reader::new(data);
            let s = Self::read_version(&mut r, version)?;
            if r.remaining() != 0 {
                return err("trailing bytes");
            }
            Ok(s)
        };
        exact(VERSION_RELEASE).or_else(|e| exact(VERSION_PRERELEASE).map_err(|_| e))
    }

    fn read_version(r: &mut Reader, version: i32) -> Result<Self, Error> {
        let secret = r.raw16()?;
        let connection_id = if version >= VERSION_PRERELEASE {
            Some(r.uuid()?)
        } else {
            None
        };
        let port = r.i32()?;
        let player = r.uuid()?;
        Ok(Self {
            version,
            secret,
            connection_id: connection_id.unwrap_or(player),
            port,
            player,
            codec: match r.u8()? {
                1 => Codec::Audio,
                2 => Codec::LowDelay,
                _ => Codec::Voip,
            },
            mtu: r.i32()?.clamp(512, 2048),
            distance: r.f64()?,
            keep_alive_ms: r.i32()?.max(100),
            groups: r.bool()?,
            voice_host: r.utf(32767)?,
            recording: r.bool()?,
        })
    }

    /// The UDP address: the voice host when the server names one (a host, `host:port` or a
    /// bare port), else the Minecraft server's host and the secret's port.
    pub fn address(&self, server_host: &str) -> (String, u16) {
        let port = u16::try_from(self.port).unwrap_or(24454);
        let host = self.voice_host.trim();
        if host.is_empty() {
            return (server_host.to_owned(), port);
        }
        if let Ok(p) = host.parse::<u16>() {
            return (server_host.to_owned(), p);
        }
        // `[v6]:port`, `host:port` or a bare host.
        if let Some(rest) = host.strip_prefix('[')
            && let Some((h, tail)) = rest.split_once(']')
        {
            let p = tail
                .strip_prefix(':')
                .and_then(|p| p.parse().ok())
                .unwrap_or(port);
            return (h.to_owned(), p);
        }
        match host.rsplit_once(':') {
            Some((h, p)) if !h.contains(':') => (h.to_owned(), p.parse().unwrap_or(port)),
            _ => (host.to_owned(), port),
        }
    }
}

/// `voicechat:request_secret`.
pub fn request_secret(version: i32) -> Vec<u8> {
    let mut w = Writer::default();
    w.i32(version);
    w.data
}

/// The mute and disable state for the server: version 21's `voicechat:client_state`, or version
/// 20's `voicechat:update_state` (which has no mute; muted just sends nothing).
pub fn client_state(version: i32, muted: bool, disabled: bool) -> (&'static str, Vec<u8>) {
    let mut w = Writer::default();
    if version >= VERSION_PRERELEASE {
        w.bool(muted).bool(disabled);
        (CLIENT_STATE, w.data)
    } else {
        w.bool(disabled);
        (UPDATE_STATE, w.data)
    }
}

/// `minecraft:register`: the channel ids, NUL-separated.
pub fn register() -> Vec<u8> {
    CHANNELS.join("\0").into_bytes()
}

/// A player's voice chat state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerState {
    pub player: u128,
    pub name: String,
    pub muted: bool,
    pub disabled: bool,
    pub disconnected: bool,
    pub group: Option<u128>,
}

impl PlayerState {
    fn read(r: &mut Reader, version: i32) -> Result<Self, Error> {
        if version < VERSION_PRERELEASE {
            // Version 20: bool disabled, bool disconnected, uuid, name, optional group.
            let disabled = r.bool()?;
            let disconnected = r.bool()?;
            let player = r.uuid()?;
            let name = r.utf(32767)?;
            let group = if r.bool()? { Some(r.uuid()?) } else { None };
            return Ok(Self {
                player,
                name,
                muted: false,
                disabled,
                disconnected,
                group,
            });
        }
        let flags = r.u8()?;
        let player = r.uuid()?;
        let name = r.utf(256)?;
        let group = if flags & 0x20 != 0 {
            Some(r.uuid()?)
        } else {
            None
        };
        Ok(Self {
            player,
            name,
            muted: flags & 0x03 != 0,
            disabled: flags & 0x0C != 0,
            disconnected: flags & 0x10 != 0,
            group,
        })
    }

    /// `voicechat:state`.
    pub fn read_one(data: &[u8], version: i32) -> Result<Self, Error> {
        Self::read(&mut Reader::new(data), version)
    }

    /// `voicechat:states`.
    pub fn read_all(data: &[u8], version: i32) -> Result<Vec<Self>, Error> {
        let mut r = Reader::new(data);
        let n = r.i32()?;
        if !(0..=100_000).contains(&n) {
            return err("bad state count");
        }
        (0..n).map(|_| Self::read(&mut r, version)).collect()
    }
}

/// A UDP packet.
#[derive(Clone, Debug, PartialEq)]
pub enum Packet {
    Mic {
        opus: Vec<u8>,
        sequence: i64,
        whispering: bool,
    },
    Sound(Sound),
    /// Version 20 carries the player and the secret; 21 is empty.
    Authenticate(Option<(u128, [u8; 16])>),
    /// Version 21's ack carries a nonce for the connection check; 20's is empty.
    AuthenticateAck(Option<[u8; 16]>),
    Ping {
        id: u128,
        timestamp: i64,
    },
    KeepAlive,
    ConnectionCheck(Option<[u8; 16]>),
    ConnectionCheckAck,
}

/// Where a received sound plays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    /// At the sender, fading out at `distance`.
    Player { distance: f32, whispering: bool },
    /// Straight into the ears (a group).
    Group,
    /// At a fixed point.
    Location { at: [f64; 3], distance: f32 },
}

/// Received voice.
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    pub channel: u128,
    pub sender: u128,
    pub opus: Vec<u8>,
    pub sequence: i64,
    pub placement: Placement,
    pub category: Option<String>,
}

impl Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        match self {
            Packet::Mic {
                opus,
                sequence,
                whispering,
            } => {
                w.u8(1).bytes(opus).i64(*sequence).bool(*whispering);
            }
            Packet::Sound(s) => {
                let flags =
                    |whisper: bool| u8::from(whisper) | if s.category.is_some() { 2 } else { 0 };
                match s.placement {
                    Placement::Player {
                        distance,
                        whispering,
                    } => {
                        w.u8(2)
                            .uuid(s.channel)
                            .uuid(s.sender)
                            .bytes(&s.opus)
                            .i64(s.sequence);
                        w.f32(distance).u8(flags(whispering));
                    }
                    Placement::Group => {
                        w.u8(3)
                            .uuid(s.channel)
                            .uuid(s.sender)
                            .bytes(&s.opus)
                            .i64(s.sequence);
                        w.u8(flags(false));
                    }
                    Placement::Location { at, distance } => {
                        w.u8(4).uuid(s.channel).uuid(s.sender);
                        w.f64(at[0]).f64(at[1]).f64(at[2]);
                        w.bytes(&s.opus)
                            .i64(s.sequence)
                            .f32(distance)
                            .u8(flags(false));
                    }
                }
                if let Some(c) = &s.category {
                    w.utf(c);
                }
            }
            Packet::Authenticate(legacy) => {
                w.u8(5);
                if let Some((player, secret)) = legacy {
                    w.uuid(*player).raw(secret);
                }
            }
            Packet::AuthenticateAck(n) => {
                w.u8(6);
                if let Some(n) = n {
                    w.raw(n);
                }
            }
            Packet::Ping { id, timestamp } => {
                w.u8(7).uuid(*id).i64(*timestamp);
            }
            Packet::KeepAlive => {
                w.u8(8);
            }
            Packet::ConnectionCheck(n) => {
                w.u8(9);
                if let Some(n) = n {
                    w.raw(n);
                }
            }
            Packet::ConnectionCheckAck => {
                w.u8(10);
            }
        }
        w.data
    }

    pub fn decode(data: &[u8]) -> Result<Self, Error> {
        let mut r = Reader::new(data);
        let category = |r: &mut Reader, flags: u8| -> Result<Option<String>, Error> {
            if flags & 2 != 0 {
                r.utf(16).map(Some)
            } else {
                Ok(None)
            }
        };
        Ok(match r.u8()? {
            1 => Packet::Mic {
                opus: r.bytes(MAX_OPUS)?.to_vec(),
                sequence: r.i64()?,
                whispering: r.bool()?,
            },
            2 => {
                let (channel, sender) = (r.uuid()?, r.uuid()?);
                let opus = r.bytes(MAX_OPUS)?.to_vec();
                let sequence = r.i64()?;
                let distance = r.f32()?;
                let flags = r.u8()?;
                Packet::Sound(Sound {
                    channel,
                    sender,
                    opus,
                    sequence,
                    placement: Placement::Player {
                        distance,
                        whispering: flags & 1 != 0,
                    },
                    category: category(&mut r, flags)?,
                })
            }
            3 => {
                let (channel, sender) = (r.uuid()?, r.uuid()?);
                let opus = r.bytes(MAX_OPUS)?.to_vec();
                let sequence = r.i64()?;
                let flags = r.u8()?;
                Packet::Sound(Sound {
                    channel,
                    sender,
                    opus,
                    sequence,
                    placement: Placement::Group,
                    category: category(&mut r, flags)?,
                })
            }
            4 => {
                let (channel, sender) = (r.uuid()?, r.uuid()?);
                let at = [r.f64()?, r.f64()?, r.f64()?];
                let opus = r.bytes(MAX_OPUS)?.to_vec();
                let sequence = r.i64()?;
                let distance = r.f32()?;
                let flags = r.u8()?;
                Packet::Sound(Sound {
                    channel,
                    sender,
                    opus,
                    sequence,
                    placement: Placement::Location { at, distance },
                    category: category(&mut r, flags)?,
                })
            }
            5 if r.remaining() == 0 => Packet::Authenticate(None),
            5 => Packet::Authenticate(Some((r.uuid()?, r.raw16()?))),
            6 if r.remaining() == 0 => Packet::AuthenticateAck(None),
            6 => Packet::AuthenticateAck(Some(r.raw16()?)),
            7 => Packet::Ping {
                id: r.uuid()?,
                timestamp: r.i64()?,
            },
            8 => Packet::KeepAlive,
            9 if r.remaining() == 0 => Packet::ConnectionCheck(None),
            9 => Packet::ConnectionCheck(Some(r.raw16()?)),
            10 => Packet::ConnectionCheckAck,
            other => return err(&format!("unknown packet type {other}")),
        })
    }
}

/// IV, then AES-128-GCM ciphertext and tag.
pub fn encrypt(key: &[u8; 16], plain: &[u8]) -> Result<Vec<u8>, Error> {
    let mut iv = [0u8; IV];
    getrandom::getrandom(&mut iv).map_err(|e| Error(e.to_string()))?;
    let cipher = Aes128Gcm::new_from_slice(key).map_err(|e| Error(e.to_string()))?;
    let sealed = cipher
        .encrypt(Nonce::from_slice(&iv), plain)
        .map_err(|_| Error("encryption failed".into()))?;
    let mut out = iv.to_vec();
    out.extend(sealed);
    Ok(out)
}

pub fn decrypt(key: &[u8; 16], data: &[u8]) -> Result<Vec<u8>, Error> {
    if data.len() < IV + 16 {
        return err("encrypted payload too short");
    }
    let cipher = Aes128Gcm::new_from_slice(key).map_err(|e| Error(e.to_string()))?;
    cipher
        .decrypt(Nonce::from_slice(&data[..IV]), &data[IV..])
        .map_err(|_| Error("decryption failed".into()))
}

/// A datagram to the server.
pub fn to_server(key: &[u8; 16], connection_id: u128, packet: &Packet) -> Result<Vec<u8>, Error> {
    let mut w = Writer::default();
    w.u8(MAGIC)
        .uuid(connection_id)
        .bytes(&encrypt(key, &packet.encode())?);
    Ok(w.data)
}

/// A datagram from the server.
pub fn from_server(key: &[u8; 16], datagram: &[u8]) -> Result<Packet, Error> {
    let mut r = Reader::new(datagram);
    if r.u8()? != MAGIC {
        return err("not a voice chat packet");
    }
    let sealed = r.bytes(MAX_DATAGRAM)?;
    Packet::decode(&decrypt(key, sealed)?)
}

/// For tests and a local stand-in server: a datagram to a client, and one read from a client.
pub fn to_client(key: &[u8; 16], packet: &Packet) -> Result<Vec<u8>, Error> {
    let mut w = Writer::default();
    w.u8(MAGIC).bytes(&encrypt(key, &packet.encode())?);
    Ok(w.data)
}
pub fn from_client(key: &[u8; 16], datagram: &[u8]) -> Result<(u128, Packet), Error> {
    let mut r = Reader::new(datagram);
    if r.u8()? != MAGIC {
        return err("not a voice chat packet");
    }
    let id = r.uuid()?;
    let sealed = r.bytes(MAX_DATAGRAM)?;
    Ok((id, Packet::decode(&decrypt(key, sealed)?)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_round_trip() {
        for v in [
            0,
            1,
            127,
            128,
            255,
            25565,
            2_097_151,
            i32::MAX,
            -1,
            i32::MIN,
        ] {
            let mut w = Writer::default();
            w.varint(v);
            assert_eq!(Reader::new(&w.data).varint().unwrap(), v);
        }
        let mut w = Writer::default();
        w.varint(300);
        assert_eq!(w.data, [0xAC, 0x02]);
    }

    #[test]
    fn the_secret_reads_in_order() {
        let mut w = Writer::default();
        w.raw(&[7; 16])
            .uuid(1)
            .i32(24454)
            .uuid(2)
            .u8(0)
            .i32(1275)
            .f64(48.0)
            .i32(1000);
        w.bool(true).utf("").bool(true);
        let s = Secret::read(&w.data).unwrap();
        assert_eq!(s.version, 21);
        assert_eq!(s.secret, [7; 16]);
        assert_eq!((s.connection_id, s.port, s.player), (1, 24454, 2));
        assert_eq!(
            (s.codec, s.mtu, s.distance, s.keep_alive_ms),
            (Codec::Voip, 1275, 48.0, 1000)
        );
        assert_eq!(
            s.address("play.example.net"),
            ("play.example.net".into(), 24454)
        );
    }

    #[test]
    fn a_released_servers_secret_has_no_connection_id() {
        let mut w = Writer::default();
        w.raw(&[7; 16])
            .i32(24454)
            .uuid(2)
            .u8(0)
            .i32(1024)
            .f64(48.0)
            .i32(1000);
        w.bool(true).utf("").bool(true);
        let s = Secret::read(&w.data).unwrap();
        assert_eq!(s.version, 20);
        // Version 20's UDP packets carry the player's uuid.
        assert_eq!((s.connection_id, s.port, s.player), (2, 24454, 2));
        assert_eq!((s.mtu, s.distance, s.keep_alive_ms), (1024, 48.0, 1000));
    }

    #[test]
    fn released_states_read_their_booleans() {
        let mut w = Writer::default();
        w.i32(2);
        w.bool(false).bool(false).uuid(10).utf("Alex").bool(false);
        w.bool(true)
            .bool(true)
            .uuid(11)
            .utf("Steve")
            .bool(true)
            .uuid(99);
        let states = PlayerState::read_all(&w.data, 20).unwrap();
        assert!(!states[0].disabled && states[0].group.is_none() && states[0].name == "Alex");
        assert!(states[1].disabled && states[1].disconnected && states[1].group == Some(99));
    }

    #[test]
    fn voice_hosts_override_the_address() {
        let mut s = Secret::read(&{
            let mut w = Writer::default();
            w.raw(&[0; 16])
                .uuid(1)
                .i32(24454)
                .uuid(2)
                .u8(0)
                .i32(1275)
                .f64(48.0)
                .i32(1000);
            w.bool(true).utf("").bool(true);
            w.data
        })
        .unwrap();
        for (host, expect) in [
            ("25000", ("mc.net", 25000)),
            ("voice.mc.net", ("voice.mc.net", 24454)),
            ("voice.mc.net:1234", ("voice.mc.net", 1234)),
            ("[::1]:99", ("::1", 99)),
        ] {
            s.voice_host = host.into();
            assert_eq!(s.address("mc.net"), (expect.0.into(), expect.1), "{host}");
        }
    }

    #[test]
    fn packets_round_trip_through_the_cipher() {
        let key = [3u8; 16];
        let mic = Packet::Mic {
            opus: vec![1, 2, 3],
            sequence: 42,
            whispering: true,
        };
        let datagram = to_server(&key, 99, &mic).unwrap();
        assert_eq!(datagram[0], 0xFF);
        assert_eq!(from_client(&key, &datagram).unwrap(), (99, mic));
        for p in [
            Packet::Sound(Sound {
                channel: 5,
                sender: 5,
                opus: vec![9; 40],
                sequence: 7,
                placement: Placement::Player {
                    distance: 48.0,
                    whispering: false,
                },
                category: None,
            }),
            Packet::Sound(Sound {
                channel: 6,
                sender: 8,
                opus: vec![],
                sequence: 1,
                placement: Placement::Location {
                    at: [1.0, 2.0, 3.0],
                    distance: 10.0,
                },
                category: Some("music".into()),
            }),
            Packet::AuthenticateAck(Some([4; 16])),
            Packet::AuthenticateAck(None),
            Packet::Authenticate(Some((12, [6; 16]))),
            Packet::ConnectionCheck(None),
            Packet::Ping {
                id: 77,
                timestamp: 123,
            },
            Packet::KeepAlive,
            Packet::ConnectionCheckAck,
        ] {
            assert_eq!(from_server(&key, &to_client(&key, &p).unwrap()).unwrap(), p);
        }
        // Another key fails the tag.
        assert!(from_server(&[4; 16], &to_client(&key, &Packet::KeepAlive).unwrap()).is_err());
    }

    #[test]
    fn malformed_input_is_an_error_not_a_panic() {
        let key = [3u8; 16];
        let good = Packet::Sound(Sound {
            channel: 1,
            sender: 2,
            opus: vec![7; 60],
            sequence: 3,
            placement: Placement::Location {
                at: [1.0, 2.0, 3.0],
                distance: 8.0,
            },
            category: Some("x".into()),
        })
        .encode();
        // Every truncation of a valid packet, and of the secret, fails cleanly.
        for n in 0..good.len() {
            assert!(Packet::decode(&good[..n]).is_err(), "{n}");
        }
        // Lengths past the limits, negative varints, unknown ids.
        assert!(Packet::decode(&[1, 0xFF, 0xFF, 0xFF, 0xFF, 0x07]).is_err());
        assert!(Packet::decode(&[1, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F]).is_err());
        assert!(Packet::decode(&[1, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01]).is_err());
        assert!(Packet::decode(&[0]).is_err() && Packet::decode(&[11]).is_err());
        assert!(Secret::read(&[0; 20]).is_err());
        assert!(PlayerState::read_all(&(-1i32).to_be_bytes(), 21).is_err());
        // Garbage datagrams: a pseudo-random sweep of lengths and bytes.
        let mut x = 0x2545_F491_4F6C_DD1Du64;
        for len in 0..300 {
            let bytes: Vec<u8> = (0..len)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    x as u8
                })
                .collect();
            let _ = Packet::decode(&bytes);
            let _ = from_server(&key, &bytes);
            let _ = Secret::read(&bytes);
            let _ = PlayerState::read_all(&bytes, 20);
            let _ = PlayerState::read_all(&bytes, 21);
        }
    }

    #[test]
    fn states_read_their_flags() {
        let mut w = Writer::default();
        w.i32(2);
        w.u8(0x01).uuid(10).utf("Alex");
        w.u8(0x24).uuid(11).utf("Steve").uuid(99);
        let states = PlayerState::read_all(&w.data, 21).unwrap();
        assert!(states[0].muted && !states[0].disabled && states[0].group.is_none());
        assert!(states[1].disabled && states[1].group == Some(99) && states[1].name == "Steve");
    }
}
