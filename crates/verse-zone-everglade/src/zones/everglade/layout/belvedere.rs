//! The belvedere: a Greco-futurism loggia on the rising ground at the
//! west edge of Everglade (`docs/verse/greco-futurism.md`), where the west
//! trail climbs out of the town from Hearth Road's end. Its loggia opens
//! east through inlaid marble piers onto a terrace with benches, and looks
//! back down the slope along Hearth Road over the Lantern Quarter's roofs;
//! [`VIEW`] keeps that view clear of trees. Behind the loggia, an entry
//! court's colonnade frames a mahogany double door in a stepped bronze
//! surround, over a floor inlaid with a meander.
//!
//! `scripts/blender/greco_futurism.py` builds it as one model,
//! `generated/belvedere`, with a far level of detail. The trail leads up
//! its stair from the town, across the terrace and past the loggia on the
//! side walks, through the court, and down its back steps onward to the
//! woods. It blocks walking by the boxes of its `belvedere.footprint.json`,
//! its flat roofs are surfaces to land on, and, like every generated model,
//! it collides by its own triangles and breaks (`demolition::carve`).
//!
//! It is lit for a sunlit afternoon with a few lights for the evening:
//! lanterns at the stair's head, uplights washing the portal, and two
//! sconces in the loggia, given to the stage only near it, in its free
//! slots ([`light`]).

use super::Placement;
use super::estate::Fixture;
use super::generated::{GableRoof, Instance, Model};
use crate::pbr::{Lamp, MAX_LAMPS, Neon};
use glam::Vec3;
use std::f32::consts::FRAC_PI_2;

/// The belvedere, in its glTF frame: +z out of the loggia toward the view,
/// the origin on the ground at the center of the stair's lowest step.
pub const BELVEDERE: Model = Model {
    name: "generated/belvedere",
    blocks: &[
        // The stair's cheek walls, the terrace's parapets, its benches,
        // urn trees, and lantern posts, and the side walks' parapets.
        [-3.0, -2.6, -8.0, 0.0, 3.8],
        [2.6, 3.0, -8.0, 0.0, 3.8],
        [-9.5, -3.0, -8.4, -8.0, 4.25],
        [3.0, 9.5, -8.4, -8.0, 4.25],
        [-9.5, -9.1, -32.8, -8.4, 4.59],
        [9.1, 9.5, -32.8, -8.4, 4.59],
        [-8.9, -5.7, -10.1, -9.4, 4.2],
        [5.7, 8.9, -10.1, -9.4, 4.2],
        [-9.02, -8.18, -9.27, -8.43, 5.7],
        [8.18, 9.02, -9.27, -8.43, 5.7],
        [-4.02, -3.18, -15.62, -14.78, 5.5],
        [3.18, 4.02, -15.62, -14.78, 5.5],
        [-3.62, -3.38, -8.87, -8.63, 5.4],
        [3.38, 3.62, -8.87, -8.63, 5.4],
        // The loggia's piers, its side walls, and its back wall.
        [-7.7, -6.5, -17.9, -16.7, 8.04],
        [-3.0, -1.8, -17.9, -16.7, 8.04],
        [1.8, 3.0, -17.9, -16.7, 8.04],
        [6.5, 7.7, -17.9, -16.7, 8.04],
        [-7.6, -7.1, -26.0, -17.8, 10.04],
        [7.1, 7.6, -26.0, -17.8, 10.04],
        [-7.6, 7.6, -26.0, -25.4, 10.04],
        // The loggia's bench under the relief, and its urn trees.
        [6.4, 7.1, -23.4, -19.4, 4.54],
        [-6.32, -5.48, -19.12, -18.28, 6.24],
        [5.48, 6.32, -19.12, -18.28, 6.24],
        // The court's colonnade and the potted trees by the portal.
        [-7.23, -6.37, -33.03, -32.17, 9.04],
        [-2.83, -1.97, -33.03, -32.17, 9.04],
        [1.97, 2.83, -33.03, -32.17, 9.04],
        [6.37, 7.23, -33.03, -32.17, 9.04],
        [-3.85, -2.95, -26.9, -26.0, 5.24],
        [2.95, 3.85, -26.9, -26.0, 5.24],
    ],
    roofs: &[
        GableRoof {
            center: [0.0, -21.4],
            slopes_z: true,
            half: [5.0, 8.0],
            eave: 10.49,
            ridge: 10.5,
        },
        GableRoof {
            center: [0.0, -29.3],
            slopes_z: true,
            half: [3.9, 8.0],
            eave: 10.04,
            ridge: 10.05,
        },
    ],
    front: [0.0, 1.0],
    inside: Some(INSIDE),
};

