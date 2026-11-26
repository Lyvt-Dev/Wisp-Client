//! Waypoints (the `waypoints` client module, `docs/extras.md`): named places the player marks
//! with `/wp`, shown as labels through walls with their distance. Kept per world or server and
//! dimension in `wisp-waypoints.txt` in the game folder. Client-only: nothing is sent.
use super::Game;
use wisp_client::{chat::ChatSource, text::Text};

pub(super) const FILE: &str = "wisp-waypoints.txt";
/// The farthest a label is drawn from the camera; one farther sits at this distance in its
/// direction, still naming its real distance, so it reads at the size of a near name tag.
pub(super) const LABEL_REACH: f64 = 10.0;

/// One waypoint: the world or server it belongs to, its dimension, name and block.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Waypoint {
    pub place: String,
    pub dim: String,
    pub name: String,
    pub at: [i32; 3],
}

/// The file's lines: `place<TAB>dim<TAB>name<TAB>x<TAB>y<TAB>z`; broken lines are skipped.
pub(super) fn parse(text: &str) -> Vec<Waypoint> {
    text.lines()
        .filter_map(|line| {
            let mut f = line.split('\t');
            let (place, dim, name) = (f.next()?, f.next()?, f.next()?);
            let mut n = || f.next()?.parse::<i32>().ok();
            let at = [n()?, n()?, n()?];
            (!name.is_empty()).then(|| Waypoint {
                place: place.to_owned(),
                dim: dim.to_owned(),
                name: name.to_owned(),
                at,
            })
        })
        .collect()
}
pub(super) fn serialize(points: &[Waypoint]) -> String {
    points
        .iter()
        .map(|w| {
            format!(
                "{}\t{}\t{}\t{}\t{}\t{}\n",
                w.place, w.dim, w.name, w.at[0], w.at[1], w.at[2]
            )
        })
        .collect()
}

