//! What Verse's zones share, split out of `crates/verse` so that a zone can
//! be its own crate under the world runtime: the static `World` a zone
//! builds, the controls it answers and its atmosphere (`zone`), the avatar
//! figure, textured particle effects (`fx`), tooltips, and block-letter
//! scene labels. `verse` re-exports each item under its old path. Read
//! `docs/verse/README.md`.

pub mod avatar;
pub mod fx;
pub mod label;
pub mod tooltip;
pub mod world;
pub mod zone;

// The paths the moved modules were written against.
use verse_gfx::ui;
use verse_pbr::mesh;
use verse_world::social::controller;
