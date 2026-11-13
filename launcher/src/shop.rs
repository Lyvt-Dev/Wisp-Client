//! The Shop: Wisp Coins, the items they buy, and what the player wears in the game.
//!
//! Everything is local to this launcher (`shop.json` in its data folder). Coins are earned, never
//! bought: a welcome gift and a daily bonus. Equipped capes, weapons, hats, badges, pets and trails are written
//! as cosmetic item files (`docs/cosmetics.md`) into `shop/worn`, which the game reads through
//! `WISP_COSMETICS_LOCAL`, limited to the signed-in player.
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Coins every new wallet starts with.
pub const WELCOME: u32 = 1_000;
/// Coins for the first launcher start of each day.
pub const DAILY: u32 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Cape,
    /// On the back like a cape, so wearing one takes the other off.
    Wings,
    /// Carried across the back (a sword), beside a cape.
    Weapon,
    /// Worn on the head (a hat); a helmet hides it.
    Head,
    Badge,
    Pet,
    Trail,
    Aura,
    /// Played from the game's emote wheel (B); any number can be in it.
    Emote,
}

impl Kind {
    /// Whether the game draws both in the same place, so only one of them is worn.
    pub fn shares_slot(self, other: Kind) -> bool {
        let back = |k: Kind| matches!(k, Kind::Cape | Kind::Wings);
        self == other || (back(self) && back(other))
    }

    pub const ALL: [Kind; 9] = [
        Kind::Cape,
        Kind::Wings,
        Kind::Weapon,
        Kind::Head,
        Kind::Badge,
        Kind::Pet,
        Kind::Trail,
        Kind::Aura,
        Kind::Emote,
    ];
}

/// The file in the worn folder that lists the emotes in the wheel, one id a line.
pub const EMOTES_FILE: &str = "emotes.txt";

/// One item in the shop.
pub struct Offer {
    pub id: &'static str,
    pub name: &'static str,
    pub blurb: &'static str,
    pub kind: Kind,
    /// Wisp Coins; 0 for the Founders Cape, which is claimed rather than bought.
    pub price: u32,
    /// The item file the game reads, if the game can draw this item.
    pub item: Option<&'static [u8]>,
    /// The card's picture (PNG).
    pub art: &'static [u8],
    /// Who made it, when someone outside Wisp did (`@gold3nz`).
    pub creator: Option<&'static str>,
}

impl Offer {
    pub fn founders(&self) -> bool {
        self.id == FOUNDERS
    }
}

/// The free cape for Wisp's first players.
pub const FOUNDERS: &str = "founders_cape";

macro_rules! offer {
    ($id:literal, $name:literal, $blurb:literal, $kind:ident, $price:expr, item) => {
        Offer {
            id: $id,
            name: $name,
            blurb: $blurb,
            kind: Kind::$kind,
            price: $price,
            item: Some(include_bytes!(concat!("../assets/shop/", $id, ".json"))),
            art: include_bytes!(concat!("../assets/shop/", $id, ".png")),
            creator: None,
        }
    };
    ($id:literal, $name:literal, $blurb:literal, $kind:ident, $price:expr, item, by $creator:literal) => {
        Offer {
            creator: Some($creator),
            ..offer!($id, $name, $blurb, $kind, $price, item)
        }
    };
    ($id:literal, $name:literal, $blurb:literal, $kind:ident, $price:expr) => {
        Offer {
            id: $id,
            name: $name,
            blurb: $blurb,
            kind: Kind::$kind,
            price: $price,
            item: None,
            art: include_bytes!(concat!("../assets/shop/", $id, ".png")),
            creator: None,
        }
    };
}

