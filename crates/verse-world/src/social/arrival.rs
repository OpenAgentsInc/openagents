//! Where players arrive in a shared world, and how a moving player stops
//! against the others (#12406). Plain math with no zone types, so the
//! Grid, `/home`, and `/verse` share it.
//!
//! - [`pick_spawn`] chooses a free spot inside a [`SpawnArea`]: a sequence
//!   of candidates seeded by the player's key, the first one far enough
//!   from every known player and clear of the world's footprints. Two
//!   players who arrive at the same moment don't see each other yet, but
//!   their keys seed different sequences, so they land apart.
//! - [`sweep`] moves a player's feet from where they stood to where they
//!   are going, stopping at the first other player in the way and sliding
//!   along them, so no step is long enough to pass through anyone.
//!
//! Each player is a standing capsule of [`RADIUS`] and [`AVATAR_HEIGHT`],
//! resolved on the ground plane. `crates/physics` is a rigid-body solver;
//! a kinematic capsule against a handful of other capsules needs only this.

use glam::Vec3;

use super::controller::{AVATAR_HEIGHT, Footprint, RADIUS};

/// The smallest gap between two new players' feet, m: two radii plus room
/// to turn round.
pub const SPAWN_SPACING: f32 = RADIUS * 2.0 + 1.2;
/// How many candidates [`pick_spawn`] tries.
const CANDIDATES: u32 = 48;

/// A disc on the ground where new players appear, facing `focus`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpawnArea {
    /// The disc's center, `[x, z]`, m.
    pub center: [f32; 2],
    /// The disc's radius, m.
    pub radius: f32,
    /// What a new player faces, `[x, z]`, m.
    pub focus: [f32; 2],
}

impl SpawnArea {
    /// The Grid's: south of the plaza's center, facing it.
    pub const GRID: Self = Self {
        center: [0.0, -14.0],
        radius: 6.0,
        focus: [0.0, 0.0],
    };

    /// Whether `pos` is on the disc.
    #[must_use]
    pub fn contains(&self, pos: Vec3) -> bool {
        (pos.x - self.center[0]).hypot(pos.z - self.center[1]) <= self.radius + 1e-3
    }
}

/// A spawn: feet and facing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arrival {
    pub pos: Vec3,
    pub yaw: f32,
}

/// A free spot in `area` for the player whose key is `key`: the first of
/// the key's candidates at least [`SPAWN_SPACING`] from every one of
/// `known` (other players' feet) and clear of `blockers`. When none is,
/// the clear candidate farthest from everyone. The same key, area, and
/// players give the same spot.
#[must_use]
pub fn pick_spawn(key: &str, area: SpawnArea, known: &[Vec3], blockers: &[Footprint]) -> Arrival {
    let seed = fnv(key.as_bytes());
    let mut best: Option<(f32, Vec3)> = None;
    for i in 0..CANDIDATES {
        let a = unit(mix(seed, 2 * i));
        let b = unit(mix(seed, 2 * i + 1));
        // Uniform over the disc.
        let r = area.radius * a.sqrt();
        let angle = b * std::f32::consts::TAU;
        let pos = Vec3::new(
            area.center[0] + r * angle.cos(),
            0.0,
            area.center[1] + r * angle.sin(),
        );
        if blockers.iter().any(|f| f.contains(pos.x, pos.z, RADIUS)) {
            continue;
        }
        let gap = known
            .iter()
            .filter(|p| p.is_finite())
            .map(|p| (p.x - pos.x).hypot(p.z - pos.z))
            .fold(f32::INFINITY, f32::min);
        if gap >= SPAWN_SPACING {
            return arrival(pos, area);
        }
        if best.is_none_or(|(g, _)| gap > g) {
            best = Some((gap, pos));
        }
    }
    let pos = best.map_or(Vec3::new(area.center[0], 0.0, area.center[1]), |(_, p)| p);
    arrival(pos, area)
}

