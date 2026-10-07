//! Textured particle effects: flipbook sprites driven by effect
//! definitions, drawn in the physical renderer's scene pass.
//!
//! The pipeline (`docs/verse/particles.md`):
//!
//! - **Sprites** are sheets rendered by Blender scripts under
//!   `scripts/blender/fx/` into `assets/verse/fx/` ([`sheet`]).
//! - **Effects** are TOML files under `assets/verse/fx/effects/` naming
//!   their emitters: sheet, frames, rates, lifetimes, motion, curves over
//!   life, and blend ([`def`], [`library`]).
//! - **Running effects** live in a [`Particles`]: a zone starts one at a
//!   point, moves it if it trails something, stops it, steps it, and
//!   appends its [`Sprite`]s to the frame's [`crate::mesh::Mesh::sprites`].
//! - **The renderer** keeps a frame's sprites within the tier's [`budget`]
//!   and draws them back to front through one premultiplied-alpha pipeline
//!   that samples every sheet from one texture array ([`vertices`]).
//! - **Preview**: `cargo run --release -p verse --example fx_preview --
//!   EFFECT OUT_DIR` renders an effect over time into a contact sheet.

pub mod def;
pub mod library;
pub mod motes;
pub use verse_pbr::fx::{sheet, sprite};
pub mod system;

pub use def::Effect;
pub use library::Library;
pub use sprite::{Facing, Sprite, SpriteVertex, budget, vertices};
pub use system::{Handle, Particles, Spawn, Style};

#[cfg(test)]
mod tests;
