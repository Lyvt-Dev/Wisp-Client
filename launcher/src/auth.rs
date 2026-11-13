// Copyright (c) 2026 Ludovic Marie and the Horizon contributors. MIT license; see LICENSE.
// Adapted from Lyvt-Dev/Wisp-Client at 2181ab22: isolated Wisp account flow and secure local storage.

use serde_json::{json, Value};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod browser;
mod storage;
#[cfg(all(windows, feature = "gui"))]
pub mod window;
pub use browser::{browser_sign_in, sign_in_window};
pub use storage::{accounts, for_game, remove, save, saved, sign_out, switch};

const SCOPE: &str = "XboxLive.signin offline_access";
// Wisp's owner-provided public app registration; this is not a credential.
const DEFAULT_CLIENT_ID: &str = "7a968ec3-4976-49b6-980f-4dcb76b3a95e";
pub const NOT_CONFIGURED: &str = "Microsoft sign-in is not configured for this Wisp build. See the Microsoft sign-in setup guide.";
/// What every Play says while no account in the launcher owns the game.
pub const NEEDS_OWNER: &str =
    "Sign in with a Microsoft account that owns Minecraft: Java Edition to play";
const DOES_NOT_OWN: &str = "This account doesn't own Minecraft: Java Edition.";
const NO_PROFILE: &str =
    "This account has no Minecraft profile yet. Create one at minecraft.net, then sign in again.";
const OTHER_PROFILE: &str =
    "The Microsoft account now has another Minecraft profile. Sign in again.";
const SIGN_IN_AGAIN: &str = "Your Microsoft sign-in expired. Sign in again.";
/// The Minecraft token is renewed when it has less than this left.
const RENEW_BEFORE: u64 = 3600;

/// Runtime configuration wins, then a nonempty build override, then Wisp's registration.
/// An explicitly empty runtime value disables sign-in; an unset CI variable uses the default.
pub fn client_id() -> Option<String> {
    wisp_compat::var("WISP_MSA_CLIENT_ID")
        .ok()
        .or_else(|| {
            option_env!("WISP_MSA_CLIENT_ID")
                .filter(|id| !id.trim().is_empty())
                .map(str::to_owned)
        })
        .or_else(|| Some(DEFAULT_CLIENT_ID.to_owned()))
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
}

/// Where the requests go: the real services, or a mock in tests.
#[derive(Clone, Debug)]
pub struct Endpoints {
    /// The OAuth 2.0 v2 endpoints for personal accounts (`…/devicecode`, `…/token`).
    pub microsoft: String,
    pub xbox_user: String,
    pub xsts: String,
    /// api.minecraftservices.com.
    pub minecraft: String,
}

impl Endpoints {
    pub fn live() -> Self {
        Self {
            microsoft: "https://login.microsoftonline.com/consumers/oauth2/v2.0".into(),
            xbox_user: "https://user.auth.xboxlive.com/user/authenticate".into(),
            xsts: "https://xsts.auth.xboxlive.com/xsts/authorize".into(),
            minecraft: "https://api.minecraftservices.com".into(),
        }
    }
}

/// Requests use Wisp's user agent, a 20 s timeout, and no automatic redirects.
fn request_to(method: &str, url: &str) -> ureq::Request {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .redirects(0)
        .user_agent(concat!("Wisp/", env!("CARGO_PKG_VERSION")))
        .build()
        .request(method, url)
}

/// One request's result: the status and the JSON body (empty when there is none). Transport
/// errors name the service, never the URL's query or the request.
fn send(request: ureq::Request, body: Option<Body>, service: &str) -> Result<(u16, Value), String> {
    let result = match body {
        None => request.call(),
        Some(Body::Form(fields)) => request.send_form(&fields),
        Some(Body::Json(value)) => request.set("Accept", "application/json").send_json(value),
    };
    let response = match result {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(ureq::Error::Transport(_)) => {
            return Err(format!(
                "Could not reach {service}. Check your internet connection."
            ));
        }
    };
    let status = response.status();
    let mut text = String::new();
    use std::io::Read;
    response
        .into_reader()
        .take(1024 * 1024)
        .read_to_string(&mut text)
        .map_err(|_| format!("Could not read the response from {service}."))?;
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

enum Body<'a> {
    Form(Vec<(&'a str, &'a str)>),
    Json(Value),
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

// ----- The device-code flow ----------------------------------------------------------------------

/// What the player types at the verification page.
#[derive(Clone, PartialEq)]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_at: u64,
    device_code: String,
}

