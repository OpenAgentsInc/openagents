# Bench Harness

**Archived:** The `bench-harness` crate is not part of the current workspace. This document is
retained for historical reference.

The `bench-harness` crate provides generic infrastructure for running ML benchmark experiments.

## Overview

```rust
use bench_harness::{
    ExperimentConfig, ExperimentRunner, Method, MethodResult,
    TaskInstance, Trajectory, ExactMatchMetric,
};

// 1. Define or load tasks
let tasks: Vec<SimpleTask> = load_tasks();

// 2. Create experiment config
let config = ExperimentConfig::new("my-experiment")
    .output_dir("./results")
    .max_tasks(100)  // Optional: limit for debugging
    .seed(42);       // Optional: reproducibility

// 3. Set up runner
let mut runner = ExperimentRunner::new(config, tasks);
runner.add_method(Arc::new(my_method));
runner.add_metric(Box::new(ExactMatchMetric));
runner.set_primary_metric("exact_match");

// 4. Run experiment
let results = runner.run().await?;
```

## Tasks

### TaskInstance Trait

All benchmark tasks implement this trait:

```rust
pub trait TaskInstance: Send + Sync {
    /// Unique identifier.
    fn id(&self) -> &str;

    /// The query/question to answer.
    fn query(&self) -> &str;

    /// Optional context (for long-context tasks).
    fn context(&self) -> Option<&str>;

    /// Ground truth for evaluation.
    fn ground_truth(&self) -> &GroundTruth;

    /// Task metadata.
    fn metadata(&self) -> &TaskMetadata;
}
```

### SimpleTask

Built-in implementation for simple tasks:

```rust
use bench_harness::{SimpleTask, GroundTruth, TaskMetadata};

let task = SimpleTask::new(
    "task-001",
    "What is the capital of France?",
    GroundTruth::exact("Paris"),
)
.with_context("France is a country in Western Europe...")
.with_metadata(
    TaskMetadata::new()
        .with_source("geography")
        .with_difficulty("easy")
        .with_category("factual")
);
```

### GroundTruth

Different types of expected answers:

```rust
pub enum GroundTruth {
    /// Exact string match (case-insensitive, trimmed).
    ExactMatch(String),

    /// Multiple choice (A, B, C, D).
    MultipleChoice {
        answer: char,
        choices: Vec<String>,
    },

    /// Numeric with tolerance.
    NumericRange {
        value: f64,
        tolerance: f64,
    },

    /// Set of strings (for F1 evaluation).
    StringSet(HashSet<String>),

    /// Freeform with reference (for LLM-as-judge).
    Freeform {
        reference: String,
        rubric: Option<String>,
    },
}

// Convenience constructors
let exact = GroundTruth::exact("answer");
let mc = GroundTruth::multiple_choice('B', vec!["A".into(), "B".into(), "C".into()]);
let numeric = GroundTruth::numeric(42.0);
let numeric_tol = GroundTruth::numeric_with_tolerance(42.0, 0.5);
let set = GroundTruth::string_set(vec!["item1", "item2", "item3"]);
```

## Methods

### Method Trait

Solution methods implement this trait:

```rust
#[async_trait]
pub trait Method: Send + Sync {
    /// Method name for logging.
    fn name(&self) -> &str;

    /// Solve a task, returning trajectory and answer.
    async fn solve(&self, task: &dyn TaskInstance) -> Result<MethodResult>;

    /// Optional warmup (e.g., load models).
    async fn warmup(&mut self) -> Result<()> { Ok(()) }

    /// Reset state between tasks.
    async fn reset(&mut self) -> Result<()> { Ok(()) }
}
```

### MethodResult

What a method returns:

```rust
pub struct MethodResult {
    /// The final answer.
    pub answer: String,
    /// Execution trajectory.
    pub trajectory: Trajectory,
    /// Token usage.
    pub usage: LmUsage,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

// Create a result
let result = MethodResult::new(
    "The answer is 42",
    trajectory,
    usage,
).with_duration(1500);
```

## Trajectories

Trajectories log the execution steps of a method:

```rust
use bench_harness::{Trajectory, StepType};

let mut trajectory = Trajectory::new("task-001", "my-method");

// Log an LLM call
trajectory.add_llm_call("gpt-4", &usage, "The response text");

// Log code execution
trajectory.add_code_execution("python", 0, "Output: 42");

// Log the final answer
trajectory.add_final("42");

// Or log an error
trajectory.add_error("Timeout occurred");
```

### Step Types

