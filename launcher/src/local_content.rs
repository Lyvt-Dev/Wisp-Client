//! What an instance's game folder holds, for the launcher to show: resource packs (with their
//! `pack.mcmeta` description and `pack.png`), worlds under `saves/` and the multiplayer list in
//! `servers.dat`. Reading never writes; malformed files are skipped.
use base64::Engine;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::UNIX_EPOCH;

/// Pictures larger than this are not read (a pack icon is a few kilobytes).
const MAX_PICTURE: u64 = 4 << 20;
/// Lists stop here; the pages show this many at most.
const MAX_ENTRIES: usize = 200;

#[derive(Clone, Debug, PartialEq)]
pub struct Pack {
    /// The file or folder name in `resourcepacks/`.
    pub file: String,
    pub description: String,
    pub icon: Option<Vec<u8>>,
    pub folder: bool,
}

/// The resource packs in `dir`, and how many other files sit beside them.
pub fn packs(dir: &Path) -> (Vec<Pack>, usize) {
    let mut packs = Vec::new();
    let mut other = 0;
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        if packs.len() >= MAX_ENTRIES {
            break;
        }
        let path = entry.path();
        let file = entry.file_name().to_string_lossy().into_owned();
        let pack = if path.is_dir() {
            fs::read(path.join("pack.mcmeta")).ok().map(|meta| Pack {
                description: description(&meta),
                icon: picture(&path.join("pack.png")),
                folder: true,
                file: file.clone(),
            })
        } else if file.to_ascii_lowercase().ends_with(".zip") {
            zip_pack(&path, &file)
        } else {
            None
        };
        match pack {
            Some(pack) => packs.push(pack),
            None => other += 1,
        }
    }
    packs.sort_by_key(|p| p.file.to_lowercase());
    (packs, other)
}

fn zip_pack(path: &Path, file: &str) -> Option<Pack> {
    let mut archive = zip::ZipArchive::new(fs::File::open(path).ok()?).ok()?;
    let mut read = |name: &str| -> Option<Vec<u8>> {
        let entry = archive.by_name(name).ok()?;
        if entry.size() > MAX_PICTURE {
            return None;
        }
        let mut bytes = Vec::new();
        entry.take(MAX_PICTURE).read_to_end(&mut bytes).ok()?;
        Some(bytes)
    };
    let meta = read("pack.mcmeta")?;
    Some(Pack {
        file: file.to_owned(),
        description: description(&meta),
        icon: read("pack.png"),
        folder: false,
    })
}

/// A text component (a string, a list or an object with `text`/`extra`) as plain text, with
/// `§` formatting codes removed and whitespace collapsed.
pub fn plain_text(value: &serde_json::Value) -> String {
    fn flatten(value: &serde_json::Value, out: &mut String) {
        match value {
            serde_json::Value::String(s) => out.push_str(s),
            serde_json::Value::Array(items) => items.iter().for_each(|v| flatten(v, out)),
            serde_json::Value::Object(map) => {
                if let Some(text) = map.get("text").or_else(|| map.get("translate")) {
                    flatten(text, out);
                }
                if let Some(extra) = map.get("extra") {
                    flatten(extra, out);
                }
            }
            _ => {}
        }
    }
    let mut out = String::new();
    flatten(value, &mut out);
    let mut clean = String::new();
    let mut chars = out.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            clean.push(c);
        }
    }
    clean.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `pack.description` of a `pack.mcmeta`.
fn description(meta: &[u8]) -> String {
    let text = String::from_utf8_lossy(meta);
    let text = text.trim_start_matches('\u{feff}');
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|json| json.get("pack")?.get("description").map(plain_text))
        .unwrap_or_default()
}

/// The `resourcePacks` list of the game's `options.txt` (vanilla's format), if it can be read.
fn pack_list(text: &str) -> Option<Vec<String>> {
    let line = text
        .lines()
        .rev()
        .find_map(|l| l.strip_prefix("resourcePacks:"))?;
    serde_json::from_str(line.trim()).ok()
}

