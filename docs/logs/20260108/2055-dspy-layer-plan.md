# DSPy as the Compiler Layer for OpenAgents

## The Core Insight

**DSPy is the compiler layer for agent behavior.**

Not the market layer, not the runtime. DSPy provides: **take a goal ("solve this class of tasks") and automatically discover the best prompt + tool-use structure + few-shot examples + scoring loop**, instead of hand-tuning prompts forever.

**The Separation:**
- **DSPy** decides *what to do* (best program structure/prompting)
- **OpenAgents** decides *where/how it runs* (local/swarm/datacenter), *whether it's valid* (verification), *what it costs* (budgets/receipts), and *why it worked* (trace/HUD)

---

## Critical Discovery: dsrs Already Exists

**Location:** `/Users/christopherdavid/code/dsrs/`

A **5,771 LOC Rust DSPy implementation** already exists with:
- **Same `rig` library** that OpenAgents uses for LM abstraction
- **4 optimizers**: COPRO, MIPROv2, GEPA, Pareto
- **DAG-based tracing** with Graph/Node types
- **12+ LM providers**: OpenAI, Anthropic, Gemini, Groq, Ollama, Azure, etc.
- **Trait-based architecture**: Module, Predictor, MetaSignature, Adapter, Optimizable, Evaluator
- **Macros**: `#[Signature]`, `#[Optimizable]` for code generation
- **Hybrid caching**: Memory + disk via `foyer`

**This changes the implementation strategy fundamentally.** Instead of building DSPy integration from scratch, we integrate dsrs into OpenAgents and add Pylon as another LM provider.

## The Four Integration Points

### 1. Autopilot = Trainable Programs (not just signatures)

Autopilot today is a pile of prompts, tool policies, routing rules, and evaluation heuristics.

DSPy turns these into *programs* with trainable pieces:
```
Plan → Retrieve → Write Patch → Run Tests → Revise
  ↓        ↓           ↓            ↓          ↓
[module] [module]   [module]    [verifier]  [module]
```

Each module has optimizable prompts/demos. You optimize against your real task distribution (issues, PRs, test outcomes).

**Result:** Fewer brittle prompt edits, more measurable iteration.

### 2. Skills Marketplace = DSPy Programs as Packaged Skills

A skill is:
- A DSPy program + its learned parameters (prompt templates, demos, selectors)
- Plus a verifier (tests, linters, human review)
- Plus measured performance stats and cost profile

The marketplace sells:
- The executable skill
- Its measured performance (accuracy, latency, cost)
- Its verification requirements

**This is a clean packaging format for "agent expertise."**

### 3. Compute Network Makes DSPy Practical at Scale

DSPy's pain point: optimization requires lots of LLM calls.

OpenAgents has:
- Cheap local/swarm inference (Ollama, Apple FM, GPT-OSS 20B)
- Premium lane for hard steps (Claude, Crusoe 120B)
- Objective verifiers (SandboxRun, test execution)

**The optimization loop:**
```
┌─────────────────────────────────────────────────────────────┐
│  DSPy Compilation (cheap)                                    │
│  Generate prompt candidates → Run on swarm (10 msats/call)   │
│  Score against training data → Iterate 1000s of times        │
│  Select best prompts                                         │
├─────────────────────────────────────────────────────────────┤
│  Validation (moderate)                                       │
│  Run 3-of-5 redundancy on better models (50 msats/call)      │
│  Measure accuracy/latency/cost tradeoffs                     │
├─────────────────────────────────────────────────────────────┤
│  Verification (premium, minimal volume)                      │
│  Critical queries → Claude/GPT-4                             │
│  Only run on subset that failed redundancy check             │
└─────────────────────────────────────────────────────────────┘

Cost example: 1,800 LLM calls
  - Claude alone: ~$15
  - With swarm optimization: ~$0.10 (96.7% reduction)
```

### 4. DSPy Outputs as First-Class Trace Artifacts

DSPy normally produces: metrics, prompt variants, datasets.

In OpenAgents, each candidate program run becomes:
- A replayable trace (JSONL → ReplayBundle)
- With costs, tool calls, failures, verifications
- Diffable across candidates
- Queryable for A/B analysis

**This makes DSPy optimization debuggable instead of magical.**

---

## Current State

**OpenAgents (Wave 2 Complete):**
- 12 signatures across RLM, Autopilot Planning/Execution, Verification
- Metrics for planning, execution, verification quality
- Training data structures (PlanningExample, ExecutionExample, VerificationExample)
- Skills system with NIP-SA standard

