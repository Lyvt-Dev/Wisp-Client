//! Sharing with other Wisp players through the cosmetics service: what this player wears
//! (`PUT /v1/wear`, once signed in), the emote they play (`POST`/`DELETE /v1/emote`), and the
//! emotes the players in view are playing (`POST /v1/emotes`, asked every second while any
//! player is in view), and friends: where this player is (`POST /v1/presence`, every 20 seconds,
//! answered with the friends list) and requests, accepts and removals (`/v1/friends/...`, see
//! the server's `friends.rs`). All of it runs on a thread of its own; the game only hands it work
//! and reads the last answer.
use crate::account::{self, Call};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// How often the players in view are asked about.
const POLL: Duration = Duration::from_secs(1);
/// After a failure, the next attempt waits this long (doubling to the second).
const BACKOFF: (Duration, Duration) = (Duration::from_secs(5), Duration::from_secs(300));
/// How often the game says where it is and reads the friends list.
const FRIENDS_EVERY: Duration = Duration::from_secs(20);

/// A friend, as the service last told.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Friend {
    /// 32 hex digits.
    pub uuid: String,
    pub name: String,
    pub online: bool,
    /// In a game (else in the launcher), when online.
    pub playing: bool,
    /// The server they play on, when they play on one.
    pub server: Option<String>,
    /// When they were last signed in, Unix seconds (0: never told).
    pub last_seen: i64,
}

/// The friends list: friends (online first, then by name) and open requests as (uuid, name).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Friends {
    pub friends: Vec<Friend>,
    pub incoming: Vec<(String, String)>,
    pub outgoing: Vec<(String, String)>,
}

impl Friends {
    /// Reads the service's answer (`{"friends", "incoming", "outgoing"}`).
    pub fn parse(v: &Value) -> Option<Self> {
        let people = |key: &str| -> Vec<(String, String)> {
            v[key]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| {
                    Some((
                        p["uuid"].as_str()?.to_owned(),
                        p["name"].as_str()?.to_owned(),
                    ))
                })
                .collect()
        };
        let mut friends: Vec<Friend> = v["friends"]
            .as_array()?
            .iter()
            .filter_map(|f| {
                Some(Friend {
                    uuid: f["uuid"].as_str()?.to_owned(),
                    name: f["name"].as_str()?.to_owned(),
                    online: f["online"].as_bool().unwrap_or(false),
                    playing: f["status"].as_str() == Some("playing"),
                    server: f["server"].as_str().map(str::to_owned),
                    last_seen: f["last_seen"].as_i64().unwrap_or(0),
                })
            })
            .collect();
        friends.sort_by_key(|f| (!f.online, f.name.to_lowercase()));
        Some(Self {
            friends,
            incoming: people("incoming"),
            outgoing: people("outgoing"),
        })
    }
}

/// Something to do about friends.
#[derive(Clone, Debug, PartialEq)]
pub enum FriendAsk {
    /// A request to the player of this name.
    Request(String),
    Accept(String),
    Decline(String),
    /// Removes a friend or takes back a request (uuid).
    Remove(String),
    /// Reads the list now.
    Refresh,
}

/// What the game shows of friends.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FriendsState {
    /// The last list read (None: none yet).
    pub list: Option<Friends>,
    /// Why there is no list: no account, a failed sign-in, an unreachable service.
    pub problem: Option<String>,
    /// The answer to the last request ("That player hasn't used Wisp yet."), and when.
    pub notice: Option<(String, Instant)>,
    /// A request is on its way.
    pub busy: bool,
}

/// An emote another player is playing.
#[derive(Clone, Debug, PartialEq)]
pub struct Playing {
    pub id: String,
    /// When it started, by this game's clock.
    pub started: Instant,
}

enum Work {
    Wear(Vec<Value>),
    Emote(Option<String>),
    Friend(FriendAsk),
}

/// Signs in to Mojang for the service: the game's `auth::join_server` with its session.
pub type Join = Box<dyn Fn(&str) -> Result<(), String> + Send>;

#[derive(Default)]
struct Shared {
    /// The players in view (uuid hex), set by the game.
    asking: Vec<String>,
    /// Their emotes, by uuid.
    playing: HashMap<u128, Playing>,
    /// The server this player is on (None: singleplayer or the title screen).
    server: Option<String>,
    friends: FriendsState,
}

pub struct Sharing {
    work: Sender<Work>,
    shared: Arc<Mutex<Shared>>,
}

