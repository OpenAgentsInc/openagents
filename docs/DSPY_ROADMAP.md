# DSPy Integration Roadmap for OpenAgents

## Core Insight: DSPy as Compiler Layer

**DSPy is the compiler layer for agent behavior.**

Not the market layer, not the runtime. DSPy provides: **take a goal ("solve this class of tasks") and automatically discover the best prompt + tool-use structure + few-shot examples + scoring loop**, instead of hand-tuning prompts forever.

**The Separation:**
- **DSPy (dsrs)** decides *what to do* (best program structure/prompting)
- **The execution infrastructure** (Pylon, Nexus, Runtime) decides *where/how it runs* (local/swarm/datacenter), *whether it's valid* (verification), *what it costs* (budgets/receipts), and *why it worked* (trace/HUD)

**Philosophy (from Omar Khattab & Kevin Madura):**
- DSPy is declarative AI programming, not just prompt optimization
- Signatures decouple AI specification from ML techniques
- Optimizers (GEPA/MIPROv2) find "latent requirements" you didn't specify
- Field names act as mini-prompts — naming matters
- Enable model portability without rewriting prompts

**The Differentiator:**
> Semantic search is table stakes. The differentiator is an end-to-end *programmable* and *optimizable* retrieval+reasoning system — a **compiled agent** where retrieval is one module in an eval-driven, optimizer-tuned, environment-interacting program.

---

## Current State

### dsrs Integration (Complete)

dsrs (Rust DSPy) is now integrated into the OpenAgents workspace at `crates/dsrs/`:

- **5,771 LOC** full implementation
- **Optimizers**: COPRO, MIPROv2, GEPA, Pareto
- **DAG-based tracing** with Graph/Node types
- **14+ LM providers** via rig-core (OpenAI, OpenAI, Gemini, Groq, Ollama, Pylon, Codex SDK, etc.)
- **Trait-based architecture**: Module, Predictor, MetaSignature, Adapter, Optimizable, Evaluator
- **Macros**: `#[Signature]`, `#[Optimizable]` for code generation
- **Hybrid caching** via foyer (memory + disk)
- **Multi-provider LM support**: Codex SDK (headless) → Pylon swarm → Cerebras → Pylon local

### Wave 1: RLM Document Analysis (Complete)
- [x] `crates/rlm/src/dspy_orchestrator.rs` - 4-phase document pipeline
- [x] `crates/rlm/src/signatures.rs` - SpanRef-based evidence tracking
- [x] `crates/rlm/src/dspy_bridge.rs` - LM configuration and cost tracking

### Wave 2: Autopilot Signatures (Complete)
- [x] `crates/autopilot-core/src/dspy_planning.rs` - PlanningSignature + DeepPlanningSignature
- [x] `crates/autopilot-core/src/dspy_execution.rs` - ExecutionStrategySignature + ToolSelectionSignature
- [x] `crates/autopilot-core/src/dspy_verify.rs` - Verification + ExecutionReviewSignature
- [x] `crates/autopilot-core/src/dspy_optimization.rs` - Metrics + training data infrastructure

### Wave 2.5: LaneMux (Multi-Provider LM) (Complete)
- [x] `crates/dsrs/src/core/lm/codex_sdk.rs` - Codex headless via codex-agent-sdk
- [x] `crates/dsrs/src/core/lm/pylon.rs` - Pylon LM provider (local/swarm/hybrid)
- [x] `crates/adjutant/src/dspy/lm_config.rs` - Multi-provider with auto-detection

### Wave 3-6: Core Infrastructure (Complete)
- [x] Compiler Contract (manifest, callbacks, trace, sandbox)
- [x] Retrieval, Signatures, Swarm Dispatch
- [x] Eval Harness & Promotion Gates
- [x] SwarmCompiler

### Wave 7: Privacy Module (Complete)
- [x] `crates/dsrs/src/privacy/redaction.rs` - PathRedactor, IdentifierRedactor
- [x] `crates/dsrs/src/privacy/chunking.rs` - ChunkingPolicy, ContentChunk
- [x] `crates/dsrs/src/privacy/policy.rs` - PrivacyPolicy, PolicyViolation
- [x] `crates/dsrs/docs/PRIVACY.md` - Comprehensive documentation

### Wave 8: OANIX DSPy Signatures (Complete)
- [x] `crates/oanix/src/dspy_situation.rs` - SituationAssessmentSignature
- [x] `crates/oanix/src/dspy_lifecycle.rs` - IssueSelectionSignature, WorkPrioritizationSignature, LifecycleDecisionSignature (CoT)
- [x] All signatures implement MetaSignature trait
- [x] 18 tests passing

### Wave 9: Agent Orchestrator DSPy Signatures (Complete)
- [x] `crates/agent-orchestrator/src/dspy_delegation.rs` - DelegationSignature, TargetAgent
- [x] `crates/agent-orchestrator/src/dspy_agents.rs` - 6 agent signatures:
  - ArchitectureSignature (Oracle) - CoT for design decisions
  - LibraryLookupSignature (Librarian) - External docs lookup
  - CodeExplorationSignature (Explore) - Codebase navigation
  - UIDesignSignature (Frontend) - UI/UX design
  - DocumentationSignature (DocWriter) - Technical docs
  - MediaAnalysisSignature (Multimodal) - PDF/image analysis
- [x] Helper enums: ArchitectureComplexity, SearchType, DocType, MediaType
- [x] 152 tests passing in agent-orchestrator

### Wave 10: Tool Invocation DSPy Signatures (Complete)
- [x] `crates/runtime/src/dspy_tools.rs` - 3 tool signatures:
  - ToolSelectionSignature - Choose the right tool for any task
  - ToolResultInterpretationSignature - Understand tool output
  - ToolChainPlanningSignature - Plan multi-tool sequences
- [x] Helper enum: ToolSuccess (YES, PARTIAL, NO)
- [x] 10 tests passing in openagents-runtime

### Wave 11: Optimization Infrastructure (Complete)
- [x] `crates/autopilot-core/src/dspy_hub.rs` - DspyHub for module storage
  - Save/load optimized modules with demos
  - Query promoted modules by signature
  - A/B routing strategies (Promoted, Shadow, ABTest)
- [x] `crates/autopilot-core/src/dspy_training.rs` - TrainingExtractor
  - Parse JSONL session logs
  - Extract planning, execution, verification examples
  - Success criteria filtering
- [x] `crates/autopilot-core/src/dspy_router.rs` - SignatureRouter
  - Route by compiled_id or strategy
  - Shadow mode comparison tracking
  - Promotion decision support
- [x] 42 dspy tests passing in autopilot

### Wave 12: FRLM Integration (Complete)
- [x] FRLM DSPy signatures in `crates/frlm/`

### Wave 13: Pipeline Wiring (Complete)
- [x] **Agent-Orchestrator Pipeline** (`crates/agent-orchestrator/src/dspy_pipelines.rs`)
  - DelegationPipeline - Routes tasks to specialized subagents
  - AgentRegistry.delegate_task() with DSPy + legacy fallback
  - 155 tests passing
- [x] **Runtime Tool Pipelines** (`crates/runtime/src/dspy_pipelines.rs`)
  - ToolSelectionPipeline - Choose best tool for task
  - ToolInterpretationPipeline - Understand tool output
  - ToolChainPipeline - Plan multi-tool sequences
  - 19 dspy tests passing
- [x] **Retrieval Policy Pipelines** (`crates/dsrs/src/pipelines/retrieval.rs`)
  - QueryComposerPipeline - Transform goals into search queries
  - RetrievalRouterPipeline - Route to appropriate lane (ripgrep/lsp/semantic/git)
  - CandidateRerankPipeline - LLM-based reranking
  - 9 pipeline tests passing
