//! The Wisp Menu's features (`docs/extras.md`): every client module and built-in extra the menu
//! lists, its category, icon and settings, and the HUD widgets' shared look. A feature's switch is
//! its `wisp-extras.txt` line (a module row of [`super::MODULES`] or one of the older extras'
//! fields); its settings are `opt.<feature>.<setting>:<value>` lines in the same file
//! ([`super::Extras::options`]), so the launcher's Mods page and older clients keep working.
use super::{Extras, module_index};

/// A tab of the menu's sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Hud,
    Combat,
    Visual,
    Utility,
    General,
}
impl Category {
    pub const ALL: [Category; 5] = [
        Category::Hud,
        Category::Combat,
        Category::Visual,
        Category::Utility,
        Category::General,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Category::Hud => "HUD",
            Category::Combat => "Combat",
            Category::Visual => "Visual",
            Category::Utility => "Utility",
            Category::General => "General",
        }
    }
    pub fn lang_key(self) -> &'static str {
        match self {
            Category::Hud => "wisp.menu.category.hud",
            Category::Combat => "wisp.menu.category.combat",
            Category::Visual => "wisp.menu.category.visual",
            Category::Utility => "wisp.menu.category.utility",
            Category::General => "wisp.menu.category.general",
        }
    }
}

/// What a setting's control is, with its default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Toggle(bool),
    /// A value from `min` to `max` in steps of `step`, shown with `unit`.
    Slider {
        min: f32,
        max: f32,
        step: f32,
        default: f32,
        unit: &'static str,
    },
    /// A colour (`0xRRGGBB`) picked from [`SWATCHES`].
    Color(u32),
    /// One of the names; the default's index.
    Choice(&'static [&'static str], usize),
    /// A vanilla key name (`key.keyboard.h`).
    Key(&'static str),
}

/// One setting of a feature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Setting {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: Kind,
}

/// One card of the menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Feature {
    /// The switch's key: a module key (`hud_fps`) or an older extra's (`fullbright`).
    pub key: &'static str,
    pub category: Category,
    /// The item drawn on the card (`minecraft:clock`).
    pub icon: &'static str,
    /// Its own settings; a HUD widget also has [`WIDGET_SETTINGS`].
    pub settings: &'static [Setting],
    /// A HUD widget: placed in the HUD editor, with the widget look's settings.
    pub widget: bool,
}

/// The colours a colour setting offers (`0xRRGGBB`): Wisp's violets first.
pub const SWATCHES: [u32; 12] = [
    0x8B6BFF, 0xC4B5FD, 0xFFFFFF, 0xFF7AC6, 0xFF6B6B, 0xFFA94D, 0xFFD43B, 0x4ADE80, 0x4DD8FF,
    0x6B8BFF, 0x9AA0B4, 0x1F1B33,
];

const fn toggle(key: &'static str, label: &'static str, on: bool) -> Setting {
    Setting {
        key,
        label,
        kind: Kind::Toggle(on),
    }
}
const fn choice(
    key: &'static str,
    label: &'static str,
    names: &'static [&'static str],
    default: usize,
) -> Setting {
    Setting {
        key,
        label,
        kind: Kind::Choice(names, default),
    }
}
const fn color(key: &'static str, label: &'static str, default: u32) -> Setting {
    Setting {
        key,
        label,
        kind: Kind::Color(default),
    }
}
const fn slider(
    key: &'static str,
    label: &'static str,
    (min, max, step, default): (f32, f32, f32, f32),
    unit: &'static str,
) -> Setting {
    Setting {
        key,
        label,
        kind: Kind::Slider {
            min,
            max,
            step,
            default,
            unit,
        },
    }
}
const fn key(key: &'static str, label: &'static str, default: &'static str) -> Setting {
    Setting {
        key,
        label,
        kind: Kind::Key(default),
    }
}

/// Every HUD widget's look, after its own settings.
pub const WIDGET_SETTINGS: &[Setting] = &[
    slider("scale", "Size", (0.5, 2.0, 0.05, 1.0), "x"),
    toggle("background", "Background", true),
    slider(
        "opacity",
        "Background opacity",
        (0.0, 100.0, 5.0, 70.0),
        "%",
    ),
    color("text", "Text colour", 0xFFFFFF),
    color("accent", "Accent colour", 0x8B6BFF),
    toggle("shadow", "Text shadow", true),
];

const DURABILITY: &[&str] = &["Bar", "Number", "Percent", "Bar and number"];
const LAYOUT: &[&str] = &["Vertical", "Horizontal"];

