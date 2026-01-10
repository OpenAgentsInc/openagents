# DSPy Signatures Catalog

Complete inventory of all DSPy signatures across the OpenAgents codebase.

## Overview

DSPy signatures are typed input/output contracts for LLM tasks. Each signature:
- Declares inputs and outputs with types
- Uses docstrings as prompt instructions
- Field names become part of the generated prompt
- Can be optimized via MIPROv2/GEPA

## Signature Index

| Wave | Crate | Signature | Purpose | Status |
|------|-------|-----------|---------|--------|
| 2 | adjutant | SubtaskPlanningSignature | Break tasks into subtasks | Complete |
| 2 | adjutant | SubtaskExecutionSignature | Execute subtasks | Complete |
| 2 | adjutant | ResultSynthesisSignature | Synthesize results | Complete |
| 2 | adjutant | ComplexityClassifier | Classify task complexity | Complete |
| 2 | adjutant | DelegationDecider | Decide delegation path | Complete |
| 2 | adjutant | RlmTriggerDecider | Trigger RLM analysis | Complete |
| 8 | oanix | SituationAssessmentSignature | Assess current situation | Complete |
| 8 | oanix | IssueSelectionSignature | Select issue to work on | Complete |
| 10 | runtime | ToolSelectionSignature | Select tool for task | Complete |
| 10 | runtime | ToolChainPlanningSignature | Plan tool chains | Complete |
| 10 | runtime | ToolResultInterpretationSignature | Interpret tool results | Complete |
| 12 | frlm | FRLMDecomposeSignature | Decompose into subcalls | Complete |
| 12 | frlm | FRLMAggregateSignature | Aggregate worker results | Complete |
| — | dsrs | QueryComposerSignature | Compose retrieval queries | Complete |
| — | dsrs | RetrievalRouterSignature | Route queries to lanes | Complete |
| — | dsrs | CandidateRerankSignature | Rerank retrieval results | Complete |
| — | dsrs | ChunkTaskSignature | Select chunks to process | Complete |
| — | dsrs | ChunkAggregatorSignature | Aggregate chunk results | Complete |
| — | dsrs | SandboxProfileSignature | Select sandbox profile | Complete |
| — | dsrs | FailureTriageSignature | Diagnose sandbox failures | Complete |
| — | dsrs | LaneBudgeterSignature | Budget allocation | Complete |
| — | dsrs | AgentMemorySignature | Memory management | Complete |
| **16** | rlm | RlmQuerySignature | Basic RLM query | Complete |
| **16** | rlm | RlmContextQuerySignature | Context-aware RLM | Complete |
| **16** | rlm | RlmGuidedQuerySignature | Guided tier RLM | Complete |
| **16** | rlm | RlmCodeGenerationSignature | Generate REPL code | Complete |
| **16** | rlm | RlmContinuationSignature | Handle continuation | Complete |
| **19** | autopilot | TaskComplexityClassifier | Replace keyword heuristics | Complete |
| **19** | autopilot | BuildStatusClassifier | Detect build failures | Complete |
| **19** | autopilot | TestStatusClassifier | Detect test failures | Complete |
| **19** | autopilot | PathValidationSignature | Validate file paths | Complete |
| **19** | autopilot | ActionableStepSignature | Detect actionable language | Complete |
| **20** | agent-orchestrator | DirectiveStatusParser | Parse directive status | Complete |
| **20** | agent-orchestrator | DirectivePriorityClassifier | Classify directive priority | Complete |
| **20** | agent-orchestrator | DirectiveMatchingSignature | Semantic directive matching | Complete |
| **20** | agent-orchestrator | IssueSelectionSignature | Prioritize issues | Complete |
| **20** | nexus | EventIntentClassifier | Classify event intent | Complete |
| **20** | nexus | JobKindClassifier | Classify NIP-90 job types | Complete |
| **21** | marketplace | SkillSecurityClassifier | Classify skill risk | Planned |
| **21** | marketplace | FilesystemPermissionSignature | Learn safe permissions | Planned |
| **21** | marketplace | ResourceLimitSignature | Learn resource limits | Planned |
| **21** | marketplace | SafePathValidationSignature | Learn path safety | Planned |

