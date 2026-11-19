//! Wisp's Shop, shared by the launcher, the game and the Wisp server: the items (their prices,
//! cards and item files), what one player owns and wears ([`Wallet`]), and the rules for buying
//! and wearing them.
//!
//! The Wisp server keeps every player's wallet (`crates/wisp-cosmetics-server/src/shop.rs`), so
//! nobody gives themselves coins: coins come from a welcome gift and a daily bonus, and a
//! creator's share of what their supporters buy. The launcher and the game show the wallet the
//! server answers and ask it to buy and wear. Equipped capes, weapons, hats, pets and trails are
//! written as cosmetic item files (`docs/cosmetics.md`) into the launcher's `shop/worn`, which
//! the game reads through `WISP_COSMETICS_LOCAL`.
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
    /// A bag on the back (Santa's sack), beside a cape: it takes a weapon's place.
    Bag,
    /// Worn on the head (a hat); a helmet hides it.
    Head,
    /// Worn over the eyes, beside a hat; a helmet hides them.
    Glasses,
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
        let carried = |k: Kind| matches!(k, Kind::Weapon | Kind::Bag);
        self == other || (back(self) && back(other)) || (carried(self) && carried(other))
    }

    pub const ALL: [Kind; 10] = [
        Kind::Cape,
        Kind::Wings,
        Kind::Weapon,
        Kind::Bag,
        Kind::Head,
        Kind::Glasses,
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
            item: Some(include_bytes!(concat!("../assets/", $id, ".json"))),
            art: include_bytes!(concat!("../assets/", $id, ".png")),
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
            art: include_bytes!(concat!("../assets/", $id, ".png")),
            creator: None,
        }
    };
}

/// The Wisp Founders Cape: Wisp's feather in HD (12× Minecraft's cape), animated.
const FOUNDERS_CAPE: Offer = offer!(
    "founders_cape",
    "Wisp Founders Cape",
    "Wisp's feather in HD on a night sky, a gold hem and light that never stops moving. Free for Wisp's first players.",
    Cape,
    0,
    item
);

// Prices: 100 coins are worth 0.80 €, so 1,000 coins are 8 €.

/// Demon Wings: HD bat wings. 1,000 coins (8 €).
const DEMON_WINGS: Offer = offer!(
    "demon_wings",
    "Demon Wings",
    "Big bat wings of solid black bones with an ember-red skin and ivory claws, opening and folding as they beat.",
    Wings,
    1_000,
    item
);

/// Angel Wings: HD feathered wings that glow. 1,000 coins (8 €).
const ANGEL_WINGS: Offer = offer!(
    "angel_wings",
    "Angel Wings",
    "White feathered wings on a gold arm that open and close as they beat, bright even at night.",
    Wings,
    1_000,
    item
);

/// The Bamboo Sword: a bamboo stalk carried on the back like a sword. 500 coins (4 €).
const BAMBOO_SWORD: Offer = offer!(
    "bamboo_sword",
    "Bamboo Sword",
    "A cut bamboo stalk with a bamboo guard and a twine grip, slung across your back like a blade.",
    Weapon,
    500,
    item
);

/// The Stray Head: a conical straw hat in stepped layers, sized for Minecraft's head. 400 coins (3.20 €).
const STRAY_HEAD: Offer = offer!(
    "stray_head",
    "Stray Head",
    "A conical straw hat with a copper band, stitched in red and tied with a white knot.",
    Head,
    400,
    item
);

/// The Lavender cape: an animated HD lavender field on a green hill. 800 coins (6.40 €).
const LAVENDER_CAPE: Offer = offer!(
    "lavender_cape",
    "Lavender",
    "Lavender swaying on a green hill under a summer sky, clouds drifting by and a butterfly fluttering past.",
    Cape,
    800,
    item
);

/// Devil Horns: two curved crimson horns. 400 coins (3.20 €).
const DEVIL_HORNS: Offer = offer!(
    "devil_horns",
    "Devil Horns",
    "Two curved horns, dark crimson to a black point, sweeping out and up from your head.",
    Head,
    400,
    item
);

/// Angel Halo: a glowing gold ring over the head. 400 coins (3.20 €).
const ANGEL_HALO: Offer = offer!(
    "angel_halo",
    "Angel Halo",
    "A ring of glowing gold floating over your head, turning slowly.",
    Head,
    400,
    item
);

