```

⏺ Autopilot DSPy Signature Execution Flow

  Based on the codebase exploration, here's the complete signature execution order for an example session:

  Example Session: autopilot run "Fix the failing tests"

  ┌──────────────────────────────────────────────────────────────────────────┐
  │  SESSION START                                                            │
  ├──────────────────────────────────────────────────────────────────────────┤
  │                                                                          │
  │  ① ENVIRONMENT ASSESSMENT (adjutant/src/dspy_orchestrator.rs)            │
  │     └─ Extracts: CPU cores, RAM, workspace root, compute backends        │
  │     └─ Emits: DspyStage::EnvironmentAssessment                           │
  │                                                                          │
  │  ② TASK COMPLEXITY CLASSIFIER (autopilot/src/dspy_planning.rs:370)       │
  │     ┌─────────────────────────────────────────────────────────────────┐  │
  │     │ Input:                                                          │  │
  │     │   task_description: "Fix the failing tests"                     │  │
  │     │   file_count: 12                                                │  │
  │     │   codebase_context: "Rust project with 50 crates..."            │  │
  │     │ Output:                                                         │  │
  │     │   complexity: "Moderate"                                        │  │
  │     │   reasoning: "Test failures usually localized..."               │  │
  │     │   confidence: 0.78                                              │  │
  │     └─────────────────────────────────────────────────────────────────┘  │
  │     └─ Decision: use_deep_planning = false                               │
  │                                                                          │
  │  ③ PLANNING SIGNATURE (autopilot/src/dspy_planning.rs)                   │
  │     ┌─────────────────────────────────────────────────────────────────┐  │
  │     │ Input:                                                          │  │
  │     │   repository_summary: "OpenAgents monorepo..."                  │  │
  │     │   issue_description: "Fix the failing tests"                    │  │
  │     │   relevant_files: "src/lib.rs\nsrc/tests.rs\n..."               │  │
  │     │ Output:                                                         │  │
  │     │   analysis: "Tests failing due to assertion mismatch..."        │  │
  │     │   files_to_modify: ["src/lib.rs"]                               │  │
  │     │   implementation_steps: [                                       │  │
  │     │     "Run cargo test to identify failures",                      │  │
  │     │     "Read failing test file",                                   │  │
  │     │     "Analyze assertion mismatch",                               │  │
  │     │     "Fix implementation logic",                                 │  │
  │     │     "Verify with cargo test"                                    │  │
  │     │   ]                                                             │  │
  │     │   test_strategy: "cargo test"                                   │  │
  │     │   confidence: 0.85                                              │  │
  │     └─────────────────────────────────────────────────────────────────┘  │
  │     └─ Emits: DspyStage::Planning                                        │
  │                                                                          │
  │  ④ TODO LIST CREATION (adjutant/src/dspy_orchestrator.rs:203)            │
  │     └─ Creates 5 TodoTasks from implementation_steps                     │
  │     └─ Emits: DspyStage::TodoList                                        │
  │                                                                          │
  ├──────────────────────────────────────────────────────────────────────────┤
  │  EXECUTION LOOP (adjutant/src/autopilot_loop.rs:849)                     │
  ├──────────────────────────────────────────────────────────────────────────┤
  │                                                                          │
  │  For each todo item, this signature chain executes:                      │
  │                                                                          │
  │  ─── TODO #1: "Run cargo test to identify failures" ───                  │
  │                                                                          │
  │  ⑤ EXECUTION STRATEGY SIGNATURE (autopilot/src/dspy_execution.rs)        │
  │     ┌─────────────────────────────────────────────────────────────────┐  │
  │     │ Input:                                                          │  │
  │     │   plan_step: "Run cargo test to identify failures"              │  │
  │     │   current_file_state: null                                      │  │
  │     │   execution_history: "[]"                                       │  │
  │     │ Output:                                                         │  │
  │     │   next_action: "RUN_COMMAND"                                    │  │
  │     │   action_params: {"command": "cargo test"}                      │  │
  │     │   reasoning: "Need to see which tests are failing"              │  │
  │     │   progress_estimate: 0.2                                        │  │
  │     └─────────────────────────────────────────────────────────────────┘  │
  │                                                                          │
  │  ⑥ TOOL SELECTION SIGNATURE (autopilot/src/dspy_execution.rs)            │
  │     ┌─────────────────────────────────────────────────────────────────┐  │
  │     │ Input:                                                          │  │
  │     │   task_description: "Run cargo test"                            │  │
  │     │   available_tools: ["shell", "file_read", "file_edit", "git"]   │  │
  │     │   recent_context: "[]"                                          │  │
  │     │ Output:                                                         │  │
  │     │   selected_tool: "shell"                                        │  │
  │     │   tool_params: {"command": "cargo test 2>&1"}                   │  │
  │     │   expected_outcome: "Get test failure output"                   │  │
  │     └─────────────────────────────────────────────────────────────────┘  │
  │                                                                          │
  │     └─ TOOL EXECUTION: cargo test 2>&1                                   │
  │     └─ Emits: DspyStage::TaskComplete { index: 1, success: true }        │
  │                                                                          │
  │  ─── TODO #2: "Read failing test file" ───                               │
  │                                                                          │
  │  ⑦ EXECUTION STRATEGY SIGNATURE                                          │
  │     └─ next_action: "READ_FILE"                                          │
  │                                                                          │
  │  ⑧ TOOL SELECTION SIGNATURE                                              │
  │     └─ selected_tool: "file_read"                                        │
  │     └─ tool_params: {"path": "src/tests.rs"}                             │
  │                                                                          │
  │  ─── TODO #3: "Analyze assertion mismatch" ───                           │
  │                                                                          │
  │  ⑨ EXECUTION STRATEGY SIGNATURE                                          │
  │     └─ next_action: "ANALYZE" (internal reasoning)                       │
  │                                                                          │
  │  ─── TODO #4: "Fix implementation logic" ───                             │
  │                                                                          │
  │  ⑩ EXECUTION STRATEGY SIGNATURE                                          │
  │     └─ next_action: "EDIT_FILE"                                          │
  │                                                                          │
  │  ⑪ TOOL SELECTION SIGNATURE                                              │
  │     └─ selected_tool: "file_edit"                                        │
  │     └─ tool_params: {"path": "src/lib.rs", "edit": "..."}                │
  │                                                                          │
  │  ─── TODO #5: "Verify with cargo test" ───                               │
  │                                                                          │
  │  ⑫ EXECUTION STRATEGY SIGNATURE                                          │
  │     └─ next_action: "RUN_COMMAND"                                        │
  │                                                                          │
  │  ⑬ TOOL SELECTION SIGNATURE                                              │
  │     └─ selected_tool: "shell"                                            │
  │     └─ tool_params: {"command": "cargo test"}                            │
  │                                                                          │
  ├──────────────────────────────────────────────────────────────────────────┤
  │  VERIFICATION PHASE (autopilot/src/dspy_verify.rs:561)                   │
  ├──────────────────────────────────────────────────────────────────────────┤
  │                                                                          │
  │  ⑭ BUILD STATUS CLASSIFIER                                               │
  │     ┌─────────────────────────────────────────────────────────────────┐  │
  │     │ Input: build_output (cargo build output)                        │  │
  │     │ Output:                                                         │  │
  │     │   status: "Success"                                             │  │
  │     │   error_type: null                                              │  │
  │     │   actionable: false                                             │  │
  │     └─────────────────────────────────────────────────────────────────┘  │
  │                                                                          │
  │  ⑮ TEST STATUS CLASSIFIER                                                │
  │     ┌─────────────────────────────────────────────────────────────────┐  │
  │     │ Input: test_output (cargo test output)                          │  │
  │     │ Output:                                                         │  │
  │     │   status: "Passed"                                              │  │
  │     │   failure_category: null                                        │  │
  │     │   failing_tests: []                                             │  │
  │     └─────────────────────────────────────────────────────────────────┘  │
  │                                                                          │
  │  ⑯ REQUIREMENT CHECKER SIGNATURE (per requirement)                       │
  │     ┌─────────────────────────────────────────────────────────────────┐  │
  │     │ Input:                                                          │  │
  │     │   requirement: "All tests should pass"                          │  │
  │     │   solution_summary: "Fixed assertion in lib.rs"                 │  │
  │     │   code_changes: (git diff)                                      │  │
  │     │ Output:                                                         │  │
  │     │   status: "SATISFIED"                                           │  │
  │     │   explanation: "Tests now pass after fix"                       │  │
  │     │   confidence: 0.92                                              │  │
  │     └─────────────────────────────────────────────────────────────────┘  │
  │                                                                          │
  │  ⑰ SOLUTION VERIFIER SIGNATURE (final verdict)                           │
  │     ┌─────────────────────────────────────────────────────────────────┐  │
  │     │ Input:                                                          │  │
  │     │   requirements: ["All tests should pass"]                       │  │
  │     │   requirement_results: [{"status": "SATISFIED", ...}]           │  │
  │     │   build_status: "SUCCESS"                                       │  │
  │     │   test_status: "PASSED"                                         │  │
  │     │ Output:                                                         │  │
  │     │   verdict: "PASS"                                               │  │
  │     │   explanation: "All requirements satisfied"                     │  │
  │     │   confidence: 0.95                                              │  │
  │     └─────────────────────────────────────────────────────────────────┘  │
  │                                                                          │
  ├──────────────────────────────────────────────────────────────────────────┤
  │  SESSION END                                                             │
  ├──────────────────────────────────────────────────────────────────────────┤
  │                                                                          │
  │  ⑱ SESSION STORE (adjutant/src/dspy/sessions.rs)                         │
  │     └─ Records: session_id, task, all decisions, outcome: Success        │
  │     └─ Saved to: ~/.openagents/adjutant/sessions/                        │
  │     └─ Used for MIPROv2 self-improvement optimization                    │
  │                                                                          │
  └──────────────────────────────────────────────────────────────────────────┘

  Signature Count Summary

  | Phase                    | Signatures Executed                                                                                 | Count          |
  |--------------------------|-----------------------------------------------------------------------------------------------------|----------------|
  | Planning                 | TaskComplexityClassifier, PlanningSignature                                                         | 2              |
  | Execution (per todo × 5) | ExecutionStrategySignature, ToolSelectionSignature                                                  | 10             |
  | Verification             | BuildStatusClassifier, TestStatusClassifier, RequirementCheckerSignature, SolutionVerifierSignature | 4              |
  | Total                    |                                                                                                     | ~16 signatures |

  Key Files

  | File                            | Signatures                                                                                                                 |
  |---------------------------------|----------------------------------------------------------------------------------------------------------------------------|
  | autopilot/src/dspy_planning.rs  | TaskComplexityClassifier, PlanningSignature, DeepPlanningSignature                                                         |
  | autopilot/src/dspy_execution.rs | ExecutionStrategySignature, ToolSelectionSignature                                                                         |
  | autopilot/src/dspy_verify.rs    | BuildStatusClassifier, TestStatusClassifier, RequirementCheckerSignature, TestAnalyzerSignature, SolutionVerifierSignature |
  | adjutant/src/autopilot_loop.rs  | Orchestrates the loop, calls signatures                                                                                    |
  | adjutant/src/dspy/sessions.rs   | Records decisions for self-improvement
  ```
