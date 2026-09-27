//! Zone-agnostic rigid-body physics for Verse.
//!
//! - [`body`]: rigid bodies with principal inertia, quaternion attitude,
//!   and torque-free rotation (Euler's equations).
//! - [`world`]: bodies advanced together in fixed steps under a
//!   caller-supplied acceleration [`Field`].
//! - [`clock`]: a [`FixedStep`] accumulator that turns frame time into
//!   whole steps and reports time it drops.
//! - [`trace`]: sampled states and a tolerance comparison for replay tests.
//!
//! The crate has no rendering, networking, I/O, or zone knowledge. Zones
//! own their rules (fields, controls, part definitions) and consume these
//! mechanisms. See `docs/physics/2026-09-27-genesis-port-roadmap.md`.

pub mod body;
pub mod clock;
pub mod trace;
pub mod world;

pub use body::{Body, BodyKind};
pub use clock::FixedStep;
pub use trace::{Divergence, Tolerance, Trace, attitude_difference};
pub use world::{BodyId, Field, NoField, Uniform, World};
