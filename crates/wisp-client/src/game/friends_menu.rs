//! Friends in the game: the Wisp Menu's Friends tab (`wisp_client::wisp_menu`), Join to a
//! friend's server, where this player is for their friends, and a small notice when a friend
//! comes online or asks to be friends. The list lives on the cosmetics thread
//! (`wisp_cosmetics::share`), signed in with the same Mojang check as the cosmetics.
use super::Game;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use wisp_client::{
    hud::HudBatch,
    lang,
    ui_theme::{Face, Ui, color::hex},
    wisp_menu::{FriendClick, friend_tint, friend_whereabouts},
};
use wisp_cosmetics::share::{FriendAsk, FriendsState};

/// How long a friend notice shows, and its slide in and fade out.
const TOAST: Duration = Duration::from_secs(5);
const SLIDE: f32 = 0.25;
const FADE: f32 = 0.4;
/// Notices shown at once.
const TOASTS: usize = 3;

/// A notice about a friend.
pub(super) struct FriendToast {
    name: String,
    title: String,
    line: String,
    at: Instant,
}

/// Friends in the game.
#[derive(Default)]
pub(super) struct FriendsGame {
    /// A server to join once the current world is left (Join in the Friends tab).
    join_after: Option<String>,
    /// The server last told to friends.
    told: Option<Option<String>>,
    /// Friends online and requests waiting at the last list, to notice what is new.
    seen: Option<(HashSet<String>, HashSet<String>)>,
    toasts: Vec<FriendToast>,
}

