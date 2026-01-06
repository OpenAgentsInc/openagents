# Architecture

## Design Philosophy

The RLM replication infrastructure is designed with modularity and reusability as primary goals. Each crate serves a distinct purpose and can be used independently for other paper replications.

## Crate Dependency Graph

```
                    ┌─────────────────┐
                    │   rlm-methods   │
                    │  (paper-specific)│
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
    ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────────────┐
    │bench-datasets│  │bench-harness│  │              rlm                │
    │  (reusable) │  │  (reusable) │  │         (core engine)           │
    └──────┬──────┘  └──────┬──────┘  │                                 │
           │                │          │  ┌───────────────────────────┐ │
           └────────┬───────┘          │  │    dspy_orchestrator      │ │
                    │                  │  │    (feature: dspy)        │ │
                    │                  │  └─────────────┬─────────────┘ │
                    │                  │                │               │
                    │                  │  ┌─────────────▼─────────────┐ │
                    │                  │  │       dspy_bridge         │ │
                    │                  │  │  (LM config + LmRouter)   │ │
                    │                  │  └─────────────┬─────────────┘ │
                    │                  │                │               │
                    │                  │  ┌─────────────▼─────────────┐ │
                    │                  │  │         tools/            │ │
                    │                  │  │  grep, read, list, symbols│ │
                    │                  │  └─────────────┬─────────────┘ │
                    │                  │                │               │
                    │                  │  ┌─────────────▼─────────────┐ │
                    │                  │  │          span             │ │
                    │                  │  │   (SpanRef provenance)    │ │
                    │                  │  └───────────────────────────┘ │
                    │                  │                                 │
                    │                  │  ┌───────────────────────────┐ │
                    │                  │  │       signatures          │ │
                    │                  │  │  (provenance-first DSPy)  │ │
                    │                  │  └───────────────────────────┘ │
                    │                  └─────────────────┬───────────────┘
                    │                                    │
                    ▼                                    ▼
              ┌───────────┐                       ┌───────────┐
              │ lm-router │                       │  dspy-rs  │
              │ (reusable)│                       │ (external)│
              └─────┬─────┘                       └───────────┘
                    │
         ┌──────────┼──────────┐
         │          │          │
         ▼          ▼          ▼
    ┌─────────┐ ┌────────┐ ┌──────┐
    │fm-bridge│ │swarm-sim│ │ mock │
    └─────────┘ └────────┘ └──────┘
```

## RLM Module Structure

The RLM crate is organized into these major modules:

### Core Modules (Always Available)

| Module | Purpose |
|--------|---------|
| `engine` | RlmEngine execution loop |
| `client` | LlmClient trait and response types |
| `chunking` | Structure-aware document chunking |
| `span` | SpanRef for provenance tracking |
| `context` | Context management |
| `command` | Command parsing |
| `orchestrator` | High-level analysis orchestration |

### DSPy Modules (Feature: `dspy`)

| Module | Purpose |
|--------|---------|
| `dspy_bridge` | Re-exports dspy-rs + LmRouter bridge |
| `dspy_orchestrator` | Multi-phase document analysis |
| `signatures` | Provenance-first DSPy signatures |
| `tools/` | Environment tools (grep, read, list, symbols) |

