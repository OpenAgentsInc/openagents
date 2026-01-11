# Signatures (Wave 4)

Optimizable DSPy signatures for agent decision-making.

## Overview

Signatures define input/output contracts for agent decisions. They can be optimized using DSPy optimizers (MIPROv2, GEPA, COPRO) to improve agent performance.

## Signature Categories

### Retrieval Policy

Signatures for intelligent code retrieval decisions.

#### QueryComposerSignature

Transforms user goals and failure logs into targeted search queries.

```rust
use dsrs::signatures::QueryComposerSignature;

let sig = QueryComposerSignature::new();

// Inputs:
// - goal: The user's task
// - failure_log: Previous failed attempts
// - previous_queries: Queries already tried

// Outputs:
// - queries: List of new search queries
// - lanes: Suggested retrieval lane for each
// - rationale: Explanation of query strategy
```

#### RetrievalRouterSignature

Decides which retrieval lane to use and how many results to fetch.

```rust
use dsrs::signatures::RetrievalRouterSignature;

let sig = RetrievalRouterSignature::new();

// Inputs:
// - query: The search query
// - available_lanes: Available backends
// - budget_remaining: Budget in msats

// Outputs:
// - lane: Recommended lane (ripgrep, lsp, semantic, git)
// - k: Number of results to fetch
// - rationale: Why this lane was chosen
```

#### CandidateRerankSignature

Reranks retrieval candidates by relevance. Maps to `oa.retrieval_rerank.v1` swarm job.

```rust
use dsrs::signatures::{CandidateRerankSignature, RerankCandidate};

let sig = CandidateRerankSignature::new();

// Inputs:
// - user_task: The user's task
// - candidates: List of candidates with content
// - k: Number of top results to return

// Outputs:
// - topk: Ranked candidates with scores (0.0-1.0)
// - rationale: Overall ranking strategy
```

### Chunk Analysis

Signatures for analyzing code chunks.

#### ChunkTaskSelectorSignature

Decides what analysis tasks to perform on a code chunk.

```rust
use dsrs::signatures::{ChunkTaskSelectorSignature, AnalysisTask};

let sig = ChunkTaskSelectorSignature::new();

// Inputs:
// - chunk: CodeChunk with path, lines, content
// - user_task: The user's overall task
// - previous_findings: What's been found so far

// Outputs:
// - tasks: Vec<AnalysisTask> to perform
// - priority: Priority order
// - rationale: Why these tasks

// Available tasks:
// - Summarize, BugHypothesis, ExtractSymbols
// - AnalyzeDependencies, SecurityAudit
// - PerformanceAnalysis, TestCoverage
```

#### ChunkAnalysisToActionSignature

Aggregates chunk analysis results into actionable next steps.

```rust
use dsrs::signatures::{ChunkAnalysisToActionSignature, ChunkFinding, NextAction};

let sig = ChunkAnalysisToActionSignature::new();

// Inputs:
// - findings: Vec<ChunkFinding> from analyses
// - user_task: The user's original task
// - budget_remaining: Remaining budget

// Outputs:
// - summary: High-level summary
// - next_actions: Vec<NextAction> in priority order
// - confidence: Overall confidence (0.0-1.0)
// - key_insights: Most important discoveries
```

### Sandbox Operations

Signatures for sandbox resource management.

#### SandboxProfileSelectionSignature

Selects appropriate sandbox resources for command execution.

```rust
use dsrs::signatures::{SandboxProfileSelectionSignature, SandboxProfile};

let sig = SandboxProfileSelectionSignature::new();

// Inputs:
// - commands: Commands to execute
// - repo_size: Repository size in bytes
// - previous_failures: Previous error messages

// Outputs:
// - profile: Small, Medium, or Large
// - timeout: Custom timeout override
// - rationale: Why this profile
// - estimated_cost: Cost in msats

// Profiles:
// Small:  1 vCPU, 1GB RAM,  5GB disk, 60s timeout
// Medium: 2 vCPUs, 4GB RAM, 8GB disk, 120s timeout
// Large:  4 vCPUs, 8GB RAM, 10GB disk, 300s timeout
```

