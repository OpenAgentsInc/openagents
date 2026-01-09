# dsrs - Rust DSPy

Rust implementation of DSPy (Declarative Self-improving Python) for the OpenAgents ecosystem.

## Overview

dsrs is the **compiler layer for agent behavior**. It decides *what to do* (best prompt + tool-use structure + few-shot examples), while the OpenAgents execution infrastructure (Pylon, Nexus, Runtime) decides *where/how it runs*, *whether it's valid*, *what it costs*, and *why it worked*.

**Key insight:** DSPy is declarative AI programming, not just prompt optimization. Signatures decouple AI specification from ML techniques, enabling model portability without rewriting prompts.

## Quick Start

```rust
use dsrs::prelude::*;

// Define a signature
#[Signature]
struct QuestionAnswer {
    /// The question to answer
    #[input] question: String,
    /// The answer
    #[output] answer: String,
}

// Create a predictor
let predictor = Predict::new(QuestionAnswer::default());

// Configure LM
dsrs::configure(LM::new("claude-3-sonnet"));

// Run inference
let result = predictor.forward(example! {
    "question" => "What is 2+2?"
}).await?;

println!("Answer: {}", result.get("answer", None));
```

## Features

| Feature | Description |
|---------|-------------|
| **Signatures** | Declarative input/output specifications via `#[Signature]` macro |
| **Predictors** | `Predict`, `ChainOfThought`, `Refine` for different reasoning patterns |
| **Optimizers** | COPRO, MIPROv2, GEPA, Pareto for automatic prompt optimization |
| **DAG Tracing** | Graph/Node types for execution visualization |
| **14+ LM Providers** | OpenAI, Anthropic, Gemini, Groq, Ollama, Pylon, Claude SDK, etc. |
| **Callbacks** | Observability hooks for HUD integration |
| **Caching** | Hybrid memory + disk caching via foyer |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         DSRS                                 │
├─────────────────────────────────────────────────────────────┤
│  INTERFACE LAYER                                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │ Signatures  │  │  Callbacks  │  │  Manifest   │          │
│  │  (macros)   │  │   (HUD)     │  │ (compiled)  │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
│                                                              │
│  CORE LAYER                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │ Predictors  │  │ Optimizers  │  │    Trace    │          │
│  │ Predict     │  │ MIPROv2     │  │    DAG      │          │
│  │ Refine      │  │ GEPA        │  │  Contract   │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
│                                                              │
│  ADAPTER LAYER                                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │    Chat     │  │   Pylon     │  │   Nostr     │          │
│  │  Adapter    │  │  Sandbox    │  │   Bridge    │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
│                                                              │
│  LM PROVIDERS (via rig-core)                                 │
│  ┌─────────────────────────────────────────────────────────┐│
│  │ Claude SDK │ Pylon │ Ollama │ OpenAI │ Anthropic │ ... ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

## Documentation

- [Architecture](./ARCHITECTURE.md) - Core traits and design
- [Callbacks](./CALLBACKS.md) - Observability and HUD integration
- [Compiler Contract](./COMPILER-CONTRACT.md) - Wave 3 features (manifest, trace, Nostr bridge)
- [Evaluation](./EVALUATION.md) - Eval harness & promotion gates (Wave 5)
- [LM Providers](./LM-PROVIDERS.md) - Multi-provider LM configuration
- [Retrieval](./RETRIEVAL.md) - Multi-lane retrieval system (Wave 4)
- [Signatures](./SIGNATURES.md) - Optimizable agent signatures (Wave 4)

## Wave Status

| Wave | Status | Description |
|------|--------|-------------|
| 0 | Complete | Protocol + Schema Registry |
| 1-2 | Complete | RLM + Autopilot signatures |
| 2.5 | Complete | LaneMux (multi-provider LM) |
| 3 | Complete | Compiler Contract (manifest, callbacks, trace, sandbox) |
| 4 | Complete | Retrieval, Signatures, Swarm Dispatch |
| 5 | **Complete** | Eval Harness & Promotion Gates |
| 6+ | Planned | SwarmCompiler, Privacy, OANIX |

## Key Paths

```
crates/dsrs/
├── src/
│   ├── core/
│   │   ├── signature.rs    # MetaSignature trait + DummySignature
│   │   ├── module.rs       # Module, Optimizable traits
│   │   ├── settings.rs     # Global LM + callback configuration
│   │   └── lm/
│   │       ├── claude_sdk.rs  # Claude Code headless
│   │       └── pylon.rs       # Pylon LM provider
│   ├── predictors/
│   │   ├── predict.rs      # Base Predict with callbacks
│   │   └── refine.rs       # Retry/fallback meta-operator
│   ├── retrieval/          # Wave 4: Multi-lane retrieval
│   │   ├── mod.rs          # RepoIndex trait, RetrievalResult
│   │   ├── ripgrep.rs      # Text search backend
│   │   ├── lsp.rs          # LSP/ctags backend
│   │   ├── semantic.rs     # Vector embeddings backend
│   │   ├── git.rs          # Git signals backend
│   │   └── router.rs       # Multi-lane router
│   ├── signatures/         # Wave 4: Optimizable signatures
│   │   ├── query_composer.rs
│   │   ├── retrieval_router.rs
│   │   ├── candidate_rerank.rs
│   │   ├── chunk_task.rs
│   │   ├── chunk_aggregator.rs
│   │   ├── sandbox_profile.rs
│   │   ├── failure_triage.rs
│   │   ├── lane_budgeter.rs
│   │   └── agent_memory.rs
│   ├── adapter/
│   │   ├── chat.rs         # Chat completion adapter
│   │   ├── pylon_sandbox.rs # Sandbox execution provider
│   │   └── swarm_dispatch.rs # NIP-90 job dispatch (Wave 4)
│   ├── trace/
│   │   ├── dag.rs          # Graph/Node execution DAG
│   │   ├── contract.rs     # OTel-compatible spans
│   │   └── nostr_bridge.rs # DAG → Nostr events
│   ├── callbacks.rs        # DspyCallback trait + implementations
│   ├── manifest.rs         # CompiledModuleManifest, Scorecard
│   ├── evaluate/           # Wave 5: Eval harness
│   │   ├── task.rs         # EvalTask, RepoContext, Constraint
│   │   ├── metrics/        # Metric trait, proxy + truth metrics
│   │   ├── scoring.rs      # Scorer, AggregationMethod
│   │   ├── promotion.rs    # PromotionState, PromotionGate
│   │   └── priority.rs     # CompilePriority, CompileQueue
│   └── optimizer/          # MIPROv2, GEPA, COPRO, Pareto
└── docs/
    └── *.md
```

## Usage with OpenAgents

dsrs integrates with the OpenAgents stack:

1. **Pylon** - Provides inference (local/swarm) and sandbox execution
2. **Nexus** - Nostr relay for job coordination
3. **Runtime** - HUD callbacks for real-time observability
4. **Protocol** - Canonical hashing for compiled module IDs

```rust
use dsrs::prelude::*;

// Use Pylon for inference
let lm = LM::new_pylon("wss://nexus.openagents.com", budget_msats);
dsrs::configure(lm);

// Use sandbox for verification
let sandbox = PylonSandboxProvider::generate()
    .with_profile(SandboxProfile::Medium);

let result = sandbox.run_commands(vec!["cargo test"]).await?;
```

## Testing

```bash
cargo test -p dsrs
```

## License

MIT - See repository root for details.
