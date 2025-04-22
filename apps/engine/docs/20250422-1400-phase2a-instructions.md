Okay, let's analyze the agent's work based on the summary provided.

**Analysis of Agent's Work (Based on Summary):**

1.  **Phase 1 Types Implementation:** Excellent. The agent correctly created `src/github/AgentStateTypes.ts` with all the specified `Schema.Class` definitions, handled JSDoc, and verified using linting and type checking.
2.  **Phase 1 Types Testing:** **CRITICAL & POSITIVE:** The agent *did* create `test/github/AgentStateTypes.test.ts` and implemented comprehensive tests (47 tests reported) covering schema validation for success and failure cases. This addresses the major gap identified previously. **This is a huge step forward.**
3.  **State Storage Enhancement (`GitHub.ts`):** Good. The agent enhanced `saveAgentState`, `loadAgentState`, and `createAgentStateForIssue` to use the new types, added specific error classes, implemented schema validation on load, added schema version warnings, and focused on immutability and error handling.
4.  **State Storage Testing (`StateStorage.test.ts`):** **Area for Improvement.** The agent noted these tests were *simplified* due to initial issues. While basic verification is present (checking error class definition, path structure), the tests likely *do not* cover the full range of success and failure scenarios for the enhanced `save/load/create` functions (e.g., mocking filesystem errors, testing parse errors, testing validation failures during load). This needs to be revisited and made more robust.
5.  **Dependency Updates:** Correctly updated `GitHubTools.ts` to import from the new types file.
6.  **Verification:** Successfully ran `pnpm verify`, indicating the code compiles, lints, and passes *all currently implemented tests*.
7.  **Logging:** Maintained implementation logs, which is good practice.

**Conclusion:** The agent made significant progress, correctly implementing the state types, testing them, and enhancing the core storage functions. The main remaining task from the initial Phase 2 instructions is implementing `PlanManager` and `TaskExecutor`, and critically, **bolstering the tests for the State Storage functions**.

---

## Detailed Instructions for Phase 2 (Continued): PlanManager, TaskExecutor & Testing

**Objective:** Implement the `PlanManager` service for state plan manipulation and the basic `TaskExecutor` service for orchestrating step execution. **Crucially, write comprehensive Vitest tests for both new services AND enhance the existing tests for the State Storage functions.**

**Source of Truth:**
*   `docs/20250422-1325-revised-implementation-plan.md` (Sections 3.3, 3.4, 4 Phase 2, 5, 8)
*   `src/github/AgentStateTypes.ts` (From previous step)
*   `src/github/GitHub.ts` (Enhanced in previous step)
*   `docs/agent-state.md`

**Instructions for the AI Coding Agent:**

**Phase 2, Step 3: Implement Plan Management (`src/github/PlanManager.ts`)**

*   **Create File:** Create the new file `src/github/PlanManager.ts`.
*   **Import Dependencies:** Import `Effect`, `Layer`, `Schema` (if needed), and relevant types from `./AgentStateTypes.ts` (e.g., `AgentState`, `PlanStep`, `ToolCall`).
*   **Define Service (`Effect.Tag`):**
    ```typescript
    import type { AgentState, PlanStep, ToolCall } from "./AgentStateTypes.js";
    import { Effect, Layer } from "effect";

    // Define the interface for the PlanManager service
    export interface PlanManager {
      readonly addPlanStep: (state: AgentState, description: string) => Effect.Effect<AgentState>;
      readonly updateStepStatus: (
        state: AgentState,
        stepId: string,
        newStatus: PlanStep["status"], // Use type from PlanStep
        resultSummary?: string | null
      ) => Effect.Effect<AgentState>;
      readonly addToolCallToStep: (
        state: AgentState,
        stepId: string,
        toolCallData: Omit<ToolCall, "timestamp"> // Exclude timestamp as it should be generated
      ) => Effect.Effect<AgentState>;
      readonly getCurrentStep: (state: AgentState) => Effect.Effect<PlanStep, Error>; // Can fail if index is invalid
    }

    // Create the Effect Tag
    export const PlanManager = Effect.Tag<PlanManager>("PlanManager");
    ```