impl Sharing {
    /// Starts sharing for the account `name` (`join` signs it in with Mojang) through
    /// `service`. Without an account (`join` None) it only reads other players' emotes.
    pub fn start(service: impl Call + Send + 'static, name: String, join: Option<Join>) -> Self {
        let (work, inbox) = channel();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let state = shared.clone();
        std::thread::Builder::new()
            .name("Cosmetics sharing".into())
            .spawn(move || run(&service, &name, join, &inbox, &state))
            .ok();
        Self { work, shared }
    }

    /// Publishes what this player wears (item files as JSON).
    pub fn wear(&self, items: Vec<Value>) {
        let _ = self.work.send(Work::Wear(items));
    }

    /// Tells the service this player started `id`, or stopped (None).
    pub fn emote(&self, id: Option<&str>) {
        let _ = self.work.send(Work::Emote(id.map(str::to_owned)));
    }

    /// The players in view, whose emotes to ask about.
    pub fn asking(&self, players: impl IntoIterator<Item = u128>) {
        let mut s = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        s.asking = players
            .into_iter()
            .take(crate::BATCH)
            .map(|u| format!("{u:032x}"))
            .collect();
        let asked: Vec<u128> = s
            .asking
            .iter()
            .filter_map(|u| u128::from_str_radix(u, 16).ok())
            .collect();
        s.playing.retain(|u, _| asked.contains(u));
    }

    /// The server this player is on now, told to friends (None: not on a server).
    pub fn set_server(&self, server: Option<String>) {
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .server = server;
    }

    /// Asks the service something about friends; the answer shows in [`Self::friends`].
    pub fn friend(&self, ask: FriendAsk) {
        if !matches!(ask, FriendAsk::Refresh) {
            self.shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .friends
                .busy = true;
        }
        let _ = self.work.send(Work::Friend(ask));
    }

    /// The friends list and its state, as last answered.
    pub fn friends(&self) -> FriendsState {
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .friends
            .clone()
    }

    /// The emotes the players in view are playing, as last answered.
    pub fn playing(&self) -> HashMap<u128, Playing> {
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .playing
            .clone()
    }
}

