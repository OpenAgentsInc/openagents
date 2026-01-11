# OpenAgents DSPy Strategy

This document synthesizes our complete DSPy strategy—philosophy, architecture, and implementation.

## Core Insight

**DSPy is the compiler layer for agent behavior.**

Not the market layer, not the runtime. DSPy provides: take a goal ("solve this class of tasks") and automatically discover the best prompt + tool-use structure + few-shot examples + scoring loop—instead of hand-tuning prompts forever.

**The Separation:**
- **DSPy (dsrs)** decides *what to do* (best program structure/prompting)
- **Execution infrastructure** (Pylon, Nexus, Runtime) decides *where/how it runs*, *whether it's valid*, *what it costs*, and *why it worked*

## Philosophy

From [Omar Khattab](../transcripts/dspy/state-of-dspy.md) and [Kevin Madura](../transcripts/dspy/dspy-is-all-you-need.md):

1. **DSPy is NOT an optimizer.** It's a set of programming abstractions—a way to program where you declare your intent. The optimization is a nice bonus.

2. **Signatures decouple specification from implementation.** You declare what you want (inputs, outputs, types), not how to prompt for it. The prompt is generated.

3. **Field names are mini-prompts.** Naming matters. `task_description: String` becomes part of the prompt.

4. **Optimizers find latent requirements.** MIPROv2/GEPA discover things you didn't think to specify—like "always capitalize names" or "include file paths in responses."

5. **Model portability without rewriting.** The same signature works across Codex, GPT-4, Llama, Ollama. Only the adapter changes.

6. **Tight iteration loops.** Build a signature in 3 lines, test it, refine it, optimize it. No prompt engineering rabbit holes.

## The Differentiator

> Semantic search is table stakes. The differentiator is an end-to-end *programmable* and *optimizable* retrieval+reasoning system — a **compiled agent** where retrieval is one module in an eval-driven, optimizer-tuned, environment-interacting program.

## Architecture

```
┌────────────────────────────────────────────────────────────────────┐
│                        OPENAGENTS + DSRS                            │
├────────────────────────────────────────────────────────────────────┤
│  PROTOCOL LAYER                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │ Job Schemas  │  │  Canonical   │  │  Versioning  │              │
│  │  (JSON)      │  │   Hashing    │  │    Rules     │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│                                                                      │
│  COMPILER LAYER (dsrs)                                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │  Signatures  │  │  Optimizers  │  │   Modules    │              │
│  │  (macros)    │  │  MIPROv2     │  │  (pipelines) │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│                                                                      │
│  INTEGRATION LAYER                                                   │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │  SessionStore  │  OutcomeFeedback  │  PerformanceTracker      │ │
│  │  AutoOptimizer │  LabeledExamples  │  DecisionPipelines       │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                                                                      │
│  EXECUTION LAYER (OpenAgents)                                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │    Pylon     │  │   Adjutant   │  │   Autopilot  │              │
│  │  (compute)   │  │  (decisions) │  │   (loop)     │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
└────────────────────────────────────────────────────────────────────┘
```

## Key Concepts

### Signatures

Typed input/output contracts for LLM tasks:

```rust
#[Signature]
struct TaskPlannerSignature {
    /// Plan a coding task into subtasks.

    #[input] task_description: String,
    #[input] file_count: u32,

    #[output] complexity: String,     // Low/Medium/High/VeryHigh
    #[output] subtasks: String,       // JSON array
    #[output] confidence: f32,
}
```

The docstring, field names, and types all become part of the prompt.

### Modules

Composable units with `forward()` method:

```rust
pub struct AdjutantModule {
    planner: Predict<TaskPlannerSignature>,
    executor: Predict<TaskExecutorSignature>,
    synthesizer: Predict<ResultSynthesisSignature>,
}

impl Module for AdjutantModule {
    async fn forward(&self, inputs: Example) -> Result<Prediction> {
        let plan = self.planner.forward(inputs).await?;
        // ... execute subtasks ...
        self.synthesizer.forward(results).await
    }
}
```

### Optimizers

Automatic prompt improvement:

| Optimizer | Use Case | Cost |
|-----------|----------|------|
| **MIPROv2** | Instruction optimization | Low |
| **COPRO** | Instruction + demo optimization | Medium |
| **GEPA** | Feedback-driven optimization | Higher |
| **Pareto** | Multi-objective optimization | Higher |

Start with MIPROv2, graduate to GEPA when plateaued.

### Adapters

Prompt formatters between signatures and LLMs:

- **JSON Adapter** — Default, works everywhere
- **BAML Adapter** — More token-efficient, 5-10% improvement
- **XML Adapter** — Some models prefer XML

## The Self-Improvement Loop

The key innovation in OpenAgents is closing the feedback loop:

