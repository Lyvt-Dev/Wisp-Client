//! Resource packs (which are on in the game, in their order, and what else is in the folder) and
//! Servers (the multiplayer list with live status and one-click Join).
use super::*;
use gpui::AnyElement;
use std::time::Instant;
use wisp_launcher::{local_content, server_ping};

/// How long server statuses stay fresh on the Servers page.
const PING_FRESH: Duration = Duration::from_secs(45);
const PING_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_PINGS: usize = 40;

fn switch(on: bool, s: f32) -> Div {
    div()
        .w(px(38. * s))
        .h(px(22. * s))
        .flex_none()
        .rounded_full()
        .p(px(3. * s))
        .when(on, |d| d.bg(rgb(0xffffff)).justify_end())
        .when(!on, |d| d.bg(rgb(tone(0x2c2e33))))
        .flex()
        .child(
            div()
                .size(px(16. * s))
                .rounded_full()
                .bg(rgb(if on { 0x0b0b0d } else { 0x80838c })),
        )
}

impl Launcher {
    /// The page's head in the shell's style; Resource packs also gets Browse's instance
    /// picker and its switch between Mods, Shaders and Resource packs.
    fn page_header(
        &self,
        title: &str,
        summary: String,
        actions: Vec<Stateful<Div>>,
        cx: &mut Context<Self>,
    ) -> Div {
        let s = self.ui_scale;
        let browse = self.page == Page::Packs;
        let back = if browse {
            ("Back to Instances", Page::Instances)
        } else {
            ("Back to Home", Page::Home)
        };
        let title = if browse {
            "Browse Resource packs"
        } else {
            title
        };
        div()
            .flex()
            .flex_col()
            .gap(px(18. * s))
            .child(self.page_head(
                back,
                title,
                &summary,
                Some(div().flex().gap(px(10. * s)).children(actions)),
                cx,
            ))
            .when(browse, |d| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12. * s))
                        .child(self.browse_for(cx))
                        .child(div().flex_1())
                        .child(self.browse_kinds(cx)),
                )
            })
    }

    fn page_button(
        &self,
        id: &'static str,
        label: &str,
        glyph: &'static str,
        bright: bool,
    ) -> Stateful<Div> {
        let s = self.ui_scale;
        div()
            .id(id)
            .h(px(38. * s))
            .px(px(14. * s))
            .rounded(px(9. * s))
            .flex()
            .items_center()
            .gap(px(8. * s))
            .text_size(px(14. * s))
            .cursor_pointer()
            .when(bright, |d| {
                d.bg(rgb(0xffffff))
                    .text_color(rgb(0x0b0b0d))
                    .font_weight(FontWeight::SEMIBOLD)
                    .hover(|d| d.bg(rgb(0xe4e5e8)))
            })
            .when(!bright, |d| {
                d.bg(rgba(0x16171acc))
                    .border_1()
                    .border_color(rgba(0xffffff1f))
                    .hover(|d| d.bg(rgba(0xffffff1a)))
            })
            .child(icon(glyph, if bright { 0x0b0b0d } else { INK }).size(px(15. * s)))
            .child(tr(label))
    }

    fn section_title(&self, glyph: &'static str, title: &str, count: usize, note: &str) -> Div {
        let s = self.ui_scale;
        div()
            .flex()
            .items_center()
            .gap(px(10. * s))
            .child(icon(glyph, accent_tone(ACCENT)).size(px(18. * s)))
            .child(
                div()
                    .text_size(px(19. * s))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr(title)),
            )
            .child(
                div()
                    .text_size(px(13. * s))
                    .text_color(rgb(MUTED))
                    .child(count.to_string()),
            )
            .child(
                div()
                    .text_size(px(13. * s))
                    .text_color(rgb(MUTED))
                    .child(tr(note)),
            )
            .child(
                div()
                    .flex_1()
                    .h(px(1.))
                    .ml(px(8. * s))
                    .bg(rgb(tone(0x26272c))),
            )
    }

    fn running_notice(&self, what: &str) -> Div {
        let s = self.ui_scale;
        div()
            .px(px(14. * s))
            .py(px(10. * s))
            .rounded(px(10. * s))
            .border_1()
            .border_color(rgba(0xf2c14e55))
            .bg(rgba(0xf2c14e14))
            .flex()
            .items_center()
            .gap(px(10. * s))
            .text_size(px(14. * s))
            .child(icon("info", 0xf2c14e).size(px(16. * s)))
            .child(tr(what))
    }

    fn toggle_pack(&mut self, file: String, on: bool, cx: &mut Context<Self>) {
        if self.mods_locked() {
            return;
        }
        let result = self
            .settings
            .current()
            .ok_or_else(|| tr("Select an instance first."))
            .and_then(|p| local_content::set_pack_enabled(&p.game_home, &file, on));
        match result {
            Ok(()) => {
                self.failed = false;
                self.status = tr("Resource packs saved. They load when Minecraft starts.");
            }
            Err(e) => self.error(e),
        }
        self.content.invalidate();
        cx.notify();
    }

    pub(super) fn packs_page(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let locked = self.mods_locked();
        let Some(profile) = self.settings.current() else {
            return self.empty_state(
                "layers",
                "No instance yet",
                "Create an instance first, then add resource packs to it.",
            );
        };
        let enabled = &self.content.enabled_packs;
        let packs = &self.content.packs;
        let is_on = |file: &str| enabled.iter().any(|e| e == file);
        let on_count = packs.iter().filter(|p| is_on(&p.item.file)).count();
        let mut summary = trf(
            if packs.len() == 1 {
                "{count} pack, {on} on for {instance}. Changes apply when Minecraft starts."
            } else {
                "{count} packs, {on} on for {instance}. Changes apply when Minecraft starts."
            },
            &[
                ("count", &packs.len().to_string()),
                ("on", &on_count.to_string()),
                ("instance", &profile.name),
            ],
        );
        if self.content.other_files > 0 {
            summary.push_str(&trf(
                " {count} other files in the folder are ignored.",
                &[("count", &self.content.other_files.to_string())],
            ));
        }
        let name_of = |file: &str| file.strip_suffix(".zip").unwrap_or(file).to_owned();
        // On packs, highest priority (drawn on top) first, as the game's list shows them.
        let active: Vec<_> = enabled
            .iter()
            .rev()
            .filter_map(|file| packs.iter().find(|p| p.item.file == *file))
            .collect();
        let rows: Vec<AnyElement> = active
            .iter()
            .enumerate()
            .map(|(rank, pack)| {
                let file = pack.item.file.clone();
                div()
                    .id(("pack-on", rank))
                    .p(px(12. * s))
                    .rounded(px(12. * s))
                    .border_1()
                    .border_color(rgb(accent_tone(0x2f9e52)))
                    .bg(gpui::linear_gradient(
                        135.,
                        gpui::linear_color_stop(rgba(accent_tone_a(0x3ccf6a14)), 0.),
                        gpui::linear_color_stop(rgba(tone_a(0x17181bff)), 1.),
                    ))
                    .flex()
                    .items_center()
                    .gap(px(14. * s))
                    .child(
                        div()
                            .w(px(26. * s))
                            .flex_none()
                            .text_center()
                            .text_size(px(16. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(MUTED))
                            .child(format!("{}", rank + 1)),
                    )
                    .child(content::thumb(pack.icon.as_ref(), "layers", 56., s))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(16. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(name_of(&pack.item.file)),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(13. * s))
                                    .text_color(rgb(MUTED))
                                    .child(if pack.item.description.is_empty() {
                                        tr("No description")
                                    } else {
                                        pack.item.description.clone()
                                    }),
                            ),
                    )
                    .when(rank == 0, |d| {
                        d.child(
                            div()
                                .px(px(8. * s))
                                .py(px(2. * s))
                                .rounded(px(5. * s))
                                .bg(rgba(accent_tone_a(0x3ccf6a26)))
                                .text_size(px(11. * s))
                                .text_color(rgb(accent_tone(0x6fe792)))
                                .child(tr("On top")),
                        )
                    })
                    .child(
                        div()
                            .id(("pack-off", rank))
                            .when(locked, |d| d.opacity(0.45))
                            .when(!locked, |d| {
                                d.cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.toggle_pack(file.clone(), false, cx)
                                    }))
                            })
                            .child(switch(true, s)),
                    )
                    .into_any_element()
            })
            .collect();
        let available: Vec<_> = packs.iter().filter(|p| !is_on(&p.item.file)).collect();
        let mut cards: Vec<AnyElement> = available
            .iter()
            .enumerate()
            .map(|(i, pack)| {
                let file = pack.item.file.clone();
                div()
                    .id(("pack", i))
                    .flex_1()
                    .min_w_0()
                    .p(px(14. * s))
                    .rounded(px(12. * s))
                    .border_1()
                    .border_color(rgb(tone(0x2a2b30)))
                    .bg(rgb(tone(0x17181b)))
                    .hover(|d| d.border_color(rgb(tone(0x4a4c54))))
                    .flex()
                    .items_center()
                    .gap(px(14. * s))
                    .child(content::thumb(pack.icon.as_ref(), "layers", 72., s))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(4. * s))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8. * s))
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(16. * s))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(name_of(&pack.item.file)),
                                    )
                                    .child(
                                        div()
                                            .flex_none()
                                            .px(px(7. * s))
                                            .rounded(px(5. * s))
                                            .bg(rgb(tone(0x232429)))
                                            .text_size(px(11. * s))
                                            .text_color(rgb(MUTED))
                                            .child(tr(if pack.item.folder {
                                                "Folder"
                                            } else {
                                                "ZIP"
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .h(px(34. * s))
                                    .overflow_hidden()
                                    .text_size(px(13. * s))
                                    .line_height(px(17. * s))
                                    .text_color(rgb(MUTED))
                                    .child(if pack.item.description.is_empty() {
                                        tr("No description")
                                    } else {
                                        pack.item.description.clone()
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .id(("pack-turn-on", i))
                            .flex_none()
                            .h(px(32. * s))
                            .px(px(14. * s))
                            .rounded(px(8. * s))
                            .bg(rgb(0xffffff))
                            .text_color(rgb(0x0b0b0d))
                            .flex()
                            .items_center()
                            .text_size(px(13. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .when(locked, |d| d.opacity(0.45))
                            .when(!locked, |d| {
                                d.cursor_pointer()
                                    .hover(|d| d.opacity(0.9))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.toggle_pack(file.clone(), true, cx)
                                    }))
                            })
                            .child(tr("Turn on")),
                    )
                    .into_any_element()
            })
            .collect();
        if cards.len() % 2 == 1 {
            cards.push(
                div()
                    .flex_1()
                    .min_w_0()
                    .p(px(14. * s))
                    .border_1()
                    .border_color(rgba(0))
                    .into_any_element(),
            );
        }
        let mut grid = div().flex().flex_col().gap(px(12. * s));
        let mut cards = cards.into_iter().peekable();
        while cards.peek().is_some() {
            grid = grid.child(
                div()
                    .flex()
                    .gap(px(12. * s))
                    .children(cards.by_ref().take(2)),
            );
        }
        let page = div()
            .flex()
            .flex_col()
            .gap(px(16. * s))
            .pb(px(20. * s))
            .child(self.page_header(
                "Resource packs",
                summary,
                vec![self
                    .page_button("open-pack-folder", "Pack folder", "folder", true)
                    .on_click(cx.listener(|this, _, _, cx| this.open_game_folder("resourcepacks", cx)))],
                cx,
            ))
            .when(locked, |d| d.child(self.running_notice("Minecraft is running. Close it to change resource packs; the game saves its own list when it quits.")))
            .when(packs.is_empty(), |d| {
                d.child(self.empty_state(
                    "layers",
                    "No resource packs yet",
                    "Drop resource-pack ZIP files or folders into the pack folder. They show up here to turn on.",
                ))
            })
            .when(!packs.is_empty(), |d| {
                d.child(self.section_title("check-circle", "On in game", active.len(), "top of the list wins"))
                    .when(rows.is_empty(), |d| {
                        d.child(quiet("None. Minecraft uses its own textures; turn a pack on below."))
                    })
                    .children(rows)
                    .child(div().h(px(6. * s)))
                    .child(self.section_title("layers", "Available", available.len(), ""))
                    .when(available.is_empty(), |d| d.child(quiet("Every pack in the folder is on.")))
                    .child(grid)
            });
        div().size_full().child(
            div()
                .id("packs-scroll")
                .size_full()
                .overflow_y_scroll()
                .child(page),
        )
    }

    /// Pings every saved server again when the list changed or the last pings are old.
    pub(super) fn refresh_pings(&mut self, force: bool) {
        let addresses: Vec<String> = std::iter::once(server_ping::FEATURED_ADDRESS.to_owned())
            .chain(
                self.content
                    .servers
                    .iter()
                    .filter(|s| !server_ping::is_featured(&s.item.address))
                    .map(|s| s.item.address.clone()),
            )
            .take(MAX_PINGS)
            .collect();
        let fresh = self
            .pinged
            .as_ref()
            .is_some_and(|(list, at)| *list == addresses && at.elapsed() < PING_FRESH);
        if fresh && !force {
            return;
        }
        self.pinged = Some((addresses.clone(), Instant::now()));
        for address in addresses {
            self.pings.insert(address.clone(), None);
            let sender = self.sender.clone();
            std::thread::spawn(move || {
                let result = server_ping::ping(&address, PING_TIMEOUT);
                let _ = sender.send(Event::Ping(address, result));
            });
        }
    }

    pub(super) fn servers_page(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let busy = self.launching || self.child.is_some() || self.setup_busy || self.auth_busy;
        let servers = &self.content.servers;
        let online = servers
            .iter()
            .filter(|sv| matches!(self.pings.get(&sv.item.address), Some(Some(Ok(_)))))
            .count();
        let players: u32 = servers
            .iter()
            .filter_map(|sv| match self.pings.get(&sv.item.address) {
                Some(Some(Ok(status))) => Some(status.online),
                _ => None,
            })
            .sum();
        let summary = trf(
            "{count} saved servers, {online} online with {players} players. Join one straight from here.",
            &[
                ("count", &servers.len().to_string()),
                ("online", &online.to_string()),
                ("players", &players.to_string()),
            ],
        );
        let rows: Vec<AnyElement> = servers
            .iter()
            .enumerate()
            .filter(|(_, server)| !server_ping::is_featured(&server.item.address))
            .map(|(i, server)| {
                let address = server.item.address.clone();
                let status = self.pings.get(&server.item.address);
                let (dot, line) = match status {
                    Some(Some(Ok(st))) => (
                        accent_tone(0x3ccf6a),
                        if st.motd.is_empty() {
                            tr("Online")
                        } else {
                            st.motd.clone()
                        },
                    ),
                    Some(Some(Err(_))) => (
                        0xd9534f,
                        tr("No answer to the status check. Join may still work."),
                    ),
                    _ => (0x9a9ca4, tr("Checking…")),
                };
                let stats = match status {
                    Some(Some(Ok(st))) => {
                        let bars = match st.latency_ms {
                            0..=80 => (accent_tone(0x3ccf6a), 4),
                            81..=150 => (0x9acd32, 3),
                            151..=300 => (0xf2c14e, 2),
                            _ => (0xd9534f, 1),
                        };
                        div()
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap(px(4. * s))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6. * s))
                                    .text_size(px(14. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(icon("user-multiple-02", MUTED).size(px(14. * s)))
                                    .child(format!("{} / {}", st.online, st.max)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_end()
                                    .gap(px(6. * s))
                                    .text_size(px(12. * s))
                                    .text_color(rgb(MUTED))
                                    .child(div().flex().items_end().gap(px(2. * s)).children(
                                        (1..=4).map(|b| {
                                            div()
                                                .w(px(3. * s))
                                                .h(px((3 + b * 3) as f32 * s))
                                                .rounded(px(1. * s))
                                                .bg(rgb(if b <= bars.1 {
                                                    bars.0
                                                } else {
                                                    tone(0x34363d)
                                                }))
                                        }),
                                    ))
                                    .child(format!("{} ms", st.latency_ms)),
                            )
                            .when(!st.version.is_empty(), |d| {
                                d.child(
                                    div()
                                        .max_w(px(160. * s))
                                        .truncate()
                                        .text_size(px(11. * s))
                                        .text_color(rgb(tone(0x6b6e77)))
                                        .child(st.version.clone()),
                                )
                            })
                    }
                    _ => div(),
                };
                div()
                    .id(("server", i))
                    .p(px(12. * s))
                    .rounded(px(12. * s))
                    .border_1()
                    .border_color(rgb(tone(0x2a2b30)))
                    .bg(rgb(tone(0x17181b)))
                    .hover(|d| d.border_color(rgb(tone(0x4a4c54))))
                    .flex()
                    .items_center()
                    .gap(px(14. * s))
                    .child(content::thumb(server.icon.as_ref(), "server", 64., s))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(4. * s))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8. * s))
                                    .child(
                                        div()
                                            .size(px(8. * s))
                                            .flex_none()
                                            .rounded_full()
                                            .bg(rgb(dot)),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(17. * s))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(server.item.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex_none()
                                            .text_size(px(12. * s))
                                            .text_color(rgb(tone(0x6b6e77)))
                                            .child(server.item.address.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(13. * s))
                                    .text_color(rgb(MUTED))
                                    .child(line),
                            ),
                    )
                    .child(stats)
                    .child(
                        div()
                            .id(("server-join", i))
                            .flex_none()
                            .h(px(40. * s))
                            .px(px(20. * s))
                            .rounded(px(9. * s))
                            .bg(rgb(0xffffff))
                            .text_color(rgb(0x0b0b0d))
                            .flex()
                            .items_center()
                            .gap(px(7. * s))
                            .text_size(px(14. * s))
                            .font_weight(FontWeight::SEMIBOLD)
                            .when(!busy && i == 0, |d| d)
                            .when(busy, |d| d.opacity(0.4))
                            .when(!busy, |d| {
                                d.cursor_pointer()
                                    .hover(|d| d.opacity(0.88))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.launch_with(
                                            vec!["--quickPlayMultiplayer".into(), address.clone()],
                                            cx,
                                        )
                                    }))
                            })
                            .child(icon("play", 0x0b0b0d).size(px(14. * s)))
                            .child(tr("Join")),
                    )
                    .into_any_element()
            })
            .collect();
        let page = div()
            .flex()
            .flex_col()
            .gap(px(14. * s))
            .pb(px(20. * s))
            .child(self.page_header(
                "Servers",
                summary,
                vec![
                    self.page_button("servers-refresh", "Refresh", "refresh", false)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.refresh_pings(true);
                            cx.notify();
                        })),
                    self.page_button("play-multiplayer", "Open Multiplayer", "play", true)
                        .when(busy, |d| d.opacity(0.45))
                        .when(!busy, |d| d.on_click(cx.listener(|this, _, _, cx| this.play(cx)))),
                ],
                cx,
            ))
            .child(self.featured_server(cx))
            .when(servers.is_empty(), |d| {
                d.child(self.empty_state(
                    "server",
                    "No saved servers yet",
                    "Add servers in Minecraft's Multiplayer screen. They show up here with their status, so you can join with one click.",
                ))
            })
            .children(rows)
            .when(!servers.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(px(12. * s))
                        .text_color(rgb(tone(0x6b6e77)))
                        .child(tr("Status comes straight from each server. Add or edit servers in Minecraft's Multiplayer screen.")),
                )
            });
        div().size_full().child(
            div()
                .id("servers-scroll")
                .size_full()
                .overflow_y_scroll()
                .child(page),
        )
    }

    /// The featured server's card on Home and Servers: always there, live status and Join.
    pub(super) fn featured_server(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        let busy = self.launching || self.child.is_some() || self.setup_busy || self.auth_busy;
        let icon_from_list = self
            .content
            .servers
            .iter()
            .find(|sv| server_ping::is_featured(&sv.item.address))
            .and_then(|sv| sv.icon.clone());
        let status = self.pings.get(server_ping::FEATURED_ADDRESS);
        let (dot, line) = match status {
            Some(Some(Ok(st))) => (
                0x3ccf6a,
                if st.motd.is_empty() {
                    tr("Online")
                } else {
                    st.motd.clone()
                },
            ),
            Some(Some(Err(_))) => (
                0xd9534f,
                tr("No answer to the status check. Join may still work."),
            ),
            _ => (0x9a9ca4, tr("Checking…")),
        };
        let stats = match status {
            Some(Some(Ok(st))) => div()
                .flex()
                .flex_col()
                .items_end()
                .gap(px(4. * s))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6. * s))
                        .text_size(px(18. * s))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(icon("user-multiple-02", 0xbff0cf).size(px(17. * s)))
                        .child(format!("{} / {}", st.online, st.max)),
                )
                .child(
                    div()
                        .text_size(px(12. * s))
                        .text_color(rgb(0x9fd4b2))
                        .child(trf("{ms} ms ping", &[("ms", &st.latency_ms.to_string())])),
                ),
            _ => div(),
        };
        div()
            .relative()
            .flex_none()
            .p(px(16. * s))
            .rounded(px(14. * s))
            .overflow_hidden()
            .border_1()
            .border_color(rgba(0x3ccf6a66))
            // Glass, with a green glow from the left: the featured server.
            .bg(gpui::linear_gradient(
                100.,
                gpui::linear_color_stop(rgba(0x1f6b4280), 0.),
                gpui::linear_color_stop(rgba(0x14151b80), 0.85),
            ))
            .flex()
            .items_center()
            .gap(px(18. * s))
            .child(
                div()
                    .size(px(84. * s))
                    .flex_none()
                    .rounded(px(14. * s))
                    .overflow_hidden()
                    .bg(rgb(0x0b2a18))
                    .border_1()
                    .border_color(rgb(0x2f9e5a))
                    .map(|d| match icon_from_list {
                        Some(icon) => d.child(img(icon).size_full()),
                        None => d.child(
                            img("artwork/zedix.png")
                                .size_full()
                                .object_fit(ObjectFit::Cover),
                        ),
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(5. * s))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8. * s))
                            .child(
                                div()
                                    .px(px(8. * s))
                                    .py(px(2. * s))
                                    .rounded(px(5. * s))
                                    .bg(rgba(0xf2c14e26))
                                    .flex()
                                    .items_center()
                                    .gap(px(5. * s))
                                    .text_size(px(11. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xf2c14e))
                                    .child(icon("star", 0xf2c14e).size(px(11. * s)))
                                    .child(tr("FEATURED SERVER")),
                            )
                            .child(div().size(px(8. * s)).rounded_full().bg(rgb(dot))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(10. * s))
                            .child(
                                div()
                                    .text_size(px(24. * s))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(server_ping::FEATURED_NAME),
                            )
                            .child(
                                div()
                                    .text_size(px(13. * s))
                                    .text_color(rgb(0x9fd4b2))
                                    .child(server_ping::FEATURED_ADDRESS),
                            ),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(13. * s))
                            .text_color(rgb(0xd2ecdc))
                            .child(line),
                    ),
            )
            .child(stats)
            .child(
                div()
                    .id("featured-join")
                    .flex_none()
                    .h(px(46. * s))
                    .px(px(26. * s))
                    .rounded(px(10. * s))
                    .bg(gpui::linear_gradient(
                        180.,
                        gpui::linear_color_stop(rgb(0x46d36f), 0.),
                        gpui::linear_color_stop(rgb(0x23a04a), 1.),
                    ))
                    .shadow(vec![gpui::BoxShadow {
                        color: rgba(0x2fbf5a55).into(),
                        offset: gpui::point(px(0.), px(4. * s)),
                        blur_radius: px(18. * s),
                        spread_radius: px(0.),
                        inset: false,
                    }])
                    .flex()
                    .items_center()
                    .gap(px(8. * s))
                    .text_size(px(16. * s))
                    .font_weight(FontWeight::SEMIBOLD)
                    .when(busy, |d| d.opacity(0.45))
                    .when(!busy, |d| {
                        d.cursor_pointer()
                            .hover(|d| d.opacity(0.9))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.launch_with(
                                    vec![
                                        "--quickPlayMultiplayer".into(),
                                        server_ping::FEATURED_ADDRESS.into(),
                                    ],
                                    cx,
                                )
                            }))
                    })
                    .child(icon("play", INK).size(px(15. * s)))
                    .child(tr("Join")),
            )
    }
}
