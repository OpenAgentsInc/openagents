//! The Water Lab and W10 coastal fixture. Shared water spells, projectiles,
//! hotbar, and simulation live in `verse-water-spells`; these re-exports
//! preserve the Water Lab's public paths.

pub use verse_water_spells::*;

pub mod coast;
#[cfg(test)]
mod coast_tests;
