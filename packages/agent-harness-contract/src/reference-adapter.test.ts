import { Effect, Stream } from "effect";
import type { KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema";
import { describe, expect, test } from "vite-plus/test";

import { HarnessCapabilityUnsupported } from "./capability.ts";
import { toolIdentity } from "./common-tool.ts";
import { makeReferenceAdapter } from "./reference-adapter.ts";
import type { HarnessStreamEvent } from "./stream.ts";

const SOURCE: KhalaRuntimeSource = { lane: "test_fixture" };

const collect = (
  stream: Stream.Stream<HarnessStreamEvent, unknown>,
): Effect.Effect<ReadonlyArray<HarnessStreamEvent>, unknown> => Stream.runCollect(stream);

const sequences = (events: ReadonlyArray<HarnessStreamEvent>) => events.map((e) => e.sequence);

describe("reference adapter — turn semantics", () => {
  test("a full turn streams turn.started -> text.delta* -> turn.finished with contiguous sequences", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const adapter = makeReferenceAdapter({ scriptWords: ["a", "b", "c"] });
        const session = yield* adapter.start({ sessionId: "s1", source: SOURCE });
        const control = yield* session.promptTurn({ turnId: "t1", prompt: "hi" });
        const events = yield* collect(control.events);
        const done = yield* control.done;
        return { events, done };
      }),
    );

    expect(result.events[0]?.kind).toBe("turn.started");
    expect(result.events.at(-1)?.kind).toBe("turn.finished");
    // turn.started + 3 text.delta + turn.finished = 5 events, sequences 0..4.
    expect(sequences(result.events)).toEqual([0, 1, 2, 3, 4]);
    expect(result.done.finishReason).toBe("stop");
    expect(result.done.lastCursor).toBe(4);
  });

  test("suspend then continue replays from cursor+1 with no gap and no duplicate", async () => {
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const adapter = makeReferenceAdapter({ scriptWords: ["a", "b", "c"] });

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
    expect(sequences(outcome.phase2)).toEqual([2, 3, 4]);

    // Concatenation is the full, contiguous script exactly once.
    const merged = sequences([...outcome.phase1, ...outcome.phase2]);
    expect(merged).toEqual([0, 1, 2, 3, 4]);
    expect(new Set(merged).size).toBe(merged.length);
  });

  test("a lossy adapter reports the continuation as re-driven", async () => {
    const continuation = await Effect.runPromise(
      Effect.gen(function* () {
        const adapter = makeReferenceAdapter({ continueIsLossy: true });
        const session = yield* adapter.start({ sessionId: "s1", source: SOURCE });
        const control = yield* session.promptTurn({ turnId: "t1", prompt: "hi" });
        yield* collect(control.events.pipe(Stream.take(1)));
        return yield* session.suspendTurn();
      }),
    );
    expect(continuation.lossy).toBe(true);
  });
});

describe("reference adapter — capability refusal is fail-closed", () => {
  test("an adapter that cannot compact fails with a typed capability error", async () => {
    const exit = await Effect.runPromiseExit(
      Effect.gen(function* () {
        const adapter = makeReferenceAdapter({ supportsCompact: false });
        const session = yield* adapter.start({ sessionId: "s1", source: SOURCE });
        return yield* session.compact();
      }),
    );
    expect(exit._tag).toBe("Failure");
  });

  test("suspend/continue capability errors name the missing capability", async () => {
    const error = await Effect.runPromise(
      Effect.gen(function* () {
        const adapter = makeReferenceAdapter({ supportsSuspend: false });
        const session = yield* adapter.start({ sessionId: "s1", source: SOURCE });
        return yield* session.suspendTurn().pipe(Effect.flip);
      }),
    );
    expect(error).toBeInstanceOf(HarnessCapabilityUnsupported);
    expect(error.capability).toBe("suspend_turn");
  });
});

describe("reference adapter — lifecycle export", () => {
  test("detach and stop return re-importable resume state naming the session", async () => {
    const states = await Effect.runPromise(
      Effect.gen(function* () {
        const adapter = makeReferenceAdapter();
        const session = yield* adapter.start({ sessionId: "s1", source: SOURCE });
        const detached = yield* session.detach();
        const stopped = yield* session.stop();
        return { detached, stopped };
      }),
    );
    expect(states.detached.harnessId).toBe("reference");
    expect(states.detached.sessionId).toBe("s1");
    expect(states.stopped.sessionId).toBe("s1");
  });
});

describe("common-tool normalization", () => {
  test("a known native tool maps onto the common vocabulary", () => {
    const bash = toolIdentity("Bash", { providerExecuted: true });
    expect(bash.wireName).toBe("bash");
    expect(bash.nativeName).toBe("Bash");
    expect(bash.commonName).toBe("bash");
    expect(bash.providerExecuted).toBe(true);
  });

  test("an unknown native tool keeps its native name and has no common name", () => {
    const webFetch = toolIdentity("WebFetch");
    expect(webFetch.wireName).toBe("WebFetch");
    expect(webFetch.commonName).toBeUndefined();
  });
});
