//! The multiplayer list's status ping (Minecraft's Server List Ping): whether a server answers,
//! its players, message of the day, version and round trip. Plain TCP, no login, read-only.
//! SRV records are not looked up: `host[:port]` with port 25565 by default.
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// The featured server (the owner's choice), shown pinned on Home and Servers; the game keeps it
/// on top of its own list and doesn't let it be edited or deleted.
pub const FEATURED_NAME: &str = "Zedix.de";
pub const FEATURED_ADDRESS: &str = "zedix.de";

/// Whether `address` is the featured server (any case, a trailing dot or the default port).
pub fn is_featured(address: &str) -> bool {
    let address = address.trim().to_ascii_lowercase();
    let host = address.strip_suffix(":25565").unwrap_or(&address);
    host.trim_end_matches('.') == FEATURED_ADDRESS
}

/// The game's network protocol, sent in the handshake (servers answer the status either way).
const PROTOCOL: i32 = 777;
/// The largest status reply read (a favicon makes it tens of kilobytes).
const MAX_REPLY: usize = 1 << 20;

#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    pub online: u32,
    pub max: u32,
    pub motd: String,
    pub version: String,
    pub latency_ms: u32,
}

fn write_varint(out: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        if v & !0x7f == 0 {
            out.push(v as u8);
            return;
        }
        out.push((v as u8 & 0x7f) | 0x80);
        v >>= 7;
    }
}

fn read_varint(input: &mut impl Read) -> Result<i32, String> {
    let mut value = 0u32;
    for i in 0..5 {
        let mut byte = [0u8];
        input.read_exact(&mut byte).map_err(|e| e.to_string())?;
        value |= u32::from(byte[0] & 0x7f) << (7 * i);
        if byte[0] & 0x80 == 0 {
            return Ok(value as i32);
        }
    }
    Err("VarInt too long".into())
}

fn packet(id: i32, body: &[u8]) -> Vec<u8> {
    let mut inner = Vec::new();
    write_varint(&mut inner, id);
    inner.extend_from_slice(body);
    let mut out = Vec::new();
    write_varint(&mut out, inner.len() as i32);
    out.extend(inner);
    out
}

/// `host[:port]`, `[v6]:port` or a bare v6 address, with the default port.
pub fn split_address(address: &str) -> (String, u16) {
    let address = address.trim();
    if let Some(rest) = address.strip_prefix('[') {
        if let Some((host, tail)) = rest.split_once(']') {
            let port = tail
                .strip_prefix(':')
                .and_then(|p| p.parse().ok())
                .unwrap_or(25565);
            return (host.to_owned(), port);
        }
    }
    match address.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => {
            (host.to_owned(), port.parse().unwrap_or(25565))
        }
        _ => (address.to_owned(), 25565),
    }
}

/// The status JSON's useful fields.
pub fn parse_status(json: &str, latency_ms: u32) -> Result<Status, String> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let players = value.get("players");
    let count = |key: &str| {
        players
            .and_then(|p| p.get(key))
            .and_then(|n| n.as_u64())
            .unwrap_or(0) as u32
    };
    Ok(Status {
        online: count("online"),
        max: count("max"),
        motd: value
            .get("description")
            .map(crate::local_content::plain_text)
            .unwrap_or_default(),
        version: value
            .get("version")
            .and_then(|v| v.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| crate::local_content::plain_text(&serde_json::Value::String(s.into())))
            .unwrap_or_default(),
        latency_ms,
    })
}

/// Asks the server at `address` for its status, giving up after `timeout` per step.
pub fn ping(address: &str, timeout: Duration) -> Result<Status, String> {
    let (host, port) = split_address(address);
    let target = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or("no address")?;
    let mut stream = TcpStream::connect_timeout(&target, timeout).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    let mut handshake = Vec::new();
    write_varint(&mut handshake, PROTOCOL);
    write_varint(&mut handshake, host.len() as i32);
    handshake.extend_from_slice(host.as_bytes());
    handshake.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut handshake, 1);
    let started = Instant::now();
    stream
        .write_all(&[packet(0, &handshake), packet(0, &[])].concat())
        .map_err(|e| e.to_string())?;
    let length = read_varint(&mut stream)?;
    let latency = started.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
    let length = usize::try_from(length).map_err(|_| "bad length")?;
    if length == 0 || length > MAX_REPLY {
        return Err("bad reply length".into());
    }
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body).map_err(|e| e.to_string())?;
    let mut cursor = &body[..];
    if read_varint(&mut cursor)? != 0 {
        return Err("unexpected packet".into());
    }
    let size = usize::try_from(read_varint(&mut cursor)?).map_err(|_| "bad string")?;
    let json = cursor.get(..size).ok_or("short reply")?;
    parse_status(&String::from_utf8_lossy(json), latency)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_featured_address_matches_its_spellings_only() {
        for a in ["zedix.de", "Zedix.de", "zedix.de:25565", " zedix.de. "] {
            assert!(is_featured(a), "{a}");
        }
        for a in ["zedix.de:25566", "play.zedix.de", "zedix.de.evil.com", ""] {
            assert!(!is_featured(a), "{a}");
        }
    }

    #[test]
    fn varints_round_trip() {
        for value in [0, 1, 127, 128, 255, 25565, 777, 2_097_151, i32::MAX, -1] {
            let mut out = Vec::new();
            write_varint(&mut out, value);
            assert_eq!(read_varint(&mut &out[..]).unwrap(), value);
        }
    }

    #[test]
    fn addresses_split_into_host_and_port() {
        assert_eq!(
            split_address("play.example.net"),
            ("play.example.net".into(), 25565)
        );
        assert_eq!(split_address("1.2.3.4:25570"), ("1.2.3.4".into(), 25570));
        assert_eq!(split_address("[::1]:25566"), ("::1".into(), 25566));
        assert_eq!(split_address("::1"), ("::1".into(), 25565));
    }

    #[test]
    fn status_json_gives_players_motd_and_version() {
        let status = parse_status(
            r#"{"version":{"name":"Paper 1.21","protocol":767},
                "players":{"max":100,"online":42},
                "description":{"text":"§aWelcome ","extra":[{"text":"home"}]}}"#,
            31,
        )
        .unwrap();
        assert_eq!(
            status,
            Status {
                online: 42,
                max: 100,
                motd: "Welcome home".into(),
                version: "Paper 1.21".into(),
                latency_ms: 31
            }
        );
        assert!(parse_status("not json", 0).is_err());
    }

    #[test]
    fn a_closed_port_is_an_error_not_a_hang() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        assert!(ping(&format!("127.0.0.1:{port}"), Duration::from_millis(500)).is_err());
    }
}
