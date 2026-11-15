use super::*;
use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use tower::ServiceExt;

const PLAYER: &str = "0123456789abcdef0123456789abcdef";

const NOTCH: &str = "11111111111111111111111111111111";
const JEB: &str = "22222222222222222222222222222222";

/// Mojang confirms "Steve", "Notch" and "Jeb" joined, and nobody else.
struct FakeMojang;

impl Mojang for FakeMojang {
    fn has_joined(&self, name: &str, _: &str) -> Result<Option<String>, String> {
        Ok(match name {
            "Steve" => Some(PLAYER.to_owned()),
            "Notch" => Some(NOTCH.to_owned()),
            "Jeb" => Some(JEB.to_owned()),
            _ => None,
        })
    }
}

fn app() -> App {
    App::new(Store::memory(), FakeMojang, Vec::new())
}

async fn call(
    app: &App,
    method: &str,
    path: &str,
    session: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(s) = session {
        request = request.header("authorization", format!("Bearer {s}"));
    }
    let response = router(app.clone())
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn sign_in(app: &App, name: &str) -> (StatusCode, Value) {
    let (_, start) = call(app, "POST", "/v1/auth/start", None, json!({})).await;
    let id = start["server_id"].as_str().unwrap();
    call(
        app,
        "POST",
        "/v1/auth/finish",
        None,
        json!({ "server_id": id, "name": name }),
    )
    .await
}

/// A small valid hat: one 2×2×2 box on a 16×16 texture.
fn hat(id: &str) -> Value {
    let png = {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, 16, 16);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[200; 16 * 16 * 4])
            .unwrap();
        out
    };
    use base64::Engine;
    json!({
        "format": 1, "id": id, "slot": "head", "bone": "head",
        "texture": { "png": base64::engine::general_purpose::STANDARD.encode(png) },
        "parts": [{ "name": "box", "cubes": [{ "origin": [-1, -10, -1], "size": [2, 2, 2], "uv": [0, 0] }] }],
    })
}

