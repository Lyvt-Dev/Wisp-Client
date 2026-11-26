//! The Shop's wallets: every player's Wisp Coins, owned and equipped items are kept here, so
//! no game or launcher can give itself coins. The rules and prices are `wisp_shop`'s, the same
//! the launcher and the game show.
//!
//! With a session:
//! - `GET /v1/wallet`: `{"coins", "owned", "equipped", "collected", "discount", "supporting"}`.
//!   `collected`: today's daily bonus is given; `discount`: percent off with the supported
//!   creator's code (0 without one).
//! - `POST /v1/wallet/collect`: the welcome gift (once) and today's bonus (once a day):
//!   `{"given", "wallet"}`.
//! - `POST /v1/shop/buy` `{"item"}`: buys it at its price less the discount; the supported
//!   creator earns their share of the full price straight into their wallet. `{"paid",
//!   "wallet"}`.
//! - `POST /v1/shop/equip` / `POST /v1/shop/unequip` `{"item"}`: `{"wallet"}`.
//!
//! A refusal says why in `error` (the Shop's English, `wisp_shop::Refused::message`) and gives
//! the coins missing in `short` when there aren't enough.
use super::{App, creators, now_secs, refuse};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Deserialize;
use serde_json::{Value, json};
use wisp_shop::{Refused, Wallet};

pub(crate) fn tables(db: &Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS wallets (
             uuid TEXT PRIMARY KEY, coins INTEGER NOT NULL DEFAULT 0,
             welcomed INTEGER NOT NULL DEFAULT 0, bonus_day INTEGER);
         CREATE TABLE IF NOT EXISTS owned (
             uuid TEXT NOT NULL, item TEXT NOT NULL, at INTEGER NOT NULL,
             PRIMARY KEY (uuid, item));
         CREATE TABLE IF NOT EXISTS equipped (
             uuid TEXT NOT NULL, item TEXT NOT NULL, PRIMARY KEY (uuid, item));",
    )
}

/// The player's wallet as kept (a new one when they have none yet).
pub fn load(db: &Connection, player: &str) -> rusqlite::Result<Wallet> {
    let (coins, welcomed, bonus_day): (i64, bool, Option<i64>) = db
        .query_row(
            "SELECT coins, welcomed, bonus_day FROM wallets WHERE uuid = ?1",
            [player],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
        .unwrap_or((0, false, None));
    let items = |table: &str| -> rusqlite::Result<Vec<String>> {
        db.prepare_cached(&format!(
            "SELECT item FROM {table} WHERE uuid = ?1 ORDER BY rowid"
        ))?
        .query_map([player], |r| r.get(0))?
        .collect()
    };
    Ok(Wallet {
        coins: coins.clamp(0, i64::from(u32::MAX)) as u32,
        owned: items("owned")?,
        equipped: items("equipped")?,
        bonus_day: bonus_day.map(|d| d.max(0) as u64),
        welcomed,
    })
}

/// Keeps `wallet` as the player's.
pub fn save(tx: &Transaction, player: &str, wallet: &Wallet) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO wallets (uuid, coins, welcomed, bonus_day) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(uuid) DO UPDATE SET coins = ?2, welcomed = ?3, bonus_day = ?4",
        params![
            player,
            wallet.coins,
            wallet.welcomed,
            wallet.bonus_day.map(|d| d as i64)
        ],
    )?;
    let now = now_secs();
    for item in &wallet.owned {
        tx.execute(
            "INSERT OR IGNORE INTO owned (uuid, item, at) VALUES (?1, ?2, ?3)",
            params![player, item, now],
        )?;
    }
    tx.execute("DELETE FROM equipped WHERE uuid = ?1", [player])?;
    for item in &wallet.equipped {
        tx.execute(
            "INSERT INTO equipped (uuid, item) VALUES (?1, ?2)",
            params![player, item],
        )?;
    }
    Ok(())
}

/// Gives `coins` to `player` (a creator's share).
pub fn credit(tx: &Transaction, player: &str, coins: u32) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO wallets (uuid, coins) VALUES (?1, ?2)
         ON CONFLICT(uuid) DO UPDATE SET coins = coins + ?2",
        params![player, coins],
    )?;
    Ok(())
}

