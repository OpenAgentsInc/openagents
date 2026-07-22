import { describe, expect, test } from "vite-plus/test";
import type { CodexAppServerTransport, CodexEvent } from "@openagentsinc/agent-harness-contract";
import { CodexTransportError } from "@openagentsinc/agent-harness-contract";
import { Effect, Stream } from "effect";
import type { ClaudeLocalEvent } from "./claude-local-contract";
import { runCodexAppServerHarnessAttempt } from "./codex-app-server-harness-attempt";

/** A representative app-server turn: streamed reasoning + text, a command, exact usage. */
const TURN_SCRIPT: ReadonlyArray<CodexEvent> = [
  { type: "turn.started" },
  { type: "reasoning.delta", itemId: "r1", delta: "planning" },
  { type: "agent_message.delta", itemId: "m1", delta: "done: 42" },
  {
    type: "item.started",
    item: {
      itemType: "command_execution",
      id: "c1",
      commandDisplay: "pnpm test",
      status: "in_progress",
    },
  },
  {
    type: "item.completed",
    item: {
      itemType: "command_execution",
      id: "c1",
      commandDisplay: "pnpm test",
      status: "completed",
      exitCode: 0,
    },
  },
  {
    type: "token_usage.updated",
    usage: { inputTokens: 100, cachedInputTokens: 80, outputTokens: 7, reasoningOutputTokens: 3 },
  },
  { type: "turn.completed", status: "completed" },
];

/**
 * Strictly single-element-chunk stream, so a lazy pull-based consumer observes
 * each event ARRIVE and project before the next is produced (openagents#9167
 * live-streaming proof). `onProduce` records production order.
 */
const perElementStream = (
  script: ReadonlyArray<CodexEvent>,
  onProduce?: (event: CodexEvent) => void,
): Stream.Stream<CodexEvent, CodexTransportError> =>
  script.reduce(
    (acc, event) =>
      acc.pipe(
        Stream.concat(
          Stream.fromIterable([event]).pipe(
            Stream.tap((produced) => Effect.sync(() => onProduce?.(produced))),
          ),
        ),
      ),
    Stream.empty as Stream.Stream<CodexEvent, CodexTransportError>,
  );

const makeStreamingTransport = (options?: {
  readonly script?: ReadonlyArray<CodexEvent>;
  readonly onProduce?: (event: CodexEvent) => void;
  readonly failure?: CodexTransportError;
}): CodexAppServerTransport => {
  const script = options?.script ?? TURN_SCRIPT;
  return {
    startThread: (params) => Effect.succeed({ threadId: params.resumeThreadId ?? "th-app-1" }),
    runTurn: () => Effect.succeed(script),
    runTurnStreaming: () => {
      const base = perElementStream(script, options?.onProduce);
      return options?.failure !== undefined
        ? base.pipe(Stream.concat(Stream.fail(options.failure)))
        : base;
    },
    respondToApproval: () => Effect.void,
    interruptTurn: () => Effect.void,
    shutdown: () => Effect.void,
  };
};

const baseInput = (
  transport: CodexAppServerTransport,
  overrides?: Partial<Parameters<typeof runCodexAppServerHarnessAttempt>[0]>,
) => ({
  threadRef: "thread-1",
  turnRef: "turn-1",
  workspace: "/tmp/w",
  prompt: "compute the answer",
  model: "gpt-5.6-terra",
  resumeThreadId: null,
  transport,
  emit: () => {},
  ...overrides,
});

describe("codex app-server harness attempt (HARN-09 slice 1, live streaming)", () => {
  test("live turn: lowers the neutral core stream onto renderer events, tees usage", async () => {
    const emitted: ClaudeLocalEvent[] = [];
    const result = await runCodexAppServerHarnessAttempt(
      baseInput(makeStreamingTransport(), { emit: (event) => emitted.push(event) }),
    );
    expect(result.outcome).toBe("success");
    expect(result.text).toBe("done: 42");
    expect(result.threadId).toBe("th-app-1");
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
    expect(kinds).toContain("tool_use");
    expect(kinds).toContain("tool_result");
    expect(kinds).toContain("turn_completed");
  });

  test("events reach the renderer INCREMENTALLY — before the turn settles on the wire", async () => {
    const log: Array<string> = [];
    await runCodexAppServerHarnessAttempt(
      baseInput(
        makeStreamingTransport({ onProduce: (event) => log.push(`produce:${event.type}`) }),
        {
          emit: (event) => log.push(`receive:${event.kind}`),
        },
      ),
    );
    const firstReceive = log.findIndex((entry) => entry.startsWith("receive:"));
    const lastProduce = log.map((entry) => entry.startsWith("produce:")).lastIndexOf(true);
    // A renderer event was emitted BEFORE the last wire event was produced. A
    // batch drive would emit every `produce:` first, so this fails for it.
    expect(firstReceive).toBeGreaterThanOrEqual(0);
    expect(firstReceive).toBeLessThan(lastProduce);
    // The text delta reaches the renderer before the turn completes on the wire.
    const receiveText = log.indexOf("receive:text_delta");
    const produceCompleted = log.indexOf("produce:turn.completed");
    expect(receiveText).toBeGreaterThanOrEqual(0);
    expect(produceCompleted).toBeGreaterThan(receiveText);
  });

  test("a rate-limit wire error classifies as rateLimited", async () => {
    const result = await runCodexAppServerHarnessAttempt(
      baseInput(
        makeStreamingTransport({
          script: [
            { type: "turn.started" },
            { type: "error", messageSafe: "429 too many requests" },
          ],
        }),
      ),
    );
    expect(result.outcome).toBe("failed");
    expect(result.rateLimited).toBe(true);
  });

  test("a transport failure classifies as reconnect_required", async () => {
    const result = await runCodexAppServerHarnessAttempt(
      baseInput(
        makeStreamingTransport({
          failure: new CodexTransportError({
            failureClass: "account_reconnect_required",
            detail: "401 unauthorized: sign in again",
          }),
        }),
      ),
    );
    expect(result.outcome).toBe("reconnect_required");
  });

  test("resume passes the prior thread id into the adapter session", async () => {
    let resumeSeen: string | undefined;
    const transport: CodexAppServerTransport = {
      ...makeStreamingTransport(),
      startThread: (params) =>
        Effect.sync(() => {
          resumeSeen = params.resumeThreadId;
          return { threadId: params.resumeThreadId ?? "th-app-1" };
        }),
    };
    const result = await runCodexAppServerHarnessAttempt(
      baseInput(transport, { resumeThreadId: "th-prior" }),
    );
    expect(resumeSeen).toBe("th-prior");
    expect(result.threadId).toBe("th-prior");
  });
});
