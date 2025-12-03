import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, it } from "bun:test";
import { Effect } from "effect";
import { makeSessionService, SessionServiceError } from "./service.js";

const runWithBun = <A, E>(program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path>) =>
  Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

describe("SessionService", () => {
  it("creates a session and logs messages", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const dir = yield* fs.makeTempDirectory({ prefix: "session-test" });
        const service = yield* makeSessionService({ sessionsDir: dir });

        // Start session
        let session = yield* service.startSession({
          taskId: "oa-test123",
          model: "test-model",
          provider: "test-provider",
        });

        expect(session.sessionId).toContain("session-");
        expect(session.taskId).toBe("oa-test123");
        expect(session.turnCount).toBe(0);

        // Log user message
        session = yield* service.logUserMessage(session, "Hello, world!");
        expect(session.lastUuid).toBeTruthy();

        // Log assistant message with usage
        session = yield* service.logAssistantMessage(session, "Hi there!", {
          model: "test-model",
          usage: {
            inputTokens: 100,
            outputTokens: 50,
            totalCostUsd: 0.01,
          },
        });
        expect(session.turnCount).toBe(1);
        expect(session.cumulativeUsage.inputTokens).toBe(100);
        expect(session.cumulativeUsage.outputTokens).toBe(50);

        // Log tool result
        session = yield* service.logToolResult(session, "call-123", { content: "tool output" });

        // End session
        yield* service.endSession(session, "success", {
          reason: "Task completed",
          commits: ["abc123"],
        });

        // Load and verify
        const entries = yield* service.loadSession(session.sessionId);
        expect(entries.length).toBe(5); // start, user, assistant, tool_result, end

        const startEntry = entries.find((e) => e.type === "session_start");
        expect(startEntry?.type).toBe("session_start");
        if (startEntry?.type === "session_start") {
          expect(startEntry.taskId).toBe("oa-test123");
        }

        const endEntry = entries.find((e) => e.type === "session_end");
        expect(endEntry?.type).toBe("session_end");
        if (endEntry?.type === "session_end") {
          expect(endEntry.outcome).toBe("success");
          expect(endEntry.totalTurns).toBe(1);
          expect(endEntry.commits).toEqual(["abc123"]);
        }

        return session;
      }),
    );

    expect(result.sessionId).toBeTruthy();
  });

  it("lists sessions and gets metadata", async () => {
    await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const dir = yield* fs.makeTempDirectory({ prefix: "session-test" });
        const service = yield* makeSessionService({ sessionsDir: dir });

        // Create two sessions
        let session1 = yield* service.startSession({ taskId: "task-1" });
        session1 = yield* service.logUserMessage(session1, "First session message");
        yield* service.endSession(session1, "success");

        let session2 = yield* service.startSession({ taskId: "task-2" });
        session2 = yield* service.logUserMessage(session2, "Second session message");
        yield* service.endSession(session2, "failure", { reason: "Test failure" });

        // List sessions
        const sessionIds = yield* service.listSessions();
        expect(sessionIds.length).toBe(2);

        // Get metadata
        const metadata1 = yield* service.getSessionMetadata(session1.sessionId);
        expect(metadata1.taskId).toBe("task-1");
        expect(metadata1.outcome).toBe("success");
        expect(metadata1.firstUserMessage).toBe("First session message");

        const metadata2 = yield* service.getSessionMetadata(session2.sessionId);
        expect(metadata2.taskId).toBe("task-2");
        expect(metadata2.outcome).toBe("failure");
      }),
    );
  });

  it("searches sessions by content", async () => {
    await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const dir = yield* fs.makeTempDirectory({ prefix: "session-test" });
        const service = yield* makeSessionService({ sessionsDir: dir });

        // Create sessions with different content
        let session1 = yield* service.startSession({ taskId: "task-search-1" });
        session1 = yield* service.logUserMessage(session1, "This is about authentication");
        yield* service.endSession(session1, "success");

        let session2 = yield* service.startSession({ taskId: "task-search-2" });
        session2 = yield* service.logUserMessage(session2, "This is about database queries");
        yield* service.endSession(session2, "success");

        // Search for "authentication"
        const results = yield* service.searchSessions("authentication");
        expect(results.length).toBe(1);
        expect(results[0].taskId).toBe("task-search-1");

        // Search for "database"
        const results2 = yield* service.searchSessions("database");
        expect(results2.length).toBe(1);
        expect(results2[0].taskId).toBe("task-search-2");

        // Search for something not found
        const results3 = yield* service.searchSessions("nonexistent");
        expect(results3.length).toBe(0);
      }),
    );
  });

  it("finds sessions by task ID", async () => {
    await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const dir = yield* fs.makeTempDirectory({ prefix: "session-test" });
        const service = yield* makeSessionService({ sessionsDir: dir });

        // Create multiple sessions for same task
        let session1 = yield* service.startSession({ taskId: "oa-shared-task" });
        session1 = yield* service.logUserMessage(session1, "First attempt");
        yield* service.endSession(session1, "blocked");

        let session2 = yield* service.startSession({ taskId: "oa-shared-task" });
        session2 = yield* service.logUserMessage(session2, "Second attempt");
        yield* service.endSession(session2, "success");

        let session3 = yield* service.startSession({ taskId: "oa-other-task" });
        session3 = yield* service.logUserMessage(session3, "Different task");
        yield* service.endSession(session3, "success");

        // Find sessions for shared task
        const results = yield* service.findSessionsByTask("oa-shared-task");
        expect(results.length).toBe(2);

        // Find sessions for other task
        const results2 = yield* service.findSessionsByTask("oa-other-task");
        expect(results2.length).toBe(1);
      }),
    );
  });

  it("handles tool use content in assistant messages", async () => {
    await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const dir = yield* fs.makeTempDirectory({ prefix: "session-test" });
        const service = yield* makeSessionService({ sessionsDir: dir });

        let session = yield* service.startSession({});

        // Log assistant message with tool use
        session = yield* service.logAssistantMessage(
          session,
          [
            { type: "text", text: "Let me read that file" },
            { type: "tool_use", id: "call-456", name: "Read", input: { file_path: "/test/file.ts" } },
          ],
          { model: "test-model" },
        );

        // Log tool result
        session = yield* service.logToolResult(session, "call-456", "file contents here");

        yield* service.endSession(session, "success");

        // Verify entries
        const entries = yield* service.loadSession(session.sessionId);
        const assistantEntry = entries.find((e) => e.type === "assistant");
        expect(assistantEntry?.type).toBe("assistant");
        if (assistantEntry?.type === "assistant") {
          const content = assistantEntry.message.content;
          expect(Array.isArray(content)).toBe(true);
          if (Array.isArray(content)) {
            expect(content.length).toBe(2);
            expect(content[1]).toMatchObject({
              type: "tool_use",
              id: "call-456",
              name: "Read",
            });
          }
        }
      }),
    );
  });

  it("tracks modified files", async () => {
    await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const dir = yield* fs.makeTempDirectory({ prefix: "session-test" });
        const service = yield* makeSessionService({ sessionsDir: dir });

        let session = yield* service.startSession({});
        session = service.trackFileModified(session, "/path/to/file1.ts");
        session = service.trackFileModified(session, "/path/to/file2.ts");
        session = service.trackFileModified(session, "/path/to/file1.ts"); // Duplicate

        expect(session.filesModified.size).toBe(2);

        yield* service.endSession(session, "success");

        const entries = yield* service.loadSession(session.sessionId);
        const endEntry = entries.find((e) => e.type === "session_end");
        if (endEntry?.type === "session_end") {
          expect(endEntry.filesModified?.length).toBe(2);
          expect(endEntry.filesModified).toContain("/path/to/file1.ts");
          expect(endEntry.filesModified).toContain("/path/to/file2.ts");
        }
      }),
    );
  });

  it("fails gracefully on non-existent session", async () => {
    const error = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const dir = yield* fs.makeTempDirectory({ prefix: "session-test" });
        const service = yield* makeSessionService({ sessionsDir: dir });

        return yield* service.loadSession("non-existent-session").pipe(Effect.flip);
      }),
    );

    expect(error).toBeInstanceOf(SessionServiceError);
    expect((error as SessionServiceError).reason).toBe("not_found");
  });

  it("accumulates usage across multiple assistant messages", async () => {
    await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const dir = yield* fs.makeTempDirectory({ prefix: "session-test" });
        const service = yield* makeSessionService({ sessionsDir: dir });

        let session = yield* service.startSession({});

        session = yield* service.logAssistantMessage(session, "First response", {
          usage: { inputTokens: 100, outputTokens: 50, totalCostUsd: 0.01 },
        });

        session = yield* service.logAssistantMessage(session, "Second response", {
          usage: { inputTokens: 200, outputTokens: 100, totalCostUsd: 0.02 },
        });

        session = yield* service.logAssistantMessage(session, "Third response", {
          usage: { inputTokens: 150, outputTokens: 75, totalCostUsd: 0.015 },
        });

        expect(session.cumulativeUsage.inputTokens).toBe(450);
        expect(session.cumulativeUsage.outputTokens).toBe(225);
        expect(session.cumulativeUsage.totalCostUsd).toBeCloseTo(0.045, 5);
        expect(session.turnCount).toBe(3);
      }),
    );
  });
});
