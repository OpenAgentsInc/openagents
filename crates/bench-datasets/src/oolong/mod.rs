//! OOLONG (Out Of context LONg context) dataset.
//!
//! OOLONG tests long-context understanding with two variants:
//! - TrecCoarse: Numeric answers (document count queries), scored via exp(-|pred - truth|)
//! - Pairs: Pairwise aggregation queries, scored via F1

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use async_trait::async_trait;
use rand::prelude::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

use crate::dataset::{Dataset, DatasetConfig};
use crate::error::{Error, Result};
use bench_harness::{GroundTruth, SimpleTask, TaskMetadata};

/// OOLONG variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OolongVariant {
    /// TrecCoarse: Numeric counting queries.
    TrecCoarse,
    /// Pairs: Pairwise aggregation queries.
    Pairs,
}

impl OolongVariant {
    /// Get the primary metric for this variant.
    pub fn primary_metric(&self) -> &str {
        match self {
            OolongVariant::TrecCoarse => "numeric_decay",
            OolongVariant::Pairs => "f1",
        }
    }
}

/// Raw OOLONG TrecCoarse task from JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawTrecTask {
    /// Task identifier.
    id: String,
    /// The query.
    query: String,
    /// Context documents.
    context: String,
    /// Numeric answer (document count).
    answer: f64,
    /// Tolerance for scoring.
    #[serde(default)]
    tolerance: Option<f64>,
}

/// Raw OOLONG Pairs task from JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawPairsTask {
    /// Task identifier.
    id: String,
    /// The query.
    query: String,
    /// Context documents.
    context: String,
    /// Expected answer set.
    answers: Vec<String>,
}

/// OOLONG dataset loader.
pub struct OolongDataset {
    config: DatasetConfig,
    variant: OolongVariant,
}

impl OolongDataset {
    /// Create a new OOLONG dataset.
    pub fn new(config: DatasetConfig, variant: OolongVariant) -> Self {
        Self { config, variant }
    }

    /// Create TrecCoarse variant from path.
    pub fn trec_coarse(path: impl Into<PathBuf>) -> Self {
        Self::new(DatasetConfig::new(path), OolongVariant::TrecCoarse)
    }

    /// Create Pairs variant from path.
    pub fn pairs(path: impl Into<PathBuf>) -> Self {
        Self::new(DatasetConfig::new(path), OolongVariant::Pairs)
    }

    /// Load TrecCoarse tasks.
    fn load_trec_tasks(&self, path: &PathBuf) -> Result<Vec<SimpleTask>> {
        let content = fs::read_to_string(path)?;
        let mut tasks = Vec::new();

        let raw_tasks: Vec<RawTrecTask> =
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                content
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| serde_json::from_str(l))
                    .collect::<std::result::Result<Vec<_>, _>>()?
            } else {
                serde_json::from_str(&content)?
            };

        for raw in raw_tasks {
            let ground_truth = GroundTruth::numeric_with_tolerance(
                raw.answer,
                raw.tolerance.unwrap_or(0.0),
            );

            let metadata = TaskMetadata::new()
                .with_source("oolong-trec")
                .with_category("counting");

            let task = SimpleTask::new(raw.id, raw.query, ground_truth)
                .with_context(raw.context)
                .with_metadata(metadata);

            tasks.push(task);
        }

        Ok(tasks)
    }

    /// Load Pairs tasks.
    fn load_pairs_tasks(&self, path: &PathBuf) -> Result<Vec<SimpleTask>> {
        let content = fs::read_to_string(path)?;
        let mut tasks = Vec::new();

        let raw_tasks: Vec<RawPairsTask> =
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                content
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| serde_json::from_str(l))
                    .collect::<std::result::Result<Vec<_>, _>>()?
            } else {
                serde_json::from_str(&content)?
            };

        for raw in raw_tasks {
            let ground_truth = GroundTruth::string_set(raw.answers);

            let metadata = TaskMetadata::new()
                .with_source("oolong-pairs")
                .with_category("aggregation");

            let task = SimpleTask::new(raw.id, raw.query, ground_truth)
                .with_context(raw.context)
                .with_metadata(metadata);

            tasks.push(task);
        }

        Ok(tasks)
    }
}

#[async_trait]
impl Dataset for OolongDataset {
    type Task = SimpleTask;

    fn name(&self) -> &str {
        match self.variant {
            OolongVariant::TrecCoarse => "oolong-trec",
            OolongVariant::Pairs => "oolong-pairs",
        }
    }

