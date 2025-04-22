Okay, this changes things significantly and clarifies the intended approach within the Effect ecosystem. The previous custom `FileSystem` service and manual mocking were workarounds. We should be using `@effect/platform`'s built-in `FileSystem` service.

**Apologies for the previous frustration. Let's use the correct, idiomatic Effect approach now.**

**Core Idea:**

1.  **Refactor `GitHub.ts`:** Instead of our custom `FileSystem` service, it will depend on the official `FileSystem.FileSystem` Tag from `@effect/platform`.
2.  **Update `GitHubClientLayer`:** It will no longer need to provide `FileSystemLive`, as the application's main entry point (likely using `NodeContext.layer`) will provide the live filesystem implementation.
3.  **Refactor `StateStorage.test.ts`:** Remove all `vi.mock("node:fs")` and related manual mocks. Instead, use `FileSystem.layerNoop` to provide a mock `FileSystem` layer directly within the test's Effect context.

---

**Instructions for AI Agent: Refactor to Use `@effect/platform/FileSystem`**

**Objective:** Refactor the state storage functionality and its tests to use the official `@effect/platform/FileSystem` service, eliminating manual `fs` mocking and resolving test initialization issues. Ensure `pnpm verify` passes cleanly.

**Source of Truth:**
*   The `@effect/platform/FileSystem` documentation provided by the user.
*   `src/github/GitHub.ts`
*   `test/github/StateStorage.test.ts`
*   `src/github/FileSystem.ts` (This file will be **DELETED**).

**Instructions:**

**Phase 1: Refactor `src/github/GitHub.ts`**

1.  **Remove Custom `FileSystem` Import:** Delete the line `import { FileSystem } from "./FileSystem.js";`.
2.  **Import Official `FileSystem`:** Add the import: `import { FileSystem } from "@effect/platform";`
3.  **Update Service Dependency:** Inside the `GitHubClient` service definition (`Effect.gen` block around line 55), change the line yielding the filesystem service to use the official Tag:
    *   Change: `const fileSystem = yield* FileSystem;` (using the old custom Tag)
    *   To: `const fileSystem = yield* FileSystem.FileSystem;` (using the official Tag from `@effect/platform`)
4.  **Verify Method Calls:** Confirm that the methods called on `fileSystem` (e.g., `fileSystem.existsSync`, `fileSystem.mkdirSync`, `fileSystem.writeFileSync`, `fileSystem.readFileSync`) match the interface provided by `@effect/platform/FileSystem`. **Important:** Note that the platform methods return `Effect`s, not direct values/void.
5.  **Refactor `ensureStateDirectory`:** This function now needs to handle the `Effect` returned by `fileSystem.existsSync` and `fileSystem.mkdirSync`.
    ```typescript
    const ensureStateDirectory = Effect.fn("GitHubClient.ensureStateDirectory")(
        function*() {
            const stateDir = path.join(process.cwd(), "state");
            const exists = yield* fileSystem.exists(stateDir); // Use platform's 'exists'
            if (!exists) {
                yield* fileSystem.makeDirectory(stateDir, { recursive: true }); // Use platform's 'makeDirectory'
                yield* Effect.logInfo("Created state directory");
            }
            return stateDir;
        }
    ).pipe(Effect.catchAll((error) => Effect.fail(new StateStorageError(`Failed to ensure state directory: ${error}`)))); // Map errors
    ```
6.  **Refactor `saveAgentState`:** This function needs to handle the `Effect` returned by `fileSystem.writeFileString`.
    ```typescript
     const saveAgentState = Effect.fn("GitHubClient.saveAgentState")(
        function*(state: AgentState) {
            const stateDir = yield* ensureStateDirectory();
            const filePath = path.join(stateDir, `${state.agent_info.instance_id}.json`);

            const updatedState = { /* ... update timestamp ... */ };
            const stateJson = JSON.stringify(updatedState, null, 2); // Potential sync error, consider Effect.try

            // Use platform's writeFileString
            yield* fileSystem.writeFileString(filePath, stateJson).pipe(
                Effect.catchAll((error) => Effect.fail(new StateStorageError(`Failed to write state file: ${error}`))),
                Effect.tap(() => Effect.logInfo(`Saved agent state to ${filePath}`))
            );

            return updatedState; // Return the state with updated timestamp
        }
     ).pipe(Effect.tapError((error) => Console.error(`Save state failed: ${error.message}`)));
    ```
