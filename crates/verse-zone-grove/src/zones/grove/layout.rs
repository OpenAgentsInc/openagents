//! The Grove's placements: a meadow about 60 m across, ringed by the
//! nature kit's trees, bushes, rocks, and flowers from the pinned Everglade
//! pack, with no workshop. The training dummies are not placements: they
//! move, so they draw with the characters ([`super::draw`]).
//!
//! The druid's sacred grove stands around the field ([`sacred`]): a circle
//! of standing and rune stones, a great oak with lanterns in its crown and
//! an altar beneath it, a campfire with its cauldron, braziers along the
//! path from the arch, torches on posts, archery butts, a sparring ring,
//! a stone path, and mushroom rings and wildflowers. Its fires and glows
//! are listed in [`fires`], which light the dusk ([`super::light`]).

use super::super::everglade::layout::{
    Collision, GRASS, PLANTS, Placement, RING_TREES, TREES, noise,
};
use super::{MEADOW_RADIUS, RETURN_PORTAL, dummies::FIELD};
use std::f32::consts::TAU;

/// Grass clumps scattered in the meadow.
const GRASS_CLUMPS: usize = 40;
/// Flower groups around the meadow's edge.
const FLOWERS: u32 = 18;
/// Plants at the meadow's edge.
const EDGE_PLANTS: usize = 14;
/// The concrete tower that Meteor Swarm and the Thunderbolt break and
/// topple: in the field past the dummies, in plain view from the spawn,
/// its door toward it.
pub const TOWER: [f32; 2] = [-4.0, 21.0];
pub const TOWER_MODEL: &str = "generated/concrete_tower";

/// Every placement in the Grove.
#[must_use]
pub fn placements() -> Vec<Placement> {
    let mut out = Vec::with_capacity(160);
    // The same tree ring as Everglade's, so the Grove sits in its forest.
    for k in 0..RING_TREES {
        let angle = (k as f32 + 0.4 * noise(k, 1)) / RING_TREES as f32 * TAU;
        let r = 41.0 + (k % 3) as f32 * 5.0 + 2.0 * noise(k, 2);
        out.push(
            Placement::new(
                TREES[k as usize % TREES.len()],
                [angle.cos() * r, angle.sin() * r],
                noise(k, 3) * TAU,
                Collision::Core(0.4),
            )
            .scale(1.0 + 0.3 * noise(k, 4))
            .lift(-0.15),
        );
    }
    // A closer ring of bushes around the meadow, open toward the return
    // arch in the south.
    for i in 0..14_u32 {
        // From 290° around to 250°: the south (270°) stays open for the
        // arch.
        let degrees = 290.0 + i as f32 * (320.0 / 13.0) + 4.0 * (noise(i, 30) - 0.5);
        let angle = degrees.to_radians();
        let model = if i % 2 == 0 {
            "nature/Bush_Common"
        } else {
            "nature/Bush_Common_Flowers"
        };
        let r = MEADOW_RADIUS + 3.0 + 1.5 * noise(i, 31);
        out.push(
            Placement::new(
                model,
                [angle.cos() * r, angle.sin() * r],
                noise(i, 32) * TAU,
                Collision::Core(0.55),
            )
            .scale(1.1),
        );
    }
    for (i, (model, degrees)) in [
        ("nature/Rock_Medium_1", 35.0_f32),
        ("nature/Rock_Medium_3", 110.0),
        ("nature/Rock_Medium_2", 160.0),
        ("nature/Rock_Medium_1", 215.0),
        ("nature/Rock_Medium_3", 330.0),
    ]
    .into_iter()
    .enumerate()
    {
        let angle = degrees.to_radians();
        let r = MEADOW_RADIUS + 1.0;
        out.push(
            Placement::new(
                model,
                [angle.cos() * r, angle.sin() * r],
                noise(i as u32, 33) * TAU,
                Collision::Bounds,
            )
            .scale(0.6),
        );
    }
    for k in 0..FLOWERS {
        let angle = noise(k, 40) * TAU;
        let r = 22.0 + 8.0 * noise(k, 41);
        let at = [angle.cos() * r, angle.sin() * r];
        if !open(at) {
            continue;
        }
        let model = match k % 3 {
            0 => "nature/Flower_3_Group",
            1 => "nature/Flower_4_Group",
            _ => "nature/Fern_1",
        };
        let scale = if model == "nature/Fern_1" { 0.3 } else { 1.0 };
        out.push(Placement::new(model, at, noise(k, 42) * TAU, Collision::None).scale(scale));
    }
    scatter(
        &mut out,
        &GRASS.map(|m| (m, 1.0)),
        GRASS_CLUMPS,
        4.0,
        30.0,
        50,
    );
    scatter(&mut out, &PLANTS, EDGE_PLANTS, 26.0, 33.0, 51);
    out.push(Placement::new(
        TOWER_MODEL,
        TOWER,
        std::f32::consts::PI,
        Collision::None,
    ));
    sacred(&mut out);
    out
}

