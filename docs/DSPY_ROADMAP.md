# DSPy Integration Roadmap for OpenAgents

## Core Insight: DSPy as Compiler Layer

**DSPy is the compiler layer for agent behavior.**

Not the market layer, not the runtime. DSPy provides: **take a goal ("solve this class of tasks") and automatically discover the best prompt + tool-use structure + few-shot examples + scoring loop**, instead of hand-tuning prompts forever.

**The Separation:**
- **DSPy (dsrs)** decides *what to do* (best program structure/prompting)
- **OpenAgents** decides *where/how it runs* (local/swarm/datacenter), *whether it's valid* (verification), *what it costs* (budgets/receipts), and *why it worked* (trace/HUD)

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
- **14+ LM providers** via rig-core (OpenAI, Anthropic, Gemini, Groq, Ollama, Pylon, Claude SDK, etc.)
- **Trait-based architecture**: Module, Predictor, MetaSignature, Adapter, Optimizable, Evaluator
- **Macros**: `#[Signature]`, `#[Optimizable]` for code generation
- **Hybrid caching** via foyer (memory + disk)
- **Multi-provider LM support**: Claude SDK (headless) → Pylon swarm → Cerebras → Pylon local

### Wave 1: RLM Document Analysis (Complete)
- [x] `crates/rlm/src/dspy_orchestrator.rs` - 4-phase document pipeline
- [x] `crates/rlm/src/signatures.rs` - SpanRef-based evidence tracking
- [x] `crates/rlm/src/dspy_bridge.rs` - LM configuration and cost tracking

### Wave 2: Autopilot Signatures (Complete)
- [x] `crates/autopilot/src/dspy_planning.rs` - PlanningSignature + DeepPlanningSignature
- [x] `crates/autopilot/src/dspy_execution.rs` - ExecutionStrategySignature + ToolSelectionSignature
- [x] `crates/autopilot/src/dspy_verify.rs` - Verification + ExecutionReviewSignature
- [x] `crates/autopilot/src/dspy_optimization.rs` - Metrics + training data infrastructure

### Wave 2.5: Adjutant Multi-Provider LM (Complete)
- [x] `crates/dsrs/src/core/lm/claude_sdk.rs` - Claude Code headless via claude-agent-sdk
- [x] `crates/dsrs/src/core/lm/pylon.rs` - Pylon LM provider (local/swarm/hybrid)
- [x] `crates/adjutant/src/dspy/lm_config.rs` - Multi-provider with auto-detection

---

## The Cursor-Grade Agent Module Graph

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
```

#### TraceContract (dsrs DAG → OA spans)
Map dsrs execution nodes to OpenAgents trace spans:
```rust
pub fn graph_to_spans(graph: &Graph) -> Vec<OASpan> {
    // session → turn → module → retrieval → swarm.job → patch → verify
}
```

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

## Wave 4: Swarm Job Types & Retrieval Policy

### 4.1 Canonical Swarm Job Types

These become the "map-reduce primitives" DSPy compiles against:

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
  "task": "bug_hypothesis",
  "user_task": "Fix failing tests for empty prompt handling",
  "chunk": { "path": "src/service.py", "start_line": 1, "end_line": 160, "content": "..." },
  "output_constraints": { "max_summary_chars": 1500, "max_findings": 6 }
}
```

**Response:**
```json
{
  "summary": "...",
  "symbols": [{"name": "SuggestionService", "kind": "class", "line": 120}],
  "suspected_faults": [{"line": 244, "reason": "..."}],
  "recommended_next_probes": [{"lane": "lsp_refs", "query": "normalize_prompt"}],
  "confidence": 0.85
}
```

#### `oa.retrieval_rerank.v1`
LLM-based reranking of candidate set:

**Request:**
```json
{
  "user_task": "...",
  "candidates": [{"candidate_id": "c1", "lane": "semantic", "path": "...", "snippet": "..."}],
  "k": 10,
  "ranking_rubric": { "prefer_fix_surface": true }
}
```

**Response:**
```json
{
  "topk": [{"rank": 1, "candidate_id": "c1", "score": 0.92, "why": "..."}]
}
```

#### `oa.sandbox_run.v1`
Concurrent sandbox execution (builds/tests/lints):

