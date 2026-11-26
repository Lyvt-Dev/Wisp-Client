//! Offers a prepared folder to a phone as the launcher's Settings › Phone does, for testing the
//! app without the launcher (the Android emulator reaches this PC at 10.0.2.2):
//!
//!   cargo run -p wisp-phone-link --example offer -- <resources/26.3> [code] [wisp server]
fn main() {
    let mut args = std::env::args().skip(1);
    let root = args
        .next()
        .expect("usage: offer <prepared folder> [code] [wisp server]");
    let code = args.next().unwrap_or_else(wisp_phone_link::new_code);
    let server = wisp_phone_link::Server::start(wisp_phone_link::Offer {
        root: root.into(),
        version: "26.3".into(),
        name: "Test PC".into(),
        code: code.clone(),
        wisp_server: std::env::args().nth(3),
    })
    .expect("a free port");
    let _responder = wisp_phone_link::Responder::start(server.port(), "Test PC");
    println!("offering on port {} with code {code}", server.port());
    let mut shown = None;
    while !server.closed() {
        let now = (
            server.sending().map(|(s, t)| (s >> 24, t >> 24)),
            server.finished(),
        );
        if shown != Some(now) {
            println!("sending {:?} (MiB/16) finished {}", now.0, now.1);
            shown = Some(now);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    println!("closed: {:?}", server.last_error());
}