*   **Implement Service Functions:** Create the functions defined in the interface. **ENSURE IMMUTABILITY** (always return a *new* state object using spread syntax).
    *   `addPlanStep`: Generate a unique step ID (e.g., `step-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`). Append the new `PlanStep` (status `pending`, correct `step_number`) to the `plan` array. Increment `metrics.total_steps_in_plan`. Return the new `AgentState`.
    *   `updateStepStatus`: Find the step in the `plan` array by `stepId`. If found, create a new step object with updated `status`, `start_time` (if status becomes `in_progress`), `end_time` (if status becomes `completed`, `skipped`, or `error`), and `result_summary`. If status becomes `completed`, increment `metrics.steps_completed`. Create a new `plan` array with the updated step. Return the new `AgentState`. If step not found, return `Effect.fail(new Error(\`Plan step with id \${stepId} not found\`))`.
    *   `addToolCallToStep`: Find the step by `stepId`. If found, create a new `ToolCall` object (adding the current `timestamp`). Create a new `tool_calls` array for the step including the new call. Create a new `plan` array. Return the new `AgentState`. Handle step not found error.
    *   `getCurrentStep`: Get `state.current_task.current_step_index`. Check if it's a valid index for `state.plan`. If valid, return `Effect.succeed(state.plan[index])`. If invalid, return `Effect.fail(new Error(\`Invalid current_step_index: \${index}\`))`.
