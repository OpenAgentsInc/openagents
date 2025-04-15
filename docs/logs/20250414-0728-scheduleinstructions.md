Okay, let's craft the instructions for the agent to refactor the scheduling tools for more flexibility, integrate them with the internal task state, and enable continuous overnight execution.

**Instructions for Agent:**

"We need to significantly enhance the task scheduling capabilities of the `Coder` agent. The goals are:

1.  **Flexibility:** Allow scheduling of different agent methods, not just `executeTask`.
2.  **Context Preservation:** Allow passing structured data (payload) to the scheduled method.
3.  **State Integration:** Link system schedules with the agent's internal task list (`CoderState.tasks`).
4.  **Clarity:** Improve tool naming and return values.
5.  **Continuous Execution:** Implement a mechanism for the agent to schedule its *own* continuation, enabling long-running or overnight tasks until explicitly stopped.

Please implement the following changes across `src/tools.ts`, `src/server.ts`, and potentially `src/types.ts`:

**Phase 1: Refactor Existing Scheduling Tools (`src/tools.ts`)**

1.  **Update `scheduleTask` Parameters:**
    *   Modify the `parameters` Zod schema for the `scheduleTask` tool.
    *   Keep the existing `when: unstable_scheduleSchema`.
    *   Keep the `description: z.string()`.
    *   Add an optional `callbackMethodName: z.enum(['executeTask', 'continueInfer', /* Add other relevant Coder methods here */]).optional().default('executeTask').describe('The specific agent method to call when the schedule fires. Defaults to executeTask.')`. *(Self-correction: Start with just 'executeTask' and 'continueInfer' in the enum initially to keep it simple)*.
    *   Add an optional `payload: z.record(z.any()).optional().describe('Optional structured data (JSON object) to pass to the callback method.')`.

