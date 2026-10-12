//! Keeping text legible against what it is drawn on.
//!
//! The colors a view draws come from three places that know nothing about
//! each other: the palette, which the application sets; runs, which carry
//! whatever a remote user's mIRC codes asked for; and the theme, which the
//! desktop chooses. Any of them can land a color on a background it barely
//! differs from — mIRC white on a light theme, yellow on white, a nick hue
//! picked for one theme shown on the other. So every foreground is checked
//! at draw time against the background it actually lands on, and one that
//! falls short of the target contrast is moved toward black or white, by
//! as little as reaches the target.
//!
//! The arithmetic is WCAG 2's: relative luminance, and the ratio
//! `(L1 + 0.05) / (L2 + 0.05)`. Plain `[r, g, b]` in sRGB, 0..1, so it is
//! tested without a display.

/// The contrast WCAG asks of body text (level AA).
pub(crate) const MIN_RATIO: f32 = 4.5;

/// The contrast asked when the desktop wants more of it (level AAA).
pub(crate) const HIGH_RATIO: f32 = 7.0;

pub(crate) type Rgb = [f32; 3];

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn luminance_linear(l: Rgb) -> f32 {
    0.2126 * l[0] + 0.7152 * l[1] + 0.0722 * l[2]
}

/// Relative luminance, 0 for black and 1 for white.
pub(crate) fn luminance(c: Rgb) -> f32 {
    luminance_linear(c.map(to_linear))
}

/// The contrast ratio between two colors, from 1 (none) to 21.
pub(crate) fn ratio(a: Rgb, b: Rgb) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `fg` if it reaches `min` against `bg`, otherwise `fg` moved toward
/// black or white — whichever side of `bg` has more room — just far
/// enough to reach it, or as far as that side goes when nothing does.
///
/// The move is a blend in linear light, where luminance is linear in the
/// blend and so the amount needed has a closed form: no search, cheap
/// enough to run for every run on every frame. Darkening keeps the hue
/// exactly; lightening washes it out toward white, the least a lighter
/// color can do.
pub(crate) fn ensure(fg: Rgb, bg: Rgb, min: f32) -> Rgb {
    if ratio(fg, bg) >= min {
        return fg;
    }
    let lin = fg.map(to_linear);
    let l = luminance_linear(lin);
    let lb = luminance(bg);
    // A whisker past the target, so rounding on the way back to sRGB
    // can't leave the result just short of it.
    let min = min * 1.01;
    let toward_white = 1.05 / (lb + 0.05) > (lb + 0.05) / 0.05;
    let out = if toward_white {
        let want = (min * (lb + 0.05) - 0.05).min(1.0);
        // White already, or past what is wanted: nothing lighter to go to.
        if l >= want {
            return fg;
        }
        let t = ((want - l) / (1.0 - l)).clamp(0.0, 1.0);
        lin.map(|c| c + (1.0 - c) * t)
    } else {
        let want = ((lb + 0.05) / min - 0.05).max(0.0);
        if l <= want {
            return fg;
        }
        let k = want / l;
        lin.map(|c| c * k)
    };
    out.map(to_srgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Rgb = [1.0, 1.0, 1.0];
    const BLACK: Rgb = [0.0, 0.0, 0.0];

    fn hex(v: u32) -> Rgb {
        [(v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff].map(|c| c as f32 / 255.0)
    }

    #[test]
    fn ratios_match_wcag() {
        assert!((ratio(BLACK, WHITE) - 21.0).abs() < 0.01);
        assert!((ratio(WHITE, WHITE) - 1.0).abs() < 1e-6);
        // WCAG's own worked example: #777777 on white is about 4.48.
        assert!((ratio(hex(0x777777), WHITE) - 4.48).abs() < 0.01);
        assert_eq!(ratio(hex(0x1c71d8), WHITE), ratio(WHITE, hex(0x1c71d8)));
    }

    #[test]
    fn a_legible_color_is_left_alone() {
        let blue = hex(0x1c71d8);
        assert_eq!(ensure(blue, WHITE, MIN_RATIO), blue);
        assert_eq!(ensure(WHITE, BLACK, HIGH_RATIO), WHITE);
    }

    #[test]
    fn an_illegible_color_is_pushed_just_far_enough() {
        // Pairs a chat actually produces: mIRC white and yellow on a light
        // theme, mIRC black on a dark one, the default nick hues on Adwaita
        // light and dark, and the muted gray on both.
        let light = WHITE;
        let dark = hex(0x1e1e1e);
        for (fg, bg) in [
            (hex(0xffffff), light),
            (hex(0xffff00), light),
            (hex(0xe5a50a), light),
            (hex(0x2ec27e), light),
            (hex(0x000000), dark),
            (hex(0xe5a50a), dark),
            (hex(0x2ec27e), dark),
            (hex(0xc01c28), dark),
            (hex(0x1c71d8), dark),
            (hex(0x808080), light),
            (hex(0x808080), dark),
        ] {
            for min in [MIN_RATIO, HIGH_RATIO] {
                let out = ensure(fg, bg, min);
                let r = ratio(out, bg);
                assert!(r >= min, "{fg:?} on {bg:?} reached only {r} of {min}");
                if ratio(fg, bg) < min {
                    assert!(
                        r < min * 1.05,
                        "{fg:?} on {bg:?} moved further than needed: {r}"
                    );
                } else {
                    assert_eq!(out, fg, "{fg:?} on {bg:?} was legible already");
                }
            }
        }
    }

    #[test]
    fn darkening_keeps_the_hue() {
        let out = ensure(hex(0xe5a50a), WHITE, MIN_RATIO);
        let (a, b) = (hex(0xe5a50a).map(to_linear), out.map(to_linear));
        // Scaled in linear light: the channels keep their proportions.
        assert!((a[1] / a[0] - b[1] / b[0]).abs() < 0.01);
        assert!(out[0] < 0.9, "pushed darker");
    }

    #[test]
    fn the_extremes_stay_put_when_nothing_further_reaches() {
        // Neither reaches 7:1 on these, and each is already as far as its
        // side goes: the arithmetic must not divide by zero getting there.
        let gray = hex(0x6a6a6a);
        assert_eq!(ensure(WHITE, gray, HIGH_RATIO), WHITE);
        let light = hex(0x9a9a9a);
        assert_eq!(ensure(BLACK, light, HIGH_RATIO), BLACK);
    }

    #[test]
    fn a_background_no_color_can_reach_gets_the_best_there_is() {
        // Nothing reaches 7:1 on mid gray; the far end of the better
        // side is as close as it gets.
        let mid = hex(0x777777);
        let out = ensure(hex(0x7a7a7a), mid, HIGH_RATIO);
        assert!(ratio(out, mid) >= ratio(WHITE, mid).max(ratio(BLACK, mid)) - 0.01);
    }
}
