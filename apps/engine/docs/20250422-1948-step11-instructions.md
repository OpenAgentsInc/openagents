Okay, here are the detailed instructions for the next agent to implement **Phase 4, Step 11: State Integration with AI Tools**. This assumes the agent starts with the codebase where Phases 1, 2, 3, and Step 10 of Phase 4 are complete and verified (`pnpm verify` passes).

---

**Instructions for AI Agent: Implement Phase 4 - State Integration with AI & Tools**

**Objective:** Integrate the AI decision-making process with the agent's state and tools. This involves:
1.  Replacing the simulated work in `TaskExecutor` with actual calls to an AI service (Anthropic Claude via `@effect/ai-anthropic`).
2.  Constructing AI prompts using relevant context from the `AgentState`.
3.  Processing AI responses, including handling text generation and tool calls.
4.  Refactoring `GitHubTools` handlers to be state-aware, updating the `AgentState` appropriately after tool execution.
5.  Adding/Updating unit tests for `TaskExecutor` and potentially `GitHubTools` to cover stateful AI/tool interactions.

**Target Files:**
*   `src/github/TaskExecutor.ts` (Major Changes)
*   `src/github/GitHubTools.ts` (Major Changes)
*   `src/Program.ts` (Ensure AI client layer is available)
*   `test/github/TaskExecutor.test.ts` (Update/Add Tests)
*   `test/github/GitHubTools.test.ts` (Update/Add Tests)

**Source of Truth:**
*   `docs/agent-state.md` (State structure)
*   `src/github/AgentStateTypes.ts` (State schemas)
*   `docs/system-overview.md` (Architecture, Workflow)
*   `docs/guidance/effect-service-patterns.md` (Correct Effect patterns)
*   `@effect/ai`, `@effect/ai-anthropic` documentation (for `Completions`, `toolkitStream` usage)
*   Existing code (`PlanManager`, `MemoryManager`, `ContextManager`, `GitHubClient`).

**Phase 1: Prepare AI Integration**

1.  **Add AI Dependencies:**
    *   Open `package.json` for `apps/engine`.
    *   Ensure `@effect/ai` and `@effect/ai-anthropic` are listed in dependencies. If not, add them: `pnpm add @effect/ai @effect/ai-anthropic`
2.  **Configure AI Layer (`src/Program.ts` or a new `src/config.ts`):**
    *   Ensure an `AnthropicClient` layer is configured and available. If `src/Program.ts` already defines `Anthropic` layer using `AnthropicClient.layerConfig({ apiKey: Config.secret("ANTHROPIC_API_KEY") })`, ensure this layer is included in the `AllLayers` export.
    *   If not present, add it:
        ```typescript
        // src/Program.ts (or similar central config)
        import { AnthropicClient } from "@effect/ai-anthropic";
        import { Config, Layer } from "effect";
        import { NodeHttpClient } from "@effect/platform-node";
        // ... other imports ...

        // Define Anthropic Layer
        const AnthropicLayer = AnthropicClient.layerConfig({
            apiKey: Config.secret("ANTHROPIC_API_KEY")
        });

        // Include it in the combined layers, ensuring HttpClient is also provided
        export const AllLayers = Layer.mergeAll(
             // ... existing layers (GitHubClientLayer, PlanManagerLayer, etc.) ...
             Layer.provide(AnthropicLayer, NodeHttpClient.layerUndici) // Provide HTTP client to Anthropic
        ).pipe(Layer.provide(NodeContext.layer)); // Ensure NodeContext is provided at the end
        ```
    *   Verify the `ANTHROPIC_API_KEY` environment variable is documented/expected.

**Phase 2: Make `GitHubTools` Handlers Stateful (Using `Ref<AgentState>`)**

