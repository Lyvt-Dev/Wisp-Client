//! The Wisp Menu's Friends page: the same friends as the launcher's right column, through the
//! Wisp server (`wisp_cosmetics::share`). Requests to accept, friends online with where they
//! play and a Join button, friends offline with when they were last seen, requests sent, and a
//! field to add someone by their Minecraft name.
use super::*;
use wisp_cosmetics::share::{Friend, Friends, FriendsState};

/// What the Friends page shows: the state as last answered (None: no Wisp server is set), and
/// the server this player is on, which needs no Join button.
#[derive(Clone, Copy, Default)]
pub struct FriendsView<'a> {
    pub state: Option<&'a FriendsState>,
    pub server: Option<&'a str>,
}

/// A click on a friend's or a request's button; the index is into the list's part it names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FriendClick {
    /// `incoming`.
    Accept,
    Decline,
    /// `friends`: join the server they play on.
    Join,
    /// `friends`: the first click asks, the second removes.
    Remove,
    /// `outgoing`: take the request back.
    Cancel,
}

const ROW: f32 = 54.0;
const HEAD: f32 = 34.0;
/// How long the answer to a request stays under the title, in seconds.
const NOTICE: u64 = 6;

/// A friend's colour, the same for the same name.
pub fn tint(name: &str) -> u32 {
    const TINTS: [u32; 8] = [
        0x8B6BFF, 0x4ADE80, 0xFFB454, 0x5CC8FF, 0xFF6B9A, 0xC084FC, 0x2DD4BF, 0xF97316,
    ];
    let h = name.bytes().fold(0u32, |h, b| {
        h.wrapping_mul(31)
            .wrapping_add(u32::from(b.to_ascii_lowercase()))
    });
    TINTS[h as usize % TINTS.len()]
}