**Request:**
```json
{
  "sandbox": { "provider": "daytona", "resources": {"vcpus": 2, "memory_mb": 4096} },
  "repo": { "fetch": {"kind": "git", "git_url": "...", "git_commit": "..."} },
  "commands": [{"name": "test", "cmd": "pytest -q", "timeout_ms": 300000}]
}
```

**Response:**
```json
{
  "runs": [{"name": "test", "exit_code": 0, "duration_ms": 12000}],
  "overall": { "status": "pass" }
}
```

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

**Files to Create:**
- `crates/dsrs/src/signatures/retrieval_policy.rs`
- `crates/dsrs/src/signatures/chunk_analysis.rs`
- `crates/dsrs/src/signatures/sandbox.rs`
- `crates/dsrs/src/signatures/budgeting.rs`

---

## Wave 5: Swarm-Backed Optimization

### 5.1 SwarmCompiler
**File:** `crates/autopilot/src/dspy_compiler.rs`

```rust
pub struct SwarmCompiler {
    swarm_lm: LM,       // Pylon provider (cheap)
    validation_lm: LM,  // Claude/GPT-4 (premium)
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
- Claude alone: ~$15 for 1,800 calls
- With swarm optimization: ~$0.10 (96.7% reduction)

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

### 5.3 Scoring Function

```
score =
  1.0 * pass_tests
  - 0.25 * min(1, total_cost_msats / budget_msats)
  - 0.15 * min(1, total_time_ms / time_budget_ms)
  - 0.10 * min(1, diff_lines_changed / diff_budget_lines)
  - 0.10 * min(1, sandbox_runs / sandbox_budget_runs)
```

This makes the optimizer learn "don't brute force."

### 5.4 TraceExtractor
**File:** `crates/autopilot/src/trace_extraction.rs`

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

## Wave 6: Eval Harness & Promotion Gates

### 6.1 Eval Task Format

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

### 6.2 Promotion Gates (NEW)

A compiled policy should not roll out globally unless it passes:

1. **Regression suite** - All existing tasks still pass
2. **Budget cap sanity** - No runaway cost
3. **Failure mode checks** - Timeouts, runaway loops

**Policy Registry:**
```
candidate → staged → promoted → rolled_back
```

A/B routing by compiled_id in production.

### 6.3 Compile Order

Start compiling **only policies**, not everything:

1. **RetrievalRouter policy** - How much semantic vs grep vs LSP, K, chunking, dedupe
2. **EvidenceRanker rubric** - Choose best 10 chunks to analyze
3. **FailureInterpreter** - What to do after failed test run (where most agents waste money)

Then:
4. PatchPlanner style / constraints
5. PatchWriter strategies (multi-candidate vs single)

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

## Wave 8: OANIX (Agent OS Runtime)

Transform OANIX's rule-based decision making into DSPy signatures.

### Signatures

```rust
#[Signature]
struct SituationAssessmentSignature {
    /// Situation Assessment: Analyze current system state and determine priorities.

    #[input] system_state: String,        // Current process states, resources
    #[input] pending_events: String,      // Events in queue
    #[input] recent_history: String,      // Last N decisions and outcomes

    #[output] priority_action: String,    // What to do next
    #[output] urgency: String,            // IMMEDIATE, NORMAL, DEFERRED
    #[output] reasoning: String,          // Why this action
    #[output] confidence: f32,
}

#[Signature]
struct IssueSelectionSignature {
    /// Issue Selection: Choose the best issue to work on from available options.

    #[input] available_issues: String,    // JSON array of issues
    #[input] agent_capabilities: String,  // What this agent can do
    #[input] current_context: String,     // Repository state, recent work

    #[output] selected_issue: String,     // Issue ID or number
    #[output] rationale: String,          // Why this issue
    #[output] estimated_complexity: String,
    #[output] confidence: f32,
}

#[Signature]
struct WorkPrioritizationSignature {
    /// Work Prioritization: Order tasks by importance and dependencies.

    #[input] task_list: String,           // JSON array of tasks
    #[input] dependencies: String,        // Task dependency graph
    #[input] deadlines: String,           // Any time constraints

    #[output] ordered_tasks: String,      // JSON array, priority order
    #[output] blocking_tasks: String,     // Tasks blocking others
    #[output] parallel_groups: String,    // Tasks that can run together
}

