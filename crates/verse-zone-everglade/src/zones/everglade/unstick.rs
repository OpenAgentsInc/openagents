//! Getting a trapped player out. A walker moves over Everglade's solids on
//! a grid of [`CELL`] m squares, as the controller would: into a square
//! its body fits at its feet and on the floor there, never up more than a
//! step. A player held against walls in a pocket that walking can't
//! leave, such as a slot below a floor or the inside of a blocker, is
//! [`trapped`], and [`rescue`] finds the nearest square that isn't, so the
//! zone moves the player there.

use std::collections::{HashMap, VecDeque};

use glam::Vec3;

use super::solids::{STEP, Solids};
use crate::controller::RADIUS;

/// The walker's grid side, m.
pub const CELL: f32 = 0.25;
/// How far a player must be able to get from where they stand to count as
/// free, m.
const ESCAPE: f32 = 3.0;
/// How far [`rescue`] looks for a free square, m.
const SEARCH: f32 = 10.0;
/// How far above the player's feet [`rescue`] looks for a floor, m: out of
/// a pit as deep as a podium.
const CLIMB_OUT: f32 = 2.6;
/// How many squares a body overlapping something crosses to get clear of
/// it: more than its radius.
const WEDGED: u8 = 3;

/// Whether a body standing with its feet at `feet` over `(x, z)` overlaps
/// something taller than a step.
#[must_use]
pub fn blocked(solids: &Solids, x: f32, z: f32, feet: f32) -> bool {
    solids
        .blocking_near(x, z, RADIUS, feet)
        .iter()
        .any(|f| f.contains(x, z, RADIUS))
}

/// Where a walker standing at `feet` over one square ends up stepping to
/// `(x, z)`: the floor there, or `None` when its body doesn't fit on the
/// way or standing there.
#[must_use]
pub fn step_to(solids: &Solids, x: f32, z: f32, feet: f32) -> Option<f32> {
    let floor = drop_to(solids, x, z, feet)?;
    (!blocked(solids, x, z, floor)).then_some(floor)
}

/// Where a walker with its feet at `feet` lands moving over `(x, z)`, its
/// body fitting there at that height: the floor under it, which it may
/// not fit on. `None` when it can't move there.
#[must_use]
pub fn drop_to(solids: &Solids, x: f32, z: f32, feet: f32) -> Option<f32> {
    (!blocked(solids, x, z, feet)).then(|| solids.floor(x, z, feet))
}

/// The squares a walker reaches from `from` without leaving `inside`
/// (x and z), each with the floor it stands on there, keyed by its grid
/// offset from `from`; only squares its body fits in. Each move goes to a
/// side neighbor. A body that overlaps something is pushed out of it, as
/// the controller does: it crosses up to [`WEDGED`] squares it overlaps.
#[must_use]
pub fn reachable(
    solids: &Solids,
    from: Vec3,
    inside: impl Fn(f32, f32) -> bool,
) -> HashMap<[i32; 2], f32> {
    let at = |k: [i32; 2]| (from.x + k[0] as f32 * CELL, from.z + k[1] as f32 * CELL);
    let start = u8::from(blocked(solids, from.x, from.z, from.y));
    let mut seen = HashMap::from([([0, 0], (from.y, start))]);
    let mut queue = VecDeque::from([[0, 0]]);
    while let Some(k) = queue.pop_front() {
        let (feet, wedged) = seen[&k];
        for d in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            let next = [k[0] + d[0], k[1] + d[1]];
            if seen.contains_key(&next) {
                continue;
            }
            let (x, z) = at(next);
            if !inside(x, z) {
                continue;
            }
            let step = if wedged == 0 {
                step_to(solids, x, z, feet).map(|floor| (floor, 0))
            } else {
                let floor = solids.floor(x, z, feet);
                if !blocked(solids, x, z, floor) {
                    Some((floor, 0))
                } else {
                    (wedged < WEDGED).then_some((floor, wedged + 1))
                }
            };
            if let Some(step) = step {
                seen.insert(next, step);
                queue.push_back(next);
            }
        }
    }
    seen.into_iter()
        .filter(|(_, (_, wedged))| *wedged == 0)
        .map(|(k, (feet, _))| (k, feet))
        .collect()
}

/// Whether a player standing at `at` can't walk [`ESCAPE`] m away.
#[must_use]
pub fn trapped(solids: &Solids, at: Vec3) -> bool {
    let near = |x: f32, z: f32| (x - at.x).hypot(z - at.z) <= ESCAPE;
    let reach = ESCAPE - 2.0 * CELL;
    !reachable(solids, at, near)
        .keys()
        .any(|k| (k[0] as f32 * CELL).hypot(k[1] as f32 * CELL) >= reach)
}

/// The nearest place to `at` a player stands free: on a floor at most
/// [`CLIMB_OUT`] m above the feet, where the body fits and from which it
/// isn't [`trapped`]. `None` when there is none within [`SEARCH`] m.
#[must_use]
pub fn rescue(solids: &Solids, at: Vec3) -> Option<Vec3> {
    let n = (SEARCH / CELL) as i32;
    let mut offsets: Vec<[i32; 2]> = (-n..=n)
        .flat_map(|i| (-n..=n).map(move |j| [i, j]))
        .filter(|&[i, j]| i * i + j * j <= n * n && (i, j) != (0, 0))
        .collect();
    offsets.sort_by_key(|&[i, j]| i * i + j * j);
    offsets.into_iter().find_map(|[i, j]| {
        let (x, z) = (at.x + i as f32 * CELL, at.z + j as f32 * CELL);
        let feet = solids.floor(x, z, at.y + CLIMB_OUT - STEP);
        let spot = Vec3::new(x, feet, z);
        (!blocked(solids, x, z, feet) && !trapped(solids, spot)).then_some(spot)
    })
}

/// Watches a walking player for being held in one place against walls,
/// and moves a [`trapped`] one to the place [`rescue`] finds.
#[derive(Clone, Copy, Debug, Default)]
pub struct Watch {
    /// Where the player started pressing to move, and for how long, s.
    held: Option<(Vec3, f32)>,
}

impl Watch {
    /// How long a press must hold the player in place before the check, s.
    pub const HOLD: f32 = 1.0;
    /// How far counts as the same place, m.
    pub const NEAR: f32 = 0.75;

    /// After a step that left the player at `at`: `pressing` says whether
    /// the input asked to move and `grounded` whether the player stands.
    /// Returns where to move the player when they're trapped.
    pub fn after_step(
        &mut self,
        solids: &Solids,
        at: Vec3,
        pressing: bool,
        grounded: bool,
        dt: f32,
    ) -> Option<Vec3> {
        if !pressing || !grounded {
            self.held = None;
            return None;
        }
        let (from, held) = match self.held {
            Some((from, held)) if from.distance(at) <= Self::NEAR => (from, held + dt),
            _ => (at, 0.0),
        };
        if held < Self::HOLD {
            self.held = Some((from, held));
            return None;
        }
        self.held = None;
        if trapped(solids, at) {
            rescue(solids, at)
        } else {
            None
        }
    }
}

#[cfg(test)]
#[path = "unstick_tests.rs"]
mod tests;
