# Metrics

What does 'better' mean?

## Overview

Metrics define **success**. They can measure:

- Accuracy
- Consistency
- Cost
- Latency
- Style
- Domain-specific correctness

Optimizers use metrics as the objective function. Without metrics, DSPy has no direction.

> **Mental model:** "Metrics turn vague intelligence into something measurable."

---

## Core Traits

### Evaluator Trait

The base trait for module evaluation.

```rust
// File: crates/dsrs/src/evaluate/evaluator.rs

#[allow(async_fn_in_trait)]
pub trait Evaluator: Module {
    /// Maximum concurrent evaluations.
    const MAX_CONCURRENCY: usize = 32;

    /// Display progress during batch evaluation.
    const DISPLAY_PROGRESS: bool = true;

    /// Score a single prediction against expected output.
    async fn metric(&self, example: &Example, prediction: &Prediction) -> f32;

    /// Evaluate module on a batch of examples.
    /// Returns average score across all examples.
    async fn evaluate(&self, examples: Vec<Example>) -> f32 {
        // Default: batch forward, then average metric scores
    }
}
```

**Usage:**

```rust
impl Evaluator for MyModule {
    async fn metric(&self, example: &Example, prediction: &Prediction) -> f32 {
        let expected = example.get("answer");
        let actual = prediction.get("answer", None);

        // Exact match: 1.0, otherwise 0.0
        if expected == actual { 1.0 } else { 0.0 }
    }
}

// Evaluate on test set
let score = module.evaluate(test_examples).await?;
println!("Accuracy: {:.2}%", score * 100.0);
```

---

### FeedbackEvaluator Trait

Extended evaluator with rich textual feedback (for GEPA optimizer).

```rust
// File: crates/dsrs/src/evaluate/feedback.rs

#[allow(async_fn_in_trait)]
pub trait FeedbackEvaluator {
    /// Evaluate with rich feedback.
    async fn feedback_metric(
        &self,
        example: &Example,
        prediction: &Prediction,
    ) -> FeedbackMetric;

    /// Multi-objective evaluation (multiple metrics).
    async fn multi_objective_metric(
        &self,
        example: &Example,
        prediction: &Prediction,
    ) -> Vec<FeedbackMetric>;
}
```

### FeedbackMetric

```rust
/// Metric result with rich feedback.
pub struct FeedbackMetric {
    /// Numerical score (0.0 to 1.0).
    pub score: f32,

    /// Rich textual explanation of the score.
    pub feedback: String,

    /// Optional structured metadata.
    pub metadata: HashMap<String, Value>,
}
```

**Usage:**

```rust
impl FeedbackEvaluator for MyModule {
    async fn feedback_metric(
        &self,
        example: &Example,
        prediction: &Prediction,
    ) -> FeedbackMetric {
        let expected = example.get("answer");
        let actual = prediction.get("answer", None);

        if expected == actual {
            FeedbackMetric {
                score: 1.0,
                feedback: "Exact match with expected answer.".to_string(),
                metadata: HashMap::new(),
            }
        } else {
            FeedbackMetric {
                score: 0.0,
                feedback: format!(
                    "Mismatch: expected '{}', got '{}'",
                    expected, actual
                ),
                metadata: HashMap::new(),
            }
        }
    }
}
```

---

### Metric Trait

Generic metric interface for composable evaluation.

```rust
// File: crates/dsrs/src/evaluate/metrics.rs

#[async_trait]
pub trait Metric: Send + Sync {
    /// Metric name for identification.
    fn name(&self) -> &str;

    /// Metric tier (Proxy or Truth).
    fn tier(&self) -> MetricTier;

    /// Estimated cost in msats.
    fn cost_estimate(&self) -> u64;

    /// Evaluate a single input/output pair.
    async fn evaluate(
        &self,
        input: &Example,
        output: &Example,
    ) -> Result<MetricScore>;

    /// Batch evaluation.
    async fn evaluate_batch(
        &self,
        pairs: &[(Example, Example)],
    ) -> Result<Vec<MetricScore>>;
}
```

### MetricTier

```rust
/// Classification of metric cost/accuracy tradeoff.
pub enum MetricTier {
    /// Cheap, fast metrics (format, syntax checks).
    Proxy,

    /// Expensive, accurate metrics (LLM judge, sandbox).
    Truth,
}
```

### MetricScore

