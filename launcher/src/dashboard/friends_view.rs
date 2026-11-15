//! Friends: a worker thread keeps this launcher signed in to the Wisp server, says where the
//! player is every half minute and brings the list back; the right column shows it, with
//! requests, adding by name, and Join for friends on a server.
use super::reference_views::luminous;
use super::*;
use std::path::Path;
use std::time::Duration;
use wisp_launcher::friends::{Client, Failure, Snapshot};

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
}

pub(super) enum FriendsUpdate {
    List(Snapshot),
    /// Friends can't work now, and why (no server set, not signed in, server down).
    Unavailable(String),
    /// What an action did; `true` when it failed.
    Notice(String, bool),
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
    let send = |update: FriendsUpdate| events.send(Event::Friends(update)).is_ok();
    loop {
        if let FriendCmd::Presence(status, server) = &command {
            presence = (status, server.clone());
        }
        if matches!(command, FriendCmd::Reset) {
            client = None;
        }
        let keep_going = match wisp_launcher::shop::service() {
            None => send(FriendsUpdate::Unavailable(tr(
                "Friends need the Wisp server. Set its address in Settings, General.",
            ))),
            Some(base) => {
                if client.as_ref().is_none_or(|c| c.base() != base) {
                    client = Some(Client::new(&base));
                }
                let c = client.as_mut().expect("just made");
                run(c, &home, &command, &presence, &send)
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

    fn head(&self, uuid: &str, name: &str, size: f32) -> Div {
        let s = self.ui_scale;
        div()
            .size(px(size * s))
            .flex_none()
            .rounded(px(5. * s))
            .overflow_hidden()
            .bg(rgb(tone(0x2a2b30)))
            .flex()
            .items_center()
            .justify_center()
            .map(|d| match self.friend_heads.get(uuid).cloned().flatten() {
                Some(face) => d.child(img(face).size_full()),
                None => d
                    .text_size(px(size * 0.45 * s))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MUTED))
                    .child(
                        name.chars()
                            .next()
                            .unwrap_or('?')
                            .to_uppercase()
                            .to_string(),
                    ),
            })
    }

    fn small_button(
        &self,
        id: impl Into<ElementId>,
        glyph: &'static str,
        bright: bool,
    ) -> Stateful<Div> {
        let s = self.ui_scale;
        div()
            .id(id)
            .size(px(26. * s))
            .flex_none()
            .rounded(px(7. * s))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .when(bright, |d| d.bg(luminous()).hover(|d| d.opacity(0.85)))
            .when(!bright, |d| {
                d.bg(rgb(tone(0x232429)))
                    .hover(|d| d.bg(rgb(tone(0x2c2e33))))
            })
            .child(icon(glyph, if bright { INK } else { MUTED }).size(px(13. * s)))
    }

    pub(super) fn side_heading(&self, label: &str) -> Div {
        let s = self.ui_scale;
        div()
            .mt(px(16. * s))
            .mb(px(8. * s))
            .text_size(px(12. * s))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(tone(0x7d808a)))
            .child(tr(label).to_uppercase())
    }