- [x] **Adjutant Decision Pipelines** (`crates/adjutant/src/dspy/decision_pipelines.rs`)
  - ComplexityPipeline - Classify task complexity (Low/Medium/High/VeryHigh)
  - DelegationPipeline - Decide whether/where to delegate (codex_code/rlm/local_tools)
  - RlmTriggerPipeline - Decide whether to use RLM for deep analysis
  - Wired into Adjutant.execute() with DSPy-first + legacy fallback
  - 31 tests passing
- [x] **Adjutant LM Caching + Training Collection** (`crates/adjutant/src/lib.rs`)
  - Lazy LM initialization with `get_or_create_decision_lm()`
  - Cached LM shared across all decision pipeline calls
  - Training data collection for decision pipelines:
    - ComplexityTrainingExample, DelegationTrainingExample, RlmTriggerTrainingExample
    - Records high-confidence (>0.7) decisions for MIPROv2 optimization
    - Saved to `~/.openagents/adjutant/training/dataset.json`

### Wave 14: Self-Improving Autopilot (Complete)
- [x] **Session Tracking** (`crates/adjutant/src/dspy/sessions.rs`)
  - AutopilotSession with DecisionRecord, VerificationRecord
  - SessionStore with persistence to `~/.openagents/adjutant/sessions/`
  - SessionIndex for fast lookup and filtering
  - Integration with autopilot_loop.rs
- [x] **Outcome Feedback** (`crates/adjutant/src/dspy/outcome_feedback.rs`)
  - Links task outcomes to decision correctness
  - LabeledExample with ground-truth labels
  - LabeledExamplesStore with per-signature storage
  - Correctness logic: complexity (iterations), delegation (success), RLM (context size)
- [x] **Performance Tracking** (`crates/adjutant/src/dspy/performance.rs`)
  - RollingAccuracy with configurable window size (default: 50)
  - PerformanceTracker with per-signature metrics
  - AccuracySnapshot for historical tracking
  - OptimizationRun records for audit trail
- [x] **Auto Optimization** (`crates/adjutant/src/dspy/auto_optimizer.rs`)
  - AutoOptimizerConfig with configurable triggers
  - AutoOptimizer checks example threshold, accuracy drop, time-based
  - SelfImprover coordinates the full self-improvement loop
  - Automatic signature selection (prioritizes lowest accuracy)
- [x] **CLI Commands** (`crates/adjutant/src/cli/dspy.rs`)
  - `autopilot dspy sessions` - View session history
  - `autopilot dspy performance` - View accuracy metrics
  - `autopilot dspy auto-optimize` - Configure auto-optimization
- [x] **Storage Layout**
  ```
  ~/.openagents/adjutant/
  ├── sessions/           # Session tracking
  ├── training/labeled/   # Labeled examples with ground truth
  ├── metrics/            # Performance metrics
  └── config/             # Auto-optimizer settings
  ```
- [x] 46 tests passing

---

## Full Integration Waves (15-21)

The following waves complete DSPy integration across the entire codebase, replacing all hardcoded prompts and heuristics with optimizable signatures.

### Wave 15: Tiered Executor DSPy Migration (Complete)

**Goal:** Replace hardcoded prompts in tiered.rs with the existing DSPy module.

**Files:**
- `crates/adjutant/src/tiered.rs` — Main target
- `crates/adjutant/src/dspy/module.rs` — Existing signatures

**Key Insight:** The DSPy module already exists. The executor has `ExecutionMode::Dsrs` but defaults to `Gateway`. Wave 15 makes Dsrs the default.

**Tasks:**
- [x] Change default `ExecutionMode` from `Gateway` to `Dsrs`
- [x] Refactor `execute()` to route based on mode (DSPy-first with gateway fallback)
- [x] Create `execute_gateway()` for legacy execution path
- [x] Training collector already wired in `execute_dsrs()`
- [x] Add fallback to Gateway mode if DSPy fails

**New Signatures:** None (uses existing SubtaskPlanningSignature, SubtaskExecutionSignature, ResultSynthesisSignature)

### Wave 16: RLM DSPy Integration (In Progress)

**Goal:** Replace hardcoded RLM prompts with optimizable signatures.

**Files:**
- `crates/rlm/src/prompts.rs` — 4 hardcoded prompts to replace
- `crates/rlm/src/codex_client.rs` — Direct SDK calls
- `crates/rlm/src/dspy.rs` — DSPy signatures (created)

**Hardcoded Prompts to Replace:**
1. `BASIC_SYSTEM_PROMPT` — Simple code execution (line 39)
2. `CONTEXT_SYSTEM_PROMPT` — Full RLM with llm_query() (line 70)
3. `GUIDED_SYSTEM_PROMPT` — Apple FM tier (line 152)
4. `MINIMAL_SYSTEM_PROMPT` — Small models (line 196)

**Tasks:**
- [x] Create `RlmQuerySignature` — Basic RLM query
- [x] Create `RlmContextQuerySignature` — Context-aware RLM with llm_query()
- [x] Create `RlmGuidedQuerySignature` — Guided tier for Apple FM
- [x] Create `RlmCodeGenerationSignature` — Generate REPL code
- [x] Create `RlmContinuationSignature` — Handle continuation after execution
- [ ] Update `codex_client.rs` to use signatures via dsrs Predict
- [ ] Keep PromptTier enum but route to appropriate signature
- [ ] Add training collection for RLM queries

**Signatures Created** (`crates/rlm/src/dspy.rs`):
```rust
#[Signature] struct RlmQuerySignature { ... }           // Basic RLM
#[Signature] struct RlmContextQuerySignature { ... }    // Full RLM with llm_query()
#[Signature] struct RlmGuidedQuerySignature { ... }     // Apple FM tier
#[Signature] struct RlmCodeGenerationSignature { ... }  // Code generation
#[Signature] struct RlmContinuationSignature { ... }    // Continuation handling
```

### Wave 17: LM-Router DSPy Backend (Complete)

**Goal:** Create DSPy-aware LM provider that wraps all backends.

**Files:**
- `crates/lm-router/src/backends/mod.rs` — Backend registry
- `crates/lm-router/src/dspy_backend.rs` — NEW
- `crates/dsrs/src/core/lm/lm_router.rs` — NEW

**Tasks:**
- [x] Create `LmRouterLM` implementing dsrs `LM` trait
- [x] Support signature-based routing (cheap models for simple signatures)
- [x] Add signature-aware caching
- [x] Integrate with dsrs client registry

### Wave 18: Gateway DSPy Integration (Complete)

**Goal:** Wrap gateway calls in DSPy for structured tasks.

**Files:**
- `crates/gateway/src/lib.rs` — Gateway module
- `crates/gateway/src/inference/cerebras.rs` — Cerebras backend
- `crates/gateway/src/dspy.rs` — NEW

**Tasks:**
- [x] Create `GatewayLM` implementing dsrs `LM` trait
- [x] Keep raw gateway for unstructured calls
- [x] Add `gateway.query_with_signature()` method
- [x] Integrate with dsrs tracing

### Wave 19: Autopilot Heuristics → DSPy (Complete)

**Goal:** Replace keyword matching with learned signatures.

**Files:**
- `crates/autopilot-core/src/dspy_planning.rs` — `is_complex_task()` heuristics
- `crates/autopilot-core/src/dspy_verify.rs` — Status detection
- `crates/autopilot-core/src/dspy_optimization.rs` — Validation logic

