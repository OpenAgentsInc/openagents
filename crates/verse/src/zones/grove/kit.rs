//! The druid's kit in the Grove: every ability on the Archdruid's four-row
//! bar with its SRD 5.2.1 dice, saves, and area.
//!
//! The numbers follow the combat model (`docs/verse/combat-model.md`): the
//! simulation rolls each spell's SRD dice, attack roll, and saving throw
//! behind the scenes with `verse_world`'s seeded dice, and the player sees
//! the outcome. Ranges and areas convert at 5 ft = 1.5 m, and a spell that
//! acts each round acts each second here, as the demo's table says. The
//! Grove is a demo to spam spells in, so nothing gates a cast: no mana, no
//! global cooldown, and no per-spell cooldown. A held key recasts every
//! [`REPEAT`] seconds. Long Rest, the demo control, stands the field back
//! up.

use super::super::Intent;
use super::super::everglade::demolition::meteor;
use super::dummies::Condition;

/// How often a held hotbar key recasts, s: six casts a second.
pub const REPEAT: f32 = 1.0 / 6.0;
/// The level 20 druid's spell attack bonus: proficiency 6 and Wisdom 5.
pub const ATTACK_BONUS: i32 = 11;
/// The beasts' attack bonus, which a druid in their shape uses.
pub const BEAST_ATTACK_BONUS: i32 = 5;
/// The dragon's attack bonus, an adult red dragon's from the SRD, which
/// the druid uses in Shapechange's shape.
pub const DRAGON_ATTACK_BONUS: i32 = 14;
/// How far the dragon's bite reaches from where it stands, m: its neck
/// carries its jaws about 5 m ahead.
pub const DRAGON_REACH: f32 = 7.5;
/// The druid's spell save DC: 8, proficiency 6, and Wisdom 5.
pub const SAVE_DC: i32 = 19;
/// Wisdom's modifier, which Potent Spellcasting adds to cantrip damage.
pub const WISDOM: i32 = 5;
/// The seed of the Grove's dice, so a session replays exactly.
pub const DICE_SEED: u64 = 0x6720_5EED;

/// One foot in meters, at the combat model's 5 ft = 1.5 m.
pub const FT: f32 = 0.3;

/// A spell, a Wild Shape ability, or a demo control on the Grove's bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Spell {
    // Row 1: Wild Shape and druid features.
    WildShapeBear,
    WildShapeWolf,
    WildShapeEagle,
    WildShapeSpider,
    ReturnToForm,
    WildCompanion,
    LandsAid,
    NaturesSanctuary,
    ChooseLand,
    NatureMagician,
    WildResurgence,
    LongRest,
    // Row 2: cantrips and levels 1 to 2.
    ProduceFlame,
    StarryWisp,
    Shillelagh,
    PoisonSpray,
    Elementalism,
    Thunderwave,
    Entangle,
    FaerieFire,
    IceKnife,
    HealingWord,
    Moonbeam,
    GustOfWind,
    // Row 3: levels 2 to 7.
    SpikeGrowth,
    CallLightning,
    ConjureAnimals,
    WindWall,
    IceStorm,
    WallOfFire,
    Polymorph,
    MassCureWounds,
    Sunbeam,
    WallOfThorns,
    FireStorm,
    ReverseGravity,
    // Row 4: levels 8 and 9, and Speak with Animals.
    Sunburst,
    StormOfVengeance,
    Shapechange,
    SpeakWithAnimals,
    // Row 4's last keys: the spells that break buildings, aimed at a wall
    // or the ground with the cursor.
    MeteorSwarm,
    Thunderbolt,
    // The Circle of the Land's spells.
    FireBolt,
    BurningHands,
    Blur,
    Fireball,
    Blight,
    WallOfStone,
    RayOfFrost,
    FogCloud,
    HoldPerson,
    SleetStorm,
    ConeOfCold,
    ShockingGrasp,
    Sleep,
    MistyStep,
    LightningBolt,
    FreedomOfMovement,
    TreeStride,
    AcidSplash,
    RayOfSickness,
    Web,
    StinkingCloud,
    InsectPlague,
    // The beasts' attacks, which take row 2's first slots in their shape.
    BearBite,
    BearClaw,
    WolfBite,
    EagleTalons,
    SpiderBite,
    SpiderWeb,
    // The dragon's, which take row 2's first slots in Shapechange's shape.
    DragonBite,
    FireBreath,
    TailSweep,
    WingBuffet,
    Roar,
}

