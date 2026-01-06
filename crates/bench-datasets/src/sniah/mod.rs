//! Single-Needle In A Haystack (S-NIAH) dataset.
//!
//! S-NIAH tests the ability to find a single piece of information
//! embedded in a large context. Each task contains:
//! - A long "haystack" context with irrelevant text
//! - A "needle" (the answer) hidden somewhere in the context
//! - A question asking for the needle

use std::fs;
use std::path::PathBuf;

use async_trait::async_trait;
use rand::prelude::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

use crate::dataset::{Dataset, DatasetConfig};
use crate::error::{Error, Result};
use bench_harness::{GroundTruth, SimpleTask, TaskMetadata};

/// Raw S-NIAH task from JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawSnihTask {
    /// Task identifier.
    id: String,
    /// The query/question.
    query: String,
    /// The haystack context (may include the needle).
    context: String,
    /// The needle (answer) to find.
    needle: String,
    /// Position of needle in context (for analysis).
    #[serde(default)]
    needle_position: Option<f64>,
    /// Context length in tokens (approximate).
    #[serde(default)]
    context_tokens: Option<usize>,
}

/// S-NIAH dataset loader.
pub struct SnihDataset {
    config: DatasetConfig,
}

impl SnihDataset {
    /// Create a new S-NIAH dataset.
    pub fn new(config: DatasetConfig) -> Self {
        Self { config }
    }

    /// Create with just a path.
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self::new(DatasetConfig::new(path))
    }

    /// Load tasks from a JSONL file.
    fn load_from_jsonl(&self, path: &PathBuf) -> Result<Vec<SimpleTask>> {
        let content = fs::read_to_string(path)?;
        let mut tasks = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }

            let raw: RawSnihTask = serde_json::from_str(line).map_err(|e| {
                Error::InvalidFormat(format!("line {}: {}", line_num + 1, e))
            })?;

            let metadata = TaskMetadata::new()
                .with_source("s-niah")
                .with_category("needle-in-haystack");

            let metadata = if let Some(tokens) = raw.context_tokens {
                metadata.with_context_length(tokens)
            } else {
                metadata
            };

            let task = SimpleTask::new(raw.id, raw.query, GroundTruth::exact(raw.needle))
                .with_context(raw.context)
                .with_metadata(metadata);

            tasks.push(task);
        }

        Ok(tasks)
    }

    /// Load tasks from a JSON array file.
    fn load_from_json(&self, path: &PathBuf) -> Result<Vec<SimpleTask>> {
        let content = fs::read_to_string(path)?;
        let raw_tasks: Vec<RawSnihTask> = serde_json::from_str(&content)?;

        let tasks = raw_tasks
            .into_iter()
            .map(|raw| {
                let metadata = TaskMetadata::new()
                    .with_source("s-niah")
                    .with_category("needle-in-haystack");

                let metadata = if let Some(tokens) = raw.context_tokens {
                    metadata.with_context_length(tokens)
                } else {
                    metadata
                };

                SimpleTask::new(raw.id, raw.query, GroundTruth::exact(raw.needle))
                    .with_context(raw.context)
                    .with_metadata(metadata)
            })
            .collect();

        Ok(tasks)
    }
}

#[async_trait]
impl Dataset for SnihDataset {
    type Task = SimpleTask;

    fn name(&self) -> &str {
        "s-niah"
    }

    fn description(&self) -> &str {
        "Single-Needle In A Haystack: Find a specific piece of information in a long context"
    }

    fn expected_count(&self) -> Option<usize> {
        Some(50) // Standard S-NIAH has 50 tasks
    }

    fn primary_metric(&self) -> &str {
        "exact_match"
    }

    async fn load(&self) -> Result<Vec<SimpleTask>> {
        let path = &self.config.data_path;

        // Try different file formats
        let mut tasks = if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
            self.load_from_jsonl(path)?
        } else if path.extension().map(|e| e == "json").unwrap_or(false) {
            self.load_from_json(path)?
        } else if path.is_dir() {
            // Try to find a data file in the directory
            let jsonl_path = path.join("sniah.jsonl");
            let json_path = path.join("sniah.json");

            if jsonl_path.exists() {
                self.load_from_jsonl(&jsonl_path)?
            } else if json_path.exists() {
                self.load_from_json(&json_path)?
            } else {
                return Err(Error::NotFound(format!(
                    "No sniah.jsonl or sniah.json found in {}",
                    path.display()
                )));
            }
        } else {
            return Err(Error::NotFound(format!(
                "Dataset path not found: {}",
                path.display()
            )));
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

/// Generate synthetic S-NIAH tasks for testing.
pub fn generate_synthetic_tasks(count: usize, context_length: usize) -> Vec<SimpleTask> {
    let mut tasks = Vec::with_capacity(count);

    for i in 0..count {
        // Generate a simple haystack with a needle
        let needle = format!("SECRET-{:04}", i);
        let position = i as f64 / count as f64;
        let insert_pos = (context_length as f64 * position) as usize;

        // Create haystack text
        let filler = "Lorem ipsum dolor sit amet. ";
        let filler_chars: usize = filler.len();
        let num_fillers = context_length / filler_chars;

        let mut context = String::with_capacity(context_length + needle.len());
        for j in 0..num_fillers {
            if j * filler_chars >= insert_pos && !context.contains(&needle) {
                context.push_str(&format!("The secret code is {}. ", needle));
            }
            context.push_str(filler);
        }

        let query = "What is the secret code?";

        let metadata = TaskMetadata::new()
            .with_source("synthetic-s-niah")
            .with_category("needle-in-haystack")
            .with_context_length(context.len());

        let task = SimpleTask::new(
            format!("synth-sniah-{}", i),
            query,
            GroundTruth::exact(needle),
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
    fn test_generate_synthetic() {
        let tasks = generate_synthetic_tasks(5, 1000);
        assert_eq!(tasks.len(), 5);

        for task in &tasks {
            assert!(task.context().is_some());
            let context = task.context().unwrap();
            // Verify the needle is in the context
            if let GroundTruth::ExactMatch(needle) = task.ground_truth() {
                assert!(context.contains(needle));
            }
        }
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let dataset = SnihDataset::from_path("/nonexistent/path");
        let result = dataset.load().await;
        assert!(result.is_err());
    }
}