fn arrival(pos: Vec3, area: SpawnArea) -> Arrival {
    let (dx, dz) = (area.focus[0] - pos.x, area.focus[1] - pos.z);
    // Yaw zero faces +Z, counterclockwise-positive.
    let yaw = if dx.hypot(dz) > 1e-3 {
        dx.atan2(dz)
    } else {
        0.0
    };
    Arrival { pos, yaw }
}

/// Where feet moving from `from` toward `to` end up among `others` (other
/// players' feet), each a capsule of [`RADIUS`]: the step stops where it
/// first touches one, and what is left of it slides along that player.
/// A player already touching someone (they walked onto this one) is left
/// to [`super::controller::PlayerController::separate`]. `from.y` and
/// `to.y` carry through unchanged.
#[must_use]
pub fn sweep(from: Vec3, to: Vec3, others: &[Vec3]) -> Vec3 {
    let reach = RADIUS * 2.0;
    let mut at = from;
    let mut rest = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
    // A slide can meet a second player; three passes settle any corner.
    for _ in 0..3 {
        let length2 = rest.length_squared();
        if length2 < 1e-12 {
            break;
        }
        let mut first: Option<(f32, Vec3)> = None;
        for other in others {
            if !other.is_finite() || (other.y - from.y).abs() >= AVATAR_HEIGHT {
                continue;
            }
            let offset = Vec3::new(at.x - other.x, 0.0, at.z - other.z);
            let c = offset.length_squared() - reach * reach;
            let b = 2.0 * offset.dot(rest);
            // Already touching, or moving away: not this sweep's business.
            if c < 0.0 || b >= 0.0 {
                continue;
            }
            let disc = b * b - 4.0 * length2 * c;
            if disc < 0.0 {
                continue;
            }
            let t = (-b - disc.sqrt()) / (2.0 * length2);
            if (0.0..=1.0).contains(&t) && first.is_none_or(|(s, _)| t < s) {
                first = Some((t, *other));
            }
        }
        let Some((t, other)) = first else {
            at += rest;
            break;
        };
        at += rest * t;
        let normal = Vec3::new(at.x - other.x, 0.0, at.z - other.z).normalize_or_zero();
        let left = rest * (1.0 - t);
        // Drop the part that walks into them; keep the part along them.
        rest = left - normal * left.dot(normal).min(0.0);
        // A hair off the surface, so the next pass starts outside.
        at += normal * 1e-4;
    }
    Vec3::new(at.x, to.y, at.z)
}

