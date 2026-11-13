//! Built-in client features share the game's saved Extras preferences.
//! Changes are locked while playing; unknown preferences are preserved.
use crate::Profile;
use std::{collections::BTreeMap, io::ErrorKind};

pub const FEATURES: &[(&str, &str)] = &[
    ("fullbright", "Full Bright"),
    ("fps", "FPS Counter"),
    ("apple_skin", "Apple Skin"),
    ("mouseTweaks", "Mouse Tweaks"),
    ("smart_f3", "SmartF3"),
    ("shulker_preview", "Shulker Preview / Peek"),
    ("armor_status", "Armor Status"),
    ("mod_menu", "Mod Menu (Right Shift)"),
    ("fast_items", "Fast Items"),
    ("hitboxes", "Hitbox"),
    ("wisp_hud", "Wisp HUD"),
    ("hud_cps", "CPS"),
    ("hud_ping", "Ping"),
    ("hud_direction", "Direction"),
    ("hud_biome", "Biome"),
    ("hud_server", "Server"),
    ("hud_speed", "Speed"),
    ("hud_clock", "Clock"),
    ("hud_day_time", "Day Time"),
    ("hud_weather", "Weather"),
    ("hud_effects", "Potion Effects"),
    ("hud_durability", "Item Durability"),
    ("hud_arrows", "Arrow Count"),
    ("always_day", "Always Day"),
    ("item_physics", "Item Physics"),
    ("arrow_trails", "Arrow Trails"),
    ("target_crosshair", "Target Crosshair"),
    ("freelook", "Freelook"),
    ("health_indicators", "Health Indicators"),
    ("damage_numbers", "Damage Numbers"),
    ("tnt_timer", "TNT Timer"),
    ("hit_info", "Hit Info"),
    ("item_highlighter", "Item Highlighter"),
    ("chat_highlights", "Chat Highlights"),
    ("chat_peek", "Chat Peek"),
    ("waypoints", "Waypoints"),
    ("inventory_search", "Inventory Search"),
];

fn source(profile: &Profile) -> Result<String, String> {
    match wisp_compat::read_extras(&profile.game_home) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error.to_string()),
    }
}

fn parse(text: &str) -> Result<BTreeMap<String, bool>, String> {
    let mut result = BTreeMap::new();
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            if FEATURES.iter().any(|(known, _)| *known == key) {
                let value = value.trim().parse::<bool>().map_err(|_| {
                    crate::i18n::tr("Invalid client preference; the settings file was preserved.")
                })?;
                result.insert(key.to_owned(), value);
            }
        }
    }
    Ok(result)
}

pub fn read(profile: &Profile) -> Result<BTreeMap<String, bool>, String> {
    parse(&source(profile)?)
}

pub fn toggle(profile: &Profile, key: &str) -> Result<(), String> {
    if !FEATURES.iter().any(|(known, _)| *known == key) {
        return Err(crate::i18n::tr("Unknown client feature."));
    }
    let text = source(profile)?;
    let values = parse(&text)?;
    let enabled = !values.get(key).copied().unwrap_or(false);
    let mut output = String::new();
    for line in text.lines() {
        if !line
            .split_once(':')
            .is_some_and(|(name, _)| name.trim() == key)
        {
            output.push_str(line);
            output.push('\n');
        }
    }
    output.push_str(&format!("{key}:{enabled}\n"));
    wisp_mods::amod::atomic_write(
        &profile.game_home.join("wisp-extras.txt"),
        output.as_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn feature_changes_preserve_other_preferences_and_corrupt_files() {
        let root = std::env::temp_dir().join(format!(
            "wisp-features-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let profile = Profile {
            name: "Test".into(),
            binary: root.join("game.exe"),
            resources: root.join("resources"),
            game_home: root.clone(),
            args: vec![],
            use_account: false,
        };
        let file = root.join("wisp-extras.txt");
        fs::write(&file, "zoom:true\ncustom:value\nfullbright:false\n").unwrap();
        toggle(&profile, "fps").unwrap();
        toggle(&profile, "fullbright").unwrap();
        let values = read(&profile).unwrap();
        assert_eq!(values.get("fps"), Some(&true));
        assert_eq!(values.get("fullbright"), Some(&true));
        let saved = fs::read_to_string(&file).unwrap();
        assert!(saved.contains("zoom:true\ncustom:value\n"));
        toggle(&profile, "fps").unwrap();
        assert_eq!(read(&profile).unwrap().get("fps"), Some(&false));
        let bad = "fps:invalid\ncustom:value\n";
        fs::write(&file, bad).unwrap();
        assert!(toggle(&profile, "fullbright").is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), bad);
        assert!(toggle(&profile, "unexpected").is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
