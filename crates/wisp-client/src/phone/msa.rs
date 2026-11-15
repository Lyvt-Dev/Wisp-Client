//! Microsoft sign-in on the phone: the device-code flow, then Xbox Live, XSTS, the Minecraft
//! services, the ownership check and the profile. The launcher's flow (`launcher/src/auth.rs`)
//! with the same public app registration, without its desktop key stores: the phone keeps the
//! refresh token in the app's private folder, which no other app can read.
//!
//! The game itself never sees the refresh token: it gets the session file's JSON
//! ([`wisp_client::auth::AuthSession::parse`]), and [`renew_in_background`] answers its renewal
//! requests the way the launcher does on a PC.
use serde_json::{Value, json};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SCOPE: &str = "XboxLive.signin offline_access";
/// Wisp's public app registration (the launcher's); not a credential.
const CLIENT_ID: &str = "7a968ec3-4976-49b6-980f-4dcb76b3a95e";
const MICROSOFT: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const XBOX_USER: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MINECRAFT: &str = "https://api.minecraftservices.com";
/// Renew the Minecraft token when it has less than this left.
const RENEW_BEFORE: u64 = 3600;
/// The account file in the app's private folder.
const ACCOUNT_FILE: &str = "account.json";

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .redirects(0)
        .user_agent(concat!("Wisp-Android/", env!("CARGO_PKG_VERSION")))
        .build()
}

enum Body<'a> {
    None,
    Form(&'a [(&'a str, &'a str)]),
    Json(Value),
}

/// One request: its status and JSON body. Transport errors name the service only.
fn send(request: ureq::Request, body: Body, service: &str) -> Result<(u16, Value), String> {
    let result = match body {
        Body::None => request.call(),
        Body::Form(fields) => request.send_form(fields),
        Body::Json(value) => request
            .set("Content-Type", "application/json")
            .set("Accept", "application/json")
            .send_string(&value.to_string()),
    };
    let response = match result {
        Ok(r) | Err(ureq::Error::Status(_, r)) => r,
        Err(ureq::Error::Transport(_)) => {
            return Err(format!(
                "Could not reach {service}. Check your internet connection."
            ));
        }
    };
    let status = response.status();
    let mut text = String::new();
    response
        .into_reader()
        .take(1024 * 1024)
        .read_to_string(&mut text)
        .map_err(|_| format!("Could not read the response from {service}."))?;
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

fn field(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// What the player types at the verification page.
#[derive(Clone)]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    interval: u64,
    expires_at: u64,
    device_code: String,
}

impl std::fmt::Debug for DeviceCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceCode")
            .field("user_code", &self.user_code)
            .finish_non_exhaustive()
    }
}

fn oauth_error(v: &Value) -> String {
    match v.get("error").and_then(Value::as_str) {
        Some("authorization_declined") => "Sign-in was declined in the browser.".into(),
        Some("expired_token") => "The sign-in code expired. Start again.".into(),
        Some("invalid_grant") => "Your Microsoft sign-in expired. Sign in again.".into(),
        Some(_) => "Microsoft sign-in failed. Try again.".into(),
        None => "Microsoft sign-in failed.".into(),
    }
}

/// Starts a sign-in: the code to type at microsoft.com/link.
pub fn device_code() -> Result<DeviceCode, String> {
    let (status, v) = send(
        agent().post(&format!("{MICROSOFT}/devicecode")),
        Body::Form(&[("client_id", CLIENT_ID), ("scope", SCOPE)]),
        "Microsoft",
    )?;
    match (status, field(&v, "user_code"), field(&v, "device_code")) {
        (200, Some(user_code), Some(device_code)) => Ok(DeviceCode {
            user_code,
            verification_uri: field(&v, "verification_uri")
                .unwrap_or_else(|| "https://www.microsoft.com/link".into()),
            interval: v
                .get("interval")
                .and_then(Value::as_u64)
                .unwrap_or(5)
                .max(1),
            expires_at: now()
                + v.get("expires_in")
                    .and_then(Value::as_u64)
                    .unwrap_or(900)
                    .min(3600),
            device_code,
        }),
        _ => Err(oauth_error(&v)),
    }
}

/// The Microsoft tokens: the access token only for the Xbox step.
struct MsTokens {
    access: String,
    refresh: String,
}

fn ms_tokens(status: u16, v: &Value) -> Result<MsTokens, String> {
    match (status, field(v, "access_token"), field(v, "refresh_token")) {
        (200, Some(access), Some(refresh)) => Ok(MsTokens { access, refresh }),
        _ => Err(oauth_error(v)),
    }
}

/// The saved sign-in: the profile, the Minecraft token and the refresh token.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Account {
    pub name: String,
    /// Without dashes.
    pub uuid: String,
    xuid: Option<String>,
    minecraft_token: String,
    minecraft_expires_at: u64,
    refresh_token: String,
}