/// "Last seen 3 h ago" from Unix seconds.
fn last_seen(at: i64) -> String {
    if at <= 0 {
        return lang::get_or("wisp.menu.friends.offline", "Offline");
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let s = (now - at).max(0);
    let ago = if s < 60 {
        lang::get_or("wisp.menu.friends.just_now", "just now")
    } else if s < 3600 {
        format!("{} min ago", s / 60)
    } else if s < 86400 {
        format!("{} h ago", s / 3600)
    } else {
        format!("{} d ago", s / 86400)
    };
    format!(
        "{} {ago}",
        lang::get_or("wisp.menu.friends.last_seen", "Last seen")
    )
}

/// Where an online friend is.
pub fn whereabouts(f: &Friend) -> String {
    match (&f.server, f.playing) {
        (Some(s), _) => format!(
            "{} {s}",
            lang::get_or("wisp.menu.friends.playing_on", "Playing on")
        ),
        (None, true) => lang::get_or("wisp.menu.friends.playing", "Playing Wisp"),
        (None, false) => lang::get_or("wisp.menu.friends.launcher", "Online in the launcher"),
    }
}

/// A part of the list.
enum Line<'a> {
    Heading(String),
    Incoming(usize, &'a str),
    Friend(usize, &'a Friend),
    Outgoing(usize, &'a str),
}
impl Line<'_> {
    fn height(&self) -> f32 {
        match self {
            Line::Heading(_) => HEAD,
            _ => ROW + 6.0,
        }
    }
}

impl Menu {
    /// The list as last drawn: the clicks' indices point into it.
    pub fn friends_shown(&self) -> Option<&Friends> {
        self.friends_shown.as_ref()
    }

    pub(super) fn draw_friends(
        &mut self,
        ui: &mut Ui,
        view: FriendsView,
        (mx, my, mw, mh): (f32, f32, f32, f32),
    ) {
        let title = lang::get_or("wisp.menu.friends", "Friends");
        ui.text(Face::Heading, 26.0, mx, my - 4.0, INK, &title);
        let list = view.state.and_then(|s| s.list.as_ref());
        self.friends_shown = list.cloned();
        let online = list.map_or(0, |l| l.friends.iter().filter(|f| f.online).count());
        let total = list.map_or(0, |l| l.friends.len());
        let summary = format!(
            "{online} {}  \u{b7}  {total} {}",
            lang::get_or("wisp.menu.friends.online", "online"),
            lang::get_or("wisp.menu.friends.friends", "friends"),
        );
        // The answer to the last request takes the summary's place for a moment.
        let notice = view
            .state
            .and_then(|s| s.notice.as_ref())
            .filter(|(_, at)| at.elapsed().as_secs() < NOTICE);
        match notice {
            Some((text, _)) => {
                let text = fit(ui, Face::Medium, 13.0, text, mw * 0.5);
                ui.text(Face::Medium, 13.0, mx, my + 32.0, ACCENT_SOFT, &text);
            }
            None if list.is_some() => {
                ui.text(Face::Regular, 13.0, mx, my + 32.0, MUTED, &summary);
            }
            None => {}
        }
        let Some(state) = view.state else {
            self.friends_empty(
                ui,
                (mx, my + 80.0, mw),
                &lang::get_or("wisp.menu.friends.no_server", "No Wisp server is set"),
                &lang::get_or(
                    "wisp.menu.friends.no_server_hint",
                    "Set its address in the launcher: Settings, General, Wisp server. Then start the game again.",
                ),
            );
            return;
        };
        if list.is_some() {
            self.draw_add_field(ui, state.busy, (mx, my, mw));
        }
        let Some(list) = list else {
            let (head, hint) = match &state.problem {
                Some(p) => (
                    p.clone(),
                    lang::get_or(
                        "wisp.menu.friends.problem_hint",
                        "Friends belong to your Minecraft account and live on the Wisp server.",
                    ),
                ),
                None => (
                    lang::get_or(
                        "wisp.menu.friends.loading",
                        "Connecting to the Wisp server\u{2026}",
                    ),
                    String::new(),
                ),
            };
            self.friends_empty(ui, (mx, my + 80.0, mw), &head, &hint);
            return;
        };
        let mut lines = Vec::new();
        if !list.incoming.is_empty() {
            lines.push(Line::Heading(format!(
                "{}  \u{b7}  {}",
                lang::get_or("wisp.menu.friends.requests", "Requests"),
                list.incoming.len()
            )));
            lines.extend(
                list.incoming
                    .iter()
                    .enumerate()
                    .map(|(i, (_, n))| Line::Incoming(i, n)),
            );
        }
        let (on, off): (Vec<_>, Vec<_>) =
            list.friends.iter().enumerate().partition(|(_, f)| f.online);
        if !on.is_empty() {
            lines.push(Line::Heading(format!(
                "{}  \u{b7}  {}",
                lang::get_or("wisp.menu.friends.online_heading", "Online"),
                on.len()
            )));
            lines.extend(on.into_iter().map(|(i, f)| Line::Friend(i, f)));
        }
        if !off.is_empty() {
            lines.push(Line::Heading(format!(
                "{}  \u{b7}  {}",
                lang::get_or("wisp.menu.friends.offline_heading", "Offline"),
                off.len()
            )));
            lines.extend(off.into_iter().map(|(i, f)| Line::Friend(i, f)));
        }
        if !list.outgoing.is_empty() {
            lines.push(Line::Heading(format!(
                "{}  \u{b7}  {}",
                lang::get_or("wisp.menu.friends.sent", "Sent"),
                list.outgoing.len()
            )));
            lines.extend(
                list.outgoing
                    .iter()
                    .enumerate()
                    .map(|(i, (_, n))| Line::Outgoing(i, n)),
            );
        }
        if lines.is_empty() {
            self.friends_empty(
                ui,
                (mx, my + 80.0, mw),
                &lang::get_or("wisp.menu.friends.none", "No friends yet"),
                &lang::get_or(
                    "wisp.menu.friends.none_hint",
                    "Type a Minecraft name above and press Add. They need to have opened Wisp once.",
                ),
            );
            return;
        }
        let top = my + 62.0;
        let view_h = mh - 62.0;
        let content: f32 = lines.iter().map(Line::height).sum();
        self.scroll_room = (content - view_h).max(0.0);
        self.scroll = self.scroll.min(self.scroll_room);
        let start = ui.mark();
        let hits_before = self.hits.len();
        let mut y = top - self.scroll;
        for line in &lines {
            let lh = line.height();
            if y + lh >= top && y <= top + view_h {
                self.draw_friend_line(ui, line, view.server, mx, y, mw);
            }
            y += lh;
        }
        ui.clip_since(start, mx - 4.0, top, mw + 8.0, view_h);
        let clip = ui.gui_rect(mx - 4.0, top, mw + 8.0, view_h);
        for (rect, _) in &mut self.hits[hits_before..] {
            let y0 = rect[1].max(clip[1]);
            let y1 = (rect[1] + rect[3]).min(clip[1] + clip[3]);
            rect[1] = y0;
            rect[3] = (y1 - y0).max(0.0);
        }
        if self.scroll_room > 0.0 {
            let track = view_h - 8.0;
            let thumb = (view_h / content * track).max(24.0);
            let ty = top + 4.0 + (track - thumb) * (self.scroll / self.scroll_room);
            ui.rounded(mx + mw + 10.0, ty, 3.0, thumb, 1.5, hex(0xFFFFFF, 40));
        }
    }

    /// The field to add a friend by name, and its button, in the header's right.
    fn draw_add_field(&mut self, ui: &mut Ui, busy: bool, (mx, my, mw): (f32, f32, f32)) {
        let label = if busy {
            lang::get_or("wisp.menu.friends.adding", "Sending\u{2026}")
        } else {
            lang::get_or("wisp.menu.friends.add", "Add")
        };
        let bw = ui.measure(Face::Medium, 13.0, &label) + 28.0;
        let (fw, h) = (240.0_f32.min(mw * 0.45), 36.0);
        let bx = mx + mw - bw;
        let (fx, fy) = (bx - 8.0 - fw, my + 4.0);
        ui.rounded(fx, fy, fw, h, 12.0, FIELD);
        if self.friend_focused {
            ui.border(fx, fy, fw, h, 12.0, ACCENT_SOFT);
        }
        // A person with a plus.
        ui.circle(fx + 17.0, fy + 14.0, 4.0, MUTED);
        ui.rounded(fx + 11.0, fy + 20.0, 12.0, 6.0, 3.0, MUTED);
        let empty = self.friend_input.is_empty();
        let shown = if empty {
            lang::get_or("wisp.menu.friends.add_hint", "Minecraft name")
        } else {
            self.friend_input.clone()
        };
        let text = fit(ui, Face::Regular, 13.0, &shown, fw - 44.0);
        let colour = if empty { FAINT } else { INK };
        let tw = ui.text_line(Face::Regular, 13.0, h, fx + 32.0, fy, colour, &text);
        if self.friend_focused && blink() {
            let cx = if empty { fx + 32.0 } else { fx + 33.0 + tw };
            ui.rounded(cx, fy + 10.0, 1.0, h - 20.0, 0.0, INK);
        }
        self.hit(ui, fx, fy, fw, h, Action::FriendField);
        let ready = !empty && !busy;
        let fill = match (ready, hover(ui, bx, fy, bw, h)) {
            (true, true) => ACCENT_HOVER,
            (true, false) => ACCENT,
            (false, _) => CARD_HOVER,
        };
        ui.rounded(bx, fy, bw, h, 12.0, fill);
        let colour = if ready { ON_ACCENT } else { MUTED };
        ui.text_line(Face::Medium, 13.0, h, bx + 14.0, fy, colour, &label);
        self.hit(ui, bx, fy, bw, h, Action::FriendAdd);
    }

    /// A plate in the middle of the page: why there is nothing to list, and what to do.
    fn friends_empty(&mut self, ui: &mut Ui, (mx, y, mw): (f32, f32, f32), head: &str, hint: &str) {
        let (w, h) = (mw.min(460.0), 150.0);
        let x = mx + (mw - w) / 2.0;
        ui.rounded(x, y, w, h, 16.0, CARD);
        ui.border(x, y, w, h, 16.0, STROKE);
        // Two people.
        let cx = x + w / 2.0;
        ui.circle(cx - 9.0, y + 32.0, 8.0, accent_alpha(150));
        ui.rounded(cx - 21.0, y + 43.0, 24.0, 12.0, 6.0, accent_alpha(150));
        ui.circle(cx + 9.0, y + 30.0, 9.0, ACCENT_SOFT);
        ui.rounded(cx - 4.0, y + 42.0, 26.0, 14.0, 7.0, ACCENT_SOFT);
        let head = fit(ui, Face::Medium, 16.0, head, w - 32.0);
        let hw = ui.measure(Face::Medium, 16.0, &head);
        ui.text(Face::Medium, 16.0, cx - hw / 2.0, y + 68.0, INK, &head);
        for (i, line) in wrap(ui, Face::Regular, 13.0, hint, w - 40.0, 3)
            .iter()
            .enumerate()
        {
            let lw = ui.measure(Face::Regular, 13.0, line);
            ui.text(
                Face::Regular,
                13.0,
                cx - lw / 2.0,
                y + 94.0 + i as f32 * 17.0,
                MUTED,
                line,
            );
        }
    }

    /// A small button; returns its left edge. `primary` fills it with the accent.
    #[allow(clippy::too_many_arguments)]
    fn friend_button(
        &mut self,
        ui: &mut Ui,
        right: f32,
        cy: f32,
        label: &str,
        style: u8,
        action: Action,
    ) -> f32 {
        let bw = ui.measure(Face::Medium, 12.0, label) + 22.0;
        let (bx, by, bh) = (right - bw, cy - 14.0, 28.0);
        let over = hover(ui, bx, by, bw, bh);
        let fill = match (style, over) {
            (1, false) => ACCENT,
            (1, true) => ACCENT_HOVER,
            (2, false) => hex(0xFF6B6B, 200),
            (2, true) => hex(0xFF6B6B, 255),
            (_, false) => FIELD,
            (_, true) => CARD_HOVER,
        };
        ui.rounded(bx, by, bw, bh, 9.0, fill);
        let colour = match style {
            0 => INK_SOFT,
            1 => ON_ACCENT,
            _ => INK,
        };
        ui.text_line(Face::Medium, 12.0, bh, bx + 11.0, by, colour, label);
        self.hit(ui, bx, by, bw, bh, action);
        bx
    }

    fn draw_friend_line(
        &mut self,
        ui: &mut Ui,
        line: &Line,
        server: Option<&str>,
        x: f32,
        y: f32,
        w: f32,
    ) {
        let (name, sub, sub_colour, online) = match line {
            Line::Heading(text) => {
                ui.text(
                    Face::Medium,
                    12.0,
                    x,
                    y + 12.0,
                    ACCENT_SOFT,
                    &text.to_uppercase(),
                );
                return;
            }
            Line::Incoming(_, n) => (
                *n,
                lang::get_or("wisp.menu.friends.wants", "Wants to be friends"),
                ACCENT_SOFT,
                None,
            ),
            Line::Friend(_, f) if f.online => (
                f.name.as_str(),
                whereabouts(f),
                hex(0x4ADE80, 230),
                Some(true),
            ),
            Line::Friend(_, f) => (f.name.as_str(), last_seen(f.last_seen), FAINT, Some(false)),
            Line::Outgoing(_, n) => (
                *n,
                lang::get_or("wisp.menu.friends.pending", "Request sent, waiting"),
                FAINT,
                None,
            ),
        };
        let hovered = hover(ui, x, y, w, ROW);
        ui.rounded(x, y, w, ROW, 14.0, if hovered { CARD_HOVER } else { CARD });
        // The avatar: the name's first letter on its colour, with a dot when online.
        let (ax, ay, r) = (x + 14.0 + 17.0, y + ROW / 2.0, 17.0);
        let alpha = if online == Some(false) { 110 } else { 255 };
        ui.circle(ax, ay, r, hex(tint(name), alpha));
        let initial: String = name
            .chars()
            .next()
            .map(|c| c.to_ascii_uppercase())
            .into_iter()
            .collect();
        let iw = ui.measure(Face::Heading, 15.0, &initial);
        ui.text_line(
            Face::Heading,
            15.0,
            2.0 * r,
            ax - iw / 2.0,
            ay - r,
            hex(0xFFFFFF, alpha),
            &initial,
        );
        if online == Some(true) {
            ui.circle(ax + 12.0, ay + 12.0, 6.0, PANEL);
            ui.circle(ax + 12.0, ay + 12.0, 4.0, hex(0x4ADE80, 255));
        }
        // The buttons from the right; the text gets what is left.
        let right = x + w - 12.0;
        let cy = y + ROW / 2.0;
        let mut left = right;
        match line {
            Line::Incoming(i, _) => {
                let decline = lang::get_or("wisp.menu.friends.decline", "Decline");
                left = self.friend_button(
                    ui,
                    left,
                    cy,
                    &decline,
                    0,
                    Action::Friend(FriendClick::Decline, *i),
                );
                let accept = lang::get_or("wisp.menu.friends.accept", "Accept");
                left = self.friend_button(
                    ui,
                    left - 6.0,
                    cy,
                    &accept,
                    1,
                    Action::Friend(FriendClick::Accept, *i),
                );
            }
            Line::Friend(i, f) => {
                let asking = self.confirm_remove == Some(*i);
                if hovered || asking {
                    let label = if asking {
                        lang::get_or("wisp.menu.friends.remove_sure", "Remove?")
                    } else {
                        lang::get_or("wisp.menu.friends.remove", "Remove")
                    };
                    let style = if asking { 2 } else { 0 };
                    left = self.friend_button(
                        ui,
                        left,
                        cy,
                        &label,
                        style,
                        Action::Friend(FriendClick::Remove, *i),
                    ) - 6.0;
                }
                let elsewhere = f
                    .server
                    .as_deref()
                    .is_some_and(|s| server.is_none_or(|here| !here.eq_ignore_ascii_case(s)));
                if f.online && elsewhere {
                    let join = lang::get_or("wisp.menu.friends.join", "Join");
                    left = self.friend_button(
                        ui,
                        left,
                        cy,
                        &join,
                        1,
                        Action::Friend(FriendClick::Join, *i),
                    );
                } else if f.online && f.server.is_some() {
                    let here = lang::get_or("wisp.menu.friends.here", "Same server");
                    let hw = ui.measure(Face::Medium, 12.0, &here);
                    left -= hw;
                    ui.text_line(Face::Medium, 12.0, ROW, left, y, hex(0x4ADE80, 230), &here);
                }
            }
            Line::Outgoing(i, _) => {
                let cancel = lang::get_or("wisp.menu.friends.cancel", "Cancel");
                left = self.friend_button(
                    ui,
                    left,
                    cy,
                    &cancel,
                    0,
                    Action::Friend(FriendClick::Cancel, *i),
                );
            }
            Line::Heading(_) => {}
        }
        let tx = x + 14.0 + 2.0 * r + 12.0;
        let room = (left - 12.0 - tx).max(20.0);
        let name = fit(ui, Face::Medium, 15.0, name, room);
        let colour = if online == Some(false) { INK_SOFT } else { INK };
        ui.text(Face::Medium, 15.0, tx, y + 9.0, colour, &name);
        let sub = fit(ui, Face::Regular, 12.0, &sub, room);
        ui.text(Face::Regular, 12.0, tx, y + 30.0, sub_colour, &sub);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friends_keep_their_colour_and_say_where_they_are() {
        assert_eq!(tint("Alex"), tint("alex"));
        let mut f = Friend {
            name: "Alex".into(),
            online: true,
            playing: true,
            server: Some("zedix.de".into()),
            ..Friend::default()
        };
        assert!(whereabouts(&f).ends_with("zedix.de"));
        f.server = None;
        assert!(whereabouts(&f).contains("Wisp"));
        assert!(last_seen(0).contains("Offline") || !last_seen(0).is_empty());
    }
}
