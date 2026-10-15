//! The lab's liquid tank (#11660): a glass tank on a stand at the head of
//! the beach where a column of water is let go every few seconds, first at
//! one end and then the other, and runs, splashes, and sloshes. The
//! renderer steps its 1,200 or so particles (off the main thread) and
//! draws them as a screen-space surface on Medium and High
//! ([`verse_pbr::fluid::LiquidTank`]); this module places it and draws its
//! stand and frame. The glass is clear and drawn only by the liquid it
//! holds; the player can walk through the tank.

use glam::{Quat, Vec3};
use verse_pbr::fluid::LiquidTank;
use verse_pbr::pbr::LitVertex;

use crate::floats::boxed;
use crate::terrain::ground;

/// The tank's centre on the beach, x and z, m: ahead and to the right of
/// the spawn as it faces the sea.
pub const CENTER: [f32; 2] = [7.5, 12.0];
/// The inside's half extents, m.
pub const HALF: Vec3 = Vec3::new(0.8, 0.35, 0.3);
/// How high the stand lifts the tank's floor over the highest ground
/// under it, m.
const STAND: f32 = 0.45;
/// Seconds between columns.
pub const PERIOD: f32 = 7.0;
/// Particle radius, m: about 1,200 particles in the column.
const RADIUS: f32 = 0.025;

/// The tank's floor height, m.
#[must_use]
pub fn floor() -> f32 {
    let [x, z] = CENTER;
    let mut top = f32::MIN;
    for sx in [-1.0, 1.0] {
        for sz in [-1.0, 1.0] {
            top = top.max(ground(x + sx * HALF.x, z + sz * HALF.z));
        }
    }
    top + STAND
}

/// The tank at the lab's clock `time`, s.
#[must_use]
pub fn tank(time: f32) -> LiquidTank {
    let [x, z] = CENTER;
    let min = Vec3::new(x - HALF.x, floor(), z - HALF.z);
    LiquidTank {
        min,
        max: min + 2.0 * HALF,
        radius: RADIUS,
        column: 0.3,
        height: 0.55,
        round: (time.max(0.0) / PERIOD) as u32,
    }
}

/// The stand, the floor, and the frame the glass sits in.
pub fn draw(out: &mut Vec<LitVertex>) {
    let [x, z] = CENTER;
    let y = floor();
    let q = Quat::IDENTITY;
    let wood = [[0.30, 0.19, 0.10], [0.26, 0.16, 0.08]];
    let metal = [[0.09, 0.09, 0.10]];
    let rail = 0.025;
    // The stand: a top under the floor and four legs to the ground.
    let top = Vec3::new(HALF.x + 0.08, 0.04, HALF.z + 0.08);
    boxed(out, Vec3::new(x, y - top.y, z), q, top, 4, &wood, 0.8);
    for sx in [-1.0, 1.0] {
        for sz in [-1.0, 1.0] {
            let (lx, lz) = (x + sx * HALF.x, z + sz * HALF.z);
            let foot = ground(lx, lz) - 0.1;
            let h = (y - 2.0 * top.y - foot).max(0.05) * 0.5;
            boxed(
                out,
                Vec3::new(lx, foot + h, lz),
                q,
                Vec3::new(0.04, h, 0.04),
                1,
                &wood[1..],
                0.8,
            );
        }
    }
    // The frame: corner posts, and rails round the bottom and the rim.
    let c = Vec3::new(x, y + HALF.y, z);
    for sx in [-1.0, 1.0] {
        for sz in [-1.0, 1.0] {
            let at = c + Vec3::new(sx * (HALF.x + rail), 0.0, sz * (HALF.z + rail));
            boxed(
                out,
                at,
                q,
                Vec3::new(rail, HALF.y + rail, rail),
                1,
                &metal,
                0.35,
            );
        }
    }
    for level in [-1.0, 1.0] {
        let yy = c.y + level * (HALF.y + rail);
        for sz in [-1.0, 1.0] {
            boxed(
                out,
                Vec3::new(x, yy, z + sz * (HALF.z + rail)),
                q,
                Vec3::new(HALF.x + rail, rail, rail),
                1,
                &metal,
                0.35,
            );
        }
        for sx in [-1.0, 1.0] {
            boxed(
                out,
                Vec3::new(x + sx * (HALF.x + rail), yy, z),
                q,
                Vec3::new(rail, rail, HALF.z + rail),
                1,
                &metal,
                0.35,
            );
        }
    }
}
