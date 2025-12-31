//! Compute marketplace - NIP-90 DVM infrastructure

pub mod consumer;
pub mod db;
pub mod events;
pub mod fallback;
pub mod jobs;
pub mod pricing;
pub mod provider;

// Re-export commonly used types
pub use events::{ComputeJobFeedback, ComputeJobRequest, ComputeJobResult, JobCategory};
pub use fallback::{FallbackConfig, FallbackManager, FallbackMetrics, FallbackResult};
