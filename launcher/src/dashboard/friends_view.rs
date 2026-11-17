//! Friends: a worker thread keeps this launcher signed in to the Wisp server, says where the
//! player is every half minute and brings the list back; the right column shows it, with
//! requests, adding by name, and Join for friends on a server.
use super::*;
use std::path::Path;
use std::time::Duration;
use wisp_launcher::friends::{Client, Failure, Snapshot};

/// Online: the green of the shell's presence dots.
const LIVE: u32 = 0x4ade80;

/// How often the launcher says where the player is (the server forgets it after 90 s).
const EVERY: Duration = Duration::from_secs(25);

pub(super) enum FriendCmd {
    /// Say where the player is and fetch the list.
    Sync,
    /// Another account plays now: sign in again.
    Reset,
    Presence(&'static str, Option<String>),
    Request(String),
    Accept(String),
    Decline(String),
    Remove(String),
    /// The Shop, on the same server and session: read the wallet again.
    Wallet,
    /// Buy an item (the server prices it and checks the coins), then wear it; the message to
    /// show when it worked.
    Buy(String, String),
    Equip(String, String),
    Unequip(String, String),
    /// Wear exactly these (the Locker's outfit).
    Outfit(Vec<String>, String),
    /// Support the creator with this code, or nobody.
    Support(Option<String>),
}

impl FriendCmd {
    fn shop(&self) -> bool {
        matches!(
            self,
            Self::Wallet
                | Self::Buy(..)
                | Self::Equip(..)
                | Self::Unequip(..)
                | Self::Outfit(..)
                | Self::Support(..)
        )
    }
}

pub(super) enum FriendsUpdate {
    List(Snapshot),
    /// Friends can't work now, and why (no server set, not signed in, server down).
    Unavailable(String),
    /// What an action did; `true` when it failed.
    Notice(String, bool),
    /// The wallet the server keeps.
    Wallet(wisp_launcher::friends::Wallet),
    /// What a Shop action did; `true` when it failed.
    ShopNotice(String, bool),
    /// The Shop can't work now, and why.
    ShopUnavailable(String),
}

/// Runs for as long as the launcher: one command at a time, a sync when nothing happened for a
/// while.
pub(super) fn worker(
    home: PathBuf,
    events: mpsc::Sender<Event>,
    commands: mpsc::Receiver<FriendCmd>,
) {
    let mut client: Option<Client> = None;
    let mut presence: (&'static str, Option<String>) = ("launcher", None);
    let mut command = FriendCmd::Sync;
    // The welcome gift and the daily bonus, collected once the server is reached.
    let mut collected = false;
    let send = |update: FriendsUpdate| events.send(Event::Friends(update)).is_ok();
    loop {
        if let FriendCmd::Presence(status, server) = &command {
            presence = (status, server.clone());
        }
        if matches!(command, FriendCmd::Reset) {
            client = None;
            collected = false;
        }
        let keep_going = match wisp_launcher::shop::service() {
            None => {
                send(FriendsUpdate::Unavailable(tr(
                    "Friends need the Wisp server. Set its address in Settings, General.",
                ))) && send(FriendsUpdate::ShopUnavailable(tr(
                    "The Shop needs the Wisp server: your coins and items are kept there.",
                )))
            }
            Some(base) => {
                if client.as_ref().is_none_or(|c| c.base() != base) {
                    client = Some(Client::new(&base));
                }
                let c = client.as_mut().expect("just made");
                if command.shop() {
                    run_shop(c, &home, &command, &send)
                } else {
                    let mut going = run(c, &home, &command, &presence, &send);
                    // Each sync reads the wallet too: a creator's share arrives by itself.
                    if going && matches!(command, FriendCmd::Sync | FriendCmd::Reset) {
                        going = if collected {
                            run_shop(c, &home, &FriendCmd::Wallet, &send)
                        } else {
                            collected = collect(c, &home, &send);
                            true
                        };
                    }
                    going
                }
            }
        };
        if !keep_going {
            return;
        }
        command = match commands.recv_timeout(EVERY) {
            Ok(next) => next,
            Err(mpsc::RecvTimeoutError::Timeout) => FriendCmd::Sync,
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        };
    }
}

fn sign_in(client: &mut Client, home: &Path) -> Result<(), String> {
    let account = auth::for_game(home, None, &|_| {})
        .map_err(|_| tr("Sign in with Microsoft to add friends."))?;
    let credentials = account.credentials();
    client.sign_in(credentials.name(), |id| credentials.join_server(id))
}

/// One command against the server, signing in (again) when needed. False when the launcher is
/// gone.
fn run(
    client: &mut Client,
    home: &Path,
    command: &FriendCmd,
    presence: &(&'static str, Option<String>),
    send: &dyn Fn(FriendsUpdate) -> bool,
) -> bool {
    let attempt = |c: &Client| match command {
        FriendCmd::Sync | FriendCmd::Reset | FriendCmd::Presence(..) => {
            c.presence(presence.0, presence.1.as_deref())
        }
        // The Shop's commands go to `run_shop`.
        FriendCmd::Wallet
        | FriendCmd::Buy(..)
        | FriendCmd::Equip(..)
        | FriendCmd::Unequip(..)
        | FriendCmd::Outfit(..)
        | FriendCmd::Support(..) => c.presence(presence.0, presence.1.as_deref()),
        FriendCmd::Request(name) => c.request(name),
        FriendCmd::Accept(uuid) => c.accept(uuid),
        FriendCmd::Decline(uuid) => c.decline(uuid),
        FriendCmd::Remove(uuid) => c.remove(uuid),
    };
    let mut result = Err(Failure::SignIn);
    for _ in 0..2 {
        if !client.signed_in() {
            if let Err(e) = sign_in(client, home) {
                return send(FriendsUpdate::Unavailable(e));
            }
        }
        result = attempt(client);
        if result != Err(Failure::SignIn) {
            break;
        }
        client.sign_out();
    }
    let action = !matches!(
        command,
        FriendCmd::Sync | FriendCmd::Reset | FriendCmd::Presence(..)
    );
    match result {
        Ok(snapshot) => {
            let notice = match command {
                FriendCmd::Request(name)
                    if snapshot
                        .friends
                        .iter()
                        .any(|f| f.name.eq_ignore_ascii_case(name)) =>
                {
                    Some(trf("You and {name} are friends now.", &[("name", name)]))
                }
                FriendCmd::Request(name) => {
                    Some(trf("Friend request sent to {name}.", &[("name", name)]))
                }
                FriendCmd::Accept(_) => Some(tr("Friend request accepted.")),
                _ => None,
            };
            send(FriendsUpdate::List(snapshot))
                && notice.is_none_or(|n| send(FriendsUpdate::Notice(n, false)))
        }
        Err(e) if action => send(FriendsUpdate::Notice(e.message(), true)),
        Err(e) => send(FriendsUpdate::Unavailable(e.message())),
    }
}

/// The welcome gift and today's bonus: true once the server answered.
fn collect(client: &mut Client, home: &Path, send: &dyn Fn(FriendsUpdate) -> bool) -> bool {
    if !client.signed_in() && sign_in(client, home).is_err() {
        return false;
    }
    match client.collect() {
        Ok((given, wallet)) => {
            send(FriendsUpdate::Wallet(wallet));
            if given > 0 {
                send(FriendsUpdate::ShopNotice(
                    trf(
                        "+{coins} Wisp Coins collected.",
                        &[("coins", &given.to_string())],
                    ),
                    false,
                ));
            }
            true
        }
        Err(Failure::SignIn) => {
            client.sign_out();
            false
        }
        Err(e) => {
            send(FriendsUpdate::ShopUnavailable(e.message()));
            false
        }
    }
}

/// One Shop command, signing in (again) when needed. False when the launcher is gone.
fn run_shop(
    client: &mut Client,
    home: &Path,
    command: &FriendCmd,
    send: &dyn Fn(FriendsUpdate) -> bool,
) -> bool {
    let attempt =
        |c: &Client| -> Result<(wisp_launcher::friends::Wallet, Option<String>), Failure> {
            match command {
                FriendCmd::Buy(id, done) => {
                    c.buy(id)?;
                    Ok((c.equip(id)?, Some(done.clone())))
                }
                FriendCmd::Equip(id, done) => Ok((c.equip(id)?, Some(done.clone()))),
                FriendCmd::Unequip(id, done) => Ok((c.unequip(id)?, Some(done.clone()))),
                FriendCmd::Outfit(items, done) => {
                    let mut wallet = c.wallet()?;
                    for id in wallet.equipped.clone() {
                        if !items.contains(&id) {
                            wallet = c.unequip(&id)?;
                        }
                    }
                    for id in items {
                        if !wallet.equipped.contains(id) {
                            wallet = c.equip(id)?;
                        }
                    }
                    Ok((wallet, Some(done.clone())))
                }
                FriendCmd::Support(code) => {
                    c.support(code.as_deref())?;
                    let wallet = c.wallet()?;
                    let done = match &wallet.supporting {
                        Some((name, _)) => trf(
                            "You support {name}: {percent} % off everything in the Shop.",
                            &[("name", name), ("percent", &wallet.discount.to_string())],
                        ),
                        None => tr("You don't support a creator now."),
                    };
                    Ok((wallet, Some(done)))
                }
                _ => Ok((c.wallet()?, None)),
            }
        };
    let mut result = Err(Failure::SignIn);
    for _ in 0..2 {
        if !client.signed_in() {
            if let Err(e) = sign_in(client, home) {
                return send(FriendsUpdate::ShopUnavailable(e));
            }
        }
        result = attempt(client);
        if result != Err(Failure::SignIn) {
            break;
        }
        client.sign_out();
    }
    match result {
        Ok((wallet, done)) => {
            send(FriendsUpdate::Wallet(wallet))
                && done.is_none_or(|d| send(FriendsUpdate::ShopNotice(d, false)))
        }
        Err(e) if matches!(command, FriendCmd::Wallet) => {
            send(FriendsUpdate::ShopUnavailable(e.message()))
        }
        Err(e) => send(FriendsUpdate::ShopNotice(tr(&e.message()), true)),
    }
}

impl Launcher {
    pub(super) fn friends_send(&self, command: FriendCmd) {
        let _ = self.friends_tx.send(command);
    }

    /// A friend's head, fetched once from their public Minecraft profile.
    pub(super) fn friend_head(&mut self, uuid: &str) {
        if self.friend_heads.contains_key(uuid) {
            return;
        }
        self.friend_heads.insert(uuid.to_owned(), None);
        let sender = self.sender.clone();
        let uuid = uuid.to_owned();
        std::thread::spawn(move || {
            let face = wisp_launcher::friends::skin_of(&uuid)
                .and_then(|(url, slim)| crate::account_art::fetch(&url, slim))
                .map(|art| art.face);
            let _ = sender.send(Event::FriendHead(uuid, face));
        });
    }

    /// A head with a presence dot at its corner.
    pub(super) fn head(&self, uuid: &str, name: &str, size: f32, presence: Option<bool>) -> Div {
        let s = self.ui_scale;
        let face = div()
            .size(px(size * s))
            .flex_none()
            .rounded(px(size * 0.22 * s))
            .overflow_hidden()
            .bg(rgba(0xffffff14))
            .flex()
            .items_center()
            .justify_center()
            .map(|d| match self.friend_heads.get(uuid).cloned().flatten() {
                Some(face) => d.child(img(face).size_full()),
                None => d
                    .text_size(px(size * 0.42 * s))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xc4c6cc))
                    .child(
                        name.chars()
                            .next()
                            .unwrap_or('?')
                            .to_uppercase()
                            .to_string(),
                    ),
            });
        div()
            .relative()
            .flex_none()
            .child(face)
            .when_some(presence, |d, on| {
                d.child(
                    div()
                        .absolute()
                        .right(px(-3. * s))
                        .bottom(px(-3. * s))
                        .p(px(2.5 * s))
                        .rounded_full()
                        .bg(rgb(0x121316))
                        .child(div().size(px(10. * s)).rounded_full().bg(rgb(if on {
                            LIVE
                        } else {
                            0x5b5d65
                        }))),
                )
            })
    }

    /// A small square button: white for the action that counts, dark glass otherwise.
    fn small_button(
        &self,
        id: impl Into<ElementId>,
        glyph: &'static str,
        bright: bool,
    ) -> Stateful<Div> {
        let s = self.ui_scale;
        div()
            .id(id)
            .size(px(34. * s))
            .flex_none()
            .rounded(px(9. * s))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .when(bright, |d| {
                d.bg(rgb(0xffffff)).hover(|d| d.bg(rgb(0xe4e5e8)))
            })
            .when(!bright, |d| {
                d.bg(rgba(0xffffff0f))
                    .border_1()
                    .border_color(rgba(0xffffff1a))
                    .hover(|d| d.bg(rgba(0xffffff1f)))
            })
            .child(icon(glyph, if bright { 0x0b0b0d } else { INK }).size(px(16. * s)))
    }

    /// A small white pill with a word: Join, Accept.
    fn pill_button(
        &self,
        id: impl Into<ElementId>,
        label: &str,
        glyph: &'static str,
    ) -> Stateful<Div> {
        let s = self.ui_scale;
        div()
            .id(id)
            .h(px(32. * s))
            .px(px(12. * s))
            .flex_none()
            .rounded(px(9. * s))
            .bg(rgb(0xffffff))
            .flex()
            .items_center()
            .gap(px(6. * s))
            .text_size(px(13. * s))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0x0b0b0d))
            .cursor_pointer()
            .hover(|d| d.bg(rgb(0xe4e5e8)))
            .child(icon(glyph, 0x0b0b0d).size(px(13. * s)))
            .child(tr(label))
    }

    /// A section's name in small capitals, with a count on the right.
    pub(super) fn side_heading(&self, label: &str) -> Div {
        self.side_heading_count(label, None)
    }

    fn side_heading_count(&self, label: &str, count: Option<String>) -> Div {
        let s = self.ui_scale;
        div()
            .mt(px(22. * s))
            .mb(px(10. * s))
            .flex()
            .items_center()
            .justify_between()
            .text_size(px(12. * s))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0x8e9099))
            .child(tr(label).to_uppercase())
            .when_some(count, |d, c| d.child(c))
    }

    /// A row in the panel: a glass plate on hover.
    fn side_row(&self, id: impl Into<ElementId>) -> Stateful<Div> {
        let s = self.ui_scale;
        div()
            .id(id)
            .px(px(10. * s))
            .py(px(9. * s))
            .rounded(px(12. * s))
            .flex()
            .items_center()
            .gap(px(12. * s))
            .hover(|d| d.bg(rgba(0xffffff0d)))
    }

    /// The panel under the player: add a friend, requests, friends, daily coins, servers.
    pub(super) fn side_sections(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = self.ui_scale;
        let busy = self.launching || self.child.is_some() || self.setup_busy;
        let collected = self.wallet.bonus_day == Some(wisp_launcher::shop::today());
        let mut column = div()
            .id("side-sections")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(14. * s))
            .pb(px(18. * s));
        match (&self.friends, &self.friends_state) {
            (None, Some(why)) => {
                column = column.child(
                    div()
                        .mt(px(14. * s))
                        .p(px(18. * s))
                        .rounded(px(14. * s))
                        .bg(rgba(0xffffff0a))
                        .border_1()
                        .border_color(rgba(0xffffff12))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(12. * s))
                        .child(
                            div()
                                .size(px(52. * s))
                                .rounded_full()
                                .bg(rgba(0xffffff12))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon("user-multiple-02", INK).size(px(24. * s))),
                        )
                        .child(
                            div()
                                .text_size(px(16. * s))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(tr("Play with friends")),
                        )
                        .child(
                            div()
                                .text_size(px(13. * s))
                                .line_height(px(19. * s))
                                .text_color(rgb(0xa9abb3))
                                .text_center()
                                .child(why.clone()),
                        )
                        .child(
                            self.pill_button("friends-open-settings", "Open settings", "gear")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.page = Page::Settings;
                                    cx.notify();
                                })),
                        ),
                );
            }
            (None, None) => {
                column = column.child(div().mt(px(16. * s)).child(quiet("Loading friends…")));
            }
            (Some(list), _) => {
                // Add by name.
                column = column
                    .child(self.side_heading("Add a friend"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8. * s))
                            .child(div().flex_1().min_w_0().child(self.friend_input.clone()))
                            .child(
                                self.small_button("friend-add", "user-add-01", true)
                                    .size(px(40. * s))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let name =
                                            this.friend_input.read(cx).value.trim().to_owned();
                                        if name.is_empty() {
                                            return;
                                        }
                                        this.friend_input.update(cx, |input, cx| input.set("", cx));
                                        this.friends_send(FriendCmd::Request(name));
                                    })),
                            ),
                    )
                    .when_some(self.friends_notice.clone(), |d, (text, bad)| {
                        d.child(
                            div()
                                .mt(px(8. * s))
                                .text_size(px(13. * s))
                                .text_color(rgb(if bad { 0xf08a96 } else { LIVE }))
                                .child(text),
                        )
                    });
                if let Some(state) = &self.friends_state {
                    column = column.child(
                        div()
                            .mt(px(8. * s))
                            .text_size(px(13. * s))
                            .text_color(rgb(0xf2c14e))
                            .child(state.clone()),
                    );
                }
                // Requests to answer.
                if !list.incoming.is_empty() {
                    column = column.child(
                        self.side_heading_count("Requests", Some(list.incoming.len().to_string())),
                    );
                    for (i, person) in list.incoming.iter().enumerate() {
                        let (yes, no) = (person.uuid.clone(), person.uuid.clone());
                        column = column.child(
                            div()
                                .id(("friend-request", i))
                                .mb(px(8. * s))
                                .px(px(12. * s))
                                .py(px(10. * s))
                                .rounded(px(12. * s))
                                .bg(rgba(0xffffff0f))
                                .border_1()
                                .border_color(rgba(0xffffff26))
                                .flex()
                                .items_center()
                                .gap(px(12. * s))
                                .child(self.head(&person.uuid, &person.name, 40., None))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .truncate()
                                                .text_size(px(15. * s))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(person.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .truncate()
                                                .text_size(px(12. * s))
                                                .text_color(rgb(0xa9abb3))
                                                .child(tr("Wants to be friends")),
                                        ),
                                )
                                .child(
                                    self.pill_button(("friend-accept", i), "Accept", "tick-02")
                                        .on_click(cx.listener(move |this, _, _, _| {
                                            this.friends_send(FriendCmd::Accept(yes.clone()))
                                        })),
                                )
                                .child(
                                    self.small_button(("friend-decline", i), "cancel-01", false)
                                        .size(px(32. * s))
                                        .on_click(cx.listener(move |this, _, _, _| {
                                            this.friends_send(FriendCmd::Decline(no.clone()))
                                        })),
                                ),
                        );
                    }
                }
                // Friends, online first.
                column = column.child(self.side_heading_count(
                    "Friends",
                    Some(format!("{} / {}", list.online(), list.friends.len())),
                ));
                if list.friends.is_empty() {
                    column = column.child(
                        div()
                            .px(px(4. * s))
                            .text_size(px(13. * s))
                            .line_height(px(19. * s))
                            .text_color(rgb(0xa9abb3))
                            .child(tr(
                                "No friends yet. Add someone by their Minecraft name above.",
                            )),
                    );
                }
                let mut order: Vec<_> = list.friends.iter().enumerate().collect();
                order.sort_by_key(|(_, f)| (!f.online, f.name.to_lowercase()));
                for (i, friend) in order {
                    let (line, colour) =
                        match (friend.online, friend.status.as_deref(), &friend.server) {
                            (true, Some("playing"), Some(server)) => {
                                (trf("Playing on {server}", &[("server", server)]), LIVE)
                            }
                            (true, Some("playing"), None) => (tr("Playing Wisp"), LIVE),
                            (true, _, _) => (tr("Online in launcher"), LIVE),
                            (false, _, _) if friend.last_seen > 0 => (
                                trf(
                                    "Last seen {when}",
                                    &[("when", &content::ago(friend.last_seen))],
                                ),
                                0x8e9099,
                            ),
                            _ => (tr("Offline"), 0x8e9099),
                        };
                    let join = friend.server.clone().filter(|_| friend.online);
                    let forget = friend.uuid.clone();
                    column = column.child(
                        self.side_row(("friend", i))
                            .group("friend-row")
                            .when(!friend.online, |d| d.opacity(0.6))
                            .child(self.head(&friend.uuid, &friend.name, 40., Some(friend.online)))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(15. * s))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(friend.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(12. * s))
                                            .text_color(rgb(colour))
                                            .child(line),
                                    ),
                            )
                            .when_some(join, |d, server| {
                                d.child(
                                    self.pill_button(("friend-join", i), "Join", "play")
                                        .when(busy, |d| d.opacity(0.4))
                                        .when(!busy, |d| {
                                            d.on_click(cx.listener(move |this, _, _, cx| {
                                                this.launch_with(
                                                    vec![
                                                        "--quickPlayMultiplayer".into(),
                                                        server.clone(),
                                                    ],
                                                    cx,
                                                )
                                            }))
                                        }),
                                )
                            })
                            .child(
                                div()
                                    .invisible()
                                    .group_hover("friend-row", |d| d.visible())
                                    .child(
                                        self.small_button(("friend-remove", i), "cancel-01", false)
                                            .size(px(30. * s))
                                            .on_click(cx.listener(move |this, _, _, _| {
                                                this.friends_send(FriendCmd::Remove(forget.clone()))
                                            })),
                                    ),
                            ),
                    );
                }
                // Requests sent and not answered yet.
                if !list.outgoing.is_empty() {
                    column = column.child(
                        self.side_heading_count("Sent", Some(list.outgoing.len().to_string())),
                    );
                    for (i, person) in list.outgoing.iter().enumerate() {
                        let cancel = person.uuid.clone();
                        column = column.child(
                            self.side_row(("friend-sent", i))
                                .child(self.head(&person.uuid, &person.name, 34., None))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .truncate()
                                                .text_size(px(14. * s))
                                                .child(person.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12. * s))
                                                .text_color(rgb(0x8e9099))
                                                .child(tr("Request sent")),
                                        ),
                                )
                                .child(
                                    self.small_button(("friend-cancel", i), "cancel-01", false)
                                        .size(px(30. * s))
                                        .on_click(cx.listener(move |this, _, _, _| {
                                            this.friends_send(FriendCmd::Remove(cancel.clone()))
                                        })),
                                ),
                        );
                    }
                }
            }
        }
        // Daily coins, then the saved servers.
        let column = column.child(self.side_heading("Daily reward")).child(
            div()
                .id("side-daily")
                .px(px(14. * s))
                .py(px(12. * s))
                .rounded(px(14. * s))
                .border_1()
                .border_color(rgba(0xf2c14e40))
                .bg(gpui::linear_gradient(
                    120.,
                    gpui::linear_color_stop(rgba(0xf2c14e26), 0.),
                    gpui::linear_color_stop(rgba(0xffffff08), 1.),
                ))
                .flex()
                .items_center()
                .gap(px(12. * s))
                .cursor_pointer()
                .hover(|d| d.border_color(rgba(0xf2c14e80)))
                .child(
                    div()
                        .size(px(38. * s))
                        .flex_none()
                        .rounded_full()
                        .bg(rgba(0xf2c14e26))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon("coin", 0xf2c14e).size(px(20. * s))),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(15. * s))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(trf(
                                    "{coins} coins",
                                    &[("coins", &self.wallet.coins.to_string())],
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(12. * s))
                                .text_color(rgb(0xa9abb3))
                                .child(tr(if collected {
                                    "+100 collected today, more tomorrow"
                                } else {
                                    "Open Wisp daily for 100 coins"
                                })),
                        ),
                )
                .child(icon("arrow", 0x8e9099).size(px(15. * s)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.page = Page::Shop;
                    cx.notify();
                })),
        );
        self.side_servers(column, busy, cx)
    }

    /// The saved servers (the featured one has its own card on Home), each with its live
    /// players and ping, joined with a click.
    fn side_servers(
        &self,
        column: Stateful<Div>,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let s = self.ui_scale;
        let servers: Vec<_> = self
            .content
            .servers
            .iter()
            .filter(|sv| !wisp_launcher::server_ping::is_featured(&sv.item.address))
            .take(5)
            .collect();
        if servers.is_empty() {
            return column;
        }
        let mut column = column.child(self.side_heading("Your servers"));
        for (i, server) in servers.into_iter().enumerate() {
            let address = server.item.address.clone();
            let (colour, line) = match self.pings.get(&server.item.address) {
                Some(Some(Ok(st))) => (
                    LIVE,
                    trf(
                        "{online} / {max} · {ping} ms",
                        &[
                            ("online", &st.online.to_string()),
                            ("max", &st.max.to_string()),
                            ("ping", &st.latency_ms.to_string()),
                        ],
                    ),
                ),
                Some(Some(Err(_))) => (0xd9534f, tr("Offline")),
                _ => (0x9a9ca4, tr("Checking…")),
            };
            column = column.child(
                self.side_row(("side-server", i))
                    .child(content::thumb(server.icon.as_ref(), "server", 40., s))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(15. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(server.item.name.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6. * s))
                                    .text_size(px(12. * s))
                                    .text_color(rgb(0xa9abb3))
                                    .child(
                                        div()
                                            .size(px(7. * s))
                                            .flex_none()
                                            .rounded_full()
                                            .bg(rgb(colour)),
                                    )
                                    .child(div().truncate().child(line)),
                            ),
                    )
                    .when(busy, |d| d.opacity(0.5))
                    .when(!busy, |d| {
                        d.cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.launch_with(
                                    vec!["--quickPlayMultiplayer".into(), address.clone()],
                                    cx,
                                )
                            }))
                    }),
            );
        }
        column
    }
}
