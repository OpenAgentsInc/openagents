//! The druid's kit in the Grove: the spells on its hotbar with their SRD
//! dice and saves, and the mana, global cooldown, and per-spell cooldowns
//! that gate them.
//!
//! The numbers follow the combat model (`docs/verse/combat-model.md`): the
//! simulation rolls each spell's SRD dice, attack roll, and saving throw
//! behind the scenes with `verse_world`'s seeded dice, and the player sees
//! the outcome. Mana and cooldowns come from the tier table, with a
//! 1-second global cooldown and mana that comes back quickly after five
//! seconds without casting. Long Rest, the demo control, sits off the
//! global cooldown and refills everything.

use super::super::Intent;

/// The druid's mana pool (`docs/verse/druid-demo.md`, Numbers).
pub const MAX_MANA: f32 = 300.0;
/// Mana regained a second out of combat.
pub const MANA_REGEN: f32 = 10.0;
/// Seconds without casting before mana regenerates.
pub const OUT_OF_COMBAT: f32 = 5.0;
/// The global cooldown, s.
pub const GCD: f32 = 1.0;
/// The level 20 druid's spell attack bonus: proficiency 6 and Wisdom 5.
pub const ATTACK_BONUS: i32 = 11;
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
    LongRest,
}

/// A damage type, for resistances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Damage {
    Fire,
    Thunder,
    Bludgeoning,
}

impl Damage {
    /// The word a combat line uses.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Fire => "fire",
            Self::Thunder => "thunder",
            Self::Bludgeoning => "bludgeoning",
        }
    }

    /// The color its numbers float in, linear RGB.
    #[must_use]
    pub const fn color(self) -> [f32; 3] {
        match self {
            Self::Fire => [1.0, 0.45, 0.12],
            Self::Thunder => [0.45, 0.7, 1.0],
            Self::Bludgeoning => [0.9, 0.8, 0.55],
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
    /// A spell attack roll against armor class; a natural 20 doubles the
    /// dice.
    Attack,
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
    /// The SRD spell level, which sets the tier; 0 is a cantrip.
    pub level: u8,
    pub mana: f32,
    /// Cooldown after a cast, s; the global cooldown alone for a cantrip.
    pub cooldown: f32,
    /// Range to the target or the effect's center, m.
    pub range: f32,
    /// Damage dice, count and sides; `(0, 0)` deals no damage.
    pub dice: (u32, u32),
    pub kind: Damage,
    pub delivery: Delivery,
}

impl Spell {
    /// Every slot, in hotbar order (keys 1 to 9, then 0).
    pub const ALL: [Self; 10] = [
        Self::Thunderwave,
        Self::GustOfWind,
        Self::WindWall,
        Self::WallOfStone,
        Self::ReverseGravity,
        Self::FireBolt,
        Self::Fireball,
        Self::MistyStep,
        Self::Web,
        Self::LongRest,
    ];

    /// The spell's numbers: the SRD's dice and saves at level 20, ranges
    /// and areas at 5 ft = 1.5 m, and mana and cooldowns from the tier
    /// table.
    #[must_use]
    pub const fn def(self) -> Def {
        use Ability::{Constitution, Dexterity, Strength};
        use Delivery::{Attack, Automatic};
        const fn save(ability: Ability, half: bool) -> Delivery {
            Delivery::Save { ability, half }
        }
        let (label, level, mana, cooldown, range, dice, kind, delivery) = match self {
            // A 15-foot cube from the caster: 2d8 thunder and a 10-foot
            // push; a Constitution save halves it and holds ground.
            Self::Thunderwave => (
                "Thunderwave",
                1,
                10.0,
                8.0,
                6.0,
                (2, 8),
                Damage::Thunder,
                save(Constitution, true),
            ),
            // A 60-foot line; a failed Strength save is pushed 15 feet.
            Self::GustOfWind => (
                "Gust of Wind",
                2,
                15.0,
                10.0,
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
                25.0,
                15.0,
                36.0,
                (4, 8),
                Damage::Bludgeoning,
                save(Strength, true),
            ),
            // Panels rise at the target and shove it to one side.
            Self::WallOfStone => (
                "Wall of Stone",
                5,
                30.0,
                20.0,
                36.0,
                (0, 0),
                Damage::Bludgeoning,
                Automatic,
            ),
            // A 50-foot-radius cylinder on the caster; everything falls up.
            Self::ReverseGravity => (
                "Reverse Gravity",
                7,
                45.0,
                45.0,
                0.0,
                (0, 0),
                Damage::Bludgeoning,
                Automatic,
            ),
            // A 120-foot bolt: an attack roll for 4d10 fire at level 17+.
            Self::FireBolt => (
                "Fire Bolt",
                0,
                0.0,
                0.0,
                120.0 * FT,
                (4, 10),
                Damage::Fire,
                Attack,
            ),
            // A 150-foot throw: 8d6 fire in a 20-foot radius, half on a
            // Dexterity save.
            Self::Fireball => (
                "Fireball",
                3,
                25.0,
                15.0,
                150.0 * FT,
                (8, 6),
                Damage::Fire,
                save(Dexterity, true),
            ),
            // A 30-foot blink toward the target.
            Self::MistyStep => (
                "Misty Step",
                2,
                15.0,
                10.0,
                30.0 * FT,
                (0, 0),
                Damage::Fire,
                Automatic,
            ),
            // A 20-foot cube at the target; a failed Dexterity save roots.
            Self::Web => (
                "Web",
                2,
                15.0,
                10.0,
                60.0 * FT,
                (0, 0),
                Damage::Fire,
                save(Dexterity, false),
            ),
            Self::LongRest => (
                "Long Rest",
                0,
                0.0,
                0.0,
                0.0,
                (0, 0),
                Damage::Fire,
                Automatic,
            ),
        };
        Def {
            label,
            level,
            mana,
            cooldown,
            range,
            dice,
            kind,
            delivery,
        }
    }

    /// The zone intent that casts it.
    #[must_use]
    pub const fn intent(self) -> Intent {
        match self {
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
        }
    }

    /// The spell `intent` casts in the Grove, if any.
    #[must_use]
    pub fn of(intent: Intent) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.intent() == intent)
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
        matches!(self, Self::FireBolt | Self::Fireball | Self::Web)
    }

    /// Whether it waits on the global cooldown.
    #[must_use]
    pub const fn on_gcd(self) -> bool {
        !matches!(self, Self::LongRest)
    }

    const fn index(self) -> usize {
        self as usize
    }
}