/// The great oak: north-east of the field, clear of the dummies and the
/// tower, its crown over the altar.
pub const OAK: [f32; 2] = [17.0, 14.0];
const OAK_SCALE: f32 = 1.45;
/// The altar under the oak, facing the field (west).
pub const ALTAR: [f32; 2] = [14.2, 7.6];
/// The campfire and its cauldron, west of the field.
pub const CAMPFIRE: [f32; 2] = [-15.0, -5.0];
/// The sparring ring's center, north-west of the field.
pub const RING: [f32; 2] = [-17.0, 15.0];
/// The standing stones' circle radius, m.
const STONES_RADIUS: f32 = 25.5;
/// Braziers: two flanking the path from the arch, two before the altar.
const BRAZIERS: [[f32; 2]; 4] = [[-2.6, -21.0], [2.6, -21.0], [12.0, 5.2], [12.0, 10.4]];
/// Torches on posts around the field.
const TORCHES: [[f32; 2]; 5] = [
    [-11.5, 2.5],
    [-10.0, 13.0],
    [10.5, -2.0],
    [6.0, 19.5],
    [-12.5, -11.0],
];
/// Lanterns hung under the oak's crown: offsets from the trunk, m.
const LANTERNS: [[f32; 2]; 4] = [[-3.6, -2.4], [-1.2, -4.2], [3.0, -3.0], [-4.2, 1.0]];
/// How high the lanterns hang, m.
const LANTERN_HEIGHT: f32 = 4.6;
/// Where fireflies drift at dusk: the field's edges, the oak, and the
/// stones, away from the dummies' line of fire.
pub const FIREFLIES: [[f32; 2]; 9] = [
    [-18.0, -2.0],
    [-14.0, 10.0],
    [-20.0, 18.0],
    [14.0, 18.0],
    [19.0, 9.0],
    [16.0, -6.0],
    [-6.0, -18.0],
    [7.0, -16.0],
    [3.0, 24.0],
];

/// What gives light among the placements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FireKind {
    Campfire,
    Brazier,
    Torch,
    Lantern,
    /// The altar's candles.
    Candles,
    /// A rune stone's carved glow.
    Runes,
}

/// A light among the placements: where its flame or glow is, and what it
/// is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fire {
    pub at: glam::Vec3,
    pub kind: FireKind,
}

/// The standing stones' bearings, degrees from +x toward +z, and whether
/// each carries runes. The south stays open toward the arch, and the
/// stones stand clear of the tower.
fn stones() -> Vec<(f32, bool)> {
    let tower = TOWER[1].atan2(TOWER[0]).to_degrees();
    (0..16)
        .map(|k| (k as f32 * 22.5 + 11.25, k % 3 == 1))
        .filter(|&(deg, _)| (deg - 270.0).abs() > 24.0 && (deg - tower).abs() > 14.0)
        .collect()
}

fn ground(at: [f32; 2]) -> f32 {
    super::super::everglade::height(at[0], at[1])
}

/// Every light among the sacred grove's placements.
#[must_use]
pub fn fires() -> Vec<Fire> {
    let at = |p: [f32; 2], y: f32| glam::Vec3::new(p[0], ground(p) + y, p[1]);
    let mut out = vec![
        Fire {
            at: at(CAMPFIRE, 0.45),
            kind: FireKind::Campfire,
        },
        Fire {
            at: at(ALTAR, 1.35),
            kind: FireKind::Candles,
        },
    ];
    out.extend(BRAZIERS.iter().map(|&p| Fire {
        at: at(p, 1.55),
        kind: FireKind::Brazier,
    }));
    out.extend(TORCHES.iter().map(|&p| Fire {
        at: at(p, 2.45),
        kind: FireKind::Torch,
    }));
    out.extend(LANTERNS.iter().map(|&[x, z]| Fire {
        at: at([OAK[0] + x, OAK[1] + z], LANTERN_HEIGHT + 0.3),
        kind: FireKind::Lantern,
    }));
    for (deg, runes) in stones() {
        if runes {
            let a = deg.to_radians();
            // The runes face the field: a little in from the stone.
            let r = STONES_RADIUS - 0.6;
            out.push(Fire {
                at: at([a.cos() * r, a.sin() * r], 0.85),
                kind: FireKind::Runes,
            });
        }
    }
    out
}

