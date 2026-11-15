//! Settings › Phone: offers this PC's prepared game files to the Wisp app on a phone in the same
//! Wi-Fi (`wisp-phone-link`). The files are the player's own (prepared from their Minecraft) and
//! only go to a phone that types the code shown here; the offer ends after 15 idle minutes.
use std::net::{Ipv4Addr, UdpSocket};
use std::path::Path;
use std::time::{Duration, Instant};
use wisp_phone_link::{Offer, Responder, Server};

/// An offer without a phone downloading ends after this long.
const IDLE_LIMIT: Duration = Duration::from_secs(15 * 60);

/// What the page shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Waiting,
    Sending {
        sent: u64,
        total: u64,
    },
    /// A phone got everything (how many so far).
    Sent(u32),
    /// Too many wrong codes, or idle too long.
    Closed,
}

pub struct PhoneLink {
    server: Server,
    _responder: Option<Responder>,
    pub code: String,
    /// This PC's address in the local network, for a phone that can't find it by itself.
    pub address: Option<String>,
    /// The phone finds this PC by itself (discovery runs).
    pub discoverable: bool,
    last_active: Instant,
    shown: Option<Status>,
}

/// The name a phone shows for this PC.
fn pc_name() -> String {
    wisp_compat::var("COMPUTERNAME")
        .or_else(|_| wisp_compat::var("HOSTNAME"))
        .ok()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "PC".into())
}

/// The address other devices reach this PC at: the interface a route out would use. No packet
/// is sent (connecting a UDP socket only picks the route).
fn local_address() -> Option<String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect(("192.0.2.1", 9)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then(|| ip.to_string())
}

impl PhoneLink {
    /// Starts offering `resources` (a prepared `resources/<version>` folder).
    pub fn start(resources: &Path, version: &str) -> Result<Self, String> {
        if !resources.join("wisp-resources.json").is_file() {
            return Err("This instance's game files aren't prepared yet. Press Play once, then connect the phone.".into());
        }
        let code = wisp_phone_link::new_code();
        let server = Server::start(Offer {
            root: resources.to_path_buf(),
            version: version.to_owned(),
            name: pc_name(),
            code: code.clone(),
        })
        .map_err(|e| format!("Couldn't open a port for the phone: {e}"))?;
        // Another launcher may hold the discovery port: the phone can still type the address.
        let responder = Responder::start(server.port(), &pc_name()).ok();
        Ok(Self {
            discoverable: responder.is_some(),
            _responder: responder,
            server,
            code,
            address: local_address(),
            last_active: Instant::now(),
            shown: None,
        })
    }

    pub fn status(&self) -> Status {
        if let Some((sent, total)) = self.server.sending() {
            return Status::Sending { sent, total };
        }
        if self.server.closed() || self.last_active.elapsed() >= IDLE_LIMIT {
            return Status::Closed;
        }
        match self.server.finished() {
            0 => Status::Waiting,
            n => Status::Sent(n),
        }
    }

    /// Once a timer tick: true when the page should redraw.
    pub fn tick(&mut self) -> bool {
        if self.server.sending().is_some() {
            self.last_active = Instant::now();
        }
        let status = self.status();
        if status == Status::Closed {
            self.server.stop();
        }
        let changed = self.shown != Some(status);
        self.shown = Some(status);
        changed
    }
}
