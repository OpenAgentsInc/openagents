//! Shared display of accepted XP. Levels grant no spending or execution rights.

/// The name of the level curve [`xp_to_reach`] and [`level_of`] compute.
/// Every display of a level names it, so two clients never show different
/// numbers under one name; a new curve gets a new name.
pub const CURVE: &str = "trainer-curve-v1";

/// Cumulative XP needed to reach `level`. Level 1 needs nothing; level
/// `n + 1` needs `100 · n^1.5`, rounded up: 100 XP for level 2, 283 for
/// level 3, 520 for level 4, and 800 for level 5.
#[must_use]
pub fn xp_to_reach(level: u32) -> u64 {
    if level <= 1 {
        return 0;
    }
    let n = f64::from(level - 1);
    (100.0 * n.powf(1.5)).ceil() as u64
}

/// The level `xp` reaches under [`xp_to_reach`]. Everyone starts at 1.
#[must_use]
pub fn level_of(xp: u64) -> u32 {
    let mut level = 1;
    while level < 10_000 && xp_to_reach(level + 1) <= xp {
        level += 1;
    }
    level
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trainer_curve_preserves_published_thresholds_and_boundaries() {
        assert_eq!(CURVE, "trainer-curve-v1");
        assert_eq!([1, 2, 3, 4, 5].map(xp_to_reach), [0, 100, 283, 520, 800]);
        assert_eq!(
            [0, 99, 100, 282, 283, 10_000].map(level_of),
            [1, 1, 2, 2, 3, 22]
        );
        for level in 2..100 {
            let threshold = xp_to_reach(level);
            assert_eq!(level_of(threshold), level);
            assert_eq!(level_of(threshold - 1), level - 1);
        }
        assert_eq!(level_of(u64::MAX), 10_000);
    }
}
