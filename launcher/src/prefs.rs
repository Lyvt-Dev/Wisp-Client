//! The launcher's own preferences (`launcher-prefs.json` in its data folder): what happens when
//! the game starts, the game window, the interface, and the cosmetics switches the game reads
//! from each instance's `wisp-extras.txt`.
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const FILE: &str = "launcher-prefs.json";

/// What the launcher does once the game has started.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AfterLaunch {
    #[default]
    KeepOpen,
    Minimize,
    Close,
}

impl AfterLaunch {
    pub const ALL: [Self; 3] = [Self::KeepOpen, Self::Minimize, Self::Close];
    pub fn label(self) -> &'static str {
        match self {
            Self::KeepOpen => "Keep open",
            Self::Minimize => "Minimize",
            Self::Close => "Close",
        }
    }
}

/// The page the launcher opens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartPage {
    #[default]
    Home,
    Versions,
    Shop,
}

impl StartPage {
    pub const ALL: [Self; 3] = [Self::Home, Self::Versions, Self::Shop];
    pub fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Versions => "Versions",
            Self::Shop => "Shop",
        }
    }
}

/// Other players' cosmetics, as the game's `cosmeticsOthers` line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Others {
    #[default]
    Shown,
    NameTags,
    Hidden,
}

impl Others {
    pub const ALL: [Self; 3] = [Self::Shown, Self::NameTags, Self::Hidden];
    pub fn label(self) -> &'static str {
        match self {
            Self::Shown => "Shown",
            Self::NameTags => "Name tags only",
            Self::Hidden => "Hidden",
        }
    }
    fn line(self) -> &'static str {
        match self {
            Self::Shown => "shown",
            Self::NameTags => "name_tags",
            Self::Hidden => "hidden",
        }
    }
}

/// The colour of buttons, highlights and the active page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accent {
    /// Black and white: Wisp's own.
    #[default]
    Mono,
    Green,
    Blue,
    Violet,
    Pink,
    Red,
    Orange,
    Gold,
    Teal,
}

impl Accent {
    pub const ALL: [Self; 9] = [
        Self::Mono,
        Self::Green,
        Self::Blue,
        Self::Violet,
        Self::Pink,
        Self::Red,
        Self::Orange,
        Self::Gold,
        Self::Teal,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Mono => "Black & White",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Violet => "Violet",
            Self::Pink => "Pink",
            Self::Red => "Red",
            Self::Orange => "Orange",
            Self::Gold => "Gold",
            Self::Teal => "Teal",
        }
    }
    /// The colour as `0xRRGGBB`.
    pub fn rgb(self) -> u32 {
        match self {
            Self::Mono => 0xffffff,
            Self::Green => 0x3ccf6a,
            Self::Blue => 0x3d8bfd,
            Self::Violet => 0x9b6bff,
            Self::Pink => 0xf2559e,
            Self::Red => 0xef4f4f,
            Self::Orange => 0xf7853a,
            Self::Gold => 0xe3b23c,
            Self::Teal => 0x22c3b4,
        }
    }
}

/// The greys behind everything.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backdrop {
    Midnight,
    #[default]
    Black,
    Slate,
    Ocean,
    Plum,
}

impl Backdrop {
    pub const ALL: [Self; 5] = [
        Self::Midnight,
        Self::Black,
        Self::Slate,
        Self::Ocean,
        Self::Plum,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Midnight => "Midnight",
            Self::Black => "Black",
            Self::Slate => "Slate",
            Self::Ocean => "Ocean",
            Self::Plum => "Plum",
        }
    }
}

/// The artwork on the Home banner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Banner {
    #[default]
    Lake,
    Portal,
    Aurora,
    Nebula,
}

impl Banner {
    pub const ALL: [Self; 4] = [Self::Lake, Self::Portal, Self::Aurora, Self::Nebula];
    pub fn label(self) -> &'static str {
        match self {
            Self::Lake => "Sunset lake",
            Self::Portal => "Night portal",
            Self::Aurora => "Aurora forest",
            Self::Nebula => "Nebula",
        }
    }
    /// The launcher asset drawn for it.
    pub fn asset(self) -> &'static str {
        match self {
            Self::Lake => "artwork/hero.png",
            Self::Portal => "artwork/news.png",
            Self::Aurora => "artwork/aurora.png",
            Self::Nebula => "shop/banner_nebula_cape.png",
        }
    }
}

