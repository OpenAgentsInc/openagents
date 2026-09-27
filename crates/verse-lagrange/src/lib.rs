//! Sun–Earth L1 physics for Verse's Lagrange 1 zone.
//!
//! - [`orbit`] integrates the circular restricted three-body problem (Sun and
//!   Earth–Moon barycenter) for a station on a Lissajous orbit about L1, with
//!   periodic burns that cancel the linear unstable mode.
//! - [`station`] is the construction sandbox: an astronaut with a cold-gas
//!   maneuvering pack, free-flying parts, a keel jig, the ropes of the
//!   safety tether and part lines, and plume impingement.
//! - [`arrays`] flexes the two solar array wings in assumed modes.
//!
//! Rigid bodies, fixed stepping, and restorable world state come from the
//! shared `physics` crate; this crate owns the L1 rules: the orbit, the tidal
//! field, the pack, the parts, and the jig.
//!
//! The crate has no rendering, networking, or I/O. Scene geometry and input
//! mapping belong to the Verse host.

pub mod arrays;
pub mod orbit;
pub mod station;

pub use arrays::ArrayFlex;
pub use orbit::{L1, OrbitSnapshot, StationOrbit};
pub use physics::{self, Body, BodyId};
pub use station::{
    Command, Input, Line, PartKind, PartState, PlumePulse, RopeView, Snapshot, Station,
    StationState,
};

#[cfg(test)]
mod tests;
