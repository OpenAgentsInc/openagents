//! The Civic Hall: Everglade's seat of government, the Hall of the
//! Commons, in the Greco-futurism style (`docs/verse/greco-futurism.md`).
//! It stands at the east end of Main Street, facing west down the town's
//! longest street, so the street's view ends at its portico; a cobbled
//! plaza ([`PLAZA`]) leads to its stair, with the east woods behind it.
//!
//! `scripts/blender/greco_futurism.py` builds it as one model,
//! `generated/civic_hall`, with a far level of detail: a broad two-storey
//! limestone building on a podium, a projecting pavilion behind four
//! columns on high plinths, a deep dentil cornice under a stepped attic,
//! paired tall windows on its wings, and a very tall copper portal with a
//! circuit relief whose small inset door stands open. Inside, the council
//! chamber holds a ring of tiered benches round a well with a copper seal
//! in its floor, and the speaker's dais under the seal on the back wall.
//!
//! It stands on open ground like the owner's house
//! ([`super::estate`]): its walls, columns, planter walls, tiers, and
//! furniture block walking by the boxes of its `civic_hall.footprint.json`,
//! its flat roofs are surfaces to land on, and, like every generated model,
//! it collides by its own triangles and breaks (`demolition::carve`).
//!
//! The chamber is lit like the owner's great room: candles, sconces, and
//! braziers in pools of light, with lanterns and uplights outside. [`light`]
//! gives Everglade's stage the fixtures' point lights near the player, those
//! on the player's side of the walls first, then the nearest, and the
//! great room's moodier grade while the player is in the chamber.

use super::estate::{Fixture, ROOM_GRADE};
use super::generated::{GableRoof, Instance, Model};
use crate::pbr::{Lamp, MAX_LAMPS, Neon};
use glam::{Quat, Vec3};
use std::f32::consts::FRAC_PI_2;

/// The Civic Hall, in its glTF frame: +z out of the front, the origin on
/// the ground at the center of the lowest step's front edge.
pub const CIVIC_HALL: Model = Model {
    name: "generated/civic_hall",
    blocks: &[
        // The stepped walnut walls beside the stair, and the long planters
        // along the podium's foot.
        [-9.6, -7.2, -2.4, 0.0, 0.75],
        [-9.6, -7.2, -4.8, -2.4, 1.55],
        [-16.6, -9.6, -4.8, -3.1, 1.25],
        [7.2, 9.6, -2.4, 0.0, 0.75],
        [7.2, 9.6, -4.8, -2.4, 1.55],
        [9.6, 16.6, -4.8, -3.1, 1.25],
        // The four columns on their plinths.
        [-7.71, -6.1, -7.21, -5.6, 10.32],
        [-3.71, -2.09, -7.21, -5.6, 10.32],
        [2.09, 3.71, -7.21, -5.6, 10.32],
        [6.1, 7.71, -7.21, -5.6, 10.32],
        // The pavilion on each side of the portal, the wings' fronts, the
        // side walls, the chamber's side walls, and the back wall.
        [-8.6, -2.5, -10.0, -8.4, 11.22],
        [2.5, 8.6, -10.0, -8.4, 11.22],
        [-17.6, -8.6, -10.0, -9.6, 11.22],
        [8.6, 17.6, -10.0, -9.6, 11.22],
        [-18.0, -17.6, -30.4, -9.6, 11.22],
        [17.6, 18.0, -30.4, -9.6, 11.22],
        [-11.0, -10.6, -30.0, -10.0, 11.22],
        [10.6, 11.0, -30.0, -10.0, 11.22],
        [-18.0, 18.0, -30.4, -30.0, 11.22],
        // The lantern posts at the stair's foot.
        [-8.52, -8.28, 0.48, 0.72, 2.3],
        [8.28, 8.52, 0.48, 0.72, 2.3],
        // The ring of tiered benches, three boxes to each half, open on
        // the entry axis and behind, at the dais.
        [-7.34, -3.43, -21.81, -17.39, 3.8],
        [-7.0, -2.19, -18.52, -13.78, 3.8],
        [-7.0, -2.19, -25.42, -20.68, 3.8],
        [3.43, 7.34, -21.81, -17.39, 3.8],
        [2.19, 7.0, -18.52, -13.78, 3.8],
        [2.19, 7.0, -25.42, -20.68, 3.8],
        // The speaker's lectern and chair on the dais, and the braziers.
        [-0.25, 0.25, -25.15, -24.65, 3.6],
        [-0.46, 0.46, -27.04, -26.4, 4.49],
        [-3.75, -2.85, -28.95, -28.05, 2.94],
        [2.85, 3.75, -28.95, -28.05, 2.94],
    ],
    roofs: &[
        GableRoof {
            center: [0.0, -20.0],
            slopes_z: true,
            half: [10.2, 17.8],
            eave: 12.42,
            ridge: 12.43,
        },
        GableRoof {
            center: [0.0, -9.8],
            slopes_z: true,
            half: [4.05, 8.05],
            eave: 13.97,
            ridge: 13.98,
        },
    ],
    front: [0.0, 1.0],
    inside: Some(MIDDLE),
};

