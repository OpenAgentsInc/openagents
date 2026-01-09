# DSPy in Rust (dsrs)

## OpenAgents Integration

dsrs is integrated directly into the OpenAgents workspace at `crates/dsrs/` — a 5,771 LOC Rust implementation of DSPy concepts.

**This is not an external dependency.** We maintain dsrs as part of the monorepo.

```bash
# Build dsrs
cargo build -p dsrs

# Build with all features
cargo build -p dsrs --all-features
```

## Architecture

dsrs is the **compiler layer for agent behavior**. It decides *what to do* (best prompt + tool-use structure + few-shot examples), while the execution infrastructure (Pylon, Nexus, Runtime) decides *where/how it runs*.

```
crates/dsrs/src/
├── adapter/          # LM adapters (ChatAdapter)
├── core/
│   ├── lm/           # LM providers
│   │   ├── claude_sdk.rs    # Claude via claude-agent-sdk
│   │   ├── pylon.rs         # Pylon local/swarm/hybrid
│   │   ├── client_registry.rs  # 14+ provider registry
│   │   └── ...
│   ├── predict.rs    # Predictor implementation
│   ├── signature.rs  # Signature types
│   └── module.rs     # Module trait
├── data/             # Example, Dataset types
├── evaluate/         # Metrics, Evaluator trait
├── optimizer/        # COPRO, MIPROv2, GEPA, Pareto
├── predictors/       # ChainOfThought, Refine
├── trace/            # DAG tracing (Graph, Node)
└── utils/            # Retry, caching
```

## Key Features

| Feature | Description |
|---------|-------------|
| **Signatures** | `#[Signature]` macro for typed input/output schemas |
| **Modules** | `Module` trait for composable pipelines |
| **Optimizers** | COPRO, MIPROv2, GEPA, Pareto |
| **DAG Tracing** | Graph/Node types for execution tracing |
| **LM Providers** | 14+ via rig-core + Claude SDK + Pylon |
| **Hybrid Caching** | Memory + disk via foyer |
| **Evaluator Trait** | Metrics for optimization |

## LM Providers

dsrs supports multiple LM providers with smart priority/fallback:

| Priority | Provider | Detection |
|----------|----------|-----------|
| 1 | Claude SDK | `claude` CLI available |
| 2 | Pylon Swarm | `PYLON_MNEMONIC` env var |
| 3 | Cerebras | `CEREBRAS_API_KEY` env var |
| 4 | Pylon Local | Ollama on :11434 |

```rust
use dsrs::core::lm::{LMClient, claude_sdk};

// Auto-detect best available
let client = LMClient::from_model_string("claude-sdk:default")?;

// Or specify explicitly
let client = LMClient::claude_sdk()?;
let client = LMClient::pylon_local("llama3.2:3b")?;
let client = LMClient::pylon_swarm("llama3.2:3b", mnemonic)?;
```

## Usage in RLM

The `rlm` crate uses dsrs for its orchestration pipeline:

```rust
// crates/rlm/src/dspy_orchestrator.rs
use dsrs::{Signature, Module, Predict};

#[Signature]
struct RouterSignature {
    #[input] query: String,
    #[input] context_summary: String,
    #[output] strategy: String,
    #[output] confidence: f32,
}
```

See:
- [RLM DSPy Documentation](../../crates/rlm/docs/DSPY.md) - Usage guide
- [DSPy + RLM Concepts](./rlm.md) - Conceptual background
- [DSPy Roadmap](../DSPY_ROADMAP.md) - Full implementation roadmap

## Usage in Adjutant

The `adjutant` crate uses dsrs for task execution:

```rust
// crates/adjutant/src/dspy/module.rs
#[Signature]
struct SubtaskPlanningSignature {
    #[input] task_title: String,
    #[input] task_description: String,
    #[input] context: String,
    #[output] subtasks: String,
    #[output] reasoning: String,
    #[output] confidence: f32,
}
```

See [Adjutant DSPy Integration](../../crates/adjutant/docs/DSPY-INTEGRATION.md).

## Optimization

dsrs supports automatic prompt optimization:

```rust
use dsrs::optimizer::{MIPROv2, Optimizer};

let mut module = MyModule::new();
let optimizer = MIPROv2::builder()
    .num_candidates(10)
    .num_trials(20)
    .build();

optimizer.compile(&mut module, examples).await?;
```

## Swarm Job Types

Compiled modules invoke swarm job types as map-reduce primitives:

| Job Type | Mode | Purpose |
|----------|------|---------|
| `oa.code_chunk_analysis.v1` | Subjective | Parallel file/chunk analysis |
| `oa.retrieval_rerank.v1` | Subjective | LLM-based candidate reranking |
| `oa.sandbox_run.v1` | Objective | Build/test/lint in sandbox |

See [DSPY_ROADMAP.md](../DSPY_ROADMAP.md) for the full architecture.
