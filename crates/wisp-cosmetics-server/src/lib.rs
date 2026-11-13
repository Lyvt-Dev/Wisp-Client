//! The Wisp server: Wisp players see each other's cosmetics and emotes on any Minecraft
//! server, because their games ask this one, keyed by the Minecraft UUID, and the launchers keep
//! their friends lists here (`friends.rs`). Nothing goes through the game protocol
//! (`docs/cosmetics.md`, `docs/wisp-server.md`).
//!
//! Signing in is the check an online-mode Minecraft server makes: `POST /v1/auth/start` hands
//! out a one-time server id, the game tells Mojang it joins it, and `POST /v1/auth/finish` asks
//! Mojang (`hasJoined`) before it answers a session of its own. The Minecraft token goes to
//! Mojang only.
//!
//! With a session (`Authorization: Bearer`):
//! - `PUT /v1/wear` `{"items": [<item file>, …]}`: what the player wears, every file checked as
//!   the game checks it (`wisp_cosmetics::Item::parse`) and kept under its hash.
//! - `PUT /v1/visible` `{"visible": bool}`: hidden, `/v1/worn` leaves the player out.
//! - `POST /v1/emote` `{"id"}` starts an emote, `DELETE /v1/emote` stops it.
//!
//! Without one:
//! - `POST /v1/worn` `{"players": [uuid hex, …]}` answers `{"ttl", "players": {uuid: [{"id",
//!   "hash"}, …]}}`, as the game's client expects (`wisp_cosmetics::Cosmetics`).
//! - `GET /v1/items/<id>?hash=<hash>`: an item file, which never changes for its hash.
//! - `POST /v1/emotes` `{"players": [...]}` answers `{"players": {uuid: {"id", "elapsed"}}}`
//!   for the ones playing an emote now.
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Players one `/v1/worn` or `/v1/emotes` request may ask about (the game asks 64 at most).
pub const BATCH: usize = 64;
/// Items a player may wear at once (one per slot, and a few slots to spare).
pub const MAX_WORN: usize = 12;
/// An item file's largest size (an HD animated texture is well under it).
pub const MAX_ITEM: usize = 4 << 20;
/// How long the game keeps a `/v1/worn` answer (it keeps it at least a minute).
pub const WORN_TTL: u64 = 60;
/// How long a session lasts.
pub const SESSION: Duration = Duration::from_secs(3600);
/// How long a sign-in may take between start and finish.
const PENDING: Duration = Duration::from_secs(120);
/// The longest emote; an emote older than this is over even if never stopped.
pub const EMOTE: Duration = Duration::from_secs(30);

/// Asks Mojang whether `name` joined `server_id`, answering the account's UUID (hex, no dashes).
pub trait Mojang: Send + Sync + 'static {
    fn has_joined(&self, name: &str, server_id: &str) -> Result<Option<String>, String>;
}

/// Mojang's session server.
pub struct SessionServer;