/// The creator `player` supports, if any: their UUID, name and code.
fn supported(db: &Connection, player: &str) -> Option<(String, String, Option<String>)> {
    db.query_row(
        "SELECT c.uuid, c.name, c.code FROM supporters s JOIN creators c ON c.uuid = s.creator
         WHERE s.player = ?1",
        [player],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()
    .ok()
    .flatten()
}

/// The wallet as the launcher and the game read it.
fn shown(db: &Connection, player: &str, wallet: &Wallet) -> Value {
    let supporting = supported(db, player);
    json!({
        "coins": wallet.coins,
        "owned": wallet.owned,
        "equipped": wallet.equipped,
        "collected": wallet.bonus_day == Some(wisp_shop::today()),
        "discount": if supporting.is_some() { creators::DISCOUNT } else { 0 },
        "supporting": supporting.map(|(_, name, code)| json!({ "name": name, "code": code })),
    })
}

fn refused(refused: &Refused) -> Response {
    let short = match refused {
        Refused::Short(n) => Some(*n),
        _ => None,
    };
    let status = match refused {
        Refused::Unknown => StatusCode::NOT_FOUND,
        Refused::Short(_) => StatusCode::PAYMENT_REQUIRED,
        _ => StatusCode::CONFLICT,
    };
    (
        status,
        Json(json!({ "error": refused.to_string(), "short": short })),
    )
        .into_response()
}

pub(crate) async fn wallet(State(app): State<App>, headers: HeaderMap) -> Response {
    let (player, _) = match app.session(&headers) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let db = app.store.db();
    match load(&db, &player) {
        Ok(wallet) => Json(shown(&db, &player, &wallet)).into_response(),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub(crate) async fn collect(State(app): State<App>, headers: HeaderMap) -> Response {
    let (player, _) = match app.session(&headers) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let result = (|| -> rusqlite::Result<(u32, Value)> {
        let mut db = app.store.db();
        let mut wallet = load(&db, &player)?;
        let given = wallet.collect(wisp_shop::today());
        let tx = db.transaction()?;
        save(&tx, &player, &wallet)?;
        tx.commit()?;
        Ok((given, shown(&db, &player, &wallet)))
    })();
    match result {
        Ok((given, wallet)) => Json(json!({ "given": given, "wallet": wallet })).into_response(),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub(crate) struct Item {
    item: String,
}

pub(crate) async fn buy(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<Item>,
) -> Response {
    let (player, _) = match app.session(&headers) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let result = (|| -> rusqlite::Result<Result<(u32, Value), Refused>> {
        let mut db = app.store.db();
        let mut wallet = load(&db, &player)?;
        let creator = supported(&db, &player).map(|(uuid, _, _)| uuid);
        let discount = if creator.is_some() {
            creators::DISCOUNT
        } else {
            0
        };
        let paid = match wallet.buy_discounted(&body.item, discount) {
            Ok(paid) => paid,
            Err(r) => return Ok(Err(r)),
        };
        let full = wisp_shop::offer(&body.item).map_or(0, |o| o.price);
        let share = match &creator {
            Some(_) if full > 0 => wisp_shop::share(full, creators::DISCOUNT),
            _ => 0,
        };
        let tx = db.transaction()?;
        save(&tx, &player, &wallet)?;
        if let Some(creator) = &creator
            && share > 0
        {
            credit(&tx, creator, share)?;
            creators::record(&tx, &player, &body.item, full, creator, share)?;
        }
        tx.commit()?;
        Ok(Ok((paid, shown(&db, &player, &wallet))))
    })();
    match result {
        Ok(Ok((paid, wallet))) => Json(json!({ "paid": paid, "wallet": wallet })).into_response(),
        Ok(Err(r)) => refused(&r),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

fn change(
    app: &App,
    player: &str,
    act: impl FnOnce(&mut Wallet) -> Result<(), Refused>,
) -> Response {
    let result = (|| -> rusqlite::Result<Result<Value, Refused>> {
        let mut db = app.store.db();
        let mut wallet = load(&db, player)?;
        if let Err(r) = act(&mut wallet) {
            return Ok(Err(r));
        }
        let tx = db.transaction()?;
        save(&tx, player, &wallet)?;
        tx.commit()?;
        Ok(Ok(shown(&db, player, &wallet)))
    })();
    match result {
        Ok(Ok(wallet)) => Json(json!({ "wallet": wallet })).into_response(),
        Ok(Err(r)) => refused(&r),
        Err(e) => refuse(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub(crate) async fn equip(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<Item>,
) -> Response {
    match app.session(&headers) {
        Ok((player, _)) => change(&app, &player, |w| w.equip(&body.item)),
        Err(r) => r,
    }
}

pub(crate) async fn unequip(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<Item>,
) -> Response {
    match app.session(&headers) {
        Ok((player, _)) => change(&app, &player, |w| {
            w.unequip(&body.item);
            Ok(())
        }),
        Err(r) => r,
    }
}