```rust
/// Result from metric evaluation.
pub struct MetricScore {
    /// Score value (0.0 to 1.0).
    pub value: f64,

    /// Confidence in the score.
    pub confidence: f64,

    /// Optional details/explanation.
    pub details: Option<String>,

    /// Cost in millisatoshis.
    pub cost_msats: u64,
}
```

### MetricSet

```rust
/// Collection of metrics organized by tier.
pub struct MetricSet {
    /// Cheap, fast metrics (run first).
    pub proxy: Vec<BoxedMetric>,

    /// Expensive, accurate metrics (run if proxy passes).
    pub truth: Vec<BoxedMetric>,
}

impl MetricSet {
    /// Iterator in evaluation order (proxy first, then truth).
    pub fn all(&self) -> impl Iterator<Item = &BoxedMetric>;
}
```

---

## Proxy Metrics

Cheap, fast metrics that run first. If proxy metrics fail, truth metrics are skipped (saves cost).

### FormatMetric

Validates output format.

```rust
// File: crates/dsrs/src/evaluate/metrics/proxy.rs

use dsrs::evaluate::metrics::proxy::FormatMetric;

let metric = FormatMetric::builder()
    .require_non_empty(true)
    .require_valid_json(true)
    .field("answer")  // Check specific field
    .build();

let score = metric.evaluate(&input, &output).await?;
// score.value = 1.0 if valid, 0.0 if not
```

**Checks:**
- Non-empty output
- Valid JSON (optional)
- Field-specific validation

**Cost:** 0 msats (free)

---

### KeywordMetric

Validates presence/absence of keywords.

```rust
use dsrs::evaluate::metrics::proxy::KeywordMetric;

let metric = KeywordMetric::builder()
    .required(vec!["function", "return"])  // Must have ALL
    .forbidden(vec!["TODO", "FIXME"])      // Must have NONE
    .field("code")  // Check specific field
    .build();

let score = metric.evaluate(&input, &output).await?;
```

**Checks:**
- Required keywords (all must be present)
- Forbidden keywords (any fails check)
- Field-specific or full output

**Cost:** 0 msats (free)

---

### LengthMetric

Validates output length.

```rust
use dsrs::evaluate::metrics::proxy::LengthMetric;

let metric = LengthMetric::builder()
    .min_chars(10)
    .max_chars(1000)
    .field("summary")
    .build();

let score = metric.evaluate(&input, &output).await?;
// Partial credit for close-to-bounds
```

**Configuration:**
| Parameter | Description |
|-----------|-------------|
| `min_chars` | Minimum character count |
| `max_chars` | Maximum character count |
| `field` | Specific field to check |

**Cost:** 0 msats (free)

---

### SyntaxMetric

Validates syntactic correctness.

```rust
use dsrs::evaluate::metrics::proxy::SyntaxMetric;

let metric = SyntaxMetric::builder()
    .check_balanced_brackets(true)
    .check_balanced_quotes(true)
    .language(Some("rust"))
    .build();

let score = metric.evaluate(&input, &output).await?;
```

**Checks:**
- Balanced brackets: `()`, `[]`, `{}`
- Balanced quotes: `"`, `'`
- Unmatched delimiters
- Language-specific validation (optional)

**Cost:** 0 msats (free)

---

## Truth Metrics

Expensive, accurate metrics. Only run if proxy metrics pass.

### LlmJudgeMetric

Uses an LLM as evaluator.

```rust
// File: crates/dsrs/src/evaluate/metrics/truth.rs

use dsrs::evaluate::metrics::truth::LlmJudgeMetric;

let metric = LlmJudgeMetric::builder()
    .system_prompt("You are an expert code reviewer...")
    .criteria(vec![
        "Correctness",
        "Completeness",
        "Code quality",
    ])
    .build();

let score = metric.evaluate(&input, &output).await?;
// score.details contains LLM's explanation
```

**Configuration:**
| Parameter | Description |
|-----------|-------------|
| `system_prompt` | Instructions for the judge LLM |
| `criteria` | Evaluation criteria list |

**Cost:** ~100 msats (default)

---

### DiffMetric

Compares actual vs expected output.

```rust
use dsrs::evaluate::metrics::truth::DiffMetric;

let metric = DiffMetric::builder()
    .strictness(DiffStrictness::NormalizedWhitespace)
    .field("answer")
    .build();

let score = metric.evaluate(&input, &output).await?;
```

**Strictness Levels:**