**dsrs (External Rust DSPy):**
- Full Module/Predictor/Signature implementation
- COPRO, MIPROv2, GEPA, Pareto optimizers
- DAG tracing with Graph/Node types
- Uses `rig` for LM abstraction (same as OpenAgents!)
- Hybrid caching via `foyer`

---

## New Implementation Strategy: Integrate dsrs

### Key Insight from Exploration

**Python DSPy patterns worth adopting:**
1. **MIPROv2 3-stage optimization**: Bootstrap → Propose → Bayesian search (with Optuna)
2. **Refine pattern** (replaced assertions): Multiple rollouts → reward eval → feedback hints → retry
3. **Callback system**: `on_module_start/end`, `on_lm_start/end`, `on_adapter_*` for HUD integration
4. **Streaming**: `streamify()` with anyio channels, StatusMessages, StreamListeners
5. **Usage tracking**: Token counts + costs per call, aggregated via UsageTracker

**dsrs patterns we get for free:**
1. **Same `rig` library** - LM client abstraction already compatible
2. **Trait-based design** - Module, Predictor, Adapter easily extensible
3. **DAG tracing** - Graph with Node types ready for Nostr event conversion
4. **Optimizers** - COPRO, MIPROv2, GEPA already implemented in Rust

---

## Implementation Plan

### Phase 1: Add dsrs as Workspace Member

**Step 1.1: Add dsrs to workspace**
```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    # ... existing ...
    "crates/dspy-rs",  # Symlink or copy from ~/code/dsrs/crates/dspy-rs
]
```

**Step 1.2: Create Pylon LM Provider**
**File:** `crates/dspy-rs/src/adapter/pylon.rs` (NEW)

```rust
/// Pylon-backed LM provider for distributed inference
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

// Add to LMClient enum in core/lm.rs:
pub enum LMClient {
    // ... existing providers ...
    Pylon(PylonClient),
}
```

### Phase 2: Bridge DAG Traces → Nostr Events

**File:** `crates/dspy-rs/src/trace/nostr_bridge.rs` (NEW)

```rust
/// Convert dsrs execution graph to Nostr events
pub fn graph_to_nostr_events(graph: &Graph) -> Vec<NostrEvent> {
    graph.nodes.iter().map(|node| {
        match &node.node_type {
            NodeType::Root => kind_1_text_note(node),
            NodeType::Predict { signature_name, .. } => kind_5050_job(node),
            NodeType::Operator { name } => kind_1_with_tags(node),
            NodeType::Map { mapping } => kind_1_lineage(node),
        }
    }).collect()
}

/// Reconstruct graph from Nostr events
pub fn nostr_events_to_graph(events: &[NostrEvent]) -> Graph {
    // Use `e` tags for parent references
    // Rebuild node relationships
}
```

### Phase 3: Implement Callback System for HUD

**File:** `crates/dspy-rs/src/callbacks.rs` (NEW - port from Python)

```rust
pub trait DspyCallback: Send + Sync {
    fn on_module_start(&self, call_id: Uuid, instance: &dyn Module, inputs: &Example);
    fn on_module_end(&self, call_id: Uuid, outputs: Result<&Prediction>, exception: Option<&Error>);
    fn on_lm_start(&self, call_id: Uuid, instance: &LM, inputs: &Chat);
    fn on_lm_end(&self, call_id: Uuid, outputs: Result<&Message>, exception: Option<&Error>);
    fn on_optimizer_candidate(&self, candidate_id: String, metrics: HashMap<String, f32>);
}

/// HUD-streaming callback
pub struct HudCallback {
    sender: mpsc::Sender<HudEvent>,
}

impl DspyCallback for HudCallback {
    fn on_module_start(&self, call_id, instance, inputs) {
        self.sender.send(HudEvent::ModuleStart { call_id, name: instance.name() });
    }
    // ... etc
}
```

### Phase 4: Adopt Refine Pattern for Verification

**File:** `crates/dspy-rs/src/predictors/refine.rs` (NEW - port from Python)

```rust
/// Multiple rollouts with reward-based selection and feedback hints
pub struct Refine<M: Module> {
    module: M,
    n_rollouts: usize,
    reward_fn: Box<dyn Fn(&Example, &Prediction) -> f32>,
    threshold: f32,
}

impl<M: Module> Module for Refine<M> {
    async fn forward(&self, inputs: Example) -> Result<Prediction> {
        let mut best: Option<(Prediction, f32)> = None;

        for i in 0..self.n_rollouts {
            let output = self.module.forward(inputs.clone()).await?;
            let reward = (self.reward_fn)(&inputs, &output);

            if reward >= self.threshold {
                return Ok(output);  // Early exit on success
            }

            if best.is_none() || reward > best.as_ref().unwrap().1 {
                best = Some((output, reward));
            }

            // Generate feedback for next iteration
            if i < self.n_rollouts - 1 {
                let feedback = self.generate_feedback(&inputs, &best)?;
                inputs.data.insert("hint_".into(), feedback);
            }
        }

        Ok(best.unwrap().0)
    }
}
```

