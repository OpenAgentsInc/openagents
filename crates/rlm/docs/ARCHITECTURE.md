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
    ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐
    │bench-datasets│  │bench-harness│  │          rlm            │
    │  (reusable) │  │  (reusable) │  │      (core engine)      │
    └──────┬──────┘  └──────┬──────┘  │                         │
           │                │          │  ┌──────────────────┐  │
           └────────┬───────┘          │  │ dspy_orchestrator│  │
                    │                  │  │ (feature: dspy)  │  │
                    │                  │  └────────┬─────────┘  │
                    │                  │           │            │
                    │                  │  ┌────────▼─────────┐  │
                    │                  │  │   dspy_bridge    │  │
                    │                  │  └────────┬─────────┘  │
                    │                  └───────────┼────────────┘
                    │                              │
                    ▼                              ▼
              ┌───────────┐                 ┌───────────┐
              │ lm-router │                 │  dspy-rs  │
              │ (reusable)│                 │ (external)│
              └─────┬─────┘                 └───────────┘
                    │
         ┌──────────┼──────────┐
         │          │          │
         ▼          ▼          ▼
    ┌─────────┐ ┌────────┐ ┌──────┐
    │fm-bridge│ │swarm-sim│ │ mock │
    └─────────┘ └────────┘ └──────┘
```

### DSPy Integration (Optional Feature)

When the `dspy` feature is enabled, RLM gains:

- **`dspy_bridge`**: Re-exports dspy-rs types and LM configuration helpers
- **`dspy_orchestrator`**: Multi-phase document analysis using typed DSPy signatures

See [DSPY.md](./DSPY.md) for detailed documentation.

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
