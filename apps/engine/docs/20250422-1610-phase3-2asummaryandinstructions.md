Okay, fantastic! The agent successfully navigated the complexities of the Effect framework's testing and dependency injection patterns, particularly the tricky filesystem mocking. `pnpm verify` passing cleanly is a major milestone.

**Analysis of Final Agent Actions:**

1.  **DI Refactor:** Correctly refactored `GitHub.ts` to use `@effect/platform/FileSystem` via dependency injection.
2.  **Test Refactor (`StateStorage.test.ts`):** Correctly refactored the tests to provide a mock `FileSystem` layer using `Layer.succeed` and `FileSystem.of`, eliminating the need for `vi.mock("node:fs")` and resolving the persistent initialization errors.
3.  **Test Logic Restoration:** Successfully restored and adapted the original test logic for `saveAgentState`, `loadAgentState`, and `createAgentStateForIssue` to work with the new DI pattern.
4.  **Final Verification:** Achieved a clean `pnpm verify` pass, confirming type safety, linting, and passing tests.

**Current Status:** Phase 1 (State Types) and Phase 2 (State Storage, Plan Manager, Task Executor) are implemented and verified (type-checked, linted, tested). The core infrastructure for state persistence, planning, and basic execution is in place.

**Next Step: Phase 3 - Context and Memory (Brain)**

According to the implementation plan, the next phase involves building the agent's "working memory" and contextual understanding. This involves creating two new services: `ContextManager` and `MemoryManager`.

We will start with `ContextManager`.

---

**Instructions for AI Agent: Implement Phase 3 - `ContextManager` Service & Tests**

**Objective:** Implement the `ContextManager` service responsible for managing the `execution_context` section of the `AgentState`, along with comprehensive unit tests.

**Target Files:**
*   `src/github/ContextManager.ts` (New File)
*   `test/github/ContextManager.test.ts` (New File)

**Source of Truth:**
*   `docs/agent-state.md` (for the structure of `execution_context`).
*   `src/github/AgentStateTypes.ts` (for `AgentState`, `ExecutionContext`, `FileFocus`, `CodeSnippet`, `ExternalReference` types).
*   Established Effect patterns (Tag, Layer, Immutability).

**Instructions:**

**1. Create `src/github/ContextManager.ts`:**

*   **Import Dependencies:** Import `Effect`, `Layer`, and relevant types from `./AgentStateTypes.ts` (`AgentState`, `ExecutionContext`, `FileFocus`, `CodeSnippet`, `ExternalReference`).
*   **Define Service Interface (`ContextManager`):**
    ```typescript
    export interface ContextManager {
        readonly setFileFocus: (
            state: AgentState,
            filePath: string,
            relevantLines: ReadonlyArray<number>
        ) => Effect.Effect<AgentState>; // Returns updated state

        readonly addCodeSnippet: (
            state: AgentState,
            filePath: string,
            snippet: string,
            reason: string
        ) => Effect.Effect<AgentState>;

        readonly addExternalReference: (
            state: AgentState,
            type: string,
            identifier: string,
            relationship: string,
            source: string
        ) => Effect.Effect<AgentState>;

        readonly addModifiedFile: (
            state: AgentState,
            filePath: string
        ) => Effect.Effect<AgentState>;

        readonly clearFileFocus: (
            state: AgentState
        ) => Effect.Effect<AgentState>;
    }
    ```
*   **Define Service Tag:** Use the class-based pattern:
    ```typescript
    // eslint-disable-next-line @typescript-eslint/no-unsafe-declaration-merging
    export class ContextManager extends Effect.Tag("ContextManager")<ContextManager, ContextManager>() {}
    ```
