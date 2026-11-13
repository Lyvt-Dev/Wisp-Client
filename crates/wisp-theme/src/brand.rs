//! Original Wisp folded-A mark, shared with the application's SVG/ICO artwork.
pub const NAME: &str = "Wisp";

/// Convex quads in a 64-unit view box and their RGB colours.
pub const MARK: [([[f32; 2]; 4], u32); 3] = [
    (
        [[32.0, 4.0], [5.0, 57.0], [19.0, 57.0], [32.0, 29.0]],
        0xa3a3a3,
    ),
    (
        [[32.0, 4.0], [32.0, 29.0], [45.0, 57.0], [59.0, 57.0]],
        0x707070,
    ),
    (
        [[23.0, 37.0], [41.0, 37.0], [47.0, 48.0], [17.0, 48.0]],
        0xbcbcbc,
    ),
];
