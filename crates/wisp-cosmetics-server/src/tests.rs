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
