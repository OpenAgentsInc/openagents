//! The small shapes the CAD physics shares with the Verse: a walking
//! footprint, a segment against a box, a lit triangle vertex, and the
//! ground-plane facing. `verse-world` and `verse-pbr` re-export them where
//! they were.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

/// An axis-aligned footprint the character cannot walk through.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint {
    /// Minimum x and z, in meters.
    pub min: [f32; 2],
    /// Maximum x and z, in meters.
    pub max: [f32; 2],
}

impl Footprint {
    /// True when `(x, z)` lies inside this footprint grown by `margin`.
    #[must_use]
    pub fn contains(&self, x: f32, z: f32, margin: f32) -> bool {
        x > self.min[0] - margin
            && x < self.max[0] + margin
            && z > self.min[1] - margin
            && z < self.max[1] + margin
    }
}

/// Where, from 0 to 1, the segment from `from` to `to` first enters the box
/// from `min` to `max` grown by `radius` on every side, or `None` when it
/// misses it or starts inside it.
#[must_use]
pub fn box_hit(from: Vec3, to: Vec3, radius: f32, min: Vec3, max: Vec3) -> Option<f32> {
    let delta = to - from;
    let (min, max) = (min - Vec3::splat(radius), max + Vec3::splat(radius));
    let mut enter = f32::NEG_INFINITY;
    let mut leave = f32::INFINITY;
    for axis in 0..3 {
        let (o, d, lo, hi) = (from[axis], delta[axis], min[axis], max[axis]);
        if d.abs() < 1e-9 {
            if o < lo || o > hi {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo - o) / d, (hi - o) / d);
        enter = enter.max(a.min(b));
        leave = leave.min(a.max(b));
    }
    (enter <= leave && enter > 0.0 && enter <= 1.0).then_some(enter)
}

/// The ground-plane unit vector for `yaw`.
#[must_use]
pub fn forward(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, yaw.cos())
}

/// One vertex of a physically lit triangle.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct LitVertex {
    /// World position in meters.
    pub pos: [f32; 3],
    /// Unit surface normal.
    pub normal: [f32; 3],
    /// Unit tangent, the brushing direction for anisotropic metal.
    pub tangent: [f32; 3],
    /// Object-space position, so procedural detail moves with its part.
    pub local: [f32; 3],
    /// Linear base color: albedo for dielectrics, F0 for metals.
    pub color: [f32; 3],
    /// Metallic, roughness, `verse_pbr::pbr::Material` code, and baked
    /// ambient occlusion.
    pub params: [f32; 4],
}
