//! Swimming, wading, and breath under SRD 5.2.1, in real seconds on the
//! world's clock (`docs/verse/water.md`, Gameplay rules).
//!
//! - [`Medium`] (from `physics::water::medium`): ground, wading, swimming,
//!   or diving, from the gameplay surface.
//! - [`pace`]: what a medium, Difficult Terrain, a Swim Speed, and
//!   Exhaustion do to a character's speed.
//! - [`Breath`]: holding breath for 1 + Constitution modifier minutes (at
//!   least 30 s), then 1 Exhaustion level every 6 s, defeat at level 6, and
//!   every suffocation level gone as soon as the creature breathes.
//! - [`melee`], [`ranged`], and [`fire_resistant`]: underwater combat.
//! - [`fall_into_water`]: the DC 15 check that halves a fall's damage.
//! - [`held`]: which conditions leave a swimmer floating face up.
//!
//! **Ours** in a comment marks a rule where the SRD is silent and Verse
//! chose one, as the specification does.

pub use physics::water::medium::{
    self, CLIMB_LIP, FLOAT_DEPTH, Medium, SWIM_DEPTH, Stroke, WADE_DEPTH,
};

use crate::spells::{Dice, Save};

/// Multiplies every hold time, if the owner ever wants a game-scale
/// duration. One plays the SRD as written.
pub const BREATH_SCALE: f32 = 1.0;
/// The shortest held breath, s (SRD 5.2.1: at least 30 seconds).
pub const MIN_HOLD: f32 = 30.0;
/// One round, s: an out-of-breath creature gains a level at the end of
/// each of its turns.
pub const ROUND: f32 = 6.0;
/// How long breath takes to refill from empty with the eye in the air, s
/// (Ours).
pub const REFILL: f32 = 6.0;
/// The Exhaustion level that kills, or in a zone without death defeats.
pub const DEFEAT_LEVEL: u8 = 6;
/// The Speed whose fraction each Exhaustion level takes, ft.
pub const BASE_SPEED_FEET: f32 = 30.0;

/// How long a creature with Constitution modifier `con` holds its breath,
/// s: 1 + `con` minutes, at least [`MIN_HOLD`], times [`BREATH_SCALE`].
#[must_use]
pub fn hold_seconds(con: i32) -> f32 {
    (60.0 * (1 + con.clamp(-10, 20)) as f32).max(MIN_HOLD) * BREATH_SCALE
}

/// What a step of [`Breath::tick`] changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreathEvent {
    /// Breath ran out this step.
    OutOfBreath,
    /// A round ended out of breath: the creature now has this many
    /// suffocation levels of Exhaustion.
    Exhausted(u8),
    /// The sixth level: the creature dies, or where nothing dies is
    /// defeated. Its suffocation levels are already removed.
    Defeated,
    /// It breathes again, which removed this many suffocation levels.
    Recovered(u8),
}

/// A creature's breath and the Exhaustion it gained from suffocating.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Breath {
    con: i32,
    /// Breath left, s.
    left: f64,
    /// Seconds out of breath since it last breathed.
    out: f64,
    /// Exhaustion levels gained from suffocating.
    levels: u8,
}

impl Breath {
    /// Full breath for a creature with Constitution modifier `con`.
    #[must_use]
    pub fn new(con: i32) -> Self {
        Self {
            con,
            left: f64::from(hold_seconds(con)),
            out: 0.0,
            levels: 0,
        }
    }

    /// The Constitution modifier the hold time comes from; a Wild Shape or
    /// Shapechange form sets its own ([`Self::set_con`]).
    #[must_use]
    pub fn con(&self) -> i32 {
        self.con
    }

    /// Takes a form's Constitution modifier, keeping the share of breath
    /// left.
    pub fn set_con(&mut self, con: i32) {
        let share = self.left / f64::from(self.limit());
        self.con = con;
        self.left = share * f64::from(self.limit());
    }

    /// The full hold, s.
    #[must_use]
    pub fn limit(&self) -> f32 {
        hold_seconds(self.con)
    }

    /// Breath left, s.
    #[must_use]
    pub fn left(&self) -> f32 {
        self.left as f32
    }

