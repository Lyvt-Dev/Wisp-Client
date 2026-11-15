//! Phone link: the phone app gets the prepared game files from the player's own PC.
//!
//! The game files come out of `wisp-prepare`, which runs Mojang's Java code and so only works on
//! a PC. They may not be shipped in the app. The launcher on the player's PC offers them on the
//! local network while its "Connect a phone" dialog is open; the phone finds it, the player types
//! the six-digit code the launcher shows, and the files come over in one stream.
//!
//! - Discovery ([`discover`], [`Responder`]): the phone broadcasts a UDP question on
//!   [`DISCOVERY_PORT`]; every launcher offering files answers with its TCP port and name.
//! - Transfer ([`Server`], [`download`]): HTTP/1.1 on that port. `GET /v1/hello` says the
//!   Minecraft version and the PC's name; `GET /v1/bundle` with the code in `X-Wisp-Code`
//!   streams every file as a [`bundle`]. Five wrong codes close the offer.
//!
//! The files are Mojang's, from the player's own copy, and only go to a device that knows the
//! code: nothing is uploaded anywhere.
#![forbid(unsafe_code)]

pub mod bundle;
mod client;
mod discovery;
mod server;

pub use client::{DownloadError, Hello, Progress, download, hello};
pub use discovery::{Found, Responder, discover};
pub use server::{Offer, Server};

/// The UDP port the phone asks on.
pub const DISCOVERY_PORT: u16 = 25591;
/// The TCP port the launcher tries first (the next free one after it otherwise).
pub const TRANSFER_PORT: u16 = 25592;
/// The protocol's name and version, first in every discovery datagram.
pub const PROTOCOL: &str = "WISP-PHONE-LINK/1";
/// The layout of the prepared files this game reads (`wisp_prepare::SCHEMA`; a test there keeps
/// them equal). A phone refuses files of another layout instead of failing to start.
pub const RESOURCES_SCHEMA: i64 = 7;
/// Wrong codes before an offer closes.
pub const MAX_WRONG_CODES: u32 = 5;

/// A fresh six-digit code, from the system's random source.
pub fn new_code() -> String {
    let mut bytes = [0u8; 4];
    getrandom::fill(&mut bytes).expect("system random source");
    format!("{:06}", u32::from_le_bytes(bytes) % 1_000_000)
}

/// The code as typed: digits only (spaces and dashes the player added are dropped).
pub fn normalize_code(typed: &str) -> String {
    typed.chars().filter(char::is_ascii_digit).collect()
}

/// Equal without returning early on the first different byte.
fn same_code(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schema_is_the_preparers() {
        assert_eq!(RESOURCES_SCHEMA, wisp_prepare::SCHEMA);
    }

    #[test]
    fn codes_are_six_digits() {
        for _ in 0..50 {
            let code = new_code();
            assert_eq!(code.len(), 6);
            assert!(code.chars().all(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn typed_codes_lose_separators() {
        assert_eq!(normalize_code(" 123-456 "), "123456");
        assert_eq!(normalize_code("12 34 56"), "123456");
    }

    #[test]
    fn code_comparison() {
        assert!(same_code("123456", "123456"));
        assert!(!same_code("123456", "123457"));
        assert!(!same_code("12345", "123456"));
    }
}
