//! Compute marketplace - NIP-90 DVM infrastructure

pub mod events;
pub mod provider;
pub mod consumer;
pub mod pricing;
pub mod jobs;

// Re-export commonly used types
pub use events::{
    ComputeJobFeedback, ComputeJobRequest, ComputeJobResult, JobCategory,
};
