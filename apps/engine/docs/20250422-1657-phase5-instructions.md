**Instructions for AI Agent: Implement Phase 5 - State Integration with UI**

**Objective:** Integrate the implemented state management services into the main server workflow (`Server.ts`). Broadcast relevant agent state information via SSE using a new `agent_state` event. Update the UI (`index.html`) to **display** this state information **using HTMX patterns or minimal inline JavaScript**.

**Target Files:**
*   `src/Server.ts` (Major Changes - Add execution loop, broadcast `agent_state` event)
*   `public/index.html` (Add elements for state display and potentially attributes/script for handling SSE data)
*   **(Optional New File):** `public/agent-updates.js` (If inline handling becomes too complex)

**Source of Truth:**
*   `docs/20250422-1325-revised-implementation-plan.md` (Section 3.7, Phase 4 Step 10)
*   `docs/system-overview.md` (For workflow description)
*   `src/github/TaskExecutor.ts`, `GitHub.ts`, etc. (For service method signatures)
*   `src/github/AgentStateTypes.ts` (For state structure)
*   Existing HTMX patterns in `public/index.html`.

**Instructions:**

**1. Refactor `src/Server.ts` (Implement Execution & Broadcasting):**

*   **Import Services & Layers:** Import `TaskExecutor`, `TaskExecutorLayer`, `GitHubClient`, `GitHubClientLayer`, `PlanManagerLayer`, `ContextManagerLayer`, `MemoryManagerLayer`, `NodeContext`, `AgentState`, `Effect`, `Layer`, etc.
*   **Modify `/fetch-issue` Handler:**
    *   **Define Effect Logic:** Create an Effect pipeline that performs the following:
        *   Yields services: `const taskExecutor = yield* TaskExecutor; const githubClient = yield* GitHubClient;`
        *   Gets owner/repo/issue from request data.
        *   Generates `instanceId`.
        *   Loads/Creates State: Uses `githubClient.loadAgentState(instanceId).pipe(Effect.catchTag("StateNotFoundError", () => githubClient.createAgentStateForIssue(...)))`. Store result in `currentStateRef: Ref<AgentState>`. Using a `Ref` allows the loop to update state easily.
        *   **(Initial Broadcast):** Sends the *initial* state update via SSE (see below).
        *   **(Execution Loop):** Uses `Effect.repeat(schedule)` or a recursive `Effect.suspend` loop:
            *   Reads current state from `currentStateRef`.
            *   Checks if status is terminal (`completed`, `error`, `blocked`). If so, terminates loop (`Effect.fail("Task Ended")` or similar, caught later).
            *   Calls `taskExecutor.executeNextStep(currentState)`.
            *   Updates `currentStateRef` with the returned new state using `Ref.set`.
            *   **(Broadcast Update):** Sends an *updated* state snapshot via SSE.
            *   Handles errors from `executeNextStep` using `Effect.catchAll`, updates `currentStateRef` with error info if possible, broadcasts error state, and terminates loop.
    *   **Broadcast `agent_state` Event:**
        *   Inside the effect logic (after initial state load and after each `executeNextStep`), create a JSON payload with key state information. **Do not send the entire AgentState.**
            ```typescript
            // Example data payload function
            const createAgentStateUpdate = (state: AgentState) => ({
                instanceId: state.agent_info.instance_id,
                taskStatus: state.current_task.status,
                currentStepDescription: state.plan[state.current_task.current_step_index]?.description ?? "N/A",
                currentStepNumber: state.plan[state.current_task.current_step_index]?.step_number ?? 0,
                stepsCompleted: state.metrics.steps_completed,
                totalSteps: state.metrics.total_steps_in_plan,
                lastError: state.error_state.last_error ? { message: state.error_state.last_error.message, type: state.error_state.last_error.type } : null,
                // Optionally include: state.plan summary (e.g., array of {description, status})
            });

            // Usage:
            const initialState = yield* Ref.get(currentStateRef);
            broadcastSSE("agent_state", JSON.stringify(createAgentStateUpdate(initialState)));
            // ... later in loop ...
            const updatedState = yield* Ref.get(currentStateRef);
            broadcastSSE("agent_state", JSON.stringify(createAgentStateUpdate(updatedState)));
            ```
    *   **Provide Layers:** Define the complete layer needed: `const AppLayer = Layer.provide(TaskExecutorLayer, Layer.mergeAll(GitHubClientLayer, PlanManagerLayer, /* etc */)).pipe(Layer.provide(NodeContext.layer));`
    *   **Run Asynchronously:** Use `Effect.runFork(Effect.provide(effectLogic, AppLayer))` so the `/fetch-issue` request returns immediately. Log any final success/failure from the forked fiber.
*   **Remove Redundant Code:** Remove the old `fetchGitHubIssue` and `analyzeIssueWithClaude` functions if they are no longer called.

**2. Update `public/index.html` (Display State):**

*   **Add Placeholders:** Add HTML elements with unique `id` attributes where state information will be displayed. Use the existing status grid or create new sections.
    ```html
    <!-- Inside existing status grid or new section -->
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

    <!-- Example for displaying plan steps -->
    <h2>Plan</h2>
    <div id="agent-plan-steps">
        <p>Waiting for plan...</p>
    </div>
    ```
