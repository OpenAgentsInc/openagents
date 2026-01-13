# dsrs - Rust DSPy

- **Status:** Accurate
- **Last verified:** d44f9cd3f
- **Source of truth:** `crates/dsrs/`
- **Doc owner:** dsrs
- **If this doc conflicts with code, code wins.**

Rust implementation of DSPy (Declarative Self-improving Python) for the OpenAgents ecosystem.

## Overview

dsrs is the **compiler layer for agent behavior**. It decides *what to do* (best prompt + tool-use structure + few-shot examples), while the OpenAgents execution infrastructure (Pylon, Nexus, Runtime) decides *where/how it runs*, *whether it's valid*, *what it costs*, and *why it worked*.

**Key insight:** DSPy is declarative AI programming, not just prompt optimization. Signatures decouple AI specification from ML techniques, enabling model portability without rewriting prompts.

In practice, dsrs gives Autopilot and Adjutant a way to turn intent into code-like
structures that can be optimized, traced, and audited. Instead of wiring the
agent through hand-tuned prompts, dsrs provides typed signatures and reusable
pipelines so the same decision surfaces can be improved over time without
rewriting the orchestration logic.

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
| **Multi-Provider LM** | OpenAI, Gemini, Groq, OpenRouter, Ollama, Pylon, GPT-OSS, LM-Router, etc. |
| **Callbacks** | Observability hooks for HUD integration |
| **Caching** | Hybrid memory + disk caching via foyer |

The features above are intentionally biased toward production-grade behavior. We
use signatures so outputs are typed and debuggable, predictors so we can compose
decision logic, and optimizers so prompts improve without manual retuning.
Tracing and callbacks keep every call visible to the HUD and to offline
evaluation systems, while caching and LM routing keep costs predictable when we
scale to many runs.

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
│  │ OpenAI │ Gemini │ Groq │ OpenRouter │ Ollama │ Pylon │ GPT-OSS │ ... ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

This layout separates the developer-facing API from the runtime mechanics. The
interface layer defines how signatures and callbacks are registered, the core
layer handles prediction and optimization, and the adapter and provider layers
translate those abstractions into concrete model calls. This separation lets us
swap providers or change routing without touching the signature definitions.

## Documentation

- [Architecture](./ARCHITECTURE.md) - A narrative explanation of the core traits, runtime layers, and how signatures and predictors fit together in the compiled pipeline.
- [Callbacks](./CALLBACKS.md) - Details on the observability hooks that wire dsrs execution into the HUD and telemetry surfaces used by Autopilot and Adjutant.
- [Compiler Contract](./COMPILER-CONTRACT.md) - The Wave 3 contract that defines manifests, trace structure, and the Nostr bridge so compiled modules stay portable across runtimes.
- [DSPy Roadmap](./DSPY_ROADMAP.md) - The current OpenAgents DSPy implementation status, including wave sequencing and the Autopilot integration map.
- [Evaluation](./EVALUATION.md) - How eval tasks, metrics, and promotion gates measure signature quality and determine when an optimized module is promoted.
- [LM Providers](./LM-PROVIDERS.md) - A practical guide to configuring multi-provider inference and routing behavior in dsrs.
- [Marketplace](./MARKETPLACE.md) - How learned patterns can be traded and attributed over Lightning and Nostr within the DSPy ecosystem.
- [Privacy](./PRIVACY.md) - The redaction and policy layer that protects sensitive data while preserving training value for optimization.
- [Retrieval](./RETRIEVAL.md) - The multi-lane retrieval system that composes search strategies before they reach higher-level signatures.
- [Signatures](./SIGNATURES.md) - Guidance for designing optimizable signatures and aligning input/output fields with learning objectives.

## Wave Status

- **Note:** Wave status tracks component readiness (structs + unit tests); MVP readiness depends on wiring. See root ROADMAP.md "NOW" section for true MVP gates.

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
| 9 | Archived | Agent Orchestrator Signatures (moved to backroom) |
| 10 | Complete | Tool Invocation Signatures (in `crates/runtime/`) |
| 11 | Complete | Optimization Infrastructure (in `crates/autopilot-core/`) |
| 12 | Complete | FRLM Integration (in `crates/frlm/`) |
| 13 | Complete | Pipeline Wiring (decision pipelines in adjutant, runtime, oanix, autopilot-core) |
| 14 | Complete | Self-Improving Autopilot loop (sessions + auto-optimization in `crates/adjutant/`) |

Waves are sequenced so that foundational capabilities (signatures, routing, and
evaluation) are in place before the higher-level features (self-improvement and
full pipeline wiring). The current status reflects the live workspace; archived
work is called out explicitly so it does not confuse implementation planning.

## Key Paths

```

The paths above map to the core parts of the DSPy runtime: signature definition,
prediction logic, evaluation and optimization, and runtime adapters. When you
need to trace a decision, start with the signature and predictor code paths,
then follow LM routing through the core `lm` module.
crates/dsrs/
├── src/
│   ├── core/
│   │   ├── signature.rs    # MetaSignature trait + DummySignature
│   │   ├── module.rs       # Module, Optimizable traits
│   │   ├── settings.rs     # Global LM + callback configuration
│   │   └── lm/
│   │       ├── client_registry.rs # LM provider registry
│   │       ├── lm_router.rs       # Provider selection/router
│   │       ├── pylon.rs           # Pylon LM provider
│   │       └── gptoss.rs          # GPT-OSS LM provider (structured output)
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

This pattern is used across the workspace as the standard way to wrap signatures
with LM selection and fallback behavior. The retrieval pipelines live in
`crates/dsrs/src/pipelines/`, OANIX uses it for situation and issue selection in
`crates/oanix/src/dspy_pipelines.rs`, Adjutant uses it for decision routing in
`crates/adjutant/src/dspy/decision_pipelines.rs` and for Autopilot planning
stages in `crates/adjutant/src/dspy_orchestrator.rs`, Autopilot core uses it in
`crates/autopilot-core/src/dspy_*` for planning, execution, and verification,
and Runtime uses it in `crates/runtime/src/dspy_pipelines.rs` for tool selection
and result interpretation.

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
