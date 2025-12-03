/**
 * HUD Event Mapping Tests
 *
 * This test file serves dual purposes:
 * 1. Unit tests for the orchestratorEventToHudMessage mapping function
 * 2. Integration tests verifying HUD receives correct messages over WebSocket
 *
 * ## Test Fixtures
 *
 * The file exports reusable fixtures for testing HUD integrations:
 * - SAMPLE_GOLDEN_LOOP_EVENTS: Complete event sequence for a Golden Loop iteration
 * - FORWARDED_EVENT_TYPES: Event types that produce HudMessages
 * - INTERNAL_EVENT_TYPES: Event types filtered out (not sent to HUD)
 *
 * ## Test Structure
 *
 * 1. orchestratorEventToHudMessage tests:
 *    - Forwarded events: Verify each event type produces correct HudMessage
 *    - Internal events: Verify internal events return null (filtered)
 *    - Golden Loop sequence: Verify complete loop produces expected messages
 *
 * 2. HUD emitter integration tests:
 *    - Uses a real WebSocket server to verify message delivery
 *    - Tests filtering (internal events not sent)
 *    - Tests streaming output callback
 *    - Simulates full Golden Loop event sequence
 *
 * @see emit.ts for the implementation and phase mapping documentation
 */

import { describe, expect, test, beforeEach, afterEach } from "bun:test";
import {
  orchestratorEventToHudMessage,
  createHudEmitter,
  createHudOutputCallback,
  createHudCallbacks,
} from "./emit.js";
import type { OrchestratorEvent } from "../agent/orchestrator/types.js";
import type { HudMessage } from "./protocol.js";
import { parseHudMessage } from "./protocol.js";

// ============================================================================
// Test Fixtures: Sample OrchestratorEvents for Golden Loop Phases
//
// These fixtures model a complete Golden Loop iteration, useful for:
// - Testing HUD message filtering
// - Verifying event-to-message mapping
// - Integration testing with mock HUD servers
// ============================================================================

const sampleTask = {
  id: "oa-test123",
  title: "Test Task",
  description: "A test task",
  status: "in_progress" as const,
  priority: 1,
  type: "task" as const,
  labels: [] as readonly string[],
  deps: [] as readonly { readonly id: string; readonly type: "blocks" | "related" | "parent-child" | "discovered-from" }[],
  commits: [] as readonly string[],
  createdAt: "2025-12-03T10:00:00Z",
  updatedAt: "2025-12-03T10:00:00Z",
};

const sampleSubtask = {
  id: "oa-test123-sub-001",
  description: "Implement feature X",
  status: "in_progress" as const,
};

const sampleSubagentResult = {
  success: true,
  subtaskId: "oa-test123-sub-001",
  filesModified: ["src/foo.ts", "src/bar.ts"],
  turns: 5,
  agent: "claude-code" as const,
};

/**
 * Complete set of sample orchestrator events for a full Golden Loop iteration.
 *
 * This fixture models the event sequence for a successful task completion:
 *
 * 1. Session Start (session_start)
 * 2. Orient Phase:
 *    - lock_acquired (internal)
 *    - init_script_start/complete (internal)
 *    - orientation_complete (internal)
 * 3. Select Phase: task_selected
 * 4. Decompose Phase: task_decomposed
 * 5. Execute Phase (per subtask):
 *    - subtask_start
 *    - subtask_complete
 * 6. Verify Phase:
 *    - verification_start (typecheck)
 *    - verification_complete (typecheck)
 *    - verification_start (tests)
 *    - verification_complete (tests)
 * 7. Commit Phase:
 *    - commit_created
 *    - push_complete
 * 8. Update Phase:
 *    - task_updated (internal)
 *    - progress_written (internal)
 * 9. Session End: session_complete
 *
 * Use this to test that the HUD receives the expected subset of messages
 * (internal events should be filtered out).
 *
 * @example
 * ```typescript
 * import { SAMPLE_GOLDEN_LOOP_EVENTS } from "./emit.test.js";
 *
 * for (const event of SAMPLE_GOLDEN_LOOP_EVENTS) {
 *   emit(event);
 * }
 * // HUD should receive ~13 messages (not the full 17 events)
 * ```
 */
