//! Terminal-Bench shared types and services
//!
//! This crate provides core types and services for Terminal-Bench integration,
//! shared between commander, mechacoder, and other tools.
//!
//! # Core Types
//!
//! - [`TBTask`] - Task definition with metadata
//! - [`TBRunSummary`] - Summary of a benchmark run
//! - [`TBRunStatus`] / [`TBRunOutcome`] - Run state tracking
//! - [`TBModelOption`] - Available model options
//!
//! # Services
//!
//! - [`TaskLoader`] - Load tasks from JSON suite files
//! - [`RunStore`] - Persist run history to disk
//!
//! # Re-exports from Harbor
//!
//! - [`StreamEvent`] - Real-time streaming events from tbench CLI

pub mod types;
pub mod task_loader;
pub mod run_store;

// Re-export core types
pub use types::{
    TBTask,
    TBDifficulty,
    TBRunSummary,
    TBRunStatus,
    TBRunOutcome,
    TBModelOption,
    TBRunOptions,
    ExecutionSettings,
    DashboardStats,
    DifficultyStats,
    DifficultyCount,
    format_duration,
    format_percent,
};

// Re-export services
pub use task_loader::{TaskLoader, LoadedSuite, TaskLoadError, RawTask, TaskSuite};
pub use run_store::{RunStore, RunRecord};

// Re-export streaming types from harbor
pub use harbor::{StreamEvent, Trajectory, Agent, Step, StepSource, TBenchMetrics};
