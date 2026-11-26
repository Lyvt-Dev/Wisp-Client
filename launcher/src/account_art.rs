//! Account artwork comes only from the profile's official Minecraft skin: the face for small
//! avatars and a flat front view of the whole player for the profile page.
use std::io::Read;

/// The front view's size in skin texels.
pub const BODY_W: usize = 16;
pub const BODY_H: usize = 32;

/// The face (8 × 8), the front of the whole player (`BODY_W` × `BODY_H`) and the whole skin as
/// a 64 × 64 texture (an old 64 × 32 skin in its top half), RGBA.
pub struct Art {
    pub face: Vec<[u8; 4]>,
    pub body: Vec<[u8; 4]>,
    pub skin: Vec<u8>,
}

fn texture_url(url: &str) -> Option<String> {
    let hash = url
        .strip_prefix("https://textures.minecraft.net/texture/")
        .or_else(|| url.strip_prefix("http://textures.minecraft.net/texture/"))?;
    (!hash.is_empty() && hash.len() <= 128 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| format!("https://textures.minecraft.net/texture/{hash}"))
}

fn decode(png: &[u8]) -> Option<image::RgbaImage> {
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(png), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(64);
    limits.max_image_height = Some(64);
    reader.limits(limits);
    let skin = reader.decode().ok()?.to_rgba8();
    (skin.width() == 64 && [32, 64].contains(&skin.height())).then_some(skin)
}

/// `top` drawn over `base` by its alpha.
fn over(base: [u8; 4], top: [u8; 4]) -> [u8; 4] {
    let a = u32::from(top[3]);
    if a == 0 {
        return base;
    }
    let out_a = a + u32::from(base[3]) * (255 - a) / 255;
    let mut pixel = [0, 0, 0, out_a as u8];
    for i in 0..3 {
        let mixed =
            u32::from(top[i]) * a + u32::from(base[i]) * u32::from(base[3]) * (255 - a) / 255;
        pixel[i] = (mixed / out_a.max(1)) as u8;
    }
    pixel
}

fn face_of(skin: &image::RgbaImage) -> Vec<[u8; 4]> {
    let mut pixels = Vec::with_capacity(64);
    for y in 0..8 {
        for x in 0..8 {
            let base = skin.get_pixel(x + 8, y + 8).0;
            let mut base = [base[0], base[1], base[2], 255];
            base = over(base, skin.get_pixel(x + 40, y + 8).0);
            pixels.push(base);
        }
    }
    pixels
}

/// One limb's front face: where it goes, its size, where it is on the skin and its outer layer.
struct Part {
    out: (usize, usize),
    size: (usize, usize),
    base: (u32, u32),
    overlay: Option<(u32, u32)>,
    mirrored: bool,
}

/// The front faces of head, body, arms and legs with their outer layers. Old 64 × 32 skins have
/// no left limbs or outer body layers: the right limbs are mirrored, as the game does.
fn body_of(skin: &image::RgbaImage, slim: bool) -> Vec<[u8; 4]> {
    let modern = skin.height() == 64;
    let mut out = vec![[0u8; 4]; BODY_W * BODY_H];
    let arm = if slim { 3 } else { 4 };
    // (out x, out y, width, height, base x, base y, overlay x/y if any, mirrored)
    let part = |ox, oy, w, h, bx, by, overlay, mirrored| Part {
        out: (ox, oy),
        size: (w, h),
        base: (bx, by),
        overlay,
        mirrored,
    };
    let mut parts = vec![
        part(4, 0, 8, 8, 8, 8, Some((40, 8)), false),
        part(4, 8, 8, 12, 20, 20, modern.then_some((20, 36)), false),
        part(
            4 - arm,
            8,
            arm,
            12,
            44,
            20,
            modern.then_some((44, 36)),
            false,
        ),
        part(4, 20, 4, 12, 4, 20, modern.then_some((4, 36)), false),
    ];
    if modern {
        parts.push(part(12, 8, arm, 12, 36, 52, Some((52, 52)), false));
        parts.push(part(8, 20, 4, 12, 20, 52, Some((4, 52)), false));
    } else {
        parts.push(part(12, 8, arm, 12, 44, 20, None, true));
        parts.push(part(8, 20, 4, 12, 4, 20, None, true));
    }
    for Part {
        out: (ox, oy),
        size: (w, h),
        base: (bx, by),
        overlay,
        mirrored,
    } in parts
    {
        for y in 0..h {
            for x in 0..w {
                let sx = if mirrored { w - 1 - x } else { x } as u32;
                let base = skin.get_pixel(bx + sx, by + y as u32).0;
                let mut pixel = [base[0], base[1], base[2], 255];
                if let Some((lx, ly)) = overlay {
                    pixel = over(pixel, skin.get_pixel(lx + sx, ly + y as u32).0);
                }
                out[(oy + y) * BODY_W + ox + x] = pixel;
            }
        }
    }
    out
}

