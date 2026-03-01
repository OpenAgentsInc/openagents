//! Reusable CAD domain foundation for OpenAgents.
//!
//! This crate intentionally starts with a minimal, product-agnostic API surface.
//! Higher-level CAD workflows are introduced incrementally via the CAD backlog.

pub mod boolean;
pub mod contracts;
pub mod document;
pub mod error;
pub mod eval;
pub mod features;
pub mod feature_graph;
pub mod format;
pub mod kernel;
pub mod params;
pub mod policy;
pub mod primitives;
pub use error::{CadError, CadErrorCode, CadErrorEvent, CadResult};
