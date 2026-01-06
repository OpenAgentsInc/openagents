//! BrowseComp-Plus dataset.
//!
//! BrowseComp-Plus is a web browsing comprehension benchmark with:
//! - 100K documents corpus
//! - 150 tasks sampled to ensure gold documents are present
//! - Exact match evaluation (% correct)

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use async_trait::async_trait;
use rand::prelude::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

use crate::dataset::{Dataset, DatasetConfig};
use crate::error::{Error, Result};
use bench_harness::{GroundTruth, SimpleTask, TaskMetadata};

/// Raw BrowseComp task from JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawBrowseCompTask {
    /// Task identifier.
    id: String,
    /// The question.
    question: String,
    /// The expected answer.
    answer: String,
    /// Document IDs that contain the answer.
    #[serde(default)]
    gold_doc_ids: Vec<String>,
    /// Difficulty level.
    #[serde(default)]
    difficulty: Option<String>,
}

/// Raw document from the corpus.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawDocument {
    /// Document ID.
    id: String,
    /// Document title.
    #[serde(default)]
    title: Option<String>,
    /// Document content.
    content: String,
    /// URL if available.
    #[serde(default)]
    url: Option<String>,
}

/// BrowseComp-Plus dataset loader.
pub struct BrowseCompDataset {
    config: DatasetConfig,
}

impl BrowseCompDataset {
    /// Create a new BrowseComp dataset.
    pub fn new(config: DatasetConfig) -> Self {
        Self { config }
    }

    /// Create with just a path.
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self::new(DatasetConfig::new(path))
    }

    /// Load the document corpus.
    fn load_corpus(&self, path: &PathBuf) -> Result<HashMap<String, RawDocument>> {
        let content = fs::read_to_string(path)?;
        let mut corpus = HashMap::new();

        // Try JSONL format first
        if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
            for line in content.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                let doc: RawDocument = serde_json::from_str(line)?;
                corpus.insert(doc.id.clone(), doc);
            }
        } else {
            // Try JSON array
            let docs: Vec<RawDocument> = serde_json::from_str(&content)?;
            for doc in docs {
                corpus.insert(doc.id.clone(), doc);
            }
        }

        Ok(corpus)
    }

    /// Load tasks and build context from gold documents.
    fn load_tasks_with_corpus(
        &self,
        tasks_path: &PathBuf,
        corpus: &HashMap<String, RawDocument>,
    ) -> Result<Vec<SimpleTask>> {
        let content = fs::read_to_string(tasks_path)?;
        let mut tasks = Vec::new();

        let raw_tasks: Vec<RawBrowseCompTask> =
            if tasks_path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                content
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| serde_json::from_str(l))
                    .collect::<std::result::Result<Vec<_>, _>>()?
            } else {
                serde_json::from_str(&content)?
            };

        for raw in raw_tasks {
            // Build context from gold documents
            let context = raw
                .gold_doc_ids
                .iter()
                .filter_map(|id| corpus.get(id))
                .map(|doc| {
                    let title = doc.title.as_deref().unwrap_or("Document");
                    format!("## {}\n\n{}", title, doc.content)
                })
                .collect::<Vec<_>>()
                .join("\n\n---\n\n");

            let metadata = TaskMetadata::new()
                .with_source("browsecomp-plus")
                .with_category("web-comprehension");

            let metadata = if let Some(ref diff) = raw.difficulty {
                metadata.with_difficulty(diff.clone())
            } else {
                metadata
            };

            let task = SimpleTask::new(raw.id, raw.question, GroundTruth::exact(raw.answer))
                .with_context(context)
                .with_metadata(metadata);

            tasks.push(task);
        }

        Ok(tasks)
    }
}

#[async_trait]
impl Dataset for BrowseCompDataset {
    type Task = SimpleTask;

    fn name(&self) -> &str {
        "browsecomp-plus"
    }

    fn description(&self) -> &str {
        "Web browsing comprehension: Answer questions using a corpus of web documents"
    }

    fn expected_count(&self) -> Option<usize> {
        Some(150) // Standard BrowseComp-Plus has 150 tasks
    }

    fn primary_metric(&self) -> &str {
        "exact_match"
    }

    async fn load(&self) -> Result<Vec<SimpleTask>> {
        let path = &self.config.data_path;

        // Expect directory with corpus.jsonl and tasks.jsonl
        let corpus_path = path.join("corpus.jsonl");
        let tasks_path = path.join("tasks.jsonl");

        // Also try .json extension
        let corpus_path = if corpus_path.exists() {
            corpus_path
        } else {
            let alt = path.join("corpus.json");
            if alt.exists() {
                alt
            } else {
                return Err(Error::NotFound(format!(
                    "Corpus file not found in {}",
                    path.display()
                )));
            }
        };

        let tasks_path = if tasks_path.exists() {
            tasks_path
        } else {
            let alt = path.join("tasks.json");
            if alt.exists() {
                alt
            } else {
                return Err(Error::NotFound(format!(
                    "Tasks file not found in {}",
                    path.display()
                )));
            }
        };

        let corpus = self.load_corpus(&corpus_path)?;
        let mut tasks = self.load_tasks_with_corpus(&tasks_path, &corpus)?;

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

/// Generate synthetic BrowseComp tasks for testing.
pub fn generate_synthetic_tasks(count: usize) -> Vec<SimpleTask> {
    let mut tasks = Vec::with_capacity(count);

    for i in 0..count {
        let answer = format!("Answer{}", i);
        let context = format!(
            "## Document {}\n\nThis document contains information about {}. \
             The specific answer to question {} is: {}. \
             Additional filler text follows to make the context longer.",
            i, i, i, answer
        );

        let query = format!("What is the answer to question {}?", i);

        let metadata = TaskMetadata::new()
            .with_source("synthetic-browsecomp")
            .with_category("web-comprehension");

        let task = SimpleTask::new(
            format!("synth-browsecomp-{}", i),
            query,
            GroundTruth::exact(answer),
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
        let tasks = generate_synthetic_tasks(5);
        assert_eq!(tasks.len(), 5);

        for task in &tasks {
            assert!(task.context().is_some());
        }
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let dataset = BrowseCompDataset::from_path("/nonexistent/path");
        let result = dataset.load().await;
        assert!(result.is_err());
    }
}
