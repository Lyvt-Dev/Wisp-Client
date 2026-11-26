//! Friends, on the same server as the cosmetics: requests by Minecraft name, accepted friends,
//! and where each player is (in the launcher, playing, on which server, or last seen when).
//!
//! With a session (`Authorization: Bearer`, the same sign-in as the cosmetics):
//! - `POST /v1/presence` `{"status": "launcher" | "playing", "server": "host[:port]"?,
//!   "from": "game"?}` says where the player is and answers the friends list below, so a launcher
//!   needs one call a while. The game says `"from": "game"` with the server it is really on; that
//!   wins over the launcher's guess while it is fresh.
//! - `GET /v1/friends` answers `{"friends": [{"uuid", "name", "online", "status", "server",
//!   "last_seen"}], "incoming": [{"uuid", "name"}], "outgoing": [{"uuid", "name"}]}`.
//! - `POST /v1/friends/request` `{"name"}`: a request to the player of that name, who must have
//!   signed in here once. A request back to someone who asked first makes friends at once.
//! - `POST /v1/friends/accept` and `/v1/friends/decline` `{"uuid"}` answer a request.
//! - `DELETE /v1/friends/{uuid}` removes a friend or takes back a request.
use super::*;

/// Friends and open requests one player may have.
pub const MAX_FRIENDS: usize = 200;
/// A launcher that hasn't said where it is for this long is offline.
pub const PRESENCE: Duration = Duration::from_secs(90);

/// Where a player is, kept in memory only.
pub(crate) struct Presence {
    pub status: String,
    pub server: Option<String>,
    pub at: Instant,
}

pub(crate) fn tables(db: &Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS friends (
             a TEXT NOT NULL, b TEXT NOT NULL, since INTEGER NOT NULL, PRIMARY KEY (a, b));
         CREATE TABLE IF NOT EXISTS requests (
             sender TEXT NOT NULL, receiver TEXT NOT NULL, at INTEGER NOT NULL,
             PRIMARY KEY (sender, receiver));
         CREATE INDEX IF NOT EXISTS players_name ON players (name COLLATE NOCASE);",
    )?;
    // Older databases: when the player was last here.
    let _ = db.execute(
        "ALTER TABLE players ADD COLUMN last_seen INTEGER NOT NULL DEFAULT 0",
        [],
    );
    Ok(())
}

/// Remembers a signed-in player's name (found by friends' requests) and when they were here.
pub(crate) fn remember(app: &App, uuid: &str, name: &str) {
    let _ = app.store.db().execute(
        "INSERT INTO players (uuid, name, updated, last_seen) VALUES (?1, ?2, ?3, ?3)
         ON CONFLICT(uuid) DO UPDATE SET name = ?2, last_seen = ?3",
        params![uuid, name, now_secs()],
    );
}

fn name_of(db: &Connection, uuid: &str) -> String {
    db.query_row("SELECT name FROM players WHERE uuid = ?1", [uuid], |r| {
        r.get(0)
    })
    .unwrap_or_else(|_| uuid.to_owned())
}

