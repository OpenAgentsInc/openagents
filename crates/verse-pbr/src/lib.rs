//! Verse's physical renderer, split out of `crates/verse` so an edit to a
//! zone does not recompile it: the PBR pipelines, baking, sky, and textured
//! scenes (`pbr`), the world mesh they draw (`mesh`), streamed content
//! residency on native targets (`streaming`), and the shared fog distances
//! (`fog`). `verse` re-exports each module under its old path. Read
//! `docs/verse/README.md`.

pub mod fog;
pub mod fx;
pub mod mesh;
pub mod pbr;
mod shading;
#[cfg(not(target_arch = "wasm32"))]
pub mod streaming;

pub mod imported;
pub use verse_gfx::{gles, gpu_lifecycle, ui};

#[cfg(test)]
mod visual_tests;