export const SAMPLE_GOLDEN_LOOP_EVENTS: OrchestratorEvent[] = [
  // Session start
  { type: "session_start", sessionId: "session-test-123", timestamp: "2025-12-03T10:00:00Z" },

  // Orient phase (internal events - not forwarded)
  { type: "lock_acquired", pid: 12345, sessionId: "session-test-123" },
  { type: "init_script_start", path: ".openagents/init.sh" },
  {
    type: "init_script_complete",
    result: { ran: true, success: true, exitCode: 0, output: "All checks passed" },
  },
  {
    type: "orientation_complete",
    repoState: "clean",
    testsPassingAtStart: true,
    initScript: { ran: true, success: true },
  },

  // Select task phase
  { type: "task_selected", task: sampleTask },

  // Decompose phase
  {
    type: "task_decomposed",
    subtasks: [
      sampleSubtask,
      { id: "oa-test123-sub-002", description: "Add tests", status: "pending" as const },
    ],
  },

  // Execute phase
  { type: "subtask_start", subtask: sampleSubtask },
  {
    type: "subtask_complete",
    subtask: { ...sampleSubtask, status: "done" as const },
    result: sampleSubagentResult,
  },

  // Second subtask
  {
    type: "subtask_start",
    subtask: { id: "oa-test123-sub-002", description: "Add tests", status: "in_progress" as const },
  },
  {
    type: "subtask_complete",
    subtask: { id: "oa-test123-sub-002", description: "Add tests", status: "done" as const },
    result: { ...sampleSubagentResult, subtaskId: "oa-test123-sub-002" },
  },

  // Verify phase
  { type: "verification_start", command: "bun run typecheck" },
  { type: "verification_complete", command: "bun run typecheck", passed: true, output: "" },
  { type: "verification_start", command: "bun test" },
  { type: "verification_complete", command: "bun test", passed: true, output: "42 tests passed" },

  // Commit phase
  { type: "commit_created", sha: "abc123def456", message: "oa-test123: Implement feature X" },
  { type: "push_complete", branch: "main" },

  // Update phase (internal - not forwarded)
  { type: "task_updated", task: { ...sampleTask, status: "closed" as const }, status: "closed" },
  { type: "progress_written", path: ".openagents/progress.md" },

  // Session end
  { type: "session_complete", success: true, summary: "Completed task oa-test123" },
];

/**
 * Event types that ARE forwarded to the HUD (produce HudMessages).
 *
 * These events represent user-visible state changes that the HUD displays:
 * - Session lifecycle (start/complete)
 * - Task selection and decomposition
 * - Subtask execution progress
 * - Verification results
 * - Git operations (commit/push)
 * - Errors
 *
 * Use this list to verify your HUD implementation handles all event types.
 */
export const FORWARDED_EVENT_TYPES = [
  "session_start",
  "session_complete",
  "task_selected",
  "task_decomposed",
  "subtask_start",
  "subtask_complete",
  "subtask_failed",
  "verification_start",
  "verification_complete",
  "commit_created",
  "push_complete",
  "error",
] as const;

/**
 * Event types that are NOT forwarded to the HUD (internal bookkeeping).
 *
 * These events are used by the orchestrator for internal state management
 * but don't need to be displayed to users:
 * - orientation_complete: Initial repo state assessment
 * - init_script_*: Preflight script execution
 * - task_updated: Redundant with task_selected/decomposed
 * - progress_written: Internal file writes
 * - lock_*: Agent lock management
 *
 * orchestratorEventToHudMessage() returns null for these event types.
 */
export const INTERNAL_EVENT_TYPES = [
  "orientation_complete",
  "init_script_start",
  "init_script_complete",
  "task_updated",
  "progress_written",
  "lock_acquired",
  "lock_stale_removed",
  "lock_failed",
  "lock_released",
] as const;

// ============================================================================
// Tests: orchestratorEventToHudMessage
// ============================================================================