/// A damage type, for resistances and the numbers' colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Damage {
    Fire,
    Thunder,
    Bludgeoning,
    Piercing,
    Slashing,
    Poison,
    Cold,
    Radiant,
    Lightning,
    Necrotic,
    Acid,
    Force,
    /// Hit points restored rather than lost.
    Healing,
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
            Self::Slashing => "slashing",
            Self::Poison => "poison",
            Self::Cold => "cold",
            Self::Radiant => "radiant",
            Self::Lightning => "lightning",
            Self::Necrotic => "necrotic",
            Self::Acid => "acid",
            Self::Force => "force",
            Self::Healing => "healing",
        }
    }

    /// The color its numbers float in, linear RGB.
    #[must_use]
    pub const fn color(self) -> [f32; 3] {
        match self {
            Self::Fire => [1.0, 0.45, 0.12],
            Self::Thunder => [0.45, 0.7, 1.0],
            Self::Bludgeoning => [0.9, 0.8, 0.55],
            Self::Piercing | Self::Slashing => [0.92, 0.9, 0.86],
            Self::Poison => [0.55, 1.0, 0.3],
            Self::Cold => [0.6, 0.9, 1.0],
            Self::Radiant => [1.0, 0.92, 0.5],
            Self::Lightning => [0.7, 0.8, 1.0],
            Self::Necrotic => [0.7, 0.45, 0.95],
            Self::Acid => [0.75, 1.0, 0.2],
            Self::Force => [0.85, 0.6, 1.0],
            Self::Healing => [0.35, 1.0, 0.45],
        }
    }
}

/// An ability a saving throw uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ability {
    Strength,
    Dexterity,
    Constitution,
    Wisdom,
}

impl Ability {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Strength => "Strength",
            Self::Dexterity => "Dexterity",
            Self::Constitution => "Constitution",
            Self::Wisdom => "Wisdom",
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
    /// push, lift, root, or condition either way.
    Save { ability: Ability, half: bool },
    /// It simply happens.
    Automatic,
}

/// Where a spell lands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Area {
    /// A projectile flies to the target and lands after its flight.
    Bolt,
    /// The target alone, at once.
    Single,
    /// A sphere of this radius, m, at the target, or ahead with none.
    Burst(f32),
    /// A cone this long, m, from the druid toward the target.
    Cone(f32),
    /// A line this long and this wide, m, from the druid.
    Line(f32, f32),
    /// A lasting area of this radius, m, at the target ([`super::aura`]).
    Zone(f32),
    /// A lasting wall this long, m, across the druid's facing at the
    /// target ([`super::aura`]).
    Wall(f32),
    /// The druid, or no one: a change of shape, a buff, or a control.
    Caster,
}

/// A spell's numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Def {
    pub label: &'static str,
    /// The SRD spell level; 0 is a cantrip or a feature.
    pub level: u8,
    /// Range to the target or the effect's center, m.
    pub range: f32,
    /// Damage dice, count and sides; `(0, 0)` deals no damage.
    pub dice: (u32, u32),
    /// Damage added to the dice, not doubled on a critical hit.
    pub bonus: i32,
    pub kind: Damage,
    /// A second damage of another type that lands with the first, its
    /// dice and type, such as a bite's poison or Ice Storm's cold.
    pub extra: Option<(u32, u32, Damage)>,
    pub delivery: Delivery,
    pub area: Area,
    /// The condition a landing that no save halved leaves, and for how
    /// long, s.
    pub rider: Option<(Condition, f32)>,
    /// Whether it is a maintained effect: casting another ends it.
    pub concentration: bool,
}

/// The four lands of the Circle of the Land.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Land {
    Arid,
    Polar,
    Temperate,
    Tropical,
}

impl Land {
    /// Every land, in Choose Land's order.
    pub const ALL: [Self; 4] = [Self::Arid, Self::Polar, Self::Temperate, Self::Tropical];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Arid => "Arid",
            Self::Polar => "Polar",
            Self::Temperate => "Temperate",
            Self::Tropical => "Tropical",
        }
    }

    /// The land's six spells on row 4, keys 5 to 0.
    #[must_use]
    pub const fn spells(self) -> [Spell; 6] {
        use Spell as S;
        match self {
            Self::Arid => [
                S::FireBolt,
                S::BurningHands,
                S::Blur,
                S::Fireball,
                S::Blight,
                S::WallOfStone,
            ],
            Self::Polar => [
                S::RayOfFrost,
                S::FogCloud,
                S::HoldPerson,
                S::SleetStorm,
                S::IceStorm,
                S::ConeOfCold,
            ],
            Self::Temperate => [
                S::ShockingGrasp,
                S::Sleep,
                S::MistyStep,
                S::LightningBolt,
                S::FreedomOfMovement,
                S::TreeStride,
            ],
            Self::Tropical => [
                S::AcidSplash,
                S::RayOfSickness,
                S::Web,
                S::StinkingCloud,
                S::Polymorph,
                S::InsectPlague,
            ],
        }
    }

    /// The land after this one.
    #[must_use]
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|l| *l == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