*   **Implement Service Logic (within `Layer.succeed`):** Create `ContextManagerLayer`. Implement each function from the interface. **CRITICAL: Ensure immutability.** Always return a *new* `AgentState` object with the modified `execution_context`.
    ```typescript
    export const ContextManagerLive = Layer.succeed(
        ContextManager,
        ContextManager.of({
            setFileFocus: (state, filePath, relevantLines) => Effect.sync(() => ({
                ...state,
                execution_context: {
                    ...state.execution_context,
                    current_file_focus: { path: filePath, relevant_lines: relevantLines } // Create new FileFocus object
                }
            })),

            addCodeSnippet: (state, filePath, snippet, reason) => Effect.sync(() => {
                const newSnippet: CodeSnippet = { file_path: filePath, snippet, reason };
                return {
                    ...state,
                    execution_context: {
                        ...state.execution_context,
                        // Create new array with added snippet
                        relevant_code_snippets: [...state.execution_context.relevant_code_snippets, newSnippet]
                    }
                };
            }),

            addExternalReference: (state, type, identifier, relationship, source) => Effect.sync(() => {
                 const newRef: ExternalReference = { type, identifier, relationship, source };
                 return {
                     ...state,
                     execution_context: {
                         ...state.execution_context,
                         external_references: [...state.execution_context.external_references, newRef]
                     }
                 };
            }),

            addModifiedFile: (state, filePath) => Effect.sync(() => {
                // Avoid duplicates if necessary (using Set)
                const updatedFiles = new Set(state.execution_context.files_modified_in_session);
                updatedFiles.add(filePath);
                return {
                    ...state,
                    execution_context: {
                        ...state.execution_context,
                        files_modified_in_session: Array.from(updatedFiles) // Convert back to array
                    }
                };
            }),

            clearFileFocus: (state) => Effect.sync(() => ({
                ...state,
                execution_context: {
                    ...state.execution_context,
                    current_file_focus: null
                }
            }))
        })
    );

    // Alias for consistency if needed
    export const ContextManagerLayer = ContextManagerLive;
    ```

**2. Create `test/github/ContextManager.test.ts`:**

*   **Import Dependencies:** `@effect/vitest`, `Effect`, `ContextManager`, `ContextManagerLayer`, `AgentState` fixture creator (copy/adapt from other tests).
*   **Setup Test Helper (Optional but Recommended):** Similar to `PlanManager.test.ts`, create a helper to run effects providing the `ContextManagerLayer`.
    ```typescript
    import { describe, it, expect } from "@effect/vitest";
    import { Effect } from "effect";
    import { ContextManager, ContextManagerLayer } from "../../src/github/ContextManager.js";
    import type { AgentState } from "../../src/github/AgentStateTypes.js";
    // Import or define createTestState fixture here

    const runWithContextManager = <A>(effectToRun: (manager: ContextManager) => Effect.Effect<A, never>) => {
        const effect = Effect.flatMap(ContextManager, manager => effectToRun(manager));
        return Effect.runSync(Effect.provide(effect, ContextManagerLayer));
    };
    ```
*   **Write Unit Tests:** Create tests for *each* method in the `ContextManager` interface.
    *   **Arrange:** Create an initial `AgentState` using your fixture.
    *   **Act:** Call the `ContextManager` method using the helper (e.g., `runWithContextManager(manager => manager.setFileFocus(initialState, "path/to/file.ts", [10, 20]))`).
    *   **Assert:**
        *   Check that the returned `AgentState` has the expected modifications ONLY within the `execution_context` section.
        *   Verify other parts of the state remain unchanged.
        *   **Verify Immutability:** Assert that the returned state object and the modified nested objects (`execution_context`, `relevant_code_snippets`, etc.) are *not* the same instances as the originals (`expect(newState).not.toBe(initialState)`, `expect(newState.execution_context).not.toBe(initialState.execution_context)`).
    *   Cover edge cases (e.g., adding duplicate modified files, clearing focus when already null).

**3. Verification:**

*   Run `pnpm check` to ensure no type errors were introduced.
*   Run `pnpm test:run test/github/ContextManager.test.ts` to ensure the new tests pass.
*   Run `pnpm verify` to confirm everything integrates cleanly.
*   Create and update a new implementation log file (e.g., `docs/20250422-xxxx-phase3a-log.md`) documenting the creation of `ContextManager` and its tests, and paste the successful `pnpm verify` output.

This sets the stage for implementing `MemoryManager` next.