describe("orchestratorEventToHudMessage", () => {
  describe("forwarded events", () => {
    test("session_start produces HudMessage", () => {
      const event: OrchestratorEvent = {
        type: "session_start",
        sessionId: "test-session",
        timestamp: "2025-12-03T10:00:00Z",
      };
      const msg = orchestratorEventToHudMessage(event);
      expect(msg).not.toBeNull();
      expect(msg?.type).toBe("session_start");
      expect((msg as any).sessionId).toBe("test-session");
    });

    test("session_complete produces HudMessage", () => {
      const event: OrchestratorEvent = {
        type: "session_complete",
        success: true,
        summary: "Done",
      };
      const msg = orchestratorEventToHudMessage(event);
      expect(msg).not.toBeNull();
      expect(msg?.type).toBe("session_complete");
      expect((msg as any).success).toBe(true);
    });

    test("task_selected produces HudMessage with task info", () => {
      const event: OrchestratorEvent = { type: "task_selected", task: sampleTask };
      const msg = orchestratorEventToHudMessage(event);
      expect(msg).not.toBeNull();
      expect(msg?.type).toBe("task_selected");
      expect((msg as any).task.id).toBe("oa-test123");
      expect((msg as any).task.title).toBe("Test Task");
      expect((msg as any).task.priority).toBe(1);
    });

    test("task_decomposed produces HudMessage with subtask list", () => {
      const event: OrchestratorEvent = {
        type: "task_decomposed",
        subtasks: [sampleSubtask],
      };
      const msg = orchestratorEventToHudMessage(event);
      expect(msg).not.toBeNull();
      expect(msg?.type).toBe("task_decomposed");
      expect((msg as any).subtasks.length).toBe(1);
      expect((msg as any).subtasks[0].id).toBe("oa-test123-sub-001");
    });

    test("subtask_start produces HudMessage", () => {
      const event: OrchestratorEvent = { type: "subtask_start", subtask: sampleSubtask };
      const msg = orchestratorEventToHudMessage(event);
      expect(msg).not.toBeNull();
      expect(msg?.type).toBe("subtask_start");
      expect((msg as any).subtask.id).toBe("oa-test123-sub-001");
    });

    test("subtask_complete produces HudMessage with result", () => {
      const event: OrchestratorEvent = {
        type: "subtask_complete",
        subtask: { ...sampleSubtask, status: "done" },
        result: sampleSubagentResult,
      };
      const msg = orchestratorEventToHudMessage(event);
      expect(msg).not.toBeNull();
      expect(msg?.type).toBe("subtask_complete");
      expect((msg as any).result.success).toBe(true);
      expect((msg as any).result.filesModified).toContain("src/foo.ts");
      expect((msg as any).result.agent).toBe("claude-code");
    });

    test("subtask_failed produces HudMessage with error", () => {
      const event: OrchestratorEvent = {
        type: "subtask_failed",
        subtask: { ...sampleSubtask, status: "failed" },
        error: "Something went wrong",
      };
      const msg = orchestratorEventToHudMessage(event);
      expect(msg).not.toBeNull();
      expect(msg?.type).toBe("subtask_failed");
      expect((msg as any).error).toBe("Something went wrong");
    });

    test("verification_start produces HudMessage", () => {
      const event: OrchestratorEvent = { type: "verification_start", command: "bun test" };
      const msg = orchestratorEventToHudMessage(event);
      expect(msg).not.toBeNull();
      expect(msg?.type).toBe("verification_start");
      expect((msg as any).command).toBe("bun test");
    });

    test("verification_complete produces HudMessage", () => {
      const event: OrchestratorEvent = {
        type: "verification_complete",
        command: "bun test",
        passed: true,
        output: "All tests passed",
      };
      const msg = orchestratorEventToHudMessage(event);
      expect(msg).not.toBeNull();
      expect(msg?.type).toBe("verification_complete");
      expect((msg as any).passed).toBe(true);
    });

    test("commit_created produces HudMessage", () => {
      const event: OrchestratorEvent = {
        type: "commit_created",
        sha: "abc123",
        message: "feat: add feature",
      };
      const msg = orchestratorEventToHudMessage(event);
      expect(msg).not.toBeNull();
      expect(msg?.type).toBe("commit_created");
      expect((msg as any).sha).toBe("abc123");
    });

    test("push_complete produces HudMessage", () => {
      const event: OrchestratorEvent = { type: "push_complete", branch: "main" };
      const msg = orchestratorEventToHudMessage(event);
      expect(msg).not.toBeNull();
      expect(msg?.type).toBe("push_complete");
      expect((msg as any).branch).toBe("main");
    });

    test("error produces HudMessage with phase", () => {
      const event: OrchestratorEvent = {
        type: "error",
        phase: "verifying",
        error: "Tests failed",
      };
      const msg = orchestratorEventToHudMessage(event);
      expect(msg).not.toBeNull();
      expect(msg?.type).toBe("error");
      expect((msg as any).phase).toBe("verifying");
      expect((msg as any).error).toBe("Tests failed");
    });
  });

  describe("internal events (not forwarded)", () => {
    test("orientation_complete returns null", () => {
      const event: OrchestratorEvent = {
        type: "orientation_complete",
        repoState: "clean",
        testsPassingAtStart: true,
      };
      expect(orchestratorEventToHudMessage(event)).toBeNull();
    });

    test("init_script_start returns null", () => {
      const event: OrchestratorEvent = { type: "init_script_start", path: ".openagents/init.sh" };
      expect(orchestratorEventToHudMessage(event)).toBeNull();
    });

    test("init_script_complete returns null", () => {
      const event: OrchestratorEvent = {
        type: "init_script_complete",
        result: { ran: true, success: true },
      };
      expect(orchestratorEventToHudMessage(event)).toBeNull();
    });

    test("task_updated returns null", () => {
      const event: OrchestratorEvent = {
        type: "task_updated",
        task: sampleTask,
        status: "closed",
      };
      expect(orchestratorEventToHudMessage(event)).toBeNull();
    });

    test("progress_written returns null", () => {
      const event: OrchestratorEvent = { type: "progress_written", path: ".openagents/progress.md" };
      expect(orchestratorEventToHudMessage(event)).toBeNull();
    });

    test("lock_acquired returns null", () => {
      const event: OrchestratorEvent = { type: "lock_acquired", pid: 12345 };
      expect(orchestratorEventToHudMessage(event)).toBeNull();
    });

    test("lock_stale_removed returns null", () => {
      const event: OrchestratorEvent = { type: "lock_stale_removed", stalePid: 11111, newPid: 22222 };
      expect(orchestratorEventToHudMessage(event)).toBeNull();
    });

    test("lock_failed returns null", () => {
      const event: OrchestratorEvent = { type: "lock_failed", reason: "Already running" };
      expect(orchestratorEventToHudMessage(event)).toBeNull();
    });

    test("lock_released returns null", () => {
      const event: OrchestratorEvent = { type: "lock_released" };
      expect(orchestratorEventToHudMessage(event)).toBeNull();
    });
  });

  describe("Golden Loop sample event sequence", () => {
    test("produces expected HudMessages for full loop", () => {
      const hudMessages: HudMessage[] = [];
      for (const event of SAMPLE_GOLDEN_LOOP_EVENTS) {
        const msg = orchestratorEventToHudMessage(event);
        if (msg) {
          hudMessages.push(msg);
        }
      }

      // Should filter out internal events
      expect(hudMessages.length).toBeLessThan(SAMPLE_GOLDEN_LOOP_EVENTS.length);

      // Verify expected message types in order
      const types = hudMessages.map((m) => m.type);
      expect(types[0]).toBe("session_start");
      expect(types[1]).toBe("task_selected");
      expect(types[2]).toBe("task_decomposed");
      expect(types).toContain("subtask_start");
      expect(types).toContain("subtask_complete");
      expect(types).toContain("verification_start");
      expect(types).toContain("verification_complete");
      expect(types).toContain("commit_created");
      expect(types).toContain("push_complete");
      expect(types[types.length - 1]).toBe("session_complete");
    });

    test("all forwarded messages are valid JSON round-trip", () => {
      for (const event of SAMPLE_GOLDEN_LOOP_EVENTS) {
        const msg = orchestratorEventToHudMessage(event);
        if (msg) {
          const serialized = JSON.stringify(msg);
          const parsed = parseHudMessage(serialized);
          expect(parsed).not.toBeNull();
          expect(parsed?.type).toBe(msg.type);
        }
      }
    });
  });
});