| Level | Description | Cost |
|-------|-------------|------|
| `Exact` | Character-by-character match | 0 msats |
| `NormalizedWhitespace` | Whitespace-agnostic (default) | 0 msats |
| `AstEquivalent` | AST comparison (TODO) | 10 msats |
| `Semantic` | LLM-judged equivalence (TODO) | 50 msats |

---

### SandboxMetric

Executes commands in a sandbox to verify output.

```rust
use dsrs::evaluate::metrics::truth::SandboxMetric;

let metric = SandboxMetric::builder()
    .command("cargo test")
    .expected_exit_code(0)
    .timeout_secs(120)
    .build();

let score = metric.evaluate(&input, &output).await?;
// 1.0 if command succeeds, 0.0 otherwise
```

**Uses:** PylonSandboxProvider for execution

**Cost:** ~500 msats

---

### TestPassMetric

Runs unit tests in sandbox.

```rust
use dsrs::evaluate::metrics::truth::TestPassMetric;

let metric = TestPassMetric::builder()
    .command("cargo test")
    .build();

let score = metric.evaluate(&input, &output).await?;
```

**Checks:**
- Runs specified test command
- Checks exit code = 0
- Reports test output in details

**Default command:** `cargo test`

**Cost:** ~500 msats

---

## Adjutant-Specific Metrics

Custom metrics for Adjutant's task execution.

```rust
// File: crates/adjutant/src/dspy/metrics.rs
```

### subtask_planning_metric

Validates subtask planning output.

```rust
use adjutant::dspy::metrics::subtask_planning_metric;

let score = subtask_planning_metric(&example, &prediction);
```

**Checks (25% each):**
1. Subtasks is valid JSON array
2. Valid action types (read, edit, bash)
3. Valid target paths
4. Actionable instructions

---

### subtask_execution_metric

Validates subtask execution output.

```rust
use adjutant::dspy::metrics::subtask_execution_metric;

let score = subtask_execution_metric(&example, &prediction);
```

**Checks (33/33/34 split):**
1. Result is valid JSON
2. Edit strings or commands present
3. Substantive reasoning

---

### synthesis_metric

Validates result synthesis output.

```rust
use adjutant::dspy::metrics::synthesis_metric;

let score = synthesis_metric(&example, &prediction);
```

**Checks (25% each):**
1. Success boolean present
2. Substantive summary
3. Valid modified_files array
4. Calibrated confidence (0.0-1.0)

---

### tool_step_utility_metric

Validates tool step utility evaluation.

```rust
use adjutant::dspy::metrics::tool_step_utility_metric;

let score = tool_step_utility_metric(&example, &prediction);
```

**Checks (25% each):**
1. step_utility in range 0.0-1.0
2. should_continue boolean
3. Substantive next_action_hint
4. Calibrated confidence

---

### combined_metric

Weighted combination of planning and synthesis metrics.

```rust
use adjutant::dspy::metrics::combined_metric;

let score = combined_metric(&example, &prediction);
// 60% planning weight + 40% synthesis weight
```

---

## Scoring Infrastructure

### Scorer

Orchestrates multi-metric evaluation with budget awareness.

```rust
// File: crates/dsrs/src/evaluate/scoring.rs

use dsrs::evaluate::scoring::{Scorer, ScorerBuilder};

let scorer = ScorerBuilder::new()
    .with_default_proxy_metrics()   // Format, Length, Syntax
    .with_default_truth_metrics()   // LlmJudge, Diff
    .threshold(0.5)                 // Min proxy score to run truth
    .num_rollouts(3)               // Attempts per example
    .aggregation(AggregationMethod::Median)
    .build();

let score = scorer.score(&input, &output).await?;
```

### Scoring Flow

```
Input/Output Pair
        │
        ▼
┌───────────────────┐
│  Proxy Metrics    │  ← Cheap, fast
│  (Format, Length, │
│   Syntax, Keyword)│
└────────┬──────────┘
         │
         ▼
    Proxy Score >= Threshold?
         │
    ┌────┴────┐
    │         │
   Yes        No
    │         │
    ▼         ▼
┌──────────┐  Return
│  Truth   │  Proxy
│  Metrics │  Score
│ (LlmJudge│
│  Sandbox)│
└────┬─────┘
     │
     ▼
  Aggregate
   Scores
     │
     ▼
  Final Score
```

### AggregationMethod

```rust
pub enum AggregationMethod {
    /// Middle value (robust to outliers).
    Median,

    /// Simple average.
    Mean,

    /// Worst-case score.
    Min,

    /// Best-case score.
    Max,

    /// Drop percentage from each end before averaging.
    TrimmedMean(f64),
}
```

