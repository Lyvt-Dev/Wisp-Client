//! Creators: players the Wisp admins (the client's owner) choose. A creator picks a creator code
//! and may change it. A player who enters a creator's code supports that creator until they
//! enter another or clear it: they get `DISCOUNT` percent off every Shop item, they wear the
//! creator's badge before their name (in place of Wisp's badge), and the creator earns
//! `DISCOUNT` percent of each item's full price in Wisp Coins, straight into their wallet
//! (`shop.rs` buys). The admins give each creator their badge, a PNG of at most 16x16.
//!
//! Supporters, purchases and earnings are kept by the creator's UUID, so a new code keeps them.
//!
//! Admins are the accounts in `WISP_ADMINS` (UUIDs, comma separated). Without a session:
//! - `GET /v1/creators` answers `{"discount", "creators": [{"code", "name"}, …]}`.
//!
//! With one:
//! - `GET /v1/creator/me`: `{"admin", "creator", "code", "badge", "supporters", "purchases",
//!   "earned", "discount", "supporting": {"code", "name"} | null}`.
//! - `PUT /v1/creator/code` `{"code"}`: a creator sets their code (3 to 16 letters, digits or
//!   `_`, unique without regard to case).
//! - `PUT /v1/supporting` `{"code": "…" | null}`: the player supports the creator with that code
//!   (never themselves), or nobody.
//!
//! Admins only:
//! - `GET /v1/admin/creators`: every creator with their numbers.
//! - `POST /v1/admin/creators` `{"name"}`: makes a player who has used Wisp a creator.
//! - `DELETE /v1/admin/creators/<uuid>`: no longer a creator (their supporters support nobody).
//! - `PUT /v1/admin/creators/<uuid>/badge` `{"png": base64, "line": "#rrggbb"}`: their badge;
//!   `DELETE` on the same path takes it away.
//!
//! The server binary does the admin work on the server itself: `creator <name>`, `uncreator
//! <name>` and `badge <name> <file.png> [#rrggbb]` (`docs/wisp-server.md`).
use super::{App, now_secs, refuse, uuid};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Percent off every Shop item for a supporter, and the creator's share of its price.
pub const DISCOUNT: u32 = 10;
/// The id of every creator badge item (each creator's has its own hash).
pub const BADGE_ID: &str = "creator_badge";
/// Wisp's own badge, which a creator badge takes the place of.
pub const WISP_BADGE: &str = "wisp_badge";

pub(crate) fn tables(db: &Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS creators (
             uuid TEXT PRIMARY KEY, name TEXT NOT NULL,
             code TEXT UNIQUE COLLATE NOCASE, badge TEXT,
             earned INTEGER NOT NULL DEFAULT 0,
             added INTEGER NOT NULL);
         CREATE TABLE IF NOT EXISTS supporters (
             player TEXT PRIMARY KEY, creator TEXT NOT NULL, since INTEGER NOT NULL);
         CREATE TABLE IF NOT EXISTS purchases (
             player TEXT NOT NULL, item TEXT NOT NULL, price INTEGER NOT NULL,
             creator TEXT, share INTEGER NOT NULL, at INTEGER NOT NULL,
             PRIMARY KEY (player, item));",
    )
}

