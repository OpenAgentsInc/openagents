import type { Cause } from "effect";
import { Deferred, Effect, Fiber, Queue, Stream, SubscriptionRef } from "effect";
import type { KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema";
import { describe, expect, test } from "vite-plus/test";

import { toolIdentity } from "./common-tool.ts";
import { buildTextDelta, buildTurnFinished, buildTurnStarted } from "./event-builder.ts";
import { khalaEventToUiChunks, type UiMessageChunk } from "./ui-message-chunk.ts";
import {
  applyUiChunk,
  initialUiMessage,
  reduceUiMessageStream,
  UiMessageReducerError,
  type UiMessage,
  type UiToolPart,
} from "./ui-message-reducer.ts";

const BASH = toolIdentity("Bash");

/** Fold chunks into every intermediate snapshot, starting from the initial. */
const snapshotsOf = (
  chunks: ReadonlyArray<UiMessageChunk>,
  start: UiMessage = initialUiMessage(),
): ReadonlyArray<UiMessage> => {
  const snapshots: Array<UiMessage> = [start];
  let current = start;
  for (const chunk of chunks) {
    current = applyUiChunk(current, chunk);
    snapshots.push(current);
  }
  return snapshots;
};

const textOf = (message: UiMessage): string =>
  message.parts
    .filter((part) => part.type === "text")
    .map((part) => part.text)
    .join("");

const toolPartOf = (message: UiMessage): UiToolPart => {
  const part = message.parts.find((candidate) => candidate.type === "tool");
  if (part === undefined) throw new Error("expected a tool part");
  return part;
};

describe("applyUiChunk — progressive snapshots", () => {
  test("text grows across intermediate snapshots and closes on finish", () => {
    const chunks: ReadonlyArray<UiMessageChunk> = [
      { type: "message-start", messageId: "t1" },
      { type: "text-delta", id: "msg.t1", delta: "Hello " },
      { type: "text-delta", id: "msg.t1", delta: "world" },
      { type: "text-end", id: "msg.t1" },
      { type: "message-finish", finishReason: "stop" },
    ];
    const snapshots = snapshotsOf(chunks);

    // Intermediate states, not just the final one.
    expect(snapshots.map(textOf)).toEqual([
      "",
      "",
      "Hello ",
      "Hello world",
      "Hello world",
      "Hello world",
    ]);
    expect(snapshots.map((snapshot) => snapshot.status)).toEqual([
      "streaming",
      "streaming",
      "streaming",
      "streaming",
      "streaming",
      "complete",
    ]);

    const final = snapshots[snapshots.length - 1]!;
    expect(final.id).toBe("t1");
    expect(final.finishReason).toBe("stop");
    expect(final.parts).toEqual([
      { type: "text", id: "msg.t1", text: "Hello world", state: "done" },
    ]);
  });

  test("the tool-call state machine transitions land in order with accumulated input", () => {
    const chunks: ReadonlyArray<UiMessageChunk> = [
      {
        type: "tool-input-streaming",
        toolCallId: "call.1",
        tool: BASH,
        inputTextDelta: '{"command":',
      },
      { type: "tool-input-streaming", toolCallId: "call.1", tool: BASH, inputTextDelta: '"ls"}' },
      { type: "tool-input-available", toolCallId: "call.1", tool: BASH, inputRef: "input.call.1" },
      {
        type: "tool-output-available",
        toolCallId: "call.1",
        tool: BASH,
        resultRef: "result.call.1",
      },
    ];
    const snapshots = snapshotsOf(chunks).slice(1);
    const toolStates = snapshots.map((snapshot) => toolPartOf(snapshot).state);

    expect(toolStates).toEqual([
      "input-streaming",
      "input-streaming",
      "input-available",
      "output-available",
    ]);
    expect(snapshots.map((snapshot) => toolPartOf(snapshot).inputText)).toEqual([
      '{"command":',
      '{"command":"ls"}',
      '{"command":"ls"}',
      '{"command":"ls"}',
    ]);

    const final = toolPartOf(snapshots[snapshots.length - 1]!);
    expect(final).toMatchObject({
      state: "output-available",
      resultRef: "result.call.1",
      inputRef: "input.call.1",
      tool: { wireName: "bash", nativeName: "Bash" },
    });
  });

  test("tool-output-error lands the error state with the safe text", () => {
    const chunks: ReadonlyArray<UiMessageChunk> = [
      { type: "tool-input-available", toolCallId: "call.1", tool: BASH },
      {
        type: "tool-output-error",
        toolCallId: "call.1",
        tool: BASH,
        errorText: "command failed",
        errorRef: "error.call.1",
      },
    ];
    const final = snapshotsOf(chunks)[2]!;
    expect(toolPartOf(final)).toMatchObject({
      state: "output-error",
      errorText: "command failed",
      errorRef: "error.call.1",
    });
  });

  test("a malformed sequence fails with the tagged error, never silently corrupts", () => {
    // Output for a tool call that never streamed input.
    expect(() =>
      applyUiChunk(initialUiMessage(), {
        type: "tool-output-available",
        toolCallId: "call.ghost",
        tool: BASH,
        resultRef: "result.ghost",
      }),
    ).toThrowError(UiMessageReducerError);

    // Input delta after the tool already produced output (state regression).
    const settled = snapshotsOf([
      { type: "tool-input-available", toolCallId: "call.1", tool: BASH },
      { type: "tool-output-available", toolCallId: "call.1", tool: BASH, resultRef: "r.1" },
    ])[2]!;
    expect(() =>
      applyUiChunk(settled, {
        type: "tool-input-streaming",
        toolCallId: "call.1",
        tool: BASH,
        inputTextDelta: "more",
      }),
    ).toThrowError(UiMessageReducerError);

    // Text delta after text end.
    const closed = snapshotsOf([
      { type: "text-delta", id: "m", delta: "a" },
      { type: "text-end", id: "m" },
    ])[2]!;
    try {
      applyUiChunk(closed, { type: "text-delta", id: "m", delta: "b" });
      throw new Error("expected a UiMessageReducerError");
    } catch (error) {
      expect(error).toBeInstanceOf(UiMessageReducerError);
      expect((error as UiMessageReducerError)._tag).toBe("AgentHarness.UiMessageReducerError");
    }
  });

  test("transient chunks bypass the persisted message", () => {
    const message = applyUiChunk(initialUiMessage(), {
      type: "text-delta",
      id: "m",
      delta: "ephemeral",
      transient: true,
    });
    expect(message).toEqual(initialUiMessage());
  });

  test("abort, error, and step boundaries land on the message", () => {
    const aborted = applyUiChunk(initialUiMessage(), { type: "message-abort" });
    expect(aborted.status).toBe("aborted");

    const errored = applyUiChunk(initialUiMessage(), { type: "error", errorText: "masked" });
    expect(errored.errorText).toBe("masked");

    const stepped = snapshotsOf([
      { type: "step-start", stepId: "step.1" },
      { type: "text-delta", id: "m", delta: "a" },
      { type: "step-finish", stepId: "step.1", finishReason: "tool-calls" },
    ])[3]!;
    expect(stepped.parts).toEqual([
      { type: "step-start" },
      { type: "text", id: "m", text: "a", state: "done" },
    ]);
  });
});

describe("reduceUiMessageStream — SubscriptionRef progressive reduction", () => {
  test("publishes every progressive snapshot and done equals the pure fold", async () => {
    const chunks: ReadonlyArray<UiMessageChunk> = [
      { type: "message-start", messageId: "t1" },
      { type: "text-delta", id: "msg.t1", delta: "Hello " },
      { type: "text-delta", id: "msg.t1", delta: "world" },
      { type: "text-end", id: "msg.t1" },
      { type: "message-finish", finishReason: "stop" },
    ];

    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const queue = yield* Queue.unbounded<UiMessageChunk, Cause.Done>();
        const handle = yield* reduceUiMessageStream(Stream.fromQueue(queue));

        // Subscribe before feeding: `changes` first emits the current value,
        // so the Deferred proves the subscriber is live and no update is lost.
        const ready = yield* Deferred.make<void>();
        const collector = yield* Effect.forkChild(
          SubscriptionRef.changes(handle.ref).pipe(
            Stream.tap(() => Deferred.succeed(ready, undefined)),
            Stream.takeUntil((message) => message.status !== "streaming"),
            Stream.runCollect,
          ),
        );
        yield* Deferred.await(ready);

        for (const chunk of chunks) {
          yield* Queue.offer(queue, chunk);
        }
        yield* Queue.end(queue);

        const final = yield* handle.done;
        const snapshots = yield* Fiber.join(collector);
        return { final, snapshots };
      }),
    );

    // The stream saw the same progressive states the pure fold produces.
    expect(outcome.snapshots.map(textOf)).toEqual([
      "",
      "",
      "Hello ",
      "Hello world",
      "Hello world",
      "Hello world",
    ]);
    expect(outcome.final).toEqual(snapshotsOf(chunks)[chunks.length]);
    expect(outcome.snapshots[outcome.snapshots.length - 1]).toEqual(outcome.final);
  });

  test("a malformed sequence fails done with the tagged error", async () => {
    const error = await Effect.runPromise(
      Effect.gen(function* () {
        const handle = yield* reduceUiMessageStream(
          Stream.fromIterable<UiMessageChunk>([
            {
              type: "tool-output-available",
              toolCallId: "call.ghost",
              tool: BASH,
              resultRef: "result.ghost",
            },
          ]),
        );
        return yield* Effect.flip(handle.done);
      }),
    );
    expect(error).toBeInstanceOf(UiMessageReducerError);
    expect(error._tag).toBe("AgentHarness.UiMessageReducerError");
    expect(error.chunkType).toBe("tool-output-available");
  });

  test("full pipeline: a scripted KhalaRuntimeEvent turn reduces to the coalesced message", async () => {
    const SOURCE: KhalaRuntimeSource = { lane: "test_fixture" };
    const events = [
      buildTurnStarted({ turnId: "t1", threadId: "s1", sequence: 0, source: SOURCE }),
      buildTextDelta({
        turnId: "t1",
        threadId: "s1",
        sequence: 1,
        source: SOURCE,
        messageId: "msg.t1",
        text: "Hello ",
      }),
      buildTextDelta({
        turnId: "t1",
        threadId: "s1",
        sequence: 2,
        source: SOURCE,
        messageId: "msg.t1",
        text: "world",
      }),
      buildTurnFinished({
        turnId: "t1",
        threadId: "s1",
        sequence: 3,
        source: SOURCE,
        finishReason: "stop",
      }),
    ];
    const chunks = events.flatMap((event) => khalaEventToUiChunks(event));

    const final = await Effect.runPromise(
      Effect.gen(function* () {
        const handle = yield* reduceUiMessageStream(Stream.fromIterable(chunks));
        return yield* handle.done;
      }),
    );

    expect(final.id).toBe("t1");
    expect(final.status).toBe("complete");
    expect(final.finishReason).toBe("stop");
    expect(textOf(final)).toBe("Hello world");
    // The reducer final state equals the coalesced pure fold of the same turn.
    expect(final).toEqual(snapshotsOf(chunks)[chunks.length]);
  });
});