// ============================================================================
// Tests: HUD Emitter with Mock Server
// ============================================================================

describe("HUD emitter integration", () => {
  let server: ReturnType<typeof Bun.serve> | null = null;
  const TEST_PORT = 54343;
  const TEST_URL = `ws://localhost:${TEST_PORT}`;
  let receivedMessages: HudMessage[] = [];

  beforeEach(() => {
    receivedMessages = [];
    server = Bun.serve({
      port: TEST_PORT,
      fetch(req, server) {
        if (server.upgrade(req, { data: undefined })) {
          return;
        }
        return new Response("Not a WebSocket request", { status: 400 });
      },
      websocket: {
        open() {},
        message(_ws, message) {
          const parsed = parseHudMessage(message.toString());
          if (parsed) {
            receivedMessages.push(parsed);
          }
        },
        close() {},
      },
    });
  });

  afterEach(() => {
    if (server) {
      server.stop();
      server = null;
    }
  });

  test("createHudEmitter sends filtered events to server", async () => {
    const emit = createHudEmitter({ url: TEST_URL });

    // Wait for connection
    await new Promise((r) => setTimeout(r, 100));

    // Emit some events
    emit({ type: "session_start", sessionId: "test", timestamp: "2025-01-01" });
    emit({ type: "orientation_complete", repoState: "clean", testsPassingAtStart: true }); // Not forwarded
    emit({ type: "task_selected", task: sampleTask });

    // Wait for messages
    await new Promise((r) => setTimeout(r, 100));

    // Should only receive 2 messages (orientation_complete filtered out)
    expect(receivedMessages.length).toBe(2);
    expect(receivedMessages[0].type).toBe("session_start");
    expect(receivedMessages[1].type).toBe("task_selected");
  });

  test("createHudOutputCallback sends text_output messages", async () => {
    const onOutput = createHudOutputCallback({ url: TEST_URL }, "claude-code");

    await new Promise((r) => setTimeout(r, 100));

    onOutput("Hello from Claude");
    onOutput("Working on task...");

    await new Promise((r) => setTimeout(r, 100));

    expect(receivedMessages.length).toBe(2);
    expect(receivedMessages[0].type).toBe("text_output");
    expect((receivedMessages[0] as any).text).toBe("Hello from Claude");
    expect((receivedMessages[0] as any).source).toBe("claude-code");
  });

  test("createHudCallbacks provides emit, onOutput, and client", async () => {
    const { emit, onOutput, client } = createHudCallbacks({ url: TEST_URL });

    await new Promise((r) => setTimeout(r, 100));

    emit({ type: "session_start", sessionId: "test", timestamp: "2025-01-01" });
    onOutput("Agent working...");

    await new Promise((r) => setTimeout(r, 100));

    expect(receivedMessages.length).toBe(2);
    expect(receivedMessages[0].type).toBe("session_start");
    expect(receivedMessages[1].type).toBe("text_output");

    client.close();
  });

  test("simulates full Golden Loop event sequence to HUD", async () => {
    const { emit, client } = createHudCallbacks({ url: TEST_URL });

    await new Promise((r) => setTimeout(r, 100));

    // Emit all sample events
    for (const event of SAMPLE_GOLDEN_LOOP_EVENTS) {
      emit(event);
    }

    await new Promise((r) => setTimeout(r, 200));

    // Verify HUD received expected messages
    const receivedTypes = receivedMessages.map((m) => m.type);

    // First message should be session_start
    expect(receivedTypes[0]).toBe("session_start");

    // Should have task_selected
    expect(receivedTypes).toContain("task_selected");

    // Should have task_decomposed
    expect(receivedTypes).toContain("task_decomposed");

    // Should have subtask events
    expect(receivedTypes.filter((t) => t === "subtask_start").length).toBe(2);
    expect(receivedTypes.filter((t) => t === "subtask_complete").length).toBe(2);

    // Should have verification events
    expect(receivedTypes.filter((t) => t === "verification_start").length).toBe(2);
    expect(receivedTypes.filter((t) => t === "verification_complete").length).toBe(2);

    // Should have git events
    expect(receivedTypes).toContain("commit_created");
    expect(receivedTypes).toContain("push_complete");

    // Last message should be session_complete
    expect(receivedTypes[receivedTypes.length - 1]).toBe("session_complete");

    // Should NOT contain internal events
    expect(receivedTypes).not.toContain("orientation_complete");
    expect(receivedTypes).not.toContain("init_script_start");
    expect(receivedTypes).not.toContain("init_script_complete");
    expect(receivedTypes).not.toContain("task_updated");
    expect(receivedTypes).not.toContain("progress_written");
    expect(receivedTypes).not.toContain("lock_acquired");

    client.close();
  });
});