impl std::fmt::Debug for DeviceCode {
    // The device code can be traded for tokens once the player approves: never shown.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceCode")
            .field("user_code", &self.user_code)
            .field("verification_uri", &self.verification_uri)
            .finish_non_exhaustive()
    }
}

/// The Microsoft tokens; the access token only lasts for the Xbox step.
pub struct MsTokens {
    access: String,
    refresh: String,
}

impl std::fmt::Debug for MsTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MsTokens(<hidden>)")
    }
}

pub enum Poll {
    Pending,
    /// The server asked to poll less often: wait 5 s more.
    SlowDown,
    Done(MsTokens),
}

pub fn device_code(ep: &Endpoints, client_id: &str) -> Result<DeviceCode, String> {
    let (status, v) = send(
        request_to("POST", &format!("{}/devicecode", ep.microsoft)),
        Some(Body::Form(vec![("client_id", client_id), ("scope", SCOPE)])),
        "Microsoft",
    )?;
    match (
        status,
        str_field(&v, "user_code"),
        str_field(&v, "device_code"),
    ) {
        (200, Some(user_code), Some(device_code)) => Ok(DeviceCode {
            user_code,
            verification_uri: str_field(&v, "verification_uri")
                .unwrap_or_else(|| "https://www.microsoft.com/link".into()),
            interval: v
                .get("interval")
                .and_then(Value::as_u64)
                .unwrap_or(5)
                .max(1),
            expires_at: now().saturating_add(
                v.get("expires_in")
                    .and_then(Value::as_u64)
                    .unwrap_or(900)
                    .min(3600),
            ),
            device_code,
        }),
        _ => Err(oauth_error(&v)),
    }
}

/// The OAuth error codes as messages for the player.
fn oauth_error(v: &Value) -> String {
    match v.get("error").and_then(Value::as_str) {
        Some("authorization_declined") => "Sign-in was declined in the browser.".into(),
        Some("expired_token") => "The sign-in code expired. Start again.".into(),
        Some("bad_verification_code") => "The sign-in code was not recognised. Start again.".into(),
        Some("invalid_grant") => SIGN_IN_AGAIN.into(),
        Some("invalid_client" | "unauthorized_client") => {
            "Microsoft refused this launcher's app registration (client ID).".into()
        }
        Some(_) => "Microsoft sign-in failed. Try again.".into(),
        None => "Microsoft sign-in failed.".into(),
    }
}

/// One poll of the token endpoint while the player signs in.
pub fn poll(ep: &Endpoints, client_id: &str, code: &DeviceCode) -> Result<Poll, String> {
    let (status, v) = send(
        request_to("POST", &format!("{}/token", ep.microsoft)),
        Some(Body::Form(vec![
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", client_id),
            ("device_code", &code.device_code),
        ])),
        "Microsoft",
    )?;
    match v.get("error").and_then(Value::as_str) {
        Some("authorization_pending") => return Ok(Poll::Pending),
        Some("slow_down") => return Ok(Poll::SlowDown),
        _ => {}
    }
    ms_tokens(status, &v).map(Poll::Done)
}

fn ms_tokens(status: u16, v: &Value) -> Result<MsTokens, String> {
    match (
        status,
        str_field(v, "access_token"),
        str_field(v, "refresh_token"),
    ) {
        (200, Some(access), Some(refresh)) => Ok(MsTokens { access, refresh }),
        _ => Err(oauth_error(v)),
    }
}

/// Wait for the player to finish in the browser, polling as asked, until the code expires or
/// `cancelled` says stop.
pub fn wait_for_sign_in(
    ep: &Endpoints,
    client_id: &str,
    code: &DeviceCode,
    cancelled: &dyn Fn() -> bool,
) -> Result<MsTokens, String> {
    let mut interval = code.interval;
    loop {
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
        match poll(ep, client_id, code)? {
            Poll::Pending => {}
            Poll::SlowDown => interval += 5,
            Poll::Done(tokens) => return Ok(tokens),
        }
    }
}

fn refresh_ms(ep: &Endpoints, client_id: &str, refresh: &str) -> Result<MsTokens, String> {
    let (status, v) = send(
        request_to("POST", &format!("{}/token", ep.microsoft)),
        Some(Body::Form(vec![
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("refresh_token", refresh),
            ("scope", SCOPE),
        ])),
        "Microsoft",
    )?;
    ms_tokens(status, &v)
}