**Tasks:**
- [x] Create `TaskComplexityClassifier` — Replace keyword heuristics
- [x] Create `BuildStatusClassifier` — Learn build failure patterns
- [x] Create `TestStatusClassifier` — Learn test failure patterns
- [x] Create `PathValidationSignature` — Learn valid path patterns
- [x] Create `ActionableStepSignature` — Learn actionable language
- [x] Wire all to training collection

**New Signatures:**
```rust
#[Signature]
struct TaskComplexityClassifier {
    /// Classify task complexity for planning depth.
    #[input] task_description: String,
    #[input] file_count: u32,
    #[input] codebase_context: String,
    #[output] complexity: String,     // Simple/Moderate/Complex/VeryComplex
    #[output] reasoning: String,
    #[output] confidence: f32,
}

#[Signature]
struct BuildStatusClassifier {
    /// Classify build output status.
    #[input] build_output: String,
    #[input] command: String,
    #[output] status: String,         // Success/Warning/Error/Fatal
    #[output] error_type: String,
    #[output] actionable: bool,
}
```

### Wave 20: Agent-Orchestrator & Nexus DSPy (Complete)

**Goal:** Add semantic understanding to event/directive handling.

**Files:**
- `crates/agent-orchestrator/src/integrations/directives.rs` — Directive parsing
- `crates/agent-orchestrator/src/integrations/autopilot.rs` — Issue selection
- `crates/nexus/src/filter.rs` — Event filtering
- `crates/nexus/src/protocol/nip90.rs` — Job classification

**Tasks:**
- [x] Create `DirectiveStatusParser` — Learn status from text
- [x] Create `DirectivePriorityClassifier` — Learn priority from context
- [x] Create `DirectiveMatchingSignature` — Semantic matching
- [x] Create `IssueSelectionSignature` — Learn issue prioritization
- [x] Create `EventIntentClassifier` — Classify event intent from content
- [x] Create `JobKindClassifier` — Learn NIP-90 job types

**New Signatures:**
```rust
#[Signature]
struct IssueSelectionSignature {
    /// Select next issue to work on.
    #[input] open_issues: String,
    #[input] agent_capabilities: String,
    #[input] recent_work: String,
    #[output] selected_issue_id: String,
    #[output] reasoning: String,
    #[output] estimated_complexity: String,
}

#[Signature]
struct EventIntentClassifier {
    /// Classify Nostr event intent.
    #[input] event_kind: u32,
    #[input] event_content: String,
    #[input] event_tags: String,
    #[output] intent: String,
    #[output] priority: String,
    #[output] requires_response: bool,
}
```

### Wave 21: Marketplace Security DSPy (Complete)

**Goal:** Learn security policies instead of hardcoding.

**Files:**
- `crates/marketplace/src/skills/execution.rs` — Permission/resource validation
- `crates/marketplace/src/dspy_security.rs` — NEW

**Tasks:**
- [x] Create `FilesystemPermissionSignature` — Learn safe permissions
- [x] Create `ResourceLimitSignature` — Learn appropriate limits
- [x] Create `SafePathValidationSignature` — Learn path safety patterns
- [x] Create `SkillSecurityClassifier` — Classify skill risk level
- [x] Add human-in-the-loop for high-risk decisions (Critical/High only)

**Security Policy:**
- `Low` and `Medium` risk: Execute with audit logging
- `High` and `Critical` risk: Require human approval
- All decisions logged for review

**New Signatures:**
```rust
#[Signature]
struct SkillSecurityClassifier {
    /// Classify security risk of a skill execution.
    #[input] skill_manifest: String,
    #[input] requested_permissions: String,
    #[input] execution_context: String,
    #[output] risk_level: String,     // Low/Medium/High/Critical
    #[output] concerns: String,       // JSON array
    #[output] recommended_sandbox: String,
    #[output] requires_approval: bool,
}
```

---

## Wave 0: Protocol + Schema Registry (Complete)

**Why first:** If you don't do this, every client/provider will drift and "replayability" collapses.

### Deliverables

- `crates/protocol/src/jobs/{chunk_analysis,rerank,sandbox}.jsonschema` — request/response schemas
- `crates/protocol/src/hash.rs` — canonical JSON + stable hashing
- `crates/protocol/src/versioning.md` — how to bump `v1 → v2`

### Job Schema Contract

For every job type, include:
- `job_type` — e.g., `oa.code_chunk_analysis.v1`
- `schema_version` — e.g., `1.0.0`
- `job_hash` — server computed, deterministic, included in receipt

### Canonical Hashing

```rust
pub fn canonical_hash(input: &Value) -> String {
    // 1. Serialize to canonical JSON (sorted keys, no whitespace)
    // 2. SHA-256 hash
    // 3. Return hex string
}
```

### Version Bump Rules

- **Patch** (`v1.0.0` → `v1.0.1`): Bug fixes, no schema changes
- **Minor** (`v1.0.0` → `v1.1.0`): New optional fields, backward compatible
- **Major** (`v1` → `v2`): Breaking changes, new job type name

---

## Agent Module Graph

The minimal set of modules for "real agent for real repos" — optimizable (DSPy-style) and swarm-parallelizable (OpenAgents-style):

```
User Task (issue / prompt)
        │
        ▼
┌──────────────────────────┐
│ 0) Task Router           │  (budget, latency, privacy)
│    - picks lanes         │  local vs swarm vs datacenter
└──────────┬───────────────┘
           │
           ▼
┌──────────────────────────┐
│ 1) Query Composer        │  (DSPy)
│    - rewrite / expand    │
│    - generate sub-queries│
└──────────┬───────────────┘
           │
           ▼
┌──────────────────────────────────────────────────────────┐
│ 2) Retrieval Router (multi-lane)                          │
│    - lexical grep/ripgrep                                 │
│    - symbol/LSP (defs/refs, implementations, call sites)  │
│    - semantic vector search                               │
│    - history (git blame/log)                              │
│    - test failure signal (stack traces)                   │
└──────────┬───────────────────────┬───────────────────────┘
           │                       │
           ▼                       ▼
┌──────────────────────────┐    ┌──────────────────────────┐
│ 3) Evidence Ranker        │    │ 4) Evidence Workers       │
│    - pick top-K files     │    │ (SWARM MAP)               │
│    - decide chunking      │    │ - summarize file/chunks   │
│    - decide what to open  │    │ - extract symbols         │
└──────────┬───────────────┘    │ - propose hypotheses      │
           │                    └──────────┬───────────────┘
           └──────────────┬────────────────┘
                          ▼
                 ┌─────────────────────┐
                 │ 4.5) State / Memory │ (NEW)
                 │ - already tried     │
                 │ - already opened    │
                 │ - hypotheses failed │
                 └──────────┬──────────┘
                            ▼
                 ┌─────────────────────┐
                 │ 5) Patch Planner     │ (DSPy)
                 │ - minimal plan       │
                 │ - tests to run       │
                 │ - risk assessment    │
                 └──────────┬──────────┘
                            ▼
                 ┌─────────────────────┐
                 │ 6) Patch Writer      │ (tool-driven)
                 │ - apply edits        │
                 │ - format/lint        │
                 └──────────┬──────────┘
                            ▼
                 ┌─────────────────────┐
                 │ 7) Verifier          │
                 │ - build/test matrix  │ (SWARM SANDBOX)
                 │ - static checks      │ (SWARM CPU)
                 │ - runtime repro      │
                 └──────────┬──────────┘
                            ▼
                 ┌─────────────────────┐
                 │ 8) Fix Loop          │ (DSPy policy)
                 │ - interpret failures │
                 │ - decide next probes │
                 └──────────┬──────────┘
                            ▼
                        FINAL PATCH
```

