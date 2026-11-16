//! The launcher's side: offers a folder on the local network while it runs.
use crate::{MAX_WRONG_CODES, TRANSFER_PORT, bundle, same_code};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What the launcher offers: the prepared files of one Minecraft version.
#[derive(Clone, Debug)]
pub struct Offer {
    /// The folder of prepared files (`resources/<version>`).
    pub root: PathBuf,
    /// The Minecraft version they were prepared for.
    pub version: String,
    /// The name the phone shows for this PC.
    pub name: String,
    /// The code the player types on the phone.
    pub code: String,
    /// The Wisp server (Store, cosmetics, friends) set in the launcher, if any: the phone uses
    /// the same one.
    pub wisp_server: Option<String>,
}

#[derive(Default)]
struct State {
    stop: AtomicBool,
    /// Too many wrong codes: still answering, but every download is refused.
    locked: AtomicBool,
    wrong: AtomicU32,
    /// Bytes sent in the current or last transfer, and its total.
    sent: AtomicU64,
    total: AtomicU64,
    /// Transfers finished whole.
    done: AtomicU32,
    /// A phone is downloading right now.
    sending: AtomicBool,
    last_error: Mutex<Option<String>>,
}

/// A running offer. Dropping it stops it.
pub struct Server {
    port: u16,
    state: Arc<State>,
}

impl Server {
    /// Listens on [`TRANSFER_PORT`] (or the next free port within 20) on every interface.
    pub fn start(offer: Offer) -> io::Result<Self> {
        let listener = (TRANSFER_PORT..TRANSFER_PORT + 20)
            .find_map(|port| TcpListener::bind(("0.0.0.0", port)).ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::AddrInUse, "no free port"))?;
        Self::with_listener(listener, offer)
    }

    /// Serves on a listener already bound (tests bind to 127.0.0.1:0).
    pub fn with_listener(listener: TcpListener, offer: Offer) -> io::Result<Self> {
        let port = listener.local_addr()?.port();
        // Polled, so a stop is seen within 200 ms without a connection to wake the accept.
        listener.set_nonblocking(true)?;
        let state = Arc::new(State::default());
        let offer = Arc::new(offer);
        let thread_state = Arc::clone(&state);
        std::thread::Builder::new()
            .name("phone-link".into())
            .spawn(move || {
                while !thread_state.stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let state = Arc::clone(&thread_state);
                            let offer = Arc::clone(&offer);
                            let _ = std::thread::Builder::new()
                                .name("phone-link-client".into())
                                .spawn(move || {
                                    if let Err(e) = serve(stream, &offer, &state) {
                                        *state.last_error.lock().unwrap() = Some(e.to_string());
                                    }
                                });
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(200));
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(200)),
                    }
                }
            })?;
        Ok(Self { port, state })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Closed after too many wrong codes (or [`Self::stop`]).
    pub fn closed(&self) -> bool {
        self.state.stop.load(Ordering::Relaxed) || self.state.locked.load(Ordering::Relaxed)
    }

    /// Wrong codes so far.
    pub fn wrong_codes(&self) -> u32 {
        self.state.wrong.load(Ordering::Relaxed)
    }

    /// `(sent, total)` of the transfer running now, if one is.
    pub fn sending(&self) -> Option<(u64, u64)> {
        self.state.sending.load(Ordering::Relaxed).then(|| {
            (
                self.state.sent.load(Ordering::Relaxed),
                self.state.total.load(Ordering::Relaxed),
            )
        })
    }

    /// Transfers that reached their end.
    pub fn finished(&self) -> u32 {
        self.state.done.load(Ordering::Relaxed)
    }

    /// The last connection's error, if it failed.
    pub fn last_error(&self) -> Option<String> {
        self.state.last_error.lock().unwrap().clone()
    }

    pub fn stop(&self) {
        self.state.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The request line and headers, up to 8 KiB.
fn read_request(reader: &mut impl BufRead) -> io::Result<(String, Vec<(String, String)>)> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let target = line.split_whitespace().nth(1).unwrap_or("").to_owned();
    let mut headers = Vec::new();
    let mut read = line.len();
    loop {
        let mut header = String::new();
        let n = reader.read_line(&mut header)?;
        read += n;
        if n == 0 || header == "\r\n" || header == "\n" {
            break;
        }
        if read > 8192 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request too long",
            ));
        }
        if let Some((key, value)) = header.split_once(':') {
            headers.push((key.trim().to_ascii_lowercase(), value.trim().to_owned()));
        }
    }
    Ok((target, headers))
}