// ----- Xbox Live, XSTS, Minecraft ----------------------------------------------------------------

/// The XSTS `XErr` codes, as the official launcher explains them.
fn xsts_error(code: u64) -> String {
    match code {
        2148916233 => "This Microsoft account has no Xbox profile yet. Sign in once at xbox.com to create one, then try again.".into(),
        2148916235 => "Xbox Live is not available in your country or region.".into(),
        2148916236 | 2148916237 => "This account needs adult verification on xbox.com before it can play.".into(),
        2148916238 => "This is a child account: an adult has to add it to a Microsoft family first.".into(),
        other => format!("Xbox Live refused the sign-in (error {other})."),
    }
}

/// The Minecraft access token from the Microsoft one, through Xbox Live and XSTS.
struct Minecraft {
    token: String,
    expires_at: u64,
    xuid: Option<String>,
}

fn minecraft_login(
    ep: &Endpoints,
    ms_access: &str,
    progress: &dyn Fn(Phase),
) -> Result<Minecraft, String> {
    progress(Phase::Xbox);
    let (status, xbl) = send(
        request_to("POST", &ep.xbox_user),
        Some(Body::Json(json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={ms_access}"),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        }))),
        "Xbox Live",
    )?;
    let xbl_token = str_field(&xbl, "Token")
        .filter(|_| status == 200)
        .ok_or("Xbox Live refused the sign-in.")?;
    let (status, xsts) = send(
        request_to("POST", &ep.xsts),
        Some(Body::Json(json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        }))),
        "Xbox Live",
    )?;
    if let Some(code) = xsts.get("XErr").and_then(Value::as_u64) {
        return Err(xsts_error(code));
    }
    let claims = xsts.pointer("/DisplayClaims/xui/0");
    let (Some(xsts_token), Some(uhs)) = (
        str_field(&xsts, "Token").filter(|_| status == 200),
        claims.and_then(|c| str_field(c, "uhs")),
    ) else {
        return Err("Xbox Live refused the sign-in.".into());
    };
    let xuid = claims.and_then(|c| str_field(c, "xid"));
    progress(Phase::Minecraft);
    let (token, expires_in) = minecraft_token(ep, &format!("XBL3.0 x={uhs};{xsts_token}"))?;
    Ok(Minecraft {
        token,
        expires_at: now() + expires_in,
        xuid,
    })
}

/// The Minecraft access token and its lifetime from the XSTS one (`XBL3.0 x=<uhs>;<token>`).
/// `launcher/login` first: a new account's token from `login_with_xbox` names no profile, so
/// joining an online-mode server answers "Profile not found" (found and diagnosed by
/// martipops). `login_with_xbox` stays the fallback, so a sign-in that worked before still does
/// when the other refuses.
fn minecraft_token(ep: &Endpoints, xtoken: &str) -> Result<(String, u64), String> {
    let lifetime = |v: &Value| v.get("expires_in").and_then(Value::as_u64).unwrap_or(86400);
    let launcher = send(
        request_to("POST", &format!("{}/launcher/login", ep.minecraft)),
        Some(Body::Json(
            json!({ "xtoken": xtoken, "platform": "PC_LAUNCHER" }),
        )),
        "the Minecraft services",
    );
    if let Ok((200, v)) = &launcher {
        if let Some(token) = str_field(v, "access_token") {
            return Ok((token, lifetime(v)));
        }
    }
    let (status, mc) = send(
        request_to(
            "POST",
            &format!("{}/authentication/login_with_xbox", ep.minecraft),
        ),
        Some(Body::Json(json!({ "identityToken": xtoken }))),
        "the Minecraft services",
    )?;
    match (status, str_field(&mc, "access_token")) {
        (200, Some(token)) => Ok((token, lifetime(&mc))),
        // New Azure apps are refused until Mojang approves them (Minecraft API access).
        (403, _) => Err(
            "The Minecraft services refused this launcher: its app is not approved by Mojang yet."
                .into(),
        ),
        (429, _) => Err("Too many sign-ins in a short time. Wait a minute and try again.".into()),
        _ => Err("The Minecraft services refused the sign-in.".into()),
    }
}

fn bearer(request: ureq::Request, token: &str) -> ureq::Request {
    request.set("Authorization", &format!("Bearer {token}"))
}

