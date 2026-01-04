//! FRLM (Federated RLM) visualization components
//!
//! This module provides visualization components for FRLM trace events:
//! - Timeline: Horizontal view of sub-queries over time
//! - BudgetMeter: Budget usage visualization
//! - QueryLane: Individual sub-query progress indicators
//! - FrlmPanel: Composite panel combining all components

mod budget;
mod query_lane;
mod timeline;
mod panel;

pub use budget::BudgetMeter;
pub use query_lane::{QueryLane, QueryStatus};
pub use timeline::FrlmTimeline;
pub use panel::FrlmPanel;
