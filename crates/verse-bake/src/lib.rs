//! Verse's offline lighting baker.
//!
//! The baker lights a static [`TexturedScene`] ahead of time, so a zone can
//! load baked light instead of baking it on the player's device. It writes
//! [`Products`] keyed by a [`bake_key`]: SHA-256 over the scene's geometry
//! digest, the light, and the settings. The products are, for every vertex,
//! the ambient multiplier and open sky fraction after several bounces, the
//! sun's visibility toward each configured sun direction, and the probe
//! grid that characters sample.
//!
//! Two backends trace the rays. [`CpuBackend`] walks the bake's bounding
//! volume hierarchy on every core and runs anywhere. The `gpu` feature adds
//! `gpu::GpuBackend`, which traces through hardware ray queries. Sampling
//! and shading stay on the CPU for both, so the two agree within
//! [`GPU_TOLERANCE`]. A bake is deterministic: a fixed seed turns each
//! item's sampling pattern, and the thread count never changes a value, so
//! a rebake of an unchanged scene on one backend reproduces the products'
//! digest.
//!
//! [`TexturedScene`]: verse_pbr::pbr::textured::TexturedScene

pub mod backend;
pub mod bake;
pub mod fixture;
#[cfg(feature = "gpu")]
pub mod gpu;
pub mod products;
pub mod scene;

pub use backend::{Backend, CpuBackend, Ray, RayHit};
pub use bake::{BAKER_VERSION, Light, Settings, Stats, bake, bake_key};
pub use products::{Agreement, Products, Spread};
pub use scene::{Scene, hex};

/// How far a GPU bake may lie from a CPU bake of the same scene and
/// settings. Both share every sample, so they differ only where a ray
/// grazes a triangle's edge and one backend counts the crossing while the
/// other does not; a few such rays move a vertex's multiplier by a few
/// hundredths at most.
pub const GPU_TOLERANCE: Spread = Spread {
    mean: 0.002,
    p99: 0.02,
    max: 0.25,
};

#[cfg(test)]
mod tests;
