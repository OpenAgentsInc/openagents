//! Spec: Graph serialization and deserialization
//!
//! This module provides types and functions for serializing Unit graphs to JSON
//! and deserializing them back.
//!
//! # Components
//!
//! - `types` - Core serialization types (GraphSpec, UnitSpec, etc.)
//! - `stringify` - Convert Graph → GraphSpec
//! - `from_spec` - Convert GraphSpec → Graph
//!
//! # Example
//!
//! ```ignore
//! use unit::spec::{GraphSpec, graph_to_spec, graph_from_spec, UnitRegistry};
//!
//! // Serialize a graph
//! let spec = graph_to_spec(&graph);
//! let json = spec.to_json()?;
//!
//! // Deserialize a graph
//! let spec = GraphSpec::from_json(&json)?;
//! let graph = graph_from_spec(&spec, &registry)?;
//! ```

mod types;
mod stringify;
mod from_spec;

// Re-export types
pub use types::{
    BundleSpec,
    ExposureSpec,
    GraphSpec,
    MergePlugSpec,
    MergeSpec,
    PinSpec,
    PositionSpec,
    UnitSpec,
};

// Re-export stringify functions
pub use stringify::{
    bundle_to_json,
    graph_to_bundle,
    graph_to_json,
    graph_to_spec,
    merge_to_spec,
    pin_to_spec,
    unit_to_spec,
};

// Re-export from_spec functions
pub use from_spec::{
    FromSpecError,
    UnitFactory,
    UnitRegistry,
    graph_from_bundle,
    graph_from_bundle_json,
    graph_from_json,
    graph_from_spec,
    merge_from_spec,
    unit_from_spec,
};