pub const OFFERS: &[Offer] = &[
    offer!(
        "founders_cape",
        "Founders Cape",
        "A night-sky cape with the Wisp star. Free for Wisp's first players.",
        Cape,
        0,
        item
    ),
    offer!(
        "nebula_cape",
        "Nebula Cape",
        "A rose-violet nebula drifting across a dark cape.",
        Cape,
        1_200,
        item
    ),
    offer!(
        "aurora_cape",
        "Aurora Cape",
        "Northern lights in teal and ice blue.",
        Cape,
        900,
        item
    ),
    offer!(
        "zedix_cape",
        "Zedix Cape",
        "A neon purple Z that pulses with light and sparks on a dark violet cape.",
        Cape,
        400,
        item
    ),
    offer!(
        "wisp_original_cape",
        "Wisp Original Cape",
        "The original: Wisp's folded-A star glowing on a deep violet night, a sheen of light sweeping across it.",
        Cape,
        1_000,
        item
    ),
    offer!(
        "stray_head",
        "Stray Head",
        "A wide conical straw hat, woven strip by strip and bound with a copper rim.",
        Head,
        600,
        item
    ),
    offer!(
        "star_badge",
        "Star Badge",
        "A violet star before your name.",
        Badge,
        300,
        item
    ),
    offer!(
        "crown_badge",
        "Crown Badge",
        "A golden crown before your name.",
        Badge,
        500,
        item
    ),
    offer!(
        "moon_badge",
        "Moon Badge",
        "A pale crescent moon before your name.",
        Badge,
        400,
        item
    ),
    offer!(
        "nova_owl",
        "Nova the Star Owl",
        "A small star owl that rides on your shoulder and glows in the dark.",
        Pet,
        1_000,
        item
    ),
    offer!(
        "cosmic_fox",
        "Astra the Cosmic Fox",
        "A fox spun from deep space, with starlit fur and a tail that ends in a glowing star.",
        Pet,
        1_500,
        item
    ),
    offer!(
        "stardust_trail",
        "Stardust Trail",
        "Sparkling violet dust follows you.",
        Trail,
        750,
        item
    ),
    offer!(
        "comet_trail",
        "Comet Trail",
        "An icy blue comet tail behind every step.",
        Trail,
        900,
        item
    ),
    offer!(
        "storm_aura",
        "Storm Aura",
        "Lightning crackles around you, bright even in the dark.",
        Aura,
        2_000,
        item
    ),
    offer!(
        "starlight_orbit",
        "Starlight Orbit",
        "A ring of golden stars circles your waist.",
        Aura,
        1_600,
        item
    ),
    offer!(
        "nebula_aura",
        "Nebula Aura",
        "Pink and violet stardust spirals up around you.",
        Aura,
        1_400,
        item
    ),
    offer!(
        "nebula_wings",
        "Nebula Wings",
        "Wings of drifting violet and pink nebula that beat slowly and shed stardust.",
        Wings,
        1_500,
        item
    ),
    offer!(
        "daylight",
        "Daylight",
        "A pale sword with a dark star guard, worn across your back.",
        Weapon,
        800,
        item,
        by "@gold3nz"
    ),
    offer!(
        "celestial_wings",
        "Celestial Wings",
        "Two great wings of starlight unfurl behind you, beat slowly and lift you off the ground, then scatter into stardust.",
        Emote,
        1_800
    ),
    offer!(
        "dark_ascension",
        "Dark Ascension",
        "Rise wrapped in dark energy, eyes burning violet, as it spirals round you, gathers in and bursts outward.",
        Emote,
        1_400
    ),
    offer!(
        "l_dance",
        "L Dance",
        "Hold an L to your forehead and bounce to a note-block beat, then swap hands.",
        Emote,
        450
    ),
    offer!(
        "bruh",
        "Bruh",
        "Clap a hand to your face, shake your head and sigh.",
        Emote,
        300
    ),
    offer!(
        "the_rook",
        "The Rook",
        "Call a giant chess rook down from the sky: it slams down, earns its Brilliant !!, and explodes forward.",
        Emote,
        700
    ),
    offer!(
        "false_dreams",
        "False Dreams",
        "Rise off the ground as if possessed while its song plays, watched by eyes that drift around you.",
        Emote,
        500
    ),
    offer!(
        "purple_hollow",
        "Purple Hollow",
        "Gather a blue orb and a red one, bring them together and fire the purple sphere they become.",
        Emote,
        1200
    ),
    offer!(
        "supernova_crown",
        "Supernova Crown",
        "A star gathers over your head and bursts, then settles into a crown of light.",
        Emote,
        800
    ),
    offer!(
        "black_hole",
        "Black Hole",
        "Tear open a black hole and orbit it as the world bends, then watch it collapse.",
        Emote,
        600
    ),
];

pub fn offer(id: &str) -> Option<&'static Offer> {
    OFFERS.iter().find(|o| o.id == id)
}

