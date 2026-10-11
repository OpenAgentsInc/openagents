//! The Ice Cream Man's rules (`docs/verse/games/the-ice-cream-man.md`).
//!
//! A deterministic fixed-step simulation at [`HZ`] steps a second: the
//! seeded Airbnb ([`layout`]), the player and their tools, the Ice Cream
//! Man's state machine ([`man`]), and the run card. Nothing here draws,
//! reads a key, or knows the time of day; the same seed and the same
//! inputs give the same run everywhere.

pub mod bot;
pub mod game;
pub mod geom;
pub mod layout;
pub mod man;
pub mod rng;
pub mod tuning;

pub use game::{Event, Game, Input, Options, Prompt, RunCard, Status, Tool};
pub use geom::{V2, v};
pub use layout::{House, Module, Pickup};
pub use man::{Man, ManState};

/// Steps a second.
pub const HZ: u32 = 30;
/// Seconds a step.
pub const DT: f32 = 1.0 / HZ as f32;