*   **Create Layer:**
    ```typescript
    export const PlanManagerLayer = Layer.succeed(
      PlanManager,
      PlanManager.of({
        addPlanStep: (state, description) => Effect.sync(() => {
          // Implementation using immutable updates...
          const stepId = `step-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;
          const newStep: PlanStep = { /* ... fill details ... */ status: "pending", id: stepId, step_number: state.plan.length + 1, description, tool_calls: [], start_time: null, end_time: null, result_summary: null };
          return {
            ...state,
            plan: [...state.plan, newStep],
            metrics: { ...state.metrics, total_steps_in_plan: state.plan.length + 1 }
          };
        }),
        updateStepStatus: (state, stepId, newStatus, resultSummary = null) => Effect.sync(() => {
            const stepIndex = state.plan.findIndex(step => step.id === stepId);
            if (stepIndex === -1) {
                throw new Error(`Plan step with id ${stepId} not found`); // Throw inside sync or return Effect.fail outside
            }
            const now = new Date().toISOString();
            const updatedStep = {
                ...state.plan[stepIndex],
                status: newStatus,
                start_time: state.plan[stepIndex].start_time ?? (newStatus === "in_progress" ? now : null),
                end_time: (newStatus === "completed" || newStatus === "skipped" || newStatus === "error") ? now : null,
                result_summary: resultSummary
            };
            const updatedPlan = [...state.plan];
            updatedPlan[stepIndex] = updatedStep;
            const stepsCompleted = updatedPlan.filter(step => step.status === 'completed').length;

            return {
                ...state,
                plan: updatedPlan,
                metrics: { ...state.metrics, steps_completed: stepsCompleted }
            };
        }).pipe(Effect.catchAll(Effect.fail)), // Ensure errors are in Effect channel
        addToolCallToStep: (state, stepId, toolCallData) => Effect.sync(() => {
            const stepIndex = state.plan.findIndex(step => step.id === stepId);
            if (stepIndex === -1) {
                 throw new Error(`Plan step with id ${stepId} not found`);
            }
            const newToolCall: ToolCall = {
                ...toolCallData,
                timestamp: new Date().toISOString()
            };
            const originalStep = state.plan[stepIndex];
            const updatedStep = {
                ...originalStep,
                tool_calls: [...originalStep.tool_calls, newToolCall]
            };
            const updatedPlan = [...state.plan];
            updatedPlan[stepIndex] = updatedStep;
            return { ...state, plan: updatedPlan };
        }).pipe(Effect.catchAll(Effect.fail)),
        getCurrentStep: (state) => {
          const index = state.current_task.current_step_index;
          if (index >= 0 && index < state.plan.length) {
            return Effect.succeed(state.plan[index]);
          } else {
            return Effect.fail(new Error(`Invalid current_step_index: ${index}`));
          }
        }
      })
    );
    ```

**Phase 2, Step 4: TEST Plan Management (MANDATORY & CONCURRENT)**

*   **Create Test File:** `test/github/PlanManager.test.ts`.
*   **Import Dependencies:** `@effect/vitest`, `Effect`, `PlanManager`, `PlanManagerLayer`, test fixtures for `AgentState`.
*   **Write Tests:**
    *   Create a basic valid `AgentState` fixture for testing.
    *   Test `addPlanStep`: Provide state, call `addPlanStep`, assert the returned state has one more step in `plan`, `metrics.total_steps_in_plan` is incremented, and the new step has correct details (ID, pending status, description). **Verify immutability** (original state object is unchanged).
    *   Test `updateStepStatus`: Provide state, update a step to `in_progress`, check status and `start_time`. Update to `completed`, check status, `end_time`, and `metrics.steps_completed`. Update to `error`, check status and `end_time`. Test updating a non-existent step ID and assert the Effect fails. Verify immutability.
    *   Test `addToolCallToStep`: Provide state, add a tool call to a step, assert the returned state has the tool call added to the correct step's `tool_calls` array. Verify immutability. Test adding to a non-existent step.
    *   Test `getCurrentStep`: Provide state with a valid `current_step_index`, assert it returns the correct `PlanStep`. Provide state with an invalid index, assert the Effect fails.
*   **Run Tests Frequently:** Use `pnpm test test/github/PlanManager.test.ts` and `pnpm verify`.

**Phase 2, Step 5: Implement Basic Task Execution Engine (`src/github/TaskExecutor.ts`)**

*   **Create File:** `src/github/TaskExecutor.ts`.
*   **Import Dependencies:** `Effect`, `Layer`, `Tag`, `PlanManager`, `GitHubClient` (specifically for `saveAgentState`), `AgentState`, error types from `GitHub.ts` (like `StateStorageError`).
*   **Define Service (`Effect.Tag`):**
    ```typescript
    import type { AgentState } from "./AgentStateTypes.js";
    import { Effect, Layer } from "effect";
    import { PlanManager } from "./PlanManager.js";
    import { GitHubClient } from "./GitHub.js"; // Need this for saveAgentState

    export interface TaskExecutor {
       readonly executeNextStep: (currentState: AgentState) => Effect.Effect<AgentState, Error>; // Can fail due to various reasons
    }

    export const TaskExecutor = Effect.Tag<TaskExecutor>("TaskExecutor");
    ```
*   **Implement `executeNextStep` (Basic Logic Only):**
    ```typescript
    // Inside the Layer implementation
    executeNextStep: (currentState: AgentState) => Effect.gen(function*(_) {
        const planManager = yield* _(PlanManager);
        const githubClient = yield* _(GitHubClient); // Get GitHubClient from context

        // 1. Get current step
        const currentStep = yield* _(planManager.getCurrentStep(currentState));
        yield* _(Effect.logInfo(`Executing step ${currentStep.step_number}: ${currentStep.description}`));

        // 2. Update status to in_progress
        let workingState = yield* _(planManager.updateStepStatus(currentState, currentStep.id, "in_progress"));

        // 3. *** Simulate Work (Placeholder) ***
        //    Later, this will involve AI calls, tool calls, etc.
        //    For now, just log and decide success/failure for testing.
        const workEffect = Effect.logDebug("Simulating step work...")
            // .pipe(Effect.delay("10ms")) // Optional small delay
            // .pipe(Effect.flatMap(() => Effect.fail(new Error("Simulated step failure!")))); // Uncomment to test failure path
            .pipe(Effect.map(() => ({ success: true, resultSummary: "Step simulated successfully." }))); // Simulate success

        const result = yield* _(Effect.either(workEffect)); // Capture result/error

        // 4. Update based on result
        if (Either.isRight(result)) {
            // SUCCESS PATH
            yield* _(Effect.logInfo(`Step ${currentStep.step_number} completed successfully.`));
            // Update status to completed
            workingState = yield* _(planManager.updateStepStatus(workingState, currentStep.id, "completed", result.right.resultSummary));
            // Advance step index
            workingState = {
                ...workingState,
                current_task: {
                    ...workingState.current_task,
                    current_step_index: workingState.current_task.current_step_index + 1
                }
            };
        } else {
            // FAILURE PATH
            const error = result.left;
            yield* _(Effect.logError(`Step ${currentStep.step_number} failed: ${error.message}`));
            // Update status to error
            workingState = yield* _(planManager.updateStepStatus(workingState, currentStep.id, "error", `Failed: ${error.message}`));
            // Update error_state
            const now = new Date().toISOString();
            workingState = {
                ...workingState,
                error_state: {
                    ...workingState.error_state,
                    last_error: {
                        timestamp: now,
                        message: error.message,
                        type: "internal", // Or determine type based on error
                        details: error.stack ?? ""
                    },
                    consecutive_error_count: workingState.error_state.consecutive_error_count + 1
                    // retry_count_for_current_action might be handled elsewhere or reset here
                }
            };
            // Decide whether to advance index on error - typically no.
        }

        // 5. Save the final state
        yield* _(githubClient.saveAgentState(workingState)); // Use the injected githubClient
        yield* _(Effect.logInfo(`Agent state saved for instance ${workingState.agent_info.instance_id}`));

        // 6. Return the final state
        return workingState;
    }).pipe(Effect.catchTags({ // Catch specific errors from PlanManager or saveAgentState if needed
        // StateNotFoundError: ..., etc.
    }))
    ```
*   **Create Layer:**
    ```typescript
    export const TaskExecutorLayer = Layer.effect(
      TaskExecutor,
      Effect.gen(function*(_) {
        // Get dependencies from the context
        const planManager = yield* _(PlanManager);
        const githubClient = yield* _(GitHubClient);

        return TaskExecutor.of({
          executeNextStep: (currentState: AgentState) => {
             // Implementation from above, ensuring planManager and githubClient are used
             // Needs to be adapted slightly to fit Layer structure if not using Effect.gen directly in the return object
             // (The Effect.gen approach above is cleaner)
             return Effect.gen(function* () { /* ... implementation ... */ }).pipe(
                 Effect.provideService(PlanManager, planManager),
                 Effect.provideService(GitHubClient, githubClient)
             );
          }
        });
      })
    );
    ```
    *   Ensure `PlanManagerLayer` and `GitHubClientLayer` (or its dependencies) are provided when running Effects that use `TaskExecutorLayer`.

**Phase 2, Step 6: TEST Task Execution Engine (MANDATORY & CONCURRENT)**

*   **Create Test File:** `test/github/TaskExecutor.test.ts`.
*   **Import Dependencies:** `@effect/vitest`, `Effect`, `Layer`, `TaskExecutor`, `PlanManager`, `GitHubClient`, test fixtures for `AgentState`.
*   **Mock Dependencies:**
    *   Create mock implementations for `PlanManager` (using `vi.fn()` for its methods).
    *   Create a mock implementation for `GitHubClient`, specifically mocking `saveAgentState` (`vi.fn().mockReturnValue(Effect.succeed(true))`).
    *   Provide these mocks using `Layer.succeed` when testing the `TaskExecutorLayer`.
*   **Write Tests:**
    *   **Success Case:**
        *   Provide initial `AgentState`.
        *   Setup mocks: `PlanManager.getCurrentStep` returns a step, `PlanManager.updateStepStatus` returns updated state, `saveAgentState` succeeds.
        *   Run `TaskExecutor.executeNextStep`.
        *   Assert `PlanManager.getCurrentStep` was called.
        *   Assert `PlanManager.updateStepStatus` was called twice (in_progress, then completed).
        *   Assert `saveAgentState` was called *once* at the end with the *final* state (including incremented index and 'completed' status).
        *   Assert the returned state has the correct `current_step_index` and step status.
    *   **Failure Case:**
        *   Provide initial `AgentState`.
        *   Setup mocks, but make the "Simulated Work" part within your test setup fail (e.g., by mocking a dependent service called during work, or directly returning `Effect.fail` if you mock that part).
        *   Run `TaskExecutor.executeNextStep`.
        *   Assert `PlanManager.updateStepStatus` was called twice (in_progress, then error).
        *   Assert `saveAgentState` was called *once* at the end with the *final* state (including updated `error_state` and 'error' status, index likely *not* incremented).
        *   Assert the returned state has the correct `error_state` fields populated and step status is 'error'.
*   **Run Tests Frequently:** Use `pnpm test test/github/TaskExecutor.test.ts` and `pnpm verify`.

**Phase 2, Step 7: Enhance State Storage Tests (REVISIT & MANDATORY)**

*   **Modify Test File:** `test/github/StateStorage.test.ts`.
*   **Implement Thorough Mocking:** Use `vi.mock("node:fs")` effectively.
    *   Mock `fs.existsSync`.
    *   Mock `fs.readFileSync` to return different things: valid JSON, invalid JSON, or throw errors.
    *   Mock `fs.writeFileSync` to succeed or throw errors.
    *   Mock `fs.mkdirSync`.
*   **Write Detailed Tests:**
    *   **`loadAgentState`:** Test the previously omitted failure paths:
        *   File exists, but `JSON.parse` throws -> Expect `StateParseError`.
        *   File exists, JSON parses, but `Schema.decodeUnknown(AgentStateSchema)` fails -> Expect `StateValidationError`.
        *   File exists, valid JSON, valid schema, but different `state_schema_version` -> Expect success but verify warning logged (mock `Effect.logWarning`).
    *   **`saveAgentState`:** Test failure case where `writeFileSync` throws -> Expect Effect failure.
    *   **`createAgentStateForIssue`:** Test failure case where the initial `getIssue` call fails. Test failure case where the final `saveAgentState` call fails.
*   **Run Tests:** Use `pnpm test test/github/StateStorage.test.ts` and `pnpm verify`. Ensure these more robust tests now pass.

**General Requirements (Reinforced):**

*   **TESTING FIRST/CONCURRENTLY:** Do not write implementation code without writing the corresponding tests. Verify tests pass often.
*   **IMMUTABILITY:** Double-check all state updates create new objects/arrays.
*   **LOGGING:** Add `Effect.log...` statements in new services (`PlanManager`, `TaskExecutor`).
*   **LOG FILE:** Maintain your Phase 2 implementation log (`docs/20250422-1345-phase2log.md`), detailing work on PlanManager, TaskExecutor, and ALL testing activities, including the revisit of StateStorage tests.
*   **VERIFY:** Run `pnpm verify` after completing all steps to ensure everything integrates correctly.

Execute these steps meticulously. The focus is now on building the core planning/execution logic *and* ensuring robust test coverage for *all* state-related functionality implemented so far.