/// Yaw that turns a model's front (+z) toward `to` from `from`.
fn facing(from: [f32; 2], to: [f32; 2]) -> f32 {
    (to[0] - from[0]).atan2(to[1] - from[1])
}

/// The druid's sacred grove around the field.
fn sacred(out: &mut Vec<Placement>) {
    let center = [0.0, 0.0];
    for (k, (deg, runes)) in stones().into_iter().enumerate() {
        let a = deg.to_radians();
        let p = [a.cos() * STONES_RADIUS, a.sin() * STONES_RADIUS];
        let model = if runes {
            "generated/grove_rune_stone"
        } else {
            "generated/grove_standing_stone"
        };
        let tilt = (noise(k as u32, 60) - 0.5) * 0.3;
        out.push(
            Placement::new(model, p, facing(p, center) + tilt, Collision::Core(0.5))
                .scale(0.95 + 0.25 * noise(k as u32, 61))
                .lift(-0.05),
        );
    }
    // The great oak, the altar under it, and lanterns in its crown.
    out.push(
        Placement::new("generated/grove_oak", OAK, 2.4, Collision::Core(1.0))
            .scale(OAK_SCALE)
            .lift(-0.1),
    );
    out.push(Placement::new(
        "generated/grove_altar",
        ALTAR,
        facing(ALTAR, [0.0, ALTAR[1]]),
        Collision::Core(0.9),
    ));
    for (i, [x, z]) in LANTERNS.into_iter().enumerate() {
        out.push(
            Placement::new(
                "generated/grove_hanging_lantern",
                [OAK[0] + x, OAK[1] + z],
                noise(i as u32, 62) * 6.0,
                Collision::None,
            )
            .lift(LANTERN_HEIGHT),
        );
    }
    // Offerings of mushrooms and flowers around the oak's roots.
    for (i, (dx, dz)) in [(-2.6, 1.8), (2.4, -2.2), (0.6, 3.0)]
        .into_iter()
        .enumerate()
    {
        out.push(Placement::new(
            "generated/mushrooms",
            [OAK[0] + dx, OAK[1] + dz],
            noise(i as u32, 63) * 6.0,
            Collision::None,
        ));
    }
    out.push(Placement::new(
        "generated/wildflowers",
        [ALTAR[0] + 1.6, ALTAR[1] - 2.0],
        0.4,
        Collision::None,
    ));
    out.push(Placement::new(
        "generated/flower_bed",
        [ALTAR[0] + 1.4, ALTAR[1] + 2.2],
        1.3,
        Collision::None,
    ));
    // The campfire, its cauldron, and logs to sit on.
    out.push(Placement::new(
        "generated/grove_campfire",
        CAMPFIRE,
        0.3,
        Collision::Core(0.7),
    ));
    out.push(
        Placement::new(
            "props/Cauldron",
            [CAMPFIRE[0] + 1.5, CAMPFIRE[1] - 1.1],
            0.8,
            Collision::Core(0.45),
        )
        .scale(1.2),
    );
    out.push(Placement::new(
        "generated/fallen_log",
        [CAMPFIRE[0] - 2.4, CAMPFIRE[1] + 0.4],
        1.4,
        Collision::Bounds,
    ));
    for (p, yaw) in [([0.4, 2.4], 0.0), ([0.6, -2.5], 2.0)] {
        out.push(Placement::new(
            "generated/stump",
            [CAMPFIRE[0] + p[0], CAMPFIRE[1] + p[1]],
            yaw,
            Collision::Core(0.3),
        ));
    }
    for p in BRAZIERS {
        out.push(Placement::new(
            "generated/grove_brazier",
            p,
            0.0,
            Collision::Core(0.4),
        ));
    }
    for (i, p) in TORCHES.into_iter().enumerate() {
        out.push(Placement::new(
            "generated/grove_torch",
            p,
            noise(i as u32, 64) * 6.0,
            Collision::Core(0.15),
        ));
    }
    // Archery butts on the west side, facing the field.
    for p in [[-19.0, 2.0], [-19.5, 6.5]] {
        out.push(Placement::new(
            "generated/grove_archery_butt",
            p,
            facing(p, [0.0, p[1]]),
            Collision::Core(0.45),
        ));
    }
    // The sparring ring: four arcs, each bowing outward, with gaps between
    // them to step through. An arc's middle post stands `radius` from the
    // ring's center, on the side its bow faces (the model's -z).
    let radius = 5.15;
    for k in 0..4 {
        let yaw = k as f32 * std::f32::consts::FRAC_PI_2 + 0.3;
        let (s, c) = yaw.sin_cos();
        out.push(Placement::new(
            "generated/grove_training_ring",
            [RING[0] - s * radius, RING[1] - c * radius],
            yaw,
            Collision::None,
        ));
    }
    // A stone path from the arch to the spawn.
    let path = [
        "nature/RockPath_Round_Wide",
        "nature/RockPath_Round_Small_1",
        "nature/RockPath_Round_Thin",
        "nature/RockPath_Round_Small_2",
        "nature/RockPath_Round_Small_3",
    ];
    for i in 0..9_u32 {
        let z = RETURN_PORTAL.z + 4.0 + i as f32 * 1.45;
        let side = if i % 2 == 0 { -0.35 } else { 0.35 };
        let x = 0.35 * (noise(i, 65) - 0.5) + side;
        out.push(
            Placement::new(
                path[i as usize % path.len()],
                [x, z],
                noise(i, 66) * 6.0,
                Collision::None,
            )
            .lift(0.01),
        );
    }
    // Mushroom rings, mossy rocks, and wildflowers about the field.
    for (n, c) in [[7.0_f32, -12.5], [-9.0, 19.0]].into_iter().enumerate() {
        for k in 0..7 {
            let a = k as f32 / 7.0 * std::f32::consts::TAU + n as f32;
            out.push(
                Placement::new(
                    "generated/mushrooms",
                    [c[0] + a.cos() * 1.6, c[1] + a.sin() * 1.6],
                    a,
                    Collision::None,
                )
                .scale(0.7),
            );
        }
    }
    for (i, p) in [[-21.0, -14.0], [21.0, -8.0], [-6.0, 27.5], [20.0, 22.0]]
        .into_iter()
        .enumerate()
    {
        out.push(
            Placement::new(
                "generated/mossy_rock",
                p,
                noise(i as u32, 67) * 6.0,
                Collision::Core(0.6),
            )
            .scale(1.2),
        );
    }
    for (i, p) in [
        [-23.0, 9.0],
        [22.0, 2.0],
        [-8.5, -19.0],
        [9.0, -19.5],
        [4.0, 25.0],
    ]
    .into_iter()
    .enumerate()
    {
        out.push(Placement::new(
            "generated/wildflowers",
            p,
            noise(i as u32, 68) * 6.0,
            Collision::None,
        ));
    }
}

