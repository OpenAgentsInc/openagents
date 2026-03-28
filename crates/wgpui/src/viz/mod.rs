//! Shared visualization grammar for training- and evidence-style surfaces.
//!
//! Apps should treat this namespace as the owner for generic:
//!
//! - panel chrome and texture
//! - sampled scalar chart rendering
//! - event rail rendering
//! - reusable visualization token semantics
//!
//! Product panes should keep domain truth, layout, and interaction policy in the
//! app layer, but new generic chart, feed, topology, or provenance primitives
//! should land here instead of in pane-local helpers.

pub mod badge;
pub mod chart;
pub mod feed;
pub mod panel;
pub mod provenance;
pub mod sampling;
pub mod theme;
pub mod topology;