/// FNV-1a over `bytes`.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// SplitMix64 of `seed` and `i`.
fn mix(seed: u64, i: u32) -> u64 {
    let mut z = seed.wrapping_add(u64::from(i).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// `[0, 1)` from the top 24 bits.
fn unit(bits: u64) -> f32 {
    (bits >> 40) as f32 / 16_777_216.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(i: usize) -> String {
        format!("{i:064x}")
    }

    #[test]
    fn spawns_are_inside_the_area_and_the_same_for_the_same_key() {
        let area = SpawnArea::GRID;
        for i in 0..200 {
            let a = pick_spawn(&key(i), area, &[], &[]);
            assert!(area.contains(a.pos), "{a:?}");
            assert_eq!(a, pick_spawn(&key(i), area, &[], &[]));
            // Facing the plaza: forward points at the focus.
            let forward = Vec3::new(a.yaw.sin(), 0.0, a.yaw.cos());
            let to = Vec3::new(area.focus[0], 0.0, area.focus[1]) - a.pos;
            assert!(forward.dot(to.normalize()) > 0.999, "{a:?}");
        }
    }

    #[test]
    fn a_spawn_never_lands_on_a_known_player() {
        let area = SpawnArea::GRID;
        let mut known = Vec::new();
        // Twelve arrivals in turn, each seeing the ones before.
        for i in 0..12 {
            let a = pick_spawn(&key(i), area, &known, &[]);
            for p in &known {
                assert!(a.pos.distance(*p) >= SPAWN_SPACING, "{i}: {a:?} vs {p:?}");
            }
            known.push(a.pos);
        }
        // A crowd standing everywhere: the farthest clear spot, not a stack.
        let crowd: Vec<Vec3> = (0..400)
            .map(|i| {
                let a = i as f32 * 2.4;
                let r = 6.0 * ((i % 20) as f32 / 20.0);
                Vec3::new(r * a.cos(), 0.0, -14.0 + r * a.sin())
            })
            .collect();
        let a = pick_spawn(&key(1), area, &crowd, &[]);
        assert!(area.contains(a.pos));
        assert!(crowd.iter().all(|p| p.distance(a.pos) > 0.05));
    }

    #[test]
    fn simultaneous_arrivals_spread_out_by_key() {
        let area = SpawnArea::GRID;
        let spots: Vec<Vec3> = (0..3)
            .map(|i| pick_spawn(&key(i + 40), area, &[], &[]).pos)
            .collect();
        for (i, a) in spots.iter().enumerate() {
            for b in &spots[i + 1..] {
                assert!(a.distance(*b) > 1.0, "{spots:?}");
            }
        }
    }

    #[test]
    fn spawns_avoid_footprints() {
        let area = SpawnArea::GRID;
        let wall = Footprint {
            min: [-6.0, -20.0],
            max: [0.0, -8.0],
        };
        for i in 0..50 {
            let a = pick_spawn(&key(i), area, &[], &[wall]);
            assert!(!wall.contains(a.pos.x, a.pos.z, RADIUS), "{a:?}");
        }
    }

    #[test]
    fn a_long_step_stops_at_the_player_instead_of_passing_through() {
        let other = Vec3::new(0.0, 0.0, 2.0);
        // Four meters in one step, straight through them.
        let end = sweep(Vec3::ZERO, Vec3::new(0.0, 0.0, 4.0), &[other]);
        assert!((end.z - (2.0 - RADIUS * 2.0)).abs() < 1e-3, "{end:?}");
        assert!(end.x.abs() < 1e-3);
    }

    #[test]
    fn a_glancing_step_slides_along_the_player() {
        let other = Vec3::new(0.3, 0.0, 2.0);
        let end = sweep(Vec3::ZERO, Vec3::new(0.0, 0.0, 4.0), &[other]);
        // It kept moving, around them, and never inside them.
        assert!(end.distance(other) >= RADIUS * 2.0 - 1e-3, "{end:?}");
        assert!(end.x < 0.0 && end.z > 2.0 - RADIUS * 2.0, "{end:?}");
    }

    #[test]
    fn pressing_into_a_player_holds_still_without_jitter() {
        let other = Vec3::new(0.0, 0.0, 2.0);
        let mut at = Vec3::ZERO;
        let mut path = Vec::new();
        for _ in 0..120 {
            at = sweep(at, at + Vec3::new(0.0, 0.0, 0.11), &[other]);
            path.push(at);
        }
        let last = path[path.len() - 1];
        for p in &path[60..] {
            assert!(p.distance(last) < 1e-3, "{p:?} {last:?}");
        }
        assert!(last.distance(other) >= RADIUS * 2.0 - 1e-3);
    }

    #[test]
    fn steps_away_and_past_are_free() {
        let other = Vec3::new(0.0, 0.0, 2.0);
        let away = sweep(
            Vec3::new(0.0, 0.0, 1.1),
            Vec3::new(0.0, 0.0, -3.0),
            &[other],
        );
        assert_eq!(away, Vec3::new(0.0, 0.0, -3.0));
        let past = sweep(Vec3::new(2.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 4.0), &[other]);
        assert_eq!(past, Vec3::new(2.0, 0.0, 4.0));
        // Someone a story above is no obstacle.
        let above = sweep(
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, 4.0),
            &[other + Vec3::Y * 3.0],
        );
        assert_eq!(above, Vec3::new(0.0, 0.0, 4.0));
    }
}
