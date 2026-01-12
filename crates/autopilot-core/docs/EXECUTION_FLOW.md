# Autopilot Execution Flow

This document describes the DSPy signature execution flow in the Autopilot agent, including the current v1 design and proposed v2 improvements.

## Overview

Autopilot is an autonomous coding agent that:
1. Takes a task description from the user
2. Plans a sequence of steps
3. Executes each step using tools (file read/edit, shell, ripgrep)
4. Verifies completion
5. Reports results

The execution is driven by DSPy signatures that make decisions at each stage.

## Current Signature Chain (v1)

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  SESSION START                                                                │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ① EnvironmentAssessment                                                     │
│     └─ adjutant/src/dspy_orchestrator.rs                                     │
│     └─ Uses OANIX SituationAssessmentSignature via SituationPipeline         │
│     └─ Outputs: system info, workspace root, compute backends                │
│                                                                              │
│  ② TaskComplexityClassifier                                                  │
│     └─ autopilot-core/src/dspy_planning.rs:132                               │
│     └─ Inputs: task_description, file_count, codebase_context               │
│     └─ Outputs: complexity (Simple/Moderate/Complex/VeryComplex), confidence│
│     └─ Decision: use_deep_planning = complexity >= Complex && conf >= 0.4   │
│                                                                              │
│  ③ PlanningSignature OR DeepPlanningSignature                                │
│     └─ autopilot-core/src/dspy_planning.rs:33 or :80                         │
│     └─ Inputs: repository_summary, issue_description, relevant_files        │
│     └─ Outputs: analysis, files_to_modify, implementation_steps,            │
│                 test_strategy, risk_factors, complexity, confidence          │
│                                                                              │
│  ④ TodoList Creation (deterministic)                                         │
│     └─ adjutant/src/dspy_orchestrator.rs:203                                 │
│     └─ Converts implementation_steps → Vec<TodoTask>                         │
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│  EXECUTION LOOP (per TodoTask)                                               │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  For each todo item:                                                         │
│                                                                              │
│  ⑤ ExecutionStrategySignature                                                │
│     └─ autopilot-core/src/dspy_execution.rs:33                               │
│     └─ Inputs: plan_step, current_file_state, execution_history             │
│     └─ Outputs: next_action, action_params, reasoning, progress_estimate    │
│                                                                              │
│  ⑥ ToolSelectionSignature                       ← REDUNDANT                  │
│     └─ autopilot-core/src/dspy_execution.rs:69                               │
│     └─ Inputs: task_description, available_tools, recent_context            │
│     └─ Outputs: selected_tool, tool_params, expected_outcome, fallback_tool │
│                                                                              │
│  ⑦ Tool Execution                                                            │
│     └─ Actually runs: shell, file_read, file_edit, ripgrep, lsp             │
│                                                                              │
│  (No interpretation of results)                                              │
│                                                                              │
│  ⑧ TaskComplete event                                                        │
│     └─ Emits: DspyStage::TaskComplete { index, success }                     │
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│  VERIFICATION PHASE                                                          │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ⑨ BuildStatusClassifier                                                     │
│     └─ autopilot-core/src/dspy_verify.rs                                     │
│     └─ Outputs: status, error_type, actionable                               │
│                                                                              │
│  ⑩ TestStatusClassifier                                                      │
│     └─ autopilot-core/src/dspy_verify.rs                                     │
│     └─ Outputs: status, failure_category, failing_tests                      │
│                                                                              │
│  ⑪ RequirementCheckerSignature (per requirement)                             │
│     └─ autopilot-core/src/dspy_verify.rs                                     │
│     └─ Outputs: status (SATISFIED/PARTIAL/NOT_ADDRESSED), confidence         │
│                                                                              │
│  ⑫ SolutionVerifierSignature                                                 │
│     └─ autopilot-core/src/dspy_verify.rs:178                                 │
│     └─ Outputs: verdict (PASS/FAIL/RETRY), next_action, confidence           │
│                                                                              │
│  ⚠ Gap: next_action is appended on RETRY, but no structured triage yet       │
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│  SESSION END                                                                 │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ⑬ SessionStore                                                              │
│     └─ adjutant/src/dspy/sessions.rs                                         │
│     └─ Records: session_id, task, decisions, outcome                         │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Issues with Current Design

