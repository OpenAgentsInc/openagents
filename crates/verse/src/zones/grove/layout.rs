//! The Grove's placements: a meadow about 60 m across, ringed by the
//! nature kit's trees, bushes, rocks, and flowers from the pinned Everglade
//! pack, with no workshop. The training dummies are not placements: they
//! move, so they draw with the characters ([`super::draw`]).

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
    out
}

/// Whether ground cover may grow at `at`: clear of the dummies, the spawn,
/// and the arch.
fn open(at: [f32; 2]) -> bool {
    let near = |p: [f32; 2], r: f32| (p[0] - at[0]).hypot(p[1] - at[1]) < r;
    !FIELD.iter().any(|&(_, p)| near(p, 2.5))
        && !near([super::SPAWN.x, super::SPAWN.z], 2.0)
        && !near([RETURN_PORTAL.x, RETURN_PORTAL.z], 4.0)
        && !near(TOWER, 5.0)
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
