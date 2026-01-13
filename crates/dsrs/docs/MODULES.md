# Modules

Composable intelligence blocks for structuring reasoning.

## Overview

A **Module** is a unit of behavior built on one or more signatures. Modules define **how intelligence flows through your system**:

- They can call other modules
- They can branch, loop, or pipeline
- They feel like normal program components

If signatures define *what* intelligence looks like, modules define **how intelligence flows**.

> **Mental model:** "An LLM program is a graph of reasoning steps, not a single prompt."

## Core Traits

### Module Trait

The base trait for all DSPy modules.

```rust
// File: crates/dsrs/src/core/module.rs

#[allow(async_fn_in_trait)]
pub trait Module: Send + Sync {
    /// Execute the module on a single example.
    async fn forward(&self, inputs: Example) -> Result<Prediction>;

    /// Execute the module on multiple examples in parallel.
    async fn batch(
        &self,
        inputs: Vec<Example>,
        max_concurrency: usize,
        display_progress: bool,
    ) -> Result<Vec<Prediction>>;
}
```

### Predictor Trait

Extends Module with streaming support and configuration options.

```rust
// File: crates/dsrs/src/core/module.rs

pub trait Predictor: Module {
    /// Forward with a custom LM configuration.
    async fn forward_with_config(
        &self,
        inputs: Example,
        lm: Arc<LM>,
    ) -> Result<Prediction>;

    /// Forward with streaming callbacks.
    async fn forward_with_streaming(
        &self,
        inputs: Example,
        lm: Arc<LM>,
        callback: Box<dyn DspyCallback>,
    ) -> Result<Prediction>;
}
```

### Optimizable Trait

Enables optimization of module parameters.

```rust
// File: crates/dsrs/src/core/module.rs

pub trait Optimizable {
    /// Get the module's signature for analysis.
    fn get_signature(&self) -> &dyn MetaSignature;

    /// Get nested optimizable parameters (for composite modules).
    fn parameters(&mut self) -> IndexMap<String, &mut dyn Optimizable>;

    /// Update the signature's instruction text.
    fn update_signature_instruction(&mut self, instruction: String) -> Result<()>;
}
```

---

## Core Module Implementations

### Predict

The fundamental DSPy module that executes a single signature.

```rust
// File: crates/dsrs/src/predictors/predict.rs

use dsrs::{Predict, Signature, example};

// Create a predictor from any signature
let predictor = Predict::new(MySignature::new());

// Basic execution
let prediction = predictor.forward(example! {
    "input_field": "input" => "some value"
}).await?;

// With custom LM
let prediction = predictor.forward_with_config(example, lm).await?;

// With streaming callbacks
let prediction = predictor.forward_with_streaming(example, lm, callback).await?;

// Batch processing
let predictions = predictor.batch(examples, 4, true).await?;
```

**Features:**
- Supports any `MetaSignature` implementation
- Tool attachment via `with_tools()`
- Streaming with callbacks
- Batch processing with concurrency control
- Tracing and logging support

**Implements:** `Module`, `Predictor`, `Optimizable`

---

### Refine

A meta-operator that wraps any module with retry, reward, and fallback logic.

```rust
// File: crates/dsrs/src/predictors/refine.rs

use dsrs::predictors::Refine;

// Wrap any module with Refine
let refined = Refine::new(predictor)
    .with_max_retries(3)           // Retry up to 3 times
    .with_threshold(0.7)           // Accept predictions scoring >= 0.7
    .with_reward_fn(|_input, pred| {
        // Score the prediction (0.0 to 1.0)
        if pred.data.contains_key("answer") { 1.0 } else { 0.0 }
    })
    .with_best_of_n(true)          // Return best across all retries
    .with_fallback_lm(fallback);   // Use fallback LM on failure

let prediction = refined.forward(example).await?;
```

**Configuration:**
| Option | Default | Description |
|--------|---------|-------------|
| `max_retries` | 3 | Maximum retry attempts |
| `threshold` | 0.5 | Minimum reward to accept |
| `best_of_n` | false | Return best of all attempts |
| `fallback_lm` | None | Fallback LM on failure |

**Implements:** `Module` (for any wrapped module type)

---

## Composite Modules

### AdjutantModule

A complex composite module implementing Adjutant's 3-phase tiered execution.

