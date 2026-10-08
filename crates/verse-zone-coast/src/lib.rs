//! The coast's deterministic terrain and water (`docs/verse/coast.md`).

pub mod terrain;
pub mod water;

pub use terrain::ground;

/// The playable square's half extent, m.
pub const HALF_EXTENT: f32 = 600.0;
/// The arrival terrace, facing southwest over the bay.
pub const SPAWN: glam::Vec3 = glam::Vec3::new(120.0, 14.0, -120.0);
pub const SPAWN_YAW: f32 = 2.45;
/// The return portal on the terrace.
pub const RETURN_PORTAL: glam::Vec3 = glam::Vec3::new(128.0, 14.0, -125.0);
