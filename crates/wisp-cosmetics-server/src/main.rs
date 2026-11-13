//! `wisp-cosmetics-server`, the Wisp server (cosmetics and friends): serves on `PORT`
//! (default 8080), keeps its data in `WISP_COSMETICS_DB` (default `/data/cosmetics.sqlite3`
//! when a `/data` volume is there, else `cosmetics.sqlite3`); `WISP_COSMETICS_ITEMS` (comma
//! separated ids) limits the items players may wear. Setup: `docs/wisp-server.md`.
use std::net::SocketAddr;
use std::time::Duration;

use wisp_cosmetics_server::{App, SessionServer, Store, router};

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
    let app = App::new(store, SessionServer, allowed);
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