/// The pack files that are on in the game, lowest priority first (`file/<name>` entries).
pub fn enabled_packs(game: &Path) -> Vec<String> {
    let text = fs::read_to_string(game.join("options.txt")).unwrap_or_default();
    pack_list(&text)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| p.strip_prefix("file/").map(str::to_owned))
        .collect()
}

/// Turns a pack file on (on top of the others) or off in `options.txt`, keeping every other line.
/// A `resourcePacks` line that can't be read is left alone.
pub fn set_pack_enabled(game: &Path, file: &str, on: bool) -> Result<(), String> {
    let path = game.join("options.txt");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.to_string()),
    };
    let has_line = text.lines().any(|l| l.starts_with("resourcePacks:"));
    let mut list = match pack_list(&text) {
        Some(list) => list,
        None if !has_line => vec!["vanilla".to_owned()],
        None => return Err("options.txt has a resourcePacks line Wisp can't read.".into()),
    };
    let entry = format!("file/{file}");
    list.retain(|p| *p != entry);
    if on {
        list.push(entry);
    }
    let line = format!(
        "resourcePacks:{}",
        serde_json::to_string(&list).map_err(|e| e.to_string())?
    );
    let mut out: Vec<String> = text
        .lines()
        .map(|l| {
            if l.starts_with("resourcePacks:") {
                line.clone()
            } else {
                l.to_owned()
            }
        })
        .collect();
    if !has_line {
        out.push(line);
    }
    let temp = path.with_extension("txt.wisp-tmp");
    fs::write(&temp, out.join("\n") + "\n").map_err(|e| e.to_string())?;
    fs::rename(&temp, &path).map_err(|e| e.to_string())
}

fn picture(path: &Path) -> Option<Vec<u8>> {
    let meta = fs::metadata(path).ok()?;
    (meta.len() <= MAX_PICTURE)
        .then(|| fs::read(path).ok())
        .flatten()
}

#[derive(Clone, Debug, PartialEq)]
pub struct World {
    /// The folder name under `saves/`, which `--quickPlaySingleplayer` takes.
    pub folder: String,
    /// When `level.dat` was last written, in seconds since 1970.
    pub played: u64,
    pub icon: Option<Vec<u8>>,
}

/// The worlds under `saves`, most recently played first.
pub fn worlds(saves: &Path) -> Vec<World> {
    let mut worlds: Vec<World> = fs::read_dir(saves)
        .into_iter()
        .flatten()
        .flatten()
        .take(MAX_ENTRIES)
        .filter_map(|entry| {
            let path = entry.path();
            let level = fs::metadata(path.join("level.dat")).ok()?;
            let played = level
                .modified()
                .ok()?
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs();
            Some(World {
                folder: entry.file_name().to_string_lossy().into_owned(),
                played,
                icon: picture(&path.join("icon.png")),
            })
        })
        .collect();
    worlds.sort_by_key(|w| std::cmp::Reverse(w.played));
    worlds
}

#[derive(Clone, Debug, PartialEq)]
pub struct Server {
    pub name: String,
    pub address: String,
    pub icon: Option<Vec<u8>>,
}

/// The multiplayer list in `servers.dat` (uncompressed NBT), in its own order.
pub fn servers(file: &Path) -> Vec<Server> {
    let Ok(bytes) = fs::read(file) else {
        return Vec::new();
    };
    let mut reader = Nbt {
        bytes: &bytes,
        at: 0,
    };
    let Some(Tag::Compound(root)) = reader.root() else {
        return Vec::new();
    };
    let Some((_, Tag::List(list))) = root.into_iter().find(|(name, _)| name == "servers") else {
        return Vec::new();
    };
    list.into_iter()
        .take(MAX_ENTRIES)
        .filter_map(|server| {
            let Tag::Compound(fields) = server else {
                return None;
            };
            let text = |key: &str| {
                fields.iter().find_map(|(name, tag)| match tag {
                    Tag::String(s) if name == key => Some(s.clone()),
                    _ => None,
                })
            };
            let address = text("ip")?;
            Some(Server {
                name: text("name").unwrap_or_else(|| address.clone()),
                icon: text("icon").and_then(|b64| {
                    base64::engine::general_purpose::STANDARD
                        .decode(b64.trim())
                        .ok()
                }),
                address,
            })
        })
        .collect()
}

