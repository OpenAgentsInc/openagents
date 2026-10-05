//! Verse's drawing foundations, split out of `crates/verse` so an edit to a
//! zone does not recompile them: the amber palette, the glyph atlas and UI
//! batch, the overlay panel, GLES shader variants, device-loss observation,
//! frame profiling, and the third-person follow camera. `verse` re-exports
//! each module under its old path. Read `docs/verse/README.md`.

pub mod camera;
pub mod gles;
pub mod gpu_lifecycle;
pub mod overlay;
pub mod palette;
pub mod profiling;
pub mod ui;
