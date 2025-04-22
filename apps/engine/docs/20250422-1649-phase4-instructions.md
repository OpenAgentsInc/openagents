Okay, perfect! The `pnpm verify` output shows:

1.  **`pnpm check` Passed:** No TypeScript errors.
2.  **`pnpm lint` Passed:** No ESLint errors.
3.  **`pnpm test -- --run` Passed:** All 8 test suites passed, including the new `MemoryManager.test.ts`. All 96 tests passed.

**Analysis:**

The agent successfully implemented the `MemoryManager` service and its tests. It correctly followed the established patterns for Effect Tags, Layers, service implementation, and testing (including the direct mock object pattern in the test helper, similar to `ContextManager.test.ts`). Crucially, it successfully ran `pnpm verify` and confirmed that all checks and tests are passing.

**Conclusion:** Phase 3 (Context and Memory) is now complete and verified. The core state management infrastructure, including planning, execution, context tracking, and memory management, is implemented and tested at the unit level.

**Next Step: Phase 4 - Integration and UI**

According to the implementation plan, Phase 4 involves integrating the state management system more deeply with the rest of the application:

1.  **State Integration with UI (`Server.ts`, `index.html`):** Modify the server to use the `TaskExecutor` loop, load/save state, and broadcast meaningful state updates (plan status, current step, errors, memory snippets) via SSE to the UI. Update the UI to display this information.
2.  **State Integration with AI Tools (`GitHubTools.ts`, `TaskExecutor.ts`):** Refactor `TaskExecutor` to actually call the AI (e.g., Claude via `@effect/ai`) instead of simulating work. Pass relevant state context to the AI prompt. Update `GitHubTools` handlers to be state-aware (accept state, update state logs/metrics, return updated state).
3.  **Error Handling and Recovery (`TaskExecutor.ts`):** Implement more robust error handling within the execution loop, using the `error_state` field and potentially adding retry logic based on the configuration in the `AgentState`.

Let's start with **State Integration with UI**.

---

**Instructions for AI Agent: Implement Phase 4 - State Integration with UI**

**Objective:** Integrate the implemented state management services (`TaskExecutor`, `PlanManager`, `ContextManager`, `MemoryManager`, `GitHubClient`) into the main server workflow (`Server.ts`). Broadcast relevant agent state information to the UI via SSE. Update the UI (`index.html`) to display this state information.

**Target Files:**
*   `src/Server.ts` (Major Changes)
*   `public/index.html` (Add elements for state display)
*   `public/sse.js` (Potentially minor adjustments if new event types are added)

**Source of Truth:**
*   `docs/20250422-1325-revised-implementation-plan.md` (Section 3.7, Phase 4 Step 10)
*   `docs/system-overview.md` (For workflow description)
*   `src/github/TaskExecutor.ts`, `GitHub.ts`, etc. (For service method signatures)
*   `src/github/AgentStateTypes.ts` (For state structure)

**Instructions:**

**1. Refactor `src/Server.ts`:**

