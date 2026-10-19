//! Geometry shared by CAD simulation and Verse renderers.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

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
    /// Metallic, roughness, material code, and baked ambient occlusion.
    pub params: [f32; 4],
}

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

/// The ground-plane unit vector for `yaw`.
#[must_use]
pub fn forward(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, yaw.cos())
}
