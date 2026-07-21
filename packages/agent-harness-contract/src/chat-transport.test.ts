import { Effect, Option, Stream } from "effect";
import type { KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema";
import { describe, expect, test } from "vite-plus/test";

import {
  buildTextDelta,
  buildTurnFinished,
  buildTurnStarted,
} from "./event-builder.ts";
import { ChatTransportError } from "./chat-transport.ts";
import { makeEventLogChatTransport } from "./chat-transport-event-log.ts";
import {
  decodeDesktopChunkFrame,
  encodeDesktopChunkFrame,
  makeDesktopIpcChatTransport,
} from "./chat-transport-ipc.ts";
import {
  decodeSseBody,
  encodeSseChunk,
  makeWebSseChatTransport,
  parseSseBuffer,
  reconnectHttpStatus,
  uiChunkStreamToSse,
} from "./chat-transport-sse.ts";
import { makeHarnessEventLog } from "./event-log.ts";
import { makeInMemoryEventLogStore } from "./event-log-store.ts";
import type { HarnessStreamEvent } from "./stream.ts";
import type { UiMessageChunk } from "./ui-message-chunk.ts";

const SOURCE: KhalaRuntimeSource = { lane: "test_fixture" };

const scriptTurn = (
  turnId: string,
  words: ReadonlyArray<string>,
  startSeq = 0,
): ReadonlyArray<HarnessStreamEvent> => {
  const events: Array<HarnessStreamEvent> = [];
  let seq = startSeq;
  events.push(buildTurnStarted({ turnId, threadId: "thread.1", sequence: seq++, source: SOURCE }));
  for (const word of words) {
    events.push(
      buildTextDelta({
        turnId,
        threadId: "thread.1",
        sequence: seq++,
        source: SOURCE,
        messageId: `msg.${turnId}`,
        text: word,
      }),
    );
  }
  events.push(
    buildTurnFinished({
      turnId,
      threadId: "thread.1",
      sequence: seq++,
      source: SOURCE,
      finishReason: "stop",
    }),
  );
  return events;
};

const collect = <A, E>(stream: Stream.Stream<A, E>) =>
  Effect.runPromise(Stream.runCollect(stream));

describe("ChatTransport — event-log core (STREAM-03)", () => {
  test("sendMessages projects a completed turn to UiMessageChunk via finite replay", async () => {
    const chunks = await Effect.runPromise(
      Effect.gen(function* () {
        const store = makeInMemoryEventLogStore();
        const log = yield* makeHarnessEventLog(store);
        // Persist the full turn first — the transport uses finite replay when
        // the terminal event is already stored (attach's live tail never ends).
        yield* Effect.forEach(scriptTurn("turn.send.1", ["hello", " world"]), (e) =>
          log.appendEvent(e),
        );
        const transport = makeEventLogChatTransport({ log });
        return yield* Stream.runCollect(
          transport.sendMessages({
            turnId: "turn.send.1",
            threadId: "thread.1",
            messages: [{ role: "user", text: "hi" }],
          }),
        );
      }),
    );

    const types = chunks.map((c) => c.type);
    expect(types[0]).toBe("message-start");
    expect(types).toContain("text-delta");
    expect(types[types.length - 1]).toBe("message-finish");
    const deltas = chunks
      .filter((c): c is UiMessageChunk & { type: "text-delta" } => c.type === "text-delta")
      .map((c) => c.delta);
    expect(deltas.join("")).toBe("hello world");
    for (const chunk of chunks) {
      expect(typeof chunk.cursor).toBe("number");
    }
  });

  test("sendMessages with a producer runs it then streams via finite replay", async () => {
    const chunks = await Effect.runPromise(
      Effect.gen(function* () {
        const store = makeInMemoryEventLogStore();
        const log = yield* makeHarnessEventLog(store);
        const transport = makeEventLogChatTransport({
          log,
          produce: ({ turnId, log: liveLog }) =>
            Effect.gen(function* () {
              for (const event of scriptTurn(turnId, ["live"])) {
                yield* liveLog.appendEvent(event).pipe(
                  Effect.mapError(
                    (error) =>
                      new ChatTransportError({
                        operation: "produce.append",
                        turnId,
                        detail: error.detail ?? error.operation,
                        cause: error,
                      }),
                  ),
                );
              }
            }),
        });
        return yield* Stream.runCollect(
          transport.sendMessages({
            turnId: "turn.live.prod",
            threadId: "thread.1",
            messages: [{ role: "user", text: "hi" }],
          }),
        );
      }),
    );

    expect(chunks[0]?.type).toBe("message-start");
    expect(chunks[chunks.length - 1]?.type).toBe("message-finish");
    const text = chunks
      .filter((c): c is UiMessageChunk & { type: "text-delta" } => c.type === "text-delta")
      .map((c) => c.delta)
      .join("");
    expect(text).toBe("live");
  });

  test("reconnectToStream attaches at the renderer cursor with no gap and no duplicate", async () => {
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const store = makeInMemoryEventLogStore();
        const log = yield* makeHarnessEventLog(store);
        const script = scriptTurn("turn.reconnect.1", ["a", "b", "c"]);
        yield* Effect.forEach(script, (e) => log.appendEvent(e));

        const transport = makeEventLogChatTransport({ log });

        const first = yield* Stream.runCollect(
          transport.sendMessages({
            turnId: "turn.reconnect.1",
            threadId: "thread.1",
            messages: [{ role: "user", text: "x" }],
            consumerClass: "renderer-a",
          }),
        );

        const lastCursor = first.find((c) => c.type === "text-delta")?.cursor;
        expect(lastCursor).toBe(1);

        const reconnected = yield* transport.reconnectToStream({
          turnId: "turn.reconnect.1",
          fromCursor: lastCursor!,
          consumerClass: "renderer-b",
        });
        expect(Option.isSome(reconnected)).toBe(true);
        const tail = yield* Stream.runCollect(Option.getOrThrow(reconnected));
        return { first, tail };
      }),
    );

    const tailCursors = outcome.tail.map((c) => c.cursor);
    expect(Math.min(...(tailCursors as number[]))).toBeGreaterThan(1);
    for (const c of tailCursors) {
      expect(c).toBeGreaterThan(1);
    }
    expect(outcome.tail[outcome.tail.length - 1]?.type).toBe("message-finish");
  });

  test("reconnectToStream returns none when there is no turn / nothing left", async () => {
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const store = makeInMemoryEventLogStore();
        const log = yield* makeHarnessEventLog(store);
        const transport = makeEventLogChatTransport({ log });

        const missing = yield* transport.reconnectToStream({
          turnId: "never-existed",
          fromCursor: -1,
        });

        const script = scriptTurn("turn.done", ["z"]);
        yield* Effect.forEach(script, (e) => log.appendEvent(e));
        const last = yield* log.lastCursor({ turnId: "turn.done" });
        const exhausted = yield* transport.reconnectToStream({
          turnId: "turn.done",
          fromCursor: last,
        });
        return { missing, exhausted };
      }),
    );
    expect(Option.isNone(outcome.missing)).toBe(true);
    expect(Option.isNone(outcome.exhausted)).toBe(true);
    expect(reconnectHttpStatus(outcome.missing)).toBe("no_content");
  });
});