/// The middle of the loggia, in the belvedere's frame.
pub const INSIDE: [f32; 2] = [0.0, -21.6];
/// The court's back edge, where its steps go down to the trail, in the
/// belvedere's frame.
pub const BACK: [f32; 2] = [0.0, -34.2];
/// The terrace's height over the belvedere's ground, m...
pub const TERRACE: f32 = 3.3;
/// ...and the loggia's and the court's.
pub const FLOOR: f32 = 3.64;

/// Where the belvedere stands: its stair's foot on the west trail, where
/// the trail climbs out of the town, facing east back down it.
pub const BELVEDERE_AT: Instance =
    Instance::new("belvedere", &BELVEDERE, [-164.0, -8.0], FRAC_PI_2);

/// The view kept clear of trees from the terrace down to Hearth Road's
/// end: center and half extents, m.
pub const VIEW: ([f32; 2], [f32; 2]) = ([-141.0, -8.0], [23.0, 12.0]);

/// The belvedere's lights, in its frame: x, height above its base, and z,
/// m, as `belvedere.footprint.json`'s `lights`.
pub const LIGHTS: [(Fixture, [f32; 3]); 6] = [
    (Fixture::Lantern, [-3.5, 5.12, -8.75]),
    (Fixture::Lantern, [3.5, 5.12, -8.75]),
    (Fixture::Uplight, [-2.7, 4.54, -26.3]),
    (Fixture::Uplight, [2.7, 4.54, -26.3]),
    (Fixture::Sconce, [-2.4, 6.31, -25.14]),
    (Fixture::Sconce, [2.4, 6.31, -25.14]),
];

/// The sconces' flames, in the belvedere's frame, for their halos.
const FLAMES: [[f32; 3]; 2] = [[-2.4, 6.296, -25.14], [2.4, 6.296, -25.14]];

/// Where the belvedere's lights reach the player at all, m.
const REACH: f32 = 60.0;

/// A point of the belvedere's frame, x, height, and z, in the world.
fn world(local: [f32; 3]) -> Vec3 {
    let [x, z] = BELVEDERE_AT.world([local[0], local[2]]);
    let [ax, az] = BELVEDERE_AT.at;
    Vec3::new(x, super::height(ax, az) + local[1], z)
}

/// The belvedere's lot in the world: the extent of its blocks and its
/// stair, x and z, min and max, m.
#[must_use]
pub fn lot() -> ([f32; 2], [f32; 2]) {
    let (mut min, mut max) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
    let corners = BELVEDERE
        .blocks
        .iter()
        .flat_map(|&[x0, x1, z0, z1, _]| [[x0, z0], [x1, z1]])
        .chain([[0.0, 0.0], BACK]);
    for c in corners {
        let [x, z] = BELVEDERE_AT.world(c);
        min = [min[0].min(x), min[1].min(z)];
        max = [max[0].max(x), max[1].max(z)];
    }
    (min, max)
}

/// Whether `model` is a tree, a copse, or a thicket: what would close the
/// view or reach through the walls.
fn tree(model: &str) -> bool {
    [
        "oak", "beech", "linden", "fir", "spruce", "pine", "birch", "poplar", "tree", "copse",
        "thicket", "snag",
    ]
    .iter()
    .any(|t| model.contains(t))
}