fn respond(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()
}

/// Escapes a string for a JSON value.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The files the phone gets: everything but the preparation's logs.
fn files_to_send(offer: &Offer) -> io::Result<Vec<(String, u64)>> {
    let mut files = bundle::list(&offer.root)?;
    files.retain(|(path, _)| !path.ends_with(".log"));
    Ok(files)
}

fn serve(mut stream: TcpStream, offer: &Offer, state: &State) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    let (target, headers) = read_request(&mut BufReader::new(&mut stream))?;
    match target.as_str() {
        "/v1/hello" => {
            // The layout the files were prepared in, so a phone of another Wisp version can
            // say so before it downloads them.
            let schema = std::fs::read(offer.root.join("wisp-resources.json"))
                .ok()
                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                .and_then(|v| v.get("schema")?.as_i64())
                .unwrap_or(0);
            let wisp_server = offer
                .wisp_server
                .as_deref()
                .map_or_else(|| "null".to_owned(), json_string);
            let body = format!(
                "{{\"protocol\":{},\"version\":{},\"name\":{},\"schema\":{schema},\"wisp_server\":{wisp_server}}}",
                json_string(crate::PROTOCOL),
                json_string(&offer.version),
                json_string(&offer.name)
            );
            respond(&mut stream, "200 OK", "application/json", body.as_bytes())
        }
        "/v1/bundle" => {
            let typed = headers
                .iter()
                .find(|(k, _)| k == "x-wisp-code")
                .map(|(_, v)| crate::normalize_code(v))
                .unwrap_or_default();
            if state.locked.load(Ordering::Relaxed) {
                return respond(&mut stream, "410 Gone", "text/plain", b"closed");
            }
            if !same_code(&typed, &offer.code) {
                let wrong = state.wrong.fetch_add(1, Ordering::Relaxed) + 1;
                if wrong >= MAX_WRONG_CODES {
                    state.locked.store(true, Ordering::Relaxed);
                }
                // Slows guessing down further.
                std::thread::sleep(Duration::from_secs(1));
                return respond(&mut stream, "403 Forbidden", "text/plain", b"wrong code");
            }
            if state.sending.swap(true, Ordering::Relaxed) {
                return respond(&mut stream, "409 Conflict", "text/plain", b"busy");
            }
            let result = (|| {
                let files = files_to_send(offer)?;
                let total: u64 = files.iter().map(|(_, s)| s).sum();
                state.total.store(total, Ordering::Relaxed);
                state.sent.store(0, Ordering::Relaxed);
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n"
                )?;
                let mut counted = Counting {
                    inner: io::BufWriter::with_capacity(256 * 1024, &mut stream),
                    sent: &state.sent,
                };
                bundle::write(&offer.root, &files, &mut counted)?;
                state.done.fetch_add(1, Ordering::Relaxed);
                Ok(())
            })();
            state.sending.store(false, Ordering::Relaxed);
            result
        }
        _ => respond(&mut stream, "404 Not Found", "text/plain", b"not found"),
    }
}

/// Counts what goes through, for the launcher's progress bar.
struct Counting<'a, W: Write> {
    inner: W,
    sent: &'a AtomicU64,
}

impl<W: Write> Write for Counting<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.sent.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
