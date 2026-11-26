//! `wisp-cosmetics-server`, the Wisp server (cosmetics and friends): serves on `PORT`
//! (default 8080), keeps its data in `WISP_COSMETICS_DB` (default `/data/cosmetics.sqlite3`
//! when a `/data` volume is there, else `cosmetics.sqlite3`); `WISP_COSMETICS_ITEMS` (comma
//! separated ids) limits the items players may wear. Setup: `docs/wisp-server.md`.
use std::net::SocketAddr;
use std::time::Duration;

use wisp_cosmetics_server::{App, SessionServer, Store, creators, router};

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let path = wisp_compat::var("WISP_COSMETICS_DB")
        .ok()
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| {
            if std::path::Path::new("/data").is_dir() {
                "/data/cosmetics.sqlite3".into()
            } else {
                "cosmetics.sqlite3".into()
            }
        });
    let port: u16 = wisp_compat::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let allowed: Vec<String> = wisp_compat::var("WISP_COSMETICS_ITEMS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    let store = Store::open(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    // `badge`, `creator` and `uncreator` change the data and end: run them on the server.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        std::process::exit(command(&store, &args));
    }
    let admins: Vec<String> = wisp_compat::var("WISP_ADMINS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    if admins.is_empty() {
        eprintln!("No WISP_ADMINS: nobody can choose creators in the game.");
    }
    let app = App::new(store, SessionServer, allowed).with_admins(&admins);
    let sweeper = app.clone();
    tokio::spawn(async move {
        let mut every = tokio::time::interval(Duration::from_secs(30));
        loop {
            every.tick().await;
            sweeper.sweep();
        }
    });
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port)))
        .await
        .unwrap_or_else(|e| panic!("port {port}: {e}"));
    eprintln!("Wisp server (cosmetics and friends): listening on {port}, data in {path}");
    eprintln!("Check it: http://127.0.0.1:{port}/healthz answers ok.");
    axum::serve(listener, router(app))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("the server");
}

/// The commands the server runs on its own data, an exit code back:
/// - `creator <name>`: makes the player (who has used Wisp) a creator.
/// - `uncreator <name>`: no longer a creator.
/// - `badge <name> <file.png> [#rrggbb]`: the creator's badge, a PNG of at most 16x16, with
///   the line under their name (white without one).
fn command(store: &Store, args: &[String]) -> i32 {
    use base64::Engine;
    let db = store.db();
    let creator_uuid = |name: &str| -> Option<String> {
        db.query_row(
            "SELECT uuid FROM creators WHERE name = ?1 COLLATE NOCASE",
            [name],
            |r| r.get(0),
        )
        .ok()
    };
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["creator", name] => match creators::add_by_name(&db, name) {
            Ok(Some((uuid, name))) => {
                println!("{name} ({uuid}) is a creator.");
                0
            }
            Ok(None) => {
                eprintln!("{name} hasn't used Wisp yet: they sign in once, then try again.");
                1
            }
            Err(e) => {
                eprintln!("{e}");
                1
            }
        },
        ["uncreator", name] => match creator_uuid(name) {
            Some(uuid) => {
                let _ = creators::remove(&db, &uuid);
                println!("{name} is no longer a creator.");
                0
            }
            None => {
                eprintln!("{name} isn't a creator.");
                1
            }
        },
        ["badge", name, file, rest @ ..] if rest.len() <= 1 => {
            let Some(uuid) = creator_uuid(name) else {
                eprintln!("{name} isn't a creator: `creator {name}` first.");
                return 1;
            };
            let png = match std::fs::read(file) {
                Ok(bytes) => base64::engine::general_purpose::STANDARD.encode(bytes),
                Err(e) => {
                    eprintln!("{file}: {e}");
                    return 1;
                }
            };
            let line = rest.first().copied().unwrap_or("#ffffff");
            match creators::badge_item(&png, line).and_then(|(item, hash)| {
                creators::set_badge(&db, &uuid, &item, &hash).map_err(|e| e.to_string())
            }) {
                Ok(_) => {
                    println!("{name} wears the badge from {file}.");
                    0
                }
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            }
        }
        _ => {
            eprintln!(
                "Commands: creator <name> | uncreator <name> | badge <name> <file.png> [#rrggbb]\n\
                 Without one it serves."
            );
            2
        }
    }
}