### ScoringConfig

```rust
pub struct ScoringConfig {
    /// Minimum proxy score to run truth metrics.
    pub threshold: f32,

    /// Number of rollouts per example.
    pub num_rollouts: usize,

    /// How to aggregate multiple rollouts.
    pub aggregation: AggregationMethod,

    /// Budget limit in msats.
    pub budget_msats: Option<u64>,
}
```

---

## Creating Custom Metrics

### Simple Custom Metric

```rust
use dsrs::evaluate::metrics::{Metric, MetricTier, MetricScore};

struct MyCustomMetric {
    name: String,
}

#[async_trait]
impl Metric for MyCustomMetric {
    fn name(&self) -> &str {
        &self.name
    }

    fn tier(&self) -> MetricTier {
        MetricTier::Proxy  // or MetricTier::Truth
    }

    fn cost_estimate(&self) -> u64 {
        0  // Free for proxy metrics
    }

    async fn evaluate(
        &self,
        input: &Example,
        output: &Example,
    ) -> Result<MetricScore> {
        // Custom evaluation logic
        let score = calculate_score(input, output);

        Ok(MetricScore {
            value: score,
            confidence: 1.0,
            details: Some("Explanation".to_string()),
            cost_msats: 0,
        })
    }

    async fn evaluate_batch(
        &self,
        pairs: &[(Example, Example)],
    ) -> Result<Vec<MetricScore>> {
        // Default: sequential evaluation
        let mut scores = Vec::new();
        for (input, output) in pairs {
            scores.push(self.evaluate(input, output).await?);
        }
        Ok(scores)
    }
}
```

### Custom Evaluator for Module

```rust
impl Evaluator for MyModule {
    async fn metric(&self, example: &Example, prediction: &Prediction) -> f32 {
        let mut score = 0.0;

        // Check correctness (50%)
        if prediction.get("answer", None) == example.get("expected_answer") {
            score += 0.5;
        }

        // Check format (25%)
        if prediction.data.contains_key("explanation") {
            score += 0.25;
        }

        // Check confidence calibration (25%)
        if let Some(conf) = prediction.get("confidence", None).as_f64() {
            if (0.0..=1.0).contains(&conf) {
                score += 0.25;
            }
        }

        score
    }
}
```

---

## Metric Index

| Metric | Location | Tier | Cost | Purpose |
|--------|----------|------|------|---------|
| **Proxy Metrics** |
| FormatMetric | `dsrs/src/evaluate/metrics/proxy.rs` | Proxy | 0 | Format validation |
| KeywordMetric | `dsrs/src/evaluate/metrics/proxy.rs` | Proxy | 0 | Keyword checking |
| LengthMetric | `dsrs/src/evaluate/metrics/proxy.rs` | Proxy | 0 | Length bounds |
| SyntaxMetric | `dsrs/src/evaluate/metrics/proxy.rs` | Proxy | 0 | Syntax validation |
| **Truth Metrics** |
| LlmJudgeMetric | `dsrs/src/evaluate/metrics/truth.rs` | Truth | ~100 | LLM evaluation |
| DiffMetric | `dsrs/src/evaluate/metrics/truth.rs` | Truth | 0-50 | Output comparison |
| SandboxMetric | `dsrs/src/evaluate/metrics/truth.rs` | Truth | ~500 | Command execution |
| TestPassMetric | `dsrs/src/evaluate/metrics/truth.rs` | Truth | ~500 | Test execution |
| **Adjutant Metrics** |
| subtask_planning_metric | `adjutant/src/dspy/metrics.rs` | Custom | 0 | Planning validation |
| subtask_execution_metric | `adjutant/src/dspy/metrics.rs` | Custom | 0 | Execution validation |
| synthesis_metric | `adjutant/src/dspy/metrics.rs` | Custom | 0 | Synthesis validation |
| tool_step_utility_metric | `adjutant/src/dspy/metrics.rs` | Custom | 0 | Utility validation |
| combined_metric | `adjutant/src/dspy/metrics.rs` | Custom | 0 | Weighted combination |

---

## Integration with Other Primitives

| Primitive | Relationship |
|-----------|--------------|
| **Signatures** | Metrics validate signature outputs |
| **Modules** | Modules implement Evaluator trait |
| **Tools** | Tool results feed into metrics |
| **Adapters** | Parsed outputs evaluated by metrics |
| **Optimizers** | Metrics drive optimization objective |
