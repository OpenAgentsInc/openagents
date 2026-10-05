//! Wild Shape in the Grove: the beasts the druid becomes.
//!
//! A form swaps the player's character for one of the pack's forms
//! (`beasts/`), posed where the player stands with its own clips: idle and
//! its walk by speed, and its attack when it bites. It changes the
//! player's pace and puts the beast's attacks on the hotbar
//! ([`super::hotbar`]); the druid's spells still cast in it, as Beast
//! Spells allows at level 18. Return to Form and Long Rest end it.

use super::kit::Spell;
use crate::zones::everglade::player::Beast;
use crate::zones::everglade_pack::ZonePack;

/// A beast the druid can become.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    GiantSpider,
}

impl Form {
    /// Every form, in figure order.
    pub const ALL: [Self; 1] = [Self::GiantSpider];

    /// The form's name in the pack.
    #[must_use]
    pub const fn pack_name(self) -> &'static str {
        match self {
            Self::GiantSpider => "beasts/giant_spider",
        }
    }

    /// The name a combat line uses.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::GiantSpider => "Giant Spider",
        }
    }

    /// How many times its modeled size it stands. The pack's spider is
    /// 2 m across the legs; a Large creature fills a 10-foot (3 m) square.
    #[must_use]
    pub const fn scale(self) -> f32 {
        match self {
            Self::GiantSpider => 1.5,
        }
    }

    /// The multiplier on the player's movement speeds: the spider
    /// scuttles a quarter faster than the druid runs.
    #[must_use]
    pub const fn pace(self) -> f32 {
        match self {
            Self::GiantSpider => 1.25,
        }
    }

    /// The spell that takes this form.
    #[must_use]
    pub const fn spell(self) -> Spell {
        match self {
            Self::GiantSpider => Spell::WildShapeSpider,
        }
    }

    /// The form a spell takes, if it is a Wild Shape.
    #[must_use]
    pub fn of(spell: Spell) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.spell() == spell)
    }

    /// The beast's attacks, which take the bar's first slots in its shape.
    #[must_use]
    pub const fn attacks(self) -> [Spell; 2] {
        match self {
            Self::GiantSpider => [Spell::Bite, Spell::SpiderWeb],
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
    /// When the druid took it, s on the Grove's clock.
    pub since: f32,
    /// When its latest attack began, for the attack clip.
    pub attack: Option<f32>,
}