#### FailureTriageSignature

Interprets sandbox failures and recommends next actions.

```rust
use dsrs::signatures::{FailureTriageSignature, FailureCategory, TriageAction};

let sig = FailureTriageSignature::new();

// Inputs:
// - command: The failed command
// - exit_code: Exit code
// - stderr_preview: First/last 1000 chars of stderr
// - stdout_preview: First/last 1000 chars of stdout
// - duration_ms: How long it ran

// Outputs:
// - diagnosis: FailureDiagnosis with category and confidence
// - next_action: TriageAction (RetryLarger, FixAndRetry, etc.)
// - should_retry: Boolean
// - fix_suggestion: Suggested fix if applicable

// Categories: OutOfMemory, Timeout, CompileError, TestFailure,
//            MissingDependency, PermissionDenied, NetworkError, etc.
```

### Budgeting & Memory

Signatures for cost control and avoiding wasted work.

#### LaneBudgeterSignature

Allocates budget across different execution lanes.

```rust
use dsrs::signatures::{LaneBudgeterSignature, ExecutionLane, LaneAllocation};

let sig = LaneBudgeterSignature::new();

// Inputs:
// - task_complexity: low, medium, or high
// - budget_remaining: Total remaining budget
// - available_lanes: Available execution lanes
// - task_type: inference, retrieval, or sandbox

// Outputs:
// - allocations: Vec<LaneAllocation> with budget per lane
// - rationale: Allocation strategy explanation
// - fallback_order: Lane preference for fallback

// Lanes: Local (free), Swarm (~1-10 msats), Datacenter (~100-1000 msats)
```

#### AgentMemorySignature

Detects redundant queries and suggests alternatives.

```rust
use dsrs::signatures::{AgentMemorySignature, QueryHistoryEntry, MemoryAnalysis};

let sig = AgentMemorySignature::new();

// Inputs:
// - proposed_query: Query about to execute
// - proposed_lane: Lane about to use
// - query_history: Vec<QueryHistoryEntry>
// - results_summary: What's been found

// Outputs:
// - analysis: MemoryAnalysis with redundancy check
// - alternative_query: Better query if redundant
// - should_proceed: Whether to run the query
// - rationale: Explanation

// Helper functions:
use dsrs::signatures::agent_memory::{is_simple_redundant, simple_similarity};
```

## Plan IR & Execution

Unified types for planning and execution across Adjutant and Autopilot.

### PlanIR (Canonical Intermediate Representation)

Both planning stacks (Adjutant's `SubtaskPlanningSignature` and Autopilot's `PlanningSignature`) emit this unified format:

```rust
use dsrs::ir::{PlanIR, PlanStep, StepIntent, VerificationStrategy, Complexity};

/// Canonical plan format emitted by all planning signatures.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlanIR {
    /// High-level analysis of the task
    pub analysis: String,

    /// Ordered list of steps to execute
    pub steps: Vec<PlanStep>,

    /// How to verify completion
    pub verification_strategy: VerificationStrategy,

    /// Overall task complexity
    pub complexity: Complexity,

    /// Planner confidence (0.0-1.0)
    pub confidence: f32,
}

/// A single step in the plan.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlanStep {
    /// Unique identifier (e.g., "step-1", "step-2a")
    pub id: String,

    /// Human-readable description
    pub description: String,

    /// What this step achieves
    pub intent: StepIntent,

    /// Files expected to be touched
    pub target_files: Vec<String>,

    /// Step IDs this depends on (for parallel execution)
    pub depends_on: Vec<String>,

    /// Max iterations for this step (per-step loop budget)
    pub max_iterations: u8,
}

/// Classification of what a step does.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum StepIntent {
    /// Read/search to understand (file_read, ripgrep, lsp)
    Investigate,

    /// Edit files (file_edit)
    Modify,

    /// Run tests/build (shell)
    Verify,

    /// Combine results from prior steps
    Synthesize,
}

/// How to verify task completion.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VerificationStrategy {
    /// Commands to run (e.g., ["cargo check", "cargo test"])
    pub commands: Vec<String>,

    /// What constitutes success
    pub success_criteria: String,

    /// Max verification retries
    pub max_retries: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Complexity {
    Low,
    Medium,
    High,
    VeryHigh,
}
```

