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
| `rlm` | Core engine with DSPy integration, provenance tracking, and tools |

## Key Features

### DSPy Integration (Feature: `dspy`)

The RLM crate integrates with [dspy-rs](https://github.com/dspy-rs/dspy-rs) for declarative LLM programming:

- **Provenance-first signatures** - All signatures track evidence origins via SpanRef
- **LmRouter bridge** - Per-request LM routing with unified cost tracking
- **Environment tools** - Repository traversal tools that return SpanRefs
- **4-phase orchestrator** - Router → Extractor → Reducer → Verifier pipeline

### Provenance Tracking

Every piece of evidence can be traced to its exact source:

```rust
use rlm::SpanRef;

let span = SpanRef::from_chunk(
    chunk_id,
    "docs/spec.md",
    Some("abc123def"),  // Git commit
    10, 25,             // Line range
    500, 1200,          // Byte range
    &content,           // For hash computation
);

// Verify content hasn't changed
if span.verify_content(&current_content) {
    println!("Evidence verified");
}
```

### Environment Tools

Tools for repository exploration that return provenance-tracked results:

| Tool | Purpose |
|------|---------|
| `GrepTool` | Pattern search with SpanRef results |
| `ReadLinesTool` | Read file ranges with provenance |
| `ListFilesTool` | Directory traversal with metadata |
| `SymbolsTool` | Extract code symbols (functions, classes, etc.) |

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

### DSPy Quick Start

```rust
use rlm::{DspyOrchestrator, SpanRef, GrepTool};
use lm_router::LmRouter;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Set up LmRouter
    let router = Arc::new(LmRouter::builder()
        .add_backend(/* your backend */)
        .build());

    // 2. Create orchestrator with per-request LM
    let orchestrator = DspyOrchestrator::with_lm_router(router.clone(), "model-name")
        .await?
        .with_document_path("docs/spec.md")
        .with_commit("abc123");

    // 3. Analyze a document
    let result = orchestrator.analyze(
        "What authentication methods are supported?",
        &document_content
    ).await?;

    // 4. Results include provenance
    for extraction in &result.extractions {
        println!("Finding: {}", extraction.findings);
        if let Some(span) = &extraction.span_ref {
            println!("  Source: {}:{}-{}", span.path, span.start_line, span.end_line);
        }
    }

    // 5. Use tools for additional exploration
    let grep = GrepTool::new(repo_path);
    let hits = grep.search("TODO", &["**/*.rs"], 20).await?;
    for hit in hits {
        println!("{}:{} - {}", hit.span.path, hit.span.start_line, hit.line);
    }

    Ok(())
}
```

## Documentation Index

### Core Documentation

- [Architecture](./ARCHITECTURE.md) - Crate structure, module organization, and design decisions
- [DSPy Integration](./DSPY.md) - DSPy (dspy-rs) for declarative LLM programming
- [Provenance Tracking](./PROVENANCE.md) - SpanRef type for Git-aware evidence tracking
- [Environment Tools](./TOOLS.md) - Repository traversal tools (grep, read, list, symbols)

### Benchmarking Infrastructure

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

## Feature Flags

The `rlm` crate uses feature flags to control optional functionality:

```toml
[dependencies]
rlm = { path = "crates/rlm" }  # Core only

# With DSPy integration
rlm = { path = "crates/rlm", features = ["dspy"] }
```

| Feature | Description |
|---------|-------------|
| `dspy` | DSPy integration: signatures, orchestrator, LmRouter bridge, tools |

## Module Summary

### Always Available

| Module | Description |
|--------|-------------|
| `engine` | RlmEngine execution loop |
| `client` | LlmClient trait and response types |
| `chunking` | Structure-aware document chunking |
| `span` | SpanRef for provenance tracking |
| `context` | Context management |
| `command` | Command parsing |
| `orchestrator` | High-level analysis orchestration |

### With `dspy` Feature

| Module | Description |
|--------|-------------|
| `dspy_bridge` | Global LM config + LmRouter bridge |
| `dspy_orchestrator` | 4-phase document analysis pipeline |
| `signatures` | Provenance-first DSPy signatures |
| `tools` | Environment tools (grep, read, list, symbols) |

## License

CC-0