*   **Problem:** Tool handlers need to update the shared `AgentState` (e.g., add logs, metrics) *after* execution, but passing state in and out of the `@effect/ai` toolkit stream handlers is complex.
*   **Solution:** Use a shared `Ref<AgentState>` managed by `TaskExecutor`. Provide functions to the handlers (via context or directly) that update this `Ref`.
*   **Modify `src/github/GitHubTools.ts`:**
    1.  **Import `Ref`:** Add `import { Ref } from "effect";`
    2.  **Define Tool Context:** Create a new `Tag` for a context object that holds the `Ref` and potentially manager services.
        ```typescript
        // Near top of GitHubTools.ts
        import { Context } from "effect";
        import { PlanManager } from "./PlanManager";
        import { MemoryManager } from "./MemoryManager";
        // ... other imports ...

        // Context for stateful tool execution
        export interface StatefulToolContext {
            readonly stateRef: Ref.Ref<AgentState>;
            readonly planManager: PlanManager;
            readonly memoryManager: MemoryManager;
        }
        export const StatefulToolContext = Context.Tag<StatefulToolContext>();
        ```
    3.  **Refactor `GitHubToolsLayer`:**
        *   Change `Layer.effect` to `Layer.effect(GitHubTools, Effect.gen(function*(_) { ... }))`.
        *   Yield the `StatefulToolContext` inside the generator: `const toolContext = yield* StatefulToolContext;`
        *   Keep `const github = yield* GitHubClient;`
    4.  **Modify Handler Implementation:**
        *   Handlers should now use the `toolContext` to update the shared state *after* the core action. They still primarily return the *result* of the GitHub API call, not the state itself.
        *   Use `Ref.get(toolContext.stateRef)` to get current state if needed *before* the action.
        *   Use `Ref.update(toolContext.stateRef, (currentState) => { /* immutable update logic using managers */ })` to update the state *after* the action succeeds or fails.
        ```typescript
         // Example: GetGitHubIssue handler (simplified state update)
         GetGitHubIssue: (params: { owner: string; repo: string; issueNumber: number }) =>
             Effect.gen(function*(_) {
                 const { stateRef, memoryManager, planManager } = yield* StatefulToolContext; // Get context
                 yield* Console.log(`🛠️ Tool called: GetGitHubIssue with params: ${JSON.stringify(params)}`);

                 // Perform core action
                 const result = yield* github.getIssue(params.owner, params.repo, params.issueNumber);
                 yield* Console.log(`✅ Tool result obtained.`);

                 // Update state via Ref using Managers AFTER success
                 yield* Ref.update(stateRef, (currentState) => {
                      const toolCallData = { tool_name: TOOL_NAMES.GET_ISSUE, parameters: params, status: "success", result_preview: JSON.stringify(result).substring(0,100), full_result_ref: null };
                      // Chain sync updates using pipe (or run effects separately if needed)
                      return Effect.runSync( // Okay for simple sync updates
                          Effect.gen(function*() {
                              let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData);
                              const currentStepId = state1.plan[state1.current_task.current_step_index]?.id;
                              if (currentStepId) {
                                   state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData);
                              }
                              // Update metrics (simplified)
                              return { ...state1, metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 } };
                          })
                      );
                 });

                 return result; // Return original result
             }).pipe(
                Effect.catchAll((error: Error) => // Catch errors from github call or state update
                     Effect.gen(function*(_) {
                         const { stateRef, memoryManager, planManager } = yield* StatefulToolContext;
                         yield* Console.error(`❌ Tool error: ${error.message}`);
                         // Update state via Ref on FAILURE
                         yield* Ref.update(stateRef, (currentState) => {
                             const toolCallData = { tool_name: TOOL_NAMES.GET_ISSUE, parameters: params, status: "error", result_preview: error.message, full_result_ref: null };
                             // ... similar sync update logic for logs/metrics/error_state ...
                             return /* updated error state */;
                         });
                         // Fail the tool effect
                         return yield* Effect.fail(new ToolExecutionError(error.message, TOOL_NAMES.GET_ISSUE, params));
                     })
                )
             ), // End handlers object
         // ... Implement ALL other handlers similarly ...
         SaveAgentState: (params: { state: AgentState }) => /* Note: This tool might just directly use githubClient.saveAgentState */
        ```
    5.  **Return Value:** Ensure the `GitHubToolsLayer` returns `{ tools: toolkit, handlers }` where `toolkit` contains the schemas and `handlers` contains these state-aware functions.

**Phase 3: Refactor `TaskExecutor.ts` (Use Real AI & Stateful Tools)**

