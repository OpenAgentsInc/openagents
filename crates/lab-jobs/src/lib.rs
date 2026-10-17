//! Placement and execution rules shared by the Labs service and its workers.
pub mod authority;
pub mod placement;

pub use placement::{Capabilities, Decision, EffectiveRequirements, Placement, select};

pub mod coordinator;
pub mod protocol;

pub mod worker;
