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
dsrs::configure(LM::new("codex-3-sonnet"));

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
| **14+ LM Providers** | OpenAI, OpenAI, Gemini, Groq, Ollama, Pylon, Codex SDK, etc. |
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
│  │ Codex SDK │ Pylon │ Ollama │ OpenAI │ OpenAI │ ... ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

## Documentation

- [Architecture](./ARCHITECTURE.md) - Core traits and design
- [Callbacks](./CALLBACKS.md) - Observability and HUD integration
- [Compiler Contract](./COMPILER-CONTRACT.md) - Wave 3 features (manifest, trace, Nostr bridge)
- [Evaluation](./EVALUATION.md) - Eval harness & promotion gates (Wave 5)
- [LM Providers](./LM-PROVIDERS.md) - Multi-provider LM configuration
- [Marketplace](./MARKETPLACE.md) - Trading learned patterns via Lightning + Nostr
- [Privacy](./PRIVACY.md) - Privacy module: redaction, chunking, policy (Wave 7)
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
| 5 | Complete | Eval Harness & Promotion Gates |
| 6 | Complete | SwarmCompiler |
| 7 | Complete | Privacy Module (redaction, chunking, policy) |
| 8 | Complete | OANIX DSPy Signatures (in `crates/oanix/`) |
| 9 | Complete | Agent Orchestrator Signatures (in `crates/agent-orchestrator/`) |
| 10 | Complete | Tool Invocation Signatures (in `crates/runtime/`) |
| 11 | Complete | Optimization Infrastructure (in `crates/autopilot-core/`) |
| 12 | Complete | FRLM Integration (in `crates/frlm/`) |
| 13 | Complete | Pipeline Wiring (decision pipelines in adjutant, runtime, orchestrator) |

## Key Paths

```
crates/dsrs/
├── src/
│   ├── core/
│   │   ├── signature.rs    # MetaSignature trait + DummySignature
│   │   ├── module.rs       # Module, Optimizable traits
│   │   ├── settings.rs     # Global LM + callback configuration
│   │   └── lm/
│   │       ├── codex_sdk.rs  # Codex Code headless
│   │       └── pylon.rs       # Pylon LM provider
│   ├── predictors/
│   │   ├── predict.rs      # Base Predict with callbacks
│   │   └── refine.rs       # Retry/fallback meta-operator
│   ├── pipelines/          # Ready-to-use pipeline wrappers
│   │   ├── mod.rs          # Pipeline exports
│   │   └── retrieval.rs    # QueryComposer, RetrievalRouter, CandidateRerank
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
│   ├── privacy/            # Wave 7: Privacy module
│   │   ├── mod.rs          # Module exports
│   │   ├── redaction.rs    # PathRedactor, IdentifierRedactor
│   │   ├── chunking.rs     # ChunkingPolicy, ContentChunk
│   │   └── policy.rs       # PrivacyPolicy, PolicyViolation
│   ├── compiler/           # Wave 6: SwarmCompiler
│   │   ├── swarm_compiler.rs # SwarmCompiler with budget tracking
│   │   └── provider.rs     # PylonLM provider
│   └── optimizer/          # MIPROv2, GEPA, COPRO, Pareto
└── docs/
    └── *.md
```

## Pipeline Pattern

DSPy pipelines wrap signatures with LM management and fallback logic:

```rust
pub struct MyPipeline {
    lm: Option<Arc<LM>>,
}

impl MyPipeline {
    pub fn new() -> Self { Self { lm: None } }
    pub fn with_lm(lm: Arc<LM>) -> Self { Self { lm: Some(lm) } }

    pub async fn execute(&self, input: &MyInput) -> Result<MyResult> {
        // 1. Check for configured LM, fallback to auto-detect
        let lm = match &self.lm {
            Some(lm) => lm.clone(),
            None => get_planning_lm().await?,
        };

        // 2. Create signature and predictor
        let sig = MySignature::new(input);
        let predictor = Predict::new(sig).with_lm(lm);

        // 3. Execute and parse result
        let prediction = predictor.forward(example! {
            "field" => input.value
        }).await?;

        Ok(MyResult {
            output: get_string(&prediction, "output"),
            confidence: get_f32(&prediction, "confidence"),
        })
    }
}
```

This pattern is used across:
- `crates/dsrs/src/pipelines/` - Retrieval pipelines
- `crates/adjutant/src/dspy/decision_pipelines.rs` - Decision routing
- `crates/agent-orchestrator/src/dspy_pipelines.rs` - Agent delegation
- `crates/runtime/src/dspy_pipelines.rs` - Tool selection

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
