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

---

## Current State

### dsrs Integration (Complete)

dsrs (Rust DSPy) is now integrated into the OpenAgents workspace at `crates/dsrs/`:

- **5,771 LOC** full implementation
- **Optimizers**: COPRO, MIPROv2, GEPA, Pareto
- **DAG-based tracing** with Graph/Node types
- **12+ LM providers** via rig-core (OpenAI, Anthropic, Gemini, Groq, Ollama, etc.)
- **Trait-based architecture**: Module, Predictor, MetaSignature, Adapter, Optimizable, Evaluator
- **Macros**: `#[Signature]`, `#[Optimizable]` for code generation
- **Hybrid caching** via foyer (memory + disk)

### Wave 1: RLM Document Analysis (Complete)
- [x] `crates/rlm/src/dspy_orchestrator.rs` - 4-phase document pipeline
- [x] `crates/rlm/src/signatures.rs` - SpanRef-based evidence tracking
- [x] `crates/rlm/src/dspy_bridge.rs` - LM configuration and cost tracking

### Wave 2: Autopilot Signatures (Complete)
- [x] `crates/autopilot/src/dspy_planning.rs` - PlanningSignature + DeepPlanningSignature
- [x] `crates/autopilot/src/dspy_execution.rs` - ExecutionStrategySignature + ToolSelectionSignature
- [x] `crates/autopilot/src/dspy_verify.rs` - Verification + ExecutionReviewSignature
- [x] `crates/autopilot/src/dspy_optimization.rs` - Metrics + training data infrastructure

---

## Wave 3: OpenAgents Integration Layer

Bridge dsrs with OpenAgents infrastructure for distributed optimization.

### 3.1 Pylon LM Provider
**File:** `crates/dsrs/src/adapter/pylon.rs` (NEW)

Add Pylon as an LM provider in dsrs, enabling swarm-backed inference:

```rust
pub struct PylonClient {
    relay_url: String,
    pubkey: XOnlyPublicKey,
    budget_sats: u64,
}

impl CompletionProvider for PylonClient {
    async fn completion(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        // Submit NIP-90 job to Nexus relay
        // Wait for provider response
        // Pay Lightning invoice
        // Return result
    }
}
```

### 3.2 DAG → Nostr Bridge
**File:** `crates/dsrs/src/trace/nostr_bridge.rs` (NEW)

Convert dsrs execution graphs to Nostr events for distributed tracing:

```rust
pub fn graph_to_nostr_events(graph: &Graph) -> Vec<NostrEvent> {
    graph.nodes.iter().map(|node| {
        match &node.node_type {
            NodeType::Root => kind_1_text_note(node),
            NodeType::Predict { .. } => kind_5050_job(node),
            NodeType::Operator { .. } => kind_1_with_tags(node),
            NodeType::Map { .. } => kind_1_lineage(node),
        }
    }).collect()
}
```

### 3.3 Callback System for HUD
**File:** `crates/dsrs/src/callbacks.rs` (NEW)

Port Python DSPy's callback system for real-time HUD integration:

```rust
pub trait DspyCallback: Send + Sync {
    fn on_module_start(&self, call_id: Uuid, instance: &dyn Module, inputs: &Example);
    fn on_module_end(&self, call_id: Uuid, outputs: Result<&Prediction>, exception: Option<&Error>);
    fn on_lm_start(&self, call_id: Uuid, instance: &LM, inputs: &Chat);
    fn on_lm_end(&self, call_id: Uuid, outputs: Result<&Message>, exception: Option<&Error>);
    fn on_optimizer_candidate(&self, candidate_id: String, metrics: HashMap<String, f32>);
}
```

### 3.4 Refine Pattern for Verification
**File:** `crates/dsrs/src/predictors/refine.rs` (NEW)

Port Python DSPy's Refine module (replaced assertions):

```rust
pub struct Refine<M: Module> {
    module: M,
    n_rollouts: usize,
    reward_fn: Box<dyn Fn(&Example, &Prediction) -> f32>,
    threshold: f32,
}
```

Multiple rollouts → reward evaluation → feedback hints → retry with improved context.

---

## Wave 4: Swarm-Backed Optimization

### 4.1 SwarmCompiler
**File:** `crates/autopilot/src/dspy_compiler.rs` (NEW)

Use Pylon swarm for cheap optimization iterations:

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

### 4.2 TraceExtractor
**File:** `crates/autopilot/src/trace_extraction.rs` (NEW)

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

## Wave 5: OANIX (Agent OS Runtime)

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

## Wave 6: Agent Orchestrator

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

## Wave 7: Tool Invocation

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

## Wave 8: Optimization Infrastructure

Production-ready optimization pipeline.

### Components

1. **DSPy Hub for OpenAgents**
   - Pre-optimized modules stored in `~/.openagents/dspy/optimized/`
   - Version modules with session hash
   - Share optimized modules across machines

2. **Automated Training Data Collection**
   - Extract examples from successful autopilot sessions
   - Store in `~/.openagents/dspy/training/`
   - Format: JSONL with inputs and expected outputs

3. **CI/CD for Signature Optimization**
   - Nightly optimization runs
   - Track optimization metrics over time
   - Automated regression testing

4. **A/B Testing Framework**
   - Compare optimized vs base signatures
   - Track success rates by signature
   - Gradual rollout of optimized versions

**Files to Create:**
- `crates/autopilot/src/dspy_hub.rs`
- `crates/autopilot/src/dspy_training.rs`
- `scripts/optimize_signatures.rs`

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

4. **Store Optimized Modules**
   - In `~/.openagents/dspy/optimized/`
   - Version with session hash
   - Include optimization metrics

5. **Version Optimized Modules**
   - Track which sessions used which version
   - Enable rollback if needed
   - Compare versions over time

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
│  │  PylonClient (LM provider)  │  NostrBridge (DAG ↔ events)     │ │
│  │  HudCallback (streaming)    │  TraceExtractor (training data) │ │
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
│  VALIDATION (Refine pattern)                                     │
│  Multiple rollouts → reward eval → feedback hints → retry        │
│  Premium model for final validation                              │
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

---

## Success Metrics

For each signature, track:
- **Task Success Rate**: Did the signature lead to successful outcomes?
- **Confidence Calibration**: Does confidence match actual success?
- **Latency**: How long does inference take?
- **Token Usage**: How many tokens per call?
- **Cost**: Sats spent per optimization run
- **User Corrections**: How often do users override?

---

## References

- [DSPy Documentation](https://dspy.ai)
- [dsrs Crate](crates/dsrs/) - Rust DSPy implementation
- [Omar Khattab: State of DSPy](docs/transcripts/dspy/state-of-dspy.md)
- [Kevin Madura: DSPy is All You Need](docs/transcripts/dspy/dspy-is-all-you-need.md)
