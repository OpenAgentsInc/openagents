# Running Experiments

This guide covers how to run benchmark experiments using the RLM replication infrastructure.

## Basic Workflow

```rust
use std::sync::Arc;
use bench_harness::{ExperimentConfig, ExperimentRunner, ExactMatchMetric};
use bench_datasets::{Dataset, SnihDataset, DatasetConfig};
use rlm_methods::BaseMethod;
use lm_router::{LmRouter, backends::FmBridgeBackend};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Set up LM backend
    let router = Arc::new(
        LmRouter::builder()
            .add_backend(FmBridgeBackend::new())
            .default_backend("fm-bridge")
            .build()
    );

    // 2. Load dataset
    let dataset = SnihDataset::new(DatasetConfig::new("./data/sniah"));
    let tasks = dataset.load().await?;

    // 3. Create methods
    let base = Arc::new(BaseMethod::new(router.clone(), "model-name"));

    // 4. Configure experiment
    let config = ExperimentConfig::new("sniah-experiment")
        .output_dir("./results");

    // 5. Run experiment
    let mut runner = ExperimentRunner::new(config, tasks);
    runner.add_method(base);
    runner.add_metric(Box::new(ExactMatchMetric));

    let results = runner.run().await?;

    // 6. Report results
    println!("Results: {:#?}", results);

    Ok(())
}
```

## Configuration Options

### Experiment Config

```rust
let config = ExperimentConfig::new("experiment-name")
    .output_dir("./results")   // Output directory
    .resume(true)              // Resume from checkpoint (default)
    .max_tasks(100)            // Limit tasks (optional)
    .seed(42);                 // Random seed (optional)
```

### Dataset Config

```rust
let dataset_config = DatasetConfig::new("./data/dataset")
    .max_tasks(50)      // Limit loading
    .seed(42)           // Sampling seed
    .shuffle();         // Randomize order
```

## Running Multiple Methods

Compare different methods on the same dataset:

```rust
let router = Arc::new(router);

// Create methods
let base = Arc::new(BaseMethod::new(router.clone(), "model"));
let summary = Arc::new(SummaryAgentMethod::new(router.clone(), "model"));

// Run experiment
let mut runner = ExperimentRunner::new(config, tasks);
runner.add_method(base);
runner.add_method(summary);
runner.add_metric(Box::new(ExactMatchMetric));

let results = runner.run().await?;

// Print comparison
println!("Method Comparison:");
println!("{:-<60}", "");
for (method, stats) in &results.per_method {
    println!("{:<20} Score: {:.2}%  Tokens: {:.0}  Time: {:.0}ms",
        method,
        stats.primary_score * 100.0,
        stats.mean_usage.total_tokens as f64,
        stats.mean_duration_ms
    );
}
```

## Running Multiple Datasets

```rust
async fn run_all_benchmarks(router: Arc<LmRouter>) -> Result<()> {
    let datasets: Vec<Box<dyn Dataset<Task = SimpleTask>>> = vec![
        Box::new(SnihDataset::new(DatasetConfig::new("./data/sniah"))),
        Box::new(OolongDataset::trec_coarse("./data/oolong")),
        Box::new(OolongDataset::pairs("./data/oolong")),
    ];

    for dataset in datasets {
        let tasks = dataset.load().await?;
        let config = ExperimentConfig::new(dataset.name())
            .output_dir(format!("./results/{}", dataset.name()));

        let mut runner = ExperimentRunner::new(config, tasks);
        runner.add_method(Arc::new(BaseMethod::new(router.clone(), "model")));

        // Add appropriate metric based on dataset
        match dataset.primary_metric() {
            "exact_match" => runner.add_metric(Box::new(ExactMatchMetric)),
            "numeric_decay" => runner.add_metric(Box::new(NumericDecayMetric)),
            "f1" => runner.add_metric(Box::new(F1Metric)),
            _ => runner.add_metric(Box::new(ExactMatchMetric)),
        }

        let results = runner.run().await?;
        println!("{}: {:.2}%", dataset.name(), results.per_method["base"].primary_score * 100.0);
    }

    Ok(())
}
```

