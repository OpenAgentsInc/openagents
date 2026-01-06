//! Dataset trait and configuration.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use bench_harness::TaskInstance;

/// Configuration for loading a dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetConfig {
    /// Path to the dataset files.
    pub data_path: PathBuf,
    /// Maximum number of tasks to load (for debugging).
    pub max_tasks: Option<usize>,
    /// Random seed for sampling.
    pub seed: Option<u64>,
    /// Whether to shuffle tasks.
    pub shuffle: bool,
}

impl DatasetConfig {
    /// Create a new dataset config.
    pub fn new(data_path: impl Into<PathBuf>) -> Self {
        Self {
            data_path: data_path.into(),
            max_tasks: None,
            seed: None,
            shuffle: false,
        }
    }

    /// Set maximum tasks to load.
    pub fn max_tasks(mut self, max: usize) -> Self {
        self.max_tasks = Some(max);
        self
    }

    /// Set random seed.
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Enable shuffling.
    pub fn shuffle(mut self) -> Self {
        self.shuffle = true;
        self
    }
}

impl Default for DatasetConfig {
    fn default() -> Self {
        Self {
            data_path: PathBuf::from("."),
            max_tasks: None,
            seed: None,
            shuffle: false,
        }
    }
}

/// Trait for loading benchmark datasets.
#[async_trait]
pub trait Dataset: Send + Sync {
    /// The task type this dataset produces.
    type Task: TaskInstance;

    /// Name of the dataset.
    fn name(&self) -> &str;

    /// Description of the dataset.
    fn description(&self) -> &str {
        ""
    }

    /// Load tasks from the dataset.
    async fn load(&self) -> Result<Vec<Self::Task>>;

    /// Get the number of tasks in the dataset (if known without loading).
    fn expected_count(&self) -> Option<usize> {
        None
    }

    /// Get the primary metric for this dataset.
    fn primary_metric(&self) -> &str {
        "exact_match"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dataset_config() {
        let config = DatasetConfig::new("/data/test")
            .max_tasks(10)
            .seed(42)
            .shuffle();

        assert_eq!(config.data_path, PathBuf::from("/data/test"));
        assert_eq!(config.max_tasks, Some(10));
        assert_eq!(config.seed, Some(42));
        assert!(config.shuffle);
    }
}