/// A creator code as typed: trimmed, 3 to 16 letters, digits or `_`, in capitals.
pub fn code(text: &str) -> Option<String> {
    let code = text.trim();
    ((3..=16).contains(&code.len()) && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .then(|| code.to_uppercase())
}

/// A purchase a supporter made: `creator` earned `share` of the item's full `price`.
pub(crate) fn record(
    tx: &rusqlite::Transaction,
    player: &str,
    item: &str,
    price: u32,
    creator: &str,
    share: u32,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT OR IGNORE INTO purchases (player, item, price, creator, share, at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![player, item, price, creator, share, now_secs()],
    )?;
    tx.execute(
        "UPDATE creators SET earned = earned + ?2 WHERE uuid = ?1",
        params![creator, share],
    )?;
    Ok(())
}

/// The name tag item of a badge: the PNG as the mark, `line` (`#rrggbb`) under the name.
/// Checked exactly as the game checks it. Answers the item file and its hash.
pub fn badge_item(png_base64: &str, line: &str) -> Result<(Vec<u8>, String), String> {
    let line = line.trim();
    let valid_line = line.len() == 7
        && line.starts_with('#')
        && line[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !valid_line {
        return Err("the line colour must look like #rrggbb".into());
    }
    let file = json!({
        "format": 1,
        "id": BADGE_ID,
        "parts": [],
        "slot": "name_tag",
        "tag": { "line": line },
        "texture": { "png": png_base64.trim() },
    })
    .to_string()
    .into_bytes();
    wisp_cosmetics::Item::parse(&file).map_err(|e| format!("the badge is refused: {e}"))?;
    let hash = Sha256::digest(&file)
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok((file, hash))
}

/// Stores `file` (from [`badge_item`]) as `creator`'s badge. False when they are no creator.
pub fn set_badge(
    db: &Connection,
    creator: &str,
    file: &[u8],
    hash: &str,
) -> rusqlite::Result<bool> {
    db.execute(
        "INSERT OR IGNORE INTO items (hash, id, body) VALUES (?1, ?2, ?3)",
        params![hash, BADGE_ID, file],
    )?;
    Ok(db.execute(
        "UPDATE creators SET badge = ?2 WHERE uuid = ?1",
        params![creator, hash],
    )? == 1)
}

/// Makes the player named `name` (as they last signed in) a creator: their UUID and name.
pub fn add_by_name(db: &Connection, name: &str) -> rusqlite::Result<Option<(String, String)>> {
    let found: Option<(String, String)> = db
        .query_row(
            "SELECT uuid, name FROM players WHERE name = ?1 COLLATE NOCASE
             ORDER BY last_seen DESC LIMIT 1",
            [name.trim()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((uuid, name)) = &found {
        db.execute(
            "INSERT INTO creators (uuid, name, added) VALUES (?1, ?2, ?3)
             ON CONFLICT(uuid) DO UPDATE SET name = ?2",
            params![uuid, name, now_secs()],
        )?;
    }
    Ok(found)
}

/// No longer a creator: their supporters support nobody. False when they weren't one.
pub fn remove(db: &Connection, creator: &str) -> rusqlite::Result<bool> {
    db.execute("DELETE FROM supporters WHERE creator = ?1", [creator])?;
    Ok(db.execute("DELETE FROM creators WHERE uuid = ?1", [creator])? == 1)
}

/// The badge each of `players` wears for being a creator or supporting one, by hash.
pub(crate) fn badges(db: &Connection, players: &[String]) -> Vec<(String, String)> {
    let Ok(mut query) = db.prepare_cached(
        "SELECT badge FROM creators WHERE badge IS NOT NULL AND (uuid = ?1 OR uuid =
             (SELECT creator FROM supporters WHERE player = ?1))
         ORDER BY uuid = ?1 DESC LIMIT 1",
    ) else {
        return Vec::new();
    };
    players
        .iter()
        .filter_map(|p| {
            query
                .query_row([p], |r| r.get::<_, String>(0))
                .ok()
                .map(|hash| (p.clone(), hash))
        })
        .collect()
}

/// `worn` with Wisp's badge (or none) replaced by the creator badge `hash`.
pub(crate) fn wear_badge(worn: &mut Vec<Value>, hash: &str) {
    worn.retain(|w| {
        let id = w["id"].as_str().unwrap_or_default();
        id != WISP_BADGE && id != BADGE_ID
    });
    worn.push(json!({ "id": BADGE_ID, "hash": hash }));
}

/// A creator's supporters, purchases made supporting them, and coins earned.
fn numbers(db: &Connection, creator: &str) -> (i64, i64, i64) {
    let supporters = db
        .query_row(
            "SELECT COUNT(*) FROM supporters WHERE creator = ?1",
            [creator],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let purchases = db
        .query_row(
            "SELECT COUNT(*) FROM purchases WHERE creator = ?1",
            [creator],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let earned = db
        .query_row(
            "SELECT earned FROM creators WHERE uuid = ?1",
            [creator],
            |r| r.get(0),
        )
        .unwrap_or(0);
    (supporters, purchases, earned)
}

pub(crate) async fn list(State(app): State<App>) -> Response {
    let db = app.store.db();
    let result = db
        .prepare_cached("SELECT code, name FROM creators WHERE code IS NOT NULL ORDER BY code")
        .and_then(|mut q| {
            q.query_map([], |r| {
                Ok(json!({ "code": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)? }))
            })?
            .collect::<rusqlite::Result<Vec<Value>>>()
        });
    match result {
        Ok(creators) => Json(json!({ "discount": DISCOUNT, "creators": creators })).into_response(),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub(crate) async fn me(State(app): State<App>, headers: HeaderMap) -> Response {
    let (player, _) = match app.session(&headers) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let admin = app.admins.contains(&player);
    let db = app.store.db();
    let mine: Option<(Option<String>, bool)> = db
        .query_row(
            "SELECT code, badge IS NOT NULL FROM creators WHERE uuid = ?1",
            [&player],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .ok()
        .flatten();
    let supporting: Option<Value> = db
        .query_row(
            "SELECT c.code, c.name FROM supporters s JOIN creators c ON c.uuid = s.creator
             WHERE s.player = ?1",
            [&player],
            |r| {
                Ok(json!({ "code": r.get::<_, Option<String>>(0)?,
                           "name": r.get::<_, String>(1)? }))
            },
        )
        .optional()
        .ok()
        .flatten();
    let creator = mine.is_some();
    let (code, badge) = mine.unwrap_or((None, false));
    let (supporters, purchases, earned) = numbers(&db, &player);
    Json(json!({
        "admin": admin, "creator": creator, "code": code, "badge": badge,
        "supporters": supporters, "purchases": purchases, "earned": earned,
        "discount": DISCOUNT, "supporting": supporting,
    }))
    .into_response()
}

#[derive(Deserialize)]
pub(crate) struct Code {
    code: String,
}

pub(crate) async fn set_code(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<Code>,
) -> Response {
    let (player, _) = match app.session(&headers) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let Some(code) = code(&body.code) else {
        return refuse(
            StatusCode::BAD_REQUEST,
            "A creator code has 3 to 16 letters, digits or underscores.",
        );
    };
    let db = app.store.db();
    let taken: Option<String> = db
        .query_row("SELECT uuid FROM creators WHERE code = ?1", [&code], |r| {
            r.get(0)
        })
        .optional()
        .ok()
        .flatten();
    if taken.as_ref().is_some_and(|owner| *owner != player) {
        return refuse(StatusCode::CONFLICT, "That code is taken.");
    }
    match db.execute(
        "UPDATE creators SET code = ?2 WHERE uuid = ?1",
        params![player, code],
    ) {
        Ok(1) => Json(json!({ "code": code })).into_response(),
        Ok(_) => refuse(StatusCode::FORBIDDEN, "Only creators have a creator code."),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub(crate) struct Supporting {
    code: Option<String>,
}

pub(crate) async fn set_supporting(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<Supporting>,
) -> Response {
    let (player, _) = match app.session(&headers) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let db = app.store.db();
    let Some(typed) = body.code.as_deref().filter(|c| !c.trim().is_empty()) else {
        return match db.execute("DELETE FROM supporters WHERE player = ?1", [&player]) {
            Ok(_) => Json(json!({ "supporting": null })).into_response(),
            Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        };
    };
    let creator: Option<(String, String, String)> = db
        .query_row(
            "SELECT uuid, name, code FROM creators WHERE code = ?1",
            [typed.trim()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .ok()
        .flatten();
    match creator {
        None => refuse(StatusCode::NOT_FOUND, "No creator has that code."),
        Some((uuid, _, _)) if uuid == player => {
            refuse(StatusCode::BAD_REQUEST, "You can't support yourself.")
        }
        Some((uuid, name, code)) => match db.execute(
            "INSERT INTO supporters (player, creator, since) VALUES (?1, ?2, ?3)
             ON CONFLICT(player) DO UPDATE SET creator = ?2, since = ?3",
            params![player, uuid, now_secs()],
        ) {
            Ok(_) => Json(json!({ "supporting": { "code": code, "name": name } })).into_response(),
            Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        },
    }
}

// The refusal is a ready HTTP response the handlers return as it is.
#[allow(clippy::result_large_err)]
fn admin(app: &App, headers: &HeaderMap) -> Result<String, Response> {
    let (player, _) = app.session(headers)?;
    if app.admins.contains(&player) {
        Ok(player)
    } else {
        Err(refuse(
            StatusCode::FORBIDDEN,
            "Only Wisp admins can do that.",
        ))
    }
}

/// A creator as the admins' list shows them: UUID, name, code, whether they have a badge.
type Row = (String, String, Option<String>, bool);

pub(crate) async fn admin_list(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Err(r) = admin(&app, &headers) {
        return r;
    }
    let db = app.store.db();
    let rows: rusqlite::Result<Vec<Row>> = db
        .prepare_cached("SELECT uuid, name, code, badge IS NOT NULL FROM creators ORDER BY name")
        .and_then(|mut q| {
            q.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .collect()
        });
    match rows {
        Ok(rows) => {
            let creators: Vec<Value> = rows
                .into_iter()
                .map(|(uuid, name, code, badge)| {
                    let (supporters, purchases, earned) = numbers(&db, &uuid);
                    json!({ "uuid": uuid, "name": name, "code": code, "badge": badge,
                            "supporters": supporters, "purchases": purchases, "earned": earned })
                })
                .collect();
            Json(json!({ "creators": creators })).into_response()
        }
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub(crate) struct Name {
    name: String,
}

pub(crate) async fn admin_add(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<Name>,
) -> Response {
    if let Err(r) = admin(&app, &headers) {
        return r;
    }
    let name = body.name.trim();
    if name.is_empty() || name.len() > 16 {
        return refuse(StatusCode::BAD_REQUEST, "not a Minecraft name");
    }
    match add_by_name(&app.store.db(), name) {
        Ok(Some((uuid, name))) => Json(json!({ "uuid": uuid, "name": name })).into_response(),
        Ok(None) => refuse(StatusCode::NOT_FOUND, "That player hasn't used Wisp yet."),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub(crate) async fn admin_remove(
    State(app): State<App>,
    headers: HeaderMap,
    Path(player): Path<String>,
) -> Response {
    if let Err(r) = admin(&app, &headers) {
        return r;
    }
    let Some(player) = uuid(&player) else {
        return refuse(StatusCode::BAD_REQUEST, "not a UUID");
    };
    match remove(&app.store.db(), &player) {
        Ok(removed) => Json(json!({ "removed": removed })).into_response(),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub(crate) struct Badge {
    png: String,
    #[serde(default = "white")]
    line: String,
}

fn white() -> String {
    "#ffffff".into()
}

pub(crate) async fn admin_badge(
    State(app): State<App>,
    headers: HeaderMap,
    Path(player): Path<String>,
    Json(body): Json<Badge>,
) -> Response {
    if let Err(r) = admin(&app, &headers) {
        return r;
    }
    let Some(player) = uuid(&player) else {
        return refuse(StatusCode::BAD_REQUEST, "not a UUID");
    };
    let (file, hash) = match badge_item(&body.png, &body.line) {
        Ok(item) => item,
        Err(e) => return refuse(StatusCode::BAD_REQUEST, &e),
    };
    match set_badge(&app.store.db(), &player, &file, &hash) {
        Ok(true) => Json(json!({ "badge": hash })).into_response(),
        Ok(false) => refuse(StatusCode::NOT_FOUND, "That player isn't a creator."),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub(crate) async fn admin_unbadge(
    State(app): State<App>,
    headers: HeaderMap,
    Path(player): Path<String>,
) -> Response {
    if let Err(r) = admin(&app, &headers) {
        return r;
    }
    let Some(player) = uuid(&player) else {
        return refuse(StatusCode::BAD_REQUEST, "not a UUID");
    };
    match app.store.db().execute(
        "UPDATE creators SET badge = NULL WHERE uuid = ?1",
        [&player],
    ) {
        Ok(n) => Json(json!({ "removed": n == 1 })).into_response(),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