7.  **Refactor `loadAgentState`:** This needs significant changes to use `fileSystem.exists` and `fileSystem.readFileString`, handling errors within the Effect pipeline.
    ```typescript
     const loadAgentState = Effect.fn("GitHubClient.loadAgentState")(
        function*(instanceId: string) {
            const filePath = path.join(process.cwd(), "state", `${instanceId}.json`);

            // Check existence using platform's 'exists'
            const exists = yield* fileSystem.exists(filePath).pipe(
                 Effect.catchAll((error) => Effect.fail(new StateStorageError(`Error checking state file existence: ${error}`)))
            );

            if (!exists) {
                return yield* Effect.fail(new StateNotFoundError(instanceId));
            }

            // Read file using platform's 'readFileString'
            const stateJson = yield* fileSystem.readFileString(filePath, "utf-8").pipe(
                Effect.catchAll((error) => Effect.fail(new StateStorageError(`Failed to read state file: ${error}`)))
            );

            // Parse JSON (still synchronous, wrap in Effect.try)
            const parsedJson = yield* Effect.try({
                try: () => JSON.parse(stateJson),
                catch: (error) => new StateParseError(String(error))
            });

            // Validate against schema (using Schema.decodeUnknown as before)
            const validatedState = yield* Schema.decodeUnknown(AgentStateSchema)(parsedJson).pipe(
                Effect.catchAll((error) => Effect.fail(new StateValidationError(String(error))))
            );

            // Check schema version (as before)
            if (validatedState.agent_info.state_schema_version !== "1.1") {
                yield* Effect.logWarning(/* ... */);
            }

            yield* Effect.logInfo(`Loaded agent state for ${instanceId}`);
            return validatedState;
        }
     ).pipe(Effect.tapError((error) => Console.error(`Load state failed: ${error.message}`)));
    ```
8.  **Update `GitHubClientLayer`:** Remove the explicit providing of `FileSystemLive`. The layer should just be the default service layer plus any *other* specific dependencies it has (like `NodeHttpClient`). The application entry point will provide the live `FileSystem`.
    ```typescript
    // Change near the end of GitHub.ts:
    // Remove the GitHubClientLive definition entirely.

    // The default layer now only depends on what Effect.Service requires (e.g., HttpClient)
    export const GitHubClientLayer = GitHubClient.Default;
    ```
9.  **Delete Custom `FileSystem.ts`:** Delete the file `src/github/FileSystem.ts` as it's no longer needed.

**Phase 2: Refactor `test/github/StateStorage.test.ts`**