## Output Files

Each experiment creates these files:

```
results/
├── experiment_checkpoint.json     # For resuming interrupted runs
├── experiment_results.jsonl       # Per-task results
├── experiment_trajectories.jsonl  # Execution traces
└── experiment_final.json          # Aggregated final results
```

### results.jsonl Format

```json
{"task_id":"task-001","method":"base","answer":"42","ground_truth":{"ExactMatch":"42"},"scores":{"exact_match":1.0},"usage":{"prompt_tokens":100,"completion_tokens":50,"total_tokens":150},"duration_ms":1500,"success":true,"error":null}
```

### trajectories.jsonl Format

```json
{"task_id":"task-001","method":"base","steps":[{"step_id":0,"timestamp_ms":0,"step_type":{"type":"LlmCall","model":"gpt-4","prompt_tokens":100,"completion_tokens":50},"content":"The answer is 42"},{"step_id":1,"timestamp_ms":100,"step_type":{"type":"Final","answer":"42"},"content":"42"}],"start_time":1704067200000,"end_time":1704067201500}
```

### final.json Format

```json
{
  "config": {
    "name": "experiment",
    "output_dir": "./results",
    "resume": true
  },
  "per_method": {
    "base": {
      "method": "base",
      "primary_score": 0.85,
      "std_dev": 0.12,
      "metric_scores": {"exact_match": 0.85},
      "total_tasks": 50,
      "successful_tasks": 50,
      "total_usage": {"prompt_tokens": 5000, "completion_tokens": 2500, "total_tokens": 7500},
      "mean_usage": {"prompt_tokens": 100, "completion_tokens": 50, "total_tokens": 150},
      "mean_duration_ms": 1500.0
    }
  },
  "total_usage": {"prompt_tokens": 5000, "completion_tokens": 2500, "total_tokens": 7500}
}
```

## Resuming Experiments

Experiments automatically checkpoint. To resume:

```rust
// Same config as before
let config = ExperimentConfig::new("experiment-name")
    .output_dir("./results")
    .resume(true);  // This is the default

// Load same tasks in same order
let tasks = dataset.load().await?;

// Create runner - will skip completed tasks
let mut runner = ExperimentRunner::new(config, tasks);
runner.add_method(method);

let results = runner.run().await?;  // Continues from checkpoint
```

To force a fresh start:

```rust
let config = ExperimentConfig::new("experiment-name")
    .resume(false);  // Don't load checkpoint
```

## Debugging Tips

### Limit Tasks

```rust
let config = ExperimentConfig::new("debug-run")
    .max_tasks(5);  // Only run 5 tasks
```

### Use Mock Backend

```rust
use lm_router::backends::MockBackend;

let mock = MockBackend::new()
    .with_model("test-model")
    .with_response("Mock answer: 42");

let router = LmRouter::builder()
    .add_backend(mock)
    .default_backend("mock")
    .build();
```

### Use Synthetic Data

```rust
use bench_datasets::sniah::generate_synthetic_tasks;

let tasks = generate_synthetic_tasks(10, 1000);  // 10 tasks, 1000 char context
```

### Inspect Trajectories

```rust
let result = method.solve(&task).await?;

for step in &result.trajectory.steps {
    println!("Step {}: {:?}", step.step_id, step.step_type);
    println!("  Content: {}", &step.content[..100.min(step.content.len())]);
}
```

## Testing with Swarm Simulation

Test distributed scenarios:

```rust
use lm_router::backends::{SwarmSimulator, SwarmSimConfig, LatencyDist};

let config = SwarmSimConfig {
    latency: LatencyDist::Normal { mean_ms: 500, std_ms: 200 },
    failure_rate: 0.1,
    timeout_rate: 0.05,
    quorum_size: 3,
    ..Default::default()
};

let swarm = SwarmSimulator::new(config);
let router = LmRouter::builder()
    .add_backend(swarm)
    .default_backend("swarm-sim")
    .build();
```

