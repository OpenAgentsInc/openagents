//! Wild Shape in the Grove: the beasts the druid becomes.
//!
//! A form swaps the player's character for one of the pack's forms
//! (`beasts/`), posed where the player stands with its own clips: idle and
//! its walk by speed, and its attack when it strikes. It changes the
//! player's pace and puts the beast's attacks on row 2 of the bar
//! ([`super::slots`]); the druid's spells still cast in it, as Beast Spells
//! allows at level 18. The eagle flies, with Everglade's levitation. Return
//! to Form and Long Rest end it.
//!
//! The spider is Quaternius's, from the Easy Animated Enemy Pack; the bear,
//! wolf, and eagle are the stylized animals `scripts/blender/animals.py`
//! builds, until a CC0 pack has them.

use super::kit::Spell;
use crate::zones::everglade::player::Beast;
use crate::zones::everglade_pack::ZonePack;

/// A beast the druid can become.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    BrownBear,
    DireWolf,
    GiantEagle,
    GiantSpider,
}

impl Form {
    /// Every form, in figure order.
    pub const ALL: [Self; 4] = [
        Self::BrownBear,
        Self::DireWolf,
        Self::GiantEagle,
        Self::GiantSpider,
    ];

    /// The form's name in the pack.
    #[must_use]
    pub const fn pack_name(self) -> &'static str {
        match self {
            Self::BrownBear => "beasts/bear",
            Self::DireWolf => "beasts/wolf",
            Self::GiantEagle => "beasts/eagle",
            Self::GiantSpider => "beasts/giant_spider",
        }
    }

    /// The name a combat line uses.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::BrownBear => "Brown Bear",
            Self::DireWolf => "Dire Wolf",
            Self::GiantEagle => "Giant Eagle",
            Self::GiantSpider => "Giant Spider",
        }
    }

    /// How many times its modeled size it stands. Each is Large, filling a
    /// 10-foot (3 m) square: the pack's bear is 2.1 m long, its wolf
    /// 1.6 m, its eagle 1.1 m across the wings, and its spider 2 m across
    /// the legs.
    #[must_use]
    pub const fn scale(self) -> f32 {
        match self {
            Self::BrownBear => 1.3,
            Self::DireWolf => 1.6,
            Self::GiantEagle => 2.6,
            Self::GiantSpider => 1.5,
        }
    }

    /// The multiplier on the player's movement speeds: the bear is slow,
    /// the wolf fast, the eagle swift in the air, and the spider scuttles a
    /// quarter faster than the druid runs.
    #[must_use]
    pub const fn pace(self) -> f32 {
        match self {
            Self::BrownBear => 0.85,
            Self::DireWolf => 1.5,
            Self::GiantEagle => 1.4,
            Self::GiantSpider => 1.25,
        }
    }

    /// Whether it flies.
    #[must_use]
    pub const fn flies(self) -> bool {
        matches!(self, Self::GiantEagle)
    }

    /// The spell that takes this form.
    #[must_use]
    pub const fn spell(self) -> Spell {
        match self {
            Self::BrownBear => Spell::WildShapeBear,
            Self::DireWolf => Spell::WildShapeWolf,
            Self::GiantEagle => Spell::WildShapeEagle,
            Self::GiantSpider => Spell::WildShapeSpider,
        }
    }

    /// The form a spell takes, if it is a Wild Shape.
    #[must_use]
    pub fn of(spell: Spell) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.spell() == spell)
    }

    /// The beast's attacks, which take row 2's first slots in its shape.
    #[must_use]
    pub const fn attacks(self) -> &'static [Spell] {
        match self {
            Self::BrownBear => &[Spell::BearBite, Spell::BearClaw],
            Self::DireWolf => &[Spell::WolfBite],
            Self::GiantEagle => &[Spell::EagleTalons],
            Self::GiantSpider => &[Spell::SpiderBite, Spell::SpiderWeb],
        }
    }

    pub(super) fn index(self) -> usize {
        self as usize
    }
}

/// Each form the pack carries, ready to pose, in [`Form::ALL`] order; a
/// form the pack lacks is `None` and can't be taken.
///
/// # Errors
///
/// Returns a message when a carried form can't play.
pub(crate) fn beasts(pack: &ZonePack) -> Result<Vec<Option<Beast>>, String> {
    Form::ALL
        .into_iter()
        .map(|form| {
            pack.form(form.pack_name())
                .map(|c| Beast::new(pack, c))
                .transpose()
        })
        .collect()
}

/// The shape the druid wears now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub form: Form,
    /// When its latest attack began, for the attack clip.
    pub attack: Option<f32>,
}
