//! Eval task format for reproducible benchmarks.
//!
//! Provides a standardized schema for evaluation tasks that define:
//! - Repository context (source, ref, focus files)
//! - Goal/prompt to accomplish
//! - Constraints (tokens, latency, budget)
//! - Gold standard files for comparison
//! - Expected outputs for verification

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An evaluation task defining a benchmark scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalTask {
    /// Unique identifier for this task.
    pub id: String,

    /// Repository context for the task.
    pub repo: RepoContext,

    /// The goal/prompt to accomplish.
    pub goal: String,

    /// Constraints for the solution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<Constraint>,

    /// Gold standard files for comparison (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gold_files: Option<Vec<GoldFile>>,

    /// Expected outputs for verification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<ExpectedOutput>,

    /// Task metadata (tags, difficulty, etc.).
    #[serde(default)]
    pub metadata: TaskMetadata,
}

impl EvalTask {
    /// Create a new eval task.
    pub fn new(id: impl Into<String>, goal: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            repo: RepoContext::default(),
            goal: goal.into(),
            constraints: Vec::new(),
            gold_files: None,
            expected: None,
            metadata: TaskMetadata::default(),
        }
    }

    /// Set repository context.
    pub fn with_repo(mut self, repo: RepoContext) -> Self {
        self.repo = repo;
        self
    }

    /// Add a constraint.
    pub fn with_constraint(mut self, constraint: Constraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// Set gold files.
    pub fn with_gold_files(mut self, files: Vec<GoldFile>) -> Self {
        self.gold_files = Some(files);
        self
    }

    /// Set expected output.
    pub fn with_expected(mut self, expected: ExpectedOutput) -> Self {
        self.expected = Some(expected);
        self
    }

    /// Set metadata.
    pub fn with_metadata(mut self, metadata: TaskMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Add a tag to metadata.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.metadata.tags.push(tag.into());
        self
    }
}

/// Repository context for an eval task.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepoContext {
    /// Path to local repo or URL to clone.
    pub source: String,

    /// Specific commit/branch to checkout.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_spec: Option<String>,

    /// Files relevant to this task.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus_files: Vec<String>,
}

impl RepoContext {
    /// Create a new repo context from a source path/URL.
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            ref_spec: None,
            focus_files: Vec::new(),
        }
    }

    /// Set the ref spec (commit/branch).
    pub fn with_ref(mut self, ref_spec: impl Into<String>) -> Self {
        self.ref_spec = Some(ref_spec.into());
        self
    }

    /// Add focus files.
    pub fn with_focus_files(mut self, files: Vec<String>) -> Self {
        self.focus_files = files;
        self
    }
}

/// A constraint on the solution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    /// Type of constraint.
    pub kind: ConstraintKind,

    /// Human-readable description.
    pub description: String,

    /// Optional value for the constraint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

impl Constraint {
    /// Create a new constraint.
    pub fn new(kind: ConstraintKind, description: impl Into<String>) -> Self {
        Self {
            kind,
            description: description.into(),
            value: None,
        }
    }

    /// Set constraint value.
    pub fn with_value(mut self, value: impl Into<serde_json::Value>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Create a max tokens constraint.
    pub fn max_tokens(max: usize) -> Self {
        Self::new(ConstraintKind::MaxTokens, format!("Maximum {} tokens", max))
            .with_value(serde_json::json!(max))
    }

    /// Create a max latency constraint.
    pub fn max_latency_ms(ms: u64) -> Self {
        Self::new(
            ConstraintKind::MaxLatency,
            format!("Maximum {}ms latency", ms),
        )
        .with_value(serde_json::json!(ms))
    }

    /// Create a budget constraint.
    pub fn budget_msats(msats: u64) -> Self {
        Self::new(
            ConstraintKind::BudgetMsats,
            format!("Budget: {} msats", msats),
        )
        .with_value(serde_json::json!(msats))
    }

    /// Create a "must use file" constraint.
    pub fn must_use_file(path: impl Into<String>) -> Self {
        let path = path.into();
        Self::new(
            ConstraintKind::MustUseFile,
            format!("Must use file: {}", path),
        )
        .with_value(serde_json::json!(path))
    }

    /// Create a "must not modify" constraint.
    pub fn must_not_modify(path: impl Into<String>) -> Self {
        let path = path.into();
        Self::new(
            ConstraintKind::MustNotModify,
            format!("Must not modify: {}", path),
        )
        .with_value(serde_json::json!(path))
    }
}

/// Types of constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintKind {
    /// Maximum number of tokens in output.
    MaxTokens,
    /// Maximum latency in milliseconds.
    MaxLatency,
    /// Must use a specific file.
    MustUseFile,
    /// Must not modify a specific file.
    MustNotModify,
    /// Must include tests.
    RequireTests,
    /// Budget in millisatoshis.
    BudgetMsats,
    /// Custom constraint type.
    Custom,
}