/// Whether the account owns Minecraft: Java Edition (Game Pass included).
fn owns_game(ep: &Endpoints, token: &str) -> Result<bool, String> {
    let (status, v) = send(
        bearer(
            request_to("GET", &format!("{}/entitlements/mcstore", ep.minecraft)),
            token,
        ),
        None,
        "the Minecraft services",
    )?;
    if status != 200 {
        return Err("Could not check which games this account owns.".into());
    }
    Ok(v.get("items")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|i| {
                matches!(
                    i.get("name").and_then(Value::as_str),
                    Some("game_minecraft" | "product_minecraft")
                )
            })
        }))
}

/// A Minecraft profile, as the launcher shows and remembers it (no secrets).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Account {
    /// The profile UUID without dashes.
    pub uuid: String,
    pub name: String,
    /// The active skin's URL (textures.minecraft.net) and model ("classic" or "slim").
    #[serde(default)]
    pub skin: Option<(String, String)>,
    #[serde(default)]
    pub cape: Option<String>,
}

/// The profile; none when the account has none (it doesn't own the game, or never made one).
fn profile(ep: &Endpoints, token: &str) -> Result<Option<Account>, String> {
    let (status, v) = send(
        bearer(
            request_to("GET", &format!("{}/minecraft/profile", ep.minecraft)),
            token,
        ),
        None,
        "the Minecraft services",
    )?;
    if status == 404 {
        return Ok(None);
    }
    if status != 200 {
        return Err("Could not read the Minecraft profile.".into());
    }
    let (Some(uuid), Some(name)) = (str_field(&v, "id"), str_field(&v, "name")) else {
        return Err("Could not read the Minecraft profile.".into());
    };
    let normalized = uuid.replace('-', "");
    if normalized.len() != 32
        || !normalized.bytes().all(|b| b.is_ascii_hexdigit())
        || name.chars().count() > 16
        || name.chars().any(char::is_control)
    {
        return Err("The Minecraft profile is not valid.".into());
    }
    let active = |key: &str| {
        v.get(key)
            .and_then(Value::as_array)
            .and_then(|list| {
                list.iter()
                    .find(|s| s.get("state").and_then(Value::as_str) == Some("ACTIVE"))
            })
            .cloned()
    };
    Ok(Some(Account {
        uuid: normalized.to_lowercase(),
        name,
        skin: active("skins").and_then(|s| {
            Some((
                str_field(&s, "url")?,
                str_field(&s, "variant")
                    .unwrap_or_else(|| "CLASSIC".into())
                    .to_lowercase(),
            ))
        }),
        cape: active("capes").and_then(|c| str_field(&c, "url")),
    }))
}

/// What the keychain keeps for one account.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Secrets {
    refresh_token: String,
    #[serde(default)]
    sign_in_required: bool,
    minecraft_token: String,
    minecraft_expires_at: u64,
    #[serde(default)]
    xuid: Option<String>,
    /// The last answer the Minecraft services gave: whether the account owns the game. It is
    /// kept here, beside the tokens, so that no plain file can say yes. Accounts saved before it
    /// existed passed the same check at sign-in, which refuses everyone else.
    #[serde(default = "owned_at_sign_in")]
    owns: bool,
}
fn owned_at_sign_in() -> bool {
    true
}
impl std::fmt::Debug for Secrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secrets(<hidden>)")
    }
}

/// From Microsoft tokens to a ready account: Xbox, XSTS, Minecraft, ownership, profile.
pub fn finish_sign_in(ep: &Endpoints, ms: MsTokens) -> Result<(Account, Secrets), String> {
    finish_sign_in_with_progress(ep, ms, &|_| {})
}

pub fn finish_sign_in_with_progress(
    ep: &Endpoints,
    ms: MsTokens,
    progress: &dyn Fn(Phase),
) -> Result<(Account, Secrets), String> {
    let mc = minecraft_login(ep, &ms.access, progress)?;
    progress(Phase::Ownership);
    if !owns_game(ep, &mc.token)? {
        return Err(DOES_NOT_OWN.into());
    }
    progress(Phase::Profile);
    let account = profile(ep, &mc.token)?.ok_or(NO_PROFILE)?;
    Ok((account, Secrets::new(ms.refresh, mc)))
}

