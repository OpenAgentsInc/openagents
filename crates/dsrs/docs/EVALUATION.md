# Evaluation System (Wave 5)

> **Status:** Accurate
> **Last verified:** d44f9cd3f
> **Source of truth:** `crates/dsrs/src/evaluate/`
> **Doc owner:** dsrs
> **If this doc conflicts with code, code wins.**

Eval harness and promotion gates for safe, cost-efficient module optimization.

## Overview

The evaluation system provides:
- Standardized eval task format for reproducible benchmarks
- Two-tier metrics (proxy + truth) for cost efficiency
- Robust scoring with median aggregation across rollouts
- Promotion gates for staged rollout with shadow mode

## Eval Tasks

Define benchmark scenarios with `EvalTask`:

```rust
use dsrs::evaluate::{EvalTask, RepoContext, Constraint, GoldFile};

let task = EvalTask::new("task-001", "Fix the authentication bug")
    .with_repo(RepoContext::new("/path/to/repo").with_ref("main"))
    .with_constraint(Constraint::max_tokens(1000))
    .with_constraint(Constraint::budget_msats(5000))
    .with_gold_files(vec![
        GoldFile::new("src/auth.rs", "fn authenticate() { /* fixed */ }")
    ])
    .with_tag("bug-fix");
```

### Task Sets

Group tasks into benchmarks:

```rust
use dsrs::evaluate::EvalTaskSet;

let benchmark = EvalTaskSet::new("retrieval-benchmark")
    .with_description("Benchmark for retrieval tasks")
    .with_task(task1)
    .with_task(task2);

// Filter by tag or difficulty
let retrieval_tasks = benchmark.tasks_with_tag("retrieval");
let hard_tasks = benchmark.tasks_in_difficulty_range(0.7, 1.0);
```

## Two-Tier Metrics

### Proxy Metrics (Cheap, Fast)

Run frequently to filter bad outputs before expensive evaluation:

| Metric | Description | Cost |
|--------|-------------|------|
| `FormatMetric` | JSON/output format validity | Free |
| `KeywordMetric` | Required keywords present | Free |
| `LengthMetric` | Output length bounds | Free |
| `SyntaxMetric` | Code syntax validity | Free |

```rust
use dsrs::evaluate::metrics::proxy::*;

let format = FormatMetric::new().with_json_check();
let keywords = KeywordMetric::new()
    .require(vec!["function", "return"])
    .forbid(vec!["TODO", "FIXME"]);
let length = LengthMetric::new().between(10, 1000);
let syntax = SyntaxMetric::new().in_field("code");
```

### Truth Metrics (Expensive, Accurate)

Run only after proxy metrics pass:

| Metric | Description | Cost |
|--------|-------------|------|
| `LlmJudgeMetric` | LLM-as-judge scoring | ~100 msats |
| `SandboxMetric` | Code execution verification | ~500 msats |
| `DiffMetric` | Semantic diff against gold | 0-50 msats |
| `TestPassMetric` | Unit tests pass | ~500 msats |

```rust
use dsrs::evaluate::metrics::truth::*;

let judge = LlmJudgeMetric::new()
    .with_criteria(vec!["Correctness", "Completeness"])
    .with_cost(150);

let sandbox = SandboxMetric::new()
    .with_commands(vec!["cargo test", "cargo clippy"])
    .with_timeout(300);
```

### Custom Metrics

Implement the `Metric` trait:

```rust
use dsrs::evaluate::{Metric, MetricTier, MetricScore};
use async_trait::async_trait;

struct MyMetric { /* ... */ }

#[async_trait]
impl Metric for MyMetric {
    fn name(&self) -> &str { "my_metric" }
    fn tier(&self) -> MetricTier { MetricTier::Proxy }
    fn cost_estimate(&self) -> u64 { 0 }

    async fn evaluate(&self, input: &Example, output: &Example) -> Result<MetricScore> {
        // Your evaluation logic
        Ok(MetricScore::new(0.9).with_details("Good"))
    }
}
```

## Scoring

### Configuration

```rust
use dsrs::evaluate::{ScoringConfig, AggregationMethod, ScorerBuilder};

let scorer = ScorerBuilder::new()
    .with_default_proxy_metrics()
    .with_default_truth_metrics()
    .rollouts(5)                           // 5 rollouts per task
    .aggregation(AggregationMethod::Median) // Median aggregation
    .proxy_threshold(0.8)                   // Skip truth if proxy < 0.8
    .max_budget(10000)                      // 10000 msats max
    .build();
```

### Aggregation Methods

| Method | Description | Use Case |
|--------|-------------|----------|
| `Median` | Middle value | Robust to outliers (default) |
| `Mean` | Average | Smooth scores |
| `Min` | Lowest value | Pessimistic evaluation |
| `Max` | Highest value | Optimistic evaluation |
| `TrimmedMean(N)` | Mean after dropping N% from ends | Balance robustness and smoothness |

### Running Evaluation

```rust
let scorecard = scorer.score(&predictor, &tasks).await?;

println!("Overall: {:.2}", scorecard.overall_score);
println!("Tasks evaluated: {}", scorecard.tasks_evaluated);
println!("Total cost: {} msats", scorecard.total_cost_msats);

for (metric, score) in &scorecard.per_metric {
    println!("  {}: {:.2}", metric, score);
}
```

## Promotion Gates

### Pipeline States

```
Candidate → Staged → Shadow → Promoted
                               ↓
                           RolledBack
```