    /// The share of breath left, 0 to 1.
    #[must_use]
    pub fn fraction(&self) -> f32 {
        (self.left / f64::from(self.limit())).clamp(0.0, 1.0) as f32
    }

    /// Exhaustion levels from suffocating.
    #[must_use]
    pub fn levels(&self) -> u8 {
        self.levels
    }

    /// Whether breath is full and no suffocation level is left.
    #[must_use]
    pub fn full(&self) -> bool {
        self.levels == 0 && self.left >= f64::from(self.limit())
    }

    /// Advances `dt` seconds. `under`: the eye is below the gameplay
    /// surface. `breathes_water`: Water Breathing, so no breath is spent.
    /// Returns what happened, in order.
    pub fn tick(&mut self, dt: f32, under: bool, breathes_water: bool) -> Vec<BreathEvent> {
        let mut events = Vec::new();
        if !dt.is_finite() || dt <= 0.0 {
            return events;
        }
        let dt = f64::from(dt);
        let limit = f64::from(self.limit());
        if !under || breathes_water {
            // Breathing again removes every suffocation level at once
            // (SRD 5.2.1); breath refills linearly over REFILL (Ours).
            if self.levels > 0 {
                events.push(BreathEvent::Recovered(self.levels));
                self.levels = 0;
            }
            self.out = 0.0;
            if !under {
                self.left = (self.left + limit / f64::from(REFILL) * dt).min(limit);
            }
            return events;
        }
        let mut spare = dt;
        if self.left > 0.0 {
            let spent = spare.min(self.left);
            self.left -= spent;
            spare -= spent;
            // A hair of tolerance, so a clock of many short steps runs out
            // on the step it should.
            if self.left <= 1e-9 {
                self.left = 0.0;
                events.push(BreathEvent::OutOfBreath);
            }
        }
        if self.left > 0.0 {
            return events;
        }
        self.out += spare;
        let round = f64::from(ROUND);
        while self.out + 1e-9 >= round * f64::from(self.levels + 1) {
            self.levels += 1;
            if self.levels >= DEFEAT_LEVEL {
                events.push(BreathEvent::Defeated);
                // Where nothing dies, the swimmer surfaces with its
                // suffocation levels removed (Ours); a zone with death
                // reads the event first.
                *self = Self::new(self.con);
                break;
            }
            events.push(BreathEvent::Exhausted(self.levels));
        }
        events
    }
}

/// The speed multiplier for `medium` (SRD 5.2.1): swimming costs 1 extra
/// foot a foot, 2 in Difficult Terrain, unless the creature has a Swim
/// Speed; wading is Difficult Terrain (Ours); each Exhaustion level takes
/// 5 feet of a 30-foot Speed.
#[must_use]
pub fn pace(medium: Medium, swim_speed: bool, difficult: bool, exhaustion: u8) -> f32 {
    let base = match medium {
        Medium::Ground if difficult => 0.5,
        Medium::Ground => 1.0,
        Medium::Wading => 0.5,
        Medium::Swimming | Medium::Diving => match (swim_speed, difficult) {
            (true, false) => 1.0,
            (true, true) | (false, false) => 0.5,
            (false, true) => 1.0 / 3.0,
        },
    };
    base * exhaustion_speed(exhaustion)
}

/// The share of Speed Exhaustion leaves: 5 feet a level of a 30-foot
/// Speed, never below zero.
#[must_use]
pub fn exhaustion_speed(levels: u8) -> f32 {
    ((BASE_SPEED_FEET - 5.0 * f32::from(levels)) / BASE_SPEED_FEET).max(0.0)
}

/// What Exhaustion subtracts from every d20 Test: 2 a level.
#[must_use]
pub fn exhaustion_penalty(levels: u8) -> i32 {
    -2 * i32::from(levels)
}

/// A weapon, for the underwater attack rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weapon {
    Dagger,
    Javelin,
    Shortsword,
    Spear,
    Trident,
    Crossbow,
    Net,
    Dart,
    /// Any other melee weapon.
    Melee,
    /// Any other ranged weapon without the Thrown property.
    Ranged,
    /// Any other weapon with the Thrown property.
    Thrown,
}

