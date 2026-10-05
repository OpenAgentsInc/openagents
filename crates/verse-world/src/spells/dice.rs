//! Seeded dice, saving throws, and SRD falling damage.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Deterministic dice: a SplitMix64 stream from a recorded seed, plus d20
/// results a scenario forces for one target's next save. Checkpoints carry
/// the stream position, so a replay rolls the same numbers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dice {
    pub seed: u64,
    state: u64,
    /// Forced d20 results by target, used in order before the stream.
    forced: BTreeMap<u64, Vec<u32>>,
    pub rolled: u64,
}

/// One saving throw: the d20, the target's modifier, and the outcome
/// against the caster's spell save DC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Save {
    pub target: u64,
    pub ability: String,
    pub roll: u32,
    pub modifier: i32,
    pub total: i32,
    pub dc: i32,
    pub success: bool,
    pub forced: bool,
}

impl Dice {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            state: seed,
            forced: BTreeMap::new(),
            rolled: 0,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.forced.len() > 256
            || self
                .forced
                .values()
                .any(|rolls| rolls.len() > 16 || rolls.iter().any(|r| !(1..=20).contains(r)))
        {
            return Err("Invalid dice checkpoint".into());
        }
        Ok(())
    }
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// One die with `sides` faces, 1 through `sides`.
    pub fn roll(&mut self, sides: u32) -> u32 {
        let sides = sides.max(1);
        self.rolled += 1;
        // Rejection sampling keeps every face equally likely.
        let zone = u64::MAX - u64::MAX % u64::from(sides);
        loop {
            let value = self.next();
            if value < zone {
                return (value % u64::from(sides)) as u32 + 1;
            }
        }
    }
    /// The sum of `count` dice with `sides` faces.
    pub fn sum(&mut self, count: u32, sides: u32) -> u32 {
        (0..count).map(|_| self.roll(sides)).sum()
    }
    /// Forces `target`'s next d20 save to show `roll`.
    pub fn force_save(&mut self, target: u64, roll: u32) -> Result<(), String> {
        if !(1..=20).contains(&roll) {
            return Err("A d20 shows 1 through 20".into());
        }
        let queue = self.forced.entry(target).or_default();
        if queue.len() >= 16 {
            return Err("Too many forced rolls".into());
        }
        queue.push(roll);
        Ok(())
    }
    /// A saving throw: one d20 plus `modifier` against `dc`.
    pub fn save(&mut self, target: u64, ability: &str, modifier: i32, dc: i32) -> Save {
        let forced = self
            .forced
            .get_mut(&target)
            .and_then(|queue| (!queue.is_empty()).then(|| queue.remove(0)));
        self.forced.retain(|_, queue| !queue.is_empty());
        let roll = forced.unwrap_or_else(|| self.roll(20));
        let total = roll as i32 + modifier;
        Save {
            target,
            ability: ability.into(),
            roll,
            modifier,
            total,
            dc,
            success: total >= dc,
            forced: forced.is_some(),
        }
    }
}

/// Returns a caster's reproducible stream. The original stream remains the
/// primary caster's stream so retained scenario fixtures keep their rolls.
pub(crate) fn for_caster<'a>(
    legacy: &'a mut Dice,
    streams: &'a mut BTreeMap<u64, Dice>,
    primary: u64,
    caster: u64,
) -> &'a mut Dice {
    if caster == primary {
        legacy
    } else {
        let seed = legacy.seed ^ caster.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        streams.entry(caster).or_insert_with(|| Dice::new(seed))
    }
}

/// SRD falling: 1d6 Bludgeoning per 10 feet fallen, at most 20d6.
pub const FALL_DICE_MAX: u32 = 20;

/// Dice of falling damage for a fall of `height` meters.
pub fn fall_dice(height: f64) -> u32 {
    if !height.is_finite() || height <= 0. {
        return 0;
    }
    // A hair of tolerance keeps an exact 10-foot fall at 1d6.
    ((height / (10. * super::FEET) + 1e-4).floor() as u32).min(FALL_DICE_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spells::FEET;
    #[test]
    fn falling_damage_counts_one_die_per_ten_feet_up_to_twenty() {
        assert_eq!(fall_dice(9.9 * FEET), 0);
        assert_eq!(fall_dice(10. * FEET), 1);
        assert_eq!(fall_dice(100. * FEET - 0.00002), 10);
        assert_eq!(fall_dice(100. * FEET - 0.001), 9);
        assert_eq!(fall_dice(25. * FEET), 2);
        assert_eq!(fall_dice(250. * FEET), 20);
        let mut dice = Dice::new(7);
        for (feet, count) in [(10., 1), (25., 2), (250., 20)] {
            let n = fall_dice(feet * FEET);
            assert_eq!(n, count);
            let damage = dice.sum(n, 6);
            assert!((n..=n * 6).contains(&damage));
        }
    }
    #[test]
    fn seeded_rolls_repeat_and_forced_saves_come_first() {
        let mut a = Dice::new(42);
        let mut b = Dice::new(42);
        let rolls: Vec<_> = (0..200).map(|_| a.roll(20)).collect();
        assert_eq!(rolls, (0..200).map(|_| b.roll(20)).collect::<Vec<_>>());
        assert!(rolls.iter().all(|r| (1..=20).contains(r)));
        assert!((1..=20).all(|face| rolls.contains(&face)));
        a.force_save(5, 19).unwrap();
        let save = a.save(5, "Constitution", 0, 15);
        assert!(save.success && save.forced && save.roll == 19);
        assert!(!a.save(5, "Constitution", 0, 15).forced);
        assert!(a.force_save(5, 21).is_err());
    }
}