impl Game<'_> {
    /// The friends as last answered (None: no Wisp server, so no friends).
    pub(super) fn friends_state(&self) -> Option<FriendsState> {
        self.cosmetics.sharing.as_ref().map(|s| s.friends())
    }

    /// The server this player is on, as friends see it (None: singleplayer or the title).
    pub(super) fn friends_server(&self) -> Option<String> {
        if self.title.is_some() || self.remote.is_none() || self.has_singleplayer_server() {
            return None;
        }
        self.server_address()
            .map(|a| wisp_client::net::shown_address(a).to_string())
    }

    /// Once a frame: tells friends where this player is, joins a friend's server after leaving
    /// the world, and notices friends coming online and new requests.
    pub(super) fn friends_frame(&mut self) {
        if self.title.is_some()
            && self.pending_save.is_none()
            && let Some(address) = self.wisp_friends.join_after.take()
        {
            let name = self.options.player_name();
            if let Err(error) = self.connect_server(&address, &name) {
                log::warn!("Wisp: could not join a friend's server: {error}");
            }
        }
        let Some(sharing) = &self.cosmetics.sharing else {
            return;
        };
        let server = self.friends_server();
        if self.wisp_friends.told.as_ref() != Some(&server) {
            sharing.set_server(server.clone());
            // A new server: friends hear of it now, not at the next round.
            if self.wisp_friends.told.is_some() {
                sharing.friend(FriendAsk::Refresh);
            }
            self.wisp_friends.told = Some(server);
        }
        let Some(list) = sharing.friends().list else {
            return;
        };
        let online: HashSet<String> = list
            .friends
            .iter()
            .filter(|f| f.online)
            .map(|f| f.uuid.clone())
            .collect();
        let waiting: HashSet<String> = list.incoming.iter().map(|(u, _)| u.clone()).collect();
        if let Some((was_online, was_waiting)) = &self.wisp_friends.seen {
            let now = Instant::now();
            for f in list
                .friends
                .iter()
                .filter(|f| f.online && !was_online.contains(&f.uuid))
            {
                self.wisp_friends.toasts.push(FriendToast {
                    name: f.name.clone(),
                    title: format!(
                        "{} {}",
                        f.name,
                        lang::get_or("wisp.friends.toast.online", "is online")
                    ),
                    line: friend_whereabouts(f),
                    at: now,
                });
            }
            for (_, name) in list
                .incoming
                .iter()
                .filter(|(u, _)| !was_waiting.contains(u))
            {
                self.wisp_friends.toasts.push(FriendToast {
                    name: name.clone(),
                    title: format!(
                        "{name} {}",
                        lang::get_or("wisp.friends.toast.request", "wants to be friends")
                    ),
                    line: lang::get_or("wisp.friends.toast.open", "Right Shift, Friends to answer"),
                    at: now,
                });
            }
            let extra = self.wisp_friends.toasts.len().saturating_sub(TOASTS);
            self.wisp_friends.toasts.drain(..extra);
        }
        self.wisp_friends.seen = Some((online, waiting));
        self.wisp_friends.toasts.retain(|t| t.at.elapsed() < TOAST);
    }

    /// Join in the Friends tab: leaves this world (saving singleplayer) and joins theirs.
    fn join_friend(&mut self, address: String) {
        self.close_wisp_menu();
        if self.title.is_some() {
            let name = self.options.player_name();
            if let Err(error) = self.connect_server(&address, &name) {
                log::warn!("Wisp: could not join a friend's server: {error}");
            }
            return;
        }
        self.wisp_friends.join_after = Some(address);
        if let Err(error) = self.leave_server(None) {
            log::warn!("Wisp: could not leave for a friend's server: {error}");
            self.wisp_friends.join_after = None;
        }
    }

    /// The Friends tab's buttons and its Add.
    pub(super) fn friends_click(&mut self, click: Option<(FriendClick, usize)>) {
        let Some(sharing) = &self.cosmetics.sharing else {
            return;
        };
        let m = &mut self.extras.menu;
        let Some((click, i)) = click else {
            // Add.
            let name = m.friend_input.trim().to_owned();
            if !name.is_empty() && !self.friends_state().is_some_and(|s| s.busy) {
                sharing.friend(FriendAsk::Request(name));
                self.extras.menu.friend_input.clear();
            }
            return;
        };
        let Some(list) = m.friends_shown() else {
            return;
        };
        let incoming = list.incoming.get(i).map(|(u, _)| u.clone());
        let outgoing = list.outgoing.get(i).map(|(u, _)| u.clone());
        let friend = list.friends.get(i).cloned();
        match click {
            FriendClick::Accept => incoming
                .into_iter()
                .for_each(|u| sharing.friend(FriendAsk::Accept(u))),
            FriendClick::Decline => incoming
                .into_iter()
                .for_each(|u| sharing.friend(FriendAsk::Decline(u))),
            FriendClick::Cancel => outgoing
                .into_iter()
                .for_each(|u| sharing.friend(FriendAsk::Remove(u))),
            FriendClick::Remove => {
                if m.confirm_remove == Some(i) {
                    m.confirm_remove = None;
                    if let Some(f) = friend {
                        sharing.friend(FriendAsk::Remove(f.uuid));
                    }
                } else {
                    m.confirm_remove = Some(i);
                }
            }
            FriendClick::Join => {
                if let Some(server) = friend.and_then(|f| f.server) {
                    self.join_friend(server);
                }
            }
        }
    }

    /// Notices about friends, sliding in at the top right under vanilla's toasts' corner.
    pub(super) fn draw_friend_toasts(&self, batch: &mut HudBatch) {
        if self.wisp_friends.toasts.is_empty() {
            return;
        }
        let mut ui = Ui::new(&self.gui.atlas, batch, (-1.0, -1.0));
        let (w, h, gap) = (250.0, 50.0, 8.0);
        for (i, t) in self.wisp_friends.toasts.iter().enumerate() {
            let age = t.at.elapsed().as_secs_f32();
            let left = TOAST.as_secs_f32() - age;
            let slide = (age / SLIDE).min(1.0);
            let ease = 1.0 - (1.0 - slide).powi(3);
            let alpha = (left / FADE).clamp(0.0, 1.0);
            let a = |v: u8| (f32::from(v) * alpha) as u8;
            let x = ui.w - 12.0 - w * ease;
            let y = 12.0 + i as f32 * (h + gap);
            ui.rounded(x, y, w, h, 14.0, hex(0x15112E, a(240)));
            ui.border(x, y, w, h, 14.0, hex(0xFFFFFF, a(26)));
            ui.rounded(
                x,
                y + 10.0,
                3.0,
                h - 20.0,
                1.5,
                hex(wisp_theme::glass::ACCENT, a(255)),
            );
            let (cx, cy, r) = (x + 14.0 + 15.0, y + h / 2.0, 15.0);
            ui.circle(cx, cy, r, hex(friend_tint(&t.name), a(255)));
            let initial: String = t
                .name
                .chars()
                .next()
                .map(|c| c.to_ascii_uppercase())
                .into_iter()
                .collect();
            let iw = ui.measure(Face::Heading, 14.0, &initial);
            ui.text_line(
                Face::Heading,
                14.0,
                2.0 * r,
                cx - iw / 2.0,
                cy - r,
                hex(0xFFFFFF, a(255)),
                &initial,
            );
            let tx = x + 14.0 + 2.0 * r + 10.0;
            let room = w - (tx - x) - 12.0;
            let title = shorten(&ui, Face::Medium, 13.0, &t.title, room);
            ui.text(
                Face::Medium,
                13.0,
                tx,
                y + 8.0,
                hex(0xFFFFFF, a(255)),
                &title,
            );
            let line = shorten(&ui, Face::Regular, 11.0, &t.line, room);
            ui.text(
                Face::Regular,
                11.0,
                tx,
                y + 27.0,
                hex(0xFFFFFF, a(160)),
                &line,
            );
        }
        ui.finish();
    }
}

/// The text cut with an ellipsis to fit `max` dp.
fn shorten(ui: &Ui, face: Face, size: f32, text: &str, max: f32) -> String {
    if ui.measure(face, size, text) <= max {
        return text.to_owned();
    }
    let mut s = text.to_owned();
    while !s.is_empty() && ui.measure(face, size, &format!("{s}\u{2026}")) > max {
        s.pop();
    }
    format!("{}\u{2026}", s.trim_end())
}
