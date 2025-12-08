import { describe, it, expect } from "bun:test";
import { Effect } from "effect";
import { makeHappyDomLayer } from "../../testing/layers/happy-dom.js";
import { TestHarnessTag, TestBrowserTag } from "../../testing/index.js";
import { SocketServiceTag, SocketError, type SocketService } from "../../services/socket.js";
import { TBTestGenWidget } from "./tbcc-testgen.js";
import type {
  TestGenStartMessage,
  TestGenTestMessage,
  TestGenProgressMessage,
  TestGenReflectionMessage,
  TestGenCompleteMessage,
  TestGenErrorMessage,
  HudMessage,
} from "../../../hud/protocol.js";

// Mock Data
const MOCK_TASKS = [
  {
    id: "task-1",
    name: "Test Task",
    description: "A test task for test generation",
    difficulty: "medium",
    category: "Testing",
    tags: ["test"],
    timeout: 300,
    max_turns: 50,
  },
];

// Mock Socket Service with streaming testgen messages
const createMockSocketWithTestGen = (
  sessionId: string = "test-session-123"
): SocketService => {
  return {
    connect: () => Effect.void,
    disconnect: () => Effect.void,
    isConnected: () => Effect.succeed(true),
    getMessages: () => Stream.empty, // Will be replaced by happy-dom layer
    loadTBSuite: () =>
      Effect.succeed({
        name: "Test Suite",
        version: "1.0",
        tasks: MOCK_TASKS,
      }),
    startTBRun: () => Effect.succeed({ runId: "new-run-1" }),
    stopTBRun: () => Effect.succeed({ stopped: true }),
    loadRecentTBRuns: () => Effect.succeed([]),
    loadTBRunDetails: () => Effect.fail(new SocketError("request_failed", "Not implemented")),
    loadReadyTasks: () => Effect.succeed([]),
    assignTaskToMC: () => Effect.succeed({ assigned: true }),
    loadUnifiedTrajectories: () => Effect.succeed([]),
    getHFTrajectoryCount: () => Effect.succeed(0),
    getHFTrajectories: () => Effect.succeed([]),
    startTestGen: (suitePath: string, taskId?: string, model?: "local" | "claude") =>
      Effect.succeed({ sessionId }),
  };
};