impl Game<'_> {
    /// Where the player is: the singleplayer world by name, else the server's address.
    fn waypoint_place(&self) -> String {
        match self.server_address() {
            Some(address) if !self.has_singleplayer_server() => format!("server:{address}"),
            // The window's own copy of a single player level has no name; its server thread's does.
            _ => format!(
                "world:{}",
                self.integrated
                    .as_ref()
                    .map_or(self.level.name.as_str(), |i| i.world_name.as_str())
            ),
        }
    }
    fn waypoints_loaded(&mut self) -> &mut Vec<Waypoint> {
        self.extras.waypoints.get_or_insert_with(|| {
            std::fs::read_to_string(Self::game_dir().join(FILE))
                .map(|t| parse(&t))
                .unwrap_or_default()
        })
    }
    fn save_waypoints(&mut self) {
        let text = serialize(self.waypoints_loaded());
        if let Err(e) = std::fs::write(Self::game_dir().join(FILE), text) {
            log::warn!("Wisp: could not save waypoints: {e}");
        }
    }
    fn waypoint_say(&mut self, key: &str, fallback: &str, args: &[&dyn std::fmt::Display]) {
        let text = wisp_client::lang::get_or(key, fallback);
        let mut out = text;
        for a in args {
            out = out.replacen("%s", &a.to_string(), 1);
        }
        self.local_chat_line(Text::literal(out), None, ChatSource::Client);
    }
    /// `/wp add|remove|list|clear [name]` while the module is on. True when it was a waypoint
    /// command (it never reaches the server).
    pub(super) fn waypoint_command(&mut self, command: &str) -> bool {
        // Only the window's own game: never the server side of single player (no renderer) or a
        // command it runs for a LAN guest.
        if !self.settings.extras.module("waypoints")
            || self.renderer.is_none()
            || self.lan_acting.is_some()
        {
            return false;
        }
        let mut words = command.split_whitespace();
        if !matches!(words.next(), Some("wp" | "waypoint")) {
            return false;
        }
        let action = words.next().unwrap_or("list");
        let name = words.collect::<Vec<_>>().join(" ");
        let place = self.waypoint_place();
        let dim = self.dim.id().to_owned();
        match action {
            "add" | "set" => {
                let Some(p) = &self.player else {
                    return true;
                };
                let at = [p.position.x, p.position.y, p.position.z].map(|v| v.floor() as i32);
                let name = if name.is_empty() {
                    format!("{} {} {}", at[0], at[1], at[2])
                } else {
                    name
                };
                let points = self.waypoints_loaded();
                points.retain(|w| !(w.place == place && w.dim == dim && w.name == name));
                points.push(Waypoint {
                    place,
                    dim,
                    name: name.clone(),
                    at,
                });
                self.save_waypoints();
                self.waypoint_say(
                    "wisp.waypoints.added",
                    "Waypoint %s set at %s %s %s",
                    &[&name, &at[0], &at[1], &at[2]],
                );
            }
            "remove" | "delete" => {
                let points = self.waypoints_loaded();
                let before = points.len();
                points.retain(|w| !(w.place == place && w.dim == dim && w.name == name));
                let removed = before != points.len();
                self.save_waypoints();
                if removed {
                    self.waypoint_say("wisp.waypoints.removed", "Waypoint %s removed", &[&name]);
                } else {
                    self.waypoint_say("wisp.waypoints.unknown", "No waypoint named %s", &[&name]);
                }
            }
            "clear" => {
                self.waypoints_loaded()
                    .retain(|w| !(w.place == place && w.dim == dim));
                self.save_waypoints();
                self.waypoint_say("wisp.waypoints.cleared", "Waypoints here cleared", &[]);
            }
            _ => {
                let lines: Vec<String> = self
                    .waypoints_loaded()
                    .iter()
                    .filter(|w| w.place == place && w.dim == dim)
                    .map(|w| format!("{}: {} {} {}", w.name, w.at[0], w.at[1], w.at[2]))
                    .collect();
                if lines.is_empty() {
                    self.waypoint_say(
                        "wisp.waypoints.none",
                        "No waypoints here. /wp add <name> marks where you stand.",
                        &[],
                    );
                }
                for line in lines {
                    self.local_chat_line(Text::literal(line), None, ChatSource::Client);
                }
            }
        }
        true
    }
    /// The labels of this world's and dimension's waypoints: where to draw, the name and the
    /// distance in blocks.
    pub(super) fn waypoint_labels(&self, camera: glam::DVec3) -> Vec<(glam::DVec3, String, f64)> {
        if !self.settings.extras.module("waypoints") {
            return Vec::new();
        }
        let Some(points) = &self.extras.waypoints else {
            return Vec::new();
        };
        let place = self.waypoint_place();
        let dim = self.dim.id();
        points
            .iter()
            .filter(|w| w.place == place && w.dim == dim)
            .map(|w| {
                let target = glam::DVec3::new(
                    f64::from(w.at[0]) + 0.5,
                    f64::from(w.at[1]) + 1.5,
                    f64::from(w.at[2]) + 0.5,
                );
                let offset = target - camera;
                let distance = offset.length();
                let at = if distance > LABEL_REACH {
                    camera + offset / distance * LABEL_REACH
                } else {
                    target
                };
                (at, w.name.clone(), distance)
            })
            .collect()
    }
    /// Loads the file once the module is on, so labels show without a command first.
    pub(super) fn waypoints_ready(&mut self) {
        if self.settings.extras.module("waypoints") && self.extras.waypoints.is_none() {
            self.waypoints_loaded();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waypoints_survive_the_file_and_skip_broken_lines() {
        let points = vec![
            Waypoint {
                place: "world:New World".into(),
                dim: "minecraft:overworld".into(),
                name: "Home base".into(),
                at: [10, 64, -3],
            },
            Waypoint {
                place: "server:example.org".into(),
                dim: "minecraft:the_nether".into(),
                name: "Portal".into(),
                at: [-100, 70, 25],
            },
        ];
        let text = serialize(&points);
        assert_eq!(parse(&text), points);
        assert_eq!(
            parse("broken\nworld:x\tminecraft:overworld\t\t1\t2\t3\n"),
            vec![]
        );
    }
}
