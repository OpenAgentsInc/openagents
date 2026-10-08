//! Coder Noir’s neutral intensity ladder, shared by application surfaces.

use crate::coder_noir;
use serde::{Deserialize, Serialize};

/// One step of the white ladder.
///
/// The order is the ladder: `Quarter` is the faintest tone, `Full` the
/// brightest. `PartialOrd` and `Ord` order by brightness.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Intensity {
    /// Receded: the oldest scrollback, a disabled affordance.
    Quarter,
    /// Quiet: comments, rules, secondary rails.
    Half,
    /// Present: prose, strings, focused text.
    ThreeQuarters,
    /// Bright: the prompt, the caret, a keyword, an error.
    #[default]
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

    /// The white of this step, as a packed RGB value.
    pub const fn color(self) -> u32 {
        match self {
            Intensity::Quarter => coder_noir::CONTENT_TERTIARY,
            Intensity::Half => coder_noir::CONTENT_SECONDARY,
            Intensity::ThreeQuarters => coder_noir::CONTENT,
            Intensity::Full => coder_noir::ANSI[15],
        }
    }

    /// The digit this step prints as in intensity glyphs, `'1'` to `'4'`.
    pub const fn digit(self) -> char {
        match self {
            Intensity::Quarter => '1',
            Intensity::Half => '2',
            Intensity::ThreeQuarters => '3',
            Intensity::Full => '4',
        }
    }

    /// The unadorned CSS class name, `quarter` through `full`.
    pub const fn class(self) -> &'static str {
        match self {
            Intensity::Quarter => "quarter",
            Intensity::Half => "half",
            Intensity::ThreeQuarters => "three",
            Intensity::Full => "full",
        }
    }
}

/// The near-black field every white tone sits on.
pub const NEAR_BLACK: u32 = coder_noir::CANVAS;
/// The near-black tint a selected cell brightens to.
pub const NEAR_BLACK_TINT: u32 = coder_noir::SURFACE_RAISED;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ladder_orders_by_brightness() {
        assert!(Intensity::Quarter < Intensity::Half);
        assert!(Intensity::Half < Intensity::ThreeQuarters);
        assert!(Intensity::ThreeQuarters < Intensity::Full);
    }

    #[test]
    fn the_ladder_has_four_steps() {
        assert_eq!(Intensity::ALL.len(), 4);
        assert_eq!(Intensity::ALL[0], Intensity::Quarter);
        assert_eq!(Intensity::ALL[3], Intensity::Full);
    }

    #[test]
    fn each_step_owns_its_white() {
        assert_eq!(Intensity::Full.color(), coder_noir::ANSI[15]);
        assert_eq!(Intensity::Quarter.color(), coder_noir::CONTENT_TERTIARY);
    }
}