1.  **Import AI/Tool Dependencies:** Ensure `Completions`, `AnthropicCompletions`, `GitHubTools`, `StatefulToolContext`, `Ref`, `Stream` are imported.
2.  **Inject Dependencies:** Update `TaskExecutorLayer`'s `Effect.gen` to yield: `const completions = yield* Completions.Completions; const githubTools = yield* GitHubTools;` (Keep existing managers/client yields).
3.  **Modify `executeNextStep`:**
    *   **Replace `simulateAIWithTools`:** Delete the simulation function.
    *   **Create `StatefulToolContext`:** Inside the main `Effect.gen` for `executeNextStep` (after loading/creating initial state and the `stateRef`):
        ```typescript
         // Inside executeNextStep, after stateRef is created
         const toolContextService: StatefulToolContext = {
             stateRef: stateRef,
             planManager: planManager, // Assuming planManager is yielded
             memoryManager: memoryManager // Assuming memoryManager is yielded
         };
         const toolContextLayer = Layer.succeed(StatefulToolContext, toolContextService);
        ```
    *   **Call AI (`toolkitStream`):**
        ```typescript
         // Construct prompt (as before)
         const prompt = constructPromptFromState(yield* Ref.get(stateRef), currentStep);
         yield* Console.log("🧠 Prompting AI to execute step...");

         // Get tools/handlers (they are now stateless)
         const { tools, handlers } = githubTools;

         // Prepare toolkit stream
         const aiResponseStream = completions.toolkitStream({
             model: /* e.g., "claude-3-opus-..." or from config */,
             system: /* System prompt if needed */,
             messages: [{ role: "user", content: prompt }],
             tools: { toolkit: tools, handlers },
         });
        ```
    *   **Process AI Stream (Stateful):** The stream processing loop needs significant changes.
        ```typescript
         let responseBuffer = "";
         let toolOutputs = []; // Store tool results if needed for subsequent prompts
         let finalResponse = ""; // Capture final AI thought

         yield* aiResponseStream.pipe(
             Stream.tap((chunk) =>
                 Effect.gen(function*() {
                     const currentState = yield* Ref.get(stateRef); // Get latest state
                     let nextState = currentState;

                     // Handle Text Deltas
                     if (chunk.response && chunk.response.parts) {
                         for (const part of chunk.response.parts) {
                              if (part._tag === "Text" && part.content) {
                                   responseBuffer += part.content;
                                   // Optionally broadcast text deltas via SSE?
                              } else if (part._tag === "ToolCall") {
                                   // Log tool usage START (before execution)
                                   yield* Console.log(`AI requesting tool: ${part.name}`);
                                   // AI framework handles calling the handler now
                              }
                         }
                     }

                     // Handle Tool Results (after handler runs and updates Ref)
                     if (chunk.value?._tag === "Some") {
                          const toolResult = chunk.value.value; // This is the raw result from the handler
                          yield* Console.log(`🔧 Tool ${toolResult.name} executed.`);
                          // State *should* have been updated inside the handler via Ref now.
                          // We might add the *result* to conversation history here.
                          const toolResultMessage = {
                              role: "tool",
                              content: JSON.stringify(toolResult.result), // Content is the tool result
                              tool_call_id: toolResult.id // Link to the tool call
                          };
                          nextState = yield* memoryManager.addConversationMessage(
                              nextState,
                              toolResultMessage.role,
                              toolResultMessage.content,
                              [{ id: toolResultMessage.tool_call_id, name: toolResult.name, input: toolResult.input }] // Use ConversationToolCall structure
                          );
                     }

                     // Update Ref if state changed within the chunk processing
                     if (nextState !== currentState) {
                          yield* Ref.set(stateRef, nextState);
                     }
                 })
             ),
             // Provide the StatefulToolContext Layer to the stream processing
             // This makes the Ref and Managers available to the tool handlers called by toolkitStream
             (stream) => Effect.provide(stream, toolContextLayer),
             Stream.runDrain // Consume the stream
         ).pipe(Effect.catchAll((aiError) => {
             // Handle errors specifically from the AI/Tool stream
             yield* Console.error(`AI/Tool Stream Error: ${aiError}`);
             // Update stateRef with AI error details
             const now = new Date().toISOString();
             yield* Ref.update(stateRef, (s) => ({
                 ...s,
                 error_state: {
                     ...s.error_state,
                     last_error: { timestamp: now, message: `AI Error: ${aiError}`, type: "internal", details: String(aiError) },
                     consecutive_error_count: s.error_state.consecutive_error_count + 1
                 },
                 current_task: { ...s.current_task, status: "error" } // Mark task as error
             }));
             // Re-throw or fail the main effect
             return Effect.fail(aiError instanceof Error ? aiError : new Error(String(aiError)));
         })); // End of aiResponseStream processing

         // After stream processing:
         workingState = yield* Ref.get(stateRef); // Get potentially updated state

         // Add final assistant message (if any) to memory
         finalResponse = responseBuffer.trim();
         if (finalResponse) {
            workingState = yield* memoryManager.addConversationMessage(workingState, "assistant", finalResponse);
         }

         // Update Step Status (Determine success/failure based on final state's error_state or explicit AI markers)
         const stepFailed = workingState.error_state.last_error && workingState.error_state.last_error.timestamp > currentState.timestamps.last_action_at; // Crude check if error is recent
         if (stepFailed) {
             yield* Console.log(`Step ${currentStep.step_number} failed.`);
             workingState = yield* planManager.updateStepStatus(workingState, currentStep.id, "error", workingState.error_state.last_error!.message.slice(0, 200));
             // Error state already updated
         } else {
              yield* Console.log(`Step ${currentStep.step_number} completed.`);
              workingState = yield* planManager.updateStepStatus(workingState, currentStep.id, "completed", finalResponse.slice(0, 200) || "Completed");
              // Advance step index
              workingState = {
                  ...workingState,
                  current_task: {
                      ...workingState.current_task,
                      current_step_index: workingState.current_task.current_step_index + 1
                  }
              };
         }
         // Update metrics (LLM calls, tokens if available from response)
         // ...

         // Save final state for the step
         yield* githubClient.saveAgentState(workingState);
         // ... rest of executeNextStep ...
        ```
    *   **Provide AI Layer:** Ensure the `AppLayer` defined in `Server.ts` includes the `AnthropicLayer` (or appropriate AI client layer).