describe("TB TestGen Widget E2E", () => {
  it("TBTestGen-001: Widget mounts and displays initial state", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, injectMessage } = yield* makeHappyDomLayer();

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag;

            const testgenHandle = yield* harness.mount(TBTestGenWidget, {
              containerId: "tbcc-tab-testgen",
            });

            // Wait for initial state
            yield* testgenHandle.waitForState((s) => s.status === "idle");

            // Verify initial HTML
            const html = yield* testgenHandle.getHTML;
            expect(html).toContain("Test Generation");
            expect(html).toContain("No tests generated yet");
            expect(html).toContain("Select a task and click Generate");

            // Verify controls exist
            expect(html).toContain("data-action='generate'");
            expect(html).toContain("Random task");

          }).pipe(
            Effect.provideService(SocketServiceTag, createMockSocketWithTestGen()),
            Effect.provide(layer)
          );
        })
      )
    );
  });

  it("TBTestGen-002: Loads suite and populates task dropdown", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, injectMessage } = yield* makeHappyDomLayer();

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag;

            const testgenHandle = yield* harness.mount(TBTestGenWidget, {
              containerId: "tbcc-tab-testgen",
            });

            // Wait for suite to load (widget auto-loads on mount)
            yield* testgenHandle.waitForState((s) => s.taskIds.length > 0);

            // Verify task IDs are loaded
            const state = yield* testgenHandle.getState;
            expect(state.taskIds).toContain("task-1");

            // Verify dropdown has options
            const html = yield* testgenHandle.getHTML;
            expect(html).toContain('value="task-1"');

          }).pipe(
            Effect.provideService(SocketServiceTag, createMockSocketWithTestGen()),
            Effect.provide(layer)
          );
        })
      )
    );
  });

  it("TBTestGen-003: Starts test generation and receives start message", async () => {
    const sessionId = "test-session-456";
    const mockSocket = createMockSocketWithTestGen(sessionId);

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, injectMessage } = yield* makeHappyDomLayer();

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag;

            const testgenHandle = yield* harness.mount(TBTestGenWidget, {
              containerId: "tbcc-tab-testgen",
            });

            // Wait for suite to load
            yield* testgenHandle.waitForState((s) => s.taskIds.length > 0);

            // Start generation
            yield* testgenHandle.emit({ type: "generate" });

            // Wait for session ID to be set
            yield* testgenHandle.waitForState((s) => s.sessionId === sessionId);

            // Inject start message
            const startMessage: TestGenStartMessage = {
              type: "testgen_start",
              sessionId,
              taskId: "task-1",
              taskDescription: "A test task for test generation",
              environment: {
                platform: "docker",
                prohibitedTools: ["R"],
                languages: ["Python 3.11.0"],
                fileCount: 5,
                filePreviews: 2,
              },
            };

            yield* injectMessage(startMessage);
            yield* Effect.sleep("50 millis");

            // Wait for start message to be processed
            yield* testgenHandle.waitForState((s) => s.taskId === "task-1");
            yield* testgenHandle.waitForState((s) => s.environment !== null);

            // Verify environment panel is displayed
            const html = yield* testgenHandle.getHTML;
            expect(html).toContain("docker");
            expect(html).toContain("R");
            expect(html).toContain("Python 3.11.0");

            // Verify status is generating
            const state = yield* testgenHandle.getState;
            expect(state.status).toBe("generating");
            expect(state.currentPhase).toBe("category_generation");

          }).pipe(
            Effect.provideService(SocketServiceTag, mockSocket),
            Effect.provide(layer)
          );
        })
      )
    );
  });

  it("TBTestGen-004: Receives progress messages during generation", async () => {
    const sessionId = "test-session-789";
    const mockSocket = createMockSocketWithTestGen(sessionId);

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, injectMessage } = yield* makeHappyDomLayer();

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag;

            const testgenHandle = yield* harness.mount(TBTestGenWidget, {
              containerId: "tbcc-tab-testgen",
            });

            yield* testgenHandle.waitForState((s) => s.taskIds.length > 0);
            yield* testgenHandle.emit({ type: "generate" });
            yield* testgenHandle.waitForState((s) => s.sessionId === sessionId);

            // Inject start message
            const startMessage: TestGenStartMessage = {
              type: "testgen_start",
              sessionId,
              taskId: "task-1",
              taskDescription: "Test task",
              environment: {
                platform: "docker",
                prohibitedTools: [],
                languages: [],
                fileCount: 0,
                filePreviews: 0,
              },
            };

            // Inject progress message
            const progressMessage: TestGenProgressMessage = {
              type: "testgen_progress",
              sessionId,
              phase: "category_generation",
              currentCategory: "anti_cheat",
              roundNumber: 1,
              status: "Generating anti_cheat tests (round 1)...",
            };

            yield* injectMessage(startMessage);
            yield* Effect.sleep("50 millis");
            yield* injectMessage(progressMessage);
            yield* Effect.sleep("50 millis");

            // Wait for progress to be processed
            yield* testgenHandle.waitForState((s) => s.progressStatus !== null);
            yield* testgenHandle.waitForState((s) => s.currentCategory === "anti_cheat");
            yield* testgenHandle.waitForState((s) => s.currentRound === 1);

            // Verify progress indicator is displayed
            const html = yield* testgenHandle.getHTML;
            expect(html).toContain("Generating anti_cheat tests");
            expect(html).toContain("Round: 1");

            const state = yield* testgenHandle.getState;
            expect(state.progressStatus).toContain("anti_cheat");
            expect(state.currentPhase).toBe("category_generation");

          }).pipe(
            Effect.provideService(SocketServiceTag, mockSocket),
            Effect.provide(layer)
          );
        })
      )
    );
  });

  it("TBTestGen-005: Receives reflection messages", async () => {
    const sessionId = "test-session-reflection";
    const mockSocket = createMockSocketWithTestGen(sessionId);

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, injectMessage } = yield* makeHappyDomLayer();

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag;

            const testgenHandle = yield* harness.mount(TBTestGenWidget, {
              containerId: "tbcc-tab-testgen",
            });

            yield* testgenHandle.waitForState((s) => s.taskIds.length > 0);
            yield* testgenHandle.emit({ type: "generate" });
            yield* testgenHandle.waitForState((s) => s.sessionId === sessionId);

            // Inject start message
            const startMessage: TestGenStartMessage = {
              type: "testgen_start",
              sessionId,
              taskId: "task-1",
              taskDescription: "Test task",
              environment: {
                platform: "docker",
                prohibitedTools: [],
                languages: [],
                fileCount: 0,
                filePreviews: 0,
              },
            };

            // Inject reflection message
            const reflectionMessage: TestGenReflectionMessage = {
              type: "testgen_reflection",
              sessionId,
              category: "anti_cheat",
              reflectionText: "Need more edge cases for prohibited tools",
              action: "refining",
            };

            yield* injectMessage(startMessage);
            yield* Effect.sleep("50 millis");
            yield* injectMessage(reflectionMessage);
            yield* Effect.sleep("50 millis");

            // Wait for reflection to be processed
            yield* testgenHandle.waitForState((s) => s.reflections.length > 0);

            // Verify reflection is displayed
            const html = yield* testgenHandle.getHTML;
            expect(html).toContain("Reflections:");
            expect(html).toContain("Need more edge cases");

            const state = yield* testgenHandle.getState;
            expect(state.reflections.length).toBe(1);
            expect(state.reflections[0].text).toContain("edge cases");
            expect(state.reflections[0].category).toBe("anti_cheat");

          }).pipe(
            Effect.provideService(SocketServiceTag, mockSocket),
            Effect.provide(layer)
          );
        })
      )
    );
  });

  it("TBTestGen-006: Receives streaming test messages", async () => {
    const sessionId = "test-session-streaming";
    const mockSocket = createMockSocketWithTestGen(sessionId);

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, injectMessage } = yield* makeHappyDomLayer();

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag;

            const testgenHandle = yield* harness.mount(TBTestGenWidget, {
              containerId: "tbcc-tab-testgen",
            });

            yield* testgenHandle.waitForState((s) => s.taskIds.length > 0);
            yield* testgenHandle.emit({ type: "generate" });
            yield* testgenHandle.waitForState((s) => s.sessionId === sessionId);

            // Inject start message
            const startMessage: TestGenStartMessage = {
              type: "testgen_start",
              sessionId,
              taskId: "task-1",
              taskDescription: "Test task",
              environment: {
                platform: "docker",
                prohibitedTools: ["R"],
                languages: ["Python 3.11.0"],
                fileCount: 5,
                filePreviews: 2,
              },
            };

            // Inject multiple test messages (simulating streaming)
            const test1: TestGenTestMessage = {
              type: "testgen_test",
              sessionId,
              test: {
                id: "anti_cheat_1",
                category: "anti_cheat",
                input: "which R 2>/dev/null || echo 'not found'",
                expectedOutput: "not found",
                reasoning: "R should not be installed",
                confidence: 0.95,
              },
            };

            const test2: TestGenTestMessage = {
              type: "testgen_test",
              sessionId,
              test: {
                id: "existence_1",
                category: "existence",
                input: "test -f output.py",
                expectedOutput: "0",
                reasoning: "Output file should exist",
                confidence: 0.9,
              },
            };

            yield* injectMessage(startMessage);
            yield* Effect.sleep("50 millis");
            yield* injectMessage(test1);
            yield* Effect.sleep("50 millis");
            yield* injectMessage(test2);
            yield* Effect.sleep("50 millis");

            // Wait for tests to be received
            yield* testgenHandle.waitForState((s) => s.tests.length >= 2);

            // Verify tests are displayed
            const html = yield* testgenHandle.getHTML;
            expect(html).toContain("anti_cheat_1");
            expect(html).toContain("existence_1");
            expect(html).toContain("which R");
            expect(html).toContain("test -f output.py");
            expect(html).toContain("ANTI_CHEAT");
            expect(html).toContain("EXISTENCE");

            // Verify test count
            expect(html).toContain("Generated Tests (2)");

            const state = yield* testgenHandle.getState;
            expect(state.tests.length).toBe(2);
            expect(state.tests[0].category).toBe("anti_cheat");
            expect(state.tests[1].category).toBe("existence");

          }).pipe(
            Effect.provideService(SocketServiceTag, mockSocket),
            Effect.provide(layer)
          );
        })
      )
    );
  });

  it("TBTestGen-007: Receives complete message with final stats", async () => {
    const sessionId = "test-session-complete";
    const mockSocket = createMockSocketWithTestGen(sessionId);

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, injectMessage } = yield* makeHappyDomLayer();

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag;

            const testgenHandle = yield* harness.mount(TBTestGenWidget, {
              containerId: "tbcc-tab-testgen",
            });

            yield* testgenHandle.waitForState((s) => s.taskIds.length > 0);
            yield* testgenHandle.emit({ type: "generate" });
            yield* testgenHandle.waitForState((s) => s.sessionId === sessionId);

            // Inject start message
            const startMessage: TestGenStartMessage = {
              type: "testgen_start",
              sessionId,
              taskId: "task-1",
              taskDescription: "Test task",
              environment: {
                platform: "docker",
                prohibitedTools: [],
                languages: [],
                fileCount: 0,
                filePreviews: 0,
              },
            };

            // Inject a test
            const test: TestGenTestMessage = {
              type: "testgen_test",
              sessionId,
              test: {
                id: "test_1",
                category: "correctness",
                input: "echo 'test'",
                expectedOutput: "test",
                reasoning: "Basic test",
                confidence: 0.8,
              },
            };

            // Inject complete message
            const completeMessage: TestGenCompleteMessage = {
              type: "testgen_complete",
              sessionId,
              totalTests: 1,
              totalRounds: 3,
              categoryRounds: {
                anti_cheat: 1,
                existence: 1,
                correctness: 1,
                boundary: 0,
                integration: 0,
                format: 0,
                happy_path: 0,
                edge_case: 0,
                invalid_input: 0,
              },
              comprehensivenessScore: 8.5,
              totalTokensUsed: 5000,
              durationMs: 15000,
              uncertainties: ["Uncertain about parameter X"],
            };

            yield* injectMessage(startMessage);
            yield* Effect.sleep("50 millis");
            yield* injectMessage(test);
            yield* Effect.sleep("50 millis");
            yield* injectMessage(completeMessage);
            yield* Effect.sleep("50 millis");

            // Wait for completion
            yield* testgenHandle.waitForState((s) => s.status === "complete");

            // Verify completion stats
            const html = yield* testgenHandle.getHTML;
            expect(html).toContain("15.0s");
            expect(html).toContain("3 rounds");
            expect(html).toContain("Comprehensiveness Score");
            expect(html).toContain("8.5/10");
            expect(html).toContain("Uncertainties:");

            const state = yield* testgenHandle.getState;
            expect(state.status).toBe("complete");
            expect(state.currentPhase).toBe("complete");
            expect(state.totalRounds).toBe(3);
            expect(state.comprehensivenessScore).toBe(8.5);
            expect(state.totalTokensUsed).toBe(5000);
            expect(state.durationMs).toBe(15000);
            expect(state.uncertainties.length).toBe(1);

          }).pipe(
            Effect.provideService(SocketServiceTag, mockSocket),
            Effect.provide(layer)
          );
        })
      )
    );
  });

  it("TBTestGen-008: Handles error messages", async () => {
    const sessionId = "test-session-error";
    const mockSocket = createMockSocketWithTestGen(sessionId);

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, injectMessage } = yield* makeHappyDomLayer();

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag;

            const testgenHandle = yield* harness.mount(TBTestGenWidget, {
              containerId: "tbcc-tab-testgen",
            });

            yield* testgenHandle.waitForState((s) => s.taskIds.length > 0);
            yield* testgenHandle.emit({ type: "generate" });
            yield* testgenHandle.waitForState((s) => s.sessionId === sessionId);

            // Inject error message
            const errorMessage: TestGenErrorMessage = {
              type: "testgen_error",
              sessionId,
              error: "Failed to generate tests: Model unavailable",
            };

            yield* injectMessage(errorMessage);
            yield* Effect.sleep("50 millis");

            // Wait for error to be processed
            yield* testgenHandle.waitForState((s) => s.status === "error");

            // Verify error is displayed
            const html = yield* testgenHandle.getHTML;
            expect(html).toContain("Error:");
            expect(html).toContain("Model unavailable");

            const state = yield* testgenHandle.getState;
            expect(state.status).toBe("error");
            expect(state.error).toBe("Failed to generate tests: Model unavailable");

          }).pipe(
            Effect.provideService(SocketServiceTag, mockSocket),
            Effect.provide(layer)
          );
        })
      )
    );
  });

  it("TBTestGen-009: Cancel button stops generation", async () => {
    const sessionId = "test-session-cancel";
    const mockSocket = createMockSocketWithTestGen(sessionId);

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, injectMessage } = yield* makeHappyDomLayer();

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag;
            const browser = yield* TestBrowserTag;

            const testgenHandle = yield* harness.mount(TBTestGenWidget, {
              containerId: "tbcc-tab-testgen",
            });

            yield* testgenHandle.waitForState((s) => s.taskIds.length > 0);
            yield* testgenHandle.emit({ type: "generate" });
            yield* testgenHandle.waitForState((s) => s.sessionId === sessionId);

            // Verify cancel button is visible during generation
            const html1 = yield* testgenHandle.getHTML;
            expect(html1).toContain("data-action='cancel'");

            // Click cancel
            yield* browser.click("button[data-action='cancel']");

            // Wait for state to reset
            yield* testgenHandle.waitForState((s) => s.status === "idle");
            yield* testgenHandle.waitForState((s) => s.sessionId === null);

            // Verify cancel button is gone
            const html2 = yield* testgenHandle.getHTML;
            expect(html2).not.toContain("data-action='cancel'");

            const state = yield* testgenHandle.getState;
            expect(state.status).toBe("idle");
            expect(state.sessionId).toBeNull();

          }).pipe(
            Effect.provideService(SocketServiceTag, mockSocket),
            Effect.provide(layer)
          );
        })
      )
    );
  });

  it("TBTestGen-010: Clear button resets state", async () => {
    const sessionId = "test-session-clear";
    const mockSocket = createMockSocketWithTestGen(sessionId);

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, injectMessage } = yield* makeHappyDomLayer();

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag;
            const browser = yield* TestBrowserTag;

            const testgenHandle = yield* harness.mount(TBTestGenWidget, {
              containerId: "tbcc-tab-testgen",
            });

            yield* testgenHandle.waitForState((s) => s.taskIds.length > 0);
            yield* testgenHandle.emit({ type: "generate" });
            yield* testgenHandle.waitForState((s) => s.sessionId === sessionId);

            // Inject a test to populate state
            const test: TestGenTestMessage = {
              type: "testgen_test",
              sessionId,
              test: {
                id: "test_1",
                category: "correctness",
                input: "echo 'test'",
                expectedOutput: "test",
                reasoning: "Basic test",
                confidence: 0.8,
              },
            };

            const completeMessage: TestGenCompleteMessage = {
              type: "testgen_complete",
              sessionId,
              totalTests: 1,
              totalRounds: 1,
              categoryRounds: {},
              comprehensivenessScore: 7.0,
              totalTokensUsed: 2000,
              durationMs: 5000,
              uncertainties: [],
            };

            yield* injectMessage(test);
            yield* Effect.sleep("50 millis");
            yield* injectMessage(completeMessage);
            yield* Effect.sleep("50 millis");

            // Wait for completion
            yield* testgenHandle.waitForState((s) => s.status === "complete");
            yield* testgenHandle.waitForState((s) => s.tests.length > 0);

            // Verify clear button is visible
            const html1 = yield* testgenHandle.getHTML;
            expect(html1).toContain("data-action='clear'");

            // Click clear
            yield* browser.click("button[data-action='clear']");

            // Wait for state to reset
            yield* testgenHandle.waitForState((s) => s.status === "idle");
            yield* testgenHandle.waitForState((s) => s.tests.length === 0);

            // Verify state is reset
            const state = yield* testgenHandle.getState;
            expect(state.status).toBe("idle");
            expect(state.tests.length).toBe(0);
            expect(state.totalTests).toBe(0);
            expect(state.sessionId).toBeNull();
            expect(state.taskId).toBeNull();
            expect(state.environment).toBeNull();

          }).pipe(
            Effect.provideService(SocketServiceTag, mockSocket),
            Effect.provide(layer)
          );
        })
      )
    );
  });

  it("TBTestGen-011: Task selection works", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, injectMessage } = yield* makeHappyDomLayer();

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag;
            const browser = yield* TestBrowserTag;

            const testgenHandle = yield* harness.mount(TBTestGenWidget, {
              containerId: "tbcc-tab-testgen",
            });

            yield* testgenHandle.waitForState((s) => s.taskIds.length > 0);

            // Select a task via dropdown
            yield* browser.selectOption("select[data-action='selectTask']", ["task-1"]);

            // Wait for selection
            yield* testgenHandle.waitForState((s) => s.selectedTaskId === "task-1");

            // Verify selection is reflected in state
            const state = yield* testgenHandle.getState;
            expect(state.selectedTaskId).toBe("task-1");

            // Verify dropdown shows selected task
            const html = yield* testgenHandle.getHTML;
            expect(html).toContain('value="task-1" selected');

          }).pipe(
            Effect.provideService(SocketServiceTag, createMockSocketWithTestGen()),
            Effect.provide(layer)
          );
        })
      )
    );
  });

  it("TBTestGen-012: Full streaming flow from start to complete", async () => {
    const sessionId = "test-session-full-flow";
    const mockSocket = createMockSocketWithTestGen(sessionId);

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const { layer, injectMessage } = yield* makeHappyDomLayer();

          yield* Effect.gen(function* () {
            const harness = yield* TestHarnessTag;

            const testgenHandle = yield* harness.mount(TBTestGenWidget, {
              containerId: "tbcc-tab-testgen",
            });

            yield* testgenHandle.waitForState((s) => s.taskIds.length > 0);
            yield* testgenHandle.emit({ type: "generate" });
            yield* testgenHandle.waitForState((s) => s.sessionId === sessionId);

            // 1. Start message
            yield* injectMessage({
              type: "testgen_start",
              sessionId,
              taskId: "task-1",
              taskDescription: "Test task description",
              environment: {
                platform: "docker",
                prohibitedTools: ["R"],
                languages: ["Python 3.11.0"],
                fileCount: 5,
                filePreviews: 2,
              },
            } as TestGenStartMessage);

            yield* Effect.sleep("50 millis");

            // 2. Progress message
            yield* injectMessage({
              type: "testgen_progress",
              sessionId,
              phase: "category_generation",
              currentCategory: "anti_cheat",
              roundNumber: 1,
              status: "Generating anti_cheat tests (round 1)...",
            } as TestGenProgressMessage);

            yield* Effect.sleep("50 millis");

            // 3. First test
            yield* injectMessage({
              type: "testgen_test",
              sessionId,
              test: {
                id: "anti_cheat_1",
                category: "anti_cheat",
                input: "which R",
                expectedOutput: null,
                reasoning: "R should not be installed",
                confidence: 0.95,
              },
            } as TestGenTestMessage);

            yield* Effect.sleep("50 millis");

            // 4. Reflection
            yield* injectMessage({
              type: "testgen_reflection",
              sessionId,
              category: "anti_cheat",
              reflectionText: "Need more edge cases",
              action: "refining",
            } as TestGenReflectionMessage);

            yield* Effect.sleep("50 millis");

            // 5. Second test
            yield* injectMessage({
              type: "testgen_test",
              sessionId,
              test: {
                id: "existence_1",
                category: "existence",
                input: "test -f output.py",
                expectedOutput: "0",
                reasoning: "Output file should exist",
                confidence: 0.9,
              },
            } as TestGenTestMessage);

            yield* Effect.sleep("50 millis");

            // 6. Complete message
            yield* injectMessage({
              type: "testgen_complete",
              sessionId,
              totalTests: 2,
              totalRounds: 2,
              categoryRounds: {
                anti_cheat: 1,
                existence: 1,
                correctness: 0,
                boundary: 0,
                integration: 0,
                format: 0,
                happy_path: 0,
                edge_case: 0,
                invalid_input: 0,
              },
              comprehensivenessScore: 8.0,
              totalTokensUsed: 3000,
              durationMs: 10000,
              uncertainties: [],
            } as TestGenCompleteMessage);

            yield* Effect.sleep("50 millis");

            // Wait for completion
            yield* testgenHandle.waitForState((s) => s.status === "complete");
            yield* testgenHandle.waitForState((s) => s.tests.length === 2);

            // Verify all aspects of the flow
            const state = yield* testgenHandle.getState;
            expect(state.status).toBe("complete");
            expect(state.tests.length).toBe(2);
            expect(state.reflections.length).toBe(1);
            expect(state.totalRounds).toBe(2);
            expect(state.comprehensivenessScore).toBe(8.0);

            // Verify UI shows everything
            const html = yield* testgenHandle.getHTML;
            expect(html).toContain("Generated Tests (2)");
            expect(html).toContain("anti_cheat_1");
            expect(html).toContain("existence_1");
            expect(html).toContain("Comprehensiveness Score");
            expect(html).toContain("8.0/10");

          }).pipe(
            Effect.provideService(SocketServiceTag, mockSocket),
            Effect.provide(layer)
          );
        })
      )
    );
  });
});