#[tokio::test]
async fn only_players_mojang_confirms_get_a_session() {
    let app = app();
    let (status, body) = sign_in(&app, "Steve").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["uuid"], PLAYER);
    assert_eq!(sign_in(&app, "Alex").await.0, StatusCode::UNAUTHORIZED);
    // A server id is good for one sign-in only.
    let (status, _) = call(
        &app,
        "POST",
        "/v1/auth/finish",
        None,
        json!({ "server_id": "nope", "name": "Steve" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn what_a_player_wears_reaches_everyone_who_asks() {
    let app = app();
    let (_, s) = sign_in(&app, "Steve").await;
    let session = s["session"].as_str().unwrap();
    let (status, body) = call(
        &app,
        "PUT",
        "/v1/wear",
        Some(session),
        json!({ "items": [hat("top_hat")] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let hash = body["worn"][0]["hash"].as_str().unwrap().to_owned();

    // Anyone (no session) asks what the players in view wear, dashes or not.
    let dashed = "01234567-89ab-cdef-0123-456789abcdef";
    let (_, worn) = call(
        &app,
        "POST",
        "/v1/worn",
        None,
        json!({ "players": [dashed, "ffffffffffffffffffffffffffffffff"] }),
    )
    .await;
    assert_eq!(worn["players"][PLAYER][0]["id"], "top_hat");
    assert!(
        worn["players"]
            .get("ffffffffffffffffffffffffffffffff")
            .is_none()
    );
    assert_eq!(worn["ttl"], WORN_TTL);

    // The item file comes back exactly as the game can read it.
    let response = router(app.clone())
        .oneshot(
            Request::get(format!("/v1/items/top_hat?hash={hash}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(wisp_cosmetics::Item::parse(&bytes).unwrap().id, "top_hat");

    // Hidden, the player is left out.
    call(
        &app,
        "PUT",
        "/v1/visible",
        Some(session),
        json!({ "visible": false }),
    )
    .await;
    let (_, worn) = call(
        &app,
        "POST",
        "/v1/worn",
        None,
        json!({ "players": [PLAYER] }),
    )
    .await;
    assert!(worn["players"].as_object().unwrap().is_empty());
}

#[tokio::test]
async fn bad_items_and_strangers_are_refused() {
    let app = app();
    let (status, _) = call(&app, "PUT", "/v1/wear", None, json!({ "items": [] })).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (_, s) = sign_in(&app, "Steve").await;
    let session = s["session"].as_str().unwrap();
    let mut broken = hat("top_hat");
    broken["slot"] = json!("pocket");
    let (status, _) = call(
        &app,
        "PUT",
        "/v1/wear",
        Some(session),
        json!({ "items": [broken] }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &app,
        "PUT",
        "/v1/wear",
        Some(session),
        json!({ "items": [hat("a"), hat("b")] }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "two items in one slot");

    let limited = App::new(Store::memory(), FakeMojang, vec!["founders_cape".into()]);
    let (_, s) = sign_in(&limited, "Steve").await;
    let (status, _) = call(
        &limited,
        "PUT",
        "/v1/wear",
        s["session"].as_str(),
        json!({ "items": [hat("top_hat")] }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "not on the server's list");
}

#[tokio::test]
async fn an_emote_shows_while_it_plays() {
    let app = app();
    let (_, s) = sign_in(&app, "Steve").await;
    let session = s["session"].as_str().unwrap();
    let (status, _) = call(
        &app,
        "POST",
        "/v1/emote",
        Some(session),
        json!({ "id": "black_hole" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, playing) = call(
        &app,
        "POST",
        "/v1/emotes",
        None,
        json!({ "players": [PLAYER] }),
    )
    .await;
    assert_eq!(playing["players"][PLAYER]["id"], "black_hole");
    assert!(playing["players"][PLAYER]["elapsed"].as_f64().unwrap() < 5.0);
    call(&app, "DELETE", "/v1/emote", Some(session), json!({})).await;
    let (_, playing) = call(
        &app,
        "POST",
        "/v1/emotes",
        None,
        json!({ "players": [PLAYER] }),
    )
    .await;
    assert!(playing["players"].as_object().unwrap().is_empty());
    let (status, _) = call(
        &app,
        "POST",
        "/v1/emote",
        Some(session),
        json!({ "id": "../etc" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

async fn session(app: &App, name: &str) -> String {
    let (status, body) = sign_in(app, name).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["session"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn friends_ask_accept_see_each_other_and_remove() {
    let app = app();
    let steve = session(&app, "Steve").await;
    let notch = session(&app, "Notch").await;
    // Names are found whatever their case; strangers and oneself are refused.
    let (status, _) = call(
        &app,
        "POST",
        "/v1/friends/request",
        Some(&steve),
        json!({"name": "nobody"}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &app,
        "POST",
        "/v1/friends/request",
        Some(&steve),
        json!({"name": "steve"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, sent) = call(
        &app,
        "POST",
        "/v1/friends/request",
        Some(&steve),
        json!({"name": "notch"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(sent["outgoing"][0]["name"], "Notch");
    let (_, inbox) = call(&app, "GET", "/v1/friends", Some(&notch), Value::Null).await;
    assert_eq!(inbox["incoming"][0]["uuid"], PLAYER);
    assert_eq!(inbox["incoming"][0]["name"], "Steve");
    let (status, friends) = call(
        &app,
        "POST",
        "/v1/friends/accept",
        Some(&notch),
        json!({"uuid": PLAYER}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(friends["friends"][0]["name"], "Steve");
    assert_eq!(friends["incoming"].as_array().unwrap().len(), 0);
    // Steve is offline until his launcher says where he is.
    assert_eq!(friends["friends"][0]["online"], false);
    let (status, _) = call(
        &app,
        "POST",
        "/v1/presence",
        Some(&steve),
        json!({"status": "playing", "server": "zedix.de"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, seen) = call(
        &app,
        "POST",
        "/v1/presence",
        Some(&notch),
        json!({"status": "launcher"}),
    )
    .await;
    assert_eq!(seen["friends"][0]["online"], true);
    assert_eq!(seen["friends"][0]["status"], "playing");
    assert_eq!(seen["friends"][0]["server"], "zedix.de");
    // The game knows the server Steve is really on; it wins over the launcher's guess.
    call(
        &app,
        "POST",
        "/v1/presence",
        Some(&steve),
        json!({"status": "playing", "server": "hypixel.net", "from": "game"}),
    )
    .await;
    call(
        &app,
        "POST",
        "/v1/presence",
        Some(&steve),
        json!({"status": "playing", "server": "zedix.de"}),
    )
    .await;
    let (_, seen) = call(&app, "GET", "/v1/friends", Some(&notch), Value::Null).await;
    assert_eq!(seen["friends"][0]["server"], "hypixel.net");
    // Asking again is refused; removing works both ways.
    let (status, _) = call(
        &app,
        "POST",
        "/v1/friends/request",
        Some(&steve),
        json!({"name": "Notch"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, gone) = call(
        &app,
        "DELETE",
        &format!("/v1/friends/{NOTCH}"),
        Some(&steve),
        Value::Null,
    )
    .await;
    assert_eq!(gone["friends"].as_array().unwrap().len(), 0);
    let (_, theirs) = call(&app, "GET", "/v1/friends", Some(&notch), Value::Null).await;
    assert_eq!(theirs["friends"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn asking_someone_who_asked_you_makes_friends_and_decline_forgets() {
    let app = app();
    let steve = session(&app, "Steve").await;
    let notch = session(&app, "Notch").await;
    let jeb = session(&app, "Jeb").await;
    call(
        &app,
        "POST",
        "/v1/friends/request",
        Some(&steve),
        json!({"name": "Notch"}),
    )
    .await;
    let (_, both) = call(
        &app,
        "POST",
        "/v1/friends/request",
        Some(&notch),
        json!({"name": "Steve"}),
    )
    .await;
    assert_eq!(both["friends"][0]["uuid"], PLAYER);
    call(
        &app,
        "POST",
        "/v1/friends/request",
        Some(&jeb),
        json!({"name": "Steve"}),
    )
    .await;
    let (_, after) = call(
        &app,
        "POST",
        "/v1/friends/decline",
        Some(&steve),
        json!({"uuid": JEB}),
    )
    .await;
    assert_eq!(after["incoming"].as_array().unwrap().len(), 0);
    assert_eq!(after["friends"].as_array().unwrap().len(), 1);
    let (status, _) = call(&app, "GET", "/v1/friends", None, Value::Null).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = call(
        &app,
        "POST",
        "/v1/presence",
        Some(&steve),
        json!({"status": "afk"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// A 16×16 PNG of one colour, base64: a badge's mark.
fn mark_png() -> String {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, 16, 16);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[255; 16 * 16 * 4])
        .unwrap();
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(out)
}

/// Wisp's badge as the launcher writes it into every worn folder: a name tag.
fn wisp_badge() -> Value {
    json!({ "format": 1, "id": "wisp_badge", "parts": [], "slot": "name_tag",
            "tag": { "line": "#ffffff" }, "texture": { "png": mark_png() } })
}

#[tokio::test]
async fn creators_codes_supporters_shares_and_badges() {
    // Notch is the admin; Jeb becomes a creator; Steve supports him.
    let app = app().with_admins(&["11111111-1111-1111-1111-111111111111".to_owned()]);
    let steve = session(&app, "Steve").await;
    let notch = session(&app, "Notch").await;
    let jeb = session(&app, "Jeb").await;

    // Only an admin chooses creators, and only players who have used Wisp.
    let (status, _) = call(
        &app,
        "POST",
        "/v1/admin/creators",
        Some(&steve),
        json!({ "name": "Jeb" }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(
        &app,
        "POST",
        "/v1/admin/creators",
        Some(&notch),
        json!({ "name": "Herobrine" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = call(
        &app,
        "POST",
        "/v1/admin/creators",
        Some(&notch),
        json!({ "name": "jeb" }),
    )
    .await;
    assert_eq!((status, body["uuid"].as_str()), (StatusCode::OK, Some(JEB)));
    let (_, me) = call(&app, "GET", "/v1/creator/me", Some(&notch), json!({})).await;
    assert_eq!(
        (me["admin"].as_bool(), me["creator"].as_bool()),
        (Some(true), Some(false))
    );

    // A creator sets a code; nobody else can, and a code is checked and unique.
    let (status, _) = call(
        &app,
        "PUT",
        "/v1/creator/code",
        Some(&steve),
        json!({ "code": "STEVE" }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(
        &app,
        "PUT",
        "/v1/creator/code",
        Some(&jeb),
        json!({ "code": "no spaces" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, body) = call(
        &app,
        "PUT",
        "/v1/creator/code",
        Some(&jeb),
        json!({ "code": "jeb_" }),
    )
    .await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::OK, Some("JEB_"))
    );
    let (_, list) = call(&app, "GET", "/v1/creators", None, json!({})).await;
    assert_eq!(list["discount"], 10);
    assert_eq!(list["creators"], json!([{ "code": "JEB_", "name": "Jeb" }]));

    // Supporting: by any case, never yourself, never a code nobody has.
    let (status, _) = call(
        &app,
        "PUT",
        "/v1/supporting",
        Some(&jeb),
        json!({ "code": "JEB_" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &app,
        "PUT",
        "/v1/supporting",
        Some(&steve),
        json!({ "code": "NOPE" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = call(
        &app,
        "PUT",
        "/v1/supporting",
        Some(&steve),
        json!({ "code": "Jeb_" }),
    )
    .await;
    assert_eq!(
        (status, &body["supporting"]["name"]),
        (StatusCode::OK, &json!("Jeb"))
    );

    // Coins live here: the welcome gift and the daily bonus, once.
    let (_, body) = call(&app, "POST", "/v1/wallet/collect", Some(&steve), json!({})).await;
    let start = wisp_shop::WELCOME + wisp_shop::DAILY;
    assert_eq!(
        (body["given"].as_u64(), body["wallet"]["coins"].as_u64()),
        (Some(start.into()), Some(start.into()))
    );
    let (_, body) = call(&app, "POST", "/v1/wallet/collect", Some(&steve), json!({})).await;
    assert_eq!(body["given"], 0);
    assert_eq!(body["wallet"]["discount"], 10);

    // Steve buys with the 10 percent off; Jeb earns 10 percent of the full price in his wallet.
    let cape = wisp_shop::OFFERS
        .iter()
        .find(|o| o.price > start && wisp_shop::discounted(o.price, 10) <= start)
        .expect("an item Steve can afford only with the code");
    let (status, body) = call(
        &app,
        "PUT",
        "/v1/supporting",
        Some(&steve),
        json!({ "code": null }),
    )
    .await;
    assert_eq!(
        (status, &body["supporting"]),
        (StatusCode::OK, &Value::Null)
    );
    let (status, body) = call(
        &app,
        "POST",
        "/v1/shop/buy",
        Some(&steve),
        json!({ "item": cape.id }),
    )
    .await;
    assert_eq!(
        (status, body["short"].as_u64()),
        (
            StatusCode::PAYMENT_REQUIRED,
            Some((cape.price - start).into())
        )
    );
    call(
        &app,
        "PUT",
        "/v1/supporting",
        Some(&steve),
        json!({ "code": "JEB_" }),
    )
    .await;
    let (status, body) = call(
        &app,
        "POST",
        "/v1/shop/buy",
        Some(&steve),
        json!({ "item": cape.id }),
    )
    .await;
    let paid = wisp_shop::discounted(cape.price, 10);
    assert_eq!(
        (status, body["paid"].as_u64()),
        (StatusCode::OK, Some(paid.into()))
    );
    assert_eq!(
        body["wallet"]["coins"].as_u64(),
        Some((start - paid).into())
    );
    let (status, _) = call(
        &app,
        "POST",
        "/v1/shop/buy",
        Some(&steve),
        json!({ "item": cape.id }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, wallet) = call(&app, "GET", "/v1/wallet", Some(&jeb), json!({})).await;
    assert_eq!(
        wallet["coins"].as_u64(),
        Some(wisp_shop::share(cape.price, 10).into())
    );
    let (_, me) = call(&app, "GET", "/v1/creator/me", Some(&jeb), json!({})).await;
    assert_eq!(
        (me["supporters"].as_i64(), me["purchases"].as_i64()),
        (Some(1), Some(1))
    );
    assert_eq!(
        me["earned"].as_u64(),
        Some(wisp_shop::share(cape.price, 10).into())
    );

    // Wearing: only what is owned, and nothing that isn't in the Shop.
    let (status, body) = call(
        &app,
        "POST",
        "/v1/shop/equip",
        Some(&steve),
        json!({ "item": cape.id }),
    )
    .await;
    assert_eq!(
        (status, &body["wallet"]["equipped"]),
        (StatusCode::OK, &json!([cape.id]))
    );
    let (status, _) = call(
        &app,
        "POST",
        "/v1/shop/equip",
        Some(&jeb),
        json!({ "item": cape.id }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = call(
        &app,
        "POST",
        "/v1/shop/equip",
        Some(&steve),
        json!({ "item": "free_coins" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, body) = call(
        &app,
        "POST",
        "/v1/shop/unequip",
        Some(&steve),
        json!({ "item": cape.id }),
    )
    .await;
    assert_eq!(body["wallet"]["equipped"], json!([]));

    // The admin gives Jeb a badge: Steve wears it in place of Wisp's, and so does Jeb.
    let (status, _) = call(
        &app,
        "PUT",
        "/v1/wear",
        Some(&steve),
        json!({ "items": [hat("hat"), wisp_badge()] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &app,
        "PUT",
        &format!("/v1/admin/creators/{JEB}/badge"),
        Some(&notch),
        json!({ "png": "not a png" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, badge) = call(
        &app,
        "PUT",
        &format!("/v1/admin/creators/{JEB}/badge"),
        Some(&notch),
        json!({ "png": mark_png(), "line": "#ff4d8d" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let hash = badge["badge"].as_str().unwrap().to_owned();
    let (_, worn) = call(
        &app,
        "POST",
        "/v1/worn",
        None,
        json!({ "players": [PLAYER] }),
    )
    .await;
    let ids: Vec<&str> = worn["players"][PLAYER]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["hat", "creator_badge"]);
    let (_, file) = call(
        &app,
        "GET",
        &format!("/v1/items/creator_badge?hash={hash}"),
        None,
        json!({}),
    )
    .await;
    assert_eq!(file["tag"]["line"], "#ff4d8d");
    let (_, worn) = call(&app, "PUT", "/v1/wear", Some(&jeb), json!({ "items": [] })).await;
    assert_eq!(worn["worn"], json!([]));
    let (_, worn) = call(&app, "POST", "/v1/worn", None, json!({ "players": [JEB] })).await;
    assert_eq!(worn["players"][JEB][0]["id"], "creator_badge");

    // A new code keeps Jeb's supporters.
    call(
        &app,
        "PUT",
        "/v1/creator/code",
        Some(&jeb),
        json!({ "code": "JEBBY" }),
    )
    .await;
    let (_, me) = call(&app, "GET", "/v1/creator/me", Some(&steve), json!({})).await;
    assert_eq!(me["supporting"]["code"], "JEBBY");

    // No longer a creator: Steve supports nobody and wears Wisp's badge again.
    let (_, body) = call(
        &app,
        "DELETE",
        &format!("/v1/admin/creators/{JEB}"),
        Some(&notch),
        json!({}),
    )
    .await;
    assert_eq!(body["removed"], true);
    let (_, me) = call(&app, "GET", "/v1/creator/me", Some(&steve), json!({})).await;
    assert!(me["supporting"].is_null());
    let (_, worn) = call(
        &app,
        "POST",
        "/v1/worn",
        None,
        json!({ "players": [PLAYER] }),
    )
    .await;
    let ids: Vec<&str> = worn["players"][PLAYER]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["hat", "wisp_badge"]);
}