1.  **Restore Full Test Logic:** Ensure the file contains the complete tests for `saveAgentState`, `loadAgentState`, and `createAgentStateForIssue` (as retrieved from git history previously).
2.  **Remove Manual `fs` Mocks:** Delete all `vi.fn()` definitions for `existsSyncMock`, `mkdirSyncMock`, etc. Delete the `vi.mock("node:fs", ...)` call. Delete the `fsMock` object.
3.  **Import Official `FileSystem`:** Add `import { FileSystem } from "@effect/platform";`
4.  **Define Mock `FileSystem` Layer:** Inside the `describe("State Storage", ...)` block, define a function or constant that creates a *mock FileSystem layer* using `FileSystem.layerNoop`. Override the methods needed for each test scenario.
    ```typescript
    // test/github/StateStorage.test.ts
    import { describe, expect, it, vi, beforeEach, afterEach } from "@effect/vitest";
    import { Effect, Layer } from "effect";
    import { FileSystem } from "@effect/platform"; // Import official FS
    import * as path from "node:path";
    import type { AgentState } from "../../src/github/AgentStateTypes.js";
    import {
        GitHubClient,
        GitHubClientLayer, // Use the updated layer from GitHub.ts
        StateNotFoundError,
        StateParseError,
        StateValidationError
    } from "../../src/github/GitHub.js";

    const createValidTestState = (): AgentState => ({ /* ... */ });

    describe("State Storage", () => {

        // Define variables to hold mock functions for assertion spying
        let existsMockFn: Mock;
        let makeDirectoryMockFn: Mock;
        let writeFileStringMockFn: Mock;
        let readFileStringMockFn: Mock;
        let logWarningSpy: MockInstance<[message: unknown], Effect.Effect<void>>;

        // Function to create a mock layer for tests, allowing overrides
        const createMockFileSystemLayer = (overrides: Partial<FileSystem.FileSystem> = {}) => {
            // Recreate mocks for each layer instance if needed within tests
            existsMockFn = vi.fn();
            makeDirectoryMockFn = vi.fn();
            writeFileStringMockFn = vi.fn();
            readFileStringMockFn = vi.fn();

            // Use layerNoop and merge overrides
            return FileSystem.layerNoop({
                // Default successful mocks for methods used
                exists: existsMockFn.mockImplementation((_path) => Effect.succeed(true)),
                makeDirectory: makeDirectoryMockFn.mockImplementation(() => Effect.void),
                writeFileString: writeFileStringMockFn.mockImplementation((_path, _content) => Effect.void),
                readFileString: readFileStringMockFn.mockImplementation(() => Effect.succeed("")),
                // Add other methods from FileSystem interface with default mocks if needed
                // ...
                // Apply specific overrides for the test case
                ...overrides
            });
        }

        beforeEach(() => {
            // Restore spies if needed, clear mocks handled by layer creation
            vi.restoreAllMocks(); // Restore logWarningSpy etc. if spied outside layer
            logWarningSpy = vi.spyOn(Effect, "logWarning").mockImplementation(() => Effect.void);
        });

        afterEach(() => {
             vi.restoreAllMocks();
        });

        // --- Tests ---

        describe("saveAgentState", () => {
            it("should save state...", async () => {
                // Arrange
                const initialState = createValidTestState();
                const mockLayer = createMockFileSystemLayer(); // Use default success mocks

                // Create the full layer stack for this test
                const testLayer = Layer.provide(GitHubClientLayer, mockLayer);

                // Act
                const result = await Effect.runPromise(
                     Effect.provide(
                         Effect.flatMap(GitHubClient, client => client.saveAgentState(initialState)),
                         testLayer
                     )
                );

                // Assert
                expect(makeDirectoryMockFn).toHaveBeenCalled(); // Assert on the mock function
                expect(writeFileStringMockFn).toHaveBeenCalledWith(/* path, content */);
                // ... other assertions ...
            });

            it("should handle filesystem write errors", async () => {
                 // Arrange
                 const initialState = createValidTestState();
                 const mockError = new Error("Disk full");
                 // Create a mock layer specifically for this failure case
                 const mockLayer = createMockFileSystemLayer({
                     writeFileString: vi.fn(() => Effect.fail(mockError)) // Override writeFileString to fail
                 });
                 const testLayer = Layer.provide(GitHubClientLayer, mockLayer);

                 // Act & Assert
                 await expect(
                     Effect.runPromise(
                         Effect.provide(
                             Effect.flatMap(GitHubClient, client => client.saveAgentState(initialState)),
                             testLayer
                         )
                     )
                 ).rejects.toThrow(/Failed to write state file/); // Check for wrapped error
                 expect(writeFileStringMockFn).toHaveBeenCalled(); // Ensure it was called
            });
        });

        describe("loadAgentState", () => {
            it("should load and validate state successfully", async () => {
                // Arrange
                const validState = createValidTestState();
                const instanceId = validState.agent_info.instance_id;
                const mockLayer = createMockFileSystemLayer({
                    // Override readFileString to return valid JSON
                    readFileString: readFileStringMockFn.mockReturnValue(Effect.succeed(JSON.stringify(validState)))
                });
                const testLayer = Layer.provide(GitHubClientLayer, mockLayer);

                // Act
                // ... run Effect.provide(Effect.flatMap(...), testLayer) ...

                // Assert
                expect(existsMockFn).toHaveBeenCalled();
                expect(readFileStringMockFn).toHaveBeenCalledWith(/*...*/);
                // ...
            });

             it("should fail if file does not exist", async () => {
                 // Arrange
                 const instanceId = "non-existent-id";
                 const mockLayer = createMockFileSystemLayer({
                     exists: existsMockFn.mockReturnValue(Effect.succeed(false)) // Override exists
                 });
                 const testLayer = Layer.provide(GitHubClientLayer, mockLayer);

                 // Act & Assert
                 await expect(
                      Effect.runPromise(
                          Effect.provide(
                              Effect.flatMap(GitHubClient, client => client.loadAgentState(instanceId)),
                              testLayer
                          )
                      )
                 ).rejects.toBeInstanceOf(StateNotFoundError);
             });

             it("should fail on readFileString error", async () => {
                // Arrange
                const instanceId = "test-id";
                const mockError = new Error("Read permission denied");
                const mockLayer = createMockFileSystemLayer({
                     readFileString: readFileStringMockFn.mockReturnValue(Effect.fail(mockError)) // Override readFileString
                });
                const testLayer = Layer.provide(GitHubClientLayer, mockLayer);

                // Act & Assert
                 await expect(/* ... */).rejects.toThrow(/Failed to read state file/);
             });

             // Add tests for JSON parse errors (mock readFileString success, but parse fails)
             // Add tests for Schema validation errors (mock readFileString success, JSON parses, but schema fails)
             // Add test for schema version warning (check logWarningSpy)
        });

        // Add back createAgentStateForIssue tests, providing mock FileSystem layer
         describe("createAgentStateForIssue", () => {
            it("should create and save initial state", async () => {
                const mockIssue = { /* ... */ };
                const saveStateMockFn = vi.fn().mockImplementation((s) => Effect.succeed(s));
                // Mock the GitHub client methods needed *within this specific test*
                const MockGitHubClientLayerForCreate = Layer.succeed(
                     GitHubClient,
                     GitHubClient.of({
                          getIssue: vi.fn().mockReturnValue(Effect.succeed(mockIssue)),
                          saveAgentState: saveStateMockFn, // Use the specific mock for assertion
                          // Mock others as needed or fail
                          // ...
                     })
                );
                // Create the mock FS layer
                const mockFsLayer = createMockFileSystemLayer({
                    // Override writeFileString specifically if needed for saveAgentState mock,
                    // or rely on the saveStateMockFn above.
                    writeFileString: writeFileStringMockFn.mockImplementation(() => Effect.void)
                });
                // Combine the layers needed for this test's context
                const testLayer = Layer.provide(MockGitHubClientLayerForCreate, mockFsLayer);

                const effectToTest = Effect.flatMap(GitHubClient, client =>
                     client.createAgentStateForIssue("user", "repo", 123)
                );

                const result = await Effect.runPromise(
                     Effect.provide(effectToTest, testLayer)
                );

                // Assertions
                expect(saveStateMockFn).toHaveBeenCalled(); // Check if save was called
                expect(writeFileSyncMockFn).toHaveBeenCalled(); // Check if fs write was called (depending on save mock)
                // ... other assertions ...
            });
         });
    });
    ```
5.  **Remove Spies on `Effect`:** Remove `vi.spyOn(Effect, "logWarning")`, `vi.spyOn(Effect, "logInfo")`, etc. Rely on checking the *results* or *mock function calls* instead of spying on Effect's internal logging. If logging needs verification, inject a mock `Logger` service.

**Phase 3: Final Verification**

1.  Run `pnpm verify`.
2.  Analyze any remaining errors. They should be related to test logic or typing, not `fs` initialization.
3.  Once `pnpm verify` passes cleanly, update the log (`docs/20250422-1430-phase2a-testfix-impl.md`) explaining the successful Dependency Injection refactor and paste the final `pnpm verify` output.