impl Secrets {
    fn new(refresh_token: String, mc: Minecraft) -> Self {
        Self {
            refresh_token,
            sign_in_required: false,
            minecraft_token: mc.token,
            minecraft_expires_at: mc.expires_at,
            xuid: mc.xuid,
            owns: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Phase {
    Microsoft,
    Xbox,
    Minecraft,
    Ownership,
    Profile,
    Saving,
}

impl Phase {
    pub fn label(self) -> String {
        crate::i18n::tr(match self {
            Self::Microsoft => "Renewing Microsoft sign-in…",
            Self::Xbox => "Signing in to Xbox Live…",
            Self::Minecraft => "Getting Minecraft session…",
            Self::Ownership => "Checking Minecraft ownership…",
            Self::Profile => "Loading Minecraft profile…",
            Self::Saving => "Saving sign-in…",
        })
    }
}

/// Public identity and encrypted-at-rest credentials. Debug output never includes tokens.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SignedIn {
    pub account: Account,
    secrets: Secrets,
}

impl std::fmt::Debug for SignedIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedIn")
            .field("account", &self.account)
            .finish_non_exhaustive()
    }
}

impl SignedIn {
    pub fn from_tokens(account: Account, secrets: Secrets) -> Self {
        Self { account, secrets }
    }

    fn renew(
        &mut self,
        home: &Path,
        ep: &Endpoints,
        client: &str,
        progress: &dyn Fn(Phase),
    ) -> Result<(), String> {
        if self.secrets.sign_in_required {
            return Err(SIGN_IN_AGAIN.into());
        }
        if self.secrets.minecraft_expires_at <= now().saturating_add(RENEW_BEFORE) {
            progress(Phase::Microsoft);
            let ms = match refresh_ms(ep, client, &self.secrets.refresh_token) {
                Ok(ms) => ms,
                Err(error) => {
                    if error == SIGN_IN_AGAIN {
                        self.secrets.sign_in_required = true;
                        storage::save_locked(home, self)?;
                    }
                    return Err(error);
                }
            };
            // Preserve a rotated refresh credential even if the next service is down.
            self.secrets.refresh_token = ms.refresh;
            storage::save_locked(home, self)?;
            let mc = minecraft_login(ep, &ms.access, progress)?;
            self.secrets = Secrets::new(self.secrets.refresh_token.clone(), mc);
        }
        progress(Phase::Ownership);
        if !owns_game(ep, &self.secrets.minecraft_token)? {
            self.secrets.owns = false;
            storage::save_locked(home, self)?;
            return Err(DOES_NOT_OWN.into());
        }
        progress(Phase::Profile);
        let profile = profile(ep, &self.secrets.minecraft_token)?.ok_or(NO_PROFILE)?;
        if profile.uuid != self.account.uuid {
            return Err(OTHER_PROFILE.into());
        }
        self.account = profile;
        self.secrets.owns = true;
        storage::save_locked(home, self)
    }

    pub fn credentials(&self) -> GameCredentials {
        GameCredentials {
            name: self.account.name.clone(),
            uuid: self.account.uuid.clone(),
            access_token: self.secrets.minecraft_token.clone(),
            xuid: self.secrets.xuid.clone(),
        }
    }
}

/// Sent through the Rust engine's session-stdin pipe, never through arguments or settings.
pub struct GameCredentials {
    name: String,
    uuid: String,
    access_token: String,
    xuid: Option<String>,
}

impl std::fmt::Debug for GameCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameCredentials")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl GameCredentials {
    /// The profile UUID without dashes.
    pub fn uuid(&self) -> &str {
        &self.uuid
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Tells Mojang this account joins `server_id`, as the game does before a server (or the
    /// Wisp server) checks it with `hasJoined`. The token goes to Mojang only.
    pub fn join_server(&self, server_id: &str) -> Result<(), String> {
        let answer = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .post("https://sessionserver.mojang.com/session/minecraft/join")
            .send_json(json!({
                "accessToken": self.access_token,
                "selectedProfile": self.uuid,
                "serverId": server_id,
            }));
        match answer {
            Ok(r) if r.status() < 300 => Ok(()),
            Ok(r) => Err(format!("Mojang answered {} to the join.", r.status())),
            Err(ureq::Error::Status(code, _)) => Err(format!("Mojang refused the join ({code}).")),
            Err(e) => Err(format!("Can't reach Mojang: {e}")),
        }
    }

    pub(crate) fn write_to(&self, output: impl std::io::Write) -> Result<(), String> {
        serde_json::to_writer(
            output,
            &json!({
                "name": self.name, "uuid": self.uuid, "access_token": self.access_token,
                "user_type": "msa", "xuid": self.xuid,
            }),
        )
        .map_err(|_| crate::i18n::tr("Could not pass the account session to the game."))
    }
}

#[cfg(test)]
mod tests;
