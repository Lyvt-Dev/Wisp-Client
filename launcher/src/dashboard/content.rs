//! What the selected instance holds (worlds, resource packs, servers) and when instances were
//! last played, read once when the instance or page changes rather than every frame.
use super::*;
use std::path::Path;
use std::time::Instant;
use wisp_launcher::local_content::{self, Pack, Server, World};

/// How long a reading stays fresh while the page stays the same.
const FRESH: Duration = Duration::from_secs(8);

pub(super) struct Shown<T> {
    pub item: T,
    pub icon: Option<Arc<gpui::RenderImage>>,
}

#[derive(Default)]
pub(super) struct Content {
    key: Option<(PathBuf, u8)>,
    read_at: Option<Instant>,
    pub worlds: Vec<Shown<World>>,
    /// Every world in `saves/` (only the latest few are kept in `worlds`).
    pub world_count: usize,
    pub packs: Vec<Shown<Pack>>,
    /// Pack files on in the game, lowest priority first.
    pub enabled_packs: Vec<String>,
    /// Files in `resourcepacks/` that are not packs.
    pub other_files: usize,
    pub servers: Vec<Shown<Server>>,
    /// Instance name to when it was last started (seconds since 1970).
    pub launched: std::collections::BTreeMap<String, u64>,
}

/// A PNG as crisp pixel art about `size` pixels square.
fn picture(bytes: &[u8], size: f32) -> Option<Arc<gpui::RenderImage>> {
    let decoded = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?;
    let (w, h) = (decoded.width() as usize, decoded.height() as usize);
    if w == 0 || h == 0 || w > 1024 || h > 1024 {
        return None;
    }
    let rgba = decoded.to_rgba8();
    let texels: Vec<[u8; 4]> = rgba.pixels().map(|p| p.0).collect();
    Some(crate::art::bake(&texels, w, h, size / w.max(h) as f32, 1.))
}

fn shown<T>(items: Vec<T>, icon: impl Fn(&T) -> Option<&[u8]>) -> Vec<Shown<T>> {
    items
        .into_iter()
        .map(|item| Shown {
            icon: icon(&item).and_then(|b| picture(b, 128.)),
            item,
        })
        .collect()
}

impl Content {
    /// Reads again at the next frame (after the launcher changed something).
    pub fn invalidate(&mut self) {
        self.read_at = None;
    }

    /// Reads again when the instance or page changed, or the last reading is old.
    pub fn refresh(&mut self, home: &Path, game_home: Option<&Path>, page: u8) {
        let key = game_home.map(|g| (g.to_path_buf(), page));
        if key == self.key && self.read_at.is_some_and(|t| t.elapsed() < FRESH) {
            return;
        }
        self.key = key;
        self.read_at = Some(Instant::now());
        self.launched = wisp_launcher::catalog::Preferences::load(home)
            .map(|p| p.launched)
            .unwrap_or_default();
        let Some(game) = game_home else {
            self.worlds.clear();
            self.world_count = 0;
            self.packs.clear();
            self.enabled_packs.clear();
            self.servers.clear();
            self.other_files = 0;
            return;
        };
        let mut worlds = local_content::worlds(&game.join("saves"));
        self.world_count = worlds.len();
        worlds.truncate(8);
        self.worlds = shown(worlds, |w| w.icon.as_deref());
        let (packs, other) = local_content::packs(&game.join("resourcepacks"));
        self.packs = shown(packs, |p| p.icon.as_deref());
        self.other_files = other;
        self.enabled_packs = local_content::enabled_packs(game);
        self.servers = shown(local_content::servers(&game.join("servers.dat")), |s| {
            s.icon.as_deref()
        });
    }
}

/// "3 days ago", "just now".
pub(super) fn ago(timestamp: u64) -> String {
    let (n, unit) = wisp_launcher::catalog::elapsed(timestamp);
    match unit {
        "now" => tr("just now"),
        "minutes" => trf("{n} min ago", &[("n", &n.to_string())]),
        "hours" => trf("{n} h ago", &[("n", &n.to_string())]),
        _ if n == 1 => tr("yesterday"),
        _ => trf("{n} days ago", &[("n", &n.to_string())]),
    }
}

/// A square picture, or a glyph on a tile when there is none.
pub(super) fn thumb(
    icon: Option<&Arc<gpui::RenderImage>>,
    glyph: &'static str,
    size: f32,
    s: f32,
) -> Div {
    div()
        .size(px(size * s))
        .flex_none()
        .rounded(px(8. * s))
        .overflow_hidden()
        .bg(rgb(tone(0x232429)))
        .flex()
        .items_center()
        .justify_center()
        .map(|d| match icon {
            Some(icon) => d.child(img(icon.clone()).size_full()),
            None => d.child(icon_glyph(glyph, size * 0.45 * s)),
        })
}

fn icon_glyph(glyph: &'static str, size: f32) -> Svg {
    icon(glyph, MUTED).size(px(size))
}