### DSPy Architecture Detail

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         DspyOrchestrator                                │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │     Pipeline: Router → Extractor → Reducer → Verifier            │   │
│  │     Each phase uses typed DSPy signatures                        │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                                  │                                       │
│  ┌───────────────────────────────┴────────────────────────────────┐     │
│  │                      RLM Environment                            │     │
│  │  ┌──────────┐  ┌────────────┐  ┌───────────┐  ┌─────────────┐  │     │
│  │  │ GrepTool │  │ReadLinesTool│  │ListFilesTool│  │SymbolsTool │  │     │
│  │  └─────┬────┘  └──────┬─────┘  └──────┬────┘  └──────┬──────┘  │     │
│  │        └──────────────┴───────────────┴──────────────┘         │     │
│  │                       All tools return SpanRefs                 │     │
│  └─────────────────────────────────────────────────────────────────┘     │
├─────────────────────────────────────────────────────────────────────────┤
│                         dspy_bridge                                      │
│   ┌──────────────────────────┐    ┌───────────────────────────────┐     │
│   │  Global LM Configuration │    │   LmRouterDspyBridge          │     │
│   │  configure_dspy_lm()     │    │   Per-request LM routing      │     │
│   │  create_lm_for_*()       │    │   Unified cost tracking       │     │
│   └──────────────────────────┘    └───────────────────────────────┘     │
├─────────────────────────────────────────────────────────────────────────┤
│                            span                                          │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │  SpanRef: Git-aware, content-addressed provenance tracking      │   │
│   │  - Path, commit SHA, line/byte ranges                           │   │
│   │  - SHA256 content hash for verification                         │   │
│   │  - JSON serialization for DSPy signatures                       │   │
│   └─────────────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────────────┤
│                         signatures                                       │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │  Provenance-first signatures with SpanRef I/O:                  │   │
│   │  - RouterSignature (candidate_spans)                            │   │
│   │  - ExtractorSignature (evidence_spans)                          │   │
│   │  - ReducerSignature (citations)                                 │   │
│   │  - VerifierSignature (missing_spans)                            │   │
│   └─────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
```

## Crate Descriptions

### `lm-router` (Reusable)

Multi-backend LM routing with unified interface.

**Key Types:**
- `LmBackend` - Trait for LLM providers
- `LmRouter` - Routes requests to backends, tracks usage
- `LmResponse` - Standardized response format
- `LmUsage` - Token and cost tracking

**Backends:**
- `FmBridgeBackend` - Apple Foundation Models via fm-bridge
- `SwarmSimulator` - Simulated NIP-90 swarm for testing
- `MockBackend` - Deterministic responses for testing

**Location:** `crates/lm-router/`

### `bench-harness` (Reusable)

Generic benchmarking infrastructure.

**Key Types:**
- `TaskInstance` - Trait for benchmark tasks
- `Method` - Trait for solution methods
- `Trajectory` - Execution trace logging
- `Metric` - Trait for evaluation metrics
- `ExperimentRunner` - Orchestrates experiments

**Location:** `crates/bench-harness/`

### `bench-datasets` (Reusable)

Dataset loaders for various benchmarks.

**Key Types:**
- `Dataset` - Trait for loading tasks
- `DatasetConfig` - Configuration for loading
- Specific loaders: `SnihDataset`, `BrowseCompDataset`, `OolongDataset`, `CodeQADataset`

**Location:** `crates/bench-datasets/`

### `rlm-methods` (Paper-Specific)

Method implementations for the RLM paper.

**Key Types:**
- `BaseMethod` - Direct LLM baseline
- `SummaryAgentMethod` - Iterative summarization

**Location:** `crates/rlm-methods/`

## Key Traits

### `LmBackend`

```rust
#[async_trait]
pub trait LmBackend: Send + Sync {
    fn name(&self) -> &str;
    fn supported_models(&self) -> Vec<String>;
    fn supports_model(&self, model: &str) -> bool;
    async fn complete(&self, model: &str, prompt: &str, max_tokens: usize) -> Result<LmResponse>;
    async fn health_check(&self) -> bool;
}
```

### `RlmTool` (DSPy Feature)

```rust
#[async_trait]
pub trait RlmTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn args_schema(&self) -> Value;
    async fn execute(&self, args: Value) -> ToolResult<Value>;
}
```

### `TaskInstance`

```rust
pub trait TaskInstance: Send + Sync {
    fn id(&self) -> &str;
    fn query(&self) -> &str;
    fn context(&self) -> Option<&str>;
    fn ground_truth(&self) -> &GroundTruth;
    fn metadata(&self) -> &TaskMetadata;
}
```

### `Method`

```rust
#[async_trait]
pub trait Method: Send + Sync {
    fn name(&self) -> &str;
    async fn solve(&self, task: &dyn TaskInstance) -> Result<MethodResult>;
    async fn warmup(&mut self) -> Result<()> { Ok(()) }
    async fn reset(&mut self) -> Result<()> { Ok(()) }
}
```

### `Dataset`

```rust
#[async_trait]
pub trait Dataset: Send + Sync {
    type Task: TaskInstance;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn load(&self) -> Result<Vec<Self::Task>>;
    fn expected_count(&self) -> Option<usize>;
    fn primary_metric(&self) -> &str;
}
```

### `Metric`

```rust
pub trait Metric: Send + Sync {
    fn name(&self) -> &str;
    fn compute(&self, prediction: &str, truth: &GroundTruth) -> MetricValue;
}
```

## Data Flow

### Standard RLM Flow

```
┌──────────┐     ┌──────────┐     ┌────────┐     ┌─────────┐
│ Dataset  │────▶│  Tasks   │────▶│ Method │────▶│ Results │
└──────────┘     └──────────┘     └────────┘     └─────────┘
                                       │
                                       ▼
                                 ┌──────────┐
                                 │ LmRouter │
                                 └──────────┘
                                       │
                        ┌──────────────┼──────────────┐
                        ▼              ▼              ▼
                   ┌─────────┐   ┌─────────┐   ┌─────────┐
                   │Backend 1│   │Backend 2│   │Backend N│
                   └─────────┘   └─────────┘   └─────────┘