/// A gold standard file for comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldFile {
    /// File path (relative to repo root).
    pub path: String,

    /// Expected content.
    pub content: String,

    /// How to compare against this gold file.
    #[serde(default)]
    pub comparison: ComparisonMode,
}

impl GoldFile {
    /// Create a new gold file.
    pub fn new(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
            comparison: ComparisonMode::default(),
        }
    }

    /// Set comparison mode.
    pub fn with_comparison(mut self, mode: ComparisonMode) -> Self {
        self.comparison = mode;
        self
    }
}

/// How to compare output against gold standard.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonMode {
    /// Exact string match.
    #[default]
    Exact,
    /// Semantic equivalence (LLM-judged).
    Semantic,
    /// AST-equivalent (for code).
    AstEquivalent,
    /// Must contain specified patterns.
    ContainsPatterns(Vec<String>),
}

/// Expected output for verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct ExpectedOutput {
    /// Expected fields in the output.
    #[serde(default)]
    pub fields: HashMap<String, ExpectedField>,

    /// Commands that should pass (exit code 0).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pass_commands: Vec<String>,

    /// Patterns that must be present in output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_patterns: Vec<String>,

    /// Patterns that must not be present in output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbidden_patterns: Vec<String>,
}


impl ExpectedOutput {
    /// Create a new expected output.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an expected field.
    pub fn with_field(mut self, name: impl Into<String>, field: ExpectedField) -> Self {
        self.fields.insert(name.into(), field);
        self
    }

    /// Add a command that should pass.
    pub fn with_pass_command(mut self, cmd: impl Into<String>) -> Self {
        self.pass_commands.push(cmd.into());
        self
    }

    /// Add a required pattern.
    pub fn with_required_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.required_patterns.push(pattern.into());
        self
    }

    /// Add a forbidden pattern.
    pub fn with_forbidden_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.forbidden_patterns.push(pattern.into());
        self
    }
}

/// Expected value for an output field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedField {
    /// Expected value (if exact match).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,

    /// Expected type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_type: Option<String>,

    /// Minimum value (for numeric fields).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,

    /// Maximum value (for numeric fields).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,

    /// Pattern to match (for string fields).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

impl ExpectedField {
    /// Create an expected field with exact value.
    pub fn exact(value: impl Into<serde_json::Value>) -> Self {
        Self {
            value: Some(value.into()),
            expected_type: None,
            min: None,
            max: None,
            pattern: None,
        }
    }

    /// Create an expected field with type constraint.
    pub fn of_type(expected_type: impl Into<String>) -> Self {
        Self {
            value: None,
            expected_type: Some(expected_type.into()),
            min: None,
            max: None,
            pattern: None,
        }
    }

    /// Create an expected numeric field with range.
    pub fn in_range(min: f64, max: f64) -> Self {
        Self {
            value: None,
            expected_type: Some("number".into()),
            min: Some(min),
            max: Some(max),
            pattern: None,
        }
    }

    /// Create an expected field matching a pattern.
    pub fn matching(pattern: impl Into<String>) -> Self {
        Self {
            value: None,
            expected_type: Some("string".into()),
            min: None,
            max: None,
            pattern: Some(pattern.into()),
        }
    }
}

/// Task metadata for organization and filtering.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskMetadata {
    /// Tags for categorization (e.g., "retrieval", "bug-fix", "refactor").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Difficulty level (0.0 = easy, 1.0 = hard).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<f32>,

    /// Estimated time to complete in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_time_secs: Option<u64>,

    /// Source of this task (e.g., "swe-bench", "internal", "user-reported").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Additional key-value metadata.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, serde_json::Value>,
}

