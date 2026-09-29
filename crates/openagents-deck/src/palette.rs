//! The deck's own colors: white, and gradations of white, on black.
//!
//! The deck keeps the four-step [`Intensity`] ladder its layouts are
//! written against, but paints it in white instead of `coder-ui`'s amber,
//! so other apps keep the shared theme. Full white marks the one thing a
//! slide says; prose draws at three quarters, labels at half, and rules at
//! a quarter.

use coder_ui::theme::Intensity;

/// The field every slide sits on.
pub const FIELD: u32 = 0x000000;

/// The color a cell at `intensity` paints.
pub const fn color(intensity: Intensity) -> u32 {
    match intensity {
        Intensity::Full => 0xffffff,
        Intensity::ThreeQuarters => 0xbfbfbf,
        Intensity::Half => 0x808080,
        Intensity::Quarter => 0x404040,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every step is a gray, and the ladder brightens from quarter to full.
    #[test]
    fn the_ladder_is_white_and_brightens() {
        let mut last = FIELD;
        for intensity in Intensity::ALL {
            let value = color(intensity);
            let [r, g, b] = [(value >> 16) & 0xff, (value >> 8) & 0xff, value & 0xff];
            assert!(r == g && g == b, "{intensity:?} is not a gray: {value:06x}");
            assert!(
                value > last,
                "{intensity:?} is not brighter than the step under it"
            );
            last = value;
        }
        assert_eq!(color(Intensity::Full), 0xffffff);
    }
}