/// Window sizes offered for the game (0 x 0: the game's own default).
pub const WINDOW_SIZES: [(u32, u32); 6] = [
    (0, 0),
    (1280, 720),
    (1600, 900),
    (1920, 1080),
    (2560, 1440),
    (854, 480),
];
/// Render distances offered (0: the game's own setting).
pub const RENDER_DISTANCES: [u32; 7] = [0, 6, 8, 12, 16, 24, 32];
/// Interface sizes offered, as a factor on the launcher's own fit to the window.
pub const UI_SIZES: [f32; 5] = [0.85, 0.92, 1.0, 1.1, 1.2];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    // General
    pub after_launch: AfterLaunch,
    pub start_page: StartPage,
    // Game
    pub window: (u32, u32),
    pub fullscreen: bool,
    pub render_distance: u32,
    /// Keep the previous game log beside the new one (`local-launcher.prev.log`).
    pub keep_logs: bool,
    // Appearance
    pub ui_size: f32,
    /// The Home banner turns to its next slide every few seconds.
    pub hero_autoplay: bool,
    pub accent: Accent,
    pub backdrop: Backdrop,
    pub banner: Banner,
    /// The friends column on the right (on wide windows).
    pub side_panel: bool,
    /// The Wisp server (cosmetics and friends); empty: the one built in, if any.
    pub service_url: String,
    // Cosmetics
    pub show_mine: bool,
    pub others: Others,
    pub auras: bool,
    pub share_emotes: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            after_launch: AfterLaunch::KeepOpen,
            start_page: StartPage::Home,
            window: (0, 0),
            fullscreen: false,
            render_distance: 0,
            keep_logs: true,
            ui_size: 1.0,
            hero_autoplay: true,
            accent: Accent::Mono,
            backdrop: Backdrop::Black,
            banner: Banner::Lake,
            side_panel: true,
            service_url: String::new(),
            show_mine: true,
            others: Others::Shown,
            auras: true,
            share_emotes: true,
        }
    }
}

impl Prefs {
    pub fn load(home: &Path) -> Self {
        fs::read(home.join(FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, home: &Path) -> std::io::Result<()> {
        fs::create_dir_all(home)?;
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        let temporary = home.join(format!(".{FILE}.tmp"));
        fs::write(&temporary, bytes)?;
        fs::rename(temporary, home.join(FILE))
    }

    /// The game arguments these settings add (after the instance's own).
    pub fn game_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if self.window.0 > 0 && self.window.1 > 0 {
            args.extend([
                "--width".into(),
                self.window.0.to_string(),
                "--height".into(),
                self.window.1.to_string(),
            ]);
        }
        if self.fullscreen {
            args.push("--fullscreen".into());
        }
        if self.render_distance > 0 {
            args.extend(["--render-distance".into(), self.render_distance.to_string()]);
        }
        args
    }

    /// Writes the cosmetics switches into the game's `wisp-extras.txt` in `game_home`,
    /// keeping its other lines (the game's own format, `wisp_cosmetics::prefs`).
    pub fn write_cosmetics(&self, game_home: &Path) -> std::io::Result<()> {
        const KEYS: [&str; 4] = [
            "cosmeticsMine",
            "cosmeticsOthers",
            "cosmeticsAuras",
            "emotesShared",
        ];
        let path = game_home.join("wisp-extras.txt");
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        let mut out: String = text
            .lines()
            .filter(|l| {
                l.split_once(':')
                    .is_none_or(|(k, _)| !KEYS.contains(&k.trim()))
            })
            .flat_map(|l| [l, "\n"])
            .collect();
        out.push_str(&format!(
            "cosmeticsMine:{}\ncosmeticsOthers:{}\ncosmeticsAuras:{}\nemotesShared:{}\n",
            self.show_mine,
            self.others.line(),
            self.auras,
            self.share_emotes
        ));
        fs::create_dir_all(game_home)?;
        fs::write(path, out)
    }
}

/// The size of everything under `path`, in bytes (0 when missing).
pub fn folder_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => folder_size(&e.path()),
            Ok(_) => e.metadata().map_or(0, |m| m.len()),
            Err(_) => 0,
        })
        .sum()
}

/// `bytes` for people: "12.4 MB".
pub fn human_size(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", units[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_args_follow_the_settings() {
        assert!(Prefs::default().game_args().is_empty());
        let prefs = Prefs {
            window: (1920, 1080),
            fullscreen: true,
            render_distance: 12,
            ..Prefs::default()
        };
        assert_eq!(
            prefs.game_args(),
            [
                "--width",
                "1920",
                "--height",
                "1080",
                "--fullscreen",
                "--render-distance",
                "12"
            ]
        );
    }

    #[test]
    fn cosmetics_lines_replace_their_own_and_keep_the_rest() {
        let dir = std::env::temp_dir().join(format!("wisp-prefs-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("wisp-extras.txt"),
            "zoom:3\ncosmeticsOthers:hidden\n",
        )
        .unwrap();
        let prefs = Prefs {
            others: Others::NameTags,
            auras: false,
            ..Prefs::default()
        };
        prefs.write_cosmetics(&dir).unwrap();
        let text = fs::read_to_string(dir.join("wisp-extras.txt")).unwrap();
        assert!(text.starts_with("zoom:3\n"));
        assert_eq!(text.matches("cosmeticsOthers").count(), 1);
        assert!(
            text.contains("cosmeticsOthers:name_tags") && text.contains("cosmeticsAuras:false")
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sizes_read_for_people() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn a_missing_or_old_file_gives_defaults() {
        let old: Prefs = serde_json::from_str(r#"{"fullscreen": true}"#).unwrap();
        assert!(old.fullscreen && old.show_mine && old.after_launch == AfterLaunch::KeepOpen);
    }
}