    fn description(&self) -> &str {
        match self.variant {
            OolongVariant::TrecCoarse => {
                "OOLONG TrecCoarse: Count documents matching specific criteria"
            }
            OolongVariant::Pairs => {
                "OOLONG Pairs: Identify pairs of items with specific relationships"
            }
        }
    }

    fn primary_metric(&self) -> &str {
        self.variant.primary_metric()
    }

    async fn load(&self) -> Result<Vec<SimpleTask>> {
        let path = &self.config.data_path;

        // Determine file path
        let file_path = if path.is_file() {
            path.clone()
        } else if path.is_dir() {
            let name = match self.variant {
                OolongVariant::TrecCoarse => "trec_coarse",
                OolongVariant::Pairs => "pairs",
            };

            let jsonl_path = path.join(format!("{}.jsonl", name));
            let json_path = path.join(format!("{}.json", name));

            if jsonl_path.exists() {
                jsonl_path
            } else if json_path.exists() {
                json_path
            } else {
                return Err(Error::NotFound(format!(
                    "No {}.jsonl or {}.json found in {}",
                    name,
                    name,
                    path.display()
                )));
            }
        } else {
            return Err(Error::NotFound(format!(
                "Dataset path not found: {}",
                path.display()
            )));
        };

        let mut tasks = match self.variant {
            OolongVariant::TrecCoarse => self.load_trec_tasks(&file_path)?,
            OolongVariant::Pairs => self.load_pairs_tasks(&file_path)?,
        };

        // Shuffle if configured
        if self.config.shuffle {
            let seed = self.config.seed.unwrap_or(42);
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            tasks.shuffle(&mut rng);
        }

        // Limit tasks if configured
        if let Some(max) = self.config.max_tasks {
            tasks.truncate(max);
        }

        Ok(tasks)
    }
}

/// Generate synthetic TrecCoarse tasks for testing.
pub fn generate_synthetic_trec_tasks(count: usize) -> Vec<SimpleTask> {
    let mut tasks = Vec::with_capacity(count);

    for i in 0..count {
        let answer = (i % 100) as f64;
        let context = format!(
            "The following documents are about topic {}. \
             After careful analysis, there are {} documents that match the criteria.",
            i, answer
        );

        let query = format!("How many documents discuss topic {}?", i);

        let metadata = TaskMetadata::new()
            .with_source("synthetic-oolong-trec")
            .with_category("counting");

        let task = SimpleTask::new(
            format!("synth-trec-{}", i),
            query,
            GroundTruth::numeric(answer),
        )
        .with_context(context)
        .with_metadata(metadata);

        tasks.push(task);
    }

    tasks
}

/// Generate synthetic Pairs tasks for testing.
pub fn generate_synthetic_pairs_tasks(count: usize) -> Vec<SimpleTask> {
    let mut tasks = Vec::with_capacity(count);

    for i in 0..count {
        let answers: HashSet<String> = (0..3).map(|j| format!("item-{}-{}", i, j)).collect();
        let answers_vec: Vec<_> = answers.iter().cloned().collect();
        let context = format!(
            "The following items are related: {}. \
             These items share common properties.",
            answers_vec.join(", ")
        );

        let query = format!("What items are in group {}?", i);

        let metadata = TaskMetadata::new()
            .with_source("synthetic-oolong-pairs")
            .with_category("aggregation");

        let task = SimpleTask::new(
            format!("synth-pairs-{}", i),
            query,
            GroundTruth::string_set(answers_vec),
        )
        .with_context(context)
        .with_metadata(metadata);

        tasks.push(task);
    }

    tasks
}

#[cfg(test)]
mod tests {
    use super::*;
    use bench_harness::TaskInstance;

    #[test]
    fn test_generate_synthetic_trec() {
        let tasks = generate_synthetic_trec_tasks(5);
        assert_eq!(tasks.len(), 5);

        for task in &tasks {
            assert!(task.context().is_some());
            assert!(matches!(
                task.ground_truth(),
                GroundTruth::NumericRange { .. }
            ));
        }
    }

    #[test]
    fn test_generate_synthetic_pairs() {
        let tasks = generate_synthetic_pairs_tasks(5);
        assert_eq!(tasks.len(), 5);

        for task in &tasks {
            assert!(task.context().is_some());
            assert!(matches!(task.ground_truth(), GroundTruth::StringSet(_)));
        }
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let dataset = OolongDataset::trec_coarse("/nonexistent/path");
        let result = dataset.load().await;
        assert!(result.is_err());
    }
}