impl TaskMetadata {
    /// Create new metadata with a source.
    pub fn from_source(source: impl Into<String>) -> Self {
        Self {
            source: Some(source.into()),
            ..Default::default()
        }
    }

    /// Set difficulty.
    pub fn with_difficulty(mut self, difficulty: f32) -> Self {
        self.difficulty = Some(difficulty.clamp(0.0, 1.0));
        self
    }

    /// Set estimated time.
    pub fn with_estimated_time(mut self, secs: u64) -> Self {
        self.estimated_time_secs = Some(secs);
        self
    }

    /// Add extra metadata.
    pub fn with_extra(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }
}

/// A collection of eval tasks (a benchmark suite).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvalTaskSet {
    /// Name of this task set.
    pub name: String,

    /// Description of the benchmark.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Version of the task set.
    #[serde(default = "default_version")]
    pub version: String,

    /// The tasks in this set.
    pub tasks: Vec<EvalTask>,
}

fn default_version() -> String {
    "1.0.0".into()
}

impl EvalTaskSet {
    /// Create a new task set.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            version: default_version(),
            tasks: Vec::new(),
        }
    }

    /// Set description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add a task.
    pub fn with_task(mut self, task: EvalTask) -> Self {
        self.tasks.push(task);
        self
    }

    /// Add multiple tasks.
    pub fn with_tasks(mut self, tasks: Vec<EvalTask>) -> Self {
        self.tasks.extend(tasks);
        self
    }

    /// Get tasks by tag.
    pub fn tasks_with_tag(&self, tag: &str) -> Vec<&EvalTask> {
        self.tasks
            .iter()
            .filter(|t| t.metadata.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Get tasks by difficulty range.
    pub fn tasks_in_difficulty_range(&self, min: f32, max: f32) -> Vec<&EvalTask> {
        self.tasks
            .iter()
            .filter(|t| {
                t.metadata
                    .difficulty
                    .map(|d| d >= min && d <= max)
                    .unwrap_or(false)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_task_creation() {
        let task = EvalTask::new("task-001", "Fix the authentication bug")
            .with_repo(RepoContext::new("/path/to/repo").with_ref("main"))
            .with_constraint(Constraint::max_tokens(1000))
            .with_constraint(Constraint::budget_msats(5000))
            .with_tag("bug-fix");

        assert_eq!(task.id, "task-001");
        assert_eq!(task.constraints.len(), 2);
        assert!(task.metadata.tags.contains(&"bug-fix".to_string()));
    }

    #[test]
    fn test_gold_file() {
        let gold = GoldFile::new("src/auth.rs", "fn authenticate() { /* fixed */ }")
            .with_comparison(ComparisonMode::AstEquivalent);

        assert_eq!(gold.path, "src/auth.rs");
        matches!(gold.comparison, ComparisonMode::AstEquivalent);
    }

    #[test]
    fn test_expected_output() {
        let expected = ExpectedOutput::new()
            .with_pass_command("cargo test")
            .with_required_pattern("success")
            .with_field("score", ExpectedField::in_range(0.0, 1.0));

        assert_eq!(expected.pass_commands.len(), 1);
        assert!(expected.fields.contains_key("score"));
    }

    #[test]
    fn test_task_set() {
        let set = EvalTaskSet::new("retrieval-benchmark")
            .with_description("Benchmark for retrieval tasks")
            .with_task(EvalTask::new("t1", "Find error handlers").with_tag("retrieval"))
            .with_task(EvalTask::new("t2", "Fix bug").with_tag("bug-fix"));

        assert_eq!(set.tasks.len(), 2);
        assert_eq!(set.tasks_with_tag("retrieval").len(), 1);
    }

    #[test]
    fn test_task_serde() {
        let task = EvalTask::new("test", "Test goal")
            .with_repo(RepoContext::new("https://github.com/test/repo"))
            .with_constraint(Constraint::max_tokens(500));

        let json = serde_json::to_string_pretty(&task).unwrap();
        let parsed: EvalTask = serde_json::from_str(&json).unwrap();

        assert_eq!(task.id, parsed.id);
        assert_eq!(task.goal, parsed.goal);
    }
}
