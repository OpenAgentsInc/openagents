//! Coder Noir roles for the surfaces that keep the Coder Noir design: `/demo`,
//! the full-screen canvas pages, and the `/components` catalog. Pages in the
//! Coder Light / Coder Noir design language take their tokens from
//! `openagents-ui` instead.
//!
//! `Full`, `ThreeQuarters`, and `Half` carry text and meet WCAG AA (4.5:1)
//! on [`BACKGROUND`]. `Quarter` draws rules, borders, and the faintest
//! decoration only, never text a reader must read.

use coder_ui::coder_noir as noir;

/// One step of the white ladder, faintest to brightest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Intensity {
    /// 25% — rules, borders, receded decoration.
    Quarter,
    /// 50% — secondary text: labels, dates, hints.
    Half,
    /// 75% — body text.
    ThreeQuarters,
    /// 100% — headings, links, the prompt.
    Full,
}

impl Intensity {
    /// The ladder, faintest to brightest.
    pub const ALL: [Intensity; 4] = [
        Intensity::Quarter,
        Intensity::Half,
        Intensity::ThreeQuarters,
        Intensity::Full,
    ];

    /// This step's gray, as a packed RGB value. Every step is a pure gray.
    #[must_use]
    pub const fn color(self) -> u32 {
        match self {
            Intensity::Quarter => noir::CONTENT_TERTIARY,
            Intensity::Half => noir::CONTENT_SECONDARY,
            Intensity::ThreeQuarters => noir::CONTENT,
            Intensity::Full => noir::ANSI[15],
        }
    }
}

/// The near-black field every step sits on.
pub const BACKGROUND: u32 = noir::CANVAS;

/// The field a hovered or selected row brightens to.
pub const TINT: u32 = noir::SURFACE_RAISED;

/// The `:root` block the stylesheet opens with: the native theme's
/// `--noir-*` variables, so the CSS and the native palette cannot disagree.
#[must_use]
pub fn root_block() -> String {
    noir::css_variables()
}

/// Prefix application styles with the native theme's exact semantic tokens.
#[must_use]
pub fn stylesheet(rules: &str) -> String {
    format!("{}{rules}", root_block())
}

/// WCAG relative luminance of a packed RGB value.
#[must_use]
pub fn luminance(rgb: u32) -> f64 {
    let channel = |shift: u32| {
        let value = f64::from((rgb >> shift) & 0xff) / 255.0;
        if value <= 0.039_28 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

/// WCAG contrast ratio between two packed RGB values.
#[must_use]
pub fn contrast(a: u32, b: u32) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    let (light, dark) = if a > b { (a, b) } else { (b, a) };
    (light + 0.05) / (dark + 0.05)
}

/// Whether a packed RGB value is a pure gray: no hue at all.
#[must_use]
pub const fn is_gray(rgb: u32) -> bool {
    let (r, g, b) = ((rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff);
    r == g && g == b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ladder_is_four_grays_ordered_by_brightness() {
        let mut last = luminance(BACKGROUND);
        for step in Intensity::ALL {
            assert!(is_gray(step.color()), "{step:?}");
            let now = luminance(step.color());
            assert!(now > last, "{step:?} is brighter than the step below");
            last = now;
        }
        assert!(is_gray(BACKGROUND) && is_gray(TINT));
    }

    /// Every step that carries text reads at AA on the field and on the
    /// hover tint; the quarter step is decoration and stays visible.
    #[test]
    fn text_steps_meet_aa_contrast() {
        for step in [Intensity::Half, Intensity::ThreeQuarters, Intensity::Full] {
            for field in [BACKGROUND, TINT] {
                let ratio = contrast(step.color(), field);
                assert!(ratio >= 4.5, "{step:?} on {field:06x}: {ratio:.2}");
            }
        }
        assert!(contrast(Intensity::Quarter.color(), BACKGROUND) >= 1.8);
    }

    #[test]
    fn the_root_block_is_the_native_palette() {
        assert_eq!(root_block(), noir::css_variables());
        assert!(root_block().contains("--noir-canvas:"));
    }
}
