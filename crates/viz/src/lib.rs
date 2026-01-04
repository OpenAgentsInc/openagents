//! Unified Visualization Grammar
//!
//! This crate provides a visual grammar for the "execution movie" HUD system,
//! implementing 5 visual verbs:
//!
//! - **Fill**: "how much" - gauges, bars, rings, meters
//! - **Pulse**: "something happened" - events, heartbeats, flashes
//! - **Flow**: "data moving" - streams, arcs, pipelines
//! - **Heat**: "intensity/importance" - heatmaps, rails
//! - **Topology**: "structure/connections" - graphs, trees, layer stacks

pub mod grammar;
pub mod fill;
pub mod pulse;
pub mod flow;
pub mod heat;
pub mod topology;
pub mod trace;
pub mod compose;

// Re-export core traits
pub use grammar::{
    Edge, Fill, Flow, Heat, Node, NodeId, Palette, Pulse, Topology, VizPrimitive, sub_bounds,
};

// Re-export trace types
pub use trace::{TraceEvent, Venue};
