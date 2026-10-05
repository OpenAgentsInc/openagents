//! Everglade, split out of `crates/verse` so an edit to it recompiles this
//! crate and what depends on it rather than all of Verse: the zone with its
//! city, studio, spells, hotbar, wildlife, and demolition yard
//! (`zones::everglade`), and its pinned pack with the compiler that builds
//! it (`zones::everglade_pack`). `verse` re-exports both modules under their
//! old paths. Read `docs/verse/README.md` and `docs/verse/zones.md`.

pub mod zones;

// The paths these zones were written against inside `crates/verse`.
use verse_core::{avatar, fx, tooltip, world};
use verse_gfx::{palette, ui};
use verse_pbr::{mesh, pbr};
use verse_world::social::controller;

/// The runtime names these zones use.
mod runtime {
    pub use verse_core::zone::InteractHint;
}

/// The content compiler's exports, as `verse::imported` names them.
mod imported {
    pub use verse_content::compiler::{characters, icons, inventory};
}

/// The scene label, as `verse::doors` names it.
mod doors {
    pub use verse_core::label::label as scene_label;
}
