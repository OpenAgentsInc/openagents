import { describe, expect, test } from "vite-plus/test";
import { EventEmitter } from "node:events";
import type { ClaudeLocalEvent } from "./claude-local-contract";
import type { ChildLike } from "./codex-child-runtime";
import { codexHarnessExecArgs, runCodexHarnessExecAttempt } from "./codex-harness-attempt";

/** Fixture child that streams the given stdout lines then closes. */
const fixtureChild = (lines: ReadonlyArray<string>): ChildLike => {
  const stdout = new EventEmitter();
  const stderr = new EventEmitter();
  const child = new EventEmitter();
  queueMicrotask(() => {
    for (const line of lines) stdout.emit("data", Buffer.from(`${line}\n`));
    child.emit("close", 0);
  });
  return {
    stdout: stdout as unknown as NodeJS.ReadableStream,
    stderr: stderr as unknown as NodeJS.ReadableStream,
    on: (event, listener) => child.on(event, listener),
    kill: () => true,
    killed: false,
  };
};

const SUCCESS_LINES = [
  `{"type":"thread.started","thread_id":"th-123"}`,
  `{"type":"turn.started"}`,
  `{"type":"item.completed","item":{"id":"i1","type":"reasoning","text":"planning"}}`,
  `{"type":"item.completed","item":{"id":"i2","type":"agent_message","text":"done: 42"}}`,
  `{"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":80,"output_tokens":7,"reasoning_output_tokens":3}}`,
];

const baseInput = (overrides: Partial<Parameters<typeof runCodexHarnessExecAttempt>[0]>) => ({
  threadRef: "thread-1",
  turnRef: "turn-1",
  workspace: "/tmp/w",
  prompt: "compute the answer",
  model: "gpt-5.6-terra",
  reasoningEffort: "medium",
  sandbox: "workspace-write",
  imagePaths: [],
  resumeThreadId: null,
  env: {},
  spawnCodex: () => fixtureChild(SUCCESS_LINES),
  emit: () => {},
  registerChild: () => {},
  ...overrides,
});

describe("codex harness attempt (HARN-09 slice 1)", () => {
  test("fresh turn: emits lowered renderer events, tees usage and thread id", async () => {
    const emitted: ClaudeLocalEvent[] = [];
    const result = await runCodexHarnessExecAttempt(
      baseInput({ emit: (event) => emitted.push(event) }),
    );
    expect(result.outcome).toBe("success");
    expect(result.text).toBe("done: 42");
    expect(result.threadId).toBe("th-123");
    expect(result.usage).toEqual({
      inputTokens: 100,
      cachedInputTokens: 80,
      outputTokens: 7,
      reasoningOutputTokens: 3,
      totalTokens: 110,
    });
    const kinds = emitted.map((event) => event.kind);
    expect(kinds).toContain("turn_started");
    expect(kinds).toContain("reasoning");
    expect(kinds).toContain("text_delta");
    expect(kinds).toContain("turn_completed");
  });

  test("resume turn drives the receipted resume recipe", async () => {
    let seenArgs: ReadonlyArray<string> = [];
    await runCodexHarnessExecAttempt(
      baseInput({
        resumeThreadId: "th-old",
        spawnCodex: (spawn) => {
          seenArgs = spawn.args;
          return fixtureChild(SUCCESS_LINES);
        },
      }),
    );
    expect(seenArgs.slice(0, 3)).toEqual(["exec", "resume", "th-old"]);
    expect(seenArgs).toContain("--skip-git-repo-check");
  });

  test("revoked-token failure classifies as reconnect_required", async () => {
    const result = await runCodexHarnessExecAttempt(
      baseInput({
        spawnCodex: () =>
          fixtureChild([
            `{"type":"thread.started","thread_id":"th-1"}`,
            `{"type":"turn.started"}`,
            `{"type":"turn.failed","error":{"message":"Your access token could not be refreshed because your refresh token was revoked."}}`,
          ]),
      }),
    );
    expect(result.outcome).toBe("reconnect_required");
  });

  test("quota failure sets quotaExhausted", async () => {
    const result = await runCodexHarnessExecAttempt(
      baseInput({
        spawnCodex: () =>
          fixtureChild([
            `{"type":"thread.started","thread_id":"th-1"}`,
            `{"type":"turn.started"}`,
            `{"type":"turn.failed","error":{"message":"You have hit your usage limit."}}`,
          ]),
      }),
    );
    expect(result.outcome).toBe("failed");
    expect(result.quotaExhausted).toBe(true);
  });

  test("fresh-recipe args match the legacy receipted shape", () => {
    const args = codexHarnessExecArgs(
      {
        model: "gpt-5.6-terra",
        reasoningEffort: "medium",
        sandbox: "workspace-write",
        imagePaths: ["/tmp/a.png"],
        workspace: "/tmp/w",
        prompt: "p",
      },
      null,
    );
    expect(args[0]).toBe("exec");
    expect(args).toContain("--json");
    expect(args).toContain("-s");
    expect(args).toContain("-i");
    expect(args[args.length - 1]).toBe("p");
    expect(args[args.indexOf("-C") + 1]).toBe("/tmp/w");
  });
});
