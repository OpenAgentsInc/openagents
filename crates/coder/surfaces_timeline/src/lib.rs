//! # Run Timeline Surface
//!
//! Visualize agent workflow execution.
//!
//! This crate provides:
//! - Horizontal timeline with step blocks
//! - Parallel lanes for concurrent agents
//! - Streaming status updates
//! - Step detail expansion

pub mod lane;
pub mod step;
pub mod timeline;

pub use lane::{Lane, LaneId};
pub use step::{Step, StepId, StepStatus};
pub use timeline::Timeline;