/// How an attack rolls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Roll {
    Normal,
    Disadvantage,
    /// It misses without a roll.
    Miss,
}

/// A melee weapon attack by a creature fighting underwater (SRD 5.2.1):
/// Disadvantage without a Swim Speed unless the weapon is a dagger,
/// javelin, shortsword, spear, or trident.
#[must_use]
pub fn melee(weapon: Weapon, swim_speed: bool) -> Roll {
    let suited = matches!(
        weapon,
        Weapon::Dagger | Weapon::Javelin | Weapon::Shortsword | Weapon::Spear | Weapon::Trident
    );
    if swim_speed || suited {
        Roll::Normal
    } else {
        Roll::Disadvantage
    }
}

/// A ranged weapon attack underwater at `distance` against a weapon whose
/// normal range is `normal`, m (SRD 5.2.1): it misses beyond the normal
/// range, and within it has Disadvantage unless the weapon is a crossbow,
/// a net, or thrown (a javelin, spear, trident, or dart among them).
#[must_use]
pub fn ranged(weapon: Weapon, distance: f32, normal: f32) -> Roll {
    if distance > normal {
        return Roll::Miss;
    }
    let suited = matches!(
        weapon,
        Weapon::Crossbow
            | Weapon::Net
            | Weapon::Javelin
            | Weapon::Spear
            | Weapon::Trident
            | Weapon::Dart
            | Weapon::Dagger
            | Weapon::Thrown
    );
    if suited {
        Roll::Normal
    } else {
        Roll::Disadvantage
    }
}

/// Whether a creature or object has Resistance to Fire damage: when it is
/// fully underwater (SRD 5.2.1). Spell attacks take no other penalty.
#[must_use]
pub const fn fire_resistant(fully_under: bool) -> bool {
    fully_under
}

/// The DC of the check a creature falling into water makes.
pub const FALL_INTO_WATER_DC: i32 = 15;

/// A creature `target` falls into water and takes `damage` from the fall:
/// its Reaction makes a DC 15 Strength (Athletics) or Dexterity
/// (Acrobatics) check with the better `modifier`, rolled behind the scenes;
/// on a success the damage is halved, rounded down (SRD 5.2.1). Returns
/// the damage it takes and the check.
pub fn fall_into_water(dice: &mut Dice, target: u64, modifier: i32, damage: u32) -> (u32, Save) {
    let check = dice.save(target, "Athletics", modifier, FALL_INTO_WATER_DC);
    let taken = if check.success { damage / 2 } else { damage };
    (taken, check)
}

/// A condition that may hold a swimmer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    Paralyzed,
    Stunned,
    /// Asleep, as from Sleep.
    Asleep,
    Restrained,
    Grappled,
    Prone,
}