/// The druid's mana and cooldowns.
#[derive(Clone, Debug)]
pub struct Kit {
    time: f32,
    pub mana: f32,
    gcd_until: f32,
    ready_at: [f32; Spell::ALL.len()],
    last_cast: f32,
}

impl Default for Kit {
    fn default() -> Self {
        Self {
            time: 0.0,
            mana: MAX_MANA,
            gcd_until: 0.0,
            ready_at: [0.0; Spell::ALL.len()],
            last_cast: f32::NEG_INFINITY,
        }
    }
}

impl Kit {
    /// Advances the clock and regenerates mana out of combat.
    pub fn tick(&mut self, dt: f32) {
        self.time += dt.max(0.0);
        if self.time - self.last_cast >= OUT_OF_COMBAT {
            self.mana = (self.mana + MANA_REGEN * dt.max(0.0)).min(MAX_MANA);
        }
    }

    /// Seconds until `spell` is off its own cooldown and the global one.
    #[must_use]
    pub fn waiting(&self, spell: Spell) -> f32 {
        let own = self.ready_at[spell.index()] - self.time;
        let gcd = if spell.on_gcd() {
            self.gcd_until - self.time
        } else {
            0.0
        };
        own.max(gcd).max(0.0)
    }

    /// The fraction of the longer of `spell`'s cooldowns still to run.
    #[must_use]
    pub fn cooldown_fraction(&self, spell: Spell) -> f32 {
        let own = (self.ready_at[spell.index()] - self.time).max(0.0);
        let gcd = if spell.on_gcd() {
            (self.gcd_until - self.time).max(0.0)
        } else {
            0.0
        };
        if own >= gcd && own > 0.0 {
            own / spell.def().cooldown.max(1e-3)
        } else {
            gcd / GCD
        }
    }

    /// Whether `spell` could be cast now, or why not.
    ///
    /// # Errors
    ///
    /// Returns the cooldown or the missing mana.
    pub fn admit(&self, spell: Spell) -> Result<(), String> {
        let def = spell.def();
        if self.waiting(spell) > 0.0 {
            return Err(format!("{} is not ready", def.label));
        }
        if self.mana + 1e-3 < def.mana {
            return Err(format!("Not enough mana for {}", def.label));
        }
        Ok(())
    }

    /// Spends `spell`'s mana and starts its cooldowns.
    pub fn spend(&mut self, spell: Spell) {
        let def = spell.def();
        self.mana = (self.mana - def.mana).max(0.0);
        self.ready_at[spell.index()] = self.time + def.cooldown;
        if spell.on_gcd() {
            self.gcd_until = self.time + GCD;
            self.last_cast = self.time;
        }
    }

    /// Refills mana and clears every cooldown.
    pub fn rest(&mut self) {
        *self = Self {
            time: self.time,
            ..Self::default()
        };
    }
}