impl std::fmt::Debug for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Account")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// Waits for the player to approve in the browser, then finishes the sign-in. `cancelled` is
/// asked every 100 ms.
pub fn wait_for_sign_in(
    code: &DeviceCode,
    cancelled: &dyn Fn() -> bool,
    phase: &dyn Fn(&'static str),
) -> Result<Account, String> {
    let mut interval = code.interval;
    let tokens = loop {
        let until = std::time::Instant::now() + Duration::from_secs(interval);
        while std::time::Instant::now() < until {
            if cancelled() {
                return Err("Sign-in cancelled.".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if now() > code.expires_at {
            return Err("The sign-in code expired. Start again.".into());
        }
        let (status, v) = send(
            agent().post(&format!("{MICROSOFT}/token")),
            Body::Form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", CLIENT_ID),
                ("device_code", &code.device_code),
            ]),
            "Microsoft",
        )?;
        match v.get("error").and_then(Value::as_str) {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                interval += 5;
                continue;
            }
            _ => break ms_tokens(status, &v)?,
        }
    };
    finish(tokens, phase)
}

fn xsts_error(code: u64) -> String {
    match code {
        2148916233 => "This Microsoft account has no Xbox profile yet. Sign in once at xbox.com, then try again.".into(),
        2148916235 => "Xbox Live is not available in your country or region.".into(),
        2148916236 | 2148916237 => "This account needs adult verification on xbox.com first.".into(),
        2148916238 => "This is a child account: an adult has to add it to a Microsoft family first.".into(),
        other => format!("Xbox Live refused the sign-in (error {other})."),
    }
}

/// Xbox Live, XSTS, the Minecraft token, ownership and profile.
fn finish(ms: MsTokens, phase: &dyn Fn(&'static str)) -> Result<Account, String> {
    phase("Signing in to Xbox Live…");
    let (status, xbl) = send(
        agent().post(XBOX_USER),
        Body::Json(json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", ms.access),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        })),
        "Xbox Live",
    )?;
    let xbl_token = field(&xbl, "Token")
        .filter(|_| status == 200)
        .ok_or("Xbox Live refused the sign-in.")?;
    let (status, xsts) = send(
        agent().post(XSTS),
        Body::Json(json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        })),
        "Xbox Live",
    )?;
    if let Some(code) = xsts.get("XErr").and_then(Value::as_u64) {
        return Err(xsts_error(code));
    }
    let claims = xsts.pointer("/DisplayClaims/xui/0");
    let (Some(xsts_token), Some(uhs)) = (
        field(&xsts, "Token").filter(|_| status == 200),
        claims.and_then(|c| field(c, "uhs")),
    ) else {
        return Err("Xbox Live refused the sign-in.".into());
    };
    let xuid = claims.and_then(|c| field(c, "xid"));
    phase("Getting the Minecraft session…");
    let xtoken = format!("XBL3.0 x={uhs};{xsts_token}");
    let lifetime = |v: &Value| v.get("expires_in").and_then(Value::as_u64).unwrap_or(86400);
    let launcher = send(
        agent().post(&format!("{MINECRAFT}/launcher/login")),
        Body::Json(json!({ "xtoken": xtoken, "platform": "PC_LAUNCHER" })),
        "the Minecraft services",
    );
    let (token, expires_in) = match &launcher {
        Ok((200, v)) if field(v, "access_token").is_some() => {
            (field(v, "access_token").unwrap_or_default(), lifetime(v))
        }
        _ => {
            let (status, mc) = send(
                agent().post(&format!("{MINECRAFT}/authentication/login_with_xbox")),
                Body::Json(json!({ "identityToken": xtoken })),
                "the Minecraft services",
            )?;
            match (status, field(&mc, "access_token")) {
                (200, Some(token)) => (token, lifetime(&mc)),
                (429, _) => {
                    return Err(
                        "Too many sign-ins in a short time. Wait a minute and try again.".into(),
                    );
                }
                _ => return Err("The Minecraft services refused the sign-in.".into()),
            }
        }
    };
    phase("Checking Minecraft ownership…");
    let bearer = format!("Bearer {token}");
    let (status, owned) = send(
        agent()
            .get(&format!("{MINECRAFT}/entitlements/mcstore"))
            .set("Authorization", &bearer),
        Body::None,
        "the Minecraft services",
    )?;
    let owns = status == 200
        && owned
            .get("items")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items.iter().any(|i| {
                    matches!(
                        i.get("name").and_then(Value::as_str),
                        Some("game_minecraft" | "product_minecraft")
                    )
                })
            });
    if !owns {
        return Err("This account doesn't own Minecraft: Java Edition.".into());
    }
    phase("Loading the Minecraft profile…");
    let (status, profile) = send(
        agent()
            .get(&format!("{MINECRAFT}/minecraft/profile"))
            .set("Authorization", &bearer),
        Body::None,
        "the Minecraft services",
    )?;
    let (Some(uuid), Some(name)) = (field(&profile, "id"), field(&profile, "name")) else {
        return Err(if status == 404 {
            "This account has no Minecraft profile yet. Create one at minecraft.net.".into()
        } else {
            "Could not read the Minecraft profile.".into()
        });
    };
    let uuid = uuid.replace('-', "").to_lowercase();
    if uuid.len() != 32 || !uuid.bytes().all(|b| b.is_ascii_hexdigit()) || name.len() > 16 {
        return Err("The Minecraft profile is not valid.".into());
    }
    Ok(Account {
        name,
        uuid,
        xuid,
        minecraft_token: token,
        minecraft_expires_at: now() + expires_in,
        refresh_token: ms.refresh,
    })
}