### Phase 5: Swarm-Backed Optimization

**File:** `crates/autopilot/src/dspy_compiler.rs` (NEW)

```rust
use dspy_rs::{Optimizer, MIPROv2, Example};

pub struct SwarmCompiler {
    /// Cheap inference for candidate generation
    swarm_lm: LM,  // Pylon provider
    /// Better model for final validation
    validation_lm: LM,  // Claude/GPT-4
    /// Budget controls
    budget: BudgetPolicy,
}

impl SwarmCompiler {
    pub async fn compile<M: Module + Optimizable + Evaluator>(
        &self,
        module: &mut M,
        trainset: Vec<Example>,
    ) -> Result<OptimizationResult> {
        // Stage 1: Bootstrap on swarm (cheap)
        dspy_rs::configure(self.swarm_lm.clone(), ChatAdapter);
        let mipro = MIPROv2::new(/* ... */);

        // Stage 2: Optimize with budget tracking
        let callbacks = vec![
            Box::new(CostTracker::new(self.budget.clone())),
            Box::new(HudCallback::new(self.hud_sender.clone())),
        ];

        mipro.compile(module, trainset).await?;

        // Stage 3: Validate best candidates on premium model
        dspy_rs::configure(self.validation_lm.clone(), ChatAdapter);
        let final_score = module.evaluate(valset).await?;

        Ok(OptimizationResult { module, score: final_score, cost: callbacks[0].total_cost() })
    }
}
```

### Phase 6: Training Data Extraction

**File:** `crates/autopilot/src/trace_extraction.rs` (NEW)

```rust
/// Extract DSPy training examples from session traces
pub struct TraceExtractor;

impl TraceExtractor {
    /// Convert session JSONL → DSPy Examples
    pub fn extract_from_session(path: &Path) -> Vec<Example> {
        let entries = parse_jsonl(path);
        entries.iter()
            .filter(|e| e.event_type == "PlanningComplete" && e.success)
            .map(|e| Example {
                data: hashmap! {
                    "repository_summary" => e.context.repo_summary,
                    "issue_description" => e.context.issue,
                    "expected_analysis" => e.output.analysis,
                    "expected_files" => e.output.files,
                    // ... etc
                },
                input_keys: vec!["repository_summary", "issue_description"],
                output_keys: vec!["expected_analysis", "expected_files", ...],
            })
            .collect()
    }
}
```

---

## Files to Create/Modify

### New Files
| File | Purpose |
|------|---------|
| `crates/dspy-rs/` | Symlink/copy of dsrs |
| `crates/dspy-rs/src/adapter/pylon.rs` | Pylon LM provider |
| `crates/dspy-rs/src/trace/nostr_bridge.rs` | DAG ↔ Nostr events |
| `crates/dspy-rs/src/callbacks.rs` | Callback system for HUD |
| `crates/dspy-rs/src/predictors/refine.rs` | Refine pattern for verification |
| `crates/autopilot/src/dspy_compiler.rs` | Swarm-backed optimization |
| `crates/autopilot/src/trace_extraction.rs` | Session → training data |
| `docs/DSPY_ARCHITECTURE.md` | Deep technical doc |

### Modified Files
| File | Changes |
|------|---------|
| `Cargo.toml` | Add dspy-rs to workspace |
| `crates/dspy-rs/src/core/lm.rs` | Add Pylon to LMClient enum |
| `docs/DSPY_ROADMAP.md` | Update with new strategy |
| `SYNTHESIS_EXECUTION.md` | Add dsrs integration section |

---

## Success Criteria

1. **dsrs compiles in workspace**: `cargo build -p dspy-rs` works
2. **Pylon provider works**: Can run DSPy optimization using swarm inference
3. **Traces flow to HUD**: Optimization events stream to runtime HUD service
4. **Refine pattern works**: Verification uses reward functions with feedback hints
5. **Training extraction works**: Can extract examples from successful sessions
6. **DAG → Nostr bridge works**: Execution graphs serialize to Nostr events

---

## The Compelling Loop

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

## Architecture Summary

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

**DSPy (dsrs)** = decides what to do (best prompts, demos, tool use patterns)
**OpenAgents** = decides where/how/cost/observability
