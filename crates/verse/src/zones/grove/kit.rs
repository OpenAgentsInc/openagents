//! The druid's kit in the Grove: the spells on its hotbar with their SRD
//! dice and saves.
//!
//! The numbers follow the combat model (`docs/verse/combat-model.md`): the
//! simulation rolls each spell's SRD dice, attack roll, and saving throw
//! behind the scenes with `verse_world`'s seeded dice, and the player sees
//! the outcome. The Grove is a demo to spam spells in, so nothing gates a
//! cast: no mana, no global cooldown, and no per-spell cooldown. A held key
//! recasts every [`REPEAT`] seconds. Long Rest, the demo control, stands
//! the field back up.

use super::super::Intent;

/// How often a held hotbar key recasts, s: six casts a second.
pub const REPEAT: f32 = 1.0 / 6.0;
/// The level 20 druid's spell attack bonus: proficiency 6 and Wisdom 5.
pub const ATTACK_BONUS: i32 = 11;
/// The Giant Spider's attack bonus, which a druid in its shape uses.
pub const SPIDER_ATTACK_BONUS: i32 = 5;
/// The druid's spell save DC: 8, proficiency 6, and Wisdom 5.
pub const SAVE_DC: i32 = 19;
/// The seed of the Grove's dice, so a session replays exactly.
pub const DICE_SEED: u64 = 0x6720_5EED;

/// One foot in meters, at the combat model's 5 ft = 1.5 m.
const FT: f32 = 0.3;

/// A spell or control on the Grove's hotbar, in hotbar order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spell {
    Thunderwave,
    GustOfWind,
    WindWall,
    WallOfStone,
    ReverseGravity,
    FireBolt,
    Fireball,
    MistyStep,
    Web,
    /// Wild Shape into the Giant Spider ([`super::shape`]).
    WildShapeSpider,
    /// Drop the beast's shape.
    ReturnToForm,
    LongRest,
    /// The Giant Spider's bite, on the bar while the druid wears its shape.
    Bite,
    /// The Giant Spider's web, likewise.
    SpiderWeb,
}

/// A damage type, for resistances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Damage {
    Fire,
    Thunder,
    Bludgeoning,
    Piercing,
    Poison,
}

impl Damage {
    /// The word a combat line uses.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Fire => "fire",
            Self::Thunder => "thunder",
            Self::Bludgeoning => "bludgeoning",
            Self::Piercing => "piercing",
            Self::Poison => "poison",
        }
    }

    /// The color its numbers float in, linear RGB.
    #[must_use]
    pub const fn color(self) -> [f32; 3] {
        match self {
            Self::Fire => [1.0, 0.45, 0.12],
            Self::Thunder => [0.45, 0.7, 1.0],
            Self::Bludgeoning => [0.9, 0.8, 0.55],
            Self::Piercing => [0.92, 0.9, 0.86],
            Self::Poison => [0.55, 1.0, 0.3],
        }
    }
}

/// An ability a saving throw uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ability {
    Strength,
    Dexterity,
    Constitution,
}

impl Ability {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Strength => "Strength",
            Self::Dexterity => "Dexterity",
            Self::Constitution => "Constitution",
        }
    }
}

/// How a spell lands on a dummy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// An attack roll with `bonus` against armor class; a natural 20
    /// doubles the dice.
    Attack { bonus: i32 },
    /// A saving throw of `ability` against [`SAVE_DC`]. A success takes
    /// half damage when `half` is set, and none otherwise, and avoids any
    /// push, lift, or root either way.
    Save { ability: Ability, half: bool },
    /// It simply happens.
    Automatic,
}

/// A spell's numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Def {
    pub label: &'static str,
    /// The SRD spell level; 0 is a cantrip.
    pub level: u8,
    /// Range to the target or the effect's center, m.
    pub range: f32,
    /// Damage dice, count and sides; `(0, 0)` deals no damage.
    pub dice: (u32, u32),
    /// Damage added to the dice, not doubled on a critical hit.
    pub bonus: i32,
    pub kind: Damage,
    /// A second damage of another type that lands with the first, its
    /// dice and type, such as a bite's poison.
    pub extra: Option<(u32, u32, Damage)>,
    pub delivery: Delivery,
}

impl Spell {
    /// Every spell, in hotbar order; the beast's attacks come last because
    /// they take another slot's place while the druid wears its shape
    /// ([`super::hotbar`]).
    pub const ALL: [Self; 14] = [
        Self::Thunderwave,
        Self::GustOfWind,
        Self::WindWall,
        Self::WallOfStone,
        Self::ReverseGravity,
        Self::FireBolt,
        Self::Fireball,
        Self::MistyStep,
        Self::Web,
        Self::WildShapeSpider,
        Self::ReturnToForm,
        Self::LongRest,
        Self::Bite,
        Self::SpiderWeb,
    ];