impl Account {
    /// The session file's JSON the game reads ([`wisp_client::auth::AuthSession::parse`]).
    pub fn session_json(&self) -> String {
        json!({
            "name": self.name,
            "uuid": self.uuid,
            "access_token": self.minecraft_token,
            "user_type": "msa",
            "xuid": self.xuid,
        })
        .to_string()
    }

    /// The Minecraft token is close to its end.
    pub fn needs_renewal(&self) -> bool {
        self.minecraft_expires_at <= now() + RENEW_BEFORE
    }

    /// A fresh Minecraft token from the refresh token. The account must stay the same profile.
    pub fn renew(&self) -> Result<Account, String> {
        let (status, v) = send(
            agent().post(&format!("{MICROSOFT}/token")),
            Body::Form(&[
                ("grant_type", "refresh_token"),
                ("client_id", CLIENT_ID),
                ("refresh_token", &self.refresh_token),
                ("scope", SCOPE),
            ]),
            "Microsoft",
        )?;
        let fresh = finish(ms_tokens(status, &v)?, &|_| {})?;
        if fresh.uuid != self.uuid {
            return Err("The Microsoft account now has another Minecraft profile.".into());
        }
        Ok(fresh)
    }

    /// The account saved in `private` (the app's own folder), if any.
    pub fn load(private: &Path) -> Option<Account> {
        serde_json::from_slice(&std::fs::read(private.join(ACCOUNT_FILE)).ok()?).ok()
    }

    /// Saves to `private`, replacing the file in one step.
    pub fn save(&self, private: &Path) -> Result<(), String> {
        let text = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        let part = private.join(format!("{ACCOUNT_FILE}.part"));
        std::fs::create_dir_all(private)
            .and_then(|()| std::fs::write(&part, text))
            .and_then(|()| std::fs::rename(&part, private.join(ACCOUNT_FILE)))
            .map_err(|e| format!("Could not save the sign-in: {e}"))
    }

    /// Signs out: forgets the saved account.
    pub fn forget(private: &Path) {
        let _ = std::fs::remove_file(private.join(ACCOUNT_FILE));
    }
}

/// Answers the game's renewal requests (`auth::renewal`) from the saved account, as the launcher
/// does on a PC: a `request` file in `dir` gets a fresh `session.json`.
pub fn renew_in_background(private: PathBuf, dir: PathBuf) {
    let _ = std::thread::Builder::new()
        .name("phone-session-renewal".into())
        .spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(5));
                let request = dir.join(wisp_client::auth::renewal::REQUEST);
                if !request.exists() {
                    continue;
                }
                let Some(account) = Account::load(&private) else {
                    continue;
                };
                let fresh = if account.needs_renewal() {
                    match account.renew() {
                        Ok(fresh) => {
                            let _ = fresh.save(&private);
                            fresh
                        }
                        Err(e) => {
                            log::warn!("Wisp: renewing the Minecraft session failed: {e}");
                            // Ask again on the game's next request.
                            let _ = std::fs::remove_file(&request);
                            continue;
                        }
                    }
                } else {
                    account
                };
                let session = dir.join(wisp_client::auth::renewal::SESSION);
                let part = session.with_extension("part");
                if std::fs::write(&part, fresh.session_json())
                    .and_then(|()| std::fs::rename(&part, &session))
                    .is_ok()
                {
                    let _ = std::fs::remove_file(&request);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> Account {
        Account {
            name: "Steve".into(),
            uuid: "0123456789abcdef0123456789abcdef".into(),
            xuid: Some("42".into()),
            minecraft_token: "token".into(),
            minecraft_expires_at: now() + 86_400,
            refresh_token: "refresh".into(),
        }
    }

    #[test]
    fn the_game_reads_the_session() {
        let session = wisp_client::auth::AuthSession::parse(&account().session_json()).unwrap();
        assert_eq!(session.name, "Steve");
        assert_eq!(session.uuid, 0x0123456789abcdef0123456789abcdef);
        assert_eq!(session.xuid.as_deref(), Some("42"));
    }

    #[test]
    fn the_account_survives_a_restart() {
        let dir = std::env::temp_dir().join(format!("wisp-phone-msa-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        account().save(&dir).unwrap();
        let loaded = Account::load(&dir).unwrap();
        assert_eq!(loaded.name, "Steve");
        assert!(!loaded.needs_renewal());
        Account::forget(&dir);
        assert!(Account::load(&dir).is_none());
    }

    #[test]
    fn tokens_never_reach_debug_output() {
        let shown = format!("{:?}", account());
        assert!(
            !shown.contains("token") && !shown.contains("refresh"),
            "{shown}"
        );
    }
}
