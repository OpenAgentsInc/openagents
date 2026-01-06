//! Task instance trait and ground truth types.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Ground truth for evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GroundTruth {
    /// Exact string match.
    ExactMatch(String),
    /// Multiple choice answer (A, B, C, D).
    MultipleChoice {
        answer: char,
        choices: Vec<String>,
    },
    /// Numeric answer with tolerance.
    NumericRange {
        value: f64,
        tolerance: f64,
    },
    /// Set of strings (for F1 evaluation).
    StringSet(HashSet<String>),
    /// Freeform answer with reference.
    Freeform {
        reference: String,
        rubric: Option<String>,
    },
}

impl GroundTruth {
    /// Create an exact match ground truth.
    pub fn exact(s: impl Into<String>) -> Self {
        Self::ExactMatch(s.into())
    }

    /// Create a multiple choice ground truth.
    pub fn multiple_choice(answer: char, choices: Vec<String>) -> Self {
        Self::MultipleChoice { answer, choices }
    }

    /// Create a numeric ground truth.
    pub fn numeric(value: f64) -> Self {
        Self::NumericRange {
            value,
            tolerance: 0.0,
        }
    }

    /// Create a numeric ground truth with tolerance.
    pub fn numeric_with_tolerance(value: f64, tolerance: f64) -> Self {
        Self::NumericRange { value, tolerance }
    }

    /// Create a string set ground truth.
    pub fn string_set(items: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::StringSet(items.into_iter().map(|s| s.into()).collect())
    }
}

/// Metadata about a task.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskMetadata {
    /// Source dataset.
    pub source: Option<String>,
    /// Difficulty level.
    pub difficulty: Option<String>,
    /// Category/domain.
    pub category: Option<String>,
    /// Context length in characters.
    pub context_length: Option<usize>,
    /// Additional custom fields.
    pub extra: Option<serde_json::Value>,
}

impl TaskMetadata {
    /// Create new empty metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the source.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set the difficulty.
    pub fn with_difficulty(mut self, difficulty: impl Into<String>) -> Self {
        self.difficulty = Some(difficulty.into());
        self
    }

    /// Set the category.
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Set the context length.
    pub fn with_context_length(mut self, length: usize) -> Self {
        self.context_length = Some(length);
        self
    }
}

/// A single task instance from a dataset.
///
/// This trait defines what a benchmark task looks like.
/// Implementations provide the query, context, and ground truth.
pub trait TaskInstance: Send + Sync {
    /// Unique identifier for this task.
    fn id(&self) -> &str;

    /// The query/question to answer.
    fn query(&self) -> &str;

    /// Context documents (for long-context tasks).
    fn context(&self) -> Option<&str>;

    /// Ground truth for evaluation.
    fn ground_truth(&self) -> &GroundTruth;

    /// Task metadata (source, difficulty, etc.).
    fn metadata(&self) -> &TaskMetadata;
}

/// A simple task implementation for testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleTask {
    id: String,
    query: String,
    context: Option<String>,
    ground_truth: GroundTruth,
    metadata: TaskMetadata,
}

impl SimpleTask {
    /// Create a new simple task.
    pub fn new(
        id: impl Into<String>,
        query: impl Into<String>,
        ground_truth: GroundTruth,
    ) -> Self {
        Self {
            id: id.into(),
            query: query.into(),
            context: None,
            ground_truth,
            metadata: TaskMetadata::default(),
        }
    }

    /// Set the context.
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Set the metadata.
    pub fn with_metadata(mut self, metadata: TaskMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

impl TaskInstance for SimpleTask {
    fn id(&self) -> &str {
        &self.id
    }

    fn query(&self) -> &str {
        &self.query
    }

    fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }

    fn ground_truth(&self) -> &GroundTruth {
        &self.ground_truth
    }

    fn metadata(&self) -> &TaskMetadata {
        &self.metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ground_truth_exact() {
        let gt = GroundTruth::exact("hello");
        if let GroundTruth::ExactMatch(s) = gt {
            assert_eq!(s, "hello");
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_simple_task() {
        let task = SimpleTask::new("task-1", "What is 2+2?", GroundTruth::exact("4"))
            .with_context("Some context here")
            .with_metadata(TaskMetadata::new().with_category("math"));

        assert_eq!(task.id(), "task-1");
        assert_eq!(task.query(), "What is 2+2?");
        assert_eq!(task.context(), Some("Some context here"));
    }
}