---

## Complete Signatures (Waves 0-14)

### Adjutant: Tiered Execution (Wave 2)

```rust
// crates/adjutant/src/dspy/module.rs

#[Signature]
struct SubtaskPlanningSignature {
    /// Task Planner: Break the given task into concrete, atomic subtasks.
    /// Output ONLY valid JSON with a list of subtasks.
    /// Each subtask must have: id, action (read/edit/bash), target (file path), instruction.
    /// Keep subtasks atomic and focused. Order logically: read before edit, edit before test.
    /// Maximum 5 subtasks per task.

    #[input] pub task_title: String,
    #[input] pub task_description: String,
    #[input] pub context: String,
    #[output] pub subtasks: String,      // JSON array
    #[output] pub reasoning: String,
    #[output] pub confidence: f32,
}

#[Signature]
struct SubtaskExecutionSignature {
    /// Subtask Executor: Perform the given action and return the result.
    /// For "edit" actions: output JSON with old_string (exact text to find) and new_string (replacement).
    /// For "bash" actions: output JSON with command field.
    /// For "read" actions: summarize what you learned about the file.

    #[input] pub action: String,
    #[input] pub target: String,
    #[input] pub instruction: String,
    #[input] pub file_context: String,
    #[output] pub result: String,
    #[output] pub reasoning: String,
    #[output] pub success: bool,
}

#[Signature]
struct ResultSynthesisSignature {
    /// Synthesis Agent: Given subtask results, determine if the overall task succeeded.
    /// Provide a concise but informative summary of what was accomplished or what failed.

    #[input] pub task_title: String,
    #[input] pub subtask_results: String,
    #[output] pub success: bool,
    #[output] pub summary: String,
    #[output] pub modified_files: String,
    #[output] pub confidence: f32,
}
```

### Adjutant: Decision Pipelines (Wave 2)

```rust
// crates/adjutant/src/dspy/decision_pipelines.rs

#[Signature]
struct ComplexityClassifier {
    /// Classify the complexity of a coding task.
    /// Consider: number of files, scope of changes, architectural impact.

    #[input] pub task_description: String,
    #[input] pub file_count: u32,
    #[input] pub codebase_summary: String,
    #[output] pub complexity: String,     // Low/Medium/High/VeryHigh
    #[output] pub reasoning: String,
    #[output] pub confidence: f32,
}

#[Signature]
struct DelegationDecider {
    /// Decide the best execution path for a task.
    /// Options: claude_code (complex coding), rlm (recursive analysis), local_tools (simple operations).

    #[input] pub task_description: String,
    #[input] pub complexity: String,
    #[input] pub available_tools: String,
    #[output] pub delegation: String,     // claude_code/rlm/local_tools
    #[output] pub reasoning: String,
    #[output] pub confidence: f32,
}

#[Signature]
struct RlmTriggerDecider {
    /// Decide if recursive language model analysis is needed.
    /// RLM is useful for: large context, multi-document analysis, complex reasoning chains.

    #[input] pub query: String,
    #[input] pub context_size: u64,
    #[input] pub task_type: String,
    #[output] pub trigger_rlm: bool,
    #[output] pub reasoning: String,
    #[output] pub confidence: f32,
}
```

### OANIX: Situation Assessment (Wave 8)

```rust
// crates/oanix/src/dspy_situation.rs

#[Signature]
struct SituationAssessmentSignature {
    /// Assess the current situation and determine priority action.
    /// Consider: environment state, pending tasks, resource availability.

    #[input] pub environment_state: String,
    #[input] pub pending_tasks: String,
    #[input] pub recent_actions: String,
    #[output] pub urgency: String,        // Immediate/Normal/Deferred
    #[output] pub priority_action: String,
    #[output] pub reasoning: String,
}
```

### Runtime: Tool Selection (Wave 10)