```rust
// File: crates/adjutant/src/dspy/module.rs

use adjutant::dspy::AdjutantModule;

let module = AdjutantModule::builder()
    .planner(custom_planner)      // Optional: custom planner
    .executor(custom_executor)    // Optional: custom executor
    .synthesizer(custom_synth)    // Optional: custom synthesizer
    .build();

// Phase 1: Planning (GLM 4.7)
// Breaks task into atomic subtasks
let plan = module.plan(
    "Fix authentication bug",
    "Users cannot log in after password reset",
    "inline",
    &file_contents
).await?;

// Phase 2: Execution (Qwen-3-32B)
// Executes each subtask
let result = module.execute_subtask(
    "edit",                       // action: read | edit | bash
    "src/auth.rs",               // target file
    "Fix password validation",    // instruction
    &current_content             // file context
).await?;

// Phase 3: Synthesis (GLM 4.7)
// Combines results into final verdict
let synthesis = module.synthesize(
    "Fix authentication bug",
    &subtask_results
).await?;

// Full pipeline execution
let prediction = module.forward(example).await?;
```

**Internal Structure:**
```
AdjutantModule
├── planner: Predict<SubtaskPlanningSignature>
├── executor: Predict<SubtaskExecutionSignature>
└── synthesizer: Predict<ResultSynthesisSignature>
```

**Implements:** `Module`, `Evaluator`, `Optimizable`

---

### QARater (Example)

A composite module that answers questions then rates the answer quality.

```rust
// File: examples/01-simple.rs

#[derive(Clone)]
struct QARater {
    answerer: Predict,  // QASignature with chain-of-thought
    rater: Predict,     // RateSignature
}

impl Module for QARater {
    async fn forward(&self, inputs: Example) -> Result<Prediction> {
        // Step 1: Answer the question
        let answer = self.answerer.forward(inputs.clone()).await?;

        // Step 2: Rate the answer
        let rate_input = example! {
            "question": "input" => inputs.get("question"),
            "answer": "input" => answer.get("answer", None)
        };
        let rating = self.rater.forward(rate_input).await?;

        // Combine results
        Ok(Prediction::merge(answer, rating))
    }
}
```

---

## Pipeline Modules

Specialized modules for decision routing in Adjutant.

### ComplexityPipeline

Classifies task complexity for routing decisions.

```rust
// File: crates/adjutant/src/dspy/decision_pipelines.rs

use adjutant::dspy::decision_pipelines::{ComplexityPipeline, ComplexityInput};

let pipeline = ComplexityPipeline::new();
// Or with custom LM:
let pipeline = ComplexityPipeline::with_lm(custom_lm);

let result = pipeline.classify(&ComplexityInput {
    task_description: "Refactor the authentication module".to_string(),
    file_count: 5,
    estimated_tokens: 10000,
    keywords: vec!["refactor".to_string(), "auth".to_string()],
}).await?;

// Result contains:
// - complexity: "Low" | "Medium" | "High" | "VeryHigh"
// - reasoning: String
// - confidence: f32
```

**Complexity Levels:**
| Level | Scope | Risk |
|-------|-------|------|
| Low | Single-file edit | Minimal |
| Medium | Multi-file edit | Moderate |
| High | Complex refactoring | Significant |
| VeryHigh | System-wide changes | High |

---

### DelegationPipeline

Decides whether to delegate task execution and to which target.

```rust
// File: crates/adjutant/src/dspy/decision_pipelines.rs

use adjutant::dspy::decision_pipelines::{DelegationPipeline, DelegationInput};

let pipeline = DelegationPipeline::new();

let result = pipeline.decide(&DelegationInput {
    task_description: "Analyze security vulnerabilities".to_string(),
    complexity: "High".to_string(),
    file_count: 12,
    estimated_tokens: 50000,
}).await?;

// Result contains:
// - should_delegate: bool
// - delegation_target: "codex_code" | "rlm" | "local_tools"
// - reasoning: String
// - confidence: f32
```

**Delegation Targets:**
| Target | Use Case |
|--------|----------|
| `codex_code` | Complex multi-file tasks, architectural work |
| `rlm` | Large context analysis, recursive investigation |
| `local_tools` | Simple edits, small scope tasks |

---

### RlmTriggerPipeline

Decides whether to use Recursive Language Model for deep analysis.

```rust
// File: crates/adjutant/src/dspy/decision_pipelines.rs

use adjutant::dspy::decision_pipelines::{RlmTriggerPipeline, RlmTriggerInput};

let pipeline = RlmTriggerPipeline::new();

let result = pipeline.should_trigger(&RlmTriggerInput {
    task_description: "Find all usages of deprecated API".to_string(),
    complexity: "High".to_string(),
    estimated_tokens: 50000,
    file_count: 20,
    repeated_actions: true,
}).await?;

// Result contains:
// - use_rlm: bool
// - reasoning: String
// - confidence: f32
```