2.  **Update `scheduleTask` Execution Logic:**
    *   Inside the `execute` function:
        *   Retrieve the `agent` using `agentContext.getStore()` as before.
        *   Get `when`, `description`, `callbackMethodName`, and `payload` from the function arguments.
        *   Perform the `when.type` checks as before to determine the `scheduleInput` (Date, delay, or cron).
        *   In the `try` block:
            *   Modify the `agent.schedule` call: `const scheduleResult = await agent.schedule(scheduleInput!, callbackMethodName, payload || { description });` *(Note: We pass the description inside a default payload if no explicit payload is given)*.
            *   Capture the `scheduleId` from `scheduleResult.id`.
            *   **Add Integration:** Call `agent.addAgentTask(description, scheduleId, payload);` (We'll define this method on `Coder` later). Pass the schedule ID and payload.
            *   Modify the return string: `return \`Task scheduled with ID: ${scheduleId}. Method: ${callbackMethodName}. Details: ${description}\`;`
        *   Update error handling messages if necessary.

3.  **Refactor `listScheduledTasks`:**
    *   Rename the tool to `listSystemSchedules` to clarify it lists the underlying system alarms/schedules.
    *   Keep its current implementation (using `agent.getSchedules()`).

4.  **Refactor `deleteScheduledTask`:**
    *   Rename the tool to `deleteSystemSchedule`.
    *   Keep its current implementation (using `agent.cancelSchedule(id)`).
    *   **Add Integration:** Inside the `try` block, after successfully calling `agent.cancelSchedule(taskId)`, add logic to find and potentially mark a corresponding task in `agent.state.tasks` as cancelled or remove it. *(This requires enhancing `CoderState.Task` and adding a method on `Coder`, see Phase 2)*. Modify the success message accordingly.

5.  **Add New Tool `listAgentTasks`:**
    *   Create a new tool named `listAgentTasks`.
    *   Description: `"Lists the tasks currently tracked in the agent's internal state."`
    *   Parameters: `z.object({ status: z.enum(['pending', 'in-progress', 'completed', 'failed', 'all']).optional().default('pending').describe('Filter tasks by status.') })`
    *   Execution (`execute` function):
        *   Get the `agent` via `agentContext.getStore()`.
        *   Access `agent.state.tasks`.
        *   Filter the tasks based on the provided `status` argument (`all` means no filtering).
        *   Format the filtered tasks nicely for the user (include ID, description, status, maybe creation time).
        *   Return the formatted string or "No matching tasks found."

6.  **Update `tools` Export:** Update the exported `tools` object to reflect the renamed tools and include the new `listAgentTasks`.

**Phase 2: Update Agent State and Methods (`src/types.ts` & `src/server.ts`)**

1.  **Update `Task` Type (`src/types.ts`):**
    *   Add optional fields to the `Task` interface:
        *   `scheduleId?: string;` (To link to the system schedule)
        *   `payload?: Record<string, any>;` (To store the scheduled payload)
        *   `callbackMethodName?: string;` (To store the scheduled method name)
        *   Update the `status` enum if needed (e.g., add `'cancelled'`).

2.  **Update `Coder.initialState` (`src/server.ts`):** Ensure `tasks: []` is still present.

3.  **Update `Coder.addAgentTask` (`src/server.ts`):**
    *   Modify the signature to accept the optional `scheduleId` and `payload` from the `scheduleTask` tool: `private addAgentTask(description: string, scheduleId?: string, payload?: Record<string, any>)`.
    *   When creating `newTask`, include the `scheduleId` and `payload` if they are provided.
    *   Update the `updateState` call accordingly.

4.  **Add `Coder.cancelAgentTask` (or similar) (`src/server.ts`):**
    *   Create a new private method like `private cancelTaskByScheduleId(scheduleId: string)`.
    *   This method should find the task in `this.state.tasks` that has the matching `scheduleId`.
    *   If found, it should update its status to `'cancelled'` (or remove it) using `this.updateState`.
    *   Call this method from the `deleteSystemSchedule` tool's `execute` function after successfully cancelling the system schedule.

5.  **Implement `Coder.continueInfer` (`src/server.ts`):**
    *   Create a new `public async continueInfer(payload?: any)` method. This method will be the target for self-rescheduling.
    *   **Logic:**
        *   Log that it's continuing.
        *   Optionally use the `payload` if needed for context.
        *   **Crucially:** Call `await this.infer();` to make the agent think and potentially perform actions based on its current state (tasks, codebase, etc.).
        *   **Self-Reschedule:** *After* `infer` completes (or maybe before, depending on desired behavior), schedule the *next* `continueInfer` call using the `scheduleTask` tool logic *internally*.
            *   Use `this.schedule(...)` directly (since we are inside the agent).
            *   Schedule `this.continueInfer` (referencing the method itself, or using the string name `"continueInfer"`).
            *   Set a delay (e.g., 1 minute, 5 minutes, configurable?).
            *   Maybe pass some minimal payload if needed.
        ```typescript
        public async continueInfer(payload?: any) {
            console.log(`[continueInfer] Agent waking up. Payload: ${JSON.stringify(payload)}`);
            try {
                // Perform main thinking loop
                await this.infer();

                // Check if we should continue running (e.g., based on state or a flag)
                // For now, let's assume we always reschedule unless explicitly stopped.
                const shouldContinue = true; // TODO: Add logic to stop continuous run (e.g., based on a state flag)

                if (shouldContinue) {
                    const delayInSeconds = 60; // Example: run again in 60 seconds
                    console.log(`[continueInfer] Rescheduling self in ${delayInSeconds} seconds.`);
                    await this.schedule(delayInSeconds, 'continueInfer', { reason: 'continuous execution' });
                } else {
                     console.log(`[continueInfer] Not rescheduling.`);
                }

            } catch (error) {
                 console.error("[continueInfer] Error during inference or rescheduling:", error);
                 // Consider rescheduling even on error, maybe with backoff?
                 const delayInSeconds = 300; // Reschedule after 5 mins on error
                 console.log(`[continueInfer] Rescheduling self after error in ${delayInSeconds} seconds.`);
                 await this.schedule(delayInSeconds, 'continueInfer', { reason: 'error recovery' });
            }
        }
        ```

6.  **Mechanism to Start/Stop Continuous Run:**
    *   Add a new boolean field to `CoderState`, e.g., `isContinuousRunActive: boolean;` Initialize it to `false`.
    *   Create two new `@unstable_callable` methods:
        *   `async startContinuousRun()`: Sets `this.state.isContinuousRunActive` to `true` using `updateState` and immediately calls `await this.continueInfer();` to kick off the cycle. Returns a success message.
        *   `async stopContinuousRun()`: Sets `this.state.isContinuousRunActive` to `false` using `updateState`. It should also find and *cancel* any pending system schedules targeting `continueInfer` using `this.getSchedules()` and `this.cancelSchedule()`. Returns a success message.
    *   Modify the `if (shouldContinue)` check in `continueInfer` to be `if (this.state.isContinuousRunActive)`.

**Phase 3: Update Tool Exports and Usage**

1.  Ensure the `tools` export in `src/tools.ts` includes the renamed and new tools.
2.  Update any references or usage of the old tool names if necessary elsewhere in the codebase (though likely only `infer` uses them).

Apply these changes systematically. This refactoring provides much more powerful and integrated scheduling, enabling the agent to manage its own long-running processes."