**Phase 4: Update Tests**

1.  **Update `TaskExecutor.test.ts`:**
    *   **Mock AI:** Mock the `Completions` service. Provide a mock implementation for `toolkitStream` that returns a `Stream` simulating AI text output and potential tool calls/results.
    *   **Mock `GitHubTools`:** Provide a mock `GitHubTools` layer. The mock handlers should just return simple success/failure `Effect`s (e.g., `Effect.succeed({ fake: "data" })` or `Effect.fail(new Error("Mock Tool Error"))`) as the state updates are now simulated *within* the test's `executeNextStep` assertions by checking the final state in the `Ref`.
    *   **Mock State Managers:** Provide mock layers for `PlanManager`, `MemoryManager`, `ContextManager`.
    *   **Update Assertions:** Modify tests to:
        *   Assert that the AI prompt was constructed correctly (requires spying/logging).
        *   Assert that the correct state updates were performed *after* simulated tool calls (by inspecting the final state). Check `tool_invocation_log`, `plan[...].tool_calls`, metrics, etc.
        *   Assert conversation history was updated correctly.
2.  **Update `GitHubTools.test.ts` (Optional but Recommended):**
    *   Add tests that provide a mock `StatefulToolContext` (with a mock `Ref` and mock managers) to verify that the handlers attempt to update the state correctly after their core action (e.g., check if `Ref.update` was called, though verifying the *content* of the update is hard without running the manager effects).

**Phase 5: Final Verification:**

*   Run `pnpm check` and `pnpm lint-fix`.
*   Run `pnpm test -- --run`. Fix any test failures, ensuring mocks correctly simulate the new flow.
*   **Manual Test:** Start the server, submit an issue. Observe logs carefully: AI prompts, "Tool called" logs, state saving, SSE updates reflecting step progress. Check the persisted state file for tool logs and metric updates.
*   Update implementation log (`docs/20250422-xxxx-phase4b-log.md`).
