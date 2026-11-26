//! Native static Nuit schema-1 skies, from the published resource-pack format.
//! Face layout reference: Nuit, copyright (c) 2020 AMereBagatelle (MIT); see THIRD_PARTY_NOTICES.md.
//! No Java mod is loaded. Unsupported animated/conditional layers are reported.
use crate::{hud::Rect, mesher::Vertex};
use serde_json::Value;
use std::{collections::HashMap, path::Path};

pub const OVERLAY: u32 = 63;
type Image = (String, u32, u32, Vec<u8>);

#[derive(Default)]
pub struct Sky {
    layers: Vec<Layer>,
    pub decorations: Option<[bool; 3]>,
}
struct Layer {
    sprites: [String; 6],
}

fn resource_path(id: &str) -> Option<String> {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    if matches!(ns, "." | "..")
        || ns.is_empty()
        || path.is_empty()
        || ns.contains(['/', '\\', ':'])
        || path.contains(['\\', ':'])
        || path.split('/').any(|p| matches!(p, "" | "." | ".."))
    {
        return None;
    }
    Some(format!("assets/{ns}/{path}"))
}

/// Only unrestricted/Overworld layers are supported for now. Never silently
/// render a biome, weather, animated, shader-overridden or other-dimension layer globally.
fn supported(v: &Value) -> bool {
    if v["schemaVersion"] != 1 {
        return false;
    }
    if let Some(conditions) = v["conditions"].as_object() {
        for (key, rule) in conditions {
            if !matches!(key.as_str(), "skyboxes" | "dimensions" | "worlds")
                || rule["excludes"].as_bool().unwrap_or(false)
                || rule["entries"].as_array().is_none_or(|entries| {
                    !entries.is_empty() && !entries.iter().any(|e| e == "minecraft:overworld")
                })
            {
                return false;
            }
        }
    }
    let p = &v["properties"];
    let blend = p["blend"].as_str().unwrap_or("normal");
    matches!(blend, "normal" | "alpha" | "decorations")
        && p["fade"].is_null()
        && p["clock"].is_null()
        && (v["type"]
            .as_str()
            .is_some_and(|t| t.trim_start_matches("nuit:") == "decorations")
            || p["rotation"].is_null()
            || (p["rotation"]["speed"].as_f64() == Some(0.0) && p["rotation"]["mapping"].is_null()))
}

pub fn load(root: &Path) -> (Sky, Vec<Image>) {
    let mut paths = Vec::new();
    let mut pending = vec![root.join("assets/nuit/sky")];
    while let Some(dir) = pending.pop() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_dir() {
                    pending.push(entry.path());
                } else if (kind.is_file() || (kind.is_symlink() && entry.path().is_file()))
                    && entry.path().extension().is_some_and(|e| e == "json")
                {
                    paths.push(entry.path());
                }
            }
        }
    }
    paths.sort();
    let mut definitions: Vec<(i64, std::path::PathBuf, Value)> = paths
        .into_iter()
        .filter_map(|path| {
            let v: Value = match std::fs::read(&path)
                .map_err(|error| error.to_string())
                .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|error| error.to_string()))
            {
                Ok(v) => v,
                Err(error) => {
                    log::warn!("Custom sky {} not loaded: {error}", path.display());
                    return None;
                }
            };
            Some((v["properties"]["layer"].as_i64().unwrap_or(0), path, v))
        })
        .collect();
    definitions.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let (mut sky, mut images) = (Sky::default(), Vec::new());
    for (_, path, v) in definitions {
        if !supported(&v) {
            log::warn!(
                "Custom sky {} uses unsupported conditions or animation",
                path.display()
            );
            continue;
        }
        match v["type"].as_str().unwrap_or("").trim_start_matches("nuit:") {
            "overworld" => {}
            "decorations" => {
                if ["sun", "moon"].iter().any(|key| !v[key].is_null()) {
                    log::warn!(
                        "Custom sky {} uses unsupported celestial textures",
                        path.display()
                    );
                    continue;
                }
                sky.decorations = Some(
                    ["showSun", "showMoon", "showStars"]
                        .map(|key| v[key].as_bool().unwrap_or(false)),
                );
            }
            "monocolor" => {
                let color = ["red", "green", "blue", "alpha"].map(|key| {
                    let value = v["color"][key].as_f64().unwrap_or(0.0);
                    (value.clamp(0.0, 1.0) * 255.0).round() as u8
                });
                let sprites = std::array::from_fn(|face| {
                    format!("wisp/custom-sky/{}/{face}", sky.layers.len())
                });
                images.extend(
                    sprites
                        .iter()
                        .map(|name| (name.clone(), 1, 1, color.to_vec())),
                );
                sky.layers.push(Layer { sprites });
            }
            "square-textured" => {
                let Some(texture) = v["texture"].as_str().and_then(resource_path) else {
                    log::warn!(
                        "Custom sky {} has an invalid texture identifier",
                        path.display()
                    );
                    continue;
                };
                match crate::hud::decode(&root.join(texture))
                    .and_then(|(w, h, pixels)| split_faces(w, h, &pixels, sky.layers.len()))
                {
                    Ok(faces) => {
                        let sprites = std::array::from_fn(|i| faces[i].0.clone());
                        images.extend(faces);
                        sky.layers.push(Layer { sprites });
                    }
                    Err(error) => log::warn!("Custom sky {} not loaded: {error}", path.display()),
                }
            }
            kind => log::warn!("Custom sky {}: unsupported type {kind}", path.display()),
        }
    }
    if !sky.layers.is_empty() || sky.decorations.is_some() {
        log::info!(
            "Loaded {} native static custom sky layers",
            sky.layers.len()
        );
    }
    (sky, images)
}