/// Whether ground cover may grow at `at`: clear of the dummies, the spawn,
/// and the arch.
fn open(at: [f32; 2]) -> bool {
    let near = |p: [f32; 2], r: f32| (p[0] - at[0]).hypot(p[1] - at[1]) < r;
    !FIELD.iter().any(|&(_, p)| near(p, 2.5))
        && !near([super::SPAWN.x, super::SPAWN.z], 2.0)
        && !near([RETURN_PORTAL.x, RETURN_PORTAL.z], 4.0)
        && !near(TOWER, 5.0)
        && !near(OAK, 3.0)
        && !near(ALTAR, 2.0)
        && !near(CAMPFIRE, 3.0)
        && !near(RING, 6.0)
        && !BRAZIERS.iter().chain(&TORCHES).any(|&p| near(p, 1.0))
}

/// Places `count` pieces of ground cover, cycling through `models`, at
/// hashed points of open ground between radii `near` and `far`.
fn scatter(
    out: &mut Vec<Placement>,
    models: &[(&'static str, f32)],
    count: usize,
    near: f32,
    far: f32,
    salt: u32,
) {
    let mut placed = 0;
    for n in 0..count as u32 * 16 {
        if placed == count {
            break;
        }
        let angle = noise(n, salt) * TAU;
        let r = near + (far - near) * noise(n, salt + 100).sqrt();
        let at = [angle.cos() * r, angle.sin() * r];
        if !open(at) {
            continue;
        }
        let (model, scale) = models[placed % models.len()];
        out.push(
            Placement::new(model, at, noise(n, salt + 200) * TAU, Collision::None)
                .scale(scale * (0.8 + 0.4 * noise(n, salt + 300))),
        );
        placed += 1;
    }
}