enum Tag {
    String(String),
    List(Vec<Tag>),
    Compound(Vec<(String, Tag)>),
    Other,
}

/// Just enough of NBT to read `servers.dat`.
struct Nbt<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Nbt<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let end = self.at.checked_add(n)?;
        let slice = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(slice)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_be_bytes([b[0], b[1]]))
    }
    fn len(&mut self) -> Option<usize> {
        let b = self.take(4)?;
        usize::try_from(i32::from_be_bytes([b[0], b[1], b[2], b[3]])).ok()
    }
    fn string(&mut self) -> Option<String> {
        let n = self.u16()? as usize;
        Some(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    fn root(&mut self) -> Option<Tag> {
        let kind = self.u8()?;
        self.string()?;
        self.payload(kind, 0)
    }
    fn payload(&mut self, kind: u8, depth: usize) -> Option<Tag> {
        if depth > 32 {
            return None;
        }
        Some(match kind {
            1 => self.take(1).map(|_| Tag::Other)?,
            2 => self.take(2).map(|_| Tag::Other)?,
            3 | 5 => self.take(4).map(|_| Tag::Other)?,
            4 | 6 => self.take(8).map(|_| Tag::Other)?,
            7 => {
                let n = self.len()?;
                self.take(n).map(|_| Tag::Other)?
            }
            8 => Tag::String(self.string()?),
            9 => {
                let inner = self.u8()?;
                let n = self.len()?;
                if n > self.bytes.len() {
                    return None;
                }
                let mut items = Vec::new();
                for _ in 0..n {
                    items.push(self.payload(inner, depth + 1)?);
                }
                Tag::List(items)
            }
            10 => {
                let mut fields = Vec::new();
                loop {
                    let kind = self.u8()?;
                    if kind == 0 {
                        break;
                    }
                    let name = self.string()?;
                    fields.push((name, self.payload(kind, depth + 1)?));
                }
                Tag::Compound(fields)
            }
            11 => {
                let n = self.len()?;
                self.take(n.checked_mul(4)?).map(|_| Tag::Other)?
            }
            12 => {
                let n = self.len()?;
                self.take(n.checked_mul(8)?).map(|_| Tag::Other)?
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string(out: &mut Vec<u8>, s: &str) {
        out.extend((s.len() as u16).to_be_bytes());
        out.extend(s.as_bytes());
    }

    fn servers_dat(entries: &[(&str, &str, Option<&str>)]) -> Vec<u8> {
        let mut out = vec![10];
        string(&mut out, "");
        out.push(9);
        string(&mut out, "servers");
        out.push(10);
        out.extend((entries.len() as i32).to_be_bytes());
        for (name, ip, icon) in entries {
            out.push(1);
            string(&mut out, "hidden");
            out.push(0);
            out.push(8);
            string(&mut out, "name");
            string(&mut out, name);
            out.push(8);
            string(&mut out, "ip");
            string(&mut out, ip);
            if let Some(icon) = icon {
                out.push(8);
                string(&mut out, "icon");
                string(&mut out, icon);
            }
            out.push(0);
        }
        out.push(0);
        out
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("wisp-local-content-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn servers_dat_lists_names_addresses_and_icons() {
        let dir = scratch("servers");
        let file = dir.join("servers.dat");
        fs::write(
            &file,
            servers_dat(&[
                ("Hub", "play.example.net", Some("aGk=")),
                ("", "1.2.3.4:25570", None),
            ]),
        )
        .unwrap();
        let list = servers(&file);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Hub");
        assert_eq!(list[0].address, "play.example.net");
        assert_eq!(list[0].icon.as_deref(), Some(&b"hi"[..]));
        assert_eq!(list[1].address, "1.2.3.4:25570");
        assert!(list[1].icon.is_none());
    }

    #[test]
    fn truncated_servers_dat_is_empty_not_a_panic() {
        let dir = scratch("truncated");
        let file = dir.join("servers.dat");
        let bytes = servers_dat(&[("Hub", "play.example.net", None)]);
        for cut in 0..bytes.len() {
            fs::write(&file, &bytes[..cut]).unwrap();
            let _ = servers(&file);
        }
        assert!(servers(&dir.join("missing.dat")).is_empty());
    }

    #[test]
    fn packs_are_folders_and_zips_with_pack_mcmeta() {
        let dir = scratch("packs");
        let folder = dir.join("Folder Pack");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("pack.mcmeta"),
            r#"{"pack":{"pack_format":64,"description":[{"text":"§6Gold "},"and plain"]}}"#,
        )
        .unwrap();
        fs::write(folder.join("pack.png"), b"png").unwrap();
        let zip_path = dir.join("Zip Pack.zip");
        {
            let mut zip = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
            zip.start_file("pack.mcmeta", zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut zip, br#"{"pack":{"description":"Zipped"}}"#).unwrap();
            zip.finish().unwrap();
        }
        fs::write(dir.join("setup.exe"), b"no").unwrap();
        fs::create_dir_all(dir.join("not a pack")).unwrap();
        let (list, other) = packs(&dir);
        assert_eq!(other, 2);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].file, "Folder Pack");
        assert_eq!(list[0].description, "Gold and plain");
        assert_eq!(list[0].icon.as_deref(), Some(&b"png"[..]));
        assert!(list[0].folder);
        assert_eq!(list[1].description, "Zipped");
        assert!(!list[1].folder);
    }

    #[test]
    fn packs_turn_on_and_off_in_options_txt_keeping_other_lines() {
        let dir = scratch("options");
        fs::write(
            dir.join("options.txt"),
            "fov:0.5\nresourcePacks:[\"vanilla\",\"file/A.zip\"]\nlang:en_us\n",
        )
        .unwrap();
        assert_eq!(enabled_packs(&dir), ["A.zip"]);
        set_pack_enabled(&dir, "B Pack", true).unwrap();
        assert_eq!(enabled_packs(&dir), ["A.zip", "B Pack"]);
        set_pack_enabled(&dir, "A.zip", false).unwrap();
        assert_eq!(enabled_packs(&dir), ["B Pack"]);
        let text = fs::read_to_string(dir.join("options.txt")).unwrap();
        assert!(text.starts_with("fov:0.5\n"));
        assert!(text.contains("lang:en_us"));
        assert!(text.contains(r#"resourcePacks:["vanilla","file/B Pack"]"#));

        let fresh = scratch("options-fresh");
        set_pack_enabled(&fresh, "C.zip", true).unwrap();
        assert_eq!(enabled_packs(&fresh), ["C.zip"]);

        let broken = scratch("options-broken");
        fs::write(broken.join("options.txt"), "resourcePacks:[oops\n").unwrap();
        assert!(set_pack_enabled(&broken, "C.zip", true).is_err());
        assert_eq!(
            fs::read_to_string(broken.join("options.txt")).unwrap(),
            "resourcePacks:[oops\n"
        );
    }

    #[test]
    fn worlds_need_level_dat() {
        let dir = scratch("worlds");
        fs::create_dir_all(dir.join("Real")).unwrap();
        fs::write(dir.join("Real/level.dat"), b"x").unwrap();
        fs::create_dir_all(dir.join("Empty")).unwrap();
        let list = worlds(&dir);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].folder, "Real");
    }
}