/// The middle of the council ring's well, in the hall's frame.
pub const MIDDLE: [f32; 2] = [0.0, -19.6];

/// Where the hall stands: its stair's foot at Main Street's east end,
/// facing west down the street.
pub const CIVIC: Instance = Instance::new("civic hall", &CIVIC_HALL, [104.0, 46.0], -FRAC_PI_2);

/// The cobbled plaza before the stair, at Main Street's end: center and
/// half extents, m.
pub const PLAZA: ([f32; 2], [f32; 2]) = ([101.0, 46.0], [4.0, 15.0]);

/// The ground kept clear round the hall, so no tree's crown reaches
/// through its walls: center and half extents, m. It runs 5 to 7 m past
/// the walls on each side and behind, from the podium's front; trees
/// beside the plaza still frame the stair.
pub const CLEAR: ([f32; 2], [f32; 2]) = ([123.0, 46.0], [17.0, 25.0]);

/// The hall's lights, in its frame: x, height above its base, and z, m,
/// as `civic_hall.footprint.json`'s `lights`.
pub const LIGHTS: [(Fixture, [f32; 3]); 19] = [
    (Fixture::Lantern, [-8.4, 2.02, 0.6]),
    (Fixture::Uplight, [-3.6, 2.82, -8.15]),
    (Fixture::Lantern, [-10.2, 5.34, -9.22]),
    (Fixture::Lantern, [8.4, 2.02, 0.6]),
    (Fixture::Uplight, [3.6, 2.82, -8.15]),
    (Fixture::Lantern, [10.2, 5.34, -9.22]),
    (Fixture::Candles, [0.0, 4.055, -24.9]),
    (Fixture::Brazier, [-3.3, 3.29, -28.5]),
    (Fixture::Brazier, [3.3, 3.29, -28.5]),
    (Fixture::Sconce, [-10.34, 5.59, -16.5]),
    (Fixture::Sconce, [-10.34, 5.59, -23.5]),
    (Fixture::Sconce, [10.34, 5.59, -16.5]),
    (Fixture::Sconce, [10.34, 5.59, -23.5]),
    (Fixture::Sconce, [2.4, 4.79, -10.26]),
    (Fixture::Sconce, [-2.4, 4.79, -10.26]),
    (Fixture::Candles, [-9.5, 3.445, -11.2]),
    (Fixture::Candles, [-9.5, 3.445, -28.8]),
    (Fixture::Candles, [9.5, 3.445, -11.2]),
    (Fixture::Candles, [9.5, 3.445, -28.8]),
];

/// The flames, in the hall's frame, for their halos.
const FLAMES: [[f32; 3]; 23] = [
    [-0.15, 4.041, -24.9],
    [0.0, 4.101, -24.9],
    [0.15, 4.041, -24.9],
    [-3.3, 3.02, -28.5],
    [3.3, 3.02, -28.5],
    [-10.34, 5.576, -16.5],
    [-10.34, 5.576, -23.5],
    [10.34, 5.576, -16.5],
    [10.34, 5.576, -23.5],
    [2.4, 4.776, -10.26],
    [-2.4, 4.776, -10.26],
    [-9.7, 3.431, -11.2],
    [-9.5, 3.491, -11.2],
    [-9.3, 3.431, -11.2],
    [-9.7, 3.431, -28.8],
    [-9.5, 3.491, -28.8],
    [-9.3, 3.431, -28.8],
    [9.3, 3.431, -11.2],
    [9.5, 3.491, -11.2],
    [9.7, 3.431, -11.2],
    [9.3, 3.431, -28.8],
    [9.5, 3.491, -28.8],
    [9.7, 3.431, -28.8],
];