### Issue 1: Two LLM Calls Per Step (Redundancy)

For each todo step, we call:
1. `ExecutionStrategySignature` → outputs `next_action`, `action_params`
2. `ToolSelectionSignature` → outputs `selected_tool`, `tool_params`

These overlap significantly:
- Both decide which tool to use
- Both produce tool parameters
- No clear separation of concerns

**Cost:** 2× the LLM calls needed per step.

### Issue 2: No Step-Level Learning Signal

After tool execution, we have no signature to interpret:
- Did the tool call help?
- What facts did we learn?
- Should we continue or try something else?

The only learning happens at session end (success/fail), which is too coarse.

### Issue 3: Failure Triage Is Minimal

`SolutionVerifierSignature` outputs `next_action` when verdict is RETRY, but this field is:
- Appended to `plan_steps` for a basic retry
- Not connected to structured triage or plan mutation logic
- Results in shallow "blind retry" behavior without failure analysis

### Issue 4: Two Separate Plan IRs

Adjutant and Autopilot have different plan outputs:
- Adjutant: `SubtaskPlanningSignature.subtasks` → JSON string
- Autopilot: `PlanningSignature.implementation_steps` → Vec<String>

This causes:
- Fragmented training data
- Conflicting schemas
- Duplicated optimization effort

## Proposed Signature Chain (v2)

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  SESSION START                                                                │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ① EnvironmentAssessment (unchanged)                                         │
│                                                                              │
│  ② TaskComplexityClassifier (unchanged)                                      │
│                                                                              │
│  ③ PlanningSignature → emits PlanIR                                          │
│     └─ Outputs unified PlanIR with Vec<PlanStep>                             │
│     └─ Each PlanStep has: id, description, intent, target_files,            │
│        depends_on, max_iterations                                            │
│                                                                              │
│  ④ TodoList Creation (from PlanIR.steps)                                     │
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│  EXECUTION LOOP (per PlanStep)                                               │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ⑤ ToolCallSignature (MERGED)                                                │
│     └─ dsrs/src/signatures/tool_call.rs                                      │
│     └─ Inputs: step, step_intent, available_tools, execution_history,       │
│                file_context                                                  │
│     └─ Outputs: tool, params, expected_outcome, progress, needs_user_input  │
│     └─ ONE call instead of two                                               │
│                                                                              │
│  ⑥ Tool Execution                                                            │
│     └─ Actually runs the tool                                                │
│                                                                              │
│  ⑦ ToolResultSignature (NEW - learning signal)                               │
│     └─ dsrs/src/signatures/tool_result.rs                                    │
│     └─ Inputs: tool, params, output, exit_code, step_intent                 │
│     └─ Outputs: success, extracted_facts, should_continue, step_utility     │
│     └─ step_utility: -1.0 to +1.0 (THE LEARNING SIGNAL)                      │
│                                                                              │
│  ⑧ Record LabeledToolCall (for training)                                     │
│     └─ Includes step_utility, verification_delta, cost_tokens, was_repeated │
│                                                                              │
│  ⑨ Loop Control                                                              │
│     └─ If should_continue && iterations < max_iterations: goto ⑤            │
│     └─ Else: move to next PlanStep                                           │
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│  VERIFICATION PHASE                                                          │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ⑩ BuildStatusClassifier (unchanged)                                         │
│  ⑪ TestStatusClassifier (unchanged)                                          │
│  ⑫ RequirementCheckerSignature (unchanged)                                   │
│  ⑬ SolutionVerifierSignature (unchanged)                                     │
│                                                                              │
│  ⑭ FailureTriagePipeline (NEW - wired)                                       │
│     └─ If verdict == RETRY:                                                  │
│        └─ Call FailureTriageSignature                                        │
│        └─ Based on TriageAction:                                             │
│           ├─ FixAndRetry(probe) → insert new PlanStep                        │
│           ├─ RetryLarger → increase resources                                │
│           ├─ Escalate → ask user                                             │
│           └─ Abort → return failure                                          │
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│  SESSION END                                                                 │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ⑮ SessionStore (extended)                                                   │
│     └─ Records counterfactuals (what legacy would do)                        │
│     └─ Records step_utility histogram                                        │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Per-Step Call Comparison