**Migration path:**
- Adjutant's `SubtaskPlanningSignature.subtasks` → `PlanIR.steps`
- Autopilot's `PlanningSignature.implementation_steps` → `PlanIR.steps`
- Both emit the same IR, downstream doesn't care which planner ran

### ToolCallSignature (Merged Execution)

Replaces the redundant `ExecutionStrategySignature` + `ToolSelectionSignature` pair.
**One LLM call per step instead of two.**

```rust
use dsrs::signatures::ToolCallSignature;

/// Merged signature for tool selection and invocation.
/// Combines what was previously ExecutionStrategySignature + ToolSelectionSignature.
#[Signature]
pub struct ToolCallSignature {
    /// Current plan step description
    #[input]
    pub step: String,

    /// Step intent from PlanIR (Investigate/Modify/Verify/Synthesize)
    #[input]
    pub step_intent: String,

    /// JSON array of available tool specs
    #[input]
    pub available_tools: String,

    /// JSON array of recent tool calls + results (last 5)
    #[input]
    pub execution_history: String,

    /// Relevant file contents (truncated)
    #[input]
    pub file_context: String,

    /// Selected tool: "shell" | "file_read" | "file_edit" | "ripgrep" | "lsp"
    #[output]
    pub tool: String,

    /// JSON tool parameters (command, path, content, etc.)
    #[output]
    pub params: String,

    /// What we expect to learn or change
    #[output]
    pub expected_outcome: String,

    /// Step completion estimate (0.0-1.0)
    #[output]
    pub progress: f32,

    /// Should we stop and ask the human?
    #[output]
    pub needs_user_input: bool,
}

// Usage:
let sig = ToolCallSignature::new();
let predictor = Predict::new(sig);

let result = predictor.forward(example! {
    "step" : "input" => "Run tests to identify failures",
    "step_intent" : "input" => "Verify",
    "available_tools" : "input" => r#"["shell", "file_read", "ripgrep"]"#,
    "execution_history" : "input" => "[]",
    "file_context" : "input" => ""
}).await?;

let tool = result.get("tool", None).as_str();  // "shell"
let params = result.get("params", None);        // {"command": "cargo test"}
```

**Why merged:**
- Previous: ExecutionStrategySignature chose action + params, ToolSelectionSignature re-chose tool + params
- Now: Single decision point, no redundant work, clearer training signal

### ToolResultSignature (Learning Signal)

Interprets tool results and computes step utility. This is the **learning signal gold** that enables outcome-coupled optimization.

```rust
use dsrs::signatures::{ToolResultSignature, ToolSuccess};

/// Interprets tool execution results for learning.
#[Signature]
pub struct ToolResultSignature {
    /// Tool that was executed
    #[input]
    pub tool: String,

    /// Parameters used
    #[input]
    pub params: String,

    /// Tool output (truncated stdout/stderr, max 2000 chars)
    #[input]
    pub output: String,

    /// Exit code (0 = success for shell commands)
    #[input]
    pub exit_code: i32,

    /// Original step intent
    #[input]
    pub step_intent: String,

    /// Did the tool succeed? "YES" | "PARTIAL" | "NO"
    #[output]
    pub success: String,

    /// JSON: facts extracted from output (errors found, files identified, etc.)
    #[output]
    pub extracted_facts: String,

    /// Should we continue with this step or move on?
    #[output]
    pub should_continue: bool,

    /// Step utility score: -1.0 to +1.0 (THE LEARNING SIGNAL)
    #[output]
    pub step_utility: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolSuccess {
    Yes,
    Partial,
    No,
}
```