**State/Memory module** is critical for stopping repeated wasted probes and is very optimizable.

**DSPy modules** drive *routing decisions, query rewriting, evidence ranking policies, planning policies, failure interpretation* — anything you want to **optimize/compile** against an eval suite.

**Tool-driven modules** do *actually editing files, running tests, calling LSP, running grep*, etc.

**DSPy drives the policy, tools do the work.**

---

## Wave 3: Compiler Contract & Integration Layer

### 3.0 Compiler Contract (NEW)
**Goal:** dsrs modules become first-class citizens in OpenAgents with standardized traces, compiled module IDs, cost accounting, and replayability.

**Deliverables:**

#### CompiledModuleManifest
Every compiled module emits (and is tagged with):
```rust
pub struct CompiledModuleManifest {
    pub signature_name: String,
    pub compiled_id: String,          // Hash of optimized artifact
    pub optimizer: String,            // MIPROv2/GEPA
    pub trainset_id: String,
    pub scorecard: Scorecard,         // Proxy + truth metrics
    pub compatibility: Compatibility, // Model lane, tool availability
    pub created_at: u64,
}

pub struct Compatibility {
    pub required_tools: Vec<String>,     // e.g., ["ripgrep", "node", "pytest"]
    pub required_lanes: Vec<String>,     // e.g., ["lsp", "semantic"]
    pub privacy_modes_allowed: Vec<PrivacyMode>,
    pub min_provider_reputation: Option<f32>,
}
```

**Compatibility is machine-checkable** at runtime — the router can reject "this compiled module is invalid for this environment."

#### TraceContract (dsrs DAG → OA spans)
Map dsrs execution nodes to OpenAgents trace spans:
```rust
pub fn graph_to_spans(graph: &Graph) -> Vec<OASpan> {
    // session → turn → module → retrieval → swarm.job → patch → verify
}
```

Add to spans for OTel alignment (without requiring OTel):
- `span_kind`: `"client" | "server" | "internal"`
- `resource`: `{ "service.name": "...", "service.version": "...", "host.arch": "...", ... }`

#### TraceExtractorSpec (OA trace → dsrs Examples)
Extract training examples from successful runs.

### 3.1 Pylon Inference Provider
**File:** `crates/dsrs/src/adapter/pylon.rs` ✅

Pylon as an LM provider in dsrs (NIP-90 inference jobs):

```rust
pub struct PylonInferenceProvider {
    relay_url: String,
    pubkey: XOnlyPublicKey,
    budget_sats: u64,
}

impl CompletionProvider for PylonInferenceProvider {
    // Submit NIP-90 job → Wait → Pay Lightning → Return result
}
```

### 3.2 Pylon Sandbox Provider (NEW)
**File:** `crates/dsrs/src/adapter/pylon_sandbox.rs`

Separate provider for sandbox execution (Daytona-style, CPU-heavy):

```rust
pub struct PylonSandboxProvider {
    relay_url: String,
    resource_profile: SandboxProfile,  // S/M/L
}

pub enum SandboxProfile {
    S { vcpus: 1, memory_mb: 1024, disk_gb: 5 },
    M { vcpus: 2, memory_mb: 4096, disk_gb: 8 },
    L { vcpus: 4, memory_mb: 8192, disk_gb: 10 },
}
```

**Why split?** DSPy will compile differently when the tool is "LLM call" vs "run tests".

### 3.3 DAG → Nostr Bridge
**File:** `crates/dsrs/src/trace/nostr_bridge.rs`

