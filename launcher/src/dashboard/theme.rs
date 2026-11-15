//! The live palette. The views are written in the default greys and green; the Appearance
//! settings move those colours to another accent and backdrop as they are drawn.
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering::Relaxed};
use wisp_launcher::prefs::{Backdrop, Prefs};

/// The green every accent shade in the views is written in.
const BASE: u32 = 0x3ccf6a;

/// Black and white on the black backdrop until the settings are read.
static ACCENT: AtomicU32 = AtomicU32::new(0xffffff);
static BACKDROP: AtomicU8 = AtomicU8::new(1);

pub(super) fn apply(prefs: &Prefs) {
    ACCENT.store(prefs.accent.rgb(), Relaxed);
    let index = Backdrop::ALL
        .iter()
        .position(|b| *b == prefs.backdrop)
        .unwrap_or(0);
    BACKDROP.store(index as u8, Relaxed);
}

fn to_hsl(c: u32) -> (f32, f32, f32) {
    let r = ((c >> 16) & 0xff) as f32 / 255.;
    let g = ((c >> 8) & 0xff) as f32 / 255.;
    let b = (c & 0xff) as f32 / 255.;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.;
    if max == min {
        return (0., 0., l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2. - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + if g < b { 6. } else { 0. }
    } else if max == g {
        (b - r) / d + 2.
    } else {
        (r - g) / d + 4.
    };
    (h / 6., s, l)
}

fn from_hsl(h: f32, s: f32, l: f32) -> u32 {
    let h = h.rem_euclid(1.);
    let (s, l) = (s.clamp(0., 1.), l.clamp(0., 1.));
    let q = if l < 0.5 { l * (1. + s) } else { l + s - l * s };
    let p = 2. * l - q;
    let channel = |t: f32| {
        let t = t.rem_euclid(1.);
        let v = if t < 1. / 6. {
            p + (q - p) * 6. * t
        } else if t < 0.5 {
            q
        } else if t < 2. / 3. {
            p + (q - p) * (2. / 3. - t) * 6.
        } else {
            p
        };
        (v * 255.).round() as u32
    };
    (channel(h + 1. / 3.) << 16) | (channel(h) << 8) | channel(h - 1. / 3.)
}

/// A shade of the default green, moved to the chosen accent (`0xRRGGBB`).
pub(super) fn accent_tone(c: u32) -> u32 {
    let accent = ACCENT.load(Relaxed);
    if accent == BASE {
        return c;
    }
    let (h, s, l) = to_hsl(c);
    let (hb, sb, lb) = to_hsl(BASE);
    let (ha, sa, la) = to_hsl(accent);
    if sa == 0.0 {
        // Black and white: the same shade, without its colour.
        return from_hsl(0., 0., l);
    }
    from_hsl(ha + (h - hb), s * sa / sb.max(0.01), l + (la - lb))
}

/// Whether the accent is black and white.
pub(super) fn mono() -> bool {
    ACCENT.load(Relaxed) == 0xffffff
}

/// `accent_tone` for `0xRRGGBBAA`.
pub(super) fn accent_tone_a(c: u32) -> u32 {
    (accent_tone(c >> 8) << 8) | (c & 0xff)
}

/// A neutral grey of the default backdrop, moved to the chosen backdrop (`0xRRGGBB`).
pub(super) fn tone(c: u32) -> u32 {
    let backdrop = Backdrop::ALL[BACKDROP.load(Relaxed) as usize % Backdrop::ALL.len()];
    let (h, s, l) = to_hsl(c);
    match backdrop {
        Backdrop::Midnight => c,
        Backdrop::Black => from_hsl(h, s * 0.5, l * 0.58),
        Backdrop::Slate => from_hsl(0.61, 0.3, l * 1.22),
        Backdrop::Ocean => from_hsl(0.56, 0.55, l * 1.12),
        Backdrop::Plum => from_hsl(0.78, 0.42, l * 1.15),
    }
}

/// `tone` for `0xRRGGBBAA`.
pub(super) fn tone_a(c: u32) -> u32 {
    (tone(c >> 8) << 8) | (c & 0xff)
}

/// How a backdrop's frame, page and panels look, for its preview tile.
pub(super) fn preview(backdrop: Backdrop) -> [u32; 3] {
    let saved = BACKDROP.load(Relaxed);
    let index = Backdrop::ALL
        .iter()
        .position(|b| *b == backdrop)
        .unwrap_or(0);
    BACKDROP.store(index as u8, Relaxed);
    let colours = [tone(0x0b0b0d), tone(0x111215), tone(0x232429)];
    BACKDROP.store(saved, Relaxed);
    colours
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_round_trips() {
        for c in [0x3ccf6a, 0x111215, 0xffffff, 0x000000, 0x9b6bff, 0xef4f4f] {
            let (h, s, l) = to_hsl(c);
            let back = from_hsl(h, s, l);
            for shift in [0, 8, 16] {
                let (a, b) = ((c >> shift) & 0xff, (back >> shift) & 0xff);
                assert!(a.abs_diff(b) <= 1, "{c:06x} -> {back:06x}");
            }
        }
    }
}