/// Where the hall's lights reach the player at all: its outside lights
/// within this distance of the hall, m...
const OUTDOOR_REACH: f32 = 70.0;
/// ...and the chamber's within this.
const INDOOR_REACH: f32 = 34.0;

/// The council chamber, in the hall's frame: x, z, and height above the
/// base from, to, m. Its floor is the podium's top, 1.94 m up; the range
/// starts lower, so a walker the podium hasn't lifted yet counts as inside.
pub const CHAMBER: ([f32; 2], [f32; 2], [f32; 2]) = ([-10.6, 10.6], [-30.0, -10.0], [-1.0, 10.5]);

/// The chamber's floor over the hall's ground, m.
pub const FLOOR: f32 = 1.94;

/// A point of the hall's frame, x, height, and z, in the world.
fn world(local: [f32; 3]) -> Vec3 {
    let [x, z] = CIVIC.world([local[0], local[2]]);
    let [ax, az] = CIVIC.at;
    Vec3::new(x, super::height(ax, az) + local[1], z)
}

/// A world point in the hall's frame, x, height, and z.
fn local(p: Vec3) -> [f32; 3] {
    let [ax, az] = CIVIC.at;
    let q = Quat::from_rotation_y(-CIVIC.yaw) * Vec3::new(p.x - ax, 0.0, p.z - az);
    [q.x, p.y - super::height(ax, az), q.z]
}

/// Whether `p` is inside the council chamber.
#[must_use]
pub fn in_chamber(p: Vec3) -> bool {
    let [x, y, z] = local(p);
    let ([x0, x1], [z0, z1], [y0, y1]) = CHAMBER;
    (x0..=x1).contains(&x) && (z0..=z1).contains(&z) && (y0..=y1).contains(&y)
}

/// The hall's lamps that reach a player at `at` at `time`, s: those on
/// the player's side of the walls first (the chamber's inside it, the
/// lanterns and uplights outside), then the nearest, at most
/// [`MAX_LAMPS`], the flames flickering. None when the player is far from
/// the hall.
#[must_use]
pub fn lamps(at: Vec3, time: f32) -> Vec<Lamp> {
    let center = world([0.0, 0.0, -18.0]);
    let away = Vec3::new(at.x - center.x, 0.0, at.z - center.z).length();
    let inside = in_chamber(at);
    let mut lit: Vec<((bool, f32), Lamp)> = LIGHTS
        .iter()
        .enumerate()
        .filter(|(_, (fixture, _))| {
            away < if fixture.indoors() {
                INDOOR_REACH
            } else {
                OUTDOOR_REACH
            }
        })
        .map(|(i, &(fixture, local))| {
            let (color, intensity, range) = fixture.light();
            let lamp = Lamp {
                position: world(local),
                color,
                intensity,
                range,
            };
            let lamp = if fixture.flame() {
                lamp.flickering(time, 80 + i as u32)
            } else {
                lamp
            };
            (
                (
                    fixture.indoors() != inside,
                    lamp.position.distance_squared(at),
                ),
                lamp,
            )
        })
        .collect();
    lit.sort_by(|a, b| a.0.0.cmp(&b.0.0).then(a.0.1.total_cmp(&b.0.1)));
    lit.into_iter()
        .take(MAX_LAMPS)
        .map(|(_, lamp)| lamp)
        .collect()
}

/// Lights Everglade's stage `neon` for a player at `at`: the hall's lamps
/// in its free slots, and the chamber's grade while the player is in it.
/// A stage far from the hall is left as it was.
pub fn light(neon: &mut Neon, at: Vec3, time: f32) {
    let mut lamps = lamps(at, time).into_iter();
    for slot in neon.lamps.iter_mut().filter(|lamp| !lamp.lit()) {
        match lamps.next() {
            Some(lamp) => *slot = lamp,
            None => break,
        }
    }
    if in_chamber(at) {
        neon.grade = ROOM_GRADE;
        neon.vignette = neon.vignette.max(0.32);
        neon.bloom = neon.bloom.max(0.07);
    }
}