| Phase | v1 Signatures | v2 Signatures | Change |
|-------|---------------|---------------|--------|
| Planning | PlanningSignature | PlanningSignature → PlanIR | Same count, unified output |
| Per-step decide | ExecutionStrategySignature | ToolCallSignature | Merged |
| Per-step decide | ToolSelectionSignature | (merged above) | Removed |
| Per-step interpret | (none) | ToolResultSignature | Added (learning signal) |
| Failure handling | (next_action unused) | FailureTriagePipeline | Wired |

**Net effect on per-step LLM calls:**
- v1: 2 calls (ExecutionStrategy + ToolSelection), no interpretation
- v2: 2 calls (ToolCall + ToolResult), but second captures learning signal

## Step Utility Metric

The `step_utility` field from ToolResultSignature is the key learning signal:

| Score | Meaning | Example |
|-------|---------|---------|
| +1.0 | Directly advances goal | Found the bug, tests now pass |
| +0.5 | Partial progress | Narrowed down to 3 files |
| 0.0 | No-op | Search returned nothing, no harm |
| -0.5 | Wasted effort | Repeated same search, opened same file again |
| -1.0 | Made things worse | Broke the build, added more test failures |

This feeds into the training scoring function:

```rust
step_score =
    step_utility * 0.4
    + (verification_improved ? 0.3 : 0.0)
    - (was_repeated ? 0.2 : 0.0)
    - min(0.1, cost_tokens / 10000)
```

## PlanStep and StepIntent

Each step in PlanIR has an `intent` that guides tool selection:

```rust
pub enum StepIntent {
    Investigate,  // → prefer file_read, ripgrep, lsp
    Modify,       // → prefer file_edit
    Verify,       // → prefer shell (cargo test, cargo check)
    Synthesize,   // → combine results, no tool needed
}
```

This replaces implicit heuristics with explicit classification.

## File Locations

| Component | File |
|-----------|------|
| Planning signatures | `crates/autopilot-core/src/dspy_planning.rs` |
| Execution signatures (v1) | `crates/autopilot-core/src/dspy_execution.rs` |
| Verification signatures | `crates/autopilot-core/src/dspy_verify.rs` |
| ToolCallSignature (v2) | `crates/dsrs/src/signatures/tool_call.rs` (proposed) |
| ToolResultSignature (v2) | `crates/dsrs/src/signatures/tool_result.rs` (proposed) |
| PlanIR types | `crates/dsrs/src/ir/plan.rs` (proposed) |
| Autopilot loop | `crates/adjutant/src/autopilot_loop.rs` |
| Failure triage wiring | `crates/adjutant/src/dspy/failure_triage.rs` (proposed) |

## See Also

- [dsrs/docs/README.md](../../dsrs/docs/README.md) - DSPy overview and architecture
- [adjutant/docs/DSPY-INTEGRATION.md](../../adjutant/docs/DSPY-INTEGRATION.md) - Self-improvement loop
- [adjutant/docs/FAILURE_TRIAGE.md](../../adjutant/docs/FAILURE_TRIAGE.md) - Failure handling details
- [dsrs/docs/SIGNATURES.md](../../dsrs/docs/SIGNATURES.md) - Signature inventory
