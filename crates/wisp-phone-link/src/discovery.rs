//! Finding the PC: the phone broadcasts a question, launchers offering files answer.
//!
//! Question: `WISP-PHONE-LINK/1 ?`. Answer: `WISP-PHONE-LINK/1 <tcp port> <name>`.
use crate::{DISCOVERY_PORT, PROTOCOL};
use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// A launcher that answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// Its transfer address.
    pub addr: SocketAddr,
    pub name: String,
}

fn question() -> String {
    format!("{PROTOCOL} ?")
}

/// An answer datagram from `from`: its transfer address and name.
fn parse_answer(text: &str, from: SocketAddr) -> Option<Found> {
    let rest = text.strip_prefix(PROTOCOL)?.strip_prefix(' ')?;
    let (port, name) = rest.split_once(' ').unwrap_or((rest, ""));
    let port: u16 = port.parse().ok()?;
    Some(Found {
        addr: SocketAddr::new(from.ip(), port),
        name: name.chars().take(64).collect(),
    })
}

/// Asks the local network for launchers for `wait`, sending the question a few times (a phone
/// that just woke may drop the first). Every launcher once, in the order they answered.
pub fn discover(wait: Duration) -> io::Result<Vec<Found>> {
    discover_on(
        SocketAddr::from((Ipv4Addr::BROADCAST, DISCOVERY_PORT)),
        wait,
    )
}

fn discover_on(target: SocketAddr, wait: Duration) -> io::Result<Vec<Found>> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(Duration::from_millis(150)))?;
    let started = Instant::now();
    let mut asked = 0;
    let mut found: Vec<Found> = Vec::new();
    let mut buffer = [0u8; 512];
    while started.elapsed() < wait {
        if asked < 4 && started.elapsed() >= Duration::from_millis(400) * asked {
            // A network without broadcast (some guest Wi-Fi) fails the send: keep listening.
            let _ = socket.send_to(question().as_bytes(), target);
            asked += 1;
        }
        match socket.recv_from(&mut buffer) {
            Ok((n, from)) => {
                if let Some(answer) = std::str::from_utf8(&buffer[..n])
                    .ok()
                    .and_then(|t| parse_answer(t, from))
                    && !found.iter().any(|f| f.addr == answer.addr)
                {
                    found.push(answer);
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            // Windows reports an ICMP "port unreachable" from an earlier send as a reset.
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => {}
            Err(e) => return Err(e),
        }
    }
    Ok(found)
}

/// The launcher's answerer: replies to every question while it lives.
pub struct Responder {
    stop: Arc<AtomicBool>,
}

impl Responder {
    /// Answers on [`DISCOVERY_PORT`] with this transfer port and name.
    pub fn start(port: u16, name: &str) -> io::Result<Self> {
        Self::start_on(DISCOVERY_PORT, port, name)
    }

    fn start_on(listen: u16, port: u16, name: &str) -> io::Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, listen))?;
        socket.set_read_timeout(Some(Duration::from_millis(200)))?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let answer = format!("{PROTOCOL} {port} {name}");
        std::thread::Builder::new()
            .name("phone-link-discovery".into())
            .spawn(move || {
                let mut buffer = [0u8; 512];
                while !thread_stop.load(Ordering::Relaxed) {
                    if let Ok((n, from)) = socket.recv_from(&mut buffer)
                        && buffer[..n] == *question().as_bytes()
                    {
                        let _ = socket.send_to(answer.as_bytes(), from);
                    }
                }
            })?;
        Ok(Self { stop })
    }
}

impl Drop for Responder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_are_read() {
        let from: SocketAddr = "192.168.1.20:40000".parse().unwrap();
        assert_eq!(
            parse_answer("WISP-PHONE-LINK/1 25592 Gaming PC", from),
            Some(Found {
                addr: "192.168.1.20:25592".parse().unwrap(),
                name: "Gaming PC".into()
            })
        );
        assert_eq!(parse_answer("WISP-PHONE-LINK/2 25592 x", from), None);
        assert_eq!(parse_answer("WISP-PHONE-LINK/1 nope x", from), None);
    }

    #[test]
    fn a_responder_is_found() {
        // A free UDP port for the responder (bound, read, released).
        let listen = UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let _responder = Responder::start_on(listen, 25599, "Test PC").unwrap();
        let found = discover_on(
            SocketAddr::from((Ipv4Addr::LOCALHOST, listen)),
            Duration::from_millis(800),
        )
        .unwrap();
        assert_eq!(
            found,
            [Found {
                addr: "127.0.0.1:25599".parse().unwrap(),
                name: "Test PC".into()
            }]
        );
    }
}