/// Where the hall's flames burn, in the world, for their halos.
#[must_use]
pub fn flames() -> Vec<Vec3> {
    FLAMES.iter().copied().map(world).collect()
}

/// The chamber's floor height in the world, m.
#[must_use]
pub fn floor() -> f32 {
    super::height(CIVIC.at[0], CIVIC.at[1]) + FLOOR
}

/// Where a player stands at the foot of the stair, looking up at the
/// portico, and the heading, as the controller's yaw: the view the owner's
/// reference image frames.
#[must_use]
pub fn before_the_stair() -> ([f32; 2], f32) {
    let at = CIVIC.world([0.0, 16.0]);
    let toward = CIVIC.world(MIDDLE);
    (at, (toward[0] - at[0]).atan2(toward[1] - at[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hall's lot in its frame: its blocks' extent, x and z, m.
    fn lot() -> ([f32; 2], [f32; 2]) {
        let (mut min, mut max) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        for &[x0, x1, z0, z1, _] in CIVIC_HALL.blocks {
            min = [min[0].min(x0), min[1].min(z0)];
            max = [max[0].max(x1), max[1].max(z1)];
        }
        (min, max)
    }

    #[test]
    fn the_hall_closes_main_streets_view_on_flat_ground_off_every_road() {
        // Its stair's foot is Main Street's east end, on the street's axis,
        // and it faces west down the street.
        let main = super::super::PAVED[0];
        assert_eq!(CIVIC.front()[1], main.0[1]);
        assert!((CIVIC.at[0] - main.1[0]).abs() < 0.01);
        let out = CIVIC.outward();
        assert!(out[0] < -0.99, "{out:?}");
        // The plaza leads from the street to the stair.
        let ([px, pz], [hx, hz]) = PLAZA;
        assert!((pz - main.0[1]).abs() < 0.01 && px + hx >= CIVIC.at[0]);
        // Its front is inside the flat clearing, and the ground behind it
        // rises less than the podium.
        let at = CIVIC.at;
        assert!(at[0].hypot(at[1]) < verse_world::social::everglade::CLEARING_RADIUS);
        let ([x0, z0], [x1, _]) = lot();
        for corner in [[x0, z0], [x1, z0]] {
            let [x, z] = CIVIC.world(corner);
            let rise = super::super::height(x, z);
            assert!(rise < FLOOR - 0.3, "{x}, {z}: {rise}");
        }
        // No road or walk runs under its blocks.
        for (f, _) in CIVIC.blocks() {
            for &(a, b, half) in super::super::roads() {
                let corners = [f.min, f.max, [f.min[0], f.max[1]], [f.max[0], f.min[1]]];
                let center = [(f.min[0] + f.max[0]) / 2.0, (f.min[1] + f.max[1]) / 2.0];
                for [x, z] in corners.into_iter().chain([center]) {
                    let d = super::super::segment_distance(a, b, x, z);
                    assert!(d > half, "{f:?} on the road {a:?}..{b:?}");
                }
            }
        }
        // The doorway leads well inside, past the portico and the portal.
        let middle = CIVIC.world(MIDDLE);
        assert!(middle[0] > CIVIC.front()[0] + 18.0);
    }

    #[test]
    fn the_entry_axis_stays_clear_from_the_street_to_the_middle() {
        // From the stair's foot, between the inner columns, through the
        // portal's inset door, and across the ring's front aisle to the
        // well's middle, a walk 2.5 m wide crosses no block.
        let half = 1.25;
        let (foot, middle) = (CIVIC_HALL.front[1], MIDDLE[1]);
        for &[x0, x1, z0, z1, _] in CIVIC_HALL.blocks {
            let across = x1 > -half && x0 < half;
            let along = z1 > middle && z0 < foot;
            assert!(
                !(across && along),
                "{:?} blocks the entry",
                [x0, x1, z0, z1]
            );
        }
    }

    #[test]
    fn the_hall_lights_only_near_it_and_inside_within_the_lamp_limit() {
        // Far off in the town, nothing.
        assert!(lamps(Vec3::new(0.0, 0.0, -20.0), 0.0).is_empty());
        // On Main Street before it, the lanterns and uplights, not the
        // chamber.
        let street = world([0.0, 0.0, 30.0]);
        let outside = lamps(street, 0.0);
        let out = LIGHTS.iter().filter(|(f, _)| !f.indoors()).count();
        assert_eq!(outside.len(), out);
        // In the chamber, every fixture, the chamber's own first.
        let inside = world([MIDDLE[0], FLOOR + 0.2, MIDDLE[1]]);
        assert!(in_chamber(inside) && !in_chamber(street));
        let all = lamps(inside, 3.0);
        assert_eq!(all.len(), LIGHTS.len());
        assert!(all.len() <= MAX_LAMPS);
        let room = LIGHTS.len() - out;
        for lamp in &all[..room] {
            assert!(in_chamber(lamp.position), "{:?}", lamp.position);
        }
        assert!(all.iter().all(Lamp::lit));
        // Every outside light hangs on the hall or stands on its lot.
        let ([x0, z0], [x1, z1]) = lot();
        for (fixture, [x, _, z]) in LIGHTS {
            if !fixture.indoors() {
                let on = (x0 - 0.5..=x1 + 0.5).contains(&x) && (z0..=z1 + 0.5).contains(&z);
                assert!(on, "{fixture:?} at {x}, {z}");
            }
        }
        // The grade changes only in the chamber.
        let mut neon = Neon::plaza(0.0);
        let before = neon.grade;
        light(&mut neon, street, 0.0);
        assert_eq!(neon.grade, before);
        light(&mut neon, inside, 0.0);
        assert_eq!(neon.grade, ROOM_GRADE);
    }

    #[test]
    fn the_hall_and_the_owners_house_never_light_together_past_the_limit() {
        // Between them, each fills only the stage's free slots.
        let between = Vec3::new(110.0, 0.0, 10.0);
        let mut neon = Neon::plaza(0.0);
        super::super::estate::light(&mut neon, between, 0.0);
        light(&mut neon, between, 0.0);
        let lit = neon.lamps.iter().filter(|lamp| lamp.lit()).count();
        assert!(lit <= MAX_LAMPS);
        assert!(lit > 0);
    }

    #[test]
    fn nothing_else_stands_on_the_lot() {
        // Every other placement keeps off the hall's footprint, so none
        // pokes through its floors or closes its doorway.
        let ([x0, z0], [x1, z1]) = lot();
        let corners = [[x0, z0], [x1, z0], [x1, z1], [x0, z1]].map(|c| CIVIC.world(c));
        let (min, max) = corners.iter().fold(
            ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]),
            |(lo, hi), [x, z]| {
                (
                    [lo[0].min(*x), lo[1].min(*z)],
                    [hi[0].max(*x), hi[1].max(*z)],
                )
            },
        );
        for p in super::super::placements() {
            let [x, z] = p.at;
            let on = x > min[0] && x < max[0] && z > min[1] && z < max[1];
            assert!(!on || p.model == CIVIC_HALL.name, "{} at {x}, {z}", p.model);
        }
        // And no tree grows close enough for its crown to reach through
        // the walls: the town's foliage keeps off the clear ground
        // round the hall, and every tree stands at least 4.5 m from its
        // sides and back.
        let ([cx, cz], [hx, hz]) = CLEAR;
        assert!(cx + hx >= max[0] + 5.0);
        assert!(cz - hz <= min[1] - 6.0 && cz + hz >= max[1] + 6.0);
        let trees = [
            "oak", "beech", "linden", "fir", "spruce", "pine", "birch", "poplar", "tree", "copse",
        ];
        let reach = 4.5;
        for p in super::super::placements() {
            let [x, z] = p.at;
            let near =
                x > min[0] + 6.0 && x < max[0] + reach && z > min[1] - reach && z < max[1] + reach;
            let tree = trees.iter().any(|t| p.model.contains(t));
            assert!(!(near && tree), "{} at {x}, {z}", p.model);
        }
    }
}
