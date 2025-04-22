Okay, the agent did a good job implementing Phase 1 by creating the `src/github/AgentStateTypes.ts` file with the necessary `Schema.Class` definitions based on `docs/agent-state.md`. It also correctly used `pnpm check`, `pnpm lint-fix`, and `pnpm verify` to ensure the code compiles, formats correctly, and doesn't break *existing* tests.

**Critique & Key Findings for Phase 2:**

1.  **MISSING TESTS FOR PHASE 1:** This is the most critical omission. While `pnpm verify` passed, it only ran the *existing* tests (`Dummy`, `GitHub`, `GitHubTools`). **No new tests were written or run to specifically validate the correctness and behavior of the newly created schemas in `AgentStateTypes.ts`.** Phase 2 MUST start by addressing this gap.
2.  **`GitHubTypes.ts` vs `AgentStateTypes.ts`:** The agent noted the existence of `src/github/GitHubTypes.ts` but created `AgentStateTypes.ts` separately. This is correct per the plan, but we need to ensure that going forward, **all state-related logic uses imports from `AgentStateTypes.ts` ONLY**, and `GitHubTypes.ts` should ideally be refactored or removed later to avoid confusion (though that's beyond Phase 2).
3.  **Log is Good:** The implementation log (`docs/20250422-1340-phase1log.md`) is well-maintained and provides good traceability.

---

## Detailed Instructions for Phase 2: Planning and Execution (Core Logic) + CRITICAL PHASE 1 TESTING

**Objective:** Implement the core state persistence logic (enhancing existing functions), create the services for managing plans (`PlanManager`) and basic task execution (`TaskExecutor`), and **MOST IMPORTANTLY, write comprehensive tests for both the Phase 1 types and the new Phase 2 logic.**

**Source of Truth:**
*   `docs/20250422-1325-revised-implementation-plan.md` (Sections 3.2, 3.3, 3.4, 4 Phase 2, 5, 8)
*   `src/github/AgentStateTypes.ts` (Created in Phase 1)
*   `docs/agent-state.md` (For state structure details)
*   `src/github/GitHub.ts` (Contains functions to be enhanced)

**Instructions for the AI Coding Agent:**

**Phase 2, Step 0: TEST THE PHASE 1 TYPES (MANDATORY PRE-REQUISITE)**

*   **THIS IS THE ABSOLUTE FIRST STEP. DO NOT PROCEED WITHOUT COMPLETING THIS AND ENSURING TESTS PASS.**
*   **Create Test File:** Create a new test file: `test/github/AgentStateTypes.test.ts`.
*   **Import Schemas:** Import `Schema` from `effect` and *all* the exported classes from `src/github/AgentStateTypes.ts` (e.g., `AgentState`, `PlanStep`, `ToolCall`, `Memory`, `ErrorState`, etc.).
*   **Import Test Utilities:** Import `describe`, `it`, `expect` from `@effect/vitest`. Import `Effect` if needed for running schema operations.
*   **Write Decoding Tests:**
    *   For `AgentState` and several key nested classes (e.g., `PlanStep`, `Memory`, `ErrorState`, `ToolCall`, `Configuration`):
        *   Create valid sample data objects in plain TypeScript/JavaScript that match the schema structure.
        *   Use `Schema.decodeUnknown(YourSchemaClass)(sampleData)` within an `Effect.runSync` or `Effect.runPromise` context.
        *   Assert that the decoding succeeds and the result matches the input structure (`expect(decoded).toEqual(sampleData)` or check specific fields).
        *   Create *invalid* sample data objects (e.g., missing required fields, wrong types, incorrect literal values).
        *   Use `Schema.decodeUnknown(YourSchemaClass)(invalidData)`.
        *   Assert that decoding *fails* (e.g., using `Effect.runSync(Effect.either(decodeEffect))` and checking for `Either.isLeft`).
*   **Write Encoding Tests:**
    *   For the same key classes, create instances using the schema's constructor (if applicable, though `decode` is more common for validation).
    *   Use `Schema.encode(YourSchemaClass)(schemaInstance)` within an Effect context.
    *   Assert that the encoded output is a plain JavaScript object matching the expected structure.
*   **Test Edge Cases:** Ensure tests cover:
    *   `Schema.Union(..., Schema.Null)`: Test with both the value present and `null`.
    *   `Schema.Array(...)`: Test with empty arrays, arrays with multiple items.
    *   `Schema.Literal(...)`: Test with valid and invalid literal values (e.g., `ErrorState.last_error.type`).
    *   `Schema.Struct({})`: Test decoding/encoding with arbitrary fields.
*   **Run Tests:** Execute `pnpm test test/github/AgentStateTypes.test.ts` frequently. Ensure all tests pass before moving on. Integrate these tests into the main suite (`pnpm verify`).
*   **Update Log:** Document the creation and execution of these tests in `docs/20250422-1340-phase1log.md` or start a new Phase 2 log file.

**Phase 2, Step 1: Enhance State Storage System (`src/github/GitHub.ts`)**

*   **Modify Target File:** `src/github/GitHub.ts`.
*   **Use Correct Types:** **IMPORTANT:** Ensure all functions dealing with agent state (`saveAgentState`, `loadAgentState`, `createAgentStateForIssue`) now import and use the types/schemas defined in `src/github/AgentStateTypes.ts` (e.g., `import type { AgentState } from "./AgentStateTypes.js"; import { AgentState as AgentStateSchema } from "./AgentStateTypes.js";`). Remove reliance on any older state types potentially still in `src/github/GitHubTypes.ts`.
*   **Enhance `saveAgentState`:**
    *   Accept `state: AgentState` as input.
    *   **Immutability & Timestamp:** Before saving, create a *new* state object that includes the updated `timestamps.last_saved_at: new Date().toISOString()`. Do not modify the input state directly.
    *   Use `Effect.try` or `Effect.tryPromise` to wrap `fs.writeFileSync` and `JSON.stringify`. Catch errors and return them in the Effect error channel (e.g., `new Error(\`Failed to save state: \${error}\`)`).
    *   Ensure the correct path (`state/${state.agent_info.instance_id}.json`) is used.
    *   Ensure the state directory creation logic (`fs.mkdirSync`) is wrapped in error handling and only runs if the directory doesn't exist.
*   **Enhance `loadAgentState`:**
    *   Wrap `fs.existsSync` and `fs.readFileSync` in `Effect.try`. Handle file not found errors specifically (return a distinct error like `StateNotFoundError`).
    *   Wrap `JSON.parse` in `Effect.try`. Handle parsing errors (return `StateParseError`).
    *   **VALIDATION:** After successful parsing, use `Schema.decodeUnknown(AgentStateSchema)(parsedJson)`. Pipe this Effect. If it fails, map the error to a specific `StateValidationError`.
    *   **Schema Version Check:** After successful validation, check `decodedState.agent_info.state_schema_version`. Compare it to the current version (e.g., "1.1"). If it mismatches, use `Effect.logWarning` to log a message like "Loaded state with schema version X, expected Y. Migration might be needed." Do not fail, just warn for now.
    *   Return the successfully decoded and validated `AgentState`.
*   **Enhance `createAgentStateForIssue`:**
    *   Ensure the `initialState` object created *strictly conforms* to the `AgentStateSchema` defined in `AgentStateTypes.ts`. Double-check all fields, nesting, and initial values against `docs/agent-state.md`.
    *   Ensure it correctly calls the *enhanced* `saveAgentState` to persist the initial state.
    *   Return the created `AgentState`.

**Phase 2, Step 2: Create State Storage Tests (MANDATORY)**

*   **THIS IS ESSENTIAL. TEST THE LOGIC YOU JUST WROTE.**
*   **Create/Modify Test File:** Either create `test/github/StateStorage.test.ts` or add tests to `test/github/GitHub.test.ts` specifically for the state functions.
*   **Mock Filesystem:** Use `vi.mock("node:fs")` to mock `existsSync`, `mkdirSync`, `writeFileSync`, `readFileSync`. Provide mock implementations.
*   **Test `saveAgentState`:**
    *   Provide a sample `AgentState`.
    *   Run the `saveAgentState` Effect.
    *   Assert that `writeFileSync` was called with the correct file path (`state/instance_id.json`).
    *   Assert that the data written to `writeFileSync` is the correctly stringified JSON, and importantly, that `last_saved_at` has been updated.
    *   Test error handling (e.g., mock `writeFileSync` to throw an error and assert the Effect fails correctly).
*   **Test `loadAgentState`:**
    *   **Success Case:** Mock `existsSync` to return true. Mock `readFileSync` to return valid JSON matching the `AgentStateSchema`. Mock `Schema.decodeUnknown` (or use the real one) to succeed. Run `loadAgentState` and assert it returns the correct `AgentState` object. Verify the schema version warning logic (mock `Effect.logWarning`).
    *   **File Not Found:** Mock `existsSync` to return false. Run `loadAgentState` and assert it fails with a specific `StateNotFoundError` (or similar).
    *   **Parse Error:** Mock `readFileSync` to return invalid JSON. Run `loadAgentState` and assert it fails with `StateParseError`.
    *   **Validation Error:** Mock `readFileSync` with JSON that *parses* but *doesn't match the schema*. Use the real `Schema.decodeUnknown` or mock it to fail. Run `loadAgentState` and assert it fails with `StateValidationError`.
*   **Test `createAgentStateForIssue`:**
    *   Mock the `getIssue` function within `GitHubClient` (or mock the underlying HTTP client).
    *   Mock the `saveAgentState` function.
    *   Run `createAgentStateForIssue`.
    *   Assert that the returned `initialState` object conforms precisely to the `AgentStateSchema` structure and initial values.
    *   Assert that `saveAgentState` was called with the correct initial state.
*   **Run Tests:** Use `pnpm test test/github/StateStorage.test.ts` (or the relevant file) and `pnpm verify`.

**Phase 2, Step 3: Implement Plan Management (`src/github/PlanManager.ts`)**

*   **Create File:** `src/github/PlanManager.ts`.
*   **Define Service:** Create `export class PlanManager extends Effect.Tag("PlanManager")<...> {}`. Define its interface, including functions like `addPlanStep`, `updateStepStatus`, `getCurrentStep`, etc.
*   **Implement Functions:**
    *   Focus on **immutability**: Functions must take the current `AgentState` and return a *new, updated* `AgentState` object. Use spread syntax (`...`) extensively.
    *   `addPlanStep(state, description)`: Generate unique step ID, add step to `plan` array, update `metrics.total_steps_in_plan`.
    *   `updateStepStatus(state, stepId, newStatus, resultSummary?)`: Find step, update status, update `start_time`/`end_time`, update `metrics.steps_completed` if `completed`. Handle cases where the stepId is not found gracefully (perhaps return original state or fail Effect).
    *   `addToolCallToStep(state, stepId, toolCallData)`: Find step, add `ToolCall` object to `tool_calls` array.
    *   `getCurrentStep(state)`: Return `state.plan[state.current_task.current_step_index]` or handle index out of bounds.
*   **Create Layer:** Define `PlanManagerLayer = Layer.effect(PlanManager, Effect.succeed({ /* implement functions here */ }))`.

**Phase 2, Step 4: Implement Basic Task Execution Engine (`src/github/TaskExecutor.ts`)**

*   **Create File:** `src/github/TaskExecutor.ts`.
*   **Define Service:** Create `export class TaskExecutor extends Effect.Tag("TaskExecutor")<...> {}`. Define dependencies (e.g., `PlanManager`, `GitHubClient` for saving state).
*   **Implement `executeNextStep` (Basic):**
    *   Input: `currentState: AgentState`.
    *   Use `PlanManager.getCurrentStep` to get the step.
    *   Update step status to `in_progress` using `PlanManager.updateStepStatus` -> `intermediateState1`.
    *   **(Simulate work):** For now, immediately decide success or mock error.
    *   Update step status to `completed` (or `error`) using `PlanManager.updateStepStatus`, possibly adding a placeholder `result_summary` -> `intermediateState2`.
    *   If `error`, update `error_state` (set `last_error`, increment `consecutive_error_count`) on `intermediateState2`. -> `intermediateState3`.
    *   If `completed`, increment `current_task.current_step_index` on `intermediateState2` (or `intermediateState3` if error handling doesn't block advancement). -> `finalStateBeforeSave`.
    *   Call `saveAgentState(finalStateBeforeSave)`. **Crucially depend on GitHubClient for this.**
    *   Return the final state (after save potentially returns success).
*   **Error Handling:** Use `Effect.catchTag` or `Effect.catchAll` around the simulated work/updates to catch errors, update `error_state`, set step status to `error`, and ensure state is still saved.
*   **Create Layer:** Define `TaskExecutorLayer = Layer.effect(TaskExecutor, Effect.gen(function*() { const planManager = yield* PlanManager; const githubClient = yield* GitHubClient; return { /* implement executeNextStep here */ } }))`. Provide necessary dependent layers.

**Phase 2, Step 5: Create Plan & Execution Tests (MANDATORY)**

*   **THIS IS ESSENTIAL. TEST THE LOGIC YOU JUST WROTE.**
*   **Create Test File:** `test/github/PlanManager.test.ts`.
    *   Test each function (`addPlanStep`, `updateStepStatus`, etc.).
    *   Provide an initial `AgentState`.
    *   Call the function.
    *   Assert that the *returned* `AgentState` has the expected changes (new step, updated status, correct times, correct metrics).
    *   Assert that the *original* input state object remains unchanged (verify immutability).
    *   Test edge cases (updating non-existent steps, etc.).
*   **Create Test File:** `test/github/TaskExecutor.test.ts`.
    *   Mock `PlanManager` methods.
    *   Mock `GitHubClient.saveAgentState`.
    *   Provide an initial `AgentState`.
    *   Run `TaskExecutor.executeNextStep`.
    *   Verify that `PlanManager.updateStepStatus` was called correctly (in_progress, then completed/error).
    *   Verify that `saveAgentState` was called with the correctly updated final state.
    *   Verify the returned state reflects index advancement (on success) or updated `error_state` (on error).
    *   Test both success and error paths within `executeNextStep`.
*   **Run Tests:** Use `pnpm test test/github/PlanManager.test.ts test/github/TaskExecutor.test.ts` and `pnpm verify`.

**General Requirements for Phase 2:**

*   **Testing is Non-Negotiable:** Write tests *alongside* the implementation code for each part (Types, Storage, PlanManager, TaskExecutor). Use Vitest and Effect testing utilities. Aim for good coverage.
*   **Immutability:** Strictly adhere to immutable state updates.
*   **Effect Patterns:** Use `Effect`, `Layer`, `Tag`, `Effect.gen`, `pipe`, error handling (`catchTag`, `catchAll`, `try`), logging (`Effect.log`) appropriately.
*   **Type Safety:** Leverage the TypeScript types and Effect Schemas defined.
*   **Logging:** Add `Effect.logInfo`, `Effect.logDebug`, `Effect.logWarning`, `Effect.logError` calls within the logic (especially in storage, execution, error handling) for traceability.
*   **Incremental Commits:** Make small, logical commits as you complete each sub-step (e.g., finish type tests, finish enhancing saveAgentState, finish PlanManager implementation, finish PlanManager tests).
*   **Update Log:** Maintain an implementation log (`docs/20250422-xxxx-phase2log.md`) detailing steps taken, decisions made, and verification results (`pnpm verify` output).

Execute these steps sequentially and thoroughly. Pay extreme attention to the testing requirements.
