//! The UDP connection to a voice chat server: authenticate, check the connection, answer
//! keep-alives and pings, and hand received voice to the speaker.
use crate::protocol::{self, Packet, Secret, Sound};
use std::net::{ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// How far the connection has got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Connecting,
    Connected,
    /// No keep-alive for too long, or the socket failed.
    Lost,
}

struct Shared {
    key: [u8; 16],
    id: u128,
    socket: OnceLock<UdpSocket>,
    stop: AtomicBool,
    /// The authenticate ack's nonce, once it came.
    nonce: Mutex<Option<[u8; 16]>>,
    connected: AtomicBool,
    failed: AtomicBool,
    last_keep_alive: Mutex<Instant>,
}

impl Shared {
    fn send(&self, packet: &Packet) {
        match protocol::to_server(&self.key, self.id, packet) {
            Ok(datagram) => {
                let Some(socket) = self.socket.get() else {
                    return;
                };
                if let Err(e) = socket.send(&datagram)
                    && e.kind() != std::io::ErrorKind::WouldBlock
                {
                    log::debug!("Voice chat: send failed: {e}");
                }
            }
            Err(e) => log::warn!("Voice chat: {e}"),
        }
    }
}

pub struct Connection {
    shared: Arc<Shared>,
    keep_alive: Duration,
}

impl Connection {
    /// Starts connecting, on a thread of its own, to the server the secret names: `host` is the
    /// Minecraft server's host as the player typed it, which `resolve` follows through an SRV
    /// redirect when the voice server shares it. Voice the server sends goes to `sounds`.
    pub fn open(
        secret: &Secret,
        host: &str,
        resolve: Resolve,
        sounds: Sender<Sound>,
    ) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            key: secret.secret,
            id: secret.connection_id,
            socket: OnceLock::new(),
            stop: AtomicBool::new(false),
            nonce: Mutex::new(None),
            connected: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            last_keep_alive: Mutex::new(Instant::now()),
        });
        let (voice_host, port) = secret.address(host);
        // The voice server on the Minecraft server's own host: that host after its SRV redirect.
        let same_host = voice_host == host;
        let worker = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("voicechat-udp".into())
            .spawn(move || {
                let target = if same_host {
                    resolve(&voice_host)
                } else {
                    voice_host.clone()
                };
                match bind(&target, port) {
                    Ok(socket) => {
                        let _ = worker.socket.set(socket);
                    }
                    Err(e) => {
                        log::warn!("Wisp voice chat: can't reach {target}:{port}: {e}");
                        worker.failed.store(true, Ordering::Relaxed);
                        return;
                    }
                }
                let auth = Arc::clone(&worker);
                let _ = std::thread::Builder::new()
                    .name("voicechat-auth".into())
                    .spawn(move || handshake(&auth));
                receive(&worker, &sounds);
            })?;
        Ok(Self {
            shared,
            keep_alive: Duration::from_millis(secret.keep_alive_ms.max(100) as u64),
        })
    }

    pub fn status(&self) -> Status {
        let s = &self.shared;
        if s.failed.load(Ordering::Relaxed) {
            return Status::Lost;
        }
        if !s.connected.load(Ordering::Relaxed) {
            return Status::Connecting;
        }
        let quiet = s
            .last_keep_alive
            .lock()
            .map(|t| t.elapsed())
            .unwrap_or_default();
        if quiet > self.keep_alive * 10 {
            Status::Lost
        } else {
            Status::Connected
        }
    }

    /// A sender for the microphone thread.
    pub fn mic(&self) -> MicSender {
        MicSender(Arc::clone(&self.shared))
    }
}