macro_rules! feature {
    ($key:literal, $cat:ident, $icon:literal, widget, [$($s:expr),* $(,)?]) => {
        Feature { key: $key, category: Category::$cat, icon: $icon, settings: &[$($s),*], widget: true }
    };
    ($key:literal, $cat:ident, $icon:literal, [$($s:expr),* $(,)?]) => {
        Feature { key: $key, category: Category::$cat, icon: $icon, settings: &[$($s),*], widget: false }
    };
}

/// Every card, in the menu's order.
pub const FEATURES: &[Feature] = &[
    // HUD widgets.
    feature!(
        "hud_fps",
        Hud,
        "minecraft:redstone",
        widget,
        [toggle("low", "Show 1% low", false)]
    ),
    feature!(
        "hud_cps",
        Hud,
        "minecraft:stone_button",
        widget,
        [toggle("right", "Right clicks too", true)]
    ),
    feature!(
        "hud_keystrokes",
        Hud,
        "minecraft:oak_button",
        widget,
        [
            toggle("mouse", "Mouse buttons", true),
            toggle("space", "Space bar", true),
            toggle("cps", "Clicks per second", true),
        ]
    ),
    feature!(
        "hud_ping",
        Hud,
        "minecraft:ender_pearl",
        widget,
        [toggle("bars", "Signal bars", true)]
    ),
    feature!(
        "hud_coords",
        Hud,
        "minecraft:compass",
        widget,
        [
            choice("layout", "Layout", &["One line", "Stacked"], 1),
            toggle("direction", "Facing", true),
        ]
    ),
    feature!(
        "hud_direction",
        Hud,
        "minecraft:recovery_compass",
        widget,
        []
    ),
    feature!("hud_biome", Hud, "minecraft:grass_block", widget, []),
    feature!(
        "hud_speed",
        Hud,
        "minecraft:feather",
        widget,
        [choice("unit", "Unit", &["Blocks/s", "km/h"], 0)]
    ),
    feature!(
        "hud_clock",
        Hud,
        "minecraft:clock",
        widget,
        [
            choice("format", "Format", &["24-hour", "12-hour"], 0),
            toggle("seconds", "Seconds", false),
        ]
    ),
    feature!(
        "hud_day_time",
        Hud,
        "minecraft:sunflower",
        widget,
        [toggle("day", "Day number", true)]
    ),
    feature!("hud_weather", Hud, "minecraft:water_bucket", widget, []),
    feature!("hud_server", Hud, "minecraft:name_tag", widget, []),
    feature!(
        "hud_effects",
        Hud,
        "minecraft:potion",
        widget,
        [
            choice("layout", "Layout", LAYOUT, 0),
            toggle("names", "Effect names", true),
        ]
    ),
    feature!(
        "armor_status",
        Hud,
        "minecraft:diamond_chestplate",
        widget,
        [
            choice("layout", "Layout", LAYOUT, 0),
            choice("durability", "Durability", DURABILITY, 0),
            toggle("mainhand", "Main hand", false),
            toggle("offhand", "Off hand", false),
        ]
    ),
    feature!(
        "hud_durability",
        Hud,
        "minecraft:iron_pickaxe",
        widget,
        [choice("durability", "Durability", DURABILITY, 3)]
    ),
    feature!(
        "hud_arrows",
        Hud,
        "minecraft:arrow",
        widget,
        [toggle("bow", "Only with a bow", true)]
    ),
    feature!(
        "hit_info",
        Hud,
        "minecraft:iron_sword",
        widget,
        [
            toggle("reach", "Reach", true),
            toggle("combo", "Combo", true),
        ]
    ),
    // Combat.
    feature!(
        "health_indicators",
        Combat,
        "minecraft:golden_apple",
        [
            choice("style", "Style", &["Number", "Hearts"], 0),
            toggle("players", "Players only", false),
            slider("range", "Range", (8.0, 48.0, 4.0, 24.0), " blocks"),
        ]
    ),
    feature!(
        "damage_numbers",
        Combat,
        "minecraft:redstone_block",
        [
            color("color", "Damage colour", 0xFF6B6B),
            toggle("healing", "Healing too", true),
        ]
    ),
    feature!(
        "tnt_timer",
        Combat,
        "minecraft:tnt",
        [choice("decimals", "Decimals", &["0", "1", "2"], 2)]
    ),
    feature!(
        "target_crosshair",
        Combat,
        "minecraft:target",
        [color("color", "Colour", 0xFF6B6B)]
    ),
    feature!(
        "item_highlighter",
        Combat,
        "minecraft:glowstone_dust",
        [
            choice("mode", "Colour", &["Rarity", "Custom"], 0),
            color("color", "Custom colour", 0x8B6BFF),
        ]
    ),
    // Visual.
    feature!(
        "freelook",
        Visual,
        "minecraft:ender_eye",
        [
            key("key", "Key", "key.keyboard.left.alt"),
            choice("mode", "Mode", &["Hold", "Toggle"], 0),
        ]
    ),
    feature!(
        "always_day",
        Visual,
        "minecraft:daylight_detector",
        [choice(
            "time",
            "Time",
            &["Noon", "Sunrise", "Sunset", "Night"],
            0
        ),]
    ),
    feature!("item_physics", Visual, "minecraft:diamond", []),
    feature!(
        "arrow_trails",
        Visual,
        "minecraft:spectral_arrow",
        [
            choice(
                "particle",
                "Particle",
                &["Sparks", "Flames", "Hearts", "Notes", "Soul fire"],
                0
            ),
            slider("density", "Density", (1.0, 8.0, 1.0, 6.0), ""),
        ]
    ),
    feature!("fullbright", Visual, "minecraft:torch", []),
    feature!("dynamicLights", Visual, "minecraft:lantern", []),
    feature!("threeDSkinLayers", Visual, "minecraft:player_head", []),
    feature!("hideBlockParticles", Visual, "minecraft:cobblestone", []),
    feature!("hitboxes", Visual, "minecraft:scaffolding", []),
    // Utility.
    feature!(
        "chat_highlights",
        Utility,
        "minecraft:bell",
        [
            color("color", "Colour", 0xFFA94D),
            toggle("sound", "Ping sound", true),
        ]
    ),
    feature!(
        "chat_peek",
        Utility,
        "minecraft:writable_book",
        [key("key", "Key", "key.keyboard.h")]
    ),
    feature!(
        "waypoints",
        Utility,
        "minecraft:lodestone",
        [
            color("color", "Colour", 0x4DD8FF),
            toggle("distance", "Distance", true),
        ]
    ),
    feature!("inventory_search", Utility, "minecraft:spyglass", []),
    feature!("shulker_preview", Utility, "minecraft:shulker_box", []),
    feature!("mouseTweaks", Utility, "minecraft:chest", []),
    feature!("sort", Utility, "minecraft:hopper", []),
    // General.
    feature!("ownNameTag", General, "minecraft:name_tag", []),
    feature!("chatAnimation", General, "minecraft:paper", []),
    feature!("autoReconnect", General, "minecraft:repeater", []),
    feature!("fast_items", General, "minecraft:emerald", []),
    feature!("smart_f3", General, "minecraft:command_block", []),
    feature!("apple_skin", General, "minecraft:apple", []),
];

