//! Placement and execution rules shared by the Labs service and its workers.
pub mod placement;

pub use placement::{Capabilities, Decision, EffectiveRequirements, Placement, select};