#[Signature(cot)]
struct LifecycleDecisionSignature {
    /// Lifecycle Decision: Determine agent state transitions.
    /// Use chain-of-thought to reason about state changes.

    #[input] current_state: String,       // Agent's current lifecycle state
    #[input] recent_events: String,       // What just happened
    #[input] resource_status: String,     // Memory, CPU, network

    #[output] next_state: String,         // IDLE, WORKING, BLOCKED, TERMINATING
    #[output] transition_reason: String,  // Why transition
    #[output] cleanup_needed: String,     // Any cleanup before transition
}
```

**Files to Create:**
- `crates/oanix/src/dspy_situation.rs`
- `crates/oanix/src/dspy_lifecycle.rs`

---

## Wave 9: Agent Orchestrator

Convert the 7 specialized agent prompts into DSPy Signatures.

### Current Agents (in `agent-orchestrator`)
1. **Sisyphus** - Master orchestrator
2. **Oracle** - Information retrieval
3. **Architect** - System design
4. **Coder** - Implementation
5. **Reviewer** - Code review
6. **DevOps** - Deployment
7. **Documenter** - Documentation

### Signatures

```rust
#[Signature]
struct DelegationSignature {
    /// Delegation: Sisyphus decides which sub-agent should handle a task.

    #[input] task_description: String,
    #[input] available_agents: String,    // JSON with agent capabilities
    #[input] current_workload: String,    // What each agent is doing

    #[output] assigned_agent: String,
    #[output] task_refinement: String,    // Refined instructions for agent
    #[output] expected_deliverables: String,
    #[output] fallback_agent: String,
}

#[Signature]
struct OracleQuerySignature {
    /// Oracle: Find and synthesize information from various sources.

    #[input] query: String,
    #[input] search_scope: String,        // Codebase, docs, web
    #[input] relevance_criteria: String,

    #[output] findings: String,           // JSON array of findings
    #[output] sources: String,            // Where info came from
    #[output] confidence: f32,
    #[output] gaps: String,               // What couldn't be found
}

#[Signature(cot)]
struct ArchitectureSignature {
    /// Architecture: Design system changes with careful reasoning.

    #[input] requirements: String,
    #[input] existing_architecture: String,
    #[input] constraints: String,

    #[output] proposed_changes: String,
    #[output] component_diagram: String,
    #[output] migration_path: String,
    #[output] risks: String,
}

// Similar signatures for Coder, Reviewer, DevOps, Documenter...
```

**Files to Create:**
- `crates/agent-orchestrator/src/dspy_delegation.rs`
- `crates/agent-orchestrator/src/dspy_agents.rs`

---

## Wave 10: Tool Invocation

Universal tool selection and interpretation layer.

### Signatures

```rust
#[Signature]
struct ToolSelectionSignature {
    /// Tool Selection: Choose the right tool for any task.

    #[input] task_description: String,
    #[input] available_tools: String,     // JSON tool definitions
    #[input] context: String,             // Recent tool results

    #[output] selected_tool: String,
    #[output] tool_params: String,        // JSON params
    #[output] expected_outcome: String,
    #[output] fallback_tool: String,
}

#[Signature]
struct ToolResultInterpretationSignature {
    /// Result Interpretation: Understand what a tool result means.

    #[input] tool_name: String,
    #[input] tool_output: String,
    #[input] original_intent: String,

    #[output] success: String,            // YES, PARTIAL, NO
    #[output] extracted_info: String,     // Key information from output
    #[output] next_steps: String,         // What to do next
    #[output] error_analysis: String,     // If failed, why
}

#[Signature]
struct ToolChainPlanningSignature {
    /// Tool Chain: Plan multi-tool sequences for complex tasks.

    #[input] goal: String,
    #[input] available_tools: String,
    #[input] constraints: String,

