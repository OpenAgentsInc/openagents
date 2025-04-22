Okay, let's break down what happened and create instructions to fix the failing tests and type errors.

**Analysis of Agent's Actions & Current State:**

1.  **Implementation:** The agent successfully created the `PlanManager` and `TaskExecutor` services (`.ts` files) and corresponding test files (`.test.ts`). It also significantly enhanced the tests for the State Storage functions in `test/github/StateStorage.test.ts`. This covers the core implementation tasks for Phase 2a.
2.  **Debugging Attempts:** The agent encountered numerous errors (syntax, type, test failures) and made several attempts to fix them. This involved editing source files and test files, sometimes repeatedly. This is normal during development but indicates the complexity of integrating Effect services and testing them correctly.
3.  **Test Failures (`pnpm test:run`):**
    *   The initial `Unexpected ")"` syntax error in `PlanManager.ts` was likely fixed, but the test runner might still be reporting failures based on *other* issues preventing the tests from running correctly.
    *   The `ReferenceError: Cannot access '__vi_import_1__'` in `StateStorage.test.ts` strongly points to an issue with `vi.mock("effect", ...)` interfering with module initialization. This mocking strategy is fragile and needs revision.
4.  **Type Check Failures (`pnpm verify` -> `tsc`):** This is the most detailed breakdown of the problems:
    *   **Incorrect Tag Definition:** `Effect.Tag<PlanManager>("PlanManager")` is wrong. The generic should be the *interface type*, not the Tag constructor itself. Same for `TaskExecutor`.
    *   **Incorrect Layer Definition:** Passing the Tag constructor (`PlanManager`) to `Layer.succeed` instead of the Tag instance. Also, using `.of({...})` directly on the Tag constructor instead of the Tag instance within `Layer.succeed`.
    *   **Implicit `any` Types:** Parameters inside the service implementations within `Layer.succeed` are not inheriting types from the service interface. They need explicit type annotations.
    *   **Incorrect Service Access:** Using `yield* _(PlanManager)` attempts to yield the Tag *constructor*, not the service instance provided by the context. Should use the yielded service instance (`const planManager = yield* _(PlanManager)`).
    *   **`never` Error Type:** The type of `error` in `catchAll` or `Either.left` is inferred as `never`. Needs explicit handling/casting (e.g., using `Effect.catchAll((error: unknown) => ...)`).
    *   **Test Helper Issues (`runWithPlanManager`):** The helper in `PlanManager.test.ts` incorrectly uses `Effect.flatMap(PlanManager, ...)` passing the Tag constructor.
    *   **Test Mutation:** Directly modifying read-only state fixtures (`initialState.current_task.current_step_index = ...`) in tests (`PlanManager.test.ts`, `StateStorage.test.ts`). Tests must work with copies or returned values.
    *   **Mocking Services in Tests:** Providing mock implementations to `Layer.succeed` using `GitHubClient.of({...})` or `PlanManager.of({...})` is incorrect. Should use `Layer.succeed(GitHubClient, { /* mock methods */ })`. Also, the mock object needs the internal `_tag` property to satisfy the type checker when used with `Effect.flatMap(Tag, ...)` or `yield* _(Tag)`.
    *   **`unknown` Types:** Many `TS18046` errors ('xxx' is of type 'unknown') are consequences of the incorrect Tag/Layer usage upstream.
    *   **Fragile Mocking (`StateStorage.test.ts`):** `vi.mock("effect", ...)` is causing runtime errors. `vi.spyOn` is a safer alternative for specific functions like `logWarning`.

**Conclusion:** The core implementation structure is likely present, but fundamental misunderstandings of Effect's Tag/Layer/Context system and testing patterns are causing widespread type errors and test failures.

---

**Instructions for AI Agent: Fix Failing Tests and Type Errors**

**Objective:** Resolve all TypeScript errors reported by `pnpm check` and ensure all tests pass when running `pnpm test:run` (and therefore `pnpm verify`). Focus *only* on fixing the existing code and tests based on the errors reported.

**Source of Truth:**
*   The error messages from `pnpm verify` and `pnpm test:run`.
*   Effect documentation patterns for `Tag`, `Layer`, testing, and error handling.
*   The previously generated code and tests.

**Instructions:**

**IMPORTANT:** Apply fixes incrementally, file by file. After fixing errors in one file (e.g., `PlanManager.ts`), run `pnpm check` again to see if it resolves dependent errors in other files before proceeding. Run `pnpm test:run` after fixing groups of errors to check test status.

**1. Fix `src/github/PlanManager.ts`:**

*   **Correct Tag Definition:**
    *   Change `export const PlanManager = Effect.Tag<PlanManager>("PlanManager")`
    *   To: `export const PlanManager = Effect.Tag<PlanManager>();` *(The tag identifier "PlanManager" is inferred)*
