//! Sun–Earth L1 physics for Verse's Lagrange 1 zone.
//!
//! - [`orbit`] integrates the circular restricted three-body problem (Sun and
//!   Earth–Moon barycenter) for a station on a Lissajous orbit about L1, with
//!   periodic burns that cancel the linear unstable mode.
//! - [`body`] integrates torque-free rigid bodies (Euler's equations).
//! - [`station`] is the construction sandbox: an astronaut with a cold-gas
//!   maneuvering pack, free-flying parts, and a keel jig.
//!
//! The crate has no rendering, networking, or I/O. Scene geometry and input
//! mapping belong to the Verse host.

pub mod body;
pub mod orbit;
pub mod station;

pub use body::RigidBody;
pub use orbit::{L1, OrbitSnapshot, StationOrbit};
pub use station::{Command, PartKind, PartState, Snapshot, Station};

#[cfg(test)]
mod tests;
