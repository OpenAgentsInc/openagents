//! Dataset loaders for benchmark experiments.
//!
//! This crate provides loaders for various benchmark datasets:
//! - S-NIAH: Single-needle in a haystack (50 tasks)
//! - BrowseComp-Plus: Web browsing comprehension (150 tasks from 100K docs)
//! - OOLONG: Long-context understanding (trec_coarse + Pairs variants)
//! - CodeQA: Code question answering from LongBench v2
//!
//! # Example
//!
//! ```rust,ignore
//! use bench_datasets::{Dataset, SnihDataset};
//!
//! let dataset = SnihDataset::new("./data/sniah");
//! let tasks = dataset.load().await?;
//! ```

mod dataset;
mod error;

pub mod browsecomp;
pub mod codeqa;
pub mod oolong;
pub mod sniah;

pub use dataset::{Dataset, DatasetConfig};
pub use error::{Error, Result};

// Re-export task types from bench-harness
pub use bench_harness::{GroundTruth, SimpleTask, TaskInstance, TaskMetadata};