describe("ChatTransport — SSE wire (web Layer)", () => {
  test("encodes chunks as data: frames and decodes them back losslessly", async () => {
    const original: ReadonlyArray<UiMessageChunk> = [
      { type: "message-start", messageId: "m1", cursor: 0 },
      { type: "text-delta", id: "m1", delta: "hi", cursor: 1 },
      { type: "message-finish", finishReason: "stop", cursor: 2 },
    ];
    const body = await collect(uiChunkStreamToSse(Stream.fromIterable(original)));
    const joined = body.join("");
    expect(joined).toContain("data: ");
    expect(joined.endsWith("data: [DONE]\n\n")).toBe(true);
    const decoded = decodeSseBody(joined);
    expect(decoded).toEqual(original);
  });

  test("parseSseBuffer handles incremental frames and DONE", () => {
    const c1 = encodeSseChunk({ type: "text-delta", id: "m", delta: "a", cursor: 0 });
    const partial = c1.slice(0, 12);
    const mid = parseSseBuffer(partial);
    expect(mid.chunks).toEqual([]);
    expect(mid.rest).toBe(partial);

    const full = parseSseBuffer(c1 + "data: [DONE]\n\n");
    expect(full.chunks).toHaveLength(1);
    expect(full.done).toBe(true);
  });

  test("web SSE transport wraps an inner event-log transport", async () => {
    const chunks = await Effect.runPromise(
      Effect.gen(function* () {
        const store = makeInMemoryEventLogStore();
        const log = yield* makeHarnessEventLog(store);
        yield* Effect.forEach(scriptTurn("turn.sse", ["ok"]), (e) => log.appendEvent(e));
        const web = makeWebSseChatTransport(makeEventLogChatTransport({ log }));
        const sse = web.streamAsSse(
          web.sendMessages({
            turnId: "turn.sse",
            threadId: "thread.1",
            messages: [{ role: "user", text: "x" }],
          }),
        );
        return yield* Stream.runCollect(sse);
      }),
    );
    const body = chunks.join("");
    const decoded = decodeSseBody(body);
    expect(decoded.some((c) => c.type === "message-start")).toBe(true);
    expect(decoded[decoded.length - 1]?.type).toBe("message-finish");
  });
});

describe("ChatTransport — desktop IPC Layer", () => {
  test("chunk frames round-trip through the IPC codec", () => {
    const chunk: UiMessageChunk = {
      type: "text-delta",
      id: "msg.1",
      delta: "hello",
      cursor: 4,
    };
    const frame = encodeDesktopChunkFrame("turn.ipc", chunk);
    const decoded = decodeDesktopChunkFrame(frame);
    expect(decoded.turnId).toBe("turn.ipc");
    expect(decoded.chunk).toEqual(chunk);
  });

  test("desktop transport frames the event-log stream without replacing ClaudeLocalEvent", async () => {
    const frames = await Effect.runPromise(
      Effect.gen(function* () {
        const store = makeInMemoryEventLogStore();
        const log = yield* makeHarnessEventLog(store);
        yield* Effect.forEach(scriptTurn("turn.ipc.2", ["x"]), (e) => log.appendEvent(e));
        const desktop = makeDesktopIpcChatTransport(makeEventLogChatTransport({ log }));
        expect(desktop.chunkChannel).toBe("openagents:chat-transport:chunk");
        return yield* Stream.runCollect(
          desktop.framesFor(
            "turn.ipc.2",
            desktop.sendMessages({
              turnId: "turn.ipc.2",
              threadId: "thread.1",
              messages: [{ role: "user", text: "y" }],
            }),
          ),
        );
      }),
    );
    expect(frames.length).toBeGreaterThan(0);
    for (const frame of frames) {
      const decoded = decodeDesktopChunkFrame(frame);
      expect(decoded.turnId).toBe("turn.ipc.2");
      expect(decoded.chunk.cursor).toBeDefined();
    }
  });
});