    /// The spell's numbers: the SRD's dice and saves at level 20, and
    /// ranges and areas at 5 ft = 1.5 m.
    #[must_use]
    pub const fn def(self) -> Def {
        use Ability::{Constitution, Dexterity, Strength};
        use Delivery::Automatic;
        const fn save(ability: Ability, half: bool) -> Delivery {
            Delivery::Save { ability, half }
        }
        const SPELL: Delivery = Delivery::Attack {
            bonus: ATTACK_BONUS,
        };
        const SPIDER: Delivery = Delivery::Attack {
            bonus: SPIDER_ATTACK_BONUS,
        };
        let mut bonus = 0;
        let mut extra = None;
        let (label, level, range, dice, kind, delivery) = match self {
            // A 15-foot cube from the caster: 2d8 thunder and a 10-foot
            // push; a Constitution save halves it and holds ground.
            Self::Thunderwave => (
                "Thunderwave",
                1,
                6.0,
                (2, 8),
                Damage::Thunder,
                save(Constitution, true),
            ),
            // A 60-foot line; a failed Strength save is pushed 15 feet.
            Self::GustOfWind => (
                "Gust of Wind",
                2,
                18.0,
                (0, 0),
                Damage::Bludgeoning,
                save(Strength, false),
            ),
            // The wall rises through the target: 4d8 bludgeoning, half on a
            // Strength save, and a lift on a failure.
            Self::WindWall => (
                "Wind Wall",
                3,
                36.0,
                (4, 8),
                Damage::Bludgeoning,
                save(Strength, true),
            ),
            // Panels rise at the target and shove it to one side.
            Self::WallOfStone => (
                "Wall of Stone",
                5,
                36.0,
                (0, 0),
                Damage::Bludgeoning,
                Automatic,
            ),
            // A 50-foot-radius cylinder on the caster; everything falls up.
            Self::ReverseGravity => (
                "Reverse Gravity",
                7,
                0.0,
                (0, 0),
                Damage::Bludgeoning,
                Automatic,
            ),
            // A 120-foot bolt: an attack roll for 4d10 fire at level 17+.
            Self::FireBolt => ("Fire Bolt", 0, 120.0 * FT, (4, 10), Damage::Fire, SPELL),
            // A 150-foot throw: 8d6 fire in a 20-foot radius, half on a
            // Dexterity save.
            Self::Fireball => (
                "Fireball",
                3,
                150.0 * FT,
                (8, 6),
                Damage::Fire,
                save(Dexterity, true),
            ),
            // A 30-foot blink toward the target.
            Self::MistyStep => ("Misty Step", 2, 30.0 * FT, (0, 0), Damage::Fire, Automatic),
            // A 20-foot cube at the target; a failed Dexterity save roots.
            Self::Web => (
                "Web",
                2,
                60.0 * FT,
                (0, 0),
                Damage::Fire,
                save(Dexterity, false),
            ),
            Self::WildShapeSpider => (
                "Wild Shape: Giant Spider",
                0,
                0.0,
                (0, 0),
                Damage::Piercing,
                Automatic,
            ),
            Self::ReturnToForm => ("Return to Form", 0, 0.0, (0, 0), Damage::Fire, Automatic),
            Self::LongRest => ("Long Rest", 0, 0.0, (0, 0), Damage::Fire, Automatic),
            // The Giant Spider's bite: +5 to hit, reach 5 feet (a melee
            // reach at the spider's size), 1d8 + 3 piercing and 2d6 poison.
            Self::Bite => {
                bonus = 3;
                extra = Some((2, 6, Damage::Poison));
                ("Bite", 0, 3.5, (1, 8), Damage::Piercing, SPIDER)
            }
            // The Giant Spider's web: +5 to hit at 60 feet; a hit restrains.
            Self::SpiderWeb => ("Web", 0, 60.0 * FT, (0, 0), Damage::Piercing, SPIDER),
        };
        Def {
            label,
            level,
            range,
            dice,
            bonus,
            kind,
            extra,
            delivery,
        }
    }

    /// The named zone intent that casts it, for the spells that have one.
    /// The hotbar sends its slot instead ([`Intent::GroveSlot`]).
    #[must_use]
    pub const fn intent(self) -> Option<Intent> {
        Some(match self {
            Self::Thunderwave => Intent::Thunderwave,
            Self::GustOfWind => Intent::GustOfWind,
            Self::WindWall => Intent::WindWall,
            Self::WallOfStone => Intent::WallOfStone,
            Self::ReverseGravity => Intent::ReverseGravity,
            Self::FireBolt => Intent::Firebolt,
            Self::Fireball => Intent::Fireball,
            Self::MistyStep => Intent::MistyStep,
            Self::Web => Intent::Web,
            Self::LongRest => Intent::LongRest,
            _ => return None,
        })
    }

    /// The spell a named `intent` casts in the Grove, if any.
    #[must_use]
    pub fn of(intent: Intent) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.intent() == Some(intent))
    }

    /// Everglade's spell behind it, for the three the glade already casts.
    #[must_use]
    pub const fn glade(self) -> Option<crate::zones::everglade::spells::Spell> {
        use crate::zones::everglade::spells::Spell as Glade;
        match self {
            Self::WindWall => Some(Glade::WindWall),
            Self::WallOfStone => Some(Glade::WallOfStone),
            Self::ReverseGravity => Some(Glade::ReverseGravity),
            _ => None,
        }
    }

    /// Whether the spell needs a dummy in front of the druid.
    #[must_use]
    pub const fn needs_target(self) -> bool {
        matches!(
            self,
            Self::FireBolt | Self::Fireball | Self::Web | Self::Bite | Self::SpiderWeb
        )
    }

    /// Whether it is a beast's own attack, which only its shape can use.
    #[must_use]
    pub const fn beast(self) -> bool {
        matches!(self, Self::Bite | Self::SpiderWeb)
    }

    /// Whether the druid casts it as a druid: not a form change, a demo
    /// control, or a beast's attack.
    #[must_use]
    pub const fn spell(self) -> bool {
        !self.beast()
            && !matches!(
                self,
                Self::WildShapeSpider | Self::ReturnToForm | Self::LongRest
            )
    }
}
