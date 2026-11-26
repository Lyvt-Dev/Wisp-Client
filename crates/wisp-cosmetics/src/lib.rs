//! Cosmetic items that Wisp clients draw on players: hats, back items,
//! shoulder pets, name tag styles. Only Wisp clients see them; the game protocol carries
//! nothing about them ([docs/cosmetics.md](../../../docs/cosmetics.md)).
//!
//! An item is a small model of boxes (Minecraft's box UV, as Blockbench's
//! "Modded Entity" format), one texture of at most 256×256 (animated: a strip of frames), the bone of the
//! player model it rides on and a few simple looping animations; a name tag
//! style is a small mark and a line colour, with no model. This crate
//! reads and checks items and poses them; the game draws them.

pub mod account;
mod fetch;
mod item;
mod pose;
pub mod prefs;
pub mod preview;
pub mod share;

pub use fetch::{BATCH, Clock, Cosmetics, Frame, Http, Local, Mixed, Transport, Worn};
pub use item::{
    Animation, Aura, AuraStyle, Axis, Bone, Error, FORMAT, Face, Item, MAX_FRAMES, MAX_MARK,
    MAX_PARTS, MAX_REACH, MAX_TEXTURE, MAX_TRAIL_COLOURS, MAX_VERTICES, Motion, Part, Slot, Tag,
    Texture, Trail, TrailStyle,
};
pub use pose::{Pose, pose_matrix};

/// Disabled until Wisp has its own cosmetics service. Explicit configuration can enable it.
pub const ON_BY_DEFAULT: bool = false;

/// The cosmetics service (`WISP_COSMETICS_API` overrides it).
pub const API: &str = "";

/// Whether cosmetics run: `ON_BY_DEFAULT`, or `WISP_COSMETICS=1` / `0` for one run
/// (development, the bench).
pub fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| match wisp_compat::var("WISP_COSMETICS").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        _ => ON_BY_DEFAULT,
    })
}

/// `WISP_COSMETICS=1` asked for this run: the launcher shows its Wardrobe even while the
/// cosmetics service has no Shop to show (development, screenshots).
pub fn forced() -> bool {
    wisp_compat::var("WISP_COSMETICS").as_deref() == Ok("1")
}
