//! The launcher's side of friends on the Wisp server (`crates/wisp-cosmetics-server`,
//! `friends.rs`): sign in the way the game does for cosmetics (Mojang's join check), say where
//! the player is, and read or change the friends list. Every answer that changes the list
//! carries the whole list back.
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Friend {
    pub uuid: String,
    pub name: String,
    pub online: bool,
    /// "launcher" or "playing" while online.
    pub status: Option<String>,
    /// The server they play on, if their launcher said.
    pub server: Option<String>,
    /// When they were last here (seconds since 1970), 0 if never.
    pub last_seen: u64,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Person {
    pub uuid: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Snapshot {
    pub friends: Vec<Friend>,
    pub incoming: Vec<Person>,
    pub outgoing: Vec<Person>,
}

impl Snapshot {
    pub fn online(&self) -> usize {
        self.friends.iter().filter(|f| f.online).count()
    }
}

/// Why a call failed: the session ended (sign in again), or anything else with its message.
#[derive(Debug, PartialEq)]
pub enum Failure {
    SignIn,
    Message(String),
}

impl Failure {
    pub fn message(&self) -> String {
        match self {
            Self::SignIn => "Signing in to the Wisp server again.".into(),
            Self::Message(m) => m.clone(),
        }
    }
}

/// The list as the server answers it.
pub fn parse(value: &Value) -> Snapshot {
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    let people = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|p| {
                Some(Person {
                    uuid: text(p, "uuid")?,
                    name: text(p, "name").unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>()
    };
    let mut friends: Vec<Friend> = value
        .get("friends")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|f| {
            Some(Friend {
                uuid: text(f, "uuid")?,
                name: text(f, "name").unwrap_or_default(),
                online: f.get("online").and_then(Value::as_bool).unwrap_or(false),
                status: text(f, "status"),
                server: text(f, "server"),
                last_seen: f.get("last_seen").and_then(Value::as_u64).unwrap_or(0),
            })
        })
        .collect();
    // Online first, then by name.
    friends.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Snapshot {
        friends,
        incoming: people("incoming"),
        outgoing: people("outgoing"),
    }
}

/// One Wisp server and this launcher's session on it.
pub struct Client {
    base: String,
    session: Option<String>,
    agent: ureq::Agent,
}

impl Client {
    pub fn new(base: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_owned(),
            session: None,
            agent: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(10))
                .build(),
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn signed_in(&self) -> bool {
        self.session.is_some()
    }

    /// Forgets the session (it ended, or another account plays now).
    pub fn sign_out(&mut self) {
        self.session = None;
    }

    fn call(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value, Failure> {
        let mut request = self
            .agent
            .request(method, &format!("{}{path}", self.base))
            .set("Content-Type", "application/json");
        if let Some(s) = &self.session {
            request = request.set("Authorization", &format!("Bearer {s}"));
        }
        let answer = match body {
            Some(body) => request.send_string(&body.to_string()),
            None => request.call(),
        };
        match answer {
            Ok(r) => r
                .into_json()
                .map_err(|e| Failure::Message(format!("The Wisp server answered oddly: {e}"))),
            Err(ureq::Error::Status(401, _)) if path != "/v1/auth/finish" => Err(Failure::SignIn),
            Err(ureq::Error::Status(code, r)) => {
                let message = r
                    .into_json::<Value>()
                    .ok()
                    .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_owned))
                    .unwrap_or_else(|| format!("The Wisp server answered {code}."));
                Err(Failure::Message(message))
            }
            Err(e) => Err(Failure::Message(format!(
                "Can't reach the Wisp server: {e}"
            ))),
        }
    }

    /// Signs `name` in: `join` tells Mojang the account joins the id the server hands out.
    pub fn sign_in(
        &mut self,
        name: &str,
        join: impl FnOnce(&str) -> Result<(), String>,
    ) -> Result<(), String> {
        self.session = None;
        let start = self
            .call("POST", "/v1/auth/start", Some(json!({})))
            .map_err(|e| e.message())?;
        let id = start["server_id"]
            .as_str()
            .ok_or("The Wisp server sent no sign-in id.")?;
        join(id)?;
        let finish = self
            .call(
                "POST",
                "/v1/auth/finish",
                Some(json!({ "server_id": id, "name": name })),
            )
            .map_err(|e| e.message())?;
        self.session = Some(
            finish["session"]
                .as_str()
                .ok_or("The Wisp server sent no session.")?
                .to_owned(),
        );
        Ok(())
    }

    /// Says where the player is; answers the friends list.
    pub fn presence(&self, status: &str, server: Option<&str>) -> Result<Snapshot, Failure> {
        self.call(
            "POST",
            "/v1/presence",
            Some(json!({ "status": status, "server": server })),
        )
        .map(|v| parse(&v))
    }

    pub fn request(&self, name: &str) -> Result<Snapshot, Failure> {
        self.call(
            "POST",
            "/v1/friends/request",
            Some(json!({ "name": name.trim() })),
        )
        .map(|v| parse(&v))
    }

    pub fn accept(&self, uuid: &str) -> Result<Snapshot, Failure> {
        self.call("POST", "/v1/friends/accept", Some(json!({ "uuid": uuid })))
            .map(|v| parse(&v))
    }

    pub fn decline(&self, uuid: &str) -> Result<Snapshot, Failure> {
        self.call("POST", "/v1/friends/decline", Some(json!({ "uuid": uuid })))
            .map(|v| parse(&v))
    }

    /// Removes a friend, or takes back a request.
    pub fn remove(&self, uuid: &str) -> Result<Snapshot, Failure> {
        self.call("DELETE", &format!("/v1/friends/{uuid}"), None)
            .map(|v| parse(&v))
    }

    /// The player's wallet, as the server keeps it.
    pub fn wallet(&self) -> Result<Wallet, Failure> {
        self.call("GET", "/v1/wallet", None)
            .map(|v| Wallet::parse(&v))
    }

    /// The welcome gift and today's bonus, once each: the coins given and the wallet.
    pub fn collect(&self) -> Result<(u32, Wallet), Failure> {
        self.call("POST", "/v1/wallet/collect", Some(json!({})))
            .map(|v| (count(&v["given"]), Wallet::parse(&v["wallet"])))
    }

    /// Buys an item: the server prices it (less the supported creator's discount) and checks
    /// the coins. Answers the coins paid and the wallet.
    pub fn buy(&self, item: &str) -> Result<(u32, Wallet), Failure> {
        self.call("POST", "/v1/shop/buy", Some(json!({ "item": item })))
            .map(|v| (count(&v["paid"]), Wallet::parse(&v["wallet"])))
    }

    pub fn equip(&self, item: &str) -> Result<Wallet, Failure> {
        self.call("POST", "/v1/shop/equip", Some(json!({ "item": item })))
            .map(|v| Wallet::parse(&v["wallet"]))
    }

    pub fn unequip(&self, item: &str) -> Result<Wallet, Failure> {
        self.call("POST", "/v1/shop/unequip", Some(json!({ "item": item })))
            .map(|v| Wallet::parse(&v["wallet"]))
    }

    /// Supports the creator with `code`, or nobody with None.
    pub fn support(&self, code: Option<&str>) -> Result<(), Failure> {
        self.call("PUT", "/v1/supporting", Some(json!({ "code": code })))
            .map(|_| ())
    }
}

fn count(value: &Value) -> u32 {
    value
        .as_u64()
        .map_or(0, |n| n.min(u64::from(u32::MAX)) as u32)
}

/// The player's Shop wallet on the Wisp server: the only place coins are kept.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Wallet {
    pub coins: u32,
    pub owned: Vec<String>,
    pub equipped: Vec<String>,
    /// Today's daily bonus is given.
    pub collected: bool,
    /// Percent off with the supported creator's code (0 without one).
    pub discount: u32,
    /// The creator supported: their name and code.
    pub supporting: Option<(String, Option<String>)>,
}

impl Wallet {
    pub fn parse(value: &Value) -> Self {
        let list = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        let supporting = value.get("supporting").filter(|s| s.is_object()).map(|s| {
            (
                s["name"].as_str().unwrap_or_default().to_owned(),
                s["code"].as_str().map(str::to_owned),
            )
        });
        Self {
            coins: count(&value["coins"]),
            owned: list("owned"),
            equipped: list("equipped"),
            collected: value["collected"].as_bool().unwrap_or(false),
            discount: count(&value["discount"]),
            supporting,
        }
    }

    /// The Shop's own wallet type, for the views that read it.
    pub fn as_shop(&self) -> crate::shop::Wallet {
        crate::shop::Wallet {
            coins: self.coins,
            owned: self.owned.clone(),
            equipped: self.equipped.clone(),
            bonus_day: self.collected.then(crate::shop::today),
            welcomed: true,
        }
    }
}

/// A friend's skin texture URL and whether it has slim arms, from Mojang's public profile.
pub fn skin_of(uuid: &str) -> Option<(String, bool)> {
    use base64::Engine;
    if uuid.len() != 32 || !uuid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let profile: Value = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(8))
        .build()
        .get(&format!(
            "https://sessionserver.mojang.com/session/minecraft/profile/{uuid}"
        ))
        .call()
        .ok()?
        .into_json()
        .ok()?;
    let encoded = profile["properties"]
        .as_array()?
        .iter()
        .find(|p| p["name"] == "textures")?["value"]
        .as_str()?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    let textures: Value = serde_json::from_slice(&decoded).ok()?;
    let skin = &textures["textures"]["SKIN"];
    Some((
        skin["url"].as_str()?.to_owned(),
        skin["metadata"]["model"] == "slim",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wallet as the Wisp server answers it (`/v1/wallet`), into what the views read.
    #[test]
    fn the_servers_wallet_is_read_with_the_creator_supported() {
        let wallet = Wallet::parse(&json!({
            "coins": 1100, "owned": ["founders_cape"], "equipped": ["founders_cape"],
            "collected": true, "discount": 10,
            "supporting": { "name": "Jeb", "code": "JEB_" },
        }));
        assert_eq!(wallet.coins, 1100);
        assert_eq!(wallet.discount, 10);
        assert_eq!(wallet.supporting, Some(("Jeb".into(), Some("JEB_".into()))));
        let shop = wallet.as_shop();
        assert!(shop.owns("founders_cape") && shop.wears("founders_cape"));
        assert_eq!(shop.bonus_day, Some(crate::shop::today()));
        // Nobody supported, nothing collected yet: no discount, no bonus day.
        let plain = Wallet::parse(&json!({ "coins": 5, "supporting": null }));
        assert_eq!(
            (plain.discount, plain.supporting.clone(), plain.as_shop().bonus_day),
            (0, None, None)
        );
    }

    #[test]
    fn the_list_is_read_online_first_then_by_name() {
        let snapshot = parse(&json!({
            "friends": [
                {"uuid": "a", "name": "zed", "online": false, "last_seen": 100},
                {"uuid": "b", "name": "Bob", "online": true, "status": "playing", "server": "zedix.de"},
                {"uuid": "c", "name": "amy", "online": true, "status": "launcher"},
                {"name": "no uuid"}
            ],
            "incoming": [{"uuid": "d", "name": "Dan"}],
            "outgoing": [{"uuid": "e", "name": "Eve"}]
        }));
        let names: Vec<_> = snapshot.friends.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["amy", "Bob", "zed"]);
        assert_eq!(snapshot.online(), 2);
        assert_eq!(snapshot.friends[1].server.as_deref(), Some("zedix.de"));
        assert_eq!(snapshot.friends[2].last_seen, 100);
        assert_eq!(snapshot.incoming[0].name, "Dan");
        assert_eq!(snapshot.outgoing[0].uuid, "e");
        assert_eq!(parse(&Value::Null), Snapshot::default());
    }

    #[test]
    fn a_server_that_is_not_there_is_a_message_not_a_panic() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let client = Client::new(&format!("http://127.0.0.1:{port}/"));
        assert_eq!(client.base(), format!("http://127.0.0.1:{port}"));
        assert!(matches!(
            client.presence("launcher", None),
            Err(Failure::Message(_))
        ));
    }
}
