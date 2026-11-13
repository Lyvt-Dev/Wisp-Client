//! `wisp-prepare --check`: whether prepared resources are complete and current for this
//! preparer, without Java and without the game files (docs/api/launch.md). A preparation stamps
//! its folder with the `OUTPUT_VERSION` that made it (`STAMP`); tools/prepare_output.py leaves the
//! stamp out of the output's digest, so it never asks for a new `OUTPUT_VERSION`.

use crate::pyjson::Value;
use crate::{artifacts::Lock, read_json, write_json, OUTPUT_VERSION, SCHEMA};
use std::path::Path;

/// The file beside `wisp-resources.json` naming the preparer's `OUTPUT_VERSION`.
pub const STAMP: &str = "wisp-prepare.json";

pub(crate) fn write_stamp(resources: &Path) -> Result<(), String> {
    write_json(
        &resources.join(STAMP),
        &Value::Object(vec![(
            "output_version".into(),
            Value::Int(OUTPUT_VERSION.to_string()),
        )]),
    )
}

pub(crate) fn remove_stamp(resources: &Path) -> Result<(), String> {
    let path = resources.join(STAMP);
    match std::fs::remove_file(&path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("{}: {e}", path.display()))
        }
        _ => Ok(()),
    }
}

/// What the resources at `resources` lack for `root`'s pinned version, each with what to run;
/// empty when they are complete and current. Err when `root` has no readable research-lock.json.
pub fn check(root: &Path, resources: &Path) -> Result<Vec<String>, String> {
    let lock = Lock::read(root)?;
    let manifest_path = resources.join("wisp-resources.json");
    if !manifest_path.is_file() {
        return Ok(vec![format!(
            "{} is not prepared: wisp-prepare --prepare",
            resources.display()
        )]);
    }
    let mut lacking = Vec::new();
    let manifest = read_json(&manifest_path)?;
    let int = |value: Option<&Value>| value.and_then(Value::as_i64);
    if int(manifest.get("schema")) != Some(SCHEMA) {
        lacking.push(format!(
            "an older layout (schema {}, this preparer writes {SCHEMA}): wisp-prepare --prepare",
            int(manifest.get("schema")).map_or("none".into(), |s| s.to_string())
        ));
    }
    let minecraft = manifest.get("minecraft").and_then(Value::as_str);
    if minecraft != Some(lock.version.as_str()) {
        lacking.push(format!(
            "prepared for Minecraft {}, this preparer pins {}: wisp-prepare --prepare",
            minecraft.unwrap_or("unknown"),
            lock.version
        ));
    }
    let stamp = read_json(&resources.join(STAMP)).ok();
    let made_by = int(stamp.as_ref().and_then(|s| s.get("output_version")));
    if made_by != Some(i64::from(OUTPUT_VERSION)) {
        lacking.push(format!(
            "made by output version {}, this preparer makes {OUTPUT_VERSION}: wisp-prepare --prepare",
            made_by.map_or("unknown".into(), |v| v.to_string())
        ));
    }
    for (command, what) in missing_extras(resources) {
        lacking.push(format!("{what} missing: wisp-prepare {command}"));
    }
    Ok(lacking)
}

/// What the resources at `resources` lack of what a preparation may leave out when the player's
/// asset store lacks it: each as its `wisp-prepare` command (`packs`, `panorama`, `fonts`,
/// `languages`, `sounds`) and what it is. `--check` and the launcher's `COMPLETIONS`
/// (launcher/src/backend.rs) both ask here, so they never disagree.
pub fn missing_extras(resources: &Path) -> Vec<(&'static str, &'static str)> {
    EXTRAS
        .iter()
        .filter(|(_, lacks, _)| lacks(resources))
        .map(|(what, _, command)| (*command, *what))
        .collect()
}

/// What, whether it is missing, the command.
type Extra = (&'static str, fn(&Path) -> bool, &'static str);

const EXTRAS: [Extra; 5] = [
    ("the built-in packs", lacks_packs, "packs"),
    ("the title panorama", lacks_panorama, "panorama"),
    ("the Unifont glyphs", lacks_unifont, "fonts"),
    ("the languages", lacks_languages, "languages"),
    ("the sounds", lacks_sounds, "sounds"),
];

