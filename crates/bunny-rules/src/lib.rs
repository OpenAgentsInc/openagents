//! Grow Little Bunny's rules (`docs/verse/games/grow-little-bunny.md`).
//!
//! A pure, integer, fixed-step simulation of one garden run: the bunny runs
//! along a graph of three-lane corridors, eats what it finds, grows through
//! the size tiers, jumps, ducks, bumps or smashes obstacles, picks up
//! power-ups, and the farmer chases it with his net. Gardens are
//! `bunny.garden.v1` level files ([`level`]); a run's inputs make a receipt
//! that verifies by replay ([`receipt`]). Nothing here draws, reads a
//! clock, or uses randomness beyond the run's seed, so the same inputs on
//! the same ticks give the same run on every platform.
//!
//! Units: distances are in [`UNIT`]s (a tenth of a millimetre), time is in
//! ticks of [`HZ`] per second.

pub mod bot;
pub mod game;
pub mod garden;
pub mod kinds;
pub mod level;
pub mod progress;
pub mod receipt;
pub mod shade;

pub use game::{Event, Farmer, FarmerState, Game, Input, Move, Status};
pub use garden::{Edge, Edible, Garden, Node, Obstacle, Power};
pub use kinds::{Contact, EdibleKind, ObstacleKind, PowerKind};

/// Simulation ticks per second.
pub const HZ: u32 = 60;

/// Distance units per metre.
pub const UNIT: i32 = 10_000;

/// The distance between lane centres (1.2 m).
pub const LANE_WIDTH: i32 = 12 * UNIT / 10;

/// Half a corridor's width: three lanes.
pub const CORRIDOR_HALF: i32 = 3 * LANE_WIDTH / 2;

/// Milliseconds to ticks, rounded.
#[must_use]
pub const fn ticks(millis: u32) -> u32 {
    (millis * HZ + 500) / 1000
}

/// Millimetres per second to units per tick.
#[must_use]
pub const fn per_tick(millimetres_per_second: i32) -> i32 {
    millimetres_per_second * (UNIT / 1000) / HZ as i32
}

/// The size tiers' names, smallest first.
pub const TIER_NAMES: [&str; 5] = ["Kit", "Bunny", "Jack", "Big Bun", "Giant"];

/// Growth points, in quarters, needed to reach each tier.
pub const TIER_QUARTERS: [u32; 5] = [0, 8 * 4, 22 * 4, 40 * 4, 62 * 4];

/// Body height of each tier, in units.
pub const TIER_HEIGHT: [i32; 5] = [2_500, 3_500, 5_000, 7_500, 11_000];

/// Run speed of each tier, in units per tick.
pub const TIER_SPEED: [i32; 5] = [
    per_tick(6_000),
    per_tick(6_400),
    per_tick(6_800),
    per_tick(7_200),
    per_tick(7_600),
];

/// How high each tier's jump rises, in units, for drawing; every jump
/// lasts the same time.
pub const TIER_JUMP: [i32; 5] = [3_000, 4_000, 5_500, 7_500, 10_500];

/// How far the farmer sees a bunny of each tier down a corridor.
pub const TIER_SIGHT: [i32; 5] = [10 * UNIT, 13 * UNIT, 16 * UNIT, 20 * UNIT, 25 * UNIT];

/// The tier (0-based) a growth total reaches.
#[must_use]
pub fn tier_for(quarters: u32) -> u8 {
    let mut tier = 0;
    for (index, need) in TIER_QUARTERS.iter().enumerate() {
        if quarters >= *need {
            tier = index as u8;
        }
    }
    tier
}

/// An integer square root, for distances.
#[must_use]
pub fn isqrt(value: i64) -> i64 {
    if value <= 0 {
        return 0;
    }
    let mut x = 1_i64 << ((64 - value.leading_zeros()).div_ceil(2));
    loop {
        let next = (x + value / x) / 2;
        if next >= x {
            break;
        }
        x = next;
    }
    while x * x > value {
        x -= 1;
    }
    while (x + 1) * (x + 1) <= value {
        x += 1;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_follow_the_growth_thresholds() {
        assert_eq!(tier_for(0), 0);
        assert_eq!(tier_for(31), 0);
        assert_eq!(tier_for(32), 1);
        assert_eq!(tier_for(88), 2);
        assert_eq!(tier_for(160), 3);
        assert_eq!(tier_for(248), 4);
        assert_eq!(tier_for(10_000), 4);
    }

    #[test]
    fn speeds_and_square_roots_are_exact_integers() {
        assert_eq!(TIER_SPEED[0], 1_000);
        assert_eq!(per_tick(5_000), 833);
        assert_eq!(ticks(350), 21);
        assert_eq!(isqrt(0), 0);
        assert_eq!(isqrt(1), 1);
        assert_eq!(isqrt(99), 9);
        assert_eq!(isqrt(100), 10);
        assert_eq!(isqrt(1_000_000_000_000), 1_000_000);
        assert_eq!(isqrt(i64::from(i32::MAX)), 46_340);
    }
}
