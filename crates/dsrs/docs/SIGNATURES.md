# Signatures (Wave 4)

- **Status:** Needs audit (manual status table - unreliable until generator exists)
- **Last verified:** 634f5b627
- **Source of truth:** `crates/dsrs/src/signatures/`, `crates/adjutant/src/dspy/`
- **Doc owner:** dsrs
- **If this doc conflicts with code, code wins.**
>
- **Warning:** The Signature Index table at the end of this doc is manually maintained and may not reflect actual code. A signature index generator is planned to automate this.

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

### Execution Flow

> See [MODULES.md](MODULES.md#execution-flow-v2) for the canonical execution flow diagram.

The merged signature model (`ToolCallSignature` + `ToolResultSignature`) replaces the previous redundant two-signature approach and captures outcome signals for training.

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

---

## Adjutant Task Execution

Signatures for Adjutant's 3-phase tiered execution model.

**Module:** `AdjutantModule` in `crates/adjutant/src/dspy/module.rs`

### SubtaskPlanningSignature

Breaks a task into atomic subtasks. Used by the planning phase (GLM 4.7).

```rust
// File: crates/adjutant/src/dspy/module.rs:19-54

// Inputs:
// - task_title: Title of the task to accomplish
// - task_description: Detailed description of what needs to be done
// - context_handle: Context handle or summary reference for large contexts
// - context: Repository context including relevant file contents

// Outputs:
// - subtasks: JSON array with id, action (read/edit/bash), target, instruction
// - reasoning: Brief explanation of the planning approach
// - confidence: Confidence in the plan (0.0 to 1.0)

let module = AdjutantModule::new();
let plan = module.plan(
    "Fix authentication bug",
    "Users cannot log in after password reset",
    "inline",
    &file_contents
).await?;
```

### SubtaskExecutionSignature

Executes a single subtask. Used by the execution phase (Qwen-3-32B).

```rust
// File: crates/adjutant/src/dspy/module.rs:57-92

// Inputs:
// - action: Action type (read, edit, or bash)
// - target: Target file path (empty for bash actions)
// - instruction: Instruction describing what to do
// - file_context: Current file content (for read/edit actions)

// Outputs:
// - result: JSON with {old_string, new_string} for edit, {command} for bash
// - reasoning: Explanation of what was done
// - success: Whether the action completed successfully

let result = module.execute_subtask(
    "edit",
    "src/auth.rs",
    "Fix the password validation logic",
    &current_file_content
).await?;
```

### ResultSynthesisSignature

Synthesizes subtask results into final outcome. Used by synthesis phase (GLM 4.7).

```rust
// File: crates/adjutant/src/dspy/module.rs:95-124

// Inputs:
// - task_title: Original task title
// - subtask_results: Formatted results from all subtasks

// Outputs:
// - success: Overall success boolean
// - summary: Brief description of what was accomplished or failed
// - modified_files: JSON array of files that were modified
// - confidence: Confidence in the result assessment (0.0 to 1.0)

let synthesis = module.synthesize(
    "Fix authentication bug",
    &subtask_results_formatted
).await?;
```

---

## Adjutant Decision Routing

Signatures for intelligent task routing decisions.

**Location:** `crates/adjutant/src/dspy/decision_pipelines.rs`

### ComplexityClassificationSignature

Classifies task complexity level for routing decisions.

```rust
// File: crates/adjutant/src/dspy/decision_pipelines.rs:20-56

// Inputs:
// - task_description: Description of the task to classify
// - file_count: Number of files likely to be affected
// - estimated_tokens: Estimated token count for context
// - keywords: Keywords found in task (refactor, migrate, rewrite, etc.)

// Outputs:
// - complexity: Low, Medium, High, or VeryHigh
// - reasoning: Explanation of the classification
// - confidence: Confidence in classification (0.0 to 1.0)

// Complexity levels:
// - Low: Simple single-file edit, minimal risk
// - Medium: Multi-file edit, moderate scope
// - High: Complex refactoring, many files, architectural changes
// - VeryHigh: Massive scope, system-wide changes, high risk

let pipeline = ComplexityPipeline::new();
let result = pipeline.classify(&ComplexityInput {
    task_description: "Refactor auth module".to_string(),
    file_count: 5,
    estimated_tokens: 10000,
    keywords: vec!["refactor".to_string()],
}).await?;
```

### DelegationDecisionSignature

Decides whether to delegate task execution and to which target.

```rust
// File: crates/adjutant/src/dspy/decision_pipelines.rs:60-100

// Inputs:
// - task_description: Description of the task
// - complexity: Classified complexity level
// - file_count: Number of files involved
// - estimated_tokens: Estimated token count for context

// Outputs:
// - should_delegate: Whether to delegate this task
// - delegation_target: codex_code | rlm | local_tools
// - reasoning: Explanation of the delegation decision
// - confidence: Confidence in decision (0.0 to 1.0)

// Delegation targets:
// - codex_code: Complex multi-file tasks, architectural work
// - rlm: Large context analysis, recursive investigation
// - local_tools: Simple edits, small scope tasks

let pipeline = DelegationPipeline::new();
let result = pipeline.decide(&DelegationInput {
    task_description: "Fix login bug".to_string(),
    complexity: "Low".to_string(),
    file_count: 2,
    estimated_tokens: 5000,
}).await?;
```

### RlmTriggerSignature

Decides whether to use Recursive Language Model for deep analysis.

```rust
// File: crates/adjutant/src/dspy/decision_pipelines.rs:104-142

// Inputs:
// - task_description: Description of the task
// - complexity: Classified complexity level
// - estimated_tokens: Estimated token count for context
// - file_count: Number of files involved
// - repeated_actions: Whether recent actions show repetition or thrash

// Outputs:
// - use_rlm: Whether to use RLM for this task
// - reasoning: Explanation of the RLM decision
// - confidence: Confidence in decision (0.0 to 1.0)

// RLM is good for: deep code analysis, recursive investigation,
// security audits, comprehensive reviews, finding all occurrences
// RLM is overkill for: simple edits, single-file changes, well-scoped tasks

let pipeline = RlmTriggerPipeline::new();
let result = pipeline.should_trigger(&RlmTriggerInput {
    task_description: "Analyze security vulnerabilities".to_string(),
    complexity: "High".to_string(),
    estimated_tokens: 50000,
    file_count: 12,
    repeated_actions: true,
}).await?;
```

---

## Issue Management

Signatures for intelligent issue triage and suggestion.

### IssueValidationSignature

Validates if an issue is still accurate before agent starts work.

```rust
// File: crates/dsrs/src/signatures/issue_validation.rs

// Inputs:
// - issue_title: Title of the issue
// - issue_description: Full description of the issue
// - blocked_reason: Reason if issue is blocked (optional)
// - recent_commits: Recent git commits since issue was created
// - changed_files: Files that have changed recently

// Outputs:
// - is_valid: Whether the issue is still valid to work on
// - validation_status: VALID | ALREADY_ADDRESSED | STALE | NEEDS_UPDATE
// - reason: Explanation of the validation result
// - confidence: Confidence in validation (0.0 to 1.0)

use dsrs::signatures::IssueValidationSignature;
```

### IssueSuggestionSignature

Suggests top issues for an agent to work on.

```rust
// File: crates/dsrs/src/signatures/issue_suggestion.rs

// Inputs:
// - available_issues: JSON array of open issues
// - workspace_context: Current workspace state
// - recent_work: What the agent has been working on
// - user_preferences: User's stated preferences

// Outputs:
// - suggestions: JSON array of top 3 suggested issues with rationale
// - confidence: Overall confidence in suggestions

use dsrs::signatures::IssueSuggestionSignature;
```

### UnblockSuggestionSignature

Recommends which blocked issue to unblock first.

```rust
// File: crates/dsrs/src/signatures/unblock_suggestion.rs

// Inputs:
// - blocked_issues: JSON array of blocked issues
// - workspace_context: Current workspace state
// - recent_commits: Recent git activity

// Outputs:
// - selected_issue_number: Issue number to unblock
// - unblock_rationale: Why this issue should be unblocked
// - unblock_strategy: How to unblock it
// - estimated_effort: low | medium | high
// - cascade_potential: How many other issues this unblocks

use dsrs::signatures::UnblockSuggestionSignature;
```

### StalenessCheckSignature

Checks if an issue is still relevant given recent codebase changes.

```rust
// File: crates/adjutant/src/dspy/staleness.rs:23-63

// Inputs:
// - issue_title: Title of the issue
// - issue_description: Description of the issue
// - issue_type: Type of issue (bug, feature, task)
// - recent_commits: Recent git commits since issue was created
// - changed_files: Files that have changed recently

// Outputs:
// - is_relevant: Is this issue still relevant and actionable?
// - reason: Brief explanation of relevance assessment
// - recommendation: proceed | close | needs_update | blocked

use adjutant::dspy::staleness::check_issue_staleness;

let result = check_issue_staleness(&issue, &workspace_root).await?;
```

---

## Code Editing

Signatures for code generation and verification.

### CodeEditSignature

Generates code changes as unified diff patches.

```rust
// File: crates/dsrs/src/signatures/code_edit.rs

// Inputs:
// - file_path: Path to the file being edited
// - current_content: Current content of the file
// - edit_instruction: What changes to make
// - code_context: Surrounding code context

// Outputs:
// - unified_diff: The changes in unified diff format
// - edit_summary: Human-readable summary of changes
// - affected_lines: Lines that were modified
// - confidence: Confidence in the edit (0.0 to 1.0)

use dsrs::signatures::CodeEditSignature;
```

### TaskUnderstandingSignature

Parses user intent and extracts requirements.

```rust
// File: crates/dsrs/src/signatures/task_understanding.rs

// Inputs:
// - user_request: The user's original request
// - repo_context: Context about the repository

// Outputs:
// - task_type: FEATURE | BUGFIX | REFACTOR | DOCS | TEST
// - requirements: Extracted requirements list
// - scope_estimate: SMALL | MEDIUM | LARGE
// - clarifying_questions: Questions to ask user if unclear
// - confidence: Confidence in understanding (0.0 to 1.0)

use dsrs::signatures::TaskUnderstandingSignature;
```

### VerificationSignature

Verifies code changes meet requirements.

```rust
// File: crates/dsrs/src/signatures/verification.rs

// Inputs:
// - original_request: The user's original request
// - changes_made: Summary of changes that were made
// - test_output: Output from running tests

// Outputs:
// - verification_status: PASS | PARTIAL | FAIL
// - missing_requirements: Requirements not yet met
// - issues_found: Problems discovered during verification
// - suggested_fixes: How to fix the issues
// - confidence: Confidence in verification (0.0 to 1.0)

use dsrs::signatures::VerificationSignature;
```

---

## RLM Document Extraction

Signatures for provenance-first document analysis with span-based evidence tracking.

**Location:** `crates/rlm/src/signatures.rs`

### RouterSignature

Identifies relevant document sections given a query.

```rust
// File: crates/rlm/src/signatures.rs:34-54

// Inputs:
// - query: The user's question or information need
// - document_preview: First ~1000 chars of document for structure detection

// Outputs:
// - candidate_spans: JSON array of [{path, start_line, end_line, why}]
// - confidence: Confidence in routing decisions (0.0-1.0)

use rlm::signatures::RouterSignature;
```

### ExtractorSignature

Extracts findings from a chunk with chain-of-thought reasoning and provenance.

```rust
// File: crates/rlm/src/signatures.rs:65-93

#[Signature(cot)]  // Enables chain-of-thought reasoning
pub struct ExtractorSignature { ... }

// Inputs:
// - query: The user's question
// - chunk: Content of this chunk
// - span_ref: JSON-encoded SpanRef for this chunk

// Outputs:
// - findings: Extracted findings as structured text
// - evidence_spans: JSON array of SpanRefs within chunk supporting findings
// - relevance: Relevance score (0.0-1.0)
```

### SimpleExtractorSignature

Fast extraction without chain-of-thought (for speed-sensitive operations).

```rust
// File: crates/rlm/src/signatures.rs:96-119

// Same inputs as ExtractorSignature, but:
// - No chain-of-thought reasoning
// - Faster execution
// - Less thorough analysis

// Outputs:
// - findings: Extracted findings
// - relevance: Relevance score (0.0-1.0)
```

### ReducerSignature

Synthesizes findings from multiple chunks into a coherent answer with citations.

```rust
// File: crates/rlm/src/signatures.rs:130-154

// Inputs:
// - query: The user's question
// - findings: Combined findings from all chunks (section-labeled)
// - evidence_spans: JSON array of all evidence SpanRefs from extractions

// Outputs:
// - answer: Final synthesized answer
// - citations: JSON array of SpanRefs cited in the answer
// - confidence: Confidence in answer (0.0-1.0)
```

### VerifierSignature

Validates answers against cited evidence.

```rust
// File: crates/rlm/src/signatures.rs:168-199

// Inputs:
// - query: The user's question
// - answer: The proposed answer to verify
// - citations: JSON array of SpanRefs cited as evidence

// Outputs:
// - verdict: PASS | FAIL | PARTIAL
// - explanation: Explanation of the verdict
// - missing_spans: JSON array describing what evidence is missing
// - corrections: Suggested corrections if answer is incorrect
```

---

## FRLM Federated Extraction

Signatures for federated recursive language model operations (map-reduce pattern).

**Location:** `crates/frlm/src/dspy_signatures.rs`

### FRLMDecomposeSignature

Map phase: decides what subcalls to spawn over which spans.

```rust
// File: crates/frlm/src/dspy_signatures.rs:137-253

// Inputs:
// - query: The user query to process
// - env_summary: Summary of the environment/document
// - progress: Current progress state (what's been done)

// Outputs:
// - subqueries: JSON array of [{span_selector, question, schema}]
// - stopping_rule: When to stop recursing

let sig = FRLMDecomposeSignature::new();
```

### FRLMAggregateSignature

Reduce phase: merges worker results into final answer.

```rust
// File: crates/frlm/src/dspy_signatures.rs:273-390

// Inputs:
// - query: The original user query
// - worker_results: JSON array of worker outputs

// Outputs:
// - answer: The aggregated answer
// - citations: SpanRefs or doc IDs for evidence
// - confidence: Confidence score (0.0-1.0)

let sig = FRLMAggregateSignature::new();
```

### Supporting Enums

```rust
/// When to stop recursive decomposition
pub enum StoppingRule {
    Exhaustive,          // Process all spans
    SufficientEvidence,  // Stop when enough evidence found
    BudgetExhausted,     // Stop when budget depleted
    ConfidenceThreshold, // Stop when confidence exceeds threshold
}

/// Which spans to process
pub enum SpanSelector {
    All,                           // Select all spans
    ByType(String),                // Select by content type
    ByRelevance,                   // Select most relevant
    ByPosition { start, end },     // Select by position range
}
```

---

## Utility Signatures

### ToolStepUtilitySignature

Evaluates how useful a tool call was for the task (learning signal).

```rust
// File: crates/adjutant/src/dspy/tool_step_utility.rs:5-47

// Inputs:
// - tool_name: Name of the tool that executed
// - step_goal: Goal of this step in the overall task
// - inputs_summary: Deterministic summary of the tool inputs
// - outputs_summary: Deterministic summary of the tool outputs
// - receipt: JSON receipt for the tool call (hashes, latency, side effects)

// Outputs:
// - step_utility: Utility score (0.0 = no value, 1.0 = decisive progress)
// - should_continue: Whether the workflow should continue
// - next_action_hint: Short hint for the next action (max 12 words)
// - confidence: Confidence in the utility judgment (0.0 to 1.0)

use adjutant::dspy::tool_step_utility::tool_step_utility_predict;

let predictor = tool_step_utility_predict();
```

- **Naming collision note:** This signature outputs `step_utility` in range **0.0..1.0**, while `ToolResultSignature` outputs `step_utility` in range **-1.0..+1.0**. The canonical label for training/optimization is `ToolResultSignature.step_utility (-1..+1)`. `ToolStepUtilitySignature` serves as a utility judge that can be used to produce labels, but its output should be normalized or renamed to `step_utility_norm` to avoid confusion.

---

## MVP Critical Signatures

These signatures must be wired end-to-end for the MVP to function:

| # | Signature | Why Critical | Status |
|---|-----------|--------------|--------|
| 1 | **ToolCallSignature** | Single decision point for tool selection + params | Spec only |
| 2 | **ToolResultSignature** | Learning signal (step_utility) for MIPRO | Spec only |
| 3 | **SubtaskPlanningSignature** | Emits PlanIR for all task execution | Implemented |
| 4 | **SubtaskExecutionSignature** | Executes atomic plan steps | Implemented |
| 5 | **ResultSynthesisSignature** | Produces final PR_SUMMARY.md | Implemented |
| 6 | **IssueValidationSignature** | Gates stale/invalid work | Implemented |
| 7 | **VerificationSignature** | Computes verification_delta metric | Spec only |
| 8 | **ToolStepUtilitySignature** | Outcome-coupled scoring per tool call | Implemented |

**MVP Critical Path:** Issue → Plan → (ToolCall → Execute → ToolResult)* → Synthesis → PR_SUMMARY.md + RECEIPT.json

---

## Signature Index

Quick reference for all signatures by category:

| Category | Signature | Location | Status |
|----------|-----------|----------|--------|
| **Retrieval** | QueryComposerSignature | Spec in this doc | Spec only |
| | RetrievalRouterSignature | Spec in this doc | Spec only |
| | CandidateRerankSignature | Spec in this doc | Spec only |
| **Chunk Analysis** | ChunkTaskSelectorSignature | Spec in this doc | Spec only |
| | ChunkAnalysisToActionSignature | Spec in this doc | Spec only |
| **Sandbox** | SandboxProfileSelectionSignature | Spec in this doc | Spec only |
| | FailureTriageSignature | Spec in this doc | Spec only |
| **Budget** | LaneBudgeterSignature | Spec in this doc | Spec only |
| | AgentMemorySignature | Spec in this doc | Spec only |
| **Execution** | ToolCallSignature | Spec in this doc | Spec only |
| | ToolResultSignature | Spec in this doc | Spec only |
| **Adjutant** | SubtaskPlanningSignature | `adjutant/src/dspy/module.rs` | Implemented |
| | SubtaskExecutionSignature | `adjutant/src/dspy/module.rs` | Implemented |
| | ResultSynthesisSignature | `adjutant/src/dspy/module.rs` | Implemented |
| | ComplexityClassificationSignature | `adjutant/src/dspy/decision_pipelines.rs` | Implemented |
| | DelegationDecisionSignature | `adjutant/src/dspy/decision_pipelines.rs` | Implemented |
| | RlmTriggerSignature | `adjutant/src/dspy/decision_pipelines.rs` | Implemented |
| | StalenessCheckSignature | `adjutant/src/dspy/staleness.rs` | Implemented |
| | ToolStepUtilitySignature | `adjutant/src/dspy/tool_step_utility.rs` | Implemented |
| **Issues** | IssueValidationSignature | `adjutant/src/dspy/issue_validation.rs` | Implemented |
| | IssueSuggestionSignature | Spec in this doc | Spec only |
| | UnblockSuggestionSignature | Spec in this doc | Spec only |
| **Code Edit** | CodeEditSignature | Spec in this doc | Spec only |
| | TaskUnderstandingSignature | Spec in this doc | Spec only |
| | VerificationSignature | Spec in this doc | Spec only |
| **RLM** | RouterSignature | `rlm/src/signatures.rs` | Implemented |
| | ExtractorSignature | `rlm/src/signatures.rs` | Implemented |
| | SimpleExtractorSignature | `rlm/src/signatures.rs` | Implemented |
| | ReducerSignature | `rlm/src/signatures.rs` | Implemented |
| | VerifierSignature | `rlm/src/signatures.rs` | Implemented |
| **FRLM** | FRLMDecomposeSignature | `frlm/src/dspy_signatures.rs` | Implemented |
| | FRLMAggregateSignature | `frlm/src/dspy_signatures.rs` | Implemented |

**Status Legend:**
- **Implemented**: Signature struct exists in code and is called in production
- **In code**: Signature struct exists but not yet wired to production paths
- **Spec only**: Documented specification, code not yet written