/// Drops from `out` whatever earlier rounds set on the belvedere's lot
/// (the trail's footpath pieces and the woods' undergrowth), the trees
/// close enough to reach through its walls, and the trees in its view.
pub fn clear(out: &mut Vec<Placement>) {
    let (min, max) = lot();
    let ([vx, vz], [hx, hz]) = VIEW;
    out.retain(|p| {
        let [x, z] = p.at;
        let within = |m: f32| x > min[0] - m && x < max[0] + m && z > min[1] - m && z < max[1] + m;
        if p.model == BELVEDERE.name {
            return true;
        }
        let view = (x - vx).abs() < hx && (z - vz).abs() < hz;
        !(within(1.0) || (tree(p.model) && (within(5.0) || view)))
    });
}

/// The belvedere's lamps that reach a player at `at` at `time`, s, the
/// nearest first, at most [`MAX_LAMPS`]; the sconces flicker. None when
/// the player is far from it.
#[must_use]
pub fn lamps(at: Vec3, time: f32) -> Vec<Lamp> {
    let center = world([0.0, 0.0, -20.0]);
    if Vec3::new(at.x - center.x, 0.0, at.z - center.z).length() > REACH {
        return Vec::new();
    }
    let mut lit: Vec<(f32, Lamp)> = LIGHTS
        .iter()
        .enumerate()
        .map(|(i, &(fixture, local))| {
            let (color, intensity, range) = fixture.light();
            let lamp = Lamp {
                position: world(local),
                color,
                intensity,
                range,
            };
            let lamp = if fixture.flame() {
                lamp.flickering(time, 120 + i as u32)
            } else {
                lamp
            };
            (lamp.position.distance_squared(at), lamp)
        })
        .collect();
    lit.sort_by(|a, b| a.0.total_cmp(&b.0));
    lit.into_iter()
        .take(MAX_LAMPS)
        .map(|(_, lamp)| lamp)
        .collect()
}

/// Lights Everglade's stage `neon` for a player at `at`: the belvedere's
/// lamps in its free slots. Its look is the afternoon's; nothing else
/// changes.
pub fn light(neon: &mut Neon, at: Vec3, time: f32) {
    let mut lamps = lamps(at, time).into_iter();
    for slot in neon.lamps.iter_mut().filter(|lamp| !lamp.lit()) {
        match lamps.next() {
            Some(lamp) => *slot = lamp,
            None => break,
        }
    }
}

/// Where the sconces' flames burn, in the world, for their halos.
#[must_use]
pub fn flames() -> Vec<Vec3> {
    FLAMES.iter().copied().map(world).collect()
}

