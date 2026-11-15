//! The phone's side: asks a launcher what it offers and downloads it.
use crate::bundle;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// What a launcher says about its offer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hello {
    pub version: String,
    pub name: String,
    /// `wisp-resources.json`'s schema (0 when the PC has no prepared files).
    pub schema: i64,
}

/// A download's progress: bytes so far of the whole.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
}

fn connect(addr: SocketAddr) -> io::Result<TcpStream> {
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    Ok(stream)
}

/// Sends a GET and reads the status line and headers; the body follows in the reader.
fn get(
    addr: SocketAddr,
    path: &str,
    code: Option<&str>,
) -> io::Result<(u16, BufReader<TcpStream>)> {
    let mut stream = connect(addr)?;
    let code = code.map_or(String::new(), |c| format!("X-Wisp-Code: {c}\r\n"));
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\n{code}Connection: close\r\n\r\n"
    )?;
    stream.flush()?;
    let mut reader = BufReader::with_capacity(256 * 1024, stream);
    let mut status = String::new();
    reader.read_line(&mut status)?;
    let status: u16 = status
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "not an HTTP answer"))?;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header == "\r\n" || header == "\n" {
            break;
        }
    }
    Ok((status, reader))
}

/// The offer of the launcher at `addr`.
pub fn hello(addr: SocketAddr) -> io::Result<Hello> {
    let (status, mut reader) = get(addr, "/v1/hello", None)?;
    if status != 200 {
        return Err(io::Error::other(format!("the PC answered {status}")));
    }
    let mut body = String::new();
    reader.read_to_string(&mut body)?;
    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let field = |k: &str| value[k].as_str().unwrap_or_default().to_owned();
    if field("protocol") != crate::PROTOCOL {
        return Err(io::Error::other(
            "this PC's Wisp speaks another phone link version: update both",
        ));
    }
    Ok(Hello {
        version: field("version"),
        name: field("name"),
        schema: value["schema"].as_i64().unwrap_or(0),
    })
}

/// Why a download failed, in words for the player.
#[derive(Debug)]
pub enum DownloadError {
    WrongCode,
    /// Too many wrong codes, or the dialog on the PC was closed.
    Closed,
    /// Another phone is downloading.
    Busy,
    Cancelled,
    Io(io::Error),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongCode => f.write_str("wrong code"),
            Self::Closed => f.write_str("the PC closed the connection offer"),
            Self::Busy => f.write_str("another device is downloading"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for DownloadError {}

impl From<io::Error> for DownloadError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// A reader that stops with an error once `cancel` is set.
struct Cancellable<'a, R> {
    inner: R,
    cancel: &'a AtomicBool,
}

impl<R: Read> Read for Cancellable<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }
        self.inner.read(buf)
    }
}

/// Downloads the offer into `target`. The files land in `target.part` first; only a complete
/// download replaces `target`, so a cut connection leaves the old files (or none) in place.
pub fn download(
    addr: SocketAddr,
    code: &str,
    target: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<(), DownloadError> {
    let (status, reader) = get(addr, "/v1/bundle", Some(&crate::normalize_code(code)))?;
    match status {
        200 => {}
        403 => return Err(DownloadError::WrongCode),
        410 => return Err(DownloadError::Closed),
        409 => return Err(DownloadError::Busy),
        other => {
            return Err(DownloadError::Io(io::Error::other(format!(
                "the PC answered {other}"
            ))));
        }
    }
    let mut input = Cancellable {
        inner: reader,
        cancel,
    };
    let part = target.with_extension("part");
    let _ = std::fs::remove_dir_all(&part);
    std::fs::create_dir_all(&part)?;
    let result = (|| {
        let header = bundle::read_header(&mut input)?;
        progress(Progress {
            done: 0,
            total: header.bytes,
        });
        bundle::read_files(&mut input, header, &part, &mut |done| {
            progress(Progress {
                done,
                total: header.bytes,
            });
        })
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&part);
        return Err(if cancel.load(Ordering::Relaxed) {
            DownloadError::Cancelled
        } else {
            DownloadError::Io(e)
        });
    }
    let old = target.with_extension("old");
    let _ = std::fs::remove_dir_all(&old);
    if target.exists() {
        std::fs::rename(target, &old)?;
    }
    std::fs::rename(&part, target)?;
    let _ = std::fs::remove_dir_all(&old);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Offer, Server};
    use std::net::TcpListener;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "wisp-phone-link-client-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn offer(root: &Path) -> (Server, SocketAddr) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = Server::with_listener(
            listener,
            Offer {
                root: root.to_path_buf(),
                version: "26.3".into(),
                name: "Test PC".into(),
                code: "123456".into(),
            },
        )
        .unwrap();
        (server, addr)
    }

    #[test]
    fn the_phone_gets_the_files_but_not_the_logs() {
        let from = scratch("from");
        std::fs::create_dir_all(from.join("assets")).unwrap();
        std::fs::write(from.join("wisp-resources.json"), b"{\"schema\":7}").unwrap();
        std::fs::write(from.join("assets/a.png"), vec![9u8; 300_000]).unwrap();
        std::fs::write(from.join("export.log"), b"noise").unwrap();
        let (server, addr) = offer(&from);

        assert_eq!(
            hello(addr).unwrap(),
            Hello {
                version: "26.3".into(),
                name: "Test PC".into(),
                schema: 7,
            }
        );
        let to = scratch("to").join("26.3");
        let mut last = Progress::default();
        download(addr, "123 456", &to, &AtomicBool::new(false), &mut |p| {
            last = p
        })
        .unwrap();
        assert_eq!(last.done, last.total);
        assert_eq!(last.total, 300_000 + 12);
        assert_eq!(
            std::fs::read(to.join("assets/a.png")).unwrap().len(),
            300_000
        );
        assert!(to.join("wisp-resources.json").is_file());
        assert!(!to.join("export.log").exists());
        assert!(!to.with_extension("part").exists());
        assert_eq!(server.finished(), 1);
    }

    #[test]
    fn wrong_codes_close_the_offer() {
        let from = scratch("wrong");
        std::fs::write(from.join("x"), b"x").unwrap();
        let (server, addr) = offer(&from);
        let to = scratch("wrong-to").join("26.3");
        let cancel = AtomicBool::new(false);
        for _ in 0..crate::MAX_WRONG_CODES {
            assert!(matches!(
                download(addr, "000000", &to, &cancel, &mut |_| {}),
                Err(DownloadError::WrongCode)
            ));
        }
        assert!(server.closed());
        // Even the right code is refused now.
        assert!(matches!(
            download(addr, "123456", &to, &cancel, &mut |_| {}),
            Err(DownloadError::Closed)
        ));
        assert!(!to.exists());
    }
}