/// The art of a skin texture's PNG; `slim` is the Alex model's three-pixel arms.
fn art(png: &[u8], slim: bool) -> Option<Art> {
    let skin = decode(png)?;
    let mut texture = vec![0u8; 64 * 64 * 4];
    texture[..skin.as_raw().len()].copy_from_slice(skin.as_raw());
    Some(Art {
        face: face_of(&skin),
        body: body_of(&skin, slim),
        skin: texture,
    })
}

pub fn fetch(url: &str, slim: bool) -> Option<Art> {
    let url = texture_url(url)?;
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(8))
        .redirects(0)
        .build();
    let response = agent.get(&url).call().ok()?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 1024 * 1024 {
        return None;
    }
    art(&bytes, slim)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(skin: &image::RgbaImage) -> Vec<u8> {
        let mut png = std::io::Cursor::new(Vec::new());
        skin.write_to(&mut png, image::ImageFormat::Png).unwrap();
        png.into_inner()
    }

    #[test]
    fn only_official_texture_hashes_are_fetched_over_https() {
        assert_eq!(
            texture_url("http://textures.minecraft.net/texture/abc123").as_deref(),
            Some("https://textures.minecraft.net/texture/abc123")
        );
        for url in [
            "https://textures.minecraft.net.evil/texture/abc",
            "https://example.com/abc",
            "https://textures.minecraft.net/texture/../private",
            "https://textures.minecraft.net/texture/",
        ] {
            assert!(texture_url(url).is_none());
        }
    }

    #[test]
    fn face_uses_the_players_head_and_hat_layer() {
        let mut skin = image::RgbaImage::new(64, 64);
        for y in 8..16 {
            for x in 8..16 {
                skin.put_pixel(x, y, image::Rgba([20, 40, 60, 255]));
            }
        }
        skin.put_pixel(40, 8, image::Rgba([220, 40, 60, 255]));
        let art = art(&png(&skin), false).unwrap();
        assert_eq!(art.face.len(), 64);
        assert_eq!(art.face[0], [220, 40, 60, 255]);
        assert_eq!(art.face[1], [20, 40, 60, 255]);
        assert!(super::art(b"invalid skin", false).is_none());
    }

    #[test]
    fn body_places_limbs_and_leaves_slim_arms_a_gap() {
        let mut skin = image::RgbaImage::new(64, 64);
        // Body front red, right arm green, left leg blue.
        for y in 20..32 {
            for x in 20..28 {
                skin.put_pixel(x, y, image::Rgba([200, 0, 0, 255]));
            }
            for x in 44..48 {
                skin.put_pixel(x, y, image::Rgba([0, 200, 0, 255]));
            }
        }
        for y in 52..64 {
            for x in 20..24 {
                skin.put_pixel(x, y, image::Rgba([0, 0, 200, 255]));
            }
        }
        let classic = art(&png(&skin), false).unwrap().body;
        assert_eq!(classic.len(), BODY_W * BODY_H);
        assert_eq!(classic[8 * BODY_W + 4], [200, 0, 0, 255]);
        assert_eq!(classic[8 * BODY_W], [0, 200, 0, 255]);
        assert_eq!(classic[20 * BODY_W + 8], [0, 0, 200, 255]);
        let slim = art(&png(&skin), true).unwrap().body;
        assert_eq!(
            slim[8 * BODY_W][3],
            0,
            "slim arms leave the outer column empty"
        );
        assert_eq!(slim[8 * BODY_W + 1], [0, 200, 0, 255]);
        // An old 64 x 32 skin mirrors the right arm onto the left.
        let mut old = image::RgbaImage::new(64, 32);
        for y in 20..32 {
            for x in 44..48 {
                old.put_pixel(x, y, image::Rgba([0, 200, 0, 255]));
            }
        }
        let legacy = art(&png(&old), false).unwrap().body;
        assert_eq!(legacy[8 * BODY_W + 12], [0, 200, 0, 255]);
    }
}