fn lacks_packs(resources: &Path) -> bool {
    crate::BUILT_IN_PACKS
        .iter()
        .any(|pack| !resources.join("assets").join(pack).is_file())
}

/// The panorama needs six equally sized, non-placeholder square faces and an overlay: prepared
/// resources may hold only some of the full-size assets after an interrupted import, and the
/// client jar's faces are 1x1 placeholders (a PNG's width and height are the big-endian words at
/// bytes 16 and 20, in the IHDR chunk).
fn lacks_panorama(resources: &Path) -> bool {
    let background = resources.join("assets/minecraft/textures/gui/title/background");
    let mut expected = None;
    for face in 0..6 {
        let Ok(png) = std::fs::read(background.join(format!("panorama_{face}.png"))) else {
            return true;
        };
        if png.get(..16) != Some(b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR") {
            return true;
        }
        let Some(dimensions) = png.get(16..24) else {
            return true;
        };
        let width =
            u32::from_be_bytes([dimensions[0], dimensions[1], dimensions[2], dimensions[3]]);
        let height =
            u32::from_be_bytes([dimensions[4], dimensions[5], dimensions[6], dimensions[7]]);
        if width <= 1 || width != height || expected.is_some_and(|size| size != width) {
            return true;
        }
        expected = Some(width);
    }
    !background.join("panorama_overlay.png").is_file()
}

fn lacks_unifont(resources: &Path) -> bool {
    !resources
        .join("assets/minecraft/font/unifont.zip")
        .is_file()
}

/// The language list only the asset index holds (the client jar's `lang` has English alone).
fn lacks_languages(resources: &Path) -> bool {
    !resources.join("assets/pack.mcmeta").is_file()
}

/// What the game's sound registry reads (wisp-client sound_events.rs): the `sounds.json`, and
/// every sound file it names. An interrupted copy leaves files out, and the game then plays
/// nothing for their events, so a missing file is a gap.
fn lacks_sounds(resources: &Path) -> bool {
    !resources.join("assets/minecraft/sounds.json").is_file() || missing_sound_files(resources) > 0
}