```rust
// crates/runtime/src/dspy_tools.rs

#[Signature]
struct ToolSelectionSignature {
    /// Select the best tool for a given task from available options.
    /// Consider: task requirements, tool capabilities, efficiency.

    #[input] pub task: String,
    #[input] pub available_tools: String,  // JSON array
    #[input] pub context: String,
    #[output] pub selected_tool: String,
    #[output] pub reasoning: String,
    #[output] pub confidence: f32,
}

#[Signature]
struct ToolChainPlanningSignature {
    /// Plan a sequence of tool invocations to complete a complex task.

    #[input] pub task: String,
    #[input] pub available_tools: String,
    #[output] pub tool_chain: String,      // JSON array of steps
    #[output] pub reasoning: String,
}

#[Signature]
struct ToolResultInterpretationSignature {
    /// Interpret the result of a tool execution.

    #[input] pub tool_name: String,
    #[input] pub tool_output: String,
    #[input] pub expected_outcome: String,
    #[output] pub success: bool,
    #[output] pub interpretation: String,
    #[output] pub next_action: String,
}
```

### FRLM: Federated RLM (Wave 12)

```rust
// crates/frlm/src/dspy_signatures.rs

#[Signature]
struct FRLMDecomposeSignature {
    /// Decompose a query into subcalls for parallel processing.
    /// Decide chunking strategy and stopping rule.

    #[input] pub query: String,
    #[input] pub context_spans: String,    // JSON array of spans
    #[input] pub budget: u32,
    #[output] pub subcalls: String,        // JSON array
    #[output] pub stopping_rule: String,   // Exhaustive/SufficientEvidence/BudgetExhausted/ConfidenceThreshold
    #[output] pub span_selector: String,   // All/ByType/ByRelevance/ByPosition
}

#[Signature]
struct FRLMAggregateSignature {
    /// Aggregate results from parallel worker executions.

    #[input] pub query: String,
    #[input] pub worker_results: String,   // JSON array
    #[output] pub final_answer: String,
    #[output] pub confidence: f32,
    #[output] pub sources_used: String,
}
```

### DSRS: Built-in Signatures

```rust
// crates/dsrs/src/signatures/

#[Signature]
struct QueryComposerSignature {
    /// Compose a retrieval query from a user goal.

    #[input] pub goal: String,
    #[input] pub available_sources: String,
    #[output] pub query: String,
    #[output] pub target_sources: String,
}

#[Signature]
struct RetrievalRouterSignature {
    /// Route a query to the appropriate retrieval lane.

    #[input] pub query: String,
    #[input] pub available_lanes: String,
    #[output] pub selected_lane: String,
    #[output] pub reasoning: String,
}

#[Signature]
struct CandidateRerankSignature {
    /// Rerank retrieval candidates by relevance.

    #[input] pub query: String,
    #[input] pub candidates: String,
    #[output] pub ranked_candidates: String,
    #[output] pub relevance_scores: String,
}
```

---

## Planned Signatures (Waves 15-21)

### Wave 16: RLM DSPy Integration

```rust
// crates/rlm/src/dspy.rs (NEW)

#[Signature]
struct RlmQuerySignature {
    /// Execute a simple RLM query with code execution.
    /// Generate Python REPL code to solve the problem.
    /// Use FINAL to provide the answer.

    #[input] pub query: String,
    #[output] pub reasoning: String,
    #[output] pub code: String,
    #[output] pub needs_continuation: bool,
}

#[Signature]
struct RlmContextQuerySignature {
    /// RLM query with context for recursive analysis.
    /// Generate Python REPL code that uses llm_query() for sub-queries.
    /// Use FINAL(answer) or FINAL_VAR(variable) when done.

    #[input] pub query: String,
    #[input] pub context_length: u64,
    #[input] pub context_source: String,
    #[output] pub reasoning: String,
    #[output] pub code: String,
    #[output] pub needs_continuation: bool,
}

#[Signature]
struct RlmGuidedQuerySignature {
    /// Guided RLM query for models like Apple FM.
    /// Generate simple Python without imports.
    /// No llm_query() - direct string operations only.

    #[input] pub query: String,
    #[input] pub context_length: u64,
    #[input] pub context_preview: String,
    #[output] pub code: String,
    #[output] pub explanation: String,
}

#[Signature]
struct RlmCodeGenerationSignature {
    /// Generate Python REPL code for context analysis.
    /// Use only built-in Python. No imports.

    #[input] pub task: String,
    #[input] pub context_preview: String,
    #[output] pub code: String,
    #[output] pub explanation: String,
}
```

