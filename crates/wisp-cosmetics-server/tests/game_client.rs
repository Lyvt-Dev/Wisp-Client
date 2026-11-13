//! The game's own client code (`wisp_cosmetics`: the sharing thread and the worn-items cache)
//! against this server over real HTTP, as two Wisp players would use it.
use wisp_cosmetics::share::Sharing;
use wisp_cosmetics::{Cosmetics, Http};
use wisp_cosmetics_server::{App, Mojang, Store, router};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

const STEVE: u128 = 0x0123456789abcdef0123456789abcdef;

/// Mojang confirms Steve's join.
struct FakeMojang;

impl Mojang for FakeMojang {
    fn has_joined(&self, name: &str, _: &str) -> Result<Option<String>, String> {
        Ok((name == "Steve").then(|| format!("{STEVE:032x}")))
    }
}

fn serve() -> String {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            tx.send(listener.local_addr().unwrap()).unwrap();
            let app = App::new(Store::memory(), FakeMojang, Vec::new());
            axum::serve(listener, router(app)).await.unwrap();
        });
    });
    format!("http://{}", rx.recv().unwrap())
}

fn wait_for<T>(mut found: impl FnMut() -> Option<T>) -> T {
    let start = Instant::now();
    loop {
        if let Some(value) = found() {
            return value;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "timed out");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The Shop's real Nebula Wings, as the launcher writes them for the game.
fn wings() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../launcher/assets/shop/nebula_wings.json");
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn one_player_wears_and_emotes_and_another_sees_it() {
    let base = serve();
    // Steve's game: signed in, publishing what he wears and the emote he plays.
    let steve = Sharing::start(Http::new(&base, "Wisp/test"), "Steve".into(), Some(Box::new(|_| Ok(()))));
    steve.wear(vec![wings()]);
    // Published once the service lists it (the game keeps an answer a minute, so Alex asks
    // after that).
    wait_for(|| {
        let answer = ureq::post(&format!("{base}/v1/worn"))
            .set("Content-Type", "application/json")
            .send_string(&json!({ "players": [format!("{STEVE:032x}")] }).to_string())
            .ok()?
            .into_string()
            .ok()?;
        answer.contains("nebula_wings").then_some(())
    });

    // Alex's game: sees Steve in view, asks what he wears and what he plays.
    let alex = Cosmetics::new(Http::new(&base, "Wisp/test"), None);
    let worn = wait_for(|| alex.frame().worn(STEVE).filter(|w| !w.is_empty()));
    assert_eq!(worn[0].id, "nebula_wings");
    assert!(worn[0].frames.len() > 1, "the animated texture made the trip");

    let alex_sharing = Sharing::start(Http::new(&base, "Wisp/test"), String::new(), None);
    alex_sharing.asking([STEVE]);
    steve.emote(Some("supernova_crown"));
    let playing = wait_for(|| alex_sharing.playing().remove(&STEVE));
    assert_eq!(playing.id, "supernova_crown");
    assert!(playing.started.elapsed() < Duration::from_secs(5));

    steve.emote(None);
    wait_for(|| alex_sharing.playing().is_empty().then_some(()));

    // Hidden (Show my cosmetics off publishes nothing): a fresh game sees nothing.
    steve.wear(Vec::new());
    std::thread::sleep(Duration::from_millis(500));
    let fresh = Cosmetics::new(Http::new(&base, "Wisp/test"), None);
    std::thread::sleep(Duration::from_millis(800));
    let _ = fresh.frame().worn(STEVE);
    std::thread::sleep(Duration::from_millis(800));
    assert!(fresh.frame().worn(STEVE).is_none_or(|w| w.is_empty()));
}
