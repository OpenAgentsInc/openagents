//! The existing OpenAgents amber theme, shared without a renderer dependency.
//!
//! The terminal speaks in one hue: amber over a near-black field. Every
//! distinction the interface draws — prompt against draft, status rail
//! against rule, a dimmed older turn — is a difference in *how bright* the
//! same amber burns, never a second color.

use serde::{Deserialize, Serialize};

/// One step of the amber ladder.
///
/// The order is the ladder: `Quarter` is the faintest tone, `Full` the
/// brightest. `PartialOrd` and `Ord` order by brightness.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Intensity {
    /// 25% amber — receded: the oldest scrollback, a disabled affordance.
    Quarter,
    /// 50% amber — quiet: comments, rules, secondary rails.
    Half,
    /// 75% amber — present: prose, strings, focused text.
    ThreeQuarters,
    /// 100% amber — loud: the prompt, the caret, a keyword, an error.
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

    /// The amber of this step, as a packed RGB value.
    pub const fn color(self) -> u32 {
        match self {
            Intensity::Quarter => 0x463100,
            Intensity::Half => 0x835b00,
            Intensity::ThreeQuarters => 0xc18600,
            Intensity::Full => 0xffb000,
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

/// The near-black field every amber tone sits on.
pub const NEAR_BLACK: u32 = 0x080600;
/// The near-black tint a selected cell brightens to.
pub const NEAR_BLACK_TINT: u32 = 0x211700;

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
    fn each_step_owns_its_amber() {
        assert_eq!(Intensity::Full.color(), 0xffb000);
        assert_eq!(Intensity::Quarter.color(), 0x463100);
    }
}