fn run(
    service: &dyn Call,
    name: &str,
    join: Option<Join>,
    inbox: &Receiver<Work>,
    shared: &Mutex<Shared>,
) {
    let mut session: Option<String> = None;
    // Work not done yet: the last wear, the last emote change.
    let mut wear: Option<Vec<Value>> = None;
    let mut emote: Option<Option<String>> = None;
    let mut retry: Option<(Instant, Duration)> = None;
    let mut next_poll = Instant::now();
    let mut next_friends = Instant::now();
    if join.is_none() {
        shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .friends
            .problem = Some("Sign in with Microsoft to use friends.".into());
    }
    loop {
        let wait = next_poll
            .min(next_friends)
            .saturating_duration_since(Instant::now());
        let mut ask = None;
        match inbox.recv_timeout(wait) {
            Ok(Work::Wear(items)) => wear = Some(items),
            Ok(Work::Emote(id)) => emote = Some(id),
            Ok(Work::Friend(a)) => ask = Some(a),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        // Friends: a request now, or where this player is every while.
        if let Some(join) = &join
            && (ask.is_some() || Instant::now() >= next_friends)
        {
            next_friends = Instant::now() + FRIENDS_EVERY;
            friends_work(service, name, join, &mut session, ask.take(), shared);
        }
        // The account's work, signed in when needed, retried after failures.
        let due = retry.is_none_or(|(at, _)| Instant::now() >= at);
        if (wear.is_some() || emote.is_some())
            && due
            && let Some(join) = &join
        {
            let result = (|| -> Result<(), String> {
                if session.is_none() {
                    session = Some(account::sign_in(service, name, join)?);
                }
                let s = session.as_deref();
                if let Some(items) = &wear {
                    service.call("PUT", "/v1/wear", s, &json!({ "items": items }))?;
                    wear = None;
                }
                match &emote {
                    Some(Some(id)) => {
                        service.call("POST", "/v1/emote", s, &json!({ "id": id }))?;
                    }
                    Some(None) => {
                        service.call("DELETE", "/v1/emote", s, &json!({}))?;
                    }
                    None => {}
                }
                emote = None;
                Ok(())
            })();
            retry = match result {
                Ok(()) => None,
                Err(e) => {
                    // An expired session signs in again next time.
                    if e.contains("401") {
                        session = None;
                    }
                    let wait = retry.map_or(BACKOFF.0, |(_, w)| (w * 2).min(BACKOFF.1));
                    Some((Instant::now() + wait, wait))
                }
            };
        } else if join.is_none() {
            wear = None;
            emote = None;
        }
        // Other players' emotes.
        if Instant::now() >= next_poll {
            next_poll = Instant::now() + POLL;
            let asking = shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .asking
                .clone();
            if asking.is_empty() {
                continue;
            }
            let Ok(answer) =
                service.call("POST", "/v1/emotes", None, &json!({ "players": asking }))
            else {
                continue;
            };
            let now = Instant::now();
            let mut s = shared.lock().unwrap_or_else(PoisonError::into_inner);
            let mut playing = HashMap::new();
            for (uuid, e) in answer["players"].as_object().into_iter().flatten() {
                let (Ok(uuid), Some(id), Some(elapsed)) = (
                    u128::from_str_radix(uuid, 16),
                    e["id"].as_str(),
                    e["elapsed"].as_f64(),
                ) else {
                    continue;
                };
                let started = now
                    .checked_sub(Duration::from_secs_f64(elapsed.max(0.0)))
                    .unwrap_or(now);
                // The same emote keeps its start: a second's jitter would make it jump.
                let keep = s.playing.get(&uuid).filter(|p| {
                    p.id == id
                        && (p.started.max(started) - p.started.min(started))
                            < Duration::from_millis(1500)
                });
                playing.insert(
                    uuid,
                    keep.cloned().unwrap_or(Playing {
                        id: id.to_owned(),
                        started,
                    }),
                );
            }
            s.playing = playing;
        }
    }
}

const SIGN_IN: &str = "Couldn't sign in to the Wisp server.";

/// A call with the service's session, signing in first or again after it expired.
fn signed(
    service: &dyn Call,
    name: &str,
    join: &Join,
    session: &mut Option<String>,
    call: &dyn Fn(Option<&str>) -> Result<Value, String>,
) -> Result<Value, String> {
    for _ in 0..2 {
        if session.is_none() {
            *session = Some(account::sign_in(service, name, join).map_err(|_| SIGN_IN.to_owned())?);
        }
        match call(session.as_deref()) {
            Err(e) if e.starts_with("HTTP 401") => *session = None,
            other => return other,
        }
    }
    Err(SIGN_IN.into())
}

/// What to tell the player about a failed call: the service's own reason when it gave one.
fn reason(e: &str) -> String {
    match e.split_once(": ") {
        Some((head, r)) if head.starts_with("HTTP ") => r.to_owned(),
        _ if e == SIGN_IN => SIGN_IN.to_owned(),
        _ if e.starts_with("HTTP 404") => "The Wisp server has no friends yet: update it.".into(),
        _ => "Can't reach the Wisp server.".into(),
    }
}

fn friends_work(
    service: &dyn Call,
    name: &str,
    join: &Join,
    session: &mut Option<String>,
    ask: Option<FriendAsk>,
    shared: &Mutex<Shared>,
) {
    let server = shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .server
        .clone();
    let asked = ask
        .as_ref()
        .is_some_and(|a| !matches!(a, FriendAsk::Refresh));
    let result = signed(service, name, join, session, &|s| match &ask {
        Some(FriendAsk::Request(who)) => service.call(
            "POST",
            "/v1/friends/request",
            s,
            &json!({ "name": who.trim() }),
        ),
        Some(FriendAsk::Accept(uuid)) => {
            service.call("POST", "/v1/friends/accept", s, &json!({ "uuid": uuid }))
        }
        Some(FriendAsk::Decline(uuid)) => {
            service.call("POST", "/v1/friends/decline", s, &json!({ "uuid": uuid }))
        }
        Some(FriendAsk::Remove(uuid)) => {
            service.call("DELETE", &format!("/v1/friends/{uuid}"), s, &json!({}))
        }
        Some(FriendAsk::Refresh) | None => service.call(
            "POST",
            "/v1/presence",
            s,
            &json!({ "status": "playing", "server": server, "from": "game" }),
        ),
    });
    let mut s = shared.lock().unwrap_or_else(PoisonError::into_inner);
    let f = &mut s.friends;
    if asked {
        f.busy = false;
    }
    match result.map(|v| Friends::parse(&v)) {
        Ok(Some(list)) => {
            f.list = Some(list);
            f.problem = None;
            if let Some(FriendAsk::Request(who)) = &ask {
                f.notice = Some((format!("Request sent to {}.", who.trim()), Instant::now()));
            }
        }
        Ok(None) => f.problem = Some("The Wisp server has no friends yet: update it.".into()),
        Err(e) if asked => f.notice = Some((reason(&e), Instant::now())),
        Err(e) => f.problem = Some(reason(&e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// The service, answering as the real one does, and noting each call.
    struct Fake {
        calls: Arc<Mutex<Vec<String>>>,
        expired: RefCell<bool>,
    }

    // Only ever used on the sharing thread.
    unsafe impl Sync for Fake {}

    impl Call for Fake {
        fn call(
            &self,
            method: &str,
            path: &str,
            session: Option<&str>,
            body: &Value,
        ) -> Result<Value, String> {
            self.calls.lock().unwrap().push(format!("{method} {path}"));
            match path {
                "/v1/auth/start" => Ok(json!({ "server_id": "s1" })),
                "/v1/auth/finish" => Ok(json!({ "session": "t1" })),
                "/v1/emotes" => Ok(
                    json!({ "players": { "000000000000000000000000000000ab": { "id": "black_hole", "elapsed": 2.0 } } }),
                ),
                "/v1/presence" | "/v1/friends/request" if session == Some("t1") => Ok(json!({
                    "friends": [
                        { "uuid": "b", "name": "zed", "online": false, "status": null, "server": null, "last_seen": 5 },
                        { "uuid": "a", "name": "Alex", "online": true, "status": "playing", "server": "zedix.de", "last_seen": 9 }
                    ],
                    "incoming": [{ "uuid": "c", "name": "Notch" }],
                    "outgoing": []
                })),
                _ if session != Some("t1") => Err("HTTP 401".into()),
                _ if self.expired.replace(false) => Err("HTTP 401".into()),
                _ => Ok(json!({ "ok": true, "echo": body })),
            }
        }
    }

    fn wait_for(mut done: impl FnMut() -> bool) {
        let start = Instant::now();
        while !done() {
            assert!(start.elapsed() < Duration::from_secs(5), "timed out");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn it_signs_in_once_then_publishes_wear_and_emotes() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let fake = Fake {
            calls: calls.clone(),
            expired: RefCell::new(false),
        };
        let sharing = Sharing::start(fake, "Steve".into(), Some(Box::new(|_| Ok(()))));
        sharing.wear(vec![json!({ "id": "top_hat" })]);
        sharing.emote(Some("black_hole"));
        wait_for(|| calls.lock().unwrap().iter().any(|c| c == "POST /v1/emote"));
        let calls = calls.lock().unwrap().clone();
        assert_eq!(
            calls
                .iter()
                .filter(|c| *c == "POST /v1/auth/finish")
                .count(),
            1
        );
        assert!(calls.contains(&"PUT /v1/wear".to_string()));
    }

    #[test]
    fn it_says_where_it_is_and_reads_friends_online_first() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let fake = Fake {
            calls: calls.clone(),
            expired: RefCell::new(false),
        };
        let sharing = Sharing::start(fake, "Steve".into(), Some(Box::new(|_| Ok(()))));
        sharing.set_server(Some("zedix.de".into()));
        wait_for(|| sharing.friends().list.is_some());
        let list = sharing.friends().list.unwrap();
        assert_eq!(list.friends[0].name, "Alex");
        assert!(list.friends[0].playing);
        assert_eq!(list.friends[0].server.as_deref(), Some("zedix.de"));
        assert_eq!(list.incoming, vec![("c".to_owned(), "Notch".to_owned())]);
        sharing.friend(FriendAsk::Request("Notch".into()));
        wait_for(|| sharing.friends().notice.is_some());
        assert_eq!(
            sharing.friends().notice.unwrap().0,
            "Request sent to Notch."
        );
        assert!(!sharing.friends().busy);
    }

    #[test]
    fn friends_without_an_account_say_why() {
        let fake = Fake {
            calls: Arc::default(),
            expired: RefCell::new(false),
        };
        let sharing = Sharing::start(fake, String::new(), None);
        wait_for(|| sharing.friends().problem.is_some());
        assert!(sharing.friends().list.is_none());
        assert_eq!(
            reason("HTTP 404: That player hasn't used Wisp yet."),
            "That player hasn't used Wisp yet."
        );
        assert_eq!(reason("timed out"), "Can't reach the Wisp server.");
    }

    #[test]
    fn it_reads_the_emotes_of_the_players_in_view() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let fake = Fake {
            calls,
            expired: RefCell::new(false),
        };
        let sharing = Sharing::start(fake, "Steve".into(), None);
        assert!(sharing.playing().is_empty());
        sharing.asking([0xab]);
        wait_for(|| !sharing.playing().is_empty());
        let p = &sharing.playing()[&0xab];
        assert_eq!(p.id, "black_hole");
        // It started two seconds before the answer.
        assert!(p.started.elapsed() >= Duration::from_millis(1900));
    }
}