```rust
pub enum StepType {
    /// LLM completion call.
    LlmCall {
        model: String,
        prompt_tokens: usize,
        completion_tokens: usize,
    },
    /// Code execution.
    CodeExecution {
        language: String,
        exit_code: i32,
    },
    /// Sub-query to LLM.
    SubQuery {
        prompt: String,
    },
    /// Retrieval operation.
    Retrieval {
        query: String,
        num_results: usize,
    },
    /// Final answer.
    Final {
        answer: String,
    },
    /// Error encountered.
    Error {
        message: String,
    },
}
```

### Trajectory Writer

Write trajectories to JSONL:

```rust
use bench_harness::TrajectoryWriter;

let mut writer = TrajectoryWriter::new("./trajectories.jsonl")?;
writer.write(&trajectory)?;
writer.flush()?;
```

## Metrics

### Metric Trait

```rust
pub trait Metric: Send + Sync {
    fn name(&self) -> &str;
    fn compute(&self, prediction: &str, truth: &GroundTruth) -> MetricValue;
}

pub struct MetricValue {
    pub name: String,
    pub score: f64,  // Typically 0.0-1.0
    pub details: Option<String>,
}
```

### Built-in Metrics

#### ExactMatchMetric

Case-insensitive exact match:

```rust
let metric = ExactMatchMetric;
let value = metric.compute("Paris", &GroundTruth::exact("paris"));
assert_eq!(value.score, 1.0);
```

#### MultipleChoiceAccuracy

Extracts and compares answer letters:

```rust
let metric = MultipleChoiceAccuracy;
let truth = GroundTruth::multiple_choice('B', choices);
let value = metric.compute("The answer is B", &truth);
assert_eq!(value.score, 1.0);
```

#### NumericDecayMetric

Exponential decay based on distance from truth:

```rust
let metric = NumericDecayMetric;
let truth = GroundTruth::numeric_with_tolerance(100.0, 5.0);

// Within tolerance: score = 1.0
let value = metric.compute("102", &truth);
assert_eq!(value.score, 1.0);

// Outside tolerance: score = exp(-|pred - truth|)
let value = metric.compute("110", &truth);
assert!(value.score < 1.0);
```

#### F1Metric

Set-based F1 score:

```rust
let metric = F1Metric;
let truth = GroundTruth::string_set(vec!["apple", "banana", "cherry"]);

// Prediction parsed as comma/newline separated
let value = metric.compute("apple, banana", &truth);
// precision = 2/2 = 1.0, recall = 2/3 = 0.67
// F1 = 2 * 1.0 * 0.67 / (1.0 + 0.67) = 0.8
```

## Experiment Runner

### Configuration

```rust
let config = ExperimentConfig::new("experiment-name")
    .output_dir("./results")      // Where to save results
    .resume(true)                  // Resume from checkpoint (default: true)
    .max_tasks(100)               // Limit tasks (optional)
    .seed(42);                    // Random seed (optional)
```

### Running Experiments

```rust
let mut runner = ExperimentRunner::new(config, tasks);

// Add methods
runner.add_method(Arc::new(base_method));
runner.add_method(Arc::new(summary_method));

// Add metrics
runner.add_metric(Box::new(ExactMatchMetric));
runner.add_metric(Box::new(F1Metric));

// Set primary metric for ranking
runner.set_primary_metric("exact_match");

// Run!
let results = runner.run().await?;
```

### Results

```rust
pub struct ExperimentResults {
    pub config: ExperimentConfig,
    pub per_method: HashMap<String, MethodResults>,
    pub task_results: Vec<TaskResult>,
    pub total_usage: LmUsage,
}

pub struct MethodResults {
    pub method: String,
    pub primary_score: f64,
    pub std_dev: f64,
    pub metric_scores: HashMap<String, f64>,
    pub total_tasks: usize,
    pub successful_tasks: usize,
    pub total_usage: LmUsage,
    pub mean_usage: LmUsage,
    pub mean_duration_ms: f64,
}
```

### Output Files

The runner creates these files in the output directory:

```
results/
├── experiment-name_checkpoint.json   # For resuming
├── experiment-name_results.jsonl     # Per-task results
├── experiment-name_trajectories.jsonl # Execution traces
└── experiment-name_final.json        # Aggregated results
```

### Checkpointing

Experiments automatically checkpoint after each task:

```rust
// First run - processes all tasks
let results = runner.run().await?;

// If interrupted, subsequent runs resume from checkpoint
let results = runner.run().await?;  // Skips completed tasks
```

To start fresh, set `resume(false)`:

```rust
let config = ExperimentConfig::new("experiment")
    .resume(false);  // Don't use checkpoint
```