```

### DSPy Orchestrator Flow

```
┌──────────┐     ┌───────────┐     ┌───────────┐     ┌───────────┐
│ Document │────▶│  Chunking │────▶│  Router   │────▶│ Extractor │
└──────────┘     └───────────┘     └───────────┘     └─────┬─────┘
                                                           │
     ┌─────────────────────────────────────────────────────┘
     │
     ▼
┌───────────┐     ┌───────────┐     ┌────────────┐
│  Reducer  │────▶│  Verifier │────▶│   Result   │
└─────┬─────┘     └─────┬─────┘     └────────────┘
      │                 │
      │                 │           SpanRef Provenance
      │                 │           ┌─────────────────┐
      └─────────────────┴──────────▶│ Citations, etc. │
                                    └─────────────────┘
```

## File Structure

```
crates/
├── lm-router/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── backend.rs          # LmBackend trait
│       ├── router.rs           # LmRouter
│       ├── usage.rs            # Usage tracking
│       ├── error.rs
│       └── backends/
│           ├── mod.rs
│           ├── mock.rs
│           ├── fm_bridge.rs
│           └── swarm_sim.rs
│
├── rlm/
│   ├── Cargo.toml
│   ├── docs/                   # Documentation
│   │   ├── README.md
│   │   ├── ARCHITECTURE.md     # This file
│   │   ├── DSPY.md             # DSPy integration
│   │   ├── PROVENANCE.md       # SpanRef documentation
│   │   ├── TOOLS.md            # Environment tools
│   │   └── ...
│   ├── examples/
│   │   └── optimize_signatures.rs  # Optimizer scaffolding
│   └── src/
│       ├── lib.rs
│       ├── engine.rs           # RlmEngine
│       ├── client.rs           # LlmClient trait
│       ├── chunking.rs         # Document chunking
│       ├── span.rs             # SpanRef provenance (always)
│       ├── context.rs
│       ├── command.rs
│       ├── orchestrator.rs
│       │
│       │ # DSPy feature modules
│       ├── dspy_bridge.rs      # LM config + LmRouter bridge
│       ├── dspy_orchestrator.rs # 4-phase pipeline
│       ├── signatures.rs       # Provenance-first signatures
│       └── tools/              # Environment tools
│           ├── mod.rs
│           ├── grep.rs
│           ├── read_lines.rs
│           ├── list_files.rs
│           └── symbols.rs
│
├── bench-harness/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── task.rs             # TaskInstance, GroundTruth
│       ├── method.rs           # Method trait
│       ├── trajectory.rs       # Trajectory logging
│       ├── metrics.rs          # Metric implementations
│       ├── experiment.rs       # ExperimentRunner
│       └── error.rs
│
├── bench-datasets/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── dataset.rs          # Dataset trait
│       ├── error.rs
│       ├── sniah/
│       ├── browsecomp/
│       ├── oolong/
│       └── codeqa/
│
└── rlm-methods/
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── base.rs             # BaseMethod
        ├── summary_agent.rs    # SummaryAgentMethod
        ├── prompts/
        └── error.rs
```

## Extending the Framework

### Adding a New Backend

1. Implement `LmBackend` trait
2. Add to `lm-router/src/backends/`
3. Register with `LmRouter::builder().add_backend()`

### Adding a New Dataset

1. Define task structure matching `TaskInstance`
2. Implement `Dataset` trait
3. Add to `bench-datasets/src/`

### Adding a New Method

1. Implement `Method` trait
2. Add to `rlm-methods/src/` (or new crate for different papers)

### Adding a New Metric

1. Implement `Metric` trait
2. Add to `bench-harness/src/metrics.rs`

### Adding a New Tool (DSPy)

1. Implement `RlmTool` trait
2. Add to `rlm/src/tools/`
3. Export in `tools/mod.rs` and `lib.rs`

### Adding a New Signature (DSPy)

1. Define signature struct with `#[Signature]` macro
2. Add to `signatures.rs` if shared, or define inline
3. Consider provenance (SpanRef inputs/outputs)

## Documentation Index

- [README](./README.md) - Overview and quick start
- [DSPy Integration](./DSPY.md) - DSPy modules and usage
- [Provenance Tracking](./PROVENANCE.md) - SpanRef documentation
- [Environment Tools](./TOOLS.md) - Grep, read, list, symbols
- [LM Router](./LM_ROUTER.md) - Multi-backend routing
- [Bench Harness](./BENCH_HARNESS.md) - Experiment infrastructure
- [Datasets](./DATASETS.md) - Dataset formats and loaders
- [Methods](./METHODS.md) - Method implementations
- [Running Experiments](./RUNNING_EXPERIMENTS.md) - How to run benchmarks