impl Spell {
    /// Every ability, in bar order, then the land spells and the beasts'
    /// attacks.
    pub const ALL: [Self; 75] = {
        use Spell as S;
        [
            S::WildShapeBear,
            S::WildShapeWolf,
            S::WildShapeEagle,
            S::WildShapeSpider,
            S::ReturnToForm,
            S::WildCompanion,
            S::LandsAid,
            S::NaturesSanctuary,
            S::ChooseLand,
            S::NatureMagician,
            S::WildResurgence,
            S::LongRest,
            S::ProduceFlame,
            S::StarryWisp,
            S::Shillelagh,
            S::PoisonSpray,
            S::Elementalism,
            S::Thunderwave,
            S::Entangle,
            S::FaerieFire,
            S::IceKnife,
            S::HealingWord,
            S::Moonbeam,
            S::GustOfWind,
            S::SpikeGrowth,
            S::CallLightning,
            S::ConjureAnimals,
            S::WindWall,
            S::IceStorm,
            S::WallOfFire,
            S::Polymorph,
            S::MassCureWounds,
            S::Sunbeam,
            S::WallOfThorns,
            S::FireStorm,
            S::ReverseGravity,
            S::Sunburst,
            S::StormOfVengeance,
            S::Shapechange,
            S::SpeakWithAnimals,
            S::MeteorSwarm,
            S::Thunderbolt,
            S::FireBolt,
            S::BurningHands,
            S::Blur,
            S::Fireball,
            S::Blight,
            S::WallOfStone,
            S::RayOfFrost,
            S::FogCloud,
            S::HoldPerson,
            S::SleetStorm,
            S::ConeOfCold,
            S::ShockingGrasp,
            S::Sleep,
            S::MistyStep,
            S::LightningBolt,
            S::FreedomOfMovement,
            S::TreeStride,
            S::AcidSplash,
            S::RayOfSickness,
            S::Web,
            S::StinkingCloud,
            S::InsectPlague,
            S::BearBite,
            S::BearClaw,
            S::WolfBite,
            S::EagleTalons,
            S::SpiderBite,
            S::SpiderWeb,
            S::DragonBite,
            S::FireBreath,
            S::TailSweep,
            S::WingBuffet,
            S::Roar,
        ]
    };