/// Santa Hat: red velvet with a white fur brim and pompom. 500 coins (4 €).
const SANTA_HAT: Offer = offer!(
    "santa_hat",
    "Santa Hat",
    "Red velvet with a white fur brim, its tip flopping over to one side with a pompom that swings as you move.",
    Head,
    500,
    item
);

/// Santa's Sack: a burlap sack over the back with presents. 700 coins (5.60 €).
const SANTA_SACK: Offer = offer!(
    "santa_sack",
    "Santa's Sack",
    "A big burlap sack slung over your back, tied with gold rope, a present and a candy cane poking out of the top.",
    Bag,
    700,
    item
);

/// Void Dragon Wings. 1,000 coins (8 €).
const VOID_DRAGON_WINGS: Offer = offer!(
    "void_dragon_wings",
    "Void Dragon Wings",
    "Huge dragon wings of near-black bone with a violet skin that glows lilac at its scalloped edge, beating slowly.",
    Wings,
    1_000,
    item
);

/// Sakura Wings. 1,000 coins (8 €).
const SAKURA_WINGS: Offer = offer!(
    "sakura_wings",
    "Sakura Wings",
    "Pink feathered wings dotted with cherry blossoms on a dark cherry-wood arm, opening and closing as they beat.",
    Wings,
    1_000,
    item
);

/// Crimson Katana. 600 coins (4.80 €).
const CRIMSON_KATANA: Offer = offer!(
    "crimson_katana",
    "Crimson Katana",
    "A katana in a red lacquered scabbard with gold fittings, a gold guard and a grip wound in black, slung across your back.",
    Weapon,
    600,
    item
);

/// Gearwork Topper. 500 coins (4 €).
const GEARWORK_TOPPER: Offer = offer!(
    "gearwork_topper",
    "Gearwork Topper",
    "A tall leather top hat with a gold band and brass goggles resting on the front.",
    Head,
    500,
    item
);

/// Spelunker Helmet. 400 coins (3.20 €).
const SPELUNKER_HELMET: Offer = offer!(
    "spelunker_helmet",
    "Spelunker Helmet",
    "A yellow hard hat with a black stripe over the top and a bright lamp on the front.",
    Head,
    400,
    item
);

/// Frost Wizard Hat. 500 coins (4 €).
const FROST_WIZARD_HAT: Offer = offer!(
    "frost_wizard_hat",
    "Frost Wizard Hat",
    "A pointed ice-blue wizard hat bending back at the tip, a leather band with a gold buckle and a gold star at the point.",
    Head,
    500,
    item
);

/// Tangerine Topper. 400 coins (3.20 €).
const TANGERINE_TOPPER: Offer = offer!(
    "tangerine_topper",
    "Tangerine Topper",
    "A tall orange hat with a white stripe, its crown leaning over to one side.",
    Head,
    400,
    item
);

/// Panda Cap. 400 coins (3.20 €).
const PANDA_CAP: Offer = offer!(
    "panda_cap",
    "Panda Cap",
    "A soft white cap with a panda's face on the front, round black ears and cheek flaps.",
    Head,
    400,
    item
);

/// Buccaneer Tricorn. 500 coins (4 €).
const BUCCANEER_TRICORN: Offer = offer!(
    "buccaneer_tricorn",
    "Buccaneer Tricorn",
    "A brown pirate hat, its brim turned up all round and trimmed in gold, a little skull on the front.",
    Head,
    500,
    item
);

/// Orbit Cap. 600 coins (4.80 €).
const ORBIT_CAP: Offer = offer!(
    "orbit_cap",
    "Orbit Cap",
    "A starry deep-blue dome with a tilted ring round it and three little planets circling your head.",
    Head,
    600,
    item
);

/// Propeller Cap. 400 coins (3.20 €).
const PROPELLER_CAP: Offer = offer!(
    "propeller_cap",
    "Propeller Cap",
    "A patchwork cap in red, green and yellow with a blue grid visor and a propeller spinning on top.",
    Head,
    400,
    item
);

/// Neon Headset. 500 coins (4 €).
const NEON_HEADSET: Offer = offer!(
    "neon_headset",
    "Neon Headset",
    "A cyan visor wrapped round your eyes, dark ear cups that glow, a band over the top and two antennas.",
    Head,
    500,
    item
);