Convert **only receiptable nodes** to Nostr events (don't spam relays):
- LM calls (local/swarm/dc)
- sandbox runs
- retrieval calls
- patch application
- verification decisions

```rust
pub fn graph_to_nostr_events(graph: &Graph) -> Vec<NostrEvent> {
    graph.nodes.iter()
        .filter(|n| n.is_receiptable())
        .map(|node| match &node.node_type {
            NodeType::Predict { .. } => kind_5050_job(node),
            NodeType::Sandbox { .. } => kind_5050_sandbox(node),
            NodeType::Retrieval { .. } => kind_1_with_tags(node),
            _ => kind_1_text_note(node),
        }).collect()
}
```

### 3.4 Callback System for HUD
**File:** `crates/dsrs/src/callbacks.rs`

Upgrade to emit **OA span events** with compiled module hash and signature name:

```rust
pub trait DspyCallback: Send + Sync {
    fn on_module_start(&self, call_id: Uuid, instance: &dyn Module, inputs: &Example);
    fn on_module_end(&self, call_id: Uuid, outputs: Result<&Prediction>, exception: Option<&Error>);
    fn on_lm_start(&self, call_id: Uuid, instance: &LM, inputs: &Chat);
    fn on_lm_end(&self, call_id: Uuid, outputs: Result<&Message>, exception: Option<&Error>);
    fn on_optimizer_candidate(&self, candidate_id: String, metrics: HashMap<String, f32>);
}
```

Each callback attaches:
- compiled module hash
- signature name
- inputs/outputs hashes (for privacy + replay)

### 3.5 Refine Pattern for Verification
**File:** `crates/dsrs/src/predictors/refine.rs`

Meta-operator wrapping any module with:
- retry budget
- fallback model lane
- enforced structured output
- failure attribution into traces

```rust
pub struct Refine<M: Module> {
    module: M,
    n_rollouts: usize,
    reward_fn: Box<dyn Fn(&Example, &Prediction) -> f32>,
    threshold: f32,
    fallback_lane: ModelLane,
}
```

---

## Wave 4: Swarm Job Types, Retrieval Policy & Indexing

### 4.0 Repo Indexing Layer (NEW)

**Why:** Without this, "retrieval router" compiles policies on sand.

**Deliverables:**

#### RepoIndex Abstraction
```rust
pub trait RepoIndex: Send + Sync {
    async fn build(&mut self, repo_path: &Path) -> Result<()>;
    async fn update(&mut self, changed_files: &[PathBuf]) -> Result<()>;
    async fn query(&self, lane: &str, query: &str, k: usize) -> Result<Vec<Candidate>>;
}
```

#### Pluggable Backends
- `ripgrep` / lexical — fast text search
- `lsp` symbol index — definitions, references, implementations
- `vector` embedding store (local first) — semantic search
- `git` signals — blame/log/diff for history-aware retrieval

#### Commands
- `index build` — full index from scratch
- `index update` — incremental update from git diff

#### Trace Spans
- `index.build` — full index build
- `index.update` — incremental update

**Files to Create:**
- `crates/dsrs/src/retrieval/index.rs`
- `crates/dsrs/src/retrieval/backends/{ripgrep,lsp,vector,git}.rs`

### 4.1 Canonical Swarm Job Types

These become the "map-reduce primitives" DSPy compiles against.

#### Verification Mode (NEW)

**Every swarm job request includes:**
```json
{
  "verification": {
    "mode": "objective" | "subjective",
    "redundancy": 1,
    "adjudication": "none" | "majority_vote" | "judge_model"
  }
}
```

- **Objective** (verifiable): tests, lint, typecheck, deterministic commands
- **Subjective** (not strictly verifiable): summaries, hypotheses, reranks

**Defaults:**
- `oa.sandbox_run.v1` → `objective`, redundancy=1 (or 2 for anti-cheat)
- `oa.code_chunk_analysis.v1` → `subjective`, redundancy=2-3, adjudication=`judge_model`
- `oa.retrieval_rerank.v1` → `subjective`, redundancy=2, adjudication=`majority_vote`

This makes the economics and trust model legible immediately.

#### `oa.code_chunk_analysis.v1`
Parallel "map" work over candidate files/chunks:
- summarize
- extract symbols
- invariants
- bug hypotheses
- recommended next probes

**Request:**
```json
{
  "job_type": "oa.code_chunk_analysis.v1",
  "schema_version": "1.0.0",
  "verification": {
    "mode": "subjective",
    "redundancy": 2,
    "adjudication": "judge_model"
  },
  "task": "bug_hypothesis",
  "user_task": "Fix failing tests for empty prompt handling",
  "chunk": { "path": "src/service.py", "start_line": 1, "end_line": 160, "content": "..." },
  "output_constraints": { "max_summary_chars": 1500, "max_findings": 6 }
}
```

**Response:**
```json
{
  "job_hash": "sha256:...",
  "summary": "...",
  "symbols": [{"name": "SuggestionService", "kind": "class", "line": 120}],
  "suspected_faults": [{"line": 244, "reason": "..."}],
  "recommended_next_probes": [{"lane": "lsp_refs", "query": "normalize_prompt"}],
  "confidence": 0.85,
  "provenance": {
    "model_id": "...",
    "sampling": {"temperature": 0.7, "top_p": 0.95},
    "input_sha256": "...",
    "output_sha256": "..."
  }
}
```

#### `oa.retrieval_rerank.v1`
LLM-based reranking of candidate set:

**Request:**
```json
{
  "job_type": "oa.retrieval_rerank.v1",
  "schema_version": "1.0.0",
  "verification": {
    "mode": "subjective",
    "redundancy": 2,
    "adjudication": "majority_vote"
  },
  "user_task": "...",
  "candidates": [{"candidate_id": "c1", "lane": "semantic", "path": "...", "snippet": "..."}],
  "k": 10,
  "ranking_rubric": { "prefer_fix_surface": true }
}
```

**Response:**
```json
{
  "job_hash": "sha256:...",
  "topk": [{"rank": 1, "candidate_id": "c1", "score": 0.92, "why": "..."}],
  "provenance": {
    "model_id": "...",
    "sampling": {"temperature": 0.7, "top_p": 0.95},
    "input_sha256": "...",
    "output_sha256": "..."
  }
}
```

#### `oa.sandbox_run.v1`
Concurrent sandbox execution (builds/tests/lints):

**Request:**
```json
{
  "job_type": "oa.sandbox_run.v1",
  "schema_version": "1.0.0",
  "verification": {
    "mode": "objective",
    "redundancy": 1,
    "adjudication": "none"
  },
  "sandbox": {
    "provider": "daytona",
    "container_image_digest": "sha256:...",
    "network_policy": "none" | "egress_only" | "full",
    "resources": {"vcpus": 2, "memory_mb": 4096, "disk_gb": 10}
  },
  "repo": {
    "fetch_policy": "git_clone" | "tarball" | "preloaded",
    "git_url": "...",
    "git_commit": "..."
  },
  "commands": [
    {"name": "test", "cmd": "pytest -q", "timeout_ms": 300000, "max_output_bytes": 1048576}
  ]
}
```

**Response:**
```json
{
  "job_hash": "sha256:...",
  "env": {
    "image_digest": "sha256:...",
    "runner_version": "pylon-x.y.z"
  },
  "runs": [
    {
      "name": "test",
      "exit_code": 0,
      "duration_ms": 12000,
      "stdout_sha256": "...",
      "stderr_sha256": "..."
    }
  ],
  "artifacts": {
    "junit_sha256": "..."
  },
  "overall": { "status": "pass" }
}
```

**Security:** This is the backbone of "receipts" and "trust."

### 4.2 Retrieval Policy Signatures (NEW)

Turn "retrieval as feature" into "retrieval as optimizable policy":

#### QueryComposerSignature
```rust
#[Signature]
struct QueryComposerSignature {
    /// Turn user goal + failure logs into targeted retrieval probes.

    #[input] user_goal: String,
    #[input] failure_log: String,
    #[input] repo_summary: String,

    #[output] grep_queries: String,      // JSON array
    #[output] semantic_queries: String,  // JSON array
    #[output] symbol_queries: String,    // JSON array
    #[output] confidence: f32,
}
```

#### RetrievalRouterSignature
```rust
#[Signature]
struct RetrievalRouterSignature {
    /// Decide which retrieval lane and K to use. This is where you beat "always semantic search".

    #[input] user_goal: String,
    #[input] repo_summary: String,
    #[input] available_lanes: String,    // JSON: grep/semantic/lsp/git/tests
    #[input] budget_hint: String,

    #[output] lane_plan: String,         // JSON: [{lane, k, query, reason}]
    #[output] stop_condition: String,    // "enough_evidence" | "need_more"
    #[output] confidence: f32,
}
```

#### CandidateRerankSignature
```rust
#[Signature]
struct CandidateRerankSignature {
    /// Rerank candidates (maps to oa.retrieval_rerank.v1)

    #[input] user_goal: String,
    #[input] candidates: String,

    #[output] topk: String,              // JSON ordered list + why
    #[output] expansion_queries: String, // Follow-ups
}
```

### 4.3 Chunk Analysis Signatures (NEW)

#### ChunkTaskSelectorSignature
```rust
#[Signature]
struct ChunkTaskSelectorSignature {
    /// Select what analysis to request per chunk.

    #[input] user_goal: String,
    #[input] failure_log: String,
    #[input] candidate_meta: String,

    #[output] task: String,              // summarize|bug_hypothesis|extract_symbols
    #[output] instructions: String,
}
```

#### ChunkAnalysisToActionSignature
```rust
#[Signature]
struct ChunkAnalysisToActionSignature {
    /// Aggregate chunk analyses into next steps.

    #[input] user_goal: String,
    #[input] analyses: String,           // JSON array of chunk analysis outputs

    #[output] next_probes: String,       // JSON: [{lane, query, why}]
    #[output] likely_fix_files: String,
    #[output] confidence: f32,
}
```

### 4.4 Sandbox Signatures (NEW)

#### SandboxProfileSelectionSignature
```rust
#[Signature]
struct SandboxProfileSelectionSignature {
    /// Choose S/M/L resources. This directly controls cost.

    #[input] repo_summary: String,
    #[input] planned_commands: String,
    #[input] org_limits: String,

    #[output] profile: String,           // "S"|"M"|"L"
    #[output] resources: String,         // JSON {vcpus, memory_mb, disk_gb}
    #[output] rationale: String,
}
```

#### FailureTriageSignature
```rust
#[Signature]
struct FailureTriageSignature {
    /// Turn failing sandbox run into best next move.

    #[input] user_goal: String,
    #[input] command: String,
    #[input] stdout_tail: String,
    #[input] stderr_tail: String,
    #[input] last_patch_summary: String,

    #[output] diagnosis: String,
    #[output] next_action: String,       // "search"|"patch"|"rerun"|"escalate"
    #[output] recommended_probe: String,
}
```

### 4.5 Lane Budgeting Signatures (NEW)

#### LaneBudgeterSignature
```rust
#[Signature]
struct LaneBudgeterSignature {
    /// Decide when to use swarm vs local vs datacenter.

    #[input] task_type: String,
    #[input] urgency: String,            // interactive|batch
    #[input] remaining_budget_msats: u64,
    #[input] estimated_costs: String,

    #[output] chosen_lane: String,       // local|swarm|datacenter
    #[output] max_msats: u64,
    #[output] rationale: String,
}
```

### 4.6 State / Memory Signatures (NEW)

#### AgentMemorySignature
```rust
#[Signature]
struct AgentMemorySignature {
    /// Track what we've already tried to avoid repeated wasted probes.

    #[input] session_history: String,    // JSON: files opened, hypotheses tested, patches tried
    #[input] current_goal: String,
    #[input] proposed_action: String,

    #[output] is_redundant: bool,        // Have we tried this before?
    #[output] similar_attempts: String,  // JSON: previous similar attempts
    #[output] recommended_alternative: String,
}
```

**Files to Create:**
- `crates/dsrs/src/signatures/retrieval_policy.rs`
- `crates/dsrs/src/signatures/chunk_analysis.rs`
- `crates/dsrs/src/signatures/sandbox.rs`
- `crates/dsrs/src/signatures/budgeting.rs`
- `crates/dsrs/src/signatures/memory.rs`

---

## Wave 5: Eval Harness & Promotion Gates

**Why before compiler:** You can't compile anything without an eval harness.

### 5.1 Eval Task Format

```json
{
  "task_id": "py-0007",
  "repo": {
    "git_url": "...",
    "commit_bad": "BAD_SHA",
    "commit_fix": "FIX_SHA"
  },
  "goal": "Fix failing tests for empty prompt handling",
  "setup_cmds": ["pip install -e ."],
  "fail_cmd": "pytest -q",
  "gold": {
    "files_touched": ["src/service.py", "tests/test_service.py"],
    "symbols": ["SuggestionService.predict"]
  },
  "constraints": {
    "max_files_touched": 4,
    "no_new_deps": true
  }
}
```

**Why include `commit_fix`?** Auto-generate gold files/symbols by diffing, validate "your patch is in the right neighborhood."

### 5.2 Two-Tier Metrics (NEW)

#### Proxy Metrics (cheap, frequent)
- Retrieval recall@k vs gold file list
- Evidence relevance score
- Number of files opened / bytes read
- Loop count
- Sats spent

#### Truth Metrics (expensive, definitive)
- Tests pass in sandbox
- Minimal diff size / no regressions
- No policy violations

**Strategy:** Compile using proxy metrics most of the time, **validate candidates** on truth metrics.

### 5.3 Scoring Function (Robust)

```
score =
  median(score over N rollouts)  // Multi-rollout aggregation
  where single_score =
    1.0 * pass_tests
    - 0.25 * min(1, total_cost_msats / budget_msats)
    - 0.15 * min(1, total_time_ms / time_budget_ms)
    - 0.10 * min(1, diff_lines_changed / diff_budget_lines)
    - 0.10 * min(1, sandbox_runs / sandbox_budget_runs)
    - 0.10 * min(1, bytes_opened / bytes_budget)         // Evidence efficiency
    - 0.10 * min(1, evidence_workers_fanout / fanout_budget)  // Evidence efficiency
```

Also track: `p_fail` (fail rate across rollouts) — reduces optimizer overfitting to lucky sampling.

This makes the optimizer learn "don't brute force."

### 5.4 Promotion Gates (NEW)

A compiled policy should not roll out globally unless it passes:

1. **Regression suite** — All existing tasks still pass
2. **Budget cap sanity** — No runaway cost
3. **Failure mode checks** — Timeouts, runaway loops

#### Policy Registry
```
candidate → staged → shadow → promoted → rolled_back
```

#### Shadow Mode Gate (NEW)

For a period:
- Run *both* old and new policy on the same tasks
- Only ship the old result
- Compare success/cost/time metrics
- Promote only if new beats old

This avoids "we promoted a policy that looked good offline but fails live."

#### A/B Routing
Route by `compiled_id` in production.

### 5.5 Compile Order

Start compiling **only policies**, not everything:

1. **RetrievalRouter policy** — How much semantic vs grep vs LSP, K, chunking, dedupe
2. **EvidenceRanker rubric** — Choose best 10 chunks to analyze
3. **FailureInterpreter** — What to do after failed test run (where most agents waste money)

Then:
4. PatchPlanner style / constraints
5. PatchWriter strategies (multi-candidate vs single)

---

## Wave 6: Swarm-Backed Optimization

### 6.1 SwarmCompiler
**File:** `crates/autopilot-core/src/dspy_compiler.rs`

```rust
pub struct SwarmCompiler {
    swarm_lm: LM,       // Pylon provider (cheap)
    validation_lm: LM,  // Codex/GPT-4 (premium)
    budget: BudgetPolicy,
}

impl SwarmCompiler {
    pub async fn compile<M: Module + Optimizable + Evaluator>(
        &self,
        module: &mut M,
        trainset: Vec<Example>,
    ) -> Result<OptimizationResult> {
        // Stage 1: Bootstrap on swarm (10 msats/call)
        // Stage 2: Optimize with MIPROv2
        // Stage 3: Validate on premium model
    }
}
```

**Cost reduction example:**
- Codex alone: ~$15 for 1,800 calls
- With swarm optimization: ~$0.10 (96.7% reduction)

### 6.2 TraceExtractor
**File:** `crates/autopilot-core/src/trace_extraction.rs`

Extract training examples from successful sessions:

```rust
pub struct TraceExtractor;

impl TraceExtractor {
    pub fn extract_from_session(path: &Path) -> Vec<Example> {
        // Parse JSONL session log
        // Filter for successful completions
        // Extract input/output pairs per phase
    }
}
```

---

## Wave 7: Privacy & Redaction Mode

### 7.1 Private Repo Swarm Mode

For swarm compilation on real repos:

**Deliverables:**
- Redaction transforms (paths, identifiers, string literals, secrets)
- Chunking policy (only ship smallest necessary spans)
- Allowlist of job types permitted for private repos
- Optional "trusted providers" lane

### 7.2 Privacy Constraints

```rust
pub enum PrivacyMode {
    PublicOk,           // Can ship full content
    NoPii,              // Redact PII
    PrivateRepoRedacted, // Redact paths, identifiers
    PrivateRepoAllowed,  // Trusted providers only
}
```

---

## Wave 8: OANIX (Agent OS Runtime) - COMPLETE

*Implemented in `crates/oanix/`*

Transform OANIX's rule-based decision making into DSPy signatures. All signatures implement `MetaSignature` trait manually for public API compatibility.

### Implemented Signatures

| Signature | File | Purpose |
|-----------|------|---------|
| `SituationAssessmentSignature` | `dspy_situation.rs` | Analyze system state, determine priorities |
| `IssueSelectionSignature` | `dspy_lifecycle.rs` | Choose best issue to work on |
| `WorkPrioritizationSignature` | `dspy_lifecycle.rs` | Order tasks by importance/dependencies |
| `LifecycleDecisionSignature` | `dspy_lifecycle.rs` | CoT for agent state transitions |

### Enums

- `PriorityAction`: AWAIT_USER, WORK_ISSUE, ACCEPT_JOB, START_PROVIDER, INITIALIZE_IDENTITY, CONNECT_NETWORK, HOUSEKEEPING, IDLE
- `Urgency`: IMMEDIATE, NORMAL, DEFERRED
- `LifecycleState`: IDLE, WORKING, BLOCKED, PROVIDER, TERMINATING
- `Complexity`: LOW, MEDIUM, HIGH

### Files Created
- `crates/oanix/src/dspy_situation.rs` - SituationAssessmentSignature
- `crates/oanix/src/dspy_lifecycle.rs` - 3 lifecycle signatures

---

## Wave 9: Agent Orchestrator - COMPLETE

*Implemented in `crates/agent-orchestrator/`*

Convert the 7 specialized agent prompts into DSPy Signatures for learned, optimizable delegation and agent behavior.

### Actual Agents (in `agent-orchestrator`)

| Agent | Model | Role | Mode |
|-------|-------|------|------|
| **Sisyphus** | codex-opus-4-5 | Primary orchestrator | Primary |
| **Oracle** | gpt-5.2 | Architecture, debugging | Subagent |
| **Librarian** | codex-sonnet-4 | External docs, OSS | Subagent |
| **Explore** | grok-3 | Codebase navigation | Subagent |
| **Frontend** | gemini-2.5-pro | UI/UX design | Subagent |
| **DocWriter** | gemini-2.5-pro | Documentation | Subagent |
| **Multimodal** | gemini-2.5-flash | PDF/image analysis | Subagent |

### Implemented Signatures

| Signature | File | Purpose |
|-----------|------|---------|
| `DelegationSignature` | `dspy_delegation.rs` | Sisyphus decides which subagent handles task |
| `ArchitectureSignature` | `dspy_agents.rs` | CoT for Oracle architecture decisions |
| `LibraryLookupSignature` | `dspy_agents.rs` | Librarian external docs lookup |
| `CodeExplorationSignature` | `dspy_agents.rs` | Explore codebase navigation |
| `UIDesignSignature` | `dspy_agents.rs` | Frontend UI/UX design |
| `DocumentationSignature` | `dspy_agents.rs` | DocWriter technical docs |
| `MediaAnalysisSignature` | `dspy_agents.rs` | Multimodal PDF/image analysis |

### Helper Enums

- `TargetAgent`: oracle, librarian, explore, frontend, docwriter, multimodal, direct
- `ArchitectureComplexity`: LOW, MEDIUM, HIGH, CRITICAL
- `SearchType`: Definition, References, Pattern, Usage, CallGraph
- `DocType`: Readme, ApiRef, Guide, Comment, Changelog
- `MediaType`: Image, Pdf, Diagram, Screenshot, Video

### Files Created
- `crates/agent-orchestrator/src/dspy_delegation.rs` - DelegationSignature + TargetAgent
- `crates/agent-orchestrator/src/dspy_agents.rs` - 6 agent signatures + enums

---

## Wave 10: Tool Invocation - COMPLETE

*Implemented in `crates/runtime/`*

Universal tool selection and interpretation layer. All signatures implement `MetaSignature` trait manually for public API compatibility.

### Implemented Signatures

| Signature | File | Purpose |
|-----------|------|---------|
| `ToolSelectionSignature` | `dspy_tools.rs` | Choose the right tool for any task |
| `ToolResultInterpretationSignature` | `dspy_tools.rs` | Understand what a tool result means |
| `ToolChainPlanningSignature` | `dspy_tools.rs` | Plan multi-tool sequences |

### Helper Enum

- `ToolSuccess`: YES, PARTIAL, NO

### Files Created
- `crates/runtime/src/dspy_tools.rs` - 3 tool signatures + enum

---

## Wave 11: Optimization Infrastructure - COMPLETE

*Implemented in `crates/autopilot-core/`*

Production-ready optimization pipeline with module storage, training data extraction, and A/B routing.

### Implemented Components

1. **DspyHub** (`dspy_hub.rs`)
   - Pre-optimized modules stored in `~/.openagents/dspy/optimized/`
   - Version modules with compiled_id (SHA-256 hash)
   - Save/load modules with demos
   - Query promoted modules by signature name
   - A/B routing support (Promoted, Shadow, ABTest strategies)

2. **TrainingExtractor** (`dspy_training.rs`)
   - Extract examples from successful autopilot sessions
   - Parse JSONL session logs from `~/.openagents/sessions/`
   - Store training data in `~/.openagents/dspy/training/`
   - Extract planning, execution, verification, and tool examples
   - Success criteria filtering

3. **SignatureRouter** (`dspy_router.rs`)
   - Route by compiled_id or strategy
   - Shadow mode comparison tracking
   - A/B testing with configurable percentages
   - Shadow statistics (win rate, samples, duration)
   - Promotion decision support

### Exports

```rust
pub use dspy_hub::{DspyHub, RoutingStrategy, StoredModule};
pub use dspy_router::{RoutingDecision, RoutingSummary, ShadowStats, SignatureRouter};
pub use dspy_training::{ExtractedExamples, SavedTrainingPaths, SuccessCriteria, TrainingExtractor};
```

### Files Created
- `crates/autopilot-core/src/dspy_hub.rs` - Module storage hub
- `crates/autopilot-core/src/dspy_training.rs` - Training data extraction
- `crates/autopilot-core/src/dspy_router.rs` - A/B routing and shadow mode

---

## Wave 12: FRLM Integration - COMPLETE

*Implemented in `crates/frlm/`*

FRLM-specific signatures for the flagship RLM narrative. Map-reduce pattern for distributed document processing. All signatures implement `MetaSignature` trait manually for public API compatibility.

### Implemented Signatures

| Signature | File | Purpose |
|-----------|------|---------|
| `FRLMDecomposeSignature` | `dspy_signatures.rs` | Root decides what subcalls to spawn (map phase) |
| `FRLMAggregateSignature` | `dspy_signatures.rs` | Merge worker results into final answer (reduce phase) |

### Helper Enums

- `StoppingRule`: EXHAUSTIVE, SUFFICIENT_EVIDENCE, BUDGET_EXHAUSTED, CONFIDENCE_THRESHOLD
- `SpanSelector`: All, ByType(String), ByRelevance, ByPosition { start, end }

### FRLMDecomposeSignature
```rust
pub struct FRLMDecomposeSignature {
    instruction: String,
    demos: Vec<Example>,
}

// Inputs: query, env_summary, progress
// Outputs: subqueries (JSON array), stopping_rule
```

### FRLMAggregateSignature
```rust
pub struct FRLMAggregateSignature {
    instruction: String,
    demos: Vec<Example>,
}

// Inputs: query, worker_results
// Outputs: answer, citations, confidence
```

### Files Created
- `crates/frlm/src/dspy_signatures.rs` - 2 FRLM signatures + 2 enums

---

## Trace Taxonomy for HUD + Optimization

### Core Span Types

| Span Type | Attributes |
|-----------|------------|
| `session` | user_id_hash, repo_id, mode (local/swarm/dc/hybrid) |
| `turn` | prompt_chars, budget_msats, target_slo_ms |
| `module` | module_name, compiled_id, model_lane |
| `retrieval` | lane (grep/semantic/lsp/git), k, candidates_returned, latency_ms |
| `swarm.dispatch` | job_type, fanout, max_msats, fanout_plan_id |
| `swarm.job` | job_id, job_hash, provider_pubkey, provider_quote_msats, provider_quote_expiry_ms, settlement_msats, latency_ms, status |
| `patch.apply` | files_touched, diff_bytes, diff_lines_added/removed |
| `verify` | strategy (fast/full/matrix), sandbox_profile, pass_fail |

### OTel-Compatible Fields (optional)

- `span_kind`: `"client" | "server" | "internal"`
- `resource`: `{ "service.name": "...", "service.version": "...", "host.arch": "...", ... }`

### Trace Record Format

```json
{
  "trace_id": "t-123",
  "span_id": "s-200",
  "parent_span_id": "s-100",
  "name": "swarm.job",
  "span_kind": "client",
  "start_ms": 1736410001000,
  "end_ms": 1736410004200,
  "status": "ok",
  "attrs": {
    "job_type": "oa.code_chunk_analysis.v1",
    "job_id": "1a2b...",
    "job_hash": "sha256:...",
    "provider_pubkey": "npub1...",
    "provider_quote_msats": 2500,
    "provider_quote_expiry_ms": 60000,
    "settlement_msats": 2100
  }
}
```

### Result Provenance (for replay/verify)

Every job output includes:
- `model_id`
- `sampling`: {temperature, top_p, seed?}
- `input_sha256`
- `output_sha256`

---

## Optimization Strategy

1. **Collect Training Data**
   - From successful autopilot sessions
   - From manual corrections
   - From user feedback

2. **Start with MIPROv2**
   - Instruction optimization first
   - Low computational cost
   - Good baseline improvements

3. **Graduate to GEPA**
   - For complex signatures
   - When MIPROv2 plateaus
   - Higher computational cost

4. **What to Compile First**
   - RetrievalRouter policy
   - EvidenceRanker rubric
   - FailureInterpreter
   - Then PatchPlanner, PatchWriter

5. **Store Optimized Modules**
   - In `~/.openagents/dspy/optimized/`
   - Version with compiled_id (hash)
   - Include scorecard (proxy + truth metrics)

6. **Promotion Gates**
   - Regression suite pass
   - Budget cap sanity
   - Shadow mode comparison
   - A/B test before full rollout

---

## Architecture

```
┌────────────────────────────────────────────────────────────────────┐
│                        OPENAGENTS + DSRS                            │
├────────────────────────────────────────────────────────────────────┤
│  PROTOCOL LAYER (Wave 0)                                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │ Job Schemas  │  │  Canonical   │  │  Versioning  │              │
│  │  (JSON)      │  │   Hashing    │  │    Rules     │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│           │                │                │                       │
│           └────────────────┴────────────────┘                       │
│                            │                                        │
│  COMPILER LAYER (dsrs)                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │  Signatures  │  │  Optimizers  │  │   Refine     │              │
│  │  (macros)    │  │  MIPROv2     │  │  (verify)    │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│           │                │                │                       │
│           └────────────────┴────────────────┘                       │
│                            │                                        │
│  INTEGRATION LAYER         │                                        │
│  ┌─────────────────────────┴─────────────────────────────────────┐ │
│  │  PylonInference (LM)  │  PylonSandbox (tests)                 │ │
│  │  NostrBridge (DAG ↔ events)  │  TraceExtractor (examples)     │ │
│  │  HudCallback (streaming)     │  CompiledManifest (versioning) │ │
│  │  RepoIndex (retrieval)       │  Verification (obj/subj)       │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                            │                                        │
│  EXECUTION LAYER (OpenAgents)                                       │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │    Pylon     │  │    Nexus     │  │   Runtime    │              │
│  │  (compute)   │  │  (relay)     │  │  (HUD/trace) │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
└────────────────────────────────────────────────────────────────────┘
```

---

## The Flywheel

```
┌─────────────────────────────────────────────────────────────────┐
│  SUCCESSFUL SESSION                                              │
│  User runs autopilot → Task succeeds → DAG trace recorded        │
├─────────────────────────────────────────────────────────────────┤
│  TRAINING EXTRACTION                                             │
│  TraceExtractor parses JSONL → dsrs Example objects              │
├─────────────────────────────────────────────────────────────────┤
│  COMPILATION (cheap on Pylon swarm)                              │
│  SwarmCompiler → MIPROv2 → Pylon providers (10 msats/call)       │
│  HudCallback streams progress → DAG → Nostr events               │
├─────────────────────────────────────────────────────────────────┤
│  VALIDATION (Refine pattern + Promotion Gates)                   │
│  Multiple rollouts → reward eval → feedback hints → retry        │
│  Premium model for final validation                              │
│  Shadow mode comparison → Regression suite → A/B test            │
├─────────────────────────────────────────────────────────────────┤
│  IMPROVED AGENTS                                                 │
│  Optimized signatures have better prompts/demos                  │
│  More successful sessions → More training data → Loop continues  │
└─────────────────────────────────────────────────────────────────┘
```

**The flywheel**: successful sessions generate training data → dsrs optimization (cheap on swarm) → better prompts/demos → higher success rates → more training data.

---

## Model Mixing Strategy

Different signatures benefit from different models:

| Signature Type | Recommended Model | Reasoning |
|----------------|-------------------|-----------|
| Planning (Deep) | Codex Opus | Complex reasoning needed |
| Planning (Simple) | Codex Sonnet | Balance of speed/quality |
| Execution | Codex Sonnet | Balance of speed/quality |
| Review/Verify | Codex Haiku | Fast validation |
| Optimization (iterations) | Pylon Swarm | Cheap, high volume |
| Optimization (validation) | Codex/GPT-4 | Final quality check |
| OANIX Situation | Local (Ollama) | Privacy, always-on |
| Tool Selection | Any fast model | Simple classification |
| Oracle Query | Codex Sonnet | Good at synthesis |
| Architecture | Codex Opus | Needs deep reasoning |
| Retrieval Router | Pylon Local | Fast, cheap policy |
| Evidence Ranker | Pylon Swarm | Parallelizable |
| Failure Triage | Codex Sonnet | Needs reasoning |
| State/Memory | Pylon Local | Fast, always-on |

---

## Success Metrics

For each signature, track:
- **Task Success Rate**: Did the signature lead to successful outcomes?
- **Confidence Calibration**: Does confidence match actual success?
- **Latency**: How long does inference take?
- **Token Usage**: How many tokens per call?
- **Cost**: Sats spent per optimization run
- **User Corrections**: How often do users override?
- **Retrieval Recall@K**: Did we find the right files?
- **Diff Size**: How minimal is the patch?
- **Sandbox Runs**: How many verification attempts?
- **Evidence Efficiency**: Bytes opened / bytes budget
- **Rollout Fail Rate**: p_fail across multiple rollouts

---

## Implementation Priority

### Week 1: Swarm Evidence Workers
- Implement `oa.code_chunk_analysis.v1` job type end-to-end
- Root agent selects top-K chunks → dispatches to swarm → aggregates summaries
- Show in HUD

### Week 2: Swarm Verifier Matrix
- Implement `oa.sandbox_run.v1` job type
- Root dispatches: lint, unit tests, typecheck
- Show "test matrix" panel in HUD (green/red tiles)

### Week 3: DSPy Compilation Loop (offline)
- Take 30-100 real issues from OSS repos
- Define metric: tests pass + minimal diff + time
- Compile: retrieval router, chunking policy, failure interpreter

---

## References

### Strategy & Philosophy
- **[DSPy Strategy Overview](./dspy/README.md)** — Synthesized strategy document (start here)
- [Omar Khattab: State of DSPy](./transcripts/dspy/state-of-dspy.md) — Philosophy talk transcript
- [Kevin Madura: DSPy is All You Need](./transcripts/dspy/dspy-is-all-you-need.md) — Practical tutorial transcript

### Implementation
- [Signatures Catalog](./dspy/signatures-catalog.md) — Complete inventory of all DSPy signatures
- [Integration Guide](./dspy/integration-guide.md) — How to add DSPy to new components
- [dsrs Crate](../crates/dsrs/) — Rust DSPy implementation (5,771 LOC)
- [dsrs Documentation](../crates/dsrs/docs/) — Architecture, signatures, retrieval, evaluation
- [Adjutant DSPy Integration](../crates/adjutant/docs/DSPY-INTEGRATION.md) — Self-improvement system

### Concepts
- [DSPy + RLM Integration](./dspy/rlm.md) — RLM concepts
- [dsrs Usage Guide](./dspy/rust.md) — Rust implementation guide

### External
- [DSPy Documentation](https://dspy.ai) — Official DSPy docs