    /// The spell's numbers: the SRD's dice and saves at level 20, and
    /// ranges and areas at 5 ft = 1.5 m.
    #[must_use]
    pub const fn def(self) -> Def {
        use Ability::{Constitution as Con, Dexterity as Dex, Strength as Str, Wisdom as Wis};
        use Area::{Bolt, Burst, Caster, Cone, Line, Single, Wall, Zone};
        use Condition as C;
        use Damage as D;
        use Spell as S;
        const fn save(ability: Ability, half: bool) -> Delivery {
            Delivery::Save { ability, half }
        }
        const SPELL: Delivery = Delivery::Attack {
            bonus: ATTACK_BONUS,
        };
        const BEAST: Delivery = Delivery::Attack {
            bonus: BEAST_ATTACK_BONUS,
        };
        const AUTO: Delivery = Delivery::Automatic;
        const DRAGON: Delivery = Delivery::Attack {
            bonus: DRAGON_ATTACK_BONUS,
        };
        let mut def = Def {
            label: "",
            level: 0,
            range: 0.0,
            dice: (0, 0),
            bonus: 0,
            kind: D::Force,
            extra: None,
            delivery: AUTO,
            area: Caster,
            rider: None,
            concentration: false,
        };
        // (label, level, range m, dice, damage, delivery, area)
        let (label, level, range, dice, kind, delivery, area) = match self {
            S::WildShapeBear => (
                "Wild Shape: Brown Bear",
                0,
                0.0,
                (0, 0),
                D::Force,
                AUTO,
                Caster,
            ),
            S::WildShapeWolf => (
                "Wild Shape: Dire Wolf",
                0,
                0.0,
                (0, 0),
                D::Force,
                AUTO,
                Caster,
            ),
            S::WildShapeEagle => (
                "Wild Shape: Giant Eagle",
                0,
                0.0,
                (0, 0),
                D::Force,
                AUTO,
                Caster,
            ),
            S::WildShapeSpider => (
                "Wild Shape: Giant Spider",
                0,
                0.0,
                (0, 0),
                D::Force,
                AUTO,
                Caster,
            ),
            S::ReturnToForm => ("Return to Form", 0, 0.0, (0, 0), D::Force, AUTO, Caster),
            S::WildCompanion => ("Wild Companion", 0, 0.0, (0, 0), D::Force, AUTO, Caster),
            // A 10-foot burst within 60 feet: 4d6 necrotic, half on a
            // Constitution save.
            S::LandsAid => (
                "Land's Aid",
                0,
                60.0 * FT,
                (4, 6),
                D::Necrotic,
                save(Con, true),
                Burst(10.0 * FT),
            ),
            S::NaturesSanctuary => ("Nature's Sanctuary", 0, 0.0, (0, 0), D::Force, AUTO, Caster),
            S::ChooseLand => ("Choose Land", 0, 0.0, (0, 0), D::Force, AUTO, Caster),
            S::NatureMagician => ("Nature Magician", 0, 0.0, (0, 0), D::Force, AUTO, Caster),
            S::WildResurgence => ("Wild Resurgence", 0, 0.0, (0, 0), D::Force, AUTO, Caster),
            S::LongRest => ("Long Rest", 0, 0.0, (0, 0), D::Force, AUTO, Caster),
            // Cantrips at level 17 and up, with Wisdom added.
            S::ProduceFlame => {
                def.bonus = WISDOM;
                ("Produce Flame", 0, 60.0 * FT, (4, 8), D::Fire, SPELL, Bolt)
            }
            S::StarryWisp => {
                def.bonus = WISDOM;
                def.rider = Some((C::Starlit, 6.0));
                ("Starry Wisp", 0, 60.0 * FT, (4, 8), D::Radiant, SPELL, Bolt)
            }
            S::Shillelagh => {
                def.bonus = WISDOM;
                ("Shillelagh", 0, 3.5, (2, 6), D::Force, SPELL, Single)
            }
            S::PoisonSpray => {
                def.bonus = WISDOM;
                (
                    "Poison Spray",
                    0,
                    30.0 * FT,
                    (4, 12),
                    D::Poison,
                    SPELL,
                    Single,
                )
            }
            S::Elementalism => ("Elementalism", 0, 30.0 * FT, (0, 0), D::Force, AUTO, Single),
            // A 15-foot cube from the caster: 2d8 thunder and a 10-foot
            // push; a Constitution save halves it and holds ground.
            S::Thunderwave => (
                "Thunderwave",
                1,
                6.0,
                (2, 8),
                D::Thunder,
                save(Con, true),
                Caster,
            ),
            // A 20-foot square within 90 feet: a failed Strength save is
            // restrained, here rooted for 3 s.
            S::Entangle => {
                def.rider = Some((C::Restrained, 3.0));
                (
                    "Entangle",
                    1,
                    90.0 * FT,
                    (0, 0),
                    D::Force,
                    save(Str, false),
                    Zone(10.0 * FT),
                )
            }
            // A 20-foot cube within 60 feet: a failed Dexterity save is
            // outlined and takes a fifth more damage for 10 s.
            S::FaerieFire => {
                def.rider = Some((C::Outlined, 10.0));
                (
                    "Faerie Fire",
                    1,
                    60.0 * FT,
                    (0, 0),
                    D::Radiant,
                    save(Dex, false),
                    Burst(10.0 * FT),
                )
            }
            // The shard's attack for 1d10 piercing; its burst is 2d6 cold
            // on a Dexterity save ([`Spell::IceKnife`] in `cast`).
            S::IceKnife => ("Ice Knife", 1, 60.0 * FT, (1, 10), D::Piercing, SPELL, Bolt),
            S::HealingWord => {
                def.bonus = WISDOM;
                (
                    "Healing Word",
                    1,
                    60.0 * FT,
                    (2, 4),
                    D::Healing,
                    AUTO,
                    Single,
                )
            }
            // A 5-foot-radius column: 2d10 radiant a second, half on a
            // Constitution save.
            S::Moonbeam => {
                def.concentration = true;
                (
                    "Moonbeam",
                    2,
                    120.0 * FT,
                    (2, 10),
                    D::Radiant,
                    save(Con, true),
                    Zone(5.0 * FT),
                )
            }
            // A 60-foot line; a failed Strength save is pushed 15 feet.
            S::GustOfWind => (
                "Gust of Wind",
                2,
                18.0,
                (0, 0),
                D::Bludgeoning,
                save(Str, false),
                Caster,
            ),
            // A 20-foot radius of thorns: 2d4 piercing for every 5 feet a
            // dummy is moved through it.
            S::SpikeGrowth => {
                def.concentration = true;
                (
                    "Spike Growth",
                    2,
                    150.0 * FT,
                    (2, 4),
                    D::Piercing,
                    AUTO,
                    Zone(20.0 * FT),
                )
            }
            // A storm over a 60-foot radius: each bolt is 3d10 lightning in
            // a 5-foot radius, half on a Dexterity save, every 6 s and on
            // each recast.
            S::CallLightning => {
                def.concentration = true;
                (
                    "Call Lightning",
                    3,
                    120.0 * FT,
                    (3, 10),
                    D::Lightning,
                    save(Dex, true),
                    Zone(60.0 * FT),
                )
            }
            // The spectral pack: 3d10 slashing in a 10-foot cube on a failed
            // Dexterity save.
            S::ConjureAnimals => (
                "Conjure Animals",
                3,
                60.0 * FT,
                (3, 10),
                D::Slashing,
                save(Dex, false),
                Burst(10.0 * FT),
            ),
            // The wall rises through the target: 4d8 bludgeoning, half on a
            // Strength save, and a lift on a failure.
            S::WindWall => (
                "Wind Wall",
                3,
                36.0,
                (4, 8),
                D::Bludgeoning,
                save(Str, true),
                Caster,
            ),
            // Hail in a 20-foot-radius cylinder: 2d10 bludgeoning and 4d6
            // cold, half on a Dexterity save.
            S::IceStorm => {
                def.extra = Some((4, 6, D::Cold));
                (
                    "Ice Storm",
                    4,
                    300.0 * FT,
                    (2, 10),
                    D::Bludgeoning,
                    save(Dex, true),
                    Burst(20.0 * FT),
                )
            }
            // A 60-foot wall: 5d8 fire when it rises, half on a Dexterity
            // save, and 5d8 a second to a dummy in it.
            S::WallOfFire => {
                def.concentration = true;
                (
                    "Wall of Fire",
                    4,
                    120.0 * FT,
                    (5, 8),
                    D::Fire,
                    save(Dex, true),
                    Wall(60.0 * FT),
                )
            }
            S::Polymorph => {
                def.rider = Some((C::Polymorphed, 8.0));
                (
                    "Polymorph",
                    4,
                    60.0 * FT,
                    (0, 0),
                    D::Force,
                    save(Wis, false),
                    Single,
                )
            }
            S::MassCureWounds => {
                def.bonus = WISDOM;
                (
                    "Mass Cure Wounds",
                    5,
                    60.0 * FT,
                    (5, 8),
                    D::Healing,
                    AUTO,
                    Burst(30.0 * FT),
                )
            }
            // A 60-foot line 5 feet wide: 6d8 radiant and a 2 s blind, half
            // and no blind on a Constitution save.
            S::Sunbeam => {
                def.rider = Some((C::Blinded, 2.0));
                (
                    "Sunbeam",
                    6,
                    60.0 * FT,
                    (6, 8),
                    D::Radiant,
                    save(Con, true),
                    Line(60.0 * FT, 5.0 * FT),
                )
            }
            // A 60-foot wall of brambles: 7d8 piercing when it rises, half
            // on a Dexterity save, and 7d8 to a dummy moved through it.
            S::WallOfThorns => {
                def.concentration = true;
                (
                    "Wall of Thorns",
                    6,
                    120.0 * FT,
                    (7, 8),
                    D::Piercing,
                    save(Dex, true),
                    Wall(60.0 * FT),
                )
            }
            // Ten 10-foot cubes of flame: 7d10 fire, half on a Dexterity
            // save.
            S::FireStorm => (
                "Fire Storm",
                7,
                150.0 * FT,
                (7, 10),
                D::Fire,
                save(Dex, true),
                Burst(20.0 * FT),
            ),
            // A 50-foot-radius cylinder on the caster; everything falls up.
            S::ReverseGravity => (
                "Reverse Gravity",
                7,
                0.0,
                (0, 0),
                D::Bludgeoning,
                AUTO,
                Caster,
            ),
            // A 60-foot burst: 12d6 radiant and a 3 s blind, half and no
            // blind on a Constitution save.
            S::Sunburst => {
                def.rider = Some((C::Blinded, 3.0));
                (
                    "Sunburst",
                    8,
                    150.0 * FT,
                    (12, 6),
                    D::Radiant,
                    save(Con, true),
                    Burst(60.0 * FT),
                )
            }
            // A storm over a 60-foot radius with a new effect each round:
            // thunder, acid rain, lightning, then hail ([`super::aura`]).
            S::StormOfVengeance => {
                def.concentration = true;
                (
                    "Storm of Vengeance",
                    9,
                    300.0 * FT,
                    (2, 6),
                    D::Thunder,
                    save(Con, false),
                    Zone(60.0 * FT),
                )
            }
            S::Shapechange => ("Shapechange", 9, 0.0, (0, 0), D::Force, AUTO, Caster),
            S::SpeakWithAnimals => ("Speak with Animals", 1, 0.0, (0, 0), D::Force, AUTO, Caster),
            // Meteor Swarm: each meteor's 40-foot sphere is 20d6 fire and
            // 20d6 bludgeoning, half on a Dexterity save; here a 4 m blast
            // where each meteor bursts ([`meteor::BLAST`]), and a dummy
            // takes one cast's damage once.
            S::MeteorSwarm => {
                def.extra = Some((20, 6, D::Bludgeoning));
                (
                    "Meteor Swarm",
                    9,
                    meteor::RANGE,
                    (20, 6),
                    D::Fire,
                    save(Dex, true),
                    Burst(meteor::BLAST),
                )
            }
            // The Grove's own Thunderbolt, a huge Call Lightning strike:
            // 12d10 lightning in a 3.4 m blast, half on a Dexterity save.
            S::Thunderbolt => (
                "Thunderbolt",
                7,
                meteor::RANGE,
                (12, 10),
                D::Lightning,
                save(Dex, true),
                Burst(meteor::BOLT_BLAST),
            ),
            // A 120-foot bolt: an attack roll for 4d10 fire at level 17+.
            S::FireBolt => ("Fire Bolt", 0, 120.0 * FT, (4, 10), D::Fire, SPELL, Bolt),
            // A 15-foot cone: 3d6 fire, half on a Dexterity save.
            S::BurningHands => (
                "Burning Hands",
                1,
                15.0 * FT,
                (3, 6),
                D::Fire,
                save(Dex, true),
                Cone(15.0 * FT),
            ),
            S::Blur => ("Blur", 2, 0.0, (0, 0), D::Force, AUTO, Caster),
            // A 150-foot throw: 8d6 fire in a 20-foot radius, half on a
            // Dexterity save.
            S::Fireball => (
                "Fireball",
                3,
                150.0 * FT,
                (8, 6),
                D::Fire,
                save(Dex, true),
                Bolt,
            ),
            // 8d8 necrotic to one dummy, half on a Constitution save.
            S::Blight => (
                "Blight",
                4,
                30.0 * FT,
                (8, 8),
                D::Necrotic,
                save(Con, true),
                Single,
            ),
            // Panels rise at the target and shove it to one side.
            S::WallOfStone => (
                "Wall of Stone",
                5,
                36.0,
                (0, 0),
                D::Bludgeoning,
                AUTO,
                Caster,
            ),
            // A 60-foot ray: an attack for 4d8 cold that slows for 6 s.
            S::RayOfFrost => {
                def.rider = Some((C::Slowed, 6.0));
                ("Ray of Frost", 0, 60.0 * FT, (4, 8), D::Cold, SPELL, Bolt)
            }
            // A 20-foot-radius fog: a dummy in it is blinded while it lasts.
            S::FogCloud => {
                def.concentration = true;
                def.rider = Some((C::Blinded, 1.2));
                (
                    "Fog Cloud",
                    1,
                    120.0 * FT,
                    (0, 0),
                    D::Force,
                    AUTO,
                    Zone(20.0 * FT),
                )
            }
            // A failed Wisdom save is paralyzed for 6 s.
            S::HoldPerson => {
                def.rider = Some((C::Paralyzed, 6.0));
                (
                    "Hold Person",
                    2,
                    60.0 * FT,
                    (0, 0),
                    D::Force,
                    save(Wis, false),
                    Single,
                )
            }
            // A 40-foot-radius storm of sleet: a failed Dexterity save falls
            // prone, each 3 s.
            S::SleetStorm => {
                def.concentration = true;
                def.rider = Some((C::Prone, 1.5));
                (
                    "Sleet Storm",
                    3,
                    150.0 * FT,
                    (0, 0),
                    D::Cold,
                    save(Dex, false),
                    Zone(40.0 * FT),
                )
            }
            // A 60-foot cone: 8d8 cold, half on a Constitution save.
            S::ConeOfCold => (
                "Cone of Cold",
                5,
                60.0 * FT,
                (8, 8),
                D::Cold,
                save(Con, true),
                Cone(60.0 * FT),
            ),
            // A touch for 4d8 lightning at level 17+.
            S::ShockingGrasp => (
                "Shocking Grasp",
                0,
                3.5,
                (4, 8),
                D::Lightning,
                SPELL,
                Single,
            ),
            // A 5-foot-radius sphere: a failed Wisdom save sleeps for 6 s.
            S::Sleep => {
                def.rider = Some((C::Asleep, 6.0));
                (
                    "Sleep",
                    1,
                    60.0 * FT,
                    (0, 0),
                    D::Force,
                    save(Wis, false),
                    Burst(5.0 * FT),
                )
            }
            // A 30-foot blink toward the target.
            S::MistyStep => ("Misty Step", 2, 30.0 * FT, (0, 0), D::Force, AUTO, Caster),
            // A 100-foot line 5 feet wide: 8d6 lightning, half on a
            // Dexterity save.
            S::LightningBolt => (
                "Lightning Bolt",
                3,
                100.0 * FT,
                (8, 6),
                D::Lightning,
                save(Dex, true),
                Line(100.0 * FT, 5.0 * FT),
            ),
            S::FreedomOfMovement => (
                "Freedom of Movement",
                4,
                0.0,
                (0, 0),
                D::Force,
                AUTO,
                Caster,
            ),
            // A step through one tree and out of another: here a 60-foot
            // blink toward the target.
            S::TreeStride => ("Tree Stride", 5, 60.0 * FT, (0, 0), D::Force, AUTO, Caster),
            // A 5-foot-radius sphere: 4d6 acid on a failed Dexterity save.
            S::AcidSplash => (
                "Acid Splash",
                0,
                60.0 * FT,
                (4, 6),
                D::Acid,
                save(Dex, false),
                Burst(5.0 * FT),
            ),
            // A 60-foot ray: an attack for 2d8 poison that poisons for 6 s.
            S::RayOfSickness => {
                def.rider = Some((C::Poisoned, 6.0));
                (
                    "Ray of Sickness",
                    1,
                    60.0 * FT,
                    (2, 8),
                    D::Poison,
                    SPELL,
                    Bolt,
                )
            }
            // A 20-foot cube at the target; a failed Dexterity save roots.
            S::Web => (
                "Web",
                2,
                60.0 * FT,
                (0, 0),
                D::Force,
                save(Dex, false),
                Caster,
            ),
            // A 20-foot-radius cloud: a failed Constitution save is poisoned,
            // each second.
            S::StinkingCloud => {
                def.concentration = true;
                def.rider = Some((C::Poisoned, 1.2));
                (
                    "Stinking Cloud",
                    3,
                    90.0 * FT,
                    (0, 0),
                    D::Poison,
                    save(Con, false),
                    Zone(20.0 * FT),
                )
            }
            // A 20-foot-radius swarm: 4d10 piercing a second, half on a
            // Constitution save.
            S::InsectPlague => {
                def.concentration = true;
                (
                    "Insect Plague",
                    5,
                    300.0 * FT,
                    (4, 10),
                    D::Piercing,
                    save(Con, true),
                    Zone(20.0 * FT),
                )
            }
            // The Brown Bear's bite and claw; the claw knocks prone.
            S::BearBite => {
                def.bonus = 3;
                ("Bite", 0, 3.5, (1, 8), D::Piercing, BEAST, Single)
            }
            S::BearClaw => {
                def.bonus = 3;
                def.rider = Some((C::Prone, 1.5));
                ("Claw", 0, 3.5, (1, 6), D::Slashing, BEAST, Single)
            }
            // The Dire Wolf's bite knocks a dummy down for 1.5 s.
            S::WolfBite => {
                def.bonus = 3;
                def.rider = Some((C::Prone, 1.5));
                ("Bite", 0, 3.5, (1, 10), D::Piercing, BEAST, Single)
            }
            // The Giant Eagle's talons.
            S::EagleTalons => {
                def.bonus = 3;
                ("Talons", 0, 4.0, (2, 6), D::Slashing, BEAST, Single)
            }
            // The Giant Spider's bite: 1d8 + 3 piercing and 2d6 poison.
            S::SpiderBite => {
                def.bonus = 3;
                def.extra = Some((2, 6, D::Poison));
                ("Bite", 0, 3.5, (1, 8), D::Piercing, BEAST, Single)
            }
            // The Giant Spider's web: +5 to hit at 60 feet; a hit roots for
            // 6 s.
            S::SpiderWeb => {
                def.rider = Some((C::Restrained, 6.0));
                ("Web", 0, 60.0 * FT, (0, 0), D::Force, BEAST, Single)
            }
            // The dragon's numbers are an adult red dragon's from the SRD.
            // Its bite: +14 to hit, 2d10 + 8 piercing and 2d6 fire, at the
            // reach of its long neck.
            S::DragonBite => {
                def.bonus = 8;
                def.extra = Some((2, 6, D::Fire));
                (
                    "Dragon Bite",
                    0,
                    DRAGON_REACH,
                    (2, 10),
                    D::Piercing,
                    DRAGON,
                    Single,
                )
            }
            // A 60-foot cone: 18d6 fire, half on a Dexterity save; a failed
            // save leaves the dummy burning.
            S::FireBreath => {
                def.rider = Some((C::Burning, 6.0));
                (
                    "Fire Breath",
                    0,
                    60.0 * FT,
                    (18, 6),
                    D::Fire,
                    save(Dex, true),
                    Cone(60.0 * FT),
                )
            }
            // The tail swung around: 2d8 + 8 bludgeoning to every dummy
            // within 25 feet, half on a Dexterity save; a failure knocks it
            // down.
            S::TailSweep => {
                def.bonus = 8;
                def.rider = Some((C::Prone, 2.0));
                (
                    "Tail Sweep",
                    0,
                    0.0,
                    (2, 8),
                    D::Bludgeoning,
                    save(Dex, true),
                    Burst(25.0 * FT),
                )
            }
            // The wings beat down: 2d6 + 8 bludgeoning within 20 feet, half
            // on a Dexterity save; a failure is thrown back 20 feet.
            S::WingBuffet => {
                def.bonus = 8;
                (
                    "Wing Buffet",
                    0,
                    0.0,
                    (2, 6),
                    D::Bludgeoning,
                    save(Dex, true),
                    Burst(20.0 * FT),
                )
            }
            // Frightful Presence: each dummy within 60 feet that fails a
            // Wisdom save is frightened for 4 s.
            S::Roar => {
                def.rider = Some((C::Frightened, 4.0));
                (
                    "Roar",
                    0,
                    0.0,
                    (0, 0),
                    D::Thunder,
                    save(Wis, false),
                    Burst(60.0 * FT),
                )
            }
        };
        def.label = label;
        def.level = level;
        def.range = range;
        def.dice = dice;
        def.kind = kind;
        def.delivery = delivery;
        def.area = area;
        def
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

    /// What a spell aimed with the cursor at a wall or the ground calls
    /// down on the town's buildings, for the two that break them.
    #[must_use]
    pub const fn strike(self) -> Option<meteor::Strike> {
        match self {
            Self::MeteorSwarm => Some(meteor::Strike::Meteors),
            Self::Thunderbolt => Some(meteor::Strike::Lightning),
            _ => None,
        }
    }

    /// Whether the spell needs a dummy in front of the druid.
    #[must_use]
    pub fn needs_target(self) -> bool {
        matches!(self, Self::Fireball | Self::Web)
            || matches!(self.def().area, Area::Bolt | Area::Single)
                && !matches!(self, Self::Elementalism)
    }

    /// Whether it is a beast's own attack, which only its shape can use.
    #[must_use]
    pub const fn beast(self) -> bool {
        matches!(
            self,
            Self::BearBite
                | Self::BearClaw
                | Self::WolfBite
                | Self::EagleTalons
                | Self::SpiderBite
                | Self::SpiderWeb
                | Self::DragonBite
                | Self::FireBreath
                | Self::TailSweep
                | Self::WingBuffet
                | Self::Roar
        )
    }

    /// Whether it changes the druid's shape.
    #[must_use]
    pub const fn shape(self) -> bool {
        matches!(
            self,
            Self::WildShapeBear
                | Self::WildShapeWolf
                | Self::WildShapeEagle
                | Self::WildShapeSpider
                | Self::Shapechange
                | Self::ReturnToForm
        )
    }

    /// Whether the druid casts it again while its key is held: a spell or
    /// an attack, not a change of shape or a control.
    #[must_use]
    pub const fn repeats(self) -> bool {
        !self.shape()
            && !matches!(
                self,
                Self::ChooseLand
                    | Self::LongRest
                    | Self::SpeakWithAnimals
                    | Self::MeteorSwarm
                    | Self::Thunderbolt
            )
    }

    /// Whether the Grove gives it no effect of its own yet: it casts a
    /// labeled burst and logs what it would do (`docs/verse/druid-demo.md`,
    /// Scope).
    #[must_use]
    pub const fn placeholder(self) -> bool {
        matches!(
            self,
            Self::WildCompanion
                | Self::NaturesSanctuary
                | Self::NatureMagician
                | Self::WildResurgence
                | Self::Blur
                | Self::FreedomOfMovement
        )
    }
}
