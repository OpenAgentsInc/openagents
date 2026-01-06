//! CodeQA dataset from LongBench v2.
//!
//! Multiple-choice code comprehension questions evaluated by accuracy.

use std::fs;
use std::path::PathBuf;

use async_trait::async_trait;
use rand::prelude::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

use crate::dataset::{Dataset, DatasetConfig};
use crate::error::{Error, Result};
use bench_harness::{GroundTruth, SimpleTask, TaskMetadata};

/// Raw CodeQA task from JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawCodeQATask {
    /// Task identifier.
    id: String,
    /// The question.
    question: String,
    /// Code context.
    code: String,
    /// Answer choices (A, B, C, D).
    choices: Vec<String>,
    /// Correct answer (A, B, C, or D).
    answer: char,
    /// Programming language.
    #[serde(default)]
    language: Option<String>,
}

/// CodeQA dataset loader.
pub struct CodeQADataset {
    config: DatasetConfig,
}

impl CodeQADataset {
    /// Create a new CodeQA dataset.
    pub fn new(config: DatasetConfig) -> Self {
        Self { config }
    }

    /// Create with just a path.
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self::new(DatasetConfig::new(path))
    }

    /// Load tasks from a file.
    fn load_tasks(&self, path: &PathBuf) -> Result<Vec<SimpleTask>> {
        let content = fs::read_to_string(path)?;
        let mut tasks = Vec::new();

        let raw_tasks: Vec<RawCodeQATask> =
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
            // Format the question with choices
            let formatted_question = format!(
                "{}\n\nChoices:\nA) {}\nB) {}\nC) {}\nD) {}",
                raw.question,
                raw.choices.first().map(|s| s.as_str()).unwrap_or(""),
                raw.choices.get(1).map(|s| s.as_str()).unwrap_or(""),
                raw.choices.get(2).map(|s| s.as_str()).unwrap_or(""),
                raw.choices.get(3).map(|s| s.as_str()).unwrap_or("")
            );

            let ground_truth = GroundTruth::multiple_choice(raw.answer, raw.choices);

            let mut metadata = TaskMetadata::new()
                .with_source("codeqa-longbench")
                .with_category("code-comprehension");

            if let Some(lang) = raw.language {
                metadata = TaskMetadata {
                    extra: Some(serde_json::json!({ "language": lang })),
                    ..metadata
                };
            }

            let task = SimpleTask::new(raw.id, formatted_question, ground_truth)
                .with_context(raw.code)
                .with_metadata(metadata);

            tasks.push(task);
        }

        Ok(tasks)
    }
}

#[async_trait]
impl Dataset for CodeQADataset {
    type Task = SimpleTask;

    fn name(&self) -> &str {
        "codeqa"
    }

    fn description(&self) -> &str {
        "CodeQA: Multiple-choice code comprehension questions from LongBench v2"
    }

    fn primary_metric(&self) -> &str {
        "multiple_choice_accuracy"
    }

    async fn load(&self) -> Result<Vec<SimpleTask>> {
        let path = &self.config.data_path;

        // Determine file path
        let file_path = if path.is_file() {
            path.clone()
        } else if path.is_dir() {
            let jsonl_path = path.join("codeqa.jsonl");
            let json_path = path.join("codeqa.json");

            if jsonl_path.exists() {
                jsonl_path
            } else if json_path.exists() {
                json_path
            } else {
                return Err(Error::NotFound(format!(
                    "No codeqa.jsonl or codeqa.json found in {}",
                    path.display()
                )));
            }
        } else {
            return Err(Error::NotFound(format!(
                "Dataset path not found: {}",
                path.display()
            )));
        };

        let mut tasks = self.load_tasks(&file_path)?;

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

/// Generate synthetic CodeQA tasks for testing.
pub fn generate_synthetic_tasks(count: usize) -> Vec<SimpleTask> {
    let mut tasks = Vec::with_capacity(count);
    let answers = ['A', 'B', 'C', 'D'];

    for i in 0..count {
        let answer = answers[i % 4];
        let choices = vec![
            format!("Option A for question {}", i),
            format!("Option B for question {}", i),
            format!("Option C for question {}", i),
            format!("Option D for question {}", i),
        ];

        let code = format!(
            "def function_{}():\n    # This function does something\n    return {}",
            i, i
        );

        let question = format!(
            "What does function_{} return?\n\nChoices:\nA) {}\nB) {}\nC) {}\nD) {}",
            i, choices[0], choices[1], choices[2], choices[3]
        );

        let metadata = TaskMetadata::new()
            .with_source("synthetic-codeqa")
            .with_category("code-comprehension");

        let task = SimpleTask::new(
            format!("synth-codeqa-{}", i),
            question,
            GroundTruth::multiple_choice(answer, choices),
        )
        .with_context(code)
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
        let tasks = generate_synthetic_tasks(8);
        assert_eq!(tasks.len(), 8);

        for task in &tasks {
            assert!(task.context().is_some());
            assert!(matches!(
                task.ground_truth(),
                GroundTruth::MultipleChoice { .. }
            ));
        }
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let dataset = CodeQADataset::from_path("/nonexistent/path");
        let result = dataset.load().await;
        assert!(result.is_err());
    }
}