pub fn feature(key: &str) -> Option<&'static Feature> {
    FEATURES.iter().find(|f| f.key == key)
}
/// The feature's name and description: a module's from [`super::MODULES`], an older extra's
/// from its Extras row.
pub fn texts(key: &str) -> (String, String) {
    use super::Item;
    if let Some(i) = module_index(key) {
        let item = Item::Module(i as u8);
        return (item.caption_text(), item.description_text());
    }
    let item = match key {
        "fullbright" => Item::Fullbright,
        "dynamicLights" => Item::DynamicLights,
        "hideBlockParticles" => Item::HideBlockParticles,
        "hitboxes" => Item::Hitboxes,
        "shulker_preview" => Item::ShulkerPreview,
        "mouseTweaks" => Item::MouseTweaks,
        "sort" => Item::Sort,
        "ownNameTag" => Item::OwnNameTag,
        "chatAnimation" => Item::ChatAnimation,
        "autoReconnect" => Item::AutoReconnect,
        "fast_items" => Item::FastItems,
        "smart_f3" => Item::SmartF3,
        "apple_skin" => Item::AppleSkin,
        "armor_status" => Item::ArmorStatus,
        "threeDSkinLayers" => {
            return (
                crate::lang::get_or("wisp.menu.feature.skin_layers", "3D Skin Layers"),
                crate::lang::get_or(
                    "wisp.menu.feature.skin_layers.tooltip",
                    "Draws the outer layer of player skins in 3D.",
                ),
            );
        }
        _ => return (key.to_owned(), String::new()),
    };
    (item.caption_text(), item.description_text())
}

