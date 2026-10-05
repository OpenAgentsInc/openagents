//! Verse's imported content and the remote chamber, split out of
//! `crates/verse` so an edit to the chamber recompiles this crate and what
//! depends on it rather than all of Verse: the chamber's play, combat,
//! controls, overlay, and panels over the admitted-frame renderer
//! (`imported`), and the RITUAL connection a client opens to a chamber host
//! (`ritual`). `verse` re-exports both modules under their old paths. Read
//! `docs/verse/README.md`.

pub mod imported;
pub mod ritual;

// The paths the moved modules were written against inside `crates/verse`.
use verse_core::tooltip;
#[cfg(all(feature = "remote-chamber", feature = "imported-desktop"))]
use verse_gfx::profiling;
use verse_gfx::ui;
use verse_net::identity;

/// The legacy renderer's names these modules use.
mod render {
    pub use verse_engine::presentation::View;
}