**When RLM is Beneficial:**
- Deep code analysis
- Recursive investigation
- Security audits
- Comprehensive reviews
- Finding all occurrences across large codebases

---

### IssueSuggestionPipeline

Suggests top issues for an agent to work on.

```rust
// File: crates/adjutant/src/dspy/issue_suggestion.rs

use adjutant::dspy::issue_suggestion::IssueSuggestionPipeline;

let pipeline = IssueSuggestionPipeline::new();

let result = pipeline.suggest(&IssueSuggestionInput {
    available_issues: issues_json,
    workspace_context: context,
    recent_work: recent_activity,
    user_preferences: prefs,
}).await?;

// Returns top 3 suggested issues with rationale
```

---

### IssueValidationPipeline

Validates if issues are still accurate before work starts.

```rust
// File: crates/adjutant/src/dspy/issue_validation.rs

use adjutant::dspy::issue_validation::IssueValidationPipeline;

let pipeline = IssueValidationPipeline::new();

let result = pipeline.validate(&IssueValidationInput {
    issue_title: "Fix login bug".to_string(),
    issue_description: description,
    recent_commits: commits,
    changed_files: files,
}).await?;

// Result contains:
// - is_valid: bool
// - validation_status: "VALID" | "ALREADY_ADDRESSED" | "STALE" | "NEEDS_UPDATE"
// - reason: String
// - confidence: f32
```

---

## Orchestrators

### DspyOrchestrator

Multi-stage orchestrator for Adjutant's planning workflow.

```rust
// File: crates/adjutant/src/dspy_orchestrator.rs

// Three stages:
// 1. Environment Assessment - Analyzes system state
// 2. Planning - Creates implementation plan
// 3. Todo List Creation - Converts plan to actionable tasks

// Emits DspyStage events for UI rendering
```

---

### DspyOrchestrator (RLM)

Document analysis orchestrator with Router → Extractor → Reducer → Verifier pipeline.

```rust
// File: crates/rlm/src/dspy_orchestrator.rs

use rlm::dspy_orchestrator::{DspyOrchestrator, DspyOrchestratorConfig};

let config = DspyOrchestratorConfig {
    chunk_size: 4000,
    chunk_overlap: 200,
    max_chunks: 50,
    use_cot: true,          // Enable chain-of-thought
    verify_answers: true,   // Enable verification stage
};

let orchestrator = DspyOrchestrator::new(config);

// Stages:
// 1. Router - identifies relevant document sections
// 2. Extractor - extracts findings (with optional CoT)
// 3. Reducer - synthesizes findings into answer
// 4. Verifier - validates answer against evidence
```

---

### FrlmConductor

Federated RLM orchestrator for distributed execution.

```rust
// File: crates/frlm/src/conductor.rs

// Complex distributed orchestrator managing:
// - SubQueryScheduler
// - TraceEmitter
// - FrlmPolicy
// - Budget/timeout management
// - Trace-native execution records
```

---

## Creating Custom Modules

### Basic Custom Module

```rust
use dsrs::{Module, Example, Prediction, Result};

struct MyModule {
    inner: Predict,
}

impl Module for MyModule {
    async fn forward(&self, inputs: Example) -> Result<Prediction> {
        // Pre-processing
        let processed = preprocess(inputs);

        // Call inner module
        let result = self.inner.forward(processed).await?;

        // Post-processing
        Ok(postprocess(result))
    }

    async fn batch(
        &self,
        inputs: Vec<Example>,
        max_concurrency: usize,
        display_progress: bool,
    ) -> Result<Vec<Prediction>> {
        // Default: sequential forward calls
        // Override for custom batching logic
    }
}
```

### Optimizable Custom Module

```rust
use dsrs::{Module, Optimizable, MetaSignature};
use indexmap::IndexMap;

struct OptimizableModule {
    stage1: Predict,
    stage2: Predict,
}

impl Optimizable for OptimizableModule {
    fn get_signature(&self) -> &dyn MetaSignature {
        self.stage1.get_signature()
    }

    fn parameters(&mut self) -> IndexMap<String, &mut dyn Optimizable> {
        let mut params = IndexMap::new();
        params.insert("stage1".to_string(), &mut self.stage1 as &mut dyn Optimizable);
        params.insert("stage2".to_string(), &mut self.stage2 as &mut dyn Optimizable);
        params
    }

    fn update_signature_instruction(&mut self, instruction: String) -> Result<()> {
        self.stage1.update_signature_instruction(instruction)
    }
}
```

---

## MVP Artifacts

Every module execution in production should produce these three artifacts:

### PR_SUMMARY.md

Human-readable summary of what was accomplished.

```markdown
# PR Summary

## Changes Made
- Fixed authentication bug in `src/auth.rs`
- Added test case in `tests/auth_test.rs`

## Files Modified
- `src/auth.rs` (15 lines changed)
- `tests/auth_test.rs` (25 lines added)

## Verification
- `cargo check`: PASS
- `cargo test`: PASS (24/24 tests)

## Confidence: 0.92
```

**Generated by:** `ResultSynthesisSignature`

### RECEIPT.json

Cryptographic receipt for verifiability and audit.

```json
{
  "session_id": "sess_abc123",
  "started_at": "2025-01-13T10:00:00Z",
  "completed_at": "2025-01-13T10:05:32Z",
  "issue_number": 42,
  "plan_hash": "sha256:abc123...",
  "tool_calls": [
    {
      "id": "tc_001",
      "tool": "file_read",
      "params_hash": "sha256:def456...",
      "output_hash": "sha256:ghi789...",
      "step_utility": 0.8,
      "latency_ms": 45
    }
  ],
  "verification": {
    "commands_run": ["cargo check", "cargo test"],
    "exit_codes": [0, 0],
    "verification_delta": 3
  },
  "final_confidence": 0.92,
  "policy_version": "v1.2.3",
  "signature": "sig:..."
}
```

**Enables:**
- Audit trail for all agent actions
- Reproducibility verification
- Cost accounting per tool call
- Training data extraction (step_utility labels)

### REPLAY.jsonl

Canonical event stream for replay and debugging.

```jsonl
{"t":"2025-01-13T10:00:00Z","event":"session_start","session_id":"sess_abc123"}
{"t":"2025-01-13T10:00:01Z","event":"plan_start","plan_hash":"sha256:abc123..."}
{"t":"2025-01-13T10:00:02Z","event":"tool_call","tool":"file_read","params":{"path":"src/auth.rs"}}
{"t":"2025-01-13T10:00:02Z","event":"tool_result","output_hash":"sha256:def456...","step_utility":0.8}
{"t":"2025-01-13T10:05:32Z","event":"session_end","status":"success","confidence":0.92}
```

**Enables:**
- CLI replay viewer (`adjutant replay sess_abc123`)
- Counterfactual analysis
- Shadow mode comparison
- Training data generation

---

## Execution Flow v2

The MVP execution flow uses merged signatures for efficiency:

```
┌─────────────────────────────────────────────────────────────────┐
│                    Execution Flow v2                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Issue                                                           │
│    │                                                             │
│    ▼                                                             │
│  ┌──────────────────────┐                                        │
│  │ IssueValidationSig   │ ← Gates stale/invalid work             │
│  └──────────┬───────────┘                                        │
│             │                                                    │
│             ▼                                                    │
│  ┌──────────────────────┐                                        │
│  │ SubtaskPlanningSig   │ → Emits PlanIR                         │
│  └──────────┬───────────┘                                        │
│             │                                                    │
│             ▼                                                    │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │               Per-Step Loop (max_iterations)              │   │
│  │  ┌─────────────────┐                                      │   │
│  │  │ ToolCallSig     │ → tool, params, expected_outcome     │   │
│  │  └────────┬────────┘                                      │   │
│  │           │                                               │   │
│  │           ▼                                               │   │
│  │  ┌─────────────────┐                                      │   │
│  │  │ Tool Execution  │ → actual output, exit_code           │   │
│  │  └────────┬────────┘                                      │   │
│  │           │                                               │   │
│  │           ▼                                               │   │
│  │  ┌─────────────────┐                                      │   │
│  │  │ ToolResultSig   │ → step_utility (LEARNING SIGNAL)     │   │
│  │  └────────┬────────┘                                      │   │
│  │           │                                               │   │
│  │           ├─── Record to REPLAY.jsonl                     │   │
│  │           └─── Continue or break                          │   │
│  └───────────────────────────────────────────────────────────┘   │
│             │                                                    │
│             ▼                                                    │
│  ┌──────────────────────┐                                        │
│  │ ResultSynthesisSig   │ → PR_SUMMARY.md                        │
│  └──────────┬───────────┘                                        │
│             │                                                    │
│             ▼                                                    │
│  ┌──────────────────────┐                                        │
│  │ VerificationSig      │ → verification_delta                   │
│  └──────────┬───────────┘                                        │
│             │                                                    │
│             ▼                                                    │
│       RECEIPT.json                                               │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Key Changes from v1:**
- `ToolCallSignature` replaces `ExecutionStrategySignature` + `ToolSelectionSignature`
- `ToolResultSignature` adds learning signal (was missing in v1)
- Every tool call emits to REPLAY.jsonl for reproducibility
- RECEIPT.json captures full session provenance

---

## Policy Bundles

Policy bundles are versioned artifacts containing optimized prompts and configuration.

### Bundle Structure

```
.adjutant/policies/
├── v1.2.3/
│   ├── manifest.json
│   ├── signatures/
│   │   ├── ToolCallSignature.json
│   │   ├── SubtaskPlanningSignature.json
│   │   └── ...
│   └── demos/
│       ├── tool_call_demos.jsonl
│       └── planning_demos.jsonl
└── current -> v1.2.3/
```

### manifest.json

```json
{
  "version": "v1.2.3",
  "created_at": "2025-01-13T10:00:00Z",
  "parent_version": "v1.2.2",
  "optimization_method": "MIPROv2",
  "training_examples": 150,
  "validation_score": 0.87,
  "signatures": [
    "ToolCallSignature",
    "SubtaskPlanningSignature",
    "ResultSynthesisSignature"
  ],
  "locked": false
}
```

### CLI Commands

```bash
# List policy versions
adjutant policy list
# v1.2.3 (current)
# v1.2.2
# v1.2.1

