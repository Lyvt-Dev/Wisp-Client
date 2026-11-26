//! The item file: JSON with the model, its attachment and its texture (a
//! base64 PNG), checked against the limits below when read.

use crate::pose::{Pose, pose_matrix};
use serde::Deserialize;

/// The item file's version.
pub const FORMAT: u32 = 1;
/// Vertices one item may draw (4 per face): 42 boxes, or a Blockbench model of about 250 faces.
/// A player's own model and its outer layer are 288.
pub const MAX_VERTICES: usize = 1024;
/// The largest side of one frame of an item's texture: it fits one of the game atlas's HD
/// cosmetic slots (256x256; a texture of at most 64x64 takes a skin slot instead).
pub const MAX_TEXTURE: u32 = 256;
/// Frames an animated texture may have (a vertical strip of equal frames).
pub const MAX_FRAMES: u32 = 64;
/// Parts per item, the attachment's root not counted.
pub const MAX_PARTS: usize = 16;
/// How far (pixels, 1/16 block) any vertex may reach from the bone's pivot
/// at rest, animations included: an item stays near its wearer.
pub const MAX_REACH: f32 = 24.0;
/// The largest side of a name tag's mark (square, drawn before the name).
pub const MAX_MARK: u32 = 16;

/// Where an item is worn: one item per slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    Head,
    Back,
    /// A weapon carried on the back (a sword across it): beside a cape, not in its place.
    Weapon,
    Shoulder,
    /// Over the body: a shirt, its sleeves on the arms (top-level parts may ride another bone,
    /// `Part`'s `bone` in the file).
    Torso,
    /// A name tag style: no model, a square mark drawn before the player's
    /// name and a line under it (`Item::tag`).
    NameTag,
    /// A trail: no model, particles left behind the wearer as they move (`Item::trail`).
    Trail,
    /// An aura: no model, particles around the wearer all the time (`Item::aura`).
    Aura,
}

/// How an aura's particles look and move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuraStyle {
    /// Short zigzag bolts of lightning crackling around the body.
    Storm,
    /// A ring of star motes circling the waist.
    Orbit,
    /// Two strands of stardust spiralling up around the body.
    Nebula,
}

/// An aura: its style and its colours, RGB.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Aura {
    pub style: AuraStyle,
    pub colours: Vec<[u8; 3]>,
}

/// How a trail's particles look and move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrailStyle {
    /// Soft dust drifting up from the feet, with a few white sparkles.
    Stardust,
    /// A dense line of large dust along the path, fading from the head colour to the tail's.
    Comet,
}

/// A trail: its style and its colours, head (where the wearer is) to tail, RGB.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trail {
    pub style: TrailStyle,
    pub colours: Vec<[u8; 3]>,
}

/// Colours a trail takes.
pub const MAX_TRAIL_COLOURS: usize = 4;

/// The part of the player model an item follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bone {
    #[default]
    Head,
    Body,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
}

impl Bone {
    /// The part's name in the player model.
    pub fn part(self) -> &'static str {
        match self {
            Bone::Head => "head",
            Bone::Body => "body",
            Bone::LeftArm => "left_arm",
            Bone::RightArm => "right_arm",
            Bone::LeftLeg => "left_leg",
            Bone::RightLeg => "right_leg",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Motion {
    /// Turns back and forth: `amplitude` degrees about the axis.
    Swing,
    /// Moves back and forth: `amplitude` pixels along the axis.
    Bob,
    /// Turns all the way round once per period.
    Spin,
}

/// A looping animation of one part, on top of its rest pose: a sine of
/// `period` ticks starting `phase` (a fraction of the period) in.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Animation {
    pub part: String,
    pub motion: Motion,
    pub axis: Axis,
    #[serde(default)]
    pub amplitude: f32,
    pub period: f32,
    #[serde(default)]
    pub phase: f32,
}

/// One quad: the game's `Polygon` (model pixels, then UV in 0..1 of the
/// item's texture).
#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    pub normal: [f32; 3],
    pub vertices: [[f32; 5]; 4],
}

