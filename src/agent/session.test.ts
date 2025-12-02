import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { describe, expect, it } from "bun:test";
import { Effect } from "effect";
import {
  createSession,
  loadSession,
  writeSessionStart,
  writeUserMessage,
  writeTurn,
  writeSessionEnd,
  SessionError,
} from "./session.js";

const runWithBun = <A, E>(program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path>) =>
  Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

describe("session", () => {
  it("creates and loads a session", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "session-test" });
        const sessionPath = path.join(dir, "test-session.jsonl");

        const session = createSession(
          { model: "test-model", systemPrompt: "You are helpful." },
          "Hello, world!",
          "test-session-123",
        );

        yield* writeSessionStart(sessionPath, session);
        yield* writeUserMessage(sessionPath, "Hello, world!");
        yield* writeTurn(sessionPath, {
          role: "assistant",
          content: "Hi there!",
        });
        yield* writeSessionEnd(sessionPath, 1, "Hi there!");

        const loaded = yield* loadSession(sessionPath);
        return loaded;
      }),
    );

    expect(result.id).toBe("test-session-123");
    expect(result.config.model).toBe("test-model");
    expect(result.config.systemPrompt).toBe("You are helpful.");
    expect(result.userMessage).toBe("Hello, world!");
    expect(result.turns.length).toBe(1);
    expect(result.turns[0].content).toBe("Hi there!");
  });

  it("handles tool calls in turns", async () => {
    const result = await runWithBun(
      Effect.gen(function* () {
        const fs = yield* FileSystem.FileSystem;
        const path = yield* Path.Path;
        const dir = yield* fs.makeTempDirectory({ prefix: "session-test" });
        const sessionPath = path.join(dir, "tool-session.jsonl");

        const session = createSession({}, "Run a command", "tool-session");

        yield* writeSessionStart(sessionPath, session);
        yield* writeUserMessage(sessionPath, "Run a command");
        yield* writeTurn(sessionPath, {
          role: "assistant",
          content: null,
          toolCalls: [{ id: "call-1", name: "bash", arguments: '{"command":"echo hi"}' }],
          toolResults: [
            {
              toolCallId: "call-1",
              name: "bash",
              result: { content: [{ type: "text", text: "hi" }] },
              isError: false,
            },
          ],
        });
        yield* writeSessionEnd(sessionPath, 1, null);

        return yield* loadSession(sessionPath);
      }),
    );

    expect(result.turns[0].toolCalls?.length).toBe(1);
    expect(result.turns[0].toolCalls?.[0].name).toBe("bash");
    expect(result.turns[0].toolResults?.length).toBe(1);
    expect(result.turns[0].toolResults?.[0].result.content[0]).toEqual({
      type: "text",
      text: "hi",
    });
  });

  it("fails on missing session file", async () => {
    const error = await runWithBun(
      loadSession("/nonexistent/path/session.jsonl").pipe(Effect.flip),
    );

    expect(error).toBeInstanceOf(SessionError);
    expect((error as SessionError).reason).toBe("not_found");
  });
});
