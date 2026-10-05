//! The Grid's Gym, split out of `crates/verse` so an edit to the Gym
//! recompiles this crate and what depends on it rather than all of Verse:
//! the Gym boards a surface loads inside the building (`gym`), the hall's
//! shared eval board and the agents' notes (`gym_hall`, `gym_evals`,
//! `gym_notes`), and the published results panel with its trace replay
//! (`gym_results`, `gym_replay`). `verse` re-exports each module under its
//! old path. Read `docs/verse/README.md`.

#[cfg(not(target_arch = "wasm32"))]
pub mod gym;
pub mod gym_evals;
pub mod gym_hall;
pub mod gym_notes;
pub mod gym_replay;
#[cfg(not(target_arch = "wasm32"))]
pub mod gym_results;

// The paths the moved modules were written against inside `crates/verse`.
use verse_core::{crowd, world};
use verse_net::{chat, net};
use verse_pbr::mesh;

/// The replay names these modules use.
mod replay {
    pub use verse_core::place::Place;
}

/// The session names these modules use.
mod session {
    pub use verse_core::world::BARE_WORLD;
}