/// Blossom Shades. 400 coins (3.20 €).
const BLOSSOM_SHADES: Offer = offer!(
    "blossom_shades",
    "Blossom Shades",
    "Pink flower-shaped glasses with rosy lenses. Worn with any hat.",
    Glasses,
    400,
    item
);

/// Butterfly Swarm. 800 coins (6.40 €).
const BUTTERFLY_SWARM: Offer = offer!(
    "butterfly_swarm",
    "Butterfly Swarm",
    "Six pixel butterflies, blue and orange, fluttering round you on three rings at different heights.",
    Aura,
    800,
    item
);

/// Astro Buddy. 900 coins (7.20 €).
const ASTRO_BUDDY: Offer = offer!(
    "astro_buddy",
    "Astro Buddy",
    "A little astronaut in a black suit floating by your shoulder, a happy face behind its gold visor and a jetpack puffing sparks.",
    Pet,
    900,
    item
);

/// What the shop sells.
#[cfg(not(any(test, feature = "test-items")))]
pub const OFFERS: &[Offer] = &[
    FOUNDERS_CAPE,
    LAVENDER_CAPE,
    DEMON_WINGS,
    ANGEL_WINGS,
    BAMBOO_SWORD,
    STRAY_HEAD,
    DEVIL_HORNS,
    ANGEL_HALO,
    SANTA_HAT,
    SANTA_SACK,
    VOID_DRAGON_WINGS,
    SAKURA_WINGS,
    CRIMSON_KATANA,
    GEARWORK_TOPPER,
    SPELUNKER_HELMET,
    FROST_WIZARD_HAT,
    TANGERINE_TOPPER,
    PANDA_CAP,
    BUCCANEER_TRICORN,
    ORBIT_CAP,
    PROPELLER_CAP,
    NEON_HEADSET,
    BLOSSOM_SHADES,
    BUTTERFLY_SWARM,
    ASTRO_BUDDY,
];

/// With `test-items` (and in this crate's tests), made-up items beside the real ones, so the
/// rules (prices, discounts, one item per kind, emotes) can be tested while the shop sells
/// little. Never in a build players get.
#[cfg(any(test, feature = "test-items"))]
pub const OFFERS: &[Offer] = &[
    FOUNDERS_CAPE,
    LAVENDER_CAPE,
    DEMON_WINGS,
    ANGEL_WINGS,
    BAMBOO_SWORD,
    STRAY_HEAD,
    DEVIL_HORNS,
    ANGEL_HALO,
    SANTA_HAT,
    SANTA_SACK,
    VOID_DRAGON_WINGS,
    SAKURA_WINGS,
    CRIMSON_KATANA,
    GEARWORK_TOPPER,
    SPELUNKER_HELMET,
    FROST_WIZARD_HAT,
    TANGERINE_TOPPER,
    PANDA_CAP,
    BUCCANEER_TRICORN,
    ORBIT_CAP,
    PROPELLER_CAP,
    NEON_HEADSET,
    BLOSSOM_SHADES,
    BUTTERFLY_SWARM,
    ASTRO_BUDDY,
    test_offer("test_cape", Kind::Cape, 1_200),
    test_offer("test_wings", Kind::Wings, 900),
    test_offer("test_hat", Kind::Head, 1_000),
    test_offer("test_trail", Kind::Trail, 500),
    test_offer("test_emote", Kind::Emote, 600),
];

#[cfg(any(test, feature = "test-items"))]
const fn test_offer(id: &'static str, kind: Kind, price: u32) -> Offer {
    Offer {
        id,
        name: id,
        blurb: "",
        kind,
        price,
        item: None,
        art: FOUNDERS_CAPE.art,
        creator: None,
    }
}

pub fn offer(id: &str) -> Option<&'static Offer> {
    OFFERS.iter().find(|o| o.id == id)
}

/// Pictures beside the items' own: the banner's slides and the Featured tab's highlight card.
pub const PICTURES: &[(&str, &[u8])] = &[(
    "banner_founders_cape.png",
    include_bytes!("../assets/banner_founders_cape.png"),
)];

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