impl Extras {
    /// Whether the feature `key` is on.
    pub fn feature_on(&self, key: &str) -> bool {
        if let Some(i) = module_index(key) {
            return self.modules[i];
        }
        match key {
            "fullbright" => self.fullbright,
            "dynamicLights" => self.dynamic_lights,
            "threeDSkinLayers" => self.three_d_skin_layers,
            "hideBlockParticles" => self.hide_block_particles,
            "hitboxes" => self.hitboxes,
            "shulker_preview" => self.shulker_preview,
            "mouseTweaks" => self.mouse_tweaks,
            "sort" => self.sort,
            "ownNameTag" => self.own_name_tag,
            "chatAnimation" => self.chat_animation,
            "autoReconnect" => self.auto_reconnect,
            "fast_items" => self.fast_items,
            "smart_f3" => self.smart_f3,
            "apple_skin" => self.apple_skin,
            "armor_status" => self.armor_status,
            _ => false,
        }
    }
    /// Turns the feature `key` on or off.
    pub fn set_feature(&mut self, key: &str, on: bool) {
        if let Some(i) = module_index(key) {
            self.modules[i] = on;
            return;
        }
        let field = match key {
            "fullbright" => &mut self.fullbright,
            "dynamicLights" => &mut self.dynamic_lights,
            "threeDSkinLayers" => &mut self.three_d_skin_layers,
            "hideBlockParticles" => &mut self.hide_block_particles,
            "hitboxes" => &mut self.hitboxes,
            "shulker_preview" => &mut self.shulker_preview,
            "mouseTweaks" => &mut self.mouse_tweaks,
            "sort" => &mut self.sort,
            "ownNameTag" => &mut self.own_name_tag,
            "chatAnimation" => &mut self.chat_animation,
            "autoReconnect" => &mut self.auto_reconnect,
            "fast_items" => &mut self.fast_items,
            "smart_f3" => &mut self.smart_f3,
            "apple_skin" => &mut self.apple_skin,
            "armor_status" => &mut self.armor_status,
            _ => return,
        };
        *field = on;
    }
    fn setting_of(feature: &str, setting: &str) -> Option<Setting> {
        let f = self::feature(feature)?;
        f.settings
            .iter()
            .chain(if f.widget { WIDGET_SETTINGS } else { &[] })
            .find(|s| s.key == setting)
            .copied()
    }
    fn raw(&self, feature: &str, setting: &str) -> Option<&str> {
        self.options
            .get(&format!("{feature}.{setting}"))
            .map(String::as_str)
    }
    /// A toggle setting's value (its default when unset or unknown).
    pub fn opt_bool(&self, feature: &str, setting: &str) -> bool {
        let default = match Self::setting_of(feature, setting).map(|s| s.kind) {
            Some(Kind::Toggle(d)) => d,
            _ => false,
        };
        match self.raw(feature, setting) {
            Some("true") => true,
            Some("false") => false,
            _ => default,
        }
    }
    /// A slider setting's value, kept in its range and on its steps.
    pub fn opt_f32(&self, feature: &str, setting: &str) -> f32 {
        let Some(Kind::Slider {
            min,
            max,
            step,
            default,
            ..
        }) = Self::setting_of(feature, setting).map(|s| s.kind)
        else {
            return 0.0;
        };
        self.raw(feature, setting)
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite())
            .map_or(default, |v| snap(v, min, max, step))
    }
    /// A colour setting's value (`0xRRGGBB`).
    pub fn opt_color(&self, feature: &str, setting: &str) -> u32 {
        let default = match Self::setting_of(feature, setting).map(|s| s.kind) {
            Some(Kind::Color(c)) => c,
            _ => 0xFFFFFF,
        };
        self.raw(feature, setting)
            .and_then(|v| u32::from_str_radix(v, 16).ok())
            .filter(|c| *c <= 0xFF_FFFF)
            .unwrap_or(default)
    }
    /// A choice setting's index.
    pub fn opt_choice(&self, feature: &str, setting: &str) -> usize {
        let Some(Kind::Choice(names, default)) = Self::setting_of(feature, setting).map(|s| s.kind)
        else {
            return 0;
        };
        self.raw(feature, setting)
            .and_then(|v| names.iter().position(|n| n.eq_ignore_ascii_case(v)))
            .unwrap_or(default)
    }
    /// A key setting's vanilla key name.
    pub fn opt_key(&self, feature: &str, setting: &str) -> String {
        let default = match Self::setting_of(feature, setting).map(|s| s.kind) {
            Some(Kind::Key(k)) => k,
            _ => "key.keyboard.unknown",
        };
        self.raw(feature, setting)
            .filter(|v| v.starts_with("key."))
            .unwrap_or(default)
            .to_owned()
    }
    /// A HUD widget's place: its top left as fractions of the screen (`None`: not placed yet).
    pub fn widget_pos(&self, feature: &str) -> Option<(f32, f32)> {
        self.raw(feature, "pos").and_then(super::parse_pos)
    }
    pub fn set_widget_pos(&mut self, feature: &str, pos: Option<(f32, f32)>) {
        let k = format!("{feature}.pos");
        match pos {
            Some(p) => {
                self.options.insert(k, format!("{:.4},{:.4}", p.0, p.1));
            }
            None => {
                self.options.remove(&k);
                self.options.remove(&format!("{feature}.at"));
            }
        }
    }
    /// A HUD widget's anchor as fractions of the screen (`widget_anchor_place`), which wins over
    /// the older top-left `pos`.
    pub fn widget_anchor(&self, feature: &str) -> Option<(f32, f32)> {
        self.raw(feature, "at").and_then(super::parse_pos)
    }
    /// Places a widget by its anchor and forgets its older top-left place.
    pub fn set_widget_anchor(&mut self, feature: &str, at: (f32, f32)) {
        self.options.remove(&format!("{feature}.pos"));
        self.options
            .insert(format!("{feature}.at"), format!("{:.4},{:.4}", at.0, at.1));
    }
    /// Sets a setting from its text (`true`, a number, `rrggbb`, a choice's name, a key name).
    pub fn set_opt(&mut self, feature: &str, setting: &str, value: String) {
        self.options.insert(format!("{feature}.{setting}"), value);
    }
    /// Every setting of a feature back to its default (its place in the HUD included).
    pub fn reset_feature(&mut self, feature: &str) {
        let prefix = format!("{feature}.");
        self.options.retain(|k, _| !k.starts_with(&prefix));
    }
}