### Wave 19: Autopilot Heuristics

```rust
// crates/autopilot/src/dspy_planning.rs
// crates/autopilot/src/dspy_verify.rs
// crates/autopilot/src/dspy_optimization.rs

#[Signature]
struct TaskComplexityClassifier {
    /// Classify task complexity for planning depth.
    /// Replaces keyword-based heuristics with learned classification.

    #[input] pub task_description: String,
    #[input] pub file_count: u32,
    #[input] pub codebase_context: String,
    #[output] pub complexity: String,      // Simple/Moderate/Complex/VeryComplex
    #[output] pub reasoning: String,
    #[output] pub confidence: f32,
}

#[Signature]
struct BuildStatusClassifier {
    /// Classify build output status.
    /// Learn nuanced build failure patterns.

    #[input] pub build_output: String,
    #[input] pub command: String,
    #[output] pub status: String,          // Success/Warning/Error/Fatal
    #[output] pub error_type: String,      // CompileError/LinkError/ConfigError/etc
    #[output] pub actionable: bool,
}

#[Signature]
struct TestStatusClassifier {
    /// Classify test output status.
    /// Learn test failure patterns and categories.

    #[input] pub test_output: String,
    #[input] pub test_framework: String,
    #[output] pub status: String,          // Pass/Fail/Skip/Error
    #[output] pub failure_category: String,
    #[output] pub failing_tests: String,   // JSON array
}

#[Signature]
struct PathValidationSignature {
    /// Validate file paths for correctness.
    /// Learn valid path patterns for the codebase.

    #[input] pub path: String,
    #[input] pub codebase_root: String,
    #[output] pub valid: bool,
    #[output] pub reason: String,
}

#[Signature]
struct ActionableStepSignature {
    /// Detect if a step description is actionable.
    /// Learn action verb patterns and specificity.

    #[input] pub step: String,
    #[output] pub actionable: bool,
    #[output] pub suggested_improvement: String,
}
```

### Wave 20: Agent-Orchestrator & Nexus

```rust
// crates/agent-orchestrator/src/integrations/directives.rs
// crates/agent-orchestrator/src/integrations/autopilot.rs

#[Signature]
struct DirectiveStatusParser {
    /// Parse directive status from text.

    #[input] pub directive_text: String,
    #[output] pub status: String,          // Active/Pending/Complete/Blocked
    #[output] pub confidence: f32,
}

#[Signature]
struct DirectivePriorityClassifier {
    /// Classify directive priority from context.

    #[input] pub directive_text: String,
    #[input] pub context: String,
    #[output] pub priority: String,        // Critical/High/Medium/Low
    #[output] pub reasoning: String,
}

#[Signature]
struct DirectiveMatchingSignature {
    /// Semantic matching between a directive and a query.

    #[input] pub directive_text: String,
    #[input] pub query: String,
    #[output] pub matches: bool,
    #[output] pub confidence: f32,
    #[output] pub reasoning: String,
}

#[Signature]
struct IssueSelectionSignature {
    /// Select next issue to work on.
    /// Learn issue prioritization from agent capabilities and recent work.

    #[input] pub open_issues: String,      // JSON array
    #[input] pub agent_capabilities: String,
    #[input] pub recent_work: String,
    #[output] pub selected_issue_id: String,
    #[output] pub reasoning: String,
    #[output] pub estimated_complexity: String,
}

// crates/nexus/src/dspy.rs (NEW)

#[Signature]
struct EventIntentClassifier {
    /// Classify Nostr event intent from content.
    /// Learn semantic event classification beyond kind numbers.

    #[input] pub event_kind: u32,
    #[input] pub event_content: String,
    #[input] pub event_tags: String,
    #[output] pub intent: String,          // JobRequest/JobResult/StatusUpdate/etc
    #[output] pub priority: String,
    #[output] pub requires_response: bool,
}

#[Signature]
struct JobKindClassifier {
    /// Classify NIP-90 job types.

    #[input] pub job_content: String,
    #[input] pub job_params: String,
    #[output] pub job_type: String,
    #[output] pub complexity: String,
}
```

