//! The jump's shape: how long the bunny rises, hangs and falls, and how
//! high it is on the way, for a jump held for any length of time.
//!
//! A jump rises at a steady speed for as long as the jump key is held,
//! from [`RISE_TAP`] (a tap) up to [`RISE_MAX`], then hangs a moment at
//! the top ([`HANG`]) and falls in three quarters of the time it rose
//! ([`fall`]), so a tap takes exactly [`JUMP`], the jump every garden is
//! built around, and a full hold rises about four times as high and stays
//! up about four times as long. The game ([`crate::game`]) keeps the jump's
//! length in ticks; [`lift`] gives the drawing its height, in tap peaks,
//! and the meadow's free hop uses the same curve in milliseconds.

use crate::ticks;

/// A tap's whole air time: a tap jump is exactly the jump the gardens
/// were built around.
pub const JUMP: u32 = ticks(550);
/// How long a tap rises.
pub const RISE_TAP: u32 = ticks(270);
/// The longest rise, holding the jump key down.
pub const RISE_MAX: u32 = ticks(1100);
/// The hang at the top: what a tap's air time leaves after its rise and
/// fall.
pub const HANG: u32 = JUMP - RISE_TAP - fall(RISE_TAP);

/// How long the fall takes after a rise of `rise` ticks: faster than the
/// rise.
#[must_use]
pub const fn fall(rise: u32) -> u32 {
    rise * 3 / 4
}

/// A jump's whole air time after a rise of `rise` ticks.
#[must_use]
pub const fn span(rise: u32) -> u32 {
    rise + HANG + fall(rise)
}

/// The rise a key held for `held` ticks gives: at least a tap's, at most
/// [`RISE_MAX`].
#[must_use]
pub fn rise_for(held: u32) -> u32 {
    held.clamp(RISE_TAP, RISE_MAX)
}

/// The height, in tap peaks (a tap tops out at 1.0), `elapsed` ticks into
/// a jump whose rise has lasted `rise` ticks so far; `rising` while it is
/// still going up. Ticks may be fractional, for drawing between ticks.
/// The rise is steady from the first tick (a snappy take-off), the top
/// bulges a little through the hang, and the fall speeds up to the ground.
#[must_use]
pub fn lift(elapsed: f32, rise: f32, rising: bool) -> f32 {
    let tap = RISE_TAP as f32;
    let elapsed = elapsed.max(0.0);
    if rising || elapsed <= rise {
        return elapsed.min(RISE_MAX as f32) / tap;
    }
    let top = rise / tap;
    let hang = HANG as f32;
    let after = elapsed - rise;
    if after <= hang {
        // Carry on up a touch and come back: the hang at the top.
        return top + 0.06 * top.sqrt() * (std::f32::consts::PI * after / hang).sin();
    }
    let fall = (rise * 0.75).floor().max(1.0);
    let f = ((after - hang) / fall).min(1.0);
    (top * (1.0 - f * f)).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A jump's height at its highest, sampling every tick.
    fn peak(rise: u32) -> f32 {
        (0..=span(rise))
            .map(|t| lift(t as f32, rise as f32, false))
            .fold(0.0, f32::max)
    }

    #[test]
    fn a_tap_is_the_jump_the_gardens_are_built_around() {
        assert_eq!(span(RISE_TAP), JUMP);
        assert_eq!(span(rise_for(0)), JUMP);
        assert_eq!(rise_for(1), RISE_TAP);
        assert!(HANG > 0 && HANG < ticks(150), "a short hang");
    }

    #[test]
    fn holding_longer_hops_higher_up_to_a_cap() {
        let tap = peak(rise_for(0));
        let half = peak(rise_for(ticks(600)));
        let max = peak(rise_for(ticks(1100)));
        let longer = peak(rise_for(ticks(5000)));
        assert!((1.0..1.1).contains(&tap), "{tap}");
        assert!(half > tap * 2.0, "{half}");
        assert!(max > half * 1.6, "{max}");
        assert!(max >= 4.0, "a big hop: {max}");
        assert!((longer - max).abs() < 1e-6, "the cap");
    }

    #[test]
    fn the_fall_is_faster_than_the_rise_and_ends_on_the_ground() {
        for rise in [RISE_TAP, ticks(700), RISE_MAX] {
            assert!(fall(rise) < rise);
            let s = span(rise);
            assert_eq!(lift(s as f32, rise as f32, false), 0.0, "lands");
            assert!(lift(s as f32 - 1.0, rise as f32, false) > 0.0, "in the air");
            // Steady from the first tick: a snappy take-off.
            assert!(lift(1.0, rise as f32, false) > 0.003);
            // The fall's last tick drops more than its first.
            let t0 = (rise + HANG) as f32;
            let first = lift(t0, rise as f32, false) - lift(t0 + 1.0, rise as f32, false);
            let last =
                lift(s as f32 - 1.0, rise as f32, false) - lift(s as f32, rise as f32, false);
            assert!(last > first, "speeds up");
            // Never below the ground.
            assert!((0..=s + 5).all(|t| lift(t as f32, rise as f32, false) >= 0.0));
        }
    }

    #[test]
    fn while_held_it_keeps_rising() {
        let a = lift(30.0, 30.0, true);
        let b = lift(31.0, 31.0, true);
        assert!(b > a);
        assert!(lift(1e6, 1e6, true) <= RISE_MAX as f32 / RISE_TAP as f32);
    }
}