/// `v` kept within `min..=max` and on the steps from `min`.
pub fn snap(v: f32, min: f32, max: f32, step: f32) -> f32 {
    let v = v.clamp(min, max);
    if step <= 0.0 {
        return v;
    }
    let n = ((v - min) / step).round();
    (min + n * step).clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_feature_has_a_switch_text_and_valid_defaults() {
        let mut e = Extras::default();
        let mut keys = std::collections::HashSet::new();
        for f in FEATURES {
            assert!(keys.insert(f.key), "{} listed twice", f.key);
            assert!(!texts(f.key).0.is_empty(), "{}", f.key);
            // The switch round-trips through the file.
            let before = e.feature_on(f.key);
            e.set_feature(f.key, !before);
            assert_eq!(
                Extras::parse(&e.serialize()).feature_on(f.key),
                !before,
                "{}",
                f.key
            );
            for s in f.settings {
                if let Kind::Choice(names, d) = s.kind {
                    assert!(d < names.len(), "{}.{}", f.key, s.key);
                }
            }
        }
    }

    #[test]
    fn settings_survive_the_file_and_fall_back_to_their_defaults() {
        let mut e = Extras::default();
        assert!(e.opt_bool("hud_keystrokes", "mouse"));
        assert_eq!(e.opt_f32("hud_fps", "scale"), 1.0);
        assert_eq!(e.opt_choice("armor_status", "durability"), 0);
        assert_eq!(e.opt_key("freelook", "key"), "key.keyboard.left.alt");
        e.set_opt("hud_fps", "scale", "1.47".into());
        e.set_opt("armor_status", "durability", "Percent".into());
        e.set_opt("target_crosshair", "color", "4ade80".into());
        e.set_opt("freelook", "key", "key.keyboard.v".into());
        e.set_widget_pos("hud_fps", Some((0.25, 0.5)));
        let back = Extras::parse(&e.serialize());
        assert_eq!(back, e);
        assert!((back.opt_f32("hud_fps", "scale") - 1.45).abs() < 1e-4);
        assert_eq!(back.opt_choice("armor_status", "durability"), 2);
        assert_eq!(back.opt_color("target_crosshair", "color"), 0x4ADE80);
        assert_eq!(back.opt_key("freelook", "key"), "key.keyboard.v");
        assert_eq!(back.widget_pos("hud_fps"), Some((0.25, 0.5)));
        // Broken values fall back.
        let odd = Extras::parse("opt.hud_fps.scale:big\nopt.target_crosshair.color:zz\n");
        assert_eq!(odd.opt_f32("hud_fps", "scale"), 1.0);
        assert_eq!(odd.opt_color("target_crosshair", "color"), 0xFF6B6B);
        let mut r = back.clone();
        r.reset_feature("hud_fps");
        assert_eq!(r.widget_pos("hud_fps"), None);
        assert_eq!(r.opt_f32("hud_fps", "scale"), 1.0);
    }
}
