//! Generic benchmarking infrastructure for paper replication.
//!
//! This crate provides reusable components for running ML experiments:
//! - Task and Method traits for defining benchmarks
//! - Trajectory logging in JSONL format
//! - Metrics computation (accuracy, F1, etc.)
//! - Experiment runner with checkpointing
//!
//! # Example
//!
//! ```rust,ignore
//! use bench_harness::{ExperimentRunner, ExperimentConfig, Method};
//!
//! let config = ExperimentConfig::new("my-experiment")
//!     .output_dir("./results");
//!
//! let mut runner = ExperimentRunner::new(config, tasks);
//! runner.add_method(Arc::new(my_method));
//! let results = runner.run().await?;
//! ```

mod error;
mod experiment;
mod method;
mod metrics;
pub mod stats;
mod task;
mod trajectory;

pub use error::{Error, Result};
pub use experiment::{
    ExperimentConfig, ExperimentResults, ExperimentRunner, MethodResults, TaskResult,
};
pub use method::{Method, MethodResult};
pub use metrics::{
    ExactMatchMetric, F1Metric, Metric, MetricValue, MultipleChoiceAccuracy, NumericDecayMetric,
};
pub use stats::{
    bootstrap_ci, cohens_d, confidence_interval_95, independent_t_test, mean, paired_t_test,
    standard_error, std_dev, variance, TTestResult,
};
pub use task::{GroundTruth, SimpleTask, TaskInstance, TaskMetadata};
pub use trajectory::{StepType, Trajectory, TrajectoryStep, TrajectoryWriter};

// Re-export LM types for convenience
pub use lm_router::LmUsage;