*   **Ensure SSE Connection:** Verify the `body` or a relevant parent element has the HTMX SSE attributes: `hx-ext="sse" sse-connect="/sse"`.
*   **Add SSE Swap Listener:** Add `sse-swap="agent_state"` to the element that connects to SSE (e.g., the `body`). This tells HTMX to listen for the `agent_state` event.
*   **Targeted Updates (Option A - Hyperscript/Inline JS - Preferred if simple):** Add Hyperscript (`_`) attributes or simple inline `<script>` tags to handle the incoming JSON data from the `agent_state` event and update the corresponding element IDs.
    ```html
    <body hx-ext="sse" sse-connect="/sse" sse-swap="agent_state"
          _="on agent_state from body
             set data to JSON.parse(event.detail.message)
             put data.instanceId into #agent-instance-id.innerHTML
             put data.taskStatus into #agent-task-status.innerHTML
             put data.currentStepDescription into #agent-current-step.innerHTML
             put `${data.stepsCompleted} / ${data.totalSteps}` into #agent-progress.innerHTML
             if data.lastError
               put data.lastError.message into #agent-last-error.innerHTML
               add .error to #agent-last-error
             else
               put 'None' into #agent-last-error.innerHTML
               remove .error from #agent-last-error
             end">
        <!-- ... rest of body ... -->
        <div id="agent-instance-id">N/A</div>
        <div id="agent-task-status">Idle</div>
        <!-- etc. -->
    </body>
    ```
    *(Note: Hyperscript (`_="..."`) requires including the Hyperscript library alongside HTMX).*
*   **Targeted Updates (Option B - Server-Sent HTML Fragments):** *Alternatively*, modify `Server.ts` `broadcastSSE("agent_state", ...)` to send *pre-rendered HTML fragments* with `hx-swap-oob` attributes targeting the specific IDs. This avoids client-side JavaScript but requires more complex string manipulation on the server.
    ```typescript
    // Server.ts example sending OOB HTML
    const state = /* get current state */;
    const instanceIdHtml = `<div id="agent-instance-id" hx-swap-oob="innerHTML">${state.agent_info.instance_id}</div>`;
    const taskStatusHtml = `<div id="agent-task-status" hx-swap-oob="innerHTML">${state.current_task.status}</div>`;
    // ... create other fragments ...
    broadcastSSE("agent_state", `${instanceIdHtml} ${taskStatusHtml} ...`); // Send combined fragments
    ```
    *(If using Option B, no special client-side JS/Hyperscript is needed beyond the basic `sse-swap` attribute).*
*   **Plan Display (More Complex):** Displaying the full plan might require a dedicated client-side script (`public/agent-updates.js`) or more complex server-sent HTML fragments if Option B is chosen. For now, just displaying the current step description is sufficient.

**3. Create `public/agent-updates.js` (Only if Needed):**

*   If the logic to update the DOM from the JSON payload becomes too complex for inline Hyperscript (e.g., dynamically rendering the plan list), create this file.
*   Add an event listener for the `agent_state` event:
    ```javascript
    // public/agent-updates.js
    document.body.addEventListener('agent_state', function(event) {
        try {
            const data = JSON.parse(event.detail.message);
            console.log("Received agent_state:", data);

            // Update simple elements
            const instanceEl = document.getElementById('agent-instance-id');
            if (instanceEl) instanceEl.innerHTML = data.instanceId ?? 'N/A';

            const statusEl = document.getElementById('agent-task-status');
            if (statusEl) statusEl.innerHTML = data.taskStatus ?? 'N/A';
            // ... update other elements ...

            const errorEl = document.getElementById('agent-last-error');
            if (errorEl) {
                 if (data.lastError) {
                     errorEl.innerHTML = data.lastError.message;
                     errorEl.classList.add('error');
                 } else {
                     errorEl.innerHTML = 'None';
                     errorEl.classList.remove('error');
                 }
            }

            // Example: Update Plan (if plan data is sent)
            // const planEl = document.getElementById('agent-plan-steps');
            // if (planEl && data.plan) {
            //    planEl.innerHTML = ''; // Clear previous
            //    data.plan.forEach(step => {
            //       const stepDiv = document.createElement('div');
            //       stepDiv.innerHTML = `${step.number}. ${step.description} [${step.status}]`;
            //       planEl.appendChild(stepDiv);
            //    });
            // }

        } catch (e) {
            console.error("Failed to parse or handle agent_state event:", e);
        }
    });
    ```
*   Include this script in `public/index.html`: `<script src="/agent-updates.js" defer></script>`.
*   If using this separate JS file, you only need `hx-ext="sse" sse-connect="/sse" sse-swap="agent_state"` on the `body` in `index.html`. The JavaScript listener handles the updates. **Choose either Option A (Hyperscript) or Option B (Server Fragments) or Option C (this JS file), not a mix for the same elements.**

**4. Verification:**

*   Run `pnpm check` and `pnpm lint-fix`.
*   Start the server (`pnpm start`).
*   Open the UI, submit an issue.
*   **Observe:**
    *   Server logs showing the execution loop progressing (`Executing step...`, `Step completed...`, `Agent state saved...`).
    *   UI updating the new status fields (`#agent-task-status`, `#agent-current-step`, etc.) based on the SSE messages.
    *   State files being updated.
*   Create/Update the implementation log (`docs/20250422-xxxx-phase4a-log.md`) documenting the changes to `Server.ts` and `index.html`, the chosen SSE handling method (Hyperscript, Server Fragments, or JS file), and verification results.