impl Mojang for SessionServer {
    fn has_joined(&self, name: &str, server_id: &str) -> Result<Option<String>, String> {
        let answer = ureq::get("https://sessionserver.mojang.com/session/minecraft/hasJoined")
            .query("username", name)
            .query("serverId", server_id)
            .timeout(Duration::from_secs(10))
            .call();
        match answer {
            Ok(r) if r.status() == 200 => {
                let body: Value = r.into_json().map_err(|e| e.to_string())?;
                Ok(body["id"].as_str().map(str::to_lowercase))
            }
            Ok(_) => Ok(None),
            Err(ureq::Error::Status(_, _)) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// What is kept on disk: items under their hash, and what each player wears.
pub struct Store(Mutex<Connection>);

impl Store {
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        let db = Connection::open(path)?;
        db.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS items (hash TEXT PRIMARY KEY, id TEXT NOT NULL, body BLOB NOT NULL);
             CREATE TABLE IF NOT EXISTS players (
                 uuid TEXT PRIMARY KEY, name TEXT NOT NULL, worn TEXT NOT NULL DEFAULT '[]',
                 visible INTEGER NOT NULL DEFAULT 1, updated INTEGER NOT NULL);",
        )?;
        friends::tables(&db)?;
        Ok(Self(Mutex::new(db)))
    }

    pub fn memory() -> Self {
        Self::open(":memory:").expect("an in-memory database")
    }

    fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

struct Session {
    uuid: String,
    name: String,
    expires: Instant,
}

#[derive(Default)]
struct Live {
    /// Sign-ins started: server id → when.
    pending: HashMap<String, Instant>,
    sessions: HashMap<String, Session>,
    /// Emotes playing: uuid → (id, started).
    emotes: HashMap<String, (String, Instant)>,
    /// Where players are, as their launchers last said.
    presence: HashMap<String, friends::Presence>,
    /// Where players are, as their games last said (wins over the launcher while fresh).
    in_game: HashMap<String, friends::Presence>,
}

#[derive(Clone)]
pub struct App {
    store: Arc<Store>,
    mojang: Arc<dyn Mojang>,
    live: Arc<Mutex<Live>>,
    /// Item ids accepted (`WISP_COSMETICS_ITEMS`, comma separated); empty: any valid item.
    allowed: Arc<Vec<String>>,
}

impl App {
    pub fn new(store: Store, mojang: impl Mojang, allowed: Vec<String>) -> Self {
        Self {
            store: Arc::new(store),
            mojang: Arc::new(mojang),
            live: Arc::default(),
            allowed: Arc::new(allowed),
        }
    }

    fn live(&self) -> std::sync::MutexGuard<'_, Live> {
        self.live.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Forgets expired sign-ins, sessions and emotes.
    pub fn sweep(&self) {
        let now = Instant::now();
        let mut live = self.live();
        live.pending
            .retain(|_, at| now.duration_since(*at) < PENDING);
        live.sessions.retain(|_, s| s.expires > now);
        live.emotes
            .retain(|_, (_, at)| now.duration_since(*at) < EMOTE);
        live.presence
            .retain(|_, p| now.duration_since(p.at) < friends::PRESENCE);
        live.in_game
            .retain(|_, p| now.duration_since(p.at) < friends::PRESENCE);
    }

    // The refusal is a ready HTTP response the handlers return as it is.
    #[allow(clippy::result_large_err)]
    fn session(&self, headers: &HeaderMap) -> Result<(String, String), Response> {
        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| refuse(StatusCode::UNAUTHORIZED, "sign in first"))?;
        let live = self.live();
        match live.sessions.get(token) {
            Some(s) if s.expires > Instant::now() => Ok((s.uuid.clone(), s.name.clone())),
            _ => Err(refuse(StatusCode::UNAUTHORIZED, "session expired")),
        }
    }
}

fn refuse(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn token() -> String {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).expect("the system's random source");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A UUID as the game sends it: 32 lowercase hex digits, dashes dropped.
fn uuid(text: &str) -> Option<String> {
    let hex: String = text
        .chars()
        .filter(|c| *c != '-')
        .collect::<String>()
        .to_lowercase();
    (hex.len() == 32 && hex.chars().all(|c| c.is_ascii_hexdigit())).then_some(hex)
}

pub fn router(app: App) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/v1/auth/start", post(auth_start))
        .route("/v1/auth/finish", post(auth_finish))
        .route("/v1/wear", put(wear))
        .route("/v1/visible", put(visible))
        .route("/v1/worn", post(worn))
        .route("/v1/items/{id}", get(item))
        .route("/v1/emote", post(emote_start).delete(emote_stop))
        .route("/v1/emotes", post(emotes))
        .route("/v1/presence", post(friends::presence))
        .route("/v1/friends", get(friends::list))
        .route("/v1/friends/request", post(friends::request))
        .route("/v1/friends/accept", post(friends::accept))
        .route("/v1/friends/decline", post(friends::decline))
        .route("/v1/friends/{uuid}", axum::routing::delete(friends::remove))
        .layer(DefaultBodyLimit::max(MAX_WORN * MAX_ITEM / 2))
        .with_state(app)
}

async fn auth_start(State(app): State<App>) -> Json<Value> {
    let id = token()[..20].to_owned();
    app.live().pending.insert(id.clone(), Instant::now());
    Json(json!({ "server_id": id }))
}

#[derive(Deserialize)]
struct Finish {
    server_id: String,
    name: String,
}

async fn auth_finish(State(app): State<App>, Json(body): Json<Finish>) -> Response {
    let started = app.live().pending.remove(&body.server_id);
    if !started.is_some_and(|at| at.elapsed() < PENDING) {
        return refuse(StatusCode::UNAUTHORIZED, "unknown server id");
    }
    let mojang = app.mojang.clone();
    let (name, server_id) = (body.name.clone(), body.server_id.clone());
    let joined = tokio::task::spawn_blocking(move || mojang.has_joined(&name, &server_id)).await;
    let id = match joined {
        Ok(Ok(Some(id))) => id,
        Ok(Ok(None)) => return refuse(StatusCode::UNAUTHORIZED, "Mojang did not confirm the join"),
        _ => {
            return refuse(
                StatusCode::BAD_GATEWAY,
                "Mojang's session server did not answer",
            );
        }
    };
    let Some(id) = uuid(&id) else {
        return refuse(StatusCode::BAD_GATEWAY, "Mojang answered no UUID");
    };
    friends::remember(&app, &id, &body.name);
    let session = token();
    app.live().sessions.insert(
        session.clone(),
        Session {
            uuid: id.clone(),
            name: body.name,
            expires: Instant::now() + SESSION,
        },
    );
    Json(json!({ "session": session, "uuid": id, "ttl": SESSION.as_secs() })).into_response()
}

#[derive(Deserialize)]
struct Wear {
    items: Vec<Value>,
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

async fn wear(State(app): State<App>, headers: HeaderMap, Json(body): Json<Wear>) -> Response {
    let (player, name) = match app.session(&headers) {
        Ok(s) => s,
        Err(r) => return r,
    };
    if body.items.len() > MAX_WORN {
        return refuse(StatusCode::BAD_REQUEST, "too many items");
    }
    let mut checked = Vec::new();
    let mut slots = Vec::new();
    for file in &body.items {
        let bytes = file.to_string().into_bytes();
        if bytes.len() > MAX_ITEM {
            return refuse(StatusCode::PAYLOAD_TOO_LARGE, "an item file is too large");
        }
        let item = match wisp_cosmetics::Item::parse(&bytes) {
            Ok(item) => item,
            Err(e) => return refuse(StatusCode::BAD_REQUEST, &format!("an item is refused: {e}")),
        };
        if !app.allowed.is_empty() && !app.allowed.contains(&item.id) {
            return refuse(
                StatusCode::BAD_REQUEST,
                &format!("{} is not an Wisp item", item.id),
            );
        }
        if slots.contains(&item.slot) {
            return refuse(StatusCode::BAD_REQUEST, "two items in one slot");
        }
        slots.push(item.slot);
        let hash: String = Sha256::digest(&bytes)
            .iter()
            .take(16)
            .map(|b| format!("{b:02x}"))
            .collect();
        checked.push((item.id, hash, bytes));
    }
    let worn: Vec<Value> = checked
        .iter()
        .map(|(id, hash, _)| json!({ "id": id, "hash": hash }))
        .collect();
    let result = (|| -> rusqlite::Result<()> {
        let mut db = app.store.db();
        let tx = db.transaction()?;
        for (id, hash, bytes) in &checked {
            tx.execute(
                "INSERT OR IGNORE INTO items (hash, id, body) VALUES (?1, ?2, ?3)",
                params![hash, id, bytes],
            )?;
        }
        tx.execute(
            "INSERT INTO players (uuid, name, worn, updated) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(uuid) DO UPDATE SET name = ?2, worn = ?3, updated = ?4",
            params![
                player,
                name,
                Value::from(worn.clone()).to_string(),
                now_secs()
            ],
        )?;
        tx.commit()
    })();
    match result {
        Ok(()) => Json(json!({ "worn": worn })).into_response(),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
struct Visible {
    visible: bool,
}

async fn visible(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<Visible>,
) -> Response {
    let (player, name) = match app.session(&headers) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let result = app.store.db().execute(
        "INSERT INTO players (uuid, name, visible, updated) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(uuid) DO UPDATE SET visible = ?3, updated = ?4",
        params![player, name, body.visible, now_secs()],
    );
    match result {
        Ok(_) => Json(json!({ "visible": body.visible })).into_response(),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
struct Players {
    players: Vec<String>,
}

async fn worn(State(app): State<App>, Json(body): Json<Players>) -> Response {
    if body.players.len() > BATCH {
        return refuse(StatusCode::BAD_REQUEST, "too many players");
    }
    let mut out = serde_json::Map::new();
    let db = app.store.db();
    let mut query =
        match db.prepare_cached("SELECT worn FROM players WHERE uuid = ?1 AND visible = 1") {
            Ok(q) => q,
            Err(e) => return refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        };
    for player in body.players.iter().filter_map(|p| uuid(p)) {
        let worn: Option<String> = query
            .query_row([&player], |r| r.get(0))
            .optional()
            .ok()
            .flatten();
        if let Some(list) = worn.and_then(|w| serde_json::from_str::<Value>(&w).ok())
            && list.as_array().is_some_and(|a| !a.is_empty())
        {
            out.insert(player, list);
        }
    }
    Json(json!({ "ttl": WORN_TTL, "players": out })).into_response()
}

#[derive(Deserialize)]
struct Hash {
    hash: String,
}

async fn item(State(app): State<App>, Path(id): Path<String>, Query(q): Query<Hash>) -> Response {
    let body: Option<Vec<u8>> = app
        .store
        .db()
        .query_row(
            "SELECT body FROM items WHERE hash = ?1 AND id = ?2",
            params![q.hash, id],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten();
    match body {
        Some(bytes) => (
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            bytes,
        )
            .into_response(),
        None => refuse(StatusCode::NOT_FOUND, "no such item"),
    }
}

#[derive(Deserialize)]
struct Emote {
    id: String,
}

async fn emote_start(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<Emote>,
) -> Response {
    let (player, _) = match app.session(&headers) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let id = body.id.trim();
    if id.is_empty()
        || id.len() > 64
        || !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
    {
        return refuse(StatusCode::BAD_REQUEST, "not an emote id");
    }
    app.live()
        .emotes
        .insert(player, (id.to_owned(), Instant::now()));
    Json(json!({ "ok": true })).into_response()
}

async fn emote_stop(State(app): State<App>, headers: HeaderMap) -> Response {
    let (player, _) = match app.session(&headers) {
        Ok(s) => s,
        Err(r) => return r,
    };
    app.live().emotes.remove(&player);
    Json(json!({ "ok": true })).into_response()
}

async fn emotes(State(app): State<App>, Json(body): Json<Players>) -> Response {
    if body.players.len() > BATCH {
        return refuse(StatusCode::BAD_REQUEST, "too many players");
    }
    let live = app.live();
    let mut out = serde_json::Map::new();
    for player in body.players.iter().filter_map(|p| uuid(p)) {
        if let Some((id, at)) = live.emotes.get(&player)
            && at.elapsed() < EMOTE
        {
            out.insert(
                player,
                json!({ "id": id, "elapsed": at.elapsed().as_secs_f64() }),
            );
        }
    }
    Json(json!({ "players": out })).into_response()
}

mod friends;

#[cfg(test)]
mod tests;