# Pin to specific version
adjutant policy pin v1.2.2
# Pinned to v1.2.2

# Rollback to previous version
adjutant policy rollback
# Rolled back from v1.2.3 to v1.2.2

# Create new policy from training data
adjutant policy compile --trainset ./labeled_examples.jsonl
# Compiled v1.2.4 with validation score 0.89

# Diff two policy versions
adjutant policy diff v1.2.2 v1.2.3
# ToolCallSignature: instruction changed (+15 chars)
# SubtaskPlanningSignature: 3 new demos added

# Lock a policy (prevent modification)
adjutant policy lock v1.2.3
# Locked v1.2.3
```

### Shadow/Canary Mode

Run new policies alongside production without affecting output:

```bash
# Shadow mode: record what new policy would have done
adjutant run --shadow-policy v1.2.4

# Canary mode: 10% of requests use new policy
adjutant run --canary-policy v1.2.4 --canary-percent 10
```

**Shadow mode output:**
```json
{
  "production_policy": "v1.2.3",
  "shadow_policy": "v1.2.4",
  "counterfactual": {
    "production_tool_calls": 5,
    "shadow_tool_calls": 4,
    "production_success": true,
    "shadow_success": true,
    "step_utility_delta": 0.12
  }
}
```

---

## Module Index

| Module | Location | Type | Purpose |
|--------|----------|------|---------|
| **Core** |
| Predict | `dsrs/src/predictors/predict.rs` | Core | Single signature executor |
| Refine | `dsrs/src/predictors/refine.rs` | Wrapper | Retry/fallback logic |
| **Composite** |
| AdjutantModule | `adjutant/src/dspy/module.rs` | Composite | 3-phase task execution |
| QARater | `examples/01-simple.rs` | Example | Question → Answer → Rate |
| SimpleQA | `examples/08-optimize-mipro.rs` | Example | Optimizable QA |
| **Pipelines** |
| ComplexityPipeline | `adjutant/src/dspy/decision_pipelines.rs` | Decision | Task complexity classification |
| DelegationPipeline | `adjutant/src/dspy/decision_pipelines.rs` | Decision | Delegation routing |
| RlmTriggerPipeline | `adjutant/src/dspy/decision_pipelines.rs` | Decision | RLM trigger decision |
| IssueSuggestionPipeline | `adjutant/src/dspy/issue_suggestion.rs` | Decision | Issue prioritization |
| IssueValidationPipeline | `adjutant/src/dspy/issue_validation.rs` | Decision | Issue validation |
| **Orchestrators** |
| DspyOrchestrator | `adjutant/src/dspy_orchestrator.rs` | Orchestrator | Multi-stage planning |
| DspyOrchestrator | `rlm/src/dspy_orchestrator.rs` | Orchestrator | Document analysis |
| FrlmConductor | `frlm/src/conductor.rs` | Orchestrator | Federated execution |

---

## Integration with Other Primitives

| Primitive | Relationship |
|-----------|--------------|
| **Signatures** | Modules execute signatures via Predict |
| **Tools** | Modules can use tools via `Predict.with_tools()` |
| **Adapters** | ChatAdapter handles prompt formatting for modules |
| **Optimizers** | Modules implementing Optimizable can be optimized |
| **Metrics** | Modules implementing Evaluator provide metrics |