| State | Description |
|-------|-------------|
| `Candidate` | Newly compiled, untested |
| `Staged` | Passed proxy metrics |
| `Shadow` | A/B testing alongside production |
| `Promoted` | In production |
| `RolledBack` | Failed after promotion |

### Default Gates

| Gate | Transition | Requirements |
|------|------------|--------------|
| Proxy | Candidate → Staged | format ≥ 0.95, syntax ≥ 0.90 |
| Truth | Staged → Shadow | llm_judge ≥ 0.80, beat baseline by 2% |
| Shadow | Shadow → Promoted | 100 samples, 52% win rate |

### Using Promotion Manager

```rust
use dsrs::evaluate::{PromotionManager, PromotionState};

let gates = PromotionManager::default_gates();
let manager = PromotionManager::with_gates(gates, scorer);

let result = manager.try_promote(
    &candidate_module,
    &manifest,
    PromotionState::Candidate,
    &eval_tasks,
).await?;

if result.success {
    println!("Promoted to {:?}", result.new_state);
} else {
    println!("Failed: {}", result.reason);
    for req in &result.requirement_results {
        if !req.passed {
            println!("  {} - {}", req.requirement, req.reason);
        }
    }
}
```

### Shadow Mode

Compare candidate against production:

```rust
let shadow_result = manager.run_shadow(
    &candidate_module,
    &production_module,
    &eval_tasks,
).await?;

println!("Candidate wins: {}", shadow_result.candidate_wins);
println!("Production wins: {}", shadow_result.production_wins);
println!("Ties: {}", shadow_result.ties);
println!("Win rate: {:.1}%", shadow_result.candidate_win_rate() * 100.0);

if shadow_result.should_promote(100, 0.52) {
    println!("Ready for promotion!");
}
```

### Custom Gates

```rust
use dsrs::evaluate::{PromotionGate, GateRequirement, PromotionState};

let custom_gate = PromotionGate::new(
    "strict_gate",
    PromotionState::Staged,
    PromotionState::Shadow,
)
.with_requirements(vec![
    GateRequirement::min_score("llm_judge", 0.90),
    GateRequirement::min_score("tests_pass", 1.0),
    GateRequirement::beat_baseline("latency", -0.1), // 10% faster
]);
```

## Compile Priority

Schedule which modules to optimize:

```rust
use dsrs::evaluate::{CompileQueue, CompileQueueBuilder, PriorityFactors};

let mut queue = CompileQueueBuilder::new()
    .max_concurrent(3)
    .module("retrieval_router", PriorityFactors::new()
        .with_invocation_rate(1000.0)  // 1000 calls/day
        .with_failure_rate(0.1)        // 10% failures
        .with_avg_latency(500.0)       // 500ms avg
        .with_staleness(30.0))         // 30 days old
    .module("query_composer", PriorityFactors::new()
        .with_invocation_rate(500.0)
        .with_failure_rate(0.05))
    .build();

// Get highest priority module
while let Some(priority) = queue.pop() {
    println!("Compiling {} (score: {:.2})", priority.module_id, priority.score);

    // Compile...

    queue.complete(&priority.module_id);
}
```

### Priority Formula

```
score = invocation_rate * (
    failure_rate * 3.0 +      // Failures weighted heavily
    latency_factor +           // Normalized to 1.0 at 1000ms
    cost_factor +              // Normalized to 1.0 at 10000 msats
    staleness * 0.5 +          // Normalized to 1.0 at 30 days
    issues * 0.2               // 0.2 per reported issue
)
```

## Integration with Manifest

Track promotion state and evaluation history:

```rust
use dsrs::manifest::CompiledModuleManifest;
use dsrs::evaluate::{PromotionState, EvalRecord};

let manifest = CompiledModuleManifest::new("MySignature", "MIPROv2")
    .with_promotion_state(PromotionState::Staged)
    .with_eval_record(EvalRecord::new(
        PromotionState::Candidate,
        scorecard,
    ).with_promotion_result(true, "Passed proxy gate"));
```

## Best Practices

1. **Start with proxy metrics** - They're free and fast
2. **Use multiple rollouts** (3-5) for statistical robustness
3. **Set appropriate thresholds** - Don't be too strict initially
4. **Monitor shadow mode** - Ensure sufficient samples before promotion
5. **Track eval history** - Debug regressions with historical data
6. **Budget wisely** - Truth metrics cost real money

## Example: Full Pipeline

```rust
use dsrs::evaluate::*;

// 1. Define tasks
let tasks = EvalTaskSet::new("my-benchmark")
    .with_task(EvalTask::new("t1", "Task 1"))
    .with_task(EvalTask::new("t2", "Task 2"));

// 2. Configure scorer
let scorer = ScorerBuilder::new()
    .with_default_proxy_metrics()
    .with_default_truth_metrics()
    .rollouts(3)
    .build();

// 3. Set up promotion manager
let manager = PromotionManager::with_gates(
    PromotionManager::default_gates(),
    scorer,
);

// 4. Evaluate and promote
let result = manager.try_promote(
    &candidate,
    &manifest,
    manifest.promotion_state,
    &tasks.tasks,
).await?;

// 5. If in shadow, run comparison
if manifest.promotion_state == PromotionState::Shadow {
    let shadow = manager.run_shadow(&candidate, &production, &tasks.tasks).await?;
    if shadow.should_promote(100, 0.52) {
        // Promote to production
    }
}
```