```
┌─────────────────────────────────────────────────────────────────┐
│  1. TASK EXECUTION (Autopilot Loop)                              │
│     └── Decisions recorded (complexity, delegation, RLM)         │
│                                                                  │
│  2. SESSION COMPLETION                                           │
│     └── Outcome recorded (Success/Failed/MaxIterations)          │
│                                                                  │
│  3. OUTCOME FEEDBACK                                             │
│     └── Decisions labeled as correct/incorrect                   │
│     └── LabeledExamples stored with ground truth                 │
│                                                                  │
│  4. PERFORMANCE TRACKING                                         │
│     └── Rolling accuracy updated (window: 50 decisions)          │
│     └── Per-signature metrics tracked                            │
│                                                                  │
│  5. AUTO-OPTIMIZATION CHECK                                      │
│     ├── Enough new examples? (threshold: 20)                     │
│     ├── Accuracy dropped? (threshold: <70%)                      │
│     └── Time since last optimization? (threshold: >24h)          │
│                                                                  │
│  6. MIPROV2 OPTIMIZATION (if triggered)                          │
│     └── Optimizes lowest-accuracy signature                      │
│     └── Records optimization run for audit                       │
└─────────────────────────────────────────────────────────────────┘
```

**The flywheel:** Successful sessions generate training data → dsrs optimization (cheap on Pylon swarm) → better prompts/demos → higher success rates → more training data.

## Decision Pipelines

Three DSPy pipelines drive autonomous execution:

### ComplexityPipeline
Classifies task complexity: Low / Medium / High / VeryHigh

### DelegationPipeline
Decides execution path: `codex_code` / `rlm` / `local_tools`

### RlmTriggerPipeline
Decides when recursive analysis is needed

Each pipeline has:
- **DSPy-first logic** with typed signature
- **Legacy fallback** when confidence < 0.7
- **Training collection** for high-confidence decisions

## Implementation Status

### Foundation (Complete)

| Wave | Description | Status |
|------|-------------|--------|
| Wave 0 | Protocol + Schema Registry | Complete |
| Wave 1 | RLM Document Analysis | Complete |
| Wave 2 | Autopilot Signatures | Complete |
| Wave 2.5 | LaneMux (Multi-Provider LM) | Complete |
| Wave 3-6 | Core Infrastructure | Complete |
| Wave 7 | Privacy Module | Complete |
| Wave 8 | OANIX DSPy Signatures | Complete |
| Wave 9 | Agent Orchestrator | Complete |
| Wave 10 | Tool Invocation | Complete |
| Wave 11 | Optimization Infrastructure | Complete |
| Wave 12 | FRLM Integration | Complete |
| Wave 13 | Pipeline Wiring | Complete |
| Wave 14 | Self-Improving Autopilot | Complete |

### Full Integration (In Progress)

| Wave | Description | Status |
|------|-------------|--------|
| Wave 15 | Tiered Executor DSPy Migration | **Complete** |
| Wave 16 | RLM DSPy Integration | In Progress |
| Wave 17 | LM-Router DSPy Backend | Complete |
| Wave 18 | Gateway DSPy Integration | Complete |
| Wave 19 | Autopilot Heuristics → DSPy | Complete |
| Wave 20 | Agent-Orchestrator & Nexus DSPy | Complete |
| Wave 21 | Marketplace Security DSPy | Complete |

See [DSPY_ROADMAP.md](../DSPY_ROADMAP.md) for full details and [signatures-catalog.md](./signatures-catalog.md) for the complete signature inventory.

## Wave 15: Tiered Executor Migration (Complete)

The tiered executor (`crates/adjutant/src/tiered.rs`) now defaults to DSPy mode:
- **Dsrs** (default): Uses DSPy signatures from `crates/adjutant/src/dspy/module.rs`
- **Gateway** (fallback): Legacy hardcoded prompts with Cerebras GLM 4.7/Qwen-3-32B

**What changed:**
- Default `ExecutionMode` switched from `Gateway` to `Dsrs`
- `execute()` routes to DSPy path with automatic gateway fallback on failure
- Training data collection active for all planning/execution/synthesis decisions

**Signatures in use:**
- `SubtaskPlanningSignature` — Break tasks into atomic subtasks
- `SubtaskExecutionSignature` — Execute individual subtasks
- `ResultSynthesisSignature` — Synthesize results into final outcome

## Wave 16: RLM DSPy Integration (In Progress)

The RLM engine (`crates/rlm/`) has 4 hardcoded prompt tiers to replace:
- `BASIC_SYSTEM_PROMPT` — Simple code execution
- `CONTEXT_SYSTEM_PROMPT` — Full RLM with llm_query() (from paper)
- `GUIDED_SYSTEM_PROMPT` — Apple FM tier
- `MINIMAL_SYSTEM_PROMPT` — Small models