    /// The right column under the player: add a friend, requests, friends, daily coins.
    pub(super) fn side_sections(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = self.ui_scale;
        let busy = self.launching || self.child.is_some() || self.setup_busy;
        let collected = self.wallet.bonus_day == Some(wisp_launcher::shop::today());
        let mut column = div()
            .id("side-sections")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .mt(px(8. * s))
            .px(px(16. * s))
            .pb(px(16. * s))
            .border_t_1()
            .border_color(rgb(tone(RULE)));
        match (&self.friends, &self.friends_state) {
            (None, Some(why)) => {
                column = column.child(
                    div()
                        .mt(px(16. * s))
                        .p(px(12. * s))
                        .rounded(px(10. * s))
                        .border_1()
                        .border_color(rgb(tone(0x2b2c32)))
                        .flex()
                        .gap(px(10. * s))
                        .child(icon("user-multiple-02", tone(0x6b6e77)).size(px(22. * s)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(12. * s))
                                .line_height(px(17. * s))
                                .text_color(rgb(MUTED))
                                .child(why.clone()),
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
                            .gap(px(6. * s))
                            .child(div().flex_1().min_w_0().child(self.friend_input.clone()))
                            .child(
                                self.small_button("friend-add", "user-add-01", true)
                                    .size(px(36. * s))
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
                                .mt(px(6. * s))
                                .text_size(px(12. * s))
                                .text_color(rgb(if bad { 0xf08a96 } else { accent_tone(0x6fe792) }))
                                .child(text),
                        )
                    });
                if let Some(state) = &self.friends_state {
                    column = column.child(
                        div()
                            .mt(px(6. * s))
                            .text_size(px(12. * s))
                            .text_color(rgb(0xf2c14e))
                            .child(state.clone()),
                    );
                }
                // Requests to answer.
                if !list.incoming.is_empty() {
                    column = column.child(self.side_heading("Requests"));
                    for (i, person) in list.incoming.iter().enumerate() {
                        let (yes, no) = (person.uuid.clone(), person.uuid.clone());
                        column = column.child(
                            div()
                                .id(("friend-request", i))
                                .p(px(8. * s))
                                .mb(px(6. * s))
                                .rounded(px(9. * s))
                                .border_1()
                                .border_color(rgb(accent_tone(0x2f9e52)))
                                .bg(rgba(accent_tone_a(0x3ccf6a12)))
                                .flex()
                                .items_center()
                                .gap(px(10. * s))
                                .child(self.head(&person.uuid, &person.name, 30.))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .truncate()
                                                .text_size(px(14. * s))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(person.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11. * s))
                                                .text_color(rgb(MUTED))
                                                .child(tr("Wants to be friends")),
                                        ),
                                )
                                .child(
                                    self.small_button(
                                        ("friend-accept", i),
                                        "checkmark-circle-01",
                                        true,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, _, _| {
                                            this.friends_send(FriendCmd::Accept(yes.clone()))
                                        },
                                    )),
                                )
                                .child(
                                    self.small_button(("friend-decline", i), "cancel-01", false)
                                        .on_click(cx.listener(move |this, _, _, _| {
                                            this.friends_send(FriendCmd::Decline(no.clone()))
                                        })),
                                ),
                        );
                    }
                }
                // Friends, online first.
                column = column.child(self.side_heading(&trf(
                    "Friends ({online}/{all})",
                    &[
                        ("online", &list.online().to_string()),
                        ("all", &list.friends.len().to_string()),
                    ],
                )));
                if list.friends.is_empty() {
                    column = column.child(quiet(
                        "No friends yet. Add someone by their Minecraft name above.",
                    ));
                }
                for (i, friend) in list.friends.iter().enumerate() {
                    let (line, colour) =
                        match (friend.online, friend.status.as_deref(), &friend.server) {
                            (true, Some("playing"), Some(server)) => (
                                trf("Playing on {server}", &[("server", server)]),
                                accent_tone(0x6fe792),
                            ),
                            (true, Some("playing"), None) => {
                                (tr("Playing Wisp"), accent_tone(0x6fe792))
                            }
                            (true, _, _) => (tr("Online in launcher"), accent_tone(0x6fe792)),
                            (false, _, _) if friend.last_seen > 0 => (
                                trf(
                                    "Last seen {when}",
                                    &[("when", &content::ago(friend.last_seen))],
                                ),
                                MUTED,
                            ),
                            _ => (tr("Offline"), MUTED),
                        };
                    let join = friend.server.clone().filter(|_| friend.online);
                    let forget = friend.uuid.clone();
                    column = column.child(
                        div()
                            .id(("friend", i))
                            .group("friend-row")
                            .px(px(6. * s))
                            .py(px(6. * s))
                            .rounded(px(8. * s))
                            .flex()
                            .items_center()
                            .gap(px(10. * s))
                            .hover(|d| d.bg(rgb(tone(0x1c1d21))))
                            .when(!friend.online, |d| d.opacity(0.7))
                            .child(
                                div()
                                    .relative()
                                    .child(self.head(&friend.uuid, &friend.name, 32.))
                                    .child(
                                        div()
                                            .absolute()
                                            .right(px(-3. * s))
                                            .bottom(px(-3. * s))
                                            .p(px(2. * s))
                                            .rounded_full()
                                            .bg(rgb(tone(0x141518)))
                                            .child(div().size(px(8. * s)).rounded_full().bg(rgb(
                                                if friend.online {
                                                    accent_tone(0x3ccf6a)
                                                } else {
                                                    0x6b6d75
                                                },
                                            ))),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(14. * s))
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
                                    self.small_button(("friend-join", i), "play", true)
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
                                            .on_click(cx.listener(move |this, _, _, _| {
                                                this.friends_send(FriendCmd::Remove(forget.clone()))
                                            })),
                                    ),
                            ),
                    );
                }
                // Requests sent and not answered yet.
                if !list.outgoing.is_empty() {
                    column = column.child(self.side_heading("Sent"));
                    for (i, person) in list.outgoing.iter().enumerate() {
                        let cancel = person.uuid.clone();
                        column = column.child(
                            div()
                                .id(("friend-sent", i))
                                .px(px(6. * s))
                                .py(px(5. * s))
                                .flex()
                                .items_center()
                                .gap(px(10. * s))
                                .child(self.head(&person.uuid, &person.name, 26.))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .truncate()
                                                .text_size(px(13. * s))
                                                .child(person.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11. * s))
                                                .text_color(rgb(MUTED))
                                                .child(tr("Request sent")),
                                        ),
                                )
                                .child(
                                    self.small_button(("friend-cancel", i), "cancel-01", false)
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
                .p(px(12. * s))
                .rounded(px(10. * s))
                .border_1()
                .border_color(rgb(tone(0x2b2c32)))
                .bg(gpui::linear_gradient(
                    135.,
                    gpui::linear_color_stop(rgba(0xf2c14e1f), 0.),
                    gpui::linear_color_stop(rgba(tone_a(0x18191dff)), 1.),
                ))
                .flex()
                .items_center()
                .gap(px(12. * s))
                .cursor_pointer()
                .hover(|d| d.border_color(rgb(0x6b5a2a)))
                .child(icon("coin", 0xf2c14e).size(px(20. * s)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(14. * s))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(trf(
                                    "{coins} coins",
                                    &[("coins", &self.wallet.coins.to_string())],
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(12. * s))
                                .text_color(rgb(MUTED))
                                .child(tr(if collected {
                                    "+100 collected today, more tomorrow"
                                } else {
                                    "Open Wisp daily for 100 coins"
                                })),
                        ),
                )
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
                    accent_tone(0x3ccf6a),
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
                div()
                    .id(("side-server", i))
                    .px(px(6. * s))
                    .py(px(6. * s))
                    .rounded(px(8. * s))
                    .flex()
                    .items_center()
                    .gap(px(10. * s))
                    .hover(|d| d.bg(rgb(tone(0x1c1d21))))
                    .child(content::thumb(server.icon.as_ref(), "server", 30., s))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(14. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(server.item.name.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6. * s))
                                    .text_size(px(12. * s))
                                    .text_color(rgb(MUTED))
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
