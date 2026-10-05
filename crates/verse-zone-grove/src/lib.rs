//! The Grove, the druid training field on Everglade's pack, split out of
//! `crates/verse` so an edit to it recompiles this crate and what depends on
//! it rather than all of Verse. `verse` re-exports `zones::grove` under its
//! old path. Read `docs/verse/druid-demo.md`.

pub mod zones;

// The paths this zone was written against inside `crates/verse`.
use verse_core::{fx, tooltip, world};
use verse_gfx::ui;
use verse_pbr::{mesh, pbr};
use verse_world::social::controller;

/// The content compiler's exports, as `verse::imported` names them.
mod imported {
    pub use verse_content::compiler::icons;
}