/// A part: its pose (as the game's: x, y, z in pixels, rotations in
/// radians, scales), its faces and its children.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub name: String,
    pub pose: Pose,
    pub faces: Vec<Face>,
    pub children: Vec<Part>,
}

/// RGBA pixels, row by row.
#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A name tag style: the line drawn under the name, RGB. The mark is the
/// item's texture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tag {
    pub line: [u8; 3],
}

/// A checked item. `root` is named `cosmetic` and holds the attachment as
/// its pose; the item's own parts are its children.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: String,
    pub slot: Slot,
    pub bone: Bone,
    pub root: Part,
    /// The texture: its first frame, the one a still picture shows.
    pub texture: Texture,
    /// Every frame of an animated texture, the first one included (one frame: not animated).
    pub frames: Vec<Texture>,
    /// Game ticks each frame shows.
    pub frametime: u32,
    pub animations: Vec<Animation>,
    pub vertices: usize,
    /// Top-level parts that ride another bone than `bone` (a shirt's sleeves on the arms), one
    /// root per bone, named `cosmetic_<bone>`, each with the item's attachment as its pose.
    pub limbs: Vec<(Bone, Part)>,
    /// A name tag's line (`Slot::NameTag` only; it has no parts).
    pub tag: Option<Tag>,
    /// A trail's particles (`Slot::Trail` only; it has no parts or texture).
    pub trail: Option<Trail>,
    /// Drawn at full brightness, whatever the light where the wearer stands (a star pet).
    pub glow: bool,
    /// An aura's particles (`Slot::Aura` only; it has no parts or texture).
    pub aura: Option<Aura>,
    /// A shoulder item's sparkles: glowing motes shed around it, these colours (none: no
    /// sparkles).
    pub sparkles: Vec<[u8; 3]>,
}

