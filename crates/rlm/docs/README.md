# RLM Paper Replication Infrastructure

Infrastructure for replicating the Recursive Language Models paper (arXiv 2512.24601v1) using modular Rust crates designed for reuse across future paper replications.

## Overview

This project implements a benchmarking framework for evaluating language model methods on long-context tasks. The architecture is split into reusable components:

| Crate | Purpose |
|-------|---------|
| `lm-router` | Multi-backend LM routing with usage tracking |
| `bench-harness` | Generic experiment infrastructure |
| `bench-datasets` | Dataset loaders for benchmarks |
| `rlm-methods` | Paper-specific method implementations |

## Quick Start

```rust
use std::sync::Arc;
use bench_harness::{ExperimentConfig, ExperimentRunner, ExactMatchMetric};
use bench_datasets::{Dataset, SnihDataset, DatasetConfig};
use rlm_methods::BaseMethod;
use lm_router::{LmRouter, backends::FmBridgeBackend};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Set up the LM router with FM Bridge
    let fm_backend = FmBridgeBackend::new();
    let router = Arc::new(
        LmRouter::builder()
            .add_backend(fm_backend)
            .default_backend("fm-bridge")
            .build()
    );

    // 2. Load a dataset
    let dataset = SnihDataset::new(DatasetConfig::new("./data/sniah"));
    let tasks = dataset.load().await?;

    // 3. Create a method
    let method = Arc::new(BaseMethod::new(router, "apple-fm-model"));

    // 4. Run the experiment
    let config = ExperimentConfig::new("sniah-base")
        .output_dir("./results");

    let mut runner = ExperimentRunner::new(config, tasks);
    runner.add_method(method);
    runner.add_metric(Box::new(ExactMatchMetric));

    let results = runner.run().await?;
    println!("Score: {:.2}%", results.per_method["base"].primary_score * 100.0);

    Ok(())
}
```

## Documentation Index

- [Architecture](./ARCHITECTURE.md) - Crate structure and design decisions
- [DSPy Integration](./DSPY.md) - DSPy (dspy-rs) for declarative LLM programming
- [LM Router](./LM_ROUTER.md) - Multi-backend LM routing
- [Bench Harness](./BENCH_HARNESS.md) - Experiment infrastructure
- [Datasets](./DATASETS.md) - Dataset formats and loaders
- [Methods](./METHODS.md) - Method implementations
- [Running Experiments](./RUNNING_EXPERIMENTS.md) - How to run benchmarks

## Paper Methods

The RLM paper compares 5 methods:

| Method | Description | Implementation |
|--------|-------------|----------------|
| Base | Direct LLM call with full context | `rlm_methods::BaseMethod` |
| Summary Agent | Iterative context summarization | `rlm_methods::SummaryAgentMethod` |
| CodeAct+BM25 | ReAct with BM25 retrieval | Planned |
| RLM | Recursive LM with `llm_query` | Planned |
| RLM (no sub-calls) | Ablation without recursion | Planned |

## Benchmarks

| Dataset | Tasks | Metric | Description |
|---------|-------|--------|-------------|
| S-NIAH | 50 | Exact Match | Single-needle in a haystack |
| BrowseComp-Plus | 150 | Exact Match | Web browsing comprehension |
| OOLONG (TrecCoarse) | - | Numeric Decay | Document counting queries |
| OOLONG (Pairs) | - | F1 | Pairwise aggregation |
| CodeQA | - | Accuracy | Multiple-choice code comprehension |

## Primary Backend

The primary LLM backend is **FM Bridge** (Apple Foundation Models). A swarm simulator is also available for testing distributed NIP-90 scenarios.

## License

CC-0