**Step utility metric:**

| Score | Meaning | Example |
|-------|---------|---------|
| +1.0 | Directly advances goal | Found the bug, tests now pass |
| +0.5 | Partial progress | Narrowed down to 3 files |
| 0.0 | No-op | Search returned nothing, no harm |
| -0.5 | Wasted effort | Repeated same search, opened same file again |
| -1.0 | Made things worse | Broke the build, added more test failures |

**Integration with training:**

```rust
// After each tool execution:
let result = tool_result_predictor.forward(/* ... */).await?;
let utility: f32 = result.get("step_utility", None).as_f64().unwrap_or(0.0) as f32;

// Record for MIPRO optimization
training_collector.record_tool_call(LabeledToolCall {
    signature: "ToolCallSignature".into(),
    inputs: tool_call_inputs,
    outputs: tool_call_outputs,
    step_utility: utility,
    verification_delta: prev_failing_tests - curr_failing_tests,
    cost_tokens: tokens_used,
    cost_tool_calls: 1,
    was_repeated: history.contains(&tool_call_hash),
});
```

### Execution Flow Comparison

**v1 (Current):** 2 LLM calls per step
```
ExecutionStrategySignature → next_action, action_params
ToolSelectionSignature     → selected_tool, tool_params   ← REDUNDANT
Tool Execution
(no interpretation)
```

**v2 (Proposed):** 2 LLM calls per step, but second is for learning
```
ToolCallSignature      → tool, params, expected_outcome
Tool Execution
ToolResultSignature    → success, extracted_facts, step_utility   ← LEARNING SIGNAL
```

**Net effect:**
- Same call count, but second call now captures outcome signal
- Training data includes step_utility for better MIPRO optimization
- Reduces format-only metrics, adds outcome-coupled metrics

## Using Signatures

### With Predict

```rust
use dsrs::predictors::Predict;
use dsrs::signatures::QueryComposerSignature;

let predictor = Predict::new(QueryComposerSignature::new());

let result = predictor.forward(example! {
    "goal" : "input" => "Find where authentication errors are handled",
    "failure_log" : "input" => "",
    "previous_queries" : "input" => vec![]
}).await?;

let queries: Vec<String> = serde_json::from_value(result.get("queries", None))?;
```

### With Refine (Retry/Fallback)

```rust
use dsrs::predictors::{Predict, Refine};
use dsrs::signatures::RetrievalRouterSignature;

let predictor = Predict::new(RetrievalRouterSignature::new());

let refined = Refine::new(predictor)
    .with_max_retries(3)
    .with_threshold(0.8)
    .with_reward_fn(|_inputs, prediction| {
        // Score the lane choice
        let lane = prediction.get("lane", None).as_str().unwrap_or("");
        if lane.is_empty() { 0.0 } else { 1.0 }
    });
```

### Optimizing Signatures

```rust
use dsrs::optimizer::MIPROv2;

let mut predictor = Predict::new(QueryComposerSignature::new());

let optimizer = MIPROv2::new()
    .with_num_candidates(10)
    .with_max_iterations(5);

optimizer.optimize(&mut predictor, trainset, evaluator).await?;
```

## Custom Signatures

Implement `MetaSignature` for custom signatures:

```rust
use dsrs::core::signature::MetaSignature;

#[derive(Debug, Clone)]
pub struct MySignature {
    instruction: String,
    demos: Vec<Example>,
}

impl MetaSignature for MySignature {
    fn instruction(&self) -> String { self.instruction.clone() }
    fn input_fields(&self) -> Value { /* ... */ }
    fn output_fields(&self) -> Value { /* ... */ }
    fn demos(&self) -> Vec<Example> { self.demos.clone() }
    fn set_demos(&mut self, demos: Vec<Example>) -> Result<()> { /* ... */ }
    fn update_instruction(&mut self, instruction: String) -> Result<()> { /* ... */ }
    fn append(&mut self, name: &str, value: Value) -> Result<()> { /* ... */ }
}
```