#[derive(Debug, PartialEq)]
pub enum Error {
    Json(String),
    Format(u32),
    Texture(String),
    TooManyVertices(usize),
    TooManyParts(usize),
    TooFar(f32),
    Invalid(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Json(e) => write!(f, "not an item file: {e}"),
            Error::Format(v) => write!(f, "item format {v}, this game reads {FORMAT}"),
            Error::Texture(e) => write!(f, "texture: {e}"),
            Error::TooManyVertices(n) => write!(f, "{n} vertices, at most {MAX_VERTICES}"),
            Error::TooManyParts(n) => write!(f, "{n} parts, at most {MAX_PARTS}"),
            Error::TooFar(d) => write!(
                f,
                "reaches {d:.1} pixels from its bone, at most {MAX_REACH}"
            ),
            Error::Invalid(e) => f.write_str(e),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemFile {
    format: u32,
    id: String,
    slot: Slot,
    #[serde(default)]
    bone: Bone,
    #[serde(default)]
    attach: AttachFile,
    /// Every slot's but a trail's.
    #[serde(default)]
    texture: Option<TextureFile>,
    #[serde(default)]
    parts: Vec<PartFile>,
    #[serde(default)]
    animations: Vec<Animation>,
    #[serde(default)]
    tag: Option<TagFile>,
    #[serde(default)]
    trail: Option<TrailFile>,
    #[serde(default)]
    glow: bool,
    #[serde(default)]
    aura: Option<AuraFile>,
    #[serde(default)]
    sparkles: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuraFile {
    style: AuraStyle,
    colours: Vec<String>,
}

/// 1 to `MAX_TRAIL_COLOURS` `#rrggbb` colours of `what`.
fn colours(list: &[String], what: &str) -> Result<Vec<[u8; 3]>, Error> {
    if list.is_empty() || list.len() > MAX_TRAIL_COLOURS {
        return Err(invalid(format!(
            "{what} has 1 to {MAX_TRAIL_COLOURS} colours"
        )));
    }
    list.iter()
        .map(|c| colour(c).ok_or_else(|| invalid(format!("{what}'s colours are #rrggbb"))))
        .collect()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TagFile {
    line: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TrailFile {
    style: TrailStyle,
    colours: Vec<String>,
}

/// `#rrggbb`.
fn colour(text: &str) -> Option<[u8; 3]> {
    let hex = text.strip_prefix('#').filter(|h| h.len() == 6)?;
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachFile {
    #[serde(default)]
    offset: [f32; 3],
    #[serde(default)]
    rotation: [f32; 3],
    #[serde(default = "one")]
    scale: f32,
}

impl Default for AttachFile {
    fn default() -> Self {
        Self {
            offset: [0.0; 3],
            rotation: [0.0; 3],
            scale: 1.0,
        }
    }
}

fn one() -> f32 {
    1.0
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextureFile {
    png: String,
    /// An animated texture: the PNG is a vertical strip of this many equal frames.
    #[serde(default = "one_frame")]
    frames: u32,
    /// Game ticks each frame shows.
    #[serde(default = "one_frame")]
    frametime: u32,
}

fn one_frame() -> u32 {
    1
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PartFile {
    name: String,
    /// A top-level part only: the bone it rides, if not the item's.
    #[serde(default)]
    bone: Option<Bone>,
    #[serde(default)]
    pivot: [f32; 3],
    /// Degrees.
    #[serde(default)]
    rotation: [f32; 3],
    #[serde(default)]
    cubes: Vec<CubeFile>,
    /// Faces given corner by corner: a model built in Blockbench, its rotated cubes and
    /// per-face UVs already worked out (`examples/shop_items.rs` converts a `.bbmodel`).
    #[serde(default)]
    quads: Vec<QuadFile>,
    #[serde(default)]
    children: Vec<PartFile>,
}

/// One quad: four corners as `[x, y, z, u, v]`, model pixels then UV in 0..1 of the texture,
/// in vanilla's order (a face's corners at its UV's (u1, v0), (u0, v0), (u0, v1), (u1, v1)).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuadFile {
    vertices: [[f32; 5]; 4],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CubeFile {
    origin: [f32; 3],
    size: [f32; 3],
    uv: [f32; 2],
    #[serde(default)]
    inflate: f32,
    #[serde(default)]
    mirror: bool,
}

fn invalid(text: impl Into<String>) -> Error {
    Error::Invalid(text.into())
}

fn finite(values: &[f32], what: &str) -> Result<(), Error> {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(invalid(format!("{what} is not a number")))
    }
}

impl Item {
    /// Reads and checks an item file.
    pub fn parse(bytes: &[u8]) -> Result<Item, Error> {
        let file: ItemFile =
            serde_json::from_slice(bytes).map_err(|e| Error::Json(e.to_string()))?;
        if file.format != FORMAT {
            return Err(Error::Format(file.format));
        }
        if file.id.is_empty()
            || file.id.len() > 64
            || !file
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err(invalid("the id is 1 to 64 of a-z, 0-9 and _"));
        }
        let trail = match (file.slot, &file.trail) {
            (Slot::Trail, Some(t)) => {
                if !file.parts.is_empty() || !file.animations.is_empty() || file.texture.is_some() {
                    return Err(invalid("a trail has no parts, animations or texture"));
                }
                if t.colours.is_empty() || t.colours.len() > MAX_TRAIL_COLOURS {
                    return Err(invalid(format!(
                        "a trail has 1 to {MAX_TRAIL_COLOURS} colours"
                    )));
                }
                let colours = t
                    .colours
                    .iter()
                    .map(|c| colour(c).ok_or_else(|| invalid("trail.colours are #rrggbb")))
                    .collect::<Result<_, _>>()?;
                Some(Trail {
                    style: t.style,
                    colours,
                })
            }
            (Slot::Trail, None) => return Err(invalid("a trail needs its trail")),
            (_, Some(_)) => return Err(invalid("only a trail has a trail")),
            (_, None) => None,
        };
        let aura = match (file.slot, &file.aura) {
            (Slot::Aura, Some(a)) => {
                if !file.parts.is_empty() || !file.animations.is_empty() || file.texture.is_some() {
                    return Err(invalid("an aura has no parts, animations or texture"));
                }
                Some(Aura {
                    style: a.style,
                    colours: colours(&a.colours, "an aura")?,
                })
            }
            (Slot::Aura, None) => return Err(invalid("an aura needs its aura")),
            (_, Some(_)) => return Err(invalid("only an aura has an aura")),
            (_, None) => None,
        };
        let sparkles = match (file.slot, file.sparkles.is_empty()) {
            (_, true) => Vec::new(),
            (Slot::Shoulder | Slot::Back, false) => colours(&file.sparkles, "sparkles")?,
            _ => return Err(invalid("only a shoulder or back item has sparkles")),
        };
        let (frames, frametime) = match (&file.texture, file.slot) {
            (Some(t), _) => (decode_frames(t)?, t.frametime),
            // A trail or an aura draws particles, not a texture.
            (None, Slot::Trail | Slot::Aura) => (
                vec![Texture {
                    width: 1,
                    height: 1,
                    rgba: vec![0; 4],
                }],
                1,
            ),
            (None, _) => return Err(invalid("the item needs its texture")),
        };
        let texture = frames[0].clone();
        let tag = match (file.slot, &file.tag) {
            (Slot::NameTag, Some(t)) => {
                if !file.parts.is_empty() || !file.animations.is_empty() {
                    return Err(invalid("a name tag has no parts or animations"));
                }
                if texture.width != texture.height || texture.width > MAX_MARK {
                    return Err(Error::Texture(format!(
                        "a name tag's mark is square, at most {MAX_MARK}x{MAX_MARK}"
                    )));
                }
                let line = colour(&t.line).ok_or_else(|| invalid("tag.line is #rrggbb"))?;
                Some(Tag { line })
            }
            (Slot::NameTag, None) => return Err(invalid("a name tag needs its tag")),
            (_, Some(_)) => return Err(invalid("only a name tag has a tag")),
            (_, None) => None,
        };
        let a = &file.attach;
        finite(&a.offset, "attach.offset")?;
        finite(&a.rotation, "attach.rotation")?;
        if !(a.scale > 0.0 && a.scale <= 2.0) {
            return Err(invalid("attach.scale is above 0 and at most 2"));
        }
        let [rx, ry, rz] = a.rotation.map(f32::to_radians);
        let pose = [
            a.offset[0],
            a.offset[1],
            a.offset[2],
            rx,
            ry,
            rz,
            a.scale,
            a.scale,
            a.scale,
        ];
        let size = [texture.width as f32, texture.height as f32];
        let mut names = Vec::new();
        let mut children = Vec::new();
        let mut limbs: Vec<(Bone, Part)> = Vec::new();
        for p in &file.parts {
            let part = bake(p, size, &mut names, true)?;
            match p.bone.filter(|b| *b != file.bone) {
                None => children.push(part),
                Some(bone) => match limbs.iter_mut().find(|(b, _)| *b == bone) {
                    Some((_, limb)) => limb.children.push(part),
                    None => limbs.push((
                        bone,
                        Part {
                            name: format!("cosmetic_{}", bone.part()),
                            pose,
                            faces: Vec::new(),
                            children: vec![part],
                        },
                    )),
                },
            }
        }
        if names.len() > MAX_PARTS {
            return Err(Error::TooManyParts(names.len()));
        }
        let root = Part {
            name: "cosmetic".into(),
            pose,
            faces: Vec::new(),
            children,
        };
        let vertices = count(&root) + limbs.iter().map(|(_, l)| count(l)).sum::<usize>();
        if vertices > MAX_VERTICES {
            return Err(Error::TooManyVertices(vertices));
        }
        for anim in &file.animations {
            finite(&[anim.amplitude, anim.period, anim.phase], "an animation")?;
            if !names.contains(&anim.part) {
                return Err(invalid(format!(
                    "an animation moves no part {:?}",
                    anim.part
                )));
            }
            let limit = match anim.motion {
                Motion::Swing => 45.0,
                Motion::Bob => 4.0,
                Motion::Spin => 0.0,
            };
            if anim.amplitude.abs() > limit || anim.period < 4.0 || anim.period > 1200.0 {
                return Err(invalid(format!(
                    "the animation of {:?} is out of bounds",
                    anim.part
                )));
            }
        }
        let item = Item {
            id: file.id,
            slot: file.slot,
            bone: file.bone,
            root,
            texture,
            frames,
            frametime,
            animations: file.animations,
            vertices,
            limbs,
            tag,
            trail,
            glow: file.glow,
            aura,
            sparkles,
        };
        let reach = item.reach();
        if reach > MAX_REACH {
            return Err(Error::TooFar(reach));
        }
        Ok(item)
    }

    /// The item's roots and the bone each rides: `root` on `bone`, then the `limbs`.
    pub fn roots(&self) -> impl Iterator<Item = (Bone, &Part)> {
        std::iter::once((self.bone, &self.root)).chain(self.limbs.iter().map(|(b, p)| (*b, p)))
    }

    /// The farthest any vertex gets from the bone's pivot (pixels): at rest,
    /// and at 32 points of each animation's period.
    fn reach(&self) -> f32 {
        let mut times = vec![0.0];
        for anim in &self.animations {
            times.extend((0..32).map(|k| anim.period * k as f32 / 32.0));
        }
        let mut far = 0.0_f32;
        let mut poses = std::collections::HashMap::new();
        for ticks in times {
            poses.clear();
            self.animate(ticks, |name, pose| {
                poses.insert(name.to_owned(), pose);
            });
            for (_, root) in self.roots() {
                walk(root, glam::Mat4::IDENTITY, &poses, &mut |p| {
                    far = far.max(p.length() * 16.0);
                });
            }
        }
        far
    }

    /// The animated parts' poses at `ticks` (game ticks, any start): each
    /// is the part's rest pose with its animations added.
    pub fn animate(&self, ticks: f32, mut set: impl FnMut(&str, Pose)) {
        let mut done: Vec<&str> = Vec::new();
        for anim in &self.animations {
            if done.contains(&anim.part.as_str()) {
                continue;
            }
            done.push(&anim.part);
            let Some(part) = self.roots().find_map(|(_, r)| find(r, &anim.part)) else {
                continue;
            };
            let mut pose = part.pose;
            for a in self.animations.iter().filter(|a| a.part == anim.part) {
                let turns = ticks / a.period + a.phase;
                let axis = a.axis as usize;
                match a.motion {
                    Motion::Swing => {
                        let wave = (turns * std::f32::consts::TAU).sin();
                        pose[3 + axis] += (a.amplitude * wave).to_radians();
                    }
                    Motion::Bob => {
                        pose[axis] += a.amplitude * (turns * std::f32::consts::TAU).sin();
                    }
                    Motion::Spin => {
                        pose[3 + axis] += turns.rem_euclid(1.0) * std::f32::consts::TAU;
                    }
                }
            }
            set(&anim.part, pose);
        }
    }
}

/// Every vertex of `part`'s subtree under `parent`, as posed by `poses`
/// (else the parts' rest poses), in blocks.
fn walk(
    part: &Part,
    parent: glam::Mat4,
    poses: &std::collections::HashMap<String, Pose>,
    each: &mut impl FnMut(glam::Vec3),
) {
    let m = parent * pose_matrix(*poses.get(&part.name).unwrap_or(&part.pose));
    for face in &part.faces {
        for v in face.vertices {
            each(m.transform_point3(glam::Vec3::new(v[0], v[1], v[2]) / 16.0));
        }
    }
    for child in &part.children {
        walk(child, m, poses, each);
    }
}

fn find<'a>(part: &'a Part, name: &str) -> Option<&'a Part> {
    if part.name == name {
        return Some(part);
    }
    part.children.iter().find_map(|c| find(c, name))
}

fn count(part: &Part) -> usize {
    part.faces.len() * 4 + part.children.iter().map(count).sum::<usize>()
}

fn bake(
    file: &PartFile,
    size: [f32; 2],
    names: &mut Vec<String>,
    top: bool,
) -> Result<Part, Error> {
    if file.name.is_empty() || file.name.starts_with("cosmetic") || names.contains(&file.name) {
        return Err(invalid(format!("part names are unique: {:?}", file.name)));
    }
    if !top && file.bone.is_some() {
        return Err(invalid(format!(
            "only a top-level part rides its own bone: {:?}",
            file.name
        )));
    }
    names.push(file.name.clone());
    finite(&file.pivot, "a pivot")?;
    finite(&file.rotation, "a rotation")?;
    let mut faces = Vec::new();
    for cube in &file.cubes {
        finite(&cube.origin, "a cube")?;
        finite(&cube.size, "a cube")?;
        finite(&cube.uv, "a cube")?;
        if cube.size.iter().any(|s| *s < 0.0) || !(0.0..=4.0).contains(&cube.inflate) {
            return Err(invalid("a cube's size is negative"));
        }
        let [w, h, d] = cube.size;
        // Box UV: the unfolded box spans 2(d + w) by d + h texels.
        if cube.uv[0] < 0.0
            || cube.uv[1] < 0.0
            || cube.uv[0] + 2.0 * (d + w) > size[0]
            || cube.uv[1] + d + h > size[1]
        {
            return Err(invalid("a cube's UV leaves the texture"));
        }
        faces.extend(box_faces(cube, size));
    }
    for quad in &file.quads {
        let flat: Vec<f32> = quad.vertices.iter().flatten().copied().collect();
        finite(&flat, "a quad")?;
        if quad
            .vertices
            .iter()
            .any(|v| !(0.0..=1.0).contains(&v[3]) || !(0.0..=1.0).contains(&v[4]))
        {
            return Err(invalid("a quad's UV leaves the texture"));
        }
        let corner = |i: usize| glam::Vec3::from_slice(&quad.vertices[i][..3]);
        let n = (corner(1) - corner(0)).cross(corner(2) - corner(1));
        let n = if n.length_squared() > 1e-12 {
            n.normalize()
        } else {
            glam::Vec3::Y
        };
        faces.push(Face {
            normal: n.to_array(),
            vertices: quad.vertices,
        });
    }
    let [rx, ry, rz] = file.rotation.map(f32::to_radians);
    let [x, y, z] = file.pivot;
    let children = file
        .children
        .iter()
        .map(|c| bake(c, size, names, false))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Part {
        name: file.name.clone(),
        pose: [x, y, z, rx, ry, rz, 1.0, 1.0, 1.0],
        faces,
        children,
    })
}

/// `box_faces` of a box given field by field.
pub(crate) fn cube_faces(
    origin: [f32; 3],
    size: [f32; 3],
    uv: [f32; 2],
    inflate: f32,
    mirror: bool,
    texture: [f32; 2],
) -> Vec<Face> {
    let cube = CubeFile {
        origin,
        size,
        uv,
        inflate,
        mirror,
    };
    box_faces(&cube, texture)
}

/// `ModelPart.Cube`: six faces with box UV, in vanilla's order (down, up,
/// west, north, east, south), grown by `inflate`, mirrored in x on request.
fn box_faces(cube: &CubeFile, [tw, th]: [f32; 2]) -> Vec<Face> {
    let [w, h, d] = cube.size;
    let g = cube.inflate;
    let (mut x0, y0, z0) = (cube.origin[0] - g, cube.origin[1] - g, cube.origin[2] - g);
    let (mut x1, y1, z1) = (
        cube.origin[0] + w + g,
        cube.origin[1] + h + g,
        cube.origin[2] + d + g,
    );
    if cube.mirror {
        std::mem::swap(&mut x0, &mut x1);
    }
    let t0 = [x0, y0, z0];
    let t1 = [x1, y0, z0];
    let t2 = [x1, y1, z0];
    let t3 = [x0, y1, z0];
    let l0 = [x0, y0, z1];
    let l1 = [x1, y0, z1];
    let l2 = [x1, y1, z1];
    let l3 = [x0, y1, z1];
    let [u, v] = cube.uv;
    let (u1, u2, u22) = (u + d, u + d + w, u + d + w + w);
    let (u3, u4) = (u + d + w + d, u + d + w + d + w);
    let (v1, v2) = (v + d, v + d + h);
    let faces: [([[f32; 3]; 4], [f32; 4], [f32; 3]); 6] = [
        ([l1, l0, t0, t1], [u1, v, u2, v1], [0.0, -1.0, 0.0]),
        ([t2, t3, l3, l2], [u2, v1, u22, v], [0.0, 1.0, 0.0]),
        ([t0, l0, l3, t3], [u, v1, u1, v2], [-1.0, 0.0, 0.0]),
        ([t1, t0, t3, t2], [u1, v1, u2, v2], [0.0, 0.0, -1.0]),
        ([l1, t1, t2, l2], [u2, v1, u3, v2], [1.0, 0.0, 0.0]),
        ([l0, l1, l2, l3], [u3, v1, u4, v2], [0.0, 0.0, 1.0]),
    ];
    faces
        .into_iter()
        .map(|(corners, [a, b, c, e], mut normal)| {
            // Polygon's remap: (u1, v0), (u0, v0), (u0, v1), (u1, v1).
            let uvs = [[c, b], [a, b], [a, e], [c, e]];
            let mut vertices: [[f32; 5]; 4] = std::array::from_fn(|i| {
                let [x, y, z] = corners[i];
                [x, y, z, uvs[i][0] / tw, uvs[i][1] / th]
            });
            if cube.mirror {
                vertices.reverse();
                normal[0] = -normal[0];
            }
            Face { normal, vertices }
        })
        .collect()
}

/// The frames of a texture: its PNG cut into `frames` equal frames, top to bottom.
fn decode_frames(t: &TextureFile) -> Result<Vec<Texture>, Error> {
    if !(1..=MAX_FRAMES).contains(&t.frames) || !(1..=100).contains(&t.frametime) {
        return Err(Error::Texture(format!(
            "1 to {MAX_FRAMES} frames of 1 to 100 ticks each"
        )));
    }
    let strip = decode_png(&t.png, t.frames)?;
    if strip.height % t.frames != 0 {
        return Err(Error::Texture(format!(
            "{} rows do not split into {} equal frames",
            strip.height, t.frames
        )));
    }
    let h = strip.height / t.frames;
    let row = strip.width as usize * 4;
    Ok((0..t.frames as usize)
        .map(|k| Texture {
            width: strip.width,
            height: h,
            rgba: strip.rgba[k * h as usize * row..(k + 1) * h as usize * row].to_vec(),
        })
        .collect())
}

/// A PNG of `frames` frames stacked vertically, each at most `MAX_TEXTURE` square.
fn decode_png(text: &str, frames: u32) -> Result<Texture, Error> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|e| Error::Texture(e.to_string()))?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|e| Error::Texture(e.to_string()))?;
    let (width, height) = (reader.info().width, reader.info().height);
    if width == 0 || height == 0 || width > MAX_TEXTURE || height > MAX_TEXTURE * frames {
        return Err(Error::Texture(format!(
            "{width}x{height}, at most {MAX_TEXTURE}x{MAX_TEXTURE}"
        )));
    }
    let mut buf = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let frame = reader
        .next_frame(&mut buf)
        .map_err(|e| Error::Texture(e.to_string()))?;
    let pixels = &buf[..frame.buffer_size()];
    let rgba = match frame.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => pixels
            .chunks(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err(Error::Texture("unexpanded palette".into())),
    };
    Ok(Texture {
        width,
        height,
        rgba,
    })
}