*   **Correct Layer Definition:**
    *   Change `export const PlanManagerLayer = Layer.succeed(PlanManager, { ... })`
    *   To: `export const PlanManagerLayer = Layer.succeed(PlanManager, PlanManager.of({ ... }))` *(Use the Tag instance `.of`)*
*   **Add Explicit Parameter Types:** Add types to all function parameters inside the `PlanManager.of({...})` implementation block to match the `PlanManager` interface:
    ```typescript
    // Example for addPlanStep:
    addPlanStep: (state: AgentState, description: string) => Effect.sync(() => { /*...*/ }),
    // Example for updateStepStatus:
    updateStepStatus: (state: AgentState, stepId: string, newStatus: PlanStep["status"], resultSummary: string | null = null) => Effect.sync(() => { /*...*/ }).pipe(Effect.catchAll(Effect.fail)), // Keep error handling
    // Add types similarly for addToolCallToStep and getCurrentStep parameters
    ```
*   **Type Check:** Run `pnpm check`. Fix any remaining type errors *within this file*.

**2. Fix `src/github/TaskExecutor.ts`:**

*   **Correct Tag Definition:**
    *   Change `export const TaskExecutor = Effect.Tag<TaskExecutor>("TaskExecutor")`
    *   To: `export const TaskExecutor = Effect.Tag<TaskExecutor>();`
*   **Correct Layer Definition:**
    *   Change `export const TaskExecutorLayer = Layer.effect(TaskExecutor, Effect.gen(function*(_) { ... return { executeNextStep: ... } }))`
    *   To: `export const TaskExecutorLayer = Layer.effect(TaskExecutor, Effect.gen(function*(_) { ... return TaskExecutor.of({ executeNextStep: ... }) }))` *(Use the Tag instance `.of`)*