## Cost Tracking

Monitor token usage and costs:

```rust
let results = runner.run().await?;

// Per-method usage
for (method, stats) in &results.per_method {
    println!("{} usage:", method);
    println!("  Total tokens: {}", stats.total_usage.total_tokens);
    println!("  Mean tokens/task: {}", stats.mean_usage.total_tokens);
}

// Total usage
println!("Total tokens: {}", results.total_usage.total_tokens);

// Router-level tracking
let report = router.usage_report();
println!("Total cost: ${:.4}", report.total_cost_usd);
```

## Parallel Execution

The experiment runner processes tasks sequentially by default. For parallel execution across methods, run separate experiments:

```rust
use tokio::task::JoinSet;

let mut tasks = JoinSet::new();

for method in methods {
    let router = router.clone();
    let config = config.clone();
    let dataset_tasks = tasks.clone();

    tasks.spawn(async move {
        let mut runner = ExperimentRunner::new(
            config.with_name(method.name()),
            dataset_tasks,
        );
        runner.add_method(Arc::new(method));
        runner.run().await
    });
}

while let Some(result) = tasks.join_next().await {
    let results = result??;
    // Process results
}
```

## Full Example

```rust
use std::sync::Arc;
use bench_harness::{
    ExperimentConfig, ExperimentRunner,
    ExactMatchMetric, F1Metric, NumericDecayMetric,
};
use bench_datasets::{
    Dataset, DatasetConfig,
    SnihDataset, BrowseCompDataset, OolongDataset, CodeQADataset,
};
use rlm_methods::{BaseMethod, SummaryAgentMethod};
use lm_router::{LmRouter, backends::FmBridgeBackend};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup router
    let router = Arc::new(
        LmRouter::builder()
            .add_backend(FmBridgeBackend::new())
            .default_backend("fm-bridge")
            .build()
    );

    // Define benchmarks
    let benchmarks = vec![
        ("sniah", "exact_match"),
        ("browsecomp", "exact_match"),
        ("oolong-trec", "numeric_decay"),
        ("oolong-pairs", "f1"),
    ];

    // Run each benchmark
    for (name, metric) in benchmarks {
        println!("\n=== Running {} ===", name);

        // Load dataset
        let tasks = match name {
            "sniah" => SnihDataset::new(DatasetConfig::new("./data/sniah"))
                .load().await?,
            "browsecomp" => BrowseCompDataset::new(DatasetConfig::new("./data/browsecomp"))
                .load().await?,
            "oolong-trec" => OolongDataset::trec_coarse("./data/oolong")
                .load().await?,
            "oolong-pairs" => OolongDataset::pairs("./data/oolong")
                .load().await?,
            _ => continue,
        };

        // Configure experiment
        let config = ExperimentConfig::new(name)
            .output_dir(format!("./results/{}", name));

        // Setup runner
        let mut runner = ExperimentRunner::new(config, tasks);

        // Add methods
        runner.add_method(Arc::new(BaseMethod::new(router.clone(), "model")));
        runner.add_method(Arc::new(SummaryAgentMethod::new(router.clone(), "model")));

        // Add metric
        match metric {
            "exact_match" => runner.add_metric(Box::new(ExactMatchMetric)),
            "numeric_decay" => runner.add_metric(Box::new(NumericDecayMetric)),
            "f1" => runner.add_metric(Box::new(F1Metric)),
            _ => runner.add_metric(Box::new(ExactMatchMetric)),
        }

        // Run
        let results = runner.run().await?;

        // Report
        println!("\nResults for {}:", name);
        for (method, stats) in &results.per_method {
            println!("  {}: {:.2}%", method, stats.primary_score * 100.0);
        }
    }

    // Final summary
    println!("\n=== Usage Report ===");
    let report = router.usage_report();
    println!("Total calls: {}", report.total_calls);
    println!("Total tokens: {}", report.total_tokens);

    Ok(())
}
```