/// Separate faces avoid sampling across cell boundaries. A 1024px face cap
/// keeps a 6K transparent sky from expanding the shared GUI atlas by 100 MiB.
fn split_faces(w: u32, h: u32, pixels: &[u8], layer: usize) -> crate::Result<Vec<Image>> {
    if w == 0
        || !w.is_multiple_of(3)
        || !h.is_multiple_of(2)
        || w / 3 != h / 2
        || pixels.len() != w as usize * h as usize * 4
    {
        return Err("sky texture must contain six square faces in a 3 by 2 grid".into());
    }
    let original = w / 3;
    let size = original.min(1024);
    let mut images = Vec::with_capacity(6);
    for face in 0..6 {
        let mut rgba = Vec::with_capacity(size as usize * size as usize * 4);
        for y in 0..size {
            for x in 0..size {
                let sx = (face % 3) * original + x * original / size;
                let sy = (face / 3) * original + y * original / size;
                let at = (sy as usize * w as usize + sx as usize) * 4;
                rgba.extend_from_slice(&pixels[at..at + 4]);
            }
        }
        images.push((format!("wisp/custom-sky/{layer}/{face}"), size, size, rgba));
    }
    Ok(images)
}

impl Sky {
    pub fn geometry(&self, sprites: &HashMap<String, Rect>) -> Vec<Vertex> {
        let mut vertices = Vec::with_capacity(self.layers.len() * 24);
        for layer in &self.layers {
            for face in 0..6 {
                let Some(r) = sprites.get(&layer.sprites[[2, 5, 4, 3, 0, 1][face]]) else {
                    continue;
                };
                // +Z, +X, -Z, -X, -Y, +Y; inside the cube, upright walls.
                let corners = match face {
                    0 => [[1., 1., 1.], [-1., 1., 1.], [-1., -1., 1.], [1., -1., 1.]],
                    1 => [[1., 1., -1.], [1., 1., 1.], [1., -1., 1.], [1., -1., -1.]],
                    2 => [
                        [-1., 1., -1.],
                        [1., 1., -1.],
                        [1., -1., -1.],
                        [-1., -1., -1.],
                    ],
                    3 => [
                        [-1., 1., 1.],
                        [-1., 1., -1.],
                        [-1., -1., -1.],
                        [-1., -1., 1.],
                    ],
                    4 => [
                        [-1., -1., -1.],
                        [1., -1., -1.],
                        [1., -1., 1.],
                        [-1., -1., 1.],
                    ],
                    _ => [[-1., 1., 1.], [1., 1., 1.], [1., 1., -1.], [-1., 1., -1.]],
                };
                let uv = [[r.u0, r.v0], [r.u1, r.v0], [r.u1, r.v1], [r.u0, r.v1]];
                for (p, uv) in corners.into_iter().zip(uv) {
                    vertices.push(Vertex {
                        pos: p.map(|v| v * 100.0),
                        uv,
                        layer: OVERLAY,
                        normal: [0; 4],
                        color: u32::MAX,
                        light: 0,
                        glint_uv: [0.; 2],
                    });
                }
            }
        }
        vertices
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_pack_loads_texture_and_vanilla_decorations() {
        let root = std::env::temp_dir().join(format!("wisp-static-sky-{}", std::process::id()));
        std::fs::create_dir_all(root.join("assets/nuit/sky")).unwrap();
        std::fs::create_dir_all(root.join("assets/example/textures")).unwrap();
        let mut png_bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png_bytes, 3, 2);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&[255; 24])
                .unwrap();
        }
        std::fs::write(root.join("assets/example/textures/sky.png"), png_bytes).unwrap();
        std::fs::write(
            root.join("assets/nuit/sky/10_image.json"),
            r#"{
            "schemaVersion":1,"type":"square-textured","texture":"example:textures/sky.png",
            "properties":{"layer":1,"blend":"normal","rotation":{"skyboxRotation":true,"speed":0}},
            "conditions":{"skyboxes":{"entries":["minecraft:overworld"]}}
        }"#,
        )
        .unwrap();
        // Folder-pack overlays use file symlinks on Unix. Follow these without
        // walking directory symlinks (which could introduce a traversal cycle).
        #[cfg(unix)]
        {
            let definition = root.join("assets/nuit/sky/10_image.json");
            let source = root.join("image-definition.json");
            std::fs::rename(&definition, &source).unwrap();
            std::os::unix::fs::symlink(source, definition).unwrap();
        }
        std::fs::write(root.join("assets/nuit/sky/20_decorations.json"), r#"{
            "schemaVersion":1,"type":"decorations","showSun":true,"showMoon":false,"showStars":true,
            "properties":{"layer":2,"blend":"decorations","rotation":{"skyboxRotation":false,"axis":{"0":[0,0,90]}}}
        }"#).unwrap();
        let (sky, images) = load(&root);
        assert_eq!(images.len(), 6);
        assert_eq!(sky.layers.len(), 1);
        assert_eq!(sky.decorations, Some([true, false, true]));
        assert!(
            images
                .iter()
                .all(|(_, w, h, rgba)| (*w, *h) == (1, 1) && rgba == &[255; 4])
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sky_faces_use_the_correct_cells_and_keep_the_artwork_upright() {
        let names = std::array::from_fn(|i| i.to_string());
        let sprites = names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                (
                    name.clone(),
                    Rect {
                        u0: i as f32,
                        u1: i as f32 + 1.0,
                        v0: 0.0,
                        v1: 1.0,
                        w: 1,
                        h: 1,
                    },
                )
            })
            .collect();
        let sky = Sky {
            layers: vec![Layer { sprites: names }],
            decorations: None,
        };
        let vertices = sky.geometry(&sprites);
        assert_eq!(vertices.len(), 24);
        // South and north use cells 2 and 4, with the left image edge facing
        // the viewer's left. Bottom/top are the first two cells of the sheet.
        assert_eq!(vertices[0].uv, [2.0, 0.0]);
        assert_eq!(vertices[0].pos, [100.0, 100.0, 100.0]);
        assert_eq!(vertices[8].uv, [4.0, 0.0]);
        assert_eq!(vertices[8].pos, [-100.0, 100.0, -100.0]);
        assert_eq!(vertices[16].uv, [0.0, 0.0]);
        assert_eq!(vertices[20].uv, [1.0, 0.0]);
    }
    #[test]
    fn face_cells_keep_their_pixels_and_transparency() {
        let pixels: Vec<u8> = (0..6u8).flat_map(|v| [v, 0, 0, v * 40]).collect();
        let faces = split_faces(3, 2, &pixels, 0).unwrap();
        for (i, (_, w, h, rgba)) in faces.iter().enumerate() {
            assert_eq!((*w, *h), (1, 1));
            assert_eq!(rgba, &pixels[i * 4..i * 4 + 4]);
        }
        assert!(split_faces(3, 3, &[0; 36], 0).is_err());
    }
    #[test]
    fn foreign_dimension_and_unimplemented_conditions_never_render_globally() {
        let parse = |s| serde_json::from_str(s).unwrap();
        assert!(supported(&parse(
            r#"{"schemaVersion":1,"type":"square-textured","properties":{"rotation":{"speed":0}},"conditions":{"skyboxes":{"entries":["minecraft:overworld"]}}}"#
        )));
        assert!(!supported(&parse(
            r#"{"schemaVersion":1,"conditions":{"dimensions":{"entries":["minecraft:the_nether"]}}}"#
        )));
        assert!(!supported(&parse(
            r#"{"schemaVersion":1,"conditions":{"weather":{"entries":["clear"]}}}"#
        )));
        assert!(resource_path("lyvt:textures/sky/a.png").is_some());
        assert!(resource_path("lyvt:../secret").is_none());
    }
}
