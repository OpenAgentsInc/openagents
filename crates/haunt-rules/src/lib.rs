//! The Haunted House as data (`docs/verse/games/the-haunted-house.md`).
//!
//! The whole house is one typed JSON document, `haunt.v1` ([`doc`]): the
//! single source of truth the shell draws, the checks read, and agents edit
//! only through typed tools. Every area states its size, position, facing,
//! contents and intended psychological effect.
//!
//! - [`check`]: the checks every edit runs; a refusal is one plain sentence.
//! - [`edit`]: add or remove an area, place a library asset, free walls.
//! - [`describe`]: the text and map an agent reads before any edit
//!   (`haunt describe`).
//! - [`walk`]: walls with doorways, and moves that slide along them.
//! - [`trigger`]: which triggers fire as the player walks and looks.
//!
//! Pure and free of I/O, so the shell (`haunt-web`) builds it for `wasm32`.

pub mod check;
pub mod describe;
pub mod doc;
pub mod edit;
pub mod trigger;
pub mod walk;

pub use check::{Refusal, check, problems};
pub use doc::{House, KIND, v1};

#[cfg(test)]
mod tests;
