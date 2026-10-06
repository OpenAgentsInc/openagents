//! Everglade's placed characters (NPCs): characters the world places, not
//! bodies a player can choose. Today that is Alice, our original
//! explorer-druid (`docs/verse/female-character.md`), standing idle beside
//! the approach from the return arch to the workshop, turned toward
//! arrivals. She has no dialogue or behavior yet.
//!
//! An NPC draws like the town's creatures ([`super::wildlife`]): one of the
//! pack's forms on a still route, playing its idle clip. Each stands in a
//! small block of the solids, so nobody walks through her.

use glam::Vec3;

use super::height;
use super::wildlife::{Creature, Route};
use crate::controller::Footprint;
use crate::zones::everglade_pack::compile::ALICE_FORM;

/// Where Alice stands: west of the approach, clear of its path, its
/// stepping stones, and the flower beds that line it, a few strides north
/// of the spawn.
pub const ALICE_AT: [f32; 2] = [-4.6, -21.6];
/// Her heading: toward the approach and the return arch, so an arriving
/// player meets her face.
pub const ALICE_YAW: f32 = 2.6;
/// Half the side of the square she blocks, m.
pub const HALF: f32 = 0.3;
/// How tall her block stands, m: a person's height.
pub const TALL: f32 = 1.8;

/// The placed characters, as still creatures.
#[must_use]
pub fn creatures() -> Vec<Creature> {
    let [x, z] = ALICE_AT;
    vec![Creature {
        form: ALICE_FORM,
        route: Route::Sit {
            at: Vec3::new(x, height(x, z), z),
            yaw: ALICE_YAW,
        },
        scale: 1.0,
        phase: 0.0,
    }]
}

/// Each placed character's block: a footprint and its top.
#[must_use]
pub fn blocks() -> Vec<(Footprint, f32)> {
    let [x, z] = ALICE_AT;
    vec![(
        Footprint {
            min: [x - HALF, z - HALF],
            max: [x + HALF, z + HALF],
        },
        height(x, z) + TALL,
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alice_stands_off_the_approach_in_her_own_block() {
        let [x, z] = ALICE_AT;
        // Off the path the spawn walks up, and near enough to meet.
        assert!(x.abs() > 2.5);
        assert!(Vec3::new(x, 0.0, z).distance(Vec3::new(0.0, 0.0, -29.0)) < 10.0);
        let [(block, top)] = blocks()[..] else {
            panic!("one block");
        };
        assert!(block.min[0] < x && x < block.max[0]);
        assert!(block.min[1] < z && z < block.max[1]);
        assert!(top > height(x, z) + 1.5);
        let [alice] = &creatures()[..] else {
            panic!("one character");
        };
        assert_eq!(alice.form, "npc/alice");
        // She stands still, playing idle.
        let (a, b) = (alice.route.at(0.0), alice.route.at(30.0));
        assert_eq!((a.at, a.yaw, a.motion), (b.at, b.yaw, b.motion));
        assert_eq!(a.motion, super::super::player::Motion::Idle);
    }
}