### Wave 21: Marketplace Security

```rust
// crates/marketplace/src/dspy_security.rs (NEW)

#[Signature]
struct SkillSecurityClassifier {
    /// Classify security risk of a skill execution.
    /// High-risk only requires human approval.

    #[input] pub skill_manifest: String,
    #[input] pub requested_permissions: String,
    #[input] pub execution_context: String,
    #[output] pub risk_level: String,      // Low/Medium/High/Critical
    #[output] pub concerns: String,        // JSON array of concerns
    #[output] pub recommended_sandbox: String,
    #[output] pub requires_approval: bool,
}

#[Signature]
struct FilesystemPermissionSignature {
    /// Learn safe filesystem permissions for skill types.

    #[input] pub skill_type: String,
    #[input] pub requested_paths: String,
    #[input] pub operation: String,        // Read/Write/Execute
    #[output] pub allowed: bool,
    #[output] pub reasoning: String,
}

#[Signature]
struct ResourceLimitSignature {
    /// Learn appropriate resource limits for skills.

    #[input] pub skill_type: String,
    #[input] pub requested_limits: String,
    #[output] pub approved_limits: String,
    #[output] pub adjustments: String,
}

#[Signature]
struct SafePathValidationSignature {
    /// Learn path safety patterns.
    /// Detect path traversal and unsafe access.

    #[input] pub path: String,
    #[input] pub base_directory: String,
    #[output] pub safe: bool,
    #[output] pub reason: String,
}
```

---

## Signature Design Guidelines

### 1. Field Naming
Field names become part of the prompt. Use descriptive names:
```rust
// Good
#[input] pub task_description: String,
#[output] pub confidence: f32,

// Bad
#[input] pub desc: String,
#[output] pub conf: f32,
```

### 2. Docstrings
The struct docstring becomes the system prompt. Be specific:
```rust
/// Task Planner: Break the given task into concrete, atomic subtasks.
/// Output ONLY valid JSON with a list of subtasks.
/// Maximum 5 subtasks per task.
```

### 3. Output Types
- Use `String` for JSON outputs (validated at runtime)
- Use `bool` for binary decisions
- Use `f32` for confidence scores (0.0 to 1.0)

### 4. Confidence Fields
Always include a confidence output for decisions:
```rust
#[output] pub confidence: f32,
```

This enables:
- Fallback to legacy logic when confidence < threshold
- Training data filtering (only collect high-confidence examples)
- A/B testing between DSPy and legacy paths

---

## File Locations

| Crate | Signatures File |
|-------|-----------------|
| dsrs | `crates/dsrs/src/signatures/*.rs` |
| adjutant | `crates/adjutant/src/dspy/module.rs`, `decision_pipelines.rs` |
| oanix | `crates/oanix/src/dspy_*.rs` |
| runtime | `crates/runtime/src/dspy_tools.rs` |
| frlm | `crates/frlm/src/dspy_signatures.rs` |
| rlm | `crates/rlm/src/dspy.rs` (Wave 16) |
| autopilot | `crates/autopilot/src/dspy_planning.rs`, `crates/autopilot/src/dspy_verify.rs`, `crates/autopilot/src/dspy_optimization.rs` (Wave 19) |
| agent-orchestrator | `crates/agent-orchestrator/src/integrations/directives.rs`, `crates/agent-orchestrator/src/integrations/autopilot.rs` (Wave 20) |
| nexus | `crates/nexus/src/dspy.rs` (Wave 20) |
| marketplace | `crates/marketplace/src/dspy_security.rs` (Wave 21) |