/// The player's friends, with where they are, and their open requests.
fn snapshot(app: &App, me: &str) -> rusqlite::Result<Value> {
    let db = app.store.db();
    let mut friends: Vec<(String, String, i64)> = db
        .prepare_cached(
            "SELECT f.b, COALESCE(p.name, f.b), COALESCE(p.last_seen, 0)
             FROM friends f LEFT JOIN players p ON p.uuid = f.b WHERE f.a = ?1",
        )?
        .query_map([me], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    friends.sort_by_key(|(_, name, _)| name.to_lowercase());
    let list = |sql: &str| -> rusqlite::Result<Vec<Value>> {
        db.prepare_cached(sql)?
            .query_map([me], |r| r.get::<_, String>(0))?
            .map(|u| u.map(|u| json!({ "uuid": u, "name": name_of(&db, &u) })))
            .collect()
    };
    let incoming = list("SELECT sender FROM requests WHERE receiver = ?1 ORDER BY at")?;
    let outgoing = list("SELECT receiver FROM requests WHERE sender = ?1 ORDER BY at")?;
    let live = app.live();
    let friends: Vec<Value> = friends
        .into_iter()
        .map(|(uuid, name, last_seen)| {
            let fresh = |p: &&Presence| p.at.elapsed() < PRESENCE;
            let here = live
                .in_game
                .get(&uuid)
                .filter(fresh)
                .or_else(|| live.presence.get(&uuid).filter(fresh));
            json!({
                "uuid": uuid,
                "name": name,
                "online": here.is_some(),
                "status": here.map(|p| p.status.clone()),
                "server": here.and_then(|p| p.server.clone()),
                "last_seen": last_seen,
            })
        })
        .collect();
    Ok(json!({ "friends": friends, "incoming": incoming, "outgoing": outgoing }))
}

fn answer(result: rusqlite::Result<Value>) -> Response {
    match result {
        Ok(v) => Json(v).into_response(),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub(crate) struct Where {
    status: String,
    server: Option<String>,
    #[serde(default)]
    from: Option<String>,
}

pub(crate) async fn presence(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<Where>,
) -> Response {
    let (me, name) = match app.session(&headers) {
        Ok(s) => s,
        Err(r) => return r,
    };
    if !matches!(body.status.as_str(), "launcher" | "playing") {
        return refuse(StatusCode::BAD_REQUEST, "status is launcher or playing");
    }
    let server = body
        .server
        .map(|s| s.trim().chars().take(120).collect::<String>())
        .filter(|s| !s.is_empty() && s.chars().all(|c| !c.is_control()));
    let here = Presence {
        status: body.status,
        server,
        at: Instant::now(),
    };
    let mut live = app.live();
    if body.from.as_deref() == Some("game") {
        live.in_game.insert(me.clone(), here);
    } else {
        live.presence.insert(me.clone(), here);
    }
    drop(live);
    remember(&app, &me, &name);
    answer(snapshot(&app, &me))
}

pub(crate) async fn list(State(app): State<App>, headers: HeaderMap) -> Response {
    match app.session(&headers) {
        Ok((me, _)) => answer(snapshot(&app, &me)),
        Err(r) => r,
    }
}

#[derive(Deserialize)]
pub(crate) struct Name {
    name: String,
}

pub(crate) async fn request(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<Name>,
) -> Response {
    let (me, _) = match app.session(&headers) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let wanted = body.name.trim();
    if wanted.is_empty() || wanted.len() > 16 {
        return refuse(StatusCode::BAD_REQUEST, "not a Minecraft name");
    }
    let result = (|| -> rusqlite::Result<Result<(), (StatusCode, &'static str)>> {
        let mut db = app.store.db();
        let other: Option<String> = db
            .query_row(
                "SELECT uuid FROM players WHERE name = ?1 COLLATE NOCASE ORDER BY last_seen DESC LIMIT 1",
                [wanted],
                |r| r.get(0),
            )
            .optional()?;
        let Some(other) = other else {
            return Ok(Err((
                StatusCode::NOT_FOUND,
                "That player hasn't used Wisp yet.",
            )));
        };
        if other == me {
            return Ok(Err((StatusCode::BAD_REQUEST, "That's you.")));
        }
        let friends: bool = db
            .query_row(
                "SELECT 1 FROM friends WHERE a = ?1 AND b = ?2",
                params![me, other],
                |_| Ok(true),
            )
            .optional()?
            .is_some();
        if friends {
            return Ok(Err((StatusCode::CONFLICT, "You're already friends.")));
        }
        let count: i64 = db.query_row(
            "SELECT (SELECT COUNT(*) FROM friends WHERE a = ?1) + (SELECT COUNT(*) FROM requests WHERE sender = ?1)",
            [&me],
            |r| r.get(0),
        )?;
        if count as usize >= MAX_FRIENDS {
            return Ok(Err((
                StatusCode::BAD_REQUEST,
                "You have too many friends and requests.",
            )));
        }
        let tx = db.transaction()?;
        let asked_me = tx.execute(
            "DELETE FROM requests WHERE sender = ?1 AND receiver = ?2",
            params![other, me],
        )? > 0;
        if asked_me {
            befriend(&tx, &me, &other)?;
        } else {
            tx.execute(
                "INSERT OR IGNORE INTO requests (sender, receiver, at) VALUES (?1, ?2, ?3)",
                params![me, other, now_secs()],
            )?;
        }
        tx.commit()?;
        Ok(Ok(()))
    })();
    match result {
        Ok(Ok(())) => answer(snapshot(&app, &me)),
        Ok(Err((status, message))) => refuse(status, message),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

fn befriend(tx: &rusqlite::Transaction, a: &str, b: &str) -> rusqlite::Result<()> {
    let now = now_secs();
    tx.execute(
        "INSERT OR IGNORE INTO friends (a, b, since) VALUES (?1, ?2, ?3)",
        params![a, b, now],
    )?;
    tx.execute(
        "INSERT OR IGNORE INTO friends (a, b, since) VALUES (?1, ?2, ?3)",
        params![b, a, now],
    )?;
    Ok(())
}

#[derive(Deserialize)]
pub(crate) struct Who {
    uuid: String,
}

pub(crate) async fn accept(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<Who>,
) -> Response {
    let (me, _) = match app.session(&headers) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let Some(other) = uuid(&body.uuid) else {
        return refuse(StatusCode::BAD_REQUEST, "not a UUID");
    };
    let result = (|| -> rusqlite::Result<bool> {
        let mut db = app.store.db();
        let tx = db.transaction()?;
        let asked = tx.execute(
            "DELETE FROM requests WHERE sender = ?1 AND receiver = ?2",
            params![other, me],
        )? > 0;
        if asked {
            befriend(&tx, &me, &other)?;
        }
        tx.commit()?;
        Ok(asked)
    })();
    match result {
        Ok(true) => answer(snapshot(&app, &me)),
        Ok(false) => refuse(StatusCode::NOT_FOUND, "No request from that player."),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub(crate) async fn decline(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<Who>,
) -> Response {
    let (me, _) = match app.session(&headers) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let Some(other) = uuid(&body.uuid) else {
        return refuse(StatusCode::BAD_REQUEST, "not a UUID");
    };
    let result = app.store.db().execute(
        "DELETE FROM requests WHERE sender = ?1 AND receiver = ?2",
        params![other, me],
    );
    match result {
        Ok(_) => answer(snapshot(&app, &me)),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub(crate) async fn remove(
    State(app): State<App>,
    headers: HeaderMap,
    Path(other): Path<String>,
) -> Response {
    let (me, _) = match app.session(&headers) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let Some(other) = uuid(&other) else {
        return refuse(StatusCode::BAD_REQUEST, "not a UUID");
    };
    let result = (|| -> rusqlite::Result<()> {
        let mut db = app.store.db();
        let tx = db.transaction()?;
        tx.execute(
            "DELETE FROM friends WHERE (a = ?1 AND b = ?2) OR (a = ?2 AND b = ?1)",
            params![me, other],
        )?;
        tx.execute(
            "DELETE FROM requests WHERE sender = ?1 AND receiver = ?2",
            params![me, other],
        )?;
        tx.commit()
    })();
    match result {
        Ok(()) => answer(snapshot(&app, &me)),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