*   **Correct Service Access:**
    *   Inside `Effect.gen` for the Layer *and* inside `executeNextStep`, ensure you are yielding the Tag instance:
        *   `const planManager = yield* _(PlanManager);` (Correct)
        *   `const githubClient = yield* _(GitHubClient);` (Correct)
    *   Do *not* yield the Tag constructor (e.g., `yield* _(PlanManagerConstructor)` which doesn't exist anyway).
*   **Handle `never` Error Type:** In the `Either.isLeft(result)` block:
    *   Change `const error = result.left;`
    *   To: `const error = result.left as Error;` or `const error = result.left as unknown as Error;` (Use type assertion to tell TypeScript it's an Error).
    *   Alternatively, use `Effect.catchAll((error: unknown) => { /* handle error safely */ })` around the `workEffect`.
*   **Type Check:** Run `pnpm check`. Fix any remaining type errors *within this file*.

**3. Fix `test/github/PlanManager.test.ts`:**

*   **Fix Helper `runWithPlanManager`:** This helper needs to correctly access the service from the provided layer.
    ```typescript
    // Replace the existing helper with this:
    const runWithPlanManager = <A, E>(
        effectToRun: (planManager: PlanManager) => Effect.Effect<A, E> // Function that takes the service
    ) => {
        // Create an effect that gets the PlanManager service from the context
        // and then runs the effect provided by the test, passing the service instance
        const effect = Effect.flatMap(PlanManager, planManagerService => effectToRun(planManagerService));
        // Provide the layer and run sync
        return Effect.runSync(Effect.provide(effect, PlanManagerLayer));
    };
    ```
    *   Ensure all test cases now call this helper correctly: `runWithPlanManager(planManager => planManager.addPlanStep(initialState, description))`
*   **Fix State Mutation:** Remove direct assignments to `initialState`. Create copies or use the state returned by the functions for subsequent calls.
    *   Change `initialState.current_task.current_step_index = 0;`
    *   To: Create a new state object for the test: `const testState = { ...initialState, current_task: { ...initialState.current_task, current_step_index: 0 } };` and use `testState` in the `runWithPlanManager` call. Do similarly for the invalid index test case.
*   **Type Check & Test:** Run `pnpm check` and `pnpm test:run test/github/PlanManager.test.ts`. Fix any remaining errors.

**4. Fix `test/github/StateStorage.test.ts`:**

*   **Fix Fragile Mocking:**
    *   Remove `vi.mock("effect", ...)` entirely.
    *   For testing `logWarning`, use `vi.spyOn`:
        ```typescript
        // At the top level or inside beforeEach/afterEach
        const logWarningSpy = vi.spyOn(Effect, 'logWarning').mockImplementation(() => Effect.void);
        // In your test case:
        // ... run the effect ...
        expect(logWarningSpy).toHaveBeenCalled();
        // Remember to clear/restore the spy in afterEach:
        vi.restoreAllMocks(); // or logWarningSpy.mockRestore();
        ```
*   **Fix State Mutation:** Similar to PlanManager tests, create copies of the `validState` fixture when testing modifications (like changing `state_schema_version`). Do not assign directly to `validState.agent_info.state_schema_version`.
    ```typescript
    // Example for schema version test:
    const oldVersionState = {
        ...createValidTestState(),
        agent_info: {
            ...createValidTestState().agent_info,
            state_schema_version: "1.0" // Modify the copy
        }
    };
    vi.mocked(fs.readFileSync).mockReturnValue(JSON.stringify(oldVersionState));
    // ... run loadAgentState with oldVersionState ...
    ```
*   **Fix Test Execution (`Effect.runPromise` / `Effect.flatMap`):** Ensure the pattern for running effects that depend on `GitHubClient` is correct:
    ```typescript
    // Correct pattern:
    await Effect.runPromise(
      Effect.provide(
        Effect.flatMap(GitHubClient, client => client.loadAgentState(instanceId)), // Get client from context
        GitHubClientLayer // Provide the real layer (or a mock layer if testing client itself)
      )
    )
    ```
*   **Type Check & Test:** Run `pnpm check` and `pnpm test:run test/github/StateStorage.test.ts`. Fix any remaining errors. The `ReferenceError` should be gone now.

**5. Fix `test/github/TaskExecutor.test.ts`:**

*   **Fix Mock Layer Definition:** When creating `MockPlanManager` and `MockGitHubClient` layers:
    ```typescript
     // Correct way to provide a mock service implementation via Layer:
     const MockPlanManager = Layer.succeed(
       PlanManager, // The Tag instance
       PlanManager.of({ // Use the Tag instance's .of method
         addPlanStep: vi.fn().mockReturnValue(Effect.succeed(/* return mock state */)),
         updateStepStatus: updateStepStatusMock, // Assuming this mock returns Effect<AgentState>
         addToolCallToStep: vi.fn().mockReturnValue(Effect.succeed(/* return mock state */)),
         getCurrentStep: getCurrentStepMock // Assuming this mock returns Effect<PlanStep, Error>
       })
     );

     const MockGitHubClient = Layer.succeed(
       GitHubClient, // The Tag instance
       GitHubClient.of({ // Use the Tag instance's .of method
         // Mock ONLY the methods TaskExecutor actually USES (e.g., saveAgentState)
         // Provide realistic mocks that return Effects
         saveAgentState: saveAgentStateMock, // vi.fn().mockReturnValue(Effect.succeed(true))
         // Add other methods from GitHubClient interface with default mocks if needed
         // to satisfy the type, even if not called by TaskExecutor in these tests.
         getIssue: vi.fn().mockReturnValue(Effect.fail("Not mocked")),
         listIssues: vi.fn().mockReturnValue(Effect.fail("Not mocked")),
         // ... mock all other methods defined in the GitHubClient service interface ...
         createAgentStateForIssue: vi.fn().mockReturnValue(Effect.fail("Not mocked")),
       })
     );
    ```
*   **Fix Test Execution (`Effect.flatMap`):** Similar to StateStorage tests, use the correct pattern:
    ```typescript
    // Correct pattern:
    const finalState = await Effect.runPromise( // Use runPromise for async tests
      Effect.provide(
        Effect.flatMap(TaskExecutor, executor => executor.executeNextStep(initialState)), // Get executor from context
        TestTaskExecutorLayer // Provide the layer containing mocks
      )
    );
    ```
*   **Fix `Effect.either` Mocking:** The attempt to mock `Effect.either` globally is problematic. Instead, mock the *result* of the simulated work effect within the test setup.
    ```typescript
    // Inside the test case for failure:
    const mockError = new Error("Simulated failure");
    // Mock the specific service method that TaskExecutor calls during its "work" phase
    // OR, if the work is simple internal logic, directly modify the TaskExecutor mock
    // implementation provided via the Layer for this specific test case to return Effect.fail(mockError).

    // Example: If TaskExecutor called some 'doWork' method on another service:
    // const MockWorkService = Layer.succeed(WorkService, WorkService.of({ doWork: () => Effect.fail(mockError) }));
    // Layer.provide(TestTaskExecutorLayer, MockWorkService) // Provide this additional mock layer

    // If the work is internal to executeNextStep, you might need a more complex mock layer setup
    // for TaskExecutor specifically for the failure test.
    ```
*   **Fix `unknown` Type for `finalState`:** The `TS18046` errors on `finalState` should resolve once the upstream `Effect.flatMap` and Layer issues are fixed, allowing `runPromise`/`runSync` to infer the correct `AgentState` return type.
*   **Type Check & Test:** Run `pnpm check` and `pnpm test:run test/github/TaskExecutor.test.ts`. Fix any remaining errors.

**Final Step:**

*   Run `pnpm verify` one last time. Ensure ALL type checks pass and ALL tests (including the previously passing ones) pass.
*   Update the implementation log (`docs/20250422-1403-phase2a-log.md`) detailing the fixes made and confirming successful verification.

Execute these instructions carefully and methodically. The key is correcting the fundamental Effect patterns related to Tags, Layers, context, and testing.
