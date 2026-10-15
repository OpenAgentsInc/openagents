//! Lagrange 2 for Verse: a spaceship factory at Earth–Moon L2.
//!
//! - [`ship`]: the spacecraft document (`spacecraft.v1`): parts with mass,
//!   shape, and latches; derived mass properties; and the checks every edit
//!   must pass (mass budget, thrust line, latch loads, the assembly tree).
//! - [`arm`]: the factory arm, Robot Lab's SO-101 at 100 times its size.
//! - [`factory`]: the deck, four arms, racks, and cradle as rigid bodies
//!   and servoed joints in one `physics` world, with CMG attitude control.
//! - [`assembly`]: the plan document and the stage-checked sequencer that
//!   simulates each stage before committing it, checks it after, steps back
//!   or makes safe on failure, and rides out a lost link or a controller
//!   reset.
//! - [`burn`]: the finished ship's test burn.
//!
//! The orbit (Earth–Moon L2, the halo, station-keeping) is
//! `verse_lagrange::earth_moon`. No rendering, networking, or I/O.

pub mod arm;
pub mod assembly;
pub mod burn;
pub mod factory;
pub mod geometry;
pub mod ship;

pub use assembly::{Assembly, Failure, Faults, Plan, Status};
pub use factory::Factory;
pub use ship::Spacecraft;

#[cfg(test)]
mod tests;