    #[output] tool_sequence: String,      // JSON array of tool calls
    #[output] dependencies: String,       // Which calls depend on others
    #[output] parallelizable: String,     // Which can run in parallel
}
```

**Files to Create:**
- `crates/openagents-runtime/src/dspy_tools.rs`

---

## Wave 11: Optimization Infrastructure

Production-ready optimization pipeline.

### Components

1. **DSPy Hub for OpenAgents**
   - Pre-optimized modules stored in `~/.openagents/dspy/optimized/`
   - Version modules with compiled_id (hash)
   - Share optimized modules across machines
   - Include optimization scorecard

2. **Automated Training Data Collection**
   - Extract examples from successful autopilot sessions
   - Store in `~/.openagents/dspy/training/`
   - Format: JSONL with inputs and expected outputs

3. **CI/CD for Signature Optimization**
   - Nightly optimization runs
   - Track optimization metrics over time
   - Automated regression testing

4. **A/B Testing Framework**
   - Route by compiled_id
   - Compare optimized vs base signatures
   - Track success rates by signature
   - Gradual rollout of optimized versions
   - Support rollback

**Files to Create:**
- `crates/autopilot/src/dspy_hub.rs`
- `crates/autopilot/src/dspy_training.rs`
- `scripts/optimize_signatures.rs`

---

## Wave 12: FRLM Integration

FRLM-specific signatures for the flagship RLM narrative.

### FRLMDecomposeSignature
```rust
#[Signature]
struct FRLMDecomposeSignature {
    /// Root decides what subcalls to spawn over which spans.

    #[input] query: String,
    #[input] env_summary: String,
    #[input] progress: String,

    #[output] subqueries: String,         // JSON: [{span_selector, question, schema}]
    #[output] stopping_rule: String,
}
```

### FRLMAggregateSignature
```rust
#[Signature]
struct FRLMAggregateSignature {
    /// Reduce step: merge worker results into final answer.

    #[input] query: String,
    #[input] worker_results: String,

    #[output] answer: String,
    #[output] citations: String,          // SpanRefs or doc ids
    #[output] confidence: f32,
}
```

---

## Trace Taxonomy for HUD + Optimization

### Core Span Types

| Span Type | Attributes |
|-----------|------------|
| `session` | user_id_hash, repo_id, mode (local/swarm/dc/hybrid) |
| `turn` | prompt_chars, budget_msats, target_slo_ms |
| `module` | module_name, compiled_id, model_lane |
| `retrieval` | lane (grep/semantic/lsp/git), k, candidates_returned, latency_ms |
| `swarm.dispatch` | job_type, fanout, max_msats |
| `swarm.job` | job_id, provider_pubkey, charged_msats, latency_ms, status |
| `patch.apply` | files_touched, diff_bytes, diff_lines_added/removed |
| `verify` | strategy (fast/full/matrix), sandbox_profile, pass_fail |

### Trace Record Format

```json
{
  "trace_id": "t-123",
  "span_id": "s-200",
  "parent_span_id": "s-100",
  "name": "swarm.job",
  "start_ms": 1736410001000,
  "end_ms": 1736410004200,
  "status": "ok",
  "attrs": {
    "job_type": "oa.code_chunk_analysis.v1",
    "job_id": "1a2b...",
    "provider_pubkey": "npub1...",
    "charged_msats": 2100
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
   - A/B test before full rollout

---

## Architecture

```
┌────────────────────────────────────────────────────────────────────┐
│                        OPENAGENTS + DSRS                            │
├────────────────────────────────────────────────────────────────────┤
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
│  Regression suite + A/B test before promotion                    │
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
| Planning (Deep) | Claude Opus | Complex reasoning needed |
| Planning (Simple) | Claude Sonnet | Balance of speed/quality |
| Execution | Claude Sonnet | Balance of speed/quality |
| Review/Verify | Claude Haiku | Fast validation |
| Optimization (iterations) | Pylon Swarm | Cheap, high volume |
| Optimization (validation) | Claude/GPT-4 | Final quality check |
| OANIX Situation | Local (Ollama) | Privacy, always-on |
| Tool Selection | Any fast model | Simple classification |
| Oracle Query | Claude Sonnet | Good at synthesis |
| Architecture | Claude Opus | Needs deep reasoning |
| Retrieval Router | Pylon Local | Fast, cheap policy |
| Evidence Ranker | Pylon Swarm | Parallelizable |
| Failure Triage | Claude Sonnet | Needs reasoning |

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

- [DSPy Documentation](https://dspy.ai)
- [dsrs Crate](crates/dsrs/) - Rust DSPy implementation
- [Omar Khattab: State of DSPy](docs/transcripts/dspy/state-of-dspy.md)
- [Kevin Madura: DSPy is All You Need](docs/transcripts/dspy/dspy-is-all-you-need.md)
