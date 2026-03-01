//! Reusable CAD domain foundation for OpenAgents.
//!
//! This crate intentionally starts with a minimal, product-agnostic API surface.
//! Higher-level CAD workflows are introduced incrementally via the CAD backlog.

pub mod analysis;
pub mod boolean;
pub mod chat_adapter;
pub mod contracts;
pub mod dispatch;
pub mod document;
pub mod error;
pub mod eval;
pub mod events;
pub mod export;
pub mod feature_graph;
pub mod features;
pub mod format;
pub mod history;
pub mod intent;
pub mod kernel;
pub mod materials;
pub mod measurement;
pub mod mesh;
pub mod params;
pub mod policy;
pub mod primitives;
pub mod query;
pub mod rack;
pub mod section;
pub mod selection;
pub mod semantic_refs;
pub mod tessellation;
pub mod validity;
pub use error::{CadError, CadErrorCode, CadErrorEvent, CadResult};