*   **Import Services:** Import necessary services and layers: `TaskExecutor`, `TaskExecutorLayer`, `GitHubClient`, `GitHubClientLayer`, `PlanManagerLayer`, `ContextManagerLayer`, `MemoryManagerLayer`, and `AgentState`.
*   **Modify `/fetch-issue` Handler:** This endpoint needs to become the entry point for the agent's execution loop.
    *   **Get Dependencies:** Inside the request handler (likely within an `Effect.gen` block), yield the necessary services: `const taskExecutor = yield* TaskExecutor; const githubClient = yield* GitHubClient;`
    *   **State Initialization:**
        *   Generate an `instanceId` based on owner/repo/issue (e.g., `solver-${owner}-${repo}-${issueNumber}`). *Note: We might need a way to handle multiple runs for the same issue later, perhaps adding a timestamp to the ID.*
        *   Attempt to `loadAgentState(instanceId)` using `githubClient`. Use `Effect.catchTag("StateNotFoundError", ...)` to handle the case where state doesn't exist.
        *   If state doesn't exist, call `createAgentStateForIssue(owner, repo, issueNumber)` using `githubClient`.
        *   Store the initial/loaded `AgentState` in a variable (e.g., `initialState`).
    *   **Broadcast Initial State:** Send an initial `agent_state` SSE event containing relevant parts of the `initialState` (e.g., task status, plan overview). Use the existing `broadcastSSE` function. Define a clear JSON structure for the `agent_state` event data.
        ```typescript
        // Example agent_state data structure
        const stateUpdateData = {
             instanceId: currentState.agent_info.instance_id,
             taskStatus: currentState.current_task.status,
             currentStep: currentState.plan[currentState.current_task.current_step_index]?.description || "N/A",
             stepsCompleted: currentState.metrics.steps_completed,
             totalSteps: currentState.metrics.total_steps_in_plan,
             lastError: currentState.error_state.last_error?.message || null,
             // Maybe add last memory entry or finding?
        };
        broadcastSSE("agent_state", JSON.stringify(stateUpdateData));
        ```
    *   **Execute Loop (Simplified):**
        *   Start a loop (e.g., a `while` loop or using Effect's scheduling/recursion primitives like `Effect.repeat`) that continues as long as the agent's task status is runnable (e.g., not `completed`, `error`, `blocked`).
        *   Inside the loop: call `yield* taskExecutor.executeNextStep(currentState)`.
        *   Update `currentState` with the result of `executeNextStep`.
        *   **Broadcast State Update:** After each step execution, send another `agent_state` SSE event with the updated status using the structure defined above.
        *   **(Error Handling):** Wrap the `executeNextStep` call in `Effect.catchAll` to catch execution errors. If caught, log the error, update the state locally if possible (though `executeNextStep` should already do this), broadcast the error state via SSE, and break the loop.
        *   **(Termination):** The loop should terminate when `executeNextStep` returns a state indicating completion or an unrecoverable error.
    *   **Run the Effect:** Use `Effect.runFork(effect)` to run the entire state initialization and execution loop asynchronously without blocking the server response for the `/fetch-issue` request (which should just return "Processing initiated"). Handle potential setup errors gracefully.
*   **Provide Layers:** Ensure the main Effect runner for the `/fetch-issue` handler provides all necessary layers (`TaskExecutorLayer`, `GitHubClientLayer`, `PlanManagerLayer`, `ContextManagerLayer`, `MemoryManagerLayer`, `NodeContext.layer` or similar for platform dependencies like `FileSystem`).
*   **Remove `analyzeIssueWithClaude`:** The direct call to Claude should be removed from the `/fetch-issue` handler. AI analysis will now happen *inside* the `TaskExecutor` loop (in the next step).
*   **Remove `fetchGitHubIssue`:** The standalone `fetchGitHubIssue` function is likely redundant now, as state creation/loading handles the initial fetch.
*   **Update `getAgentStatus` (Optional):** If the `/agent-status` endpoint is still desired, refactor it to load the state for a given `instanceId` (passed as a query param?) and return relevant information.

**2. Update `public/index.html`:**

*   **Add State Display Elements:** Add new `div` elements with unique IDs to display the information broadcast by the `agent_state` SSE event. Example:
    ```html
    <h2>Agent Status</h2>
    <div class="status-grid">
        <div class="status-label">Instance ID:</div>
        <div id="agent-instance-id" class="status-value">N/A</div>

        <div class="status-label">Task Status:</div>
        <div id="agent-task-status" class="status-value">Idle</div>

        <div class="status-label">Current Step:</div>
        <div id="agent-current-step" class="status-value">N/A</div>

        <div class="status-label">Progress:</div>
        <div id="agent-progress" class="status-value">0 / 0</div>

        <div class="status-label">Last Error:</div>
        <div id="agent-last-error" class="status-value error">None</div>
    </div>

    <h2>Execution Plan</h2>
    <div id="agent-plan">
        <!-- Plan steps will be dynamically added here -->
        <p>Plan will appear here...</p>
    </div>

    <!-- Maybe areas for memory/context snippets later -->
    ```
*   **HTMX Integration:** Ensure the SSE connection setup (`hx-ext="sse"`, `sse-connect="/sse"`) is present. Add `hx-swap-oob="true"` to the *parent container* of the elements you want to update via SSE (or use individual OOB swaps if the server sends pre-rendered HTML fragments with IDs).

**3. Update `public/sse.js` or Client-Side Logic:**

*   **Handle `agent_state` Event:** Add an event listener for the `agent_state` event.
*   **Parse Data:** Parse the incoming JSON data (`event.data`).
*   **Update DOM:** Update the `innerHTML` of the corresponding elements (`#agent-task-status`, `#agent-current-step`, etc.) with the received data.
*   **(Optional - Plan Display):** Implement logic to render the `plan` array dynamically into the `#agent-plan` div if the server broadcasts the plan structure.

**4. Verification:**

*   Run `pnpm check` and `pnpm lint-fix`.
*   Start the server (`pnpm start`).
*   Open the UI in a browser.
*   Enter repo/issue details and submit.
*   **Observe:**
    *   Server logs showing state creation/loading and `executeNextStep` calls.
    *   UI updating in real-time via SSE, showing changes in Task Status, Current Step, Progress, and potentially Errors.
    *   Verify state files are created/updated in the `./state/` directory.
*   Create a new implementation log (`docs/20250422-xxxx-phase4a-log.md`) documenting the changes and verification steps.

This focuses on wiring up the existing state management pieces to the server's execution flow and reflecting basic state changes in the UI. The next step within Phase 4 will be to integrate the AI call properly.