/// Pictures beside the items' own: the banner's slides and the Featured tab's highlight card.
pub const PICTURES: &[(&str, &[u8])] = &[
    (
        "banner_founders_cape.png",
        include_bytes!("../assets/shop/banner_founders_cape.png"),
    ),
    (
        "banner_nebula_cape.png",
        include_bytes!("../assets/shop/banner_nebula_cape.png"),
    ),
    (
        "banner_cosmic_fox.png",
        include_bytes!("../assets/shop/banner_cosmic_fox.png"),
    ),
    (
        "banner_storm_aura.png",
        include_bytes!("../assets/shop/banner_storm_aura.png"),
    ),
    (
        "banner_stardust_trail.png",
        include_bytes!("../assets/shop/banner_stardust_trail.png"),
    ),
    (
        "highlight.png",
        include_bytes!("../assets/shop/highlight.png"),
    ),
];

/// A shop picture by file name: an item's card (`<id>.png`) or one of `PICTURES`.
pub fn picture(name: &str) -> Option<&'static [u8]> {
    PICTURES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, bytes)| *bytes)
        .or_else(|| offer(name.strip_suffix(".png")?).map(|o| o.art))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    Unknown,
    Owned,
    NotOwned,
    /// Short of this many coins.
    Short(u32),
    /// The Founders Cape is claimed, not bought; other items aren't claimed.
    NotClaimable,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Refused::Unknown => crate::i18n::tr("This item isn't in the shop."),
            Refused::Owned => crate::i18n::tr("You already own this item."),
            Refused::NotOwned => crate::i18n::tr("Buy this item before equipping it."),
            Refused::Short(n) => crate::i18n::trf(
                "You need {coins} more Wisp Coins.",
                &[("coins", &n.to_string())],
            ),
            Refused::NotClaimable => crate::i18n::tr("This item can't be claimed."),
        };
        f.write_str(&text)
    }
}

/// The player's coins and items.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wallet {
    pub coins: u32,
    #[serde(default)]
    pub owned: Vec<String>,
    /// One item per kind, by id.
    #[serde(default)]
    pub equipped: Vec<String>,
    /// The day (since 1970) the last daily bonus was given.
    #[serde(default)]
    pub bonus_day: Option<u64>,
    #[serde(default)]
    welcomed: bool,
}

pub const FILE: &str = "shop.json";

pub fn today() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400)
}

impl Wallet {
    /// The saved wallet, or a new one.
    pub fn load(home: &Path) -> Self {
        fs::read(home.join(FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, home: &Path) -> std::io::Result<()> {
        fs::create_dir_all(home)?;
        let path = home.join(FILE);
        let temp = path.with_extension("json.tmp");
        fs::write(&temp, serde_json::to_vec_pretty(self)?)?;
        fs::rename(temp, path)
    }

    /// Gives the welcome gift once and the daily bonus once per `day`: the coins given.
    pub fn collect(&mut self, day: u64) -> u32 {
        let mut given = 0;
        if !self.welcomed {
            self.welcomed = true;
            given += WELCOME;
        }
        if self.bonus_day.is_none_or(|last| day > last) {
            self.bonus_day = Some(day);
            given += DAILY;
        }
        self.coins = self.coins.saturating_add(given);
        given
    }

    pub fn owns(&self, id: &str) -> bool {
        self.owned.iter().any(|o| o == id)
    }

    pub fn wears(&self, id: &str) -> bool {
        self.equipped.iter().any(|o| o == id)
    }

    pub fn buy(&mut self, id: &str) -> Result<(), Refused> {
        let offer = offer(id).ok_or(Refused::Unknown)?;
        if offer.founders() {
            return self.claim(id);
        }
        if self.owns(id) {
            return Err(Refused::Owned);
        }
        if self.coins < offer.price {
            return Err(Refused::Short(offer.price - self.coins));
        }
        self.coins -= offer.price;
        self.owned.push(id.to_owned());
        Ok(())
    }

    /// The Founders Cape: free, once.
    pub fn claim(&mut self, id: &str) -> Result<(), Refused> {
        if !offer(id).is_some_and(Offer::founders) {
            return Err(Refused::NotClaimable);
        }
        if self.owns(id) {
            return Err(Refused::Owned);
        }
        self.owned.push(id.to_owned());
        Ok(())
    }

    /// Wears `id`, taking off what was worn of its kind.
    pub fn equip(&mut self, id: &str) -> Result<(), Refused> {
        let offer = offer(id).ok_or(Refused::Unknown)?;
        if !self.owns(id) {
            return Err(Refused::NotOwned);
        }
        if self.wears(id) {
            return Ok(());
        }
        // One item of a kind (and of a slot: capes and wings) is worn; every emote can be in the
        // wheel.
        if offer.kind != Kind::Emote {
            self.equipped
                .retain(|e| offer_kind(e).is_some_and(|k| !k.shares_slot(offer.kind)));
        }
        self.equipped.push(id.to_owned());
        Ok(())
    }

    /// How many items (emotes included) are equipped.
    pub fn worn_count(&self) -> usize {
        self.equipped.len()
    }

    pub fn unequip(&mut self, id: &str) {
        self.equipped.retain(|e| e != id);
    }

    /// Writes the item files of what is worn into `dir`, removing any other, and the emotes in
    /// the wheel (`EMOTES_FILE`). Returns how many it wrote.
    pub fn dress(&self, dir: &Path) -> std::io::Result<usize> {
        fs::create_dir_all(dir)?;
        for entry in fs::read_dir(dir)?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") || path.ends_with(EMOTES_FILE) {
                fs::remove_file(path)?;
            }
        }
        let mut written = 0;
        let mut emotes = String::new();
        for id in &self.equipped {
            match offer(id) {
                Some(o) if o.kind == Kind::Emote => {
                    emotes.push_str(id);
                    emotes.push('\n');
                    written += 1;
                }
                Some(Offer {
                    item: Some(bytes), ..
                }) => {
                    fs::write(dir.join(format!("{id}.json")), bytes)?;
                    written += 1;
                }
                _ => {}
            }
        }
        if !emotes.is_empty() {
            fs::write(dir.join(EMOTES_FILE), emotes)?;
        }
        Ok(written)
    }
}