**Signatures created** (`crates/rlm/src/dspy.rs`):
```rust
#[Signature] struct RlmQuerySignature { ... }           // Basic RLM
#[Signature] struct RlmContextQuerySignature { ... }    // Full RLM with llm_query()
#[Signature] struct RlmGuidedQuerySignature { ... }     // Apple FM tier
#[Signature] struct RlmCodeGenerationSignature { ... }  // Code generation
#[Signature] struct RlmContinuationSignature { ... }    // Continuation handling
```

**Remaining work:**
- Wire signatures to `codex_client.rs` via dsrs Predict
- Route PromptTier enum to appropriate signature
- Add training data collection

## Wave 19: Autopilot Heuristics → DSPy (Complete)

Autopilot now uses DSPy classifiers instead of keyword heuristics for:
- Planning depth selection (task complexity classifier)
- Build/test status detection
- Plan quality validation (path validity + actionable steps)

These decisions are now recorded through DSPy tracing for training collection.

## Wave 20: Agent-Orchestrator & Nexus DSPy (Complete)

Agent orchestration and Nexus now use DSPy for:
- Directive status/priority parsing and semantic matching
- Issue selection from open queues
- Event intent classification and NIP-90 job kind detection

## Wave 21: Marketplace Security DSPy (Complete)

Marketplace skill execution now uses DSPy for:
- Security risk classification (risk level + sandbox recommendation)
- Filesystem permission validation, path safety checks, and resource limit review
- Human approval gating for High/Critical risk decisions with audit logging

## Storage Layout

```
~/.openagents/adjutant/
├── training/
│   ├── dataset.json           # Training examples
│   └── labeled/               # Labeled examples with ground truth
│       ├── complexity.json
│       ├── delegation.json
│       └── rlm_trigger.json
├── sessions/                  # Session tracking
│   ├── index.json
│   └── <year>/<month>/<session-id>.json
├── metrics/                   # Performance metrics
│   └── performance.json
└── config/                    # Configuration
    └── auto_optimizer.json
```

## CLI Commands

```bash
# View session history
autopilot dspy sessions
autopilot dspy sessions --failed

# View performance metrics
autopilot dspy performance

# Configure auto-optimization
autopilot dspy auto-optimize --enable
autopilot dspy auto-optimize --min-examples 30
autopilot dspy auto-optimize --accuracy-threshold 0.75
```

## Model Mixing Strategy

Different signatures benefit from different models:

| Signature Type | Recommended Model | Reasoning |
|----------------|-------------------|-----------|
| Planning (Deep) | Codex Opus | Complex reasoning |
| Planning (Simple) | Codex Sonnet | Balance speed/quality |
| Execution | Codex Sonnet | Balance speed/quality |
| Review/Verify | Codex Haiku | Fast validation |
| Optimization (iterations) | Pylon Swarm | Cheap, high volume |
| Optimization (validation) | Codex/GPT-4 | Final quality check |
| Retrieval Router | Pylon Local | Fast, cheap policy |
| Evidence Ranker | Pylon Swarm | Parallelizable |

## LM Provider Priority

1. **Codex SDK** — Uses Codex headless mode
2. **Llama.cpp/GPT-OSS** — Local OpenAI-compatible server (default :8080)
3. **Pylon Swarm** — Distributed inference via NIP-90
4. **Cerebras** — Fast, cheap execution
5. **Pylon Local** — Ollama fallback (set `PYLON_LOCAL_MODEL`/`OLLAMA_MODEL` to override)

## Key Files

| File | Purpose |
|------|---------|
| `crates/dsrs/` | Rust DSPy implementation (5,771 LOC) |
| `crates/adjutant/src/dspy/` | Decision pipelines, self-improvement |
| `crates/adjutant/src/dspy/sessions.rs` | Session tracking |
| `crates/adjutant/src/dspy/outcome_feedback.rs` | Outcome labeling |
| `crates/adjutant/src/dspy/performance.rs` | Rolling accuracy |
| `crates/adjutant/src/dspy/auto_optimizer.rs` | Auto-optimization |

## Related Documentation

### Strategy & Implementation
- [DSPY_ROADMAP.md](../DSPY_ROADMAP.md) — Full implementation roadmap (Waves 0-21)
- [signatures-catalog.md](./signatures-catalog.md) — Complete catalog of all DSPy signatures
- [integration-guide.md](./integration-guide.md) — How to add DSPy to new components

### Technical Reference
- [rust.md](./rust.md) — dsrs usage guide
- [rlm.md](./rlm.md) — DSPy + RLM integration
- [crates/adjutant/docs/DSPY-INTEGRATION.md](../../crates/adjutant/docs/DSPY-INTEGRATION.md) — Self-improvement details
- [crates/dsrs/docs/](../../crates/dsrs/docs/) — dsrs implementation docs

### Philosophy
- [Transcripts: State of DSPy](../transcripts/dspy/state-of-dspy.md) — Omar Khattab
- [Transcripts: DSPy is All You Need](../transcripts/dspy/dspy-is-all-you-need.md) — Kevin Madura
