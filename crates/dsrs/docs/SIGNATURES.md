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