impl Refused {
    /// What to tell the player, in English (the launcher and the game translate it), with the
    /// coins missing for `Short` as `{coins}`.
    pub fn message(&self) -> &'static str {
        match self {
            Refused::Unknown => "This item isn't in the shop.",
            Refused::Owned => "You already own this item.",
            Refused::NotOwned => "Buy this item before equipping it.",
            Refused::Short(_) => "You need {coins} more Wisp Coins.",
            Refused::NotClaimable => "This item can't be claimed.",
        }
    }
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::Short(n) => f.write_str(&self.message().replace("{coins}", &n.to_string())),
            _ => f.write_str(self.message()),
        }
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
    pub welcomed: bool,
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

    /// Buys `id` for its price less `discount` percent (a supported creator's): the coins paid.
    pub fn buy_discounted(&mut self, id: &str, discount: u32) -> Result<u32, Refused> {
        let offer = offer(id).ok_or(Refused::Unknown)?;
        if offer.founders() {
            return self.claim(id).map(|()| 0);
        }
        if self.owns(id) {
            return Err(Refused::Owned);
        }
        let price = discounted(offer.price, discount);
        if self.coins < price {
            return Err(Refused::Short(price - self.coins));
        }
        self.coins -= price;
        self.owned.push(id.to_owned());
        Ok(price)
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

/// `price` less `discount` percent, rounded to the nearest coin.
pub fn discounted(price: u32, discount: u32) -> u32 {
    let off = (u64::from(price) * u64::from(discount.min(100)) + 50) / 100;
    price - off as u32
}

/// A creator's share of an item at its full `price`: `percent` of it, rounded down.
pub fn share(price: u32, percent: u32) -> u32 {
    (u64::from(price) * u64::from(percent.min(100)) / 100) as u32
}

fn offer_kind(id: &str) -> Option<Kind> {
    offer(id).map(|o| o.kind)
}

/// The folder of worn item files, in the launcher's data folder.
pub fn worn_dir(home: &Path) -> PathBuf {
    home.join("shop").join("worn")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The items players can get (the made-up test items left out).
    fn real() -> impl Iterator<Item = &'static Offer> {
        OFFERS.iter().filter(|o| !o.id.starts_with("test_"))
    }

    #[test]
    fn every_real_item_has_a_file_the_game_reads() {
        for offer in real() {
            assert!(!offer.art.is_empty(), "{}", offer.id);
            let bytes = offer
                .item
                .unwrap_or_else(|| panic!("{} has no item file", offer.id));
            let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            assert_eq!(v["id"], offer.id);
            let slot = match offer.kind {
                Kind::Cape | Kind::Wings => "back",
                Kind::Weapon | Kind::Bag => "weapon",
                Kind::Head | Kind::Glasses => "head",
                Kind::Pet | Kind::Aura => "shoulder",
                other => panic!("{}: no slot for {other:?}", offer.id),
            };
            assert_eq!(v["slot"], slot, "{}", offer.id);
        }
    }

    #[test]
    fn a_bag_takes_a_weapons_place_beside_a_cape() {
        assert!(Kind::Bag.shares_slot(Kind::Weapon));
        assert!(Kind::Weapon.shares_slot(Kind::Bag));
        assert!(!Kind::Bag.shares_slot(Kind::Cape));
        assert!(!Kind::Bag.shares_slot(Kind::Head));
        assert_eq!(offer("santa_sack").unwrap().kind, Kind::Bag);
        assert_eq!(offer("santa_hat").unwrap().kind, Kind::Head);
    }

    #[test]
    fn glasses_are_worn_beside_a_hat() {
        assert!(!Kind::Glasses.shares_slot(Kind::Head));
        assert!(Kind::Glasses.shares_slot(Kind::Glasses));
        let mut w = Wallet {
            coins: 10_000,
            ..Wallet::default()
        };
        w.buy("blossom_shades").unwrap();
        w.buy("panda_cap").unwrap();
        w.equip("blossom_shades").unwrap();
        w.equip("panda_cap").unwrap();
        assert!(w.wears("blossom_shades") && w.wears("panda_cap"));
    }

    #[test]
    fn the_shop_sells_the_founders_cape() {
        let ids: Vec<_> = real().map(|o| o.id).collect();
        assert_eq!(
            ids,
            [
                FOUNDERS,
                "lavender_cape",
                "demon_wings",
                "angel_wings",
                "bamboo_sword",
                "stray_head",
                "devil_horns",
                "angel_halo",
                "santa_hat",
                "santa_sack",
                "void_dragon_wings",
                "sakura_wings",
                "crimson_katana",
                "gearwork_topper",
                "spelunker_helmet",
                "frost_wizard_hat",
                "tangerine_topper",
                "panda_cap",
                "buccaneer_tricorn",
                "orbit_cap",
                "propeller_cap",
                "neon_headset",
                "blossom_shades",
                "butterfly_swarm",
                "astro_buddy"
            ]
        );
        let cape = offer(FOUNDERS).unwrap();
        assert!(cape.founders() && cape.price == 0 && cape.kind == Kind::Cape);
        // HD and animated: a 12× cape (at a twelfth of the scale), more than one frame.
        let v: serde_json::Value = serde_json::from_slice(cape.item.unwrap()).unwrap();
        assert!(v["texture"]["frames"].as_u64().unwrap() > 1);
        let scale = v["attach"]["scale"].as_f64().unwrap();
        assert!((scale - 1.0 / 12.0).abs() < 1e-6, "{scale}");
    }

    #[test]
    fn wings_and_a_cape_take_each_other_off() {
        let mut w = Wallet {
            coins: 10_000,
            ..Wallet::default()
        };
        w.buy("test_wings").unwrap();
        w.buy("test_cape").unwrap();
        w.buy("test_hat").unwrap();
        w.equip("test_cape").unwrap();
        w.equip("test_hat").unwrap();
        w.equip("test_wings").unwrap();
        assert!(w.wears("test_wings") && w.wears("test_hat") && !w.wears("test_cape"));
        w.equip("test_cape").unwrap();
        assert!(!w.wears("test_wings") && w.wears("test_cape"));
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
        assert_eq!(w.buy("test_cape"), Err(Refused::Short(200)));
        assert_eq!(w.buy("test_hat"), Ok(()));
        assert_eq!(w.coins, 0);
        assert_eq!(w.buy("test_hat"), Err(Refused::Owned));
        assert_eq!(w.buy(FOUNDERS), Ok(()));
        assert_eq!(w.coins, 0);
        assert_eq!(w.claim(FOUNDERS), Err(Refused::Owned));
        assert_eq!(w.claim("test_trail"), Err(Refused::NotClaimable));
        assert_eq!(w.buy("nothing"), Err(Refused::Unknown));
    }

    #[test]
    fn one_item_per_kind_is_worn_and_written_for_the_game() {
        let mut w = Wallet {
            coins: 5_000,
            ..Wallet::default()
        };
        for id in [FOUNDERS, "test_cape", "test_hat", "test_trail"] {
            w.buy(id).unwrap();
        }
        assert_eq!(w.equip("test_wings"), Err(Refused::NotOwned));
        w.equip("test_cape").unwrap();
        w.equip("test_hat").unwrap();
        w.equip("test_trail").unwrap();
        w.equip(FOUNDERS).unwrap();
        assert!(!w.wears("test_cape"));
        assert_eq!(w.equipped, ["test_hat", "test_trail", FOUNDERS]);

        let dir = std::env::temp_dir().join(format!("wisp-shop-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("old_item.json"), b"{}").unwrap();
        // Only items with a file are written (the test items have none); old files go.
        assert_eq!(w.dress(&dir).unwrap(), 1);
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, ["founders_cape.json"]);
        w.unequip(FOUNDERS);
        w.save(&dir).unwrap();
        assert_eq!(Wallet::load(&dir), w);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bought_emote_goes_in_the_wheel_beside_worn_items() {
        let mut w = Wallet {
            coins: 1_200,
            ..Wallet::default()
        };
        w.buy("test_emote").unwrap();
        assert_eq!(w.coins, 600);
        w.equip("test_emote").unwrap();
        w.buy(FOUNDERS).unwrap();
        w.equip(FOUNDERS).unwrap();
        assert!(w.wears("test_emote") && w.wears(FOUNDERS));
        let dir = std::env::temp_dir().join(format!("wisp-emotes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(w.dress(&dir).unwrap(), 2);
        assert_eq!(
            fs::read_to_string(dir.join(EMOTES_FILE)).unwrap(),
            "test_emote\n"
        );
        w.unequip("test_emote");
        w.dress(&dir).unwrap();
        assert!(!dir.join(EMOTES_FILE).exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
