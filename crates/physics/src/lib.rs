//! Zone-agnostic rigid-body physics for Verse.
//!
//! - [`body`]: rigid bodies with principal inertia, quaternion attitude,
//!   momentum-conserving rotation, and force, torque, and impulse inputs.
//! - [`world`]: bodies advanced together in fixed steps under a
//!   caller-supplied acceleration [`Field`].
//! - [`collision`]: sphere, capsule, and box colliders with bitmask filters
//!   and multi-point manifolds.
//! - [`contact`]: a sequential-impulse solver with restitution, an elliptic
//!   friction cone, and torsional friction.
//! - [`joint`]: point, weld, and tether joints, hard or soft (an implicit
//!   spring), with force and torque limits.
//! - Island sleep ([`SleepSettings`]): settled groups resting on fixed
//!   bodies stop costing anything until something touches them.
//! - [`clock`]: a [`FixedStep`] accumulator that turns frame time into
//!   whole steps and reports time it drops.
//! - [`ledger`]: linear and angular momentum with named external impulses,
//!   for conservation tests.
//! - [`oracle`]: scenes shared with the offline Genesis oracle.
//! - [`sensors`]: raycasts, per-body contact force, an IMU, and debug lines
//!   for contacts and joints.
//! - [`rope`]: a deterministic XPBD rope pinned at both ends, with
//!   long-range limits from each end, one-way by default or coupled to its
//!   end bodies.
//! - [`plume`]: free-molecular plume impingement forces on colliders.
//! - [`modal`]: damped structural modes with an exact discrete update, for
//!   flexible appendages.
//! - [`thrusters`]: body-mounted thrusters, a bounded allocator from a wanted
//!   force and torque to throttles, and a vector PID controller.
//! - [`trace`]: sampled states and a tolerance comparison for replay tests.
//!
//! The crate has no rendering, networking, I/O, or zone knowledge. Zones
//! own their rules (fields, controls, part definitions) and consume these
//! mechanisms. See `docs/physics/2026-09-27-genesis-port-roadmap.md`.

pub mod body;
pub mod clock;
pub mod collision;
pub mod contact;
pub mod joint;
pub mod ledger;
pub mod modal;
pub mod oracle;
pub mod plume;
pub mod rope;
pub mod sensors;
pub mod thrusters;
pub mod trace;
pub mod world;

pub use body::{Body, BodyKind, Composite};
pub use clock::FixedStep;
pub use collision::{Collider, ColliderId, ContactPoint, Filter, Manifold, Material, Shape};
pub use contact::{ContactReport, SolverSettings};
pub use joint::{Joint, JointId, JointKind, Spring};
pub use ledger::{Ledger, LedgerError, Momentum};
pub use modal::Mode;
pub use plume::{Plume, Reflection, Sample};
pub use rope::{Rope, RopeSettings};
pub use sensors::{DebugKind, DebugLine, Imu, ImuReading, RayHit};
pub use thrusters::{Pid, Thruster, ThrusterSet, Wrench};
pub use trace::{Divergence, Tolerance, Trace, attitude_difference};
pub use world::{BodyId, Field, NoField, SleepSettings, StepStats, Uniform, World};

#[cfg(test)]
mod tests;
