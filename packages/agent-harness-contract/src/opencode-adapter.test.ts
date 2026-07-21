import { Effect, Stream } from "effect";
import type { KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema";
import { describe, expect, test } from "vite-plus/test";

import {
  makeOpencodeAdapter,
  type OpencodeEvent,
  opencodeEventToKhalaEvents,
  type OpencodeProjectionContext,
} from "./opencode-adapter.ts";
import type { HarnessStreamEvent } from "./stream.ts";

// opencode is an external-agent (ACP-adjacent) runtime; its neutral events are
// labelled with the ACP lane while the adapter itself reports adapterKind
// "opencode".
const SOURCE: KhalaRuntimeSource = { lane: "agent_client_protocol", adapterKind: "opencode" };

const collect = (
  stream: Stream.Stream<HarnessStreamEvent, unknown>,
): Effect.Effect<ReadonlyArray<HarnessStreamEvent>, unknown> => Stream.runCollect(stream);

const sequences = (events: ReadonlyArray<HarnessStreamEvent>) => events.map((e) => e.sequence);
const kinds = (events: ReadonlyArray<HarnessStreamEvent>) => events.map((e) => e.kind);

const makeCtx = (): OpencodeProjectionContext => {
  let seq = 0;
  return {
    source: SOURCE,
    threadId: "s1",
    turnId: "t1",
    nextSequence: () => seq++,
    toolNames: new Map<string, string>(),
  };
};

// A representative opencode stream: text -> reasoning -> tool call+result ->
// step-ended (the neutral turn-finish carrier).
const REPRESENTATIVE_SCRIPT: ReadonlyArray<OpencodeEvent> = [
  {
    type: "session.next.text.delta",
    assistantMessageID: "msg_1",
    textID: "text_1",
    delta: "On it. ",
  },
  {
    type: "session.next.reasoning.delta",
    assistantMessageID: "msg_1",
    reasoningID: "r_1",
    delta: "plan",
  },
  {
    type: "session.next.tool.called",
    assistantMessageID: "msg_1",
    callID: "call_1",
    tool: "bash",
    providerExecuted: true,
  },
  { type: "session.next.tool.success", callID: "call_1", providerExecuted: true },
  {
    type: "session.next.step.ended",
    assistantMessageID: "msg_1",
    finish: "stop",
    tokens: { input: 20, output: 8, reasoning: 3, cache: { read: 1, write: 0 } },
  },
];

describe("opencode projection — neutral event mapping", () => {
  test("a representative stream projects onto a contiguous KhalaRuntimeEvent stream", async () => {
    const events = await Effect.runPromise(
      Effect.gen(function* () {
        const adapter = makeOpencodeAdapter({ script: REPRESENTATIVE_SCRIPT });
        const session = yield* adapter.start({ sessionId: "s1", source: SOURCE });
        const control = yield* session.promptTurn({ turnId: "t1", prompt: "go" });
        return yield* collect(control.events);
      }),
    );

    // turn.started + text.delta + reasoning.delta + tool.call + tool.result + turn.finished.
    expect(kinds(events)).toEqual([
      "turn.started",
      "text.delta",
      "reasoning.delta",
      "tool.call",
      "tool.result",
      "turn.finished",
    ]);
    // Sequences are contiguous 0..5 with no gap and no duplicate.
    expect(sequences(events)).toEqual([0, 1, 2, 3, 4, 5]);
    expect(new Set(sequences(events)).size).toBe(events.length);

    // step.ended carried token usage onto the neutral turn.finished.
    const finished = events.at(-1);
    expect(finished).toMatchObject({
      kind: "turn.finished",
      finishReason: "stop",
      usage: { inputTokens: 20, outputTokens: 8, reasoningTokens: 3, totalTokens: 31 },
    });
  });

  test("reasoning deltas project onto reasoning.delta events", () => {
    const ctx = makeCtx();
    const projected = opencodeEventToKhalaEvents(
      {
        type: "session.next.reasoning.delta",
        assistantMessageID: "msg_1",
        reasoningID: "r_1",
        delta: "thinking",
      },
      ctx,
    );
    expect(projected).toHaveLength(1);
    expect(projected[0]).toMatchObject({ kind: "reasoning.delta", text: "thinking" });
  });

  test("session.idle projects to no neutral event (turn boundary is step.ended)", () => {
    const ctx = makeCtx();
    expect(opencodeEventToKhalaEvents({ type: "session.idle" }, ctx)).toEqual([]);
  });
});

describe("opencode projection — tool-name normalization", () => {
  test("opencode 'bash' projects with the shared common name", () => {
    const ctx = makeCtx();
    const called = opencodeEventToKhalaEvents(
      {
        type: "session.next.tool.called",
        assistantMessageID: "m",
        callID: "c1",
        tool: "bash",
        providerExecuted: true,
      },
      ctx,
    );
    expect(called[0]).toMatchObject({ kind: "tool.call", toolName: "bash" });
    // The success event correlates the tool id from the earlier call and keeps the common name.
    const success = opencodeEventToKhalaEvents(
      { type: "session.next.tool.success", callID: "c1", providerExecuted: true },
      ctx,
    );
    expect(success[0]).toMatchObject({
      kind: "tool.result",
      toolName: "bash",
      providerExecuted: true,
    });
  });

  test("opencode 'read' normalizes to the common vocabulary", () => {
    const ctx = makeCtx();
    const called = opencodeEventToKhalaEvents(
      {
        type: "session.next.tool.called",
        assistantMessageID: "m",
        callID: "c2",
        tool: "read",
        providerExecuted: false,
      },
      ctx,
    );
    expect(called[0]).toMatchObject({ kind: "tool.call", toolName: "read" });
  });

  test("an opencode tool with no common equivalent keeps its native id", () => {
    const ctx = makeCtx();
    const called = opencodeEventToKhalaEvents(
      {
        type: "session.next.tool.called",
        assistantMessageID: "m",
        callID: "c3",
        tool: "webfetch",
        providerExecuted: true,
      },
      ctx,
    );
    expect(called[0]).toMatchObject({ kind: "tool.call", toolName: "webfetch" });
  });
});

describe("opencode adapter — turn semantics and cursor exactness", () => {
  test("a full turn streams turn.started -> ... -> turn.finished with contiguous sequences", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const adapter = makeOpencodeAdapter();
        const session = yield* adapter.start({ sessionId: "s1", source: SOURCE });
        const control = yield* session.promptTurn({ turnId: "t1", prompt: "hi" });
        const events = yield* collect(control.events);
        const done = yield* control.done;
        return { events, done };
      }),
    );

    expect(result.events[0]?.kind).toBe("turn.started");
    expect(result.events.at(-1)?.kind).toBe("turn.finished");
    // turn.started + 2 text.delta + turn.finished = 4 events, sequences 0..3.
    expect(sequences(result.events)).toEqual([0, 1, 2, 3]);
    expect(result.done.finishReason).toBe("stop");
    expect(result.done.lastCursor).toBe(3);
  });

  test("suspend then continue replays from cursor+1 with no gap and no duplicate", async () => {
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const adapter = makeOpencodeAdapter();

        // Phase 1: pull only the first two events, then suspend.
        const session = yield* adapter.start({ sessionId: "s1", source: SOURCE });
        const control = yield* session.promptTurn({ turnId: "t1", prompt: "hi" });
        const phase1 = yield* collect(control.events.pipe(Stream.take(2)));
        const continuation = yield* session.suspendTurn();

        // Phase 2: a FRESH session (different process) resumes from the cursor.
        const session2 = yield* adapter.start({
          sessionId: "s1",
          source: SOURCE,
          continueFrom: continuation,
        });
        const control2 = yield* session2.continueTurn({});
        const phase2 = yield* collect(control2.events);

        return { phase1, continuation, phase2 };
      }),
    );

    expect(sequences(outcome.phase1)).toEqual([0, 1]);
    // The cursor is exactly the last event delivered in phase 1.
    expect(outcome.continuation.cursor).toBe(1);
    expect(outcome.continuation.lossy).toBe(false);
    // Phase 2 attaches at cursor + 1 — no gap, no duplicate.
    expect(outcome.phase2[0]?.sequence).toBe(outcome.continuation.cursor + 1);
    expect(sequences(outcome.phase2)).toEqual([2, 3]);

    // Concatenation is the full, contiguous turn exactly once.
    const merged = sequences([...outcome.phase1, ...outcome.phase2]);
    expect(merged).toEqual([0, 1, 2, 3]);
    expect(new Set(merged).size).toBe(merged.length);
  });
});

describe("opencode adapter — identity and capability posture", () => {
  test("the adapter reports the opencode harness/adapter kind", () => {
    const adapter = makeOpencodeAdapter();
    expect(adapter.harnessId).toBe("opencode");
    expect(adapter.harnessKind).toBe("opencode");
    expect(adapter.adapterKind).toBe("opencode");
  });

  test("a refused capability fails closed with a typed capability error", async () => {
    const error = await Effect.runPromise(
      Effect.gen(function* () {
        const adapter = makeOpencodeAdapter({ supportsSuspend: false });
        const session = yield* adapter.start({ sessionId: "s1", source: SOURCE });
        return yield* session.suspendTurn().pipe(Effect.flip);
      }),
    );
    expect(error.capability).toBe("suspend_turn");
  });
});