/// How many sound files `sounds.json` names (not events) are not under `assets/minecraft/sounds/`.
fn missing_sound_files(resources: &Path) -> usize {
    let assets = resources.join("assets/minecraft");
    let Ok(sounds) = read_json(&assets.join("sounds.json")) else {
        return 0;
    };
    let mut missing = 0;
    for (_, event) in sounds.as_object().unwrap_or(&[]) {
        let Some(list) = event.get("sounds").and_then(Value::as_array) else {
            continue;
        };
        for sound in list {
            let name = match sound {
                Value::Str(name) => name.as_str(),
                Value::Object(_) => {
                    if sound.get("type").and_then(Value::as_str) == Some("event") {
                        continue;
                    }
                    match sound.get("name").and_then(Value::as_str) {
                        Some(name) => name,
                        None => continue,
                    }
                }
                _ => continue,
            };
            // Only vanilla's own files: another namespace's sounds are its pack's to have.
            let path = match name.split_once(':') {
                Some(("minecraft", path)) => path,
                Some(_) => continue,
                None => name,
            };
            if !assets.join("sounds").join(format!("{path}.ogg")).is_file() {
                missing += 1;
            }
        }
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn folder(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("wisp-check-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("research-lock.json"),
            r#"{"minecraft": {"version": "9.9", "java_major": 25}}"#,
        )
        .unwrap();
        dir
    }

    fn file(path: &Path, data: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }

    /// A PNG's signature and IHDR size, all `lacks_panorama` reads.
    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend(width.to_be_bytes());
        png.extend(height.to_be_bytes());
        png
    }

    #[test]
    fn panorama_check_finds_missing_and_inconsistent_faces() {
        let dir = folder("panorama");
        let background = dir.join("assets/minecraft/textures/gui/title/background");
        for face in 0..6 {
            file(
                &background.join(format!("panorama_{face}.png")),
                &png_header(2, 2),
            );
        }
        assert!(lacks_panorama(&dir));
        file(&background.join("panorama_overlay.png"), b"overlay");
        assert!(!lacks_panorama(&dir));
        file(&background.join("panorama_4.png"), &png_header(1, 1));
        assert!(lacks_panorama(&dir));
        file(&background.join("panorama_4.png"), &png_header(3, 3));
        assert!(lacks_panorama(&dir));
        fs::remove_file(background.join("panorama_4.png")).unwrap();
        assert!(lacks_panorama(&dir));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn unprepared_resources_need_a_preparation() {
        let root = folder("unprepared");
        let lacking = check(&root, &root.join("out")).unwrap();
        assert_eq!(lacking.len(), 1);
        assert!(
            lacking[0].ends_with("wisp-prepare --prepare"),
            "{lacking:?}"
        );
    }

    #[test]
    fn complete_resources_pass_and_each_gap_names_its_command() {
        let root = folder("complete");
        let out = root.join("out");
        file(
            &out.join("wisp-resources.json"),
            format!(r#"{{"schema": {SCHEMA}, "minecraft": "9.9"}}"#).as_bytes(),
        );
        write_stamp(&out).unwrap();
        let assets = out.join("assets");
        for pack in crate::BUILT_IN_PACKS {
            file(&assets.join(pack), b"zip");
        }
        let background = assets.join("minecraft/textures/gui/title/background");
        for face in 0..6 {
            file(
                &background.join(format!("panorama_{face}.png")),
                &png_header(1024, 1024),
            );
        }
        file(&background.join("panorama_overlay.png"), b"overlay");
        file(&assets.join("minecraft/font/unifont.zip"), b"zip");
        file(&assets.join("pack.mcmeta"), b"{}");
        file(&assets.join("minecraft/sounds.json"), b"{}");
        assert_eq!(check(&root, &out).unwrap(), Vec::<String>::new());

        fs::remove_file(assets.join("minecraft/font/unifont.zip")).unwrap();
        fs::remove_file(out.join(STAMP)).unwrap();
        let lacking = check(&root, &out).unwrap();
        assert_eq!(lacking.len(), 2, "{lacking:?}");
        assert!(lacking[0].starts_with("made by output version unknown"));
        assert!(lacking[1].ends_with("wisp-prepare fonts"));
    }

    #[test]
    fn sounds_check_names_a_gap_until_every_named_file_is_there() {
        let dir = folder("sounds");
        let assets = dir.join("assets/minecraft");
        file(
            &assets.join("sounds.json"),
            br#"{"block.stone.break": {"sounds": ["dig/stone1", {"name": "step/stone1"}, {"name": "x", "type": "event"}]}}"#,
        );
        assert!(lacks_sounds(&dir));
        file(&assets.join("sounds/dig/stone1.ogg"), b"ogg");
        assert!(lacks_sounds(&dir));
        file(&assets.join("sounds/step/stone1.ogg"), b"ogg");
        assert!(!lacks_sounds(&dir));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_preparation_removes_the_stamp_before_writing() {
        let root = folder("restamp");
        let out = root.join("out");
        write_stamp(&out).unwrap();
        remove_stamp(&out).unwrap();
        assert!(!out.join(STAMP).exists());
        // Nothing to remove is not an error.
        remove_stamp(&out).unwrap();
    }

    #[test]
    fn a_second_preparation_of_a_folder_waits_for_the_first() {
        let root = folder("lock");
        let first = crate::hold_lock(&root, &|_| {}).unwrap();
        let waited = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let second = {
            let (root, waited) = (root.clone(), waited.clone());
            std::thread::spawn(move || {
                let said = std::cell::Cell::new(false);
                let _lock = crate::hold_lock(&root, &|line| {
                    said.set(line.starts_with("Waiting for another wisp-prepare"))
                })
                .unwrap();
                waited.store(said.get(), std::sync::atomic::Ordering::SeqCst);
            })
        };
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(!second.is_finished(), "the second lock did not wait");
        drop(first);
        second.join().unwrap();
        assert!(waited.load(std::sync::atomic::Ordering::SeqCst));
    }
}