/// Whether `condition` leaves a swimmer floating face up at the float
/// line, drifting with the flow and still breathing (Ours: control
/// spells don't drown anyone).
#[must_use]
pub const fn held(condition: Condition) -> bool {
    matches!(
        condition,
        Condition::Paralyzed | Condition::Stunned | Condition::Asleep
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Holds breath under water on a clock of `dt` steps until it runs
    /// out, s.
    fn hold(con: i32, dt: f32) -> f32 {
        let mut breath = Breath::new(con);
        let mut t = 0.0_f64;
        loop {
            let events = breath.tick(dt, true, false);
            t += f64::from(dt);
            if events.contains(&BreathEvent::OutOfBreath) {
                return t as f32;
            }
            assert!(t < 10_000.0);
        }
    }

    #[test]
    fn breath_holds_one_plus_constitution_minutes_at_least_thirty_seconds() {
        assert_eq!(hold_seconds(-1), 30.0);
        assert_eq!(hold_seconds(-4), 30.0);
        assert_eq!(hold_seconds(0), 60.0);
        assert_eq!(hold_seconds(1), 120.0);
        assert_eq!(hold_seconds(2), 180.0);
        assert_eq!(hold_seconds(5), 360.0);
        // On the world's fixed clock: 120 steps a second.
        let dt = 1.0 / 120.0;
        for (con, seconds) in [(-1, 30.0), (0, 60.0), (2, 180.0)] {
            let held = hold(con, dt);
            assert!((held - seconds).abs() <= dt + 1e-3, "{con}: {held}");
        }
    }

    #[test]
    fn out_of_breath_a_level_lands_every_round_and_the_sixth_defeats() {
        let dt = 1.0 / 120.0;
        let mut breath = Breath::new(0);
        let mut t = 0.0_f64;
        let mut landed = Vec::new();
        let mut defeated = None;
        while defeated.is_none() && t < 200.0 {
            for event in breath.tick(dt, true, false) {
                match event {
                    BreathEvent::Exhausted(level) => landed.push((level, t + f64::from(dt))),
                    BreathEvent::Defeated => defeated = Some(t + f64::from(dt)),
                    _ => {}
                }
            }
            t += f64::from(dt);
        }
        assert_eq!(landed.len(), 5);
        for (k, &(level, at)) in landed.iter().enumerate() {
            assert_eq!(level as usize, k + 1);
            let expected = 60.0 + 6.0 * (k + 1) as f64;
            assert!((at - expected).abs() <= 0.02, "level {level} at {at}");
        }
        // The sixth, 36 s after breath ran out: defeated, with the
        // suffocation levels gone and full breath.
        let defeated = defeated.unwrap();
        assert!((defeated - 96.0).abs() <= 0.02, "{defeated}");
        assert_eq!(breath.levels(), 0);
        assert!(breath.full());
    }

    #[test]
    fn breathing_again_removes_every_suffocation_level_and_refills() {
        let mut breath = Breath::new(-1);
        breath.tick(30.0 + 13.0, true, false);
        assert_eq!(breath.levels(), 2);
        assert_eq!(breath.left(), 0.0);
        let events = breath.tick(0.5, false, false);
        assert_eq!(events, vec![BreathEvent::Recovered(2)]);
        assert_eq!(breath.levels(), 0);
        // Linear refill over six seconds.
        assert!((breath.left() - 30.0 * 0.5 / 6.0).abs() < 1e-4);
        breath.tick(6.0, false, false);
        assert!(breath.full());
        // Water Breathing spends nothing under water.
        breath.tick(600.0, true, true);
        assert!(breath.full());
    }

    #[test]
    fn swimming_and_wading_cost_speed() {
        assert_eq!(pace(Medium::Ground, false, false, 0), 1.0);
        assert_eq!(pace(Medium::Wading, false, false, 0), 0.5);
        assert_eq!(pace(Medium::Swimming, false, false, 0), 0.5);
        assert!((pace(Medium::Diving, false, true, 0) - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(pace(Medium::Swimming, true, false, 0), 1.0);
        // Each Exhaustion level takes a sixth of a 30-foot Speed.
        assert!((pace(Medium::Ground, false, false, 3) - 0.5).abs() < 1e-6);
        assert_eq!(exhaustion_speed(6), 0.0);
        assert_eq!(exhaustion_penalty(2), -4);
    }

    #[test]
    fn underwater_attacks_follow_the_srd() {
        assert_eq!(melee(Weapon::Melee, false), Roll::Disadvantage);
        assert_eq!(melee(Weapon::Melee, true), Roll::Normal);
        assert_eq!(melee(Weapon::Trident, false), Roll::Normal);
        assert_eq!(ranged(Weapon::Ranged, 10.0, 24.0), Roll::Disadvantage);
        assert_eq!(ranged(Weapon::Crossbow, 10.0, 24.0), Roll::Normal);
        assert_eq!(ranged(Weapon::Crossbow, 30.0, 24.0), Roll::Miss);
        assert!(fire_resistant(true) && !fire_resistant(false));
        assert!(held(Condition::Asleep) && !held(Condition::Prone));
    }

    #[test]
    fn a_fall_into_water_halves_on_a_successful_check() {
        let mut dice = Dice::new(7);
        dice.force_save(1, 15).unwrap();
        let (taken, check) = fall_into_water(&mut dice, 1, 0, 11);
        assert!(check.success);
        assert_eq!(taken, 5);
        dice.force_save(1, 3).unwrap();
        let (taken, check) = fall_into_water(&mut dice, 1, 2, 11);
        assert!(!check.success);
        assert_eq!(taken, 11);
    }
}