/// Follows a Minecraft host through its SRV redirect (the game's resolver).
pub type Resolve = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// A socket connected to `host:port`.
fn bind(host: &str, port: u16) -> std::io::Result<UdpSocket> {
    let address = (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::other(format!("no address for {host}")))?;
    let local = if address.is_ipv6() {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    };
    let socket = UdpSocket::bind(local)?;
    socket.connect(address)?;
    socket.set_read_timeout(Some(Duration::from_millis(200)))?;
    log::info!("Wisp voice chat: connecting to {address}");
    Ok(socket)
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

/// Sends the microphone's frames once the connection is up.
#[derive(Clone)]
pub struct MicSender(Arc<Shared>);

impl MicSender {
    pub fn send(&self, opus: Vec<u8>, sequence: i64, whispering: bool) {
        if self.0.connected.load(Ordering::Relaxed) && !self.0.stop.load(Ordering::Relaxed) {
            self.0.send(&Packet::Mic {
                opus,
                sequence,
                whispering,
            });
        }
    }
    pub fn closed(&self) -> bool {
        self.0.stop.load(Ordering::Relaxed)
    }
}

/// Authenticate, then check the connection with the ack's nonce, once a second until it's up.
fn handshake(s: &Shared) {
    while !s.stop.load(Ordering::Relaxed) && !s.connected.load(Ordering::Relaxed) {
        let nonce = *s.nonce.lock().expect("nonce lock");
        match nonce {
            None => s.send(&Packet::Authenticate),
            Some(n) => s.send(&Packet::ConnectionCheck(n)),
        }
        // A second, in steps, so a closed connection stops at once.
        for _ in 0..10 {
            if s.stop.load(Ordering::Relaxed) || s.connected.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

fn receive(s: &Shared, sounds: &Sender<Sound>) {
    let Some(socket) = s.socket.get() else {
        return;
    };
    let mut buf = vec![0u8; 8192];
    while !s.stop.load(Ordering::Relaxed) {
        let n = match socket.recv(&mut buf) {
            Ok(n) => n,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            // Windows reports an unreachable port on the next receive: keep trying, the
            // handshake goes on resending.
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => continue,
            Err(e) => {
                log::warn!("Wisp voice chat: receive failed: {e}");
                s.failed.store(true, Ordering::Relaxed);
                return;
            }
        };
        let packet = match protocol::from_server(&s.key, &buf[..n]) {
            Ok(p) => p,
            Err(e) => {
                log::debug!("Voice chat: dropped a packet: {e}");
                continue;
            }
        };
        match packet {
            Packet::AuthenticateAck(nonce) => {
                let mut held = s.nonce.lock().expect("nonce lock");
                if held.is_none() {
                    *held = Some(nonce);
                }
            }
            Packet::ConnectionCheckAck => {
                if !s.connected.swap(true, Ordering::Relaxed) {
                    log::info!("Wisp voice chat: connected");
                }
                *s.last_keep_alive.lock().expect("keep-alive lock") = Instant::now();
            }
            Packet::KeepAlive => {
                *s.last_keep_alive.lock().expect("keep-alive lock") = Instant::now();
                s.send(&Packet::KeepAlive);
            }
            ping @ Packet::Ping { .. } => s.send(&ping),
            Packet::Sound(sound) => {
                if sounds.send(sound).is_err() {
                    return;
                }
            }
            Packet::Mic { .. } | Packet::Authenticate | Packet::ConnectionCheck(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Codec, Placement};

    /// A stand-in server on localhost goes through the whole handshake, keeps the connection
    /// alive, gets a ping answered and sends voice that arrives.
    #[test]
    fn handshake_keep_alive_ping_and_voice_against_a_local_server() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let key = [9u8; 16];
        let secret = Secret {
            secret: key,
            connection_id: 1234,
            port: i32::from(server.local_addr().unwrap().port()),
            player: 1,
            codec: Codec::Voip,
            mtu: 1275,
            distance: 48.0,
            keep_alive_ms: 1000,
            groups: true,
            voice_host: String::new(),
            recording: true,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let local: Resolve = Arc::new(|_: &str| "127.0.0.1".into());
        let conn = Connection::open(&secret, "localhost", local, tx).unwrap();
        let mut buf = [0u8; 4096];
        let mut recv = || {
            let (n, from) = server.recv_from(&mut buf).unwrap();
            let (id, p) = protocol::from_client(&key, &buf[..n]).unwrap();
            assert_eq!(id, 1234);
            (p, from)
        };
        let (p, client) = recv();
        assert_eq!(p, Packet::Authenticate);
        let send = |p: &Packet| {
            server
                .send_to(&protocol::to_client(&key, p).unwrap(), client)
                .unwrap();
        };
        send(&Packet::AuthenticateAck([5; 16]));
        loop {
            match recv().0 {
                Packet::ConnectionCheck(n) => {
                    assert_eq!(n, [5; 16]);
                    break;
                }
                Packet::Authenticate => {}
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(conn.status(), Status::Connecting);
        send(&Packet::ConnectionCheckAck);
        send(&Packet::KeepAlive);
        assert_eq!(recv().0, Packet::KeepAlive);
        assert_eq!(conn.status(), Status::Connected);
        let ping = Packet::Ping {
            id: 3,
            timestamp: 4,
        };
        send(&ping);
        assert_eq!(recv().0, ping);
        conn.mic().send(vec![1, 2], 0, false);
        assert!(matches!(recv().0, Packet::Mic { sequence: 0, .. }));
        let sound = Sound {
            channel: 7,
            sender: 7,
            opus: vec![1],
            sequence: 0,
            placement: Placement::Group,
            category: None,
        };
        send(&Packet::Sound(sound.clone()));
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), sound);
    }
}
