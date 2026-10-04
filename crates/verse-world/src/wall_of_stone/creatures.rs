//! Creatures where the wall appears: pushed out of a panel's space to the
//! side the caster chooses, and a Dexterity save for a creature the wall
//! would enclose.
//!
//! These functions answer where a creature should go. The world moves it
//! there through the character sweep, so obstacles still stop it.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use super::validate::Stone;
use super::{Placement, SPEED, box_distance, ray_box};

/// Clearance left between a pushed creature and the panel, m.
pub const CLEARANCE: f64 = 0.02;
/// Horizontal directions checked for an enclosure and an exit.
pub const DIRECTIONS: usize = 16;
/// How far a ray looks for the wall around a creature, m.
pub const REACH: f64 = SPEED;

/// An upright capsule standing at `feet`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Creature {
    pub feet: DVec3,
    pub radius: f64,
    pub height: f64,
}

impl Creature {
    fn middle(&self) -> DVec3 {
        self.feet + DVec3::Y * (self.height * 0.5)
    }

    /// Whether the capsule overlaps the panel's box.
    #[must_use]
    pub fn overlaps(&self, panel: &Placement) -> bool {
        let (a, b) = (self.radius, self.height - self.radius);
        (0..=8).any(|i| {
            let p = self.feet + DVec3::Y * (a + (b - a) * f64::from(i) / 8.0);
            box_distance(panel.center, panel.orientation, panel.half(), p) < self.radius
        })
    }
}

/// Where a creature in the wall's space goes: out of every panel it
/// overlaps, along each panel's face normal on the side of `side` (the
/// caster's choice, a world direction). `None` when it overlaps nothing.
#[must_use]
pub fn push_out(panels: &[Placement], creature: Creature, side: DVec3) -> Option<DVec3> {
    let mut moved = creature;
    let mut pushed = false;
    for _ in 0..8 {
        let Some(panel) = panels.iter().find(|p| moved.overlaps(p)) else {
            break;
        };
        let mut normal = panel.normal();
        if panel.vertical() {
            normal = DVec3::new(normal.x, 0.0, normal.z).normalize_or_zero();
        }
        if normal.dot(side) < 0.0 {
            normal = -normal;
        }
        let depth = (moved.middle() - panel.center).dot(normal);
        let wanted = panel.half().z + moved.radius + CLEARANCE;
        let mut step = normal * (wanted - depth).max(CLEARANCE);
        if panel.vertical() {
            step.y = 0.0;
        }
        moved.feet += step;
        pushed = true;
    }
    pushed.then_some(moved.feet)
}

fn first_hit(panels: &[Placement], stone: &[Stone], origin: DVec3, dir: DVec3) -> Option<f64> {
    let panel_hits = panels
        .iter()
        .filter_map(|p| ray_box(p.center, p.orientation, p.half(), origin, dir));
    let stone_hits = stone
        .iter()
        .filter_map(|s| ray_box(s.center, s.orientation, s.half, origin, dir));
    panel_hits
        .chain(stone_hits)
        .map(|(near, _)| near.max(0.0))
        .filter(|&t| t <= REACH)
        .min_by(f64::total_cmp)
}

fn direction(i: usize) -> DVec3 {
    let angle = std::f64::consts::TAU * i as f64 / DIRECTIONS as f64;
    DVec3::new(angle.cos(), 0.0, angle.sin())
}

/// Whether the wall, with the stone around it, surrounds the creature on
/// all sides: every horizontal direction and straight up meet a panel or
/// stone. The floor closes the bottom.
#[must_use]
pub fn enclosed(panels: &[Placement], stone: &[Stone], creature: Creature) -> bool {
    let middle = creature.middle();
    let around = (0..DIRECTIONS).all(|i| first_hit(panels, stone, middle, direction(i)).is_some());
    around && first_hit(panels, stone, middle, DVec3::Y).is_some()
}

/// The shortest way out through the wall's footprint before it closes: the
/// nearest point beyond the panels in one of [`DIRECTIONS`] directions that
/// is clear of the panels and stone, with its distance, m.
#[must_use]
pub fn shortest_exit(
    panels: &[Placement],
    stone: &[Stone],
    creature: Creature,
) -> Option<(DVec3, f64)> {
    let middle = creature.middle();
    let mut best: Option<(DVec3, f64)> = None;
    for i in 0..DIRECTIONS {
        let dir = direction(i);
        let far = panels
            .iter()
            .filter_map(|p| ray_box(p.center, p.orientation, p.half(), middle, dir))
            .filter(|&(near, _)| near <= REACH)
            .map(|(_, far)| far)
            .fold(f64::NEG_INFINITY, f64::max);
        if !far.is_finite() {
            continue;
        }
        let distance = far + creature.radius + CLEARANCE;
        let feet = creature.feet + dir * distance;
        let out = Creature { feet, ..creature };
        let blocked = panels.iter().any(|p| out.overlaps(p))
            || stone.iter().any(|s| {
                let (a, b) = (out.radius + 0.05, out.height - out.radius);
                (0..=4).any(|k| {
                    let p = feet + DVec3::Y * (a + (b - a) * f64::from(k) / 4.0);
                    s.distance(p) < out.radius
                })
            });
        if !blocked && best.is_none_or(|(_, d)| distance < d) {
            best = Some((feet, distance));
        }
    }
    best
}

/// What happened to a creature the wall would enclose.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Enclosure {
    /// Not enclosed.
    Free,
    /// Made the Dexterity save and moved out with its reaction.
    Escaped {
        roll: i32,
        total: i32,
        to: DVec3,
        distance: f64,
    },
    /// Failed the save, or had no way out within its Speed.
    Trapped { roll: i32, total: i32 },
}

/// Resolve the SRD enclosure rule for one creature: if the wall would
/// surround it, it makes a Dexterity save (`roll` is the d20) against `dc`;
/// on a success it moves up to its Speed out along the shortest exit.
#[must_use]
pub fn resolve_enclosure(
    panels: &[Placement],
    stone: &[Stone],
    creature: Creature,
    roll: i32,
    dexterity: i32,
    dc: i32,
) -> Enclosure {
    if !enclosed(panels, stone, creature) {
        return Enclosure::Free;
    }
    let total = roll + dexterity;
    if total >= dc {
        if let Some((to, distance)) = shortest_exit(panels, stone, creature) {
            if distance <= SPEED {
                return Enclosure::Escaped {
                    roll,
                    total,
                    to,
                    distance,
                };
            }
        }
    }
    Enclosure::Trapped { roll, total }
}