fn offer_kind(id: &str) -> Option<Kind> {
    offer(id).map(|o| o.kind)
}

/// The Wisp cosmetics server players share what they wear through
/// (`crates/wisp-cosmetics-server`). Empty: not deployed yet, and only the player sees their
/// own items. `WISP_COSMETICS_API` overrides it.
pub const SERVICE: &str = "";

/// The address set in the launcher's Settings, which wins over `SERVICE`.
static CHOSEN: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

/// Uses `url` (the launcher's Settings) for the Wisp server; empty or None: `SERVICE`.
pub fn set_service(url: Option<&str>) {
    let url = url
        .map(|s| s.trim().trim_end_matches('/').to_owned())
        .filter(|s| !s.is_empty());
    *CHOSEN
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = url;
}

/// The Wisp server (cosmetics and friends) to use, if any: `WISP_COSMETICS_API`, then the
/// address in the launcher's Settings, then `SERVICE`.
pub fn service() -> Option<String> {
    let chosen = CHOSEN
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    wisp_compat::var("WISP_COSMETICS_API")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or(chosen)
        .or_else(|| Some(SERVICE.to_owned()))
        .map(|s| s.trim().trim_end_matches('/').to_owned())
        .filter(|s| !s.is_empty())
}

/// The folder of worn item files, in the launcher's data folder.
pub fn worn_dir(home: &Path) -> PathBuf {
    home.join("shop").join("worn")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_item_file_is_one_the_game_reads() {
        for offer in OFFERS {
            assert!(!offer.art.is_empty(), "{}", offer.id);
            if offer.kind == Kind::Emote {
                // The game plays emotes itself: the wheel lists them by id.
                assert!(offer.item.is_none(), "{}", offer.id);
                continue;
            }
            let bytes = offer
                .item
                .unwrap_or_else(|| panic!("{} has no item file", offer.id));
            let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            assert_eq!(v["id"], offer.id);
            let slot = match offer.kind {
                Kind::Cape | Kind::Wings => "back",
                Kind::Weapon => "weapon",
                Kind::Head => "head",
                Kind::Badge => "name_tag",
                Kind::Pet => "shoulder",
                Kind::Trail => "trail",
                Kind::Aura => "aura",
                Kind::Emote => unreachable!(),
            };
            assert_eq!(v["slot"], slot, "{}", offer.id);
        }
    }

    #[test]
    fn wings_and_a_cape_take_each_other_off() {
        let dir = std::env::temp_dir().join(format!("wisp-shop-wings-{}", std::process::id()));
        let mut w = Wallet::load(&dir);
        w.coins = 10_000;
        w.buy("nebula_wings").unwrap();
        w.buy("nebula_cape").unwrap();
        w.buy("daylight").unwrap();
        w.equip("nebula_cape").unwrap();
        w.equip("daylight").unwrap();
        w.equip("nebula_wings").unwrap();
        assert!(w.wears("nebula_wings") && w.wears("daylight") && !w.wears("nebula_cape"));
        w.equip("nebula_cape").unwrap();
        assert!(!w.wears("nebula_wings") && w.wears("nebula_cape"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_picture_is_a_png() {
        let names = OFFERS
            .iter()
            .map(|o| format!("{}.png", o.id))
            .chain(PICTURES.iter().map(|(n, _)| n.to_string()));
        for name in names {
            let bytes = picture(&name).unwrap_or_else(|| panic!("{name}"));
            assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']), "{name}");
        }
        assert!(picture("missing.png").is_none());
    }

    #[test]
    fn coins_come_once_a_day_after_the_welcome_gift() {
        let mut w = Wallet::default();
        assert_eq!(w.collect(100), WELCOME + DAILY);
        assert_eq!(w.collect(100), 0);
        assert_eq!(w.collect(101), DAILY);
        // A clock turned back gives nothing.
        assert_eq!(w.collect(90), 0);
        assert_eq!(w.coins, WELCOME + 2 * DAILY);
    }

    #[test]
    fn buying_spends_coins_once_and_founders_is_free() {
        let mut w = Wallet {
            coins: 1_000,
            ..Wallet::default()
        };
        assert_eq!(w.buy("nebula_cape"), Err(Refused::Short(200)));
        assert_eq!(w.buy("nova_owl"), Ok(()));
        assert_eq!(w.coins, 0);
        assert_eq!(w.buy("nova_owl"), Err(Refused::Owned));
        assert_eq!(w.buy("founders_cape"), Ok(()));
        assert_eq!(w.coins, 0);
        assert_eq!(w.claim("founders_cape"), Err(Refused::Owned));
        assert_eq!(w.claim("star_badge"), Err(Refused::NotClaimable));
        assert_eq!(w.buy("nothing"), Err(Refused::Unknown));
    }

    #[test]
    fn one_item_per_kind_is_worn_and_written_for_the_game() {
        let mut w = Wallet {
            coins: 5_000,
            ..Wallet::default()
        };
        for id in [
            "founders_cape",
            "aurora_cape",
            "star_badge",
            "stardust_trail",
        ] {
            w.buy(id).unwrap();
        }
        assert_eq!(w.equip("moon_badge"), Err(Refused::NotOwned));
        w.equip("founders_cape").unwrap();
        w.equip("aurora_cape").unwrap();
        w.equip("star_badge").unwrap();
        w.equip("stardust_trail").unwrap();
        assert!(!w.wears("founders_cape"));
        assert_eq!(w.equipped, ["aurora_cape", "star_badge", "stardust_trail"]);

        let dir = std::env::temp_dir().join(format!("wisp-shop-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("old_item.json"), b"{}").unwrap();
        assert_eq!(w.dress(&dir).unwrap(), 3);
        let mut names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(
            names,
            ["aurora_cape.json", "star_badge.json", "stardust_trail.json"]
        );
        w.unequip("aurora_cape");
        w.save(&dir).unwrap();
        assert_eq!(Wallet::load(&dir), w);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bought_emote_goes_in_the_wheel_beside_worn_items() {
        let mut w = Wallet {
            coins: 1_000,
            ..Wallet::default()
        };
        w.buy("black_hole").unwrap();
        assert_eq!(w.coins, 400);
        w.equip("black_hole").unwrap();
        w.buy("star_badge").unwrap();
        w.equip("star_badge").unwrap();
        assert!(w.wears("black_hole") && w.wears("star_badge"));
        let dir = std::env::temp_dir().join(format!("wisp-emotes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(w.dress(&dir).unwrap(), 2);
        assert_eq!(
            fs::read_to_string(dir.join(EMOTES_FILE)).unwrap(),
            "black_hole\n"
        );
        w.unequip("black_hole");
        w.dress(&dir).unwrap();
        assert!(!dir.join(EMOTES_FILE).exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