/// The loggia's floor height in the world, m.
#[must_use]
pub fn floor() -> f32 {
    super::height(BELVEDERE_AT.at[0], BELVEDERE_AT.at[1]) + FLOOR
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zones::everglade::layout::{segment_distance, trails};

    #[test]
    fn the_belvedere_stands_on_high_ground_at_the_west_trails_climb() {
        // The west trail reaches its stair's foot and leaves from its
        // court's back steps.
        assert!(trails::distance(BELVEDERE_AT.at[0], BELVEDERE_AT.at[1]) < trails::HALF);
        let [bx, bz] = BELVEDERE_AT.world(BACK);
        assert!(trails::distance(bx, bz) < trails::HALF + 1.0, "{bx}, {bz}");
        // It faces east, back down the trail to Hearth Road's end.
        let out = BELVEDERE_AT.outward();
        assert!(out[0] > 0.99, "{out:?}");
        let road = super::super::PAVED[3];
        assert!((road.0[1] - BELVEDERE_AT.at[1]).abs() < 0.01);
        // It stands outside the flat clearing, on rising ground, so the
        // loggia's floor looks down on the town.
        let [ax, az] = BELVEDERE_AT.at;
        assert!(ax.hypot(az) > verse_world::social::everglade::CLEARING_RADIUS + 20.0);
        let below = super::super::height(road.0[0], road.0[1]);
        assert!(floor() > below + 6.0, "{} over {below}", floor());
        // The ground stays under its floors: the terrace's at the front,
        // the court's at the back.
        let base = super::super::height(ax, az);
        for (corner, top) in [
            ([-9.5, -8.0], TERRACE),
            ([9.5, -8.0], TERRACE),
            ([-9.5, BACK[1]], FLOOR),
            ([9.5, BACK[1]], FLOOR),
        ] {
            let [x, z] = BELVEDERE_AT.world(corner);
            let ground = super::super::height(x, z) - base;
            assert!(ground < top - 0.1, "{corner:?}: {ground} under {top}");
        }
        // No road runs under its blocks, and it keeps well away from the
        // owner's house, the Civic Hall, Alice, and the spawn.
        for (f, _) in BELVEDERE_AT.blocks() {
            for &(a, b, half) in super::super::roads() {
                let center = [(f.min[0] + f.max[0]) / 2.0, (f.min[1] + f.max[1]) / 2.0];
                for [x, z] in [f.min, f.max, center] {
                    assert!(segment_distance(a, b, x, z) > half, "{f:?} on {a:?}..{b:?}");
                }
            }
        }
        let spawn = verse_world::social::everglade::SPAWN;
        for [x, z] in [
            super::super::estate::OWNERS_HOUSE.at,
            super::super::civic::CIVIC.at,
            [spawn.x, spawn.z],
        ] {
            assert!((x - ax).hypot(z - az) > 150.0);
        }
    }

    #[test]
    fn the_walk_from_the_stair_into_the_loggia_stays_clear() {
        // Up the stair, across the terrace, over the threshold, and
        // through the middle bay between the inlaid piers, a walk 2.5 m
        // wide crosses no block.
        let half = 1.25;
        let (foot, inside) = (BELVEDERE.front[1], INSIDE[1]);
        for &[x0, x1, z0, z1, _] in BELVEDERE.blocks {
            let across = x1 > -half && x0 < half;
            let along = z1 > inside && z0 < foot;
            assert!(!(across && along), "{:?} blocks the walk", [x0, x1, z0, z1]);
        }
        // And the side walks lead past the loggia to the court, 1.5 m
        // wide between its walls and the parapets.
        for &[x0, x1, z0, z1, _] in BELVEDERE.blocks {
            for (a, b) in [(7.65, 9.05), (-9.05, -7.65)] {
                let on = x1 > a && x0 < b && z1 > -32.0 && z0 < -18.0;
                assert!(!on, "{:?} closes a side walk", [x0, x1, z0, z1]);
            }
        }
    }

    #[test]
    fn the_belvedere_lights_only_near_it_on_its_lot() {
        assert!(lamps(Vec3::new(0.0, 0.0, -20.0), 0.0).is_empty());
        let inside = world([INSIDE[0], FLOOR + 0.2, INSIDE[1]]);
        let all = lamps(inside, 1.0);
        assert_eq!(all.len(), LIGHTS.len());
        assert!(all.iter().all(Lamp::lit));
        let (min, max) = lot();
        for lamp in &all {
            let p = lamp.position;
            assert!(
                p.x > min[0] && p.x < max[0] && p.z > min[1] && p.z < max[1],
                "{p}"
            );
        }
        // Its look is the afternoon's: only free slots fill.
        let mut neon = Neon::plaza(0.0);
        let before = neon.grade;
        light(&mut neon, inside, 0.0);
        assert_eq!(neon.grade, before);
        assert!(neon.lamps.iter().filter(|lamp| lamp.lit()).count() <= MAX_LAMPS);
    }

    #[test]
    fn nothing_else_stands_on_the_lot_and_no_tree_closes_the_view() {
        let (min, max) = lot();
        let ([vx, vz], [hx, hz]) = VIEW;
        for p in super::super::placements() {
            let [x, z] = p.at;
            if p.model == BELVEDERE.name {
                continue;
            }
            let on = x > min[0] && x < max[0] && z > min[1] && z < max[1];
            assert!(!on, "{} at {x}, {z}", p.model);
            let near = x > min[0] - 4.5 && x < max[0] + 4.5 && z > min[1] - 4.5 && z < max[1] + 4.5;
            let view = (x - vx).abs() < hx && (z - vz).abs() < hz;
            assert!(
                !(tree(p.model) && (near || view)),
                "{} at {x}, {z}",
                p.model
            );
        }
        // The view runs from the stair's foot to Hearth Road's end.
        let road = super::super::PAVED[3];
        assert!(vx + hx >= BELVEDERE_AT.at[0] - 0.5 && vx - hx <= road.0[0]);
    }
}
