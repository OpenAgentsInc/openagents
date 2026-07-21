import { Effect, Stream } from "effect";
import type { KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema";
import { describe, expect, test } from "vite-plus/test";

import { makeHarnessEventLog } from "./event-log.ts";
import { makeInMemoryEventLogStore } from "./event-log-store.ts";
import { makeReferenceAdapter } from "./reference-adapter.ts";
import { runHarnessSlice, runTurnInSlices } from "./slice-runner.ts";

const SOURCE: KhalaRuntimeSource = { lane: "test_fixture" };

describe("slice runner — one slice", () => {
  test("a budget larger than the turn completes in a single slice", async () => {
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const adapter = makeReferenceAdapter({ scriptWords: ["a", "b"] }); // 4 events (0..3)
        const session = yield* adapter.start({ sessionId: "s1", source: SOURCE });
        const control = yield* session.promptTurn({ turnId: "t1", prompt: "hi" });
        return yield* runHarnessSlice({
          session,
          control,
          budget: { maxEvents: 100 },
        });
      }),
    );
    expect(outcome.status).toBe("completed");
    expect(outcome.lastEventKind).toBe("turn.finished");
    expect(outcome.cursor).toBe(3);
  });

  test("a budget smaller than the turn suspends at the exact cursor", async () => {
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const adapter = makeReferenceAdapter({ scriptWords: ["a", "b", "c"] }); // 5 events (0..4)
        const session = yield* adapter.start({ sessionId: "s1", source: SOURCE });
        const control = yield* session.promptTurn({ turnId: "t1", prompt: "hi" });
        return yield* runHarnessSlice({
          session,
          control,
          budget: { maxEvents: 2 },
        });
      }),
    );
    expect(outcome.status).toBe("suspended");
    expect(outcome.cursor).toBe(1); // consumed events 0 and 1
    expect(outcome.continuation?.cursor).toBe(1);
    expect(outcome.continuation?.lossy).toBe(false);
  });
});

describe("slice runner — full turn across many slices", () => {
  test("re-enters from continueFrom each slice and delivers every event once, in order", async () => {
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        // 6 words -> 8 events (turn.started + 6 text.delta + turn.finished), seq 0..7.
        const adapter = makeReferenceAdapter({
          scriptWords: ["a", "b", "c", "d", "e", "f"],
        });
        const store = makeInMemoryEventLogStore();
        const log = yield* makeHarnessEventLog(store);

        const run = yield* runTurnInSlices({
          adapter,
          startOptions: { sessionId: "s1", source: SOURCE },
          turnId: "t1",
          prompt: "hi",
          budget: { maxEvents: 3 }, // forces several slices
          eventLog: log,
        });

        // Read everything the log persisted for the turn.
        const persisted = yield* Stream.runCollect(log.replay({ turnId: "t1", fromCursor: -1 }));
        return { run, persisted };
      }),
    );

    // Completed, and took ceil(8/3) = 3 slices.
    expect(outcome.run.result.finishReason).toBe("stop");
    expect(outcome.run.slices).toBe(3);

    // Every event persisted exactly once, contiguous 0..7 — no gap, no duplicate
    // across the slice boundaries.
    const sequences = outcome.persisted.map((e) => e.sequence);
    expect(sequences).toEqual([0, 1, 2, 3, 4, 5, 6, 7]);
    expect(new Set(sequences).size).toBe(sequences.length);
  });

  test("a budget that lands exactly on the final event still completes (boundary)", async () => {
    const run = await Effect.runPromise(
      Effect.gen(function* () {
        // 2 words -> 4 events (seq 0..3). Budget 4 lands exactly on turn.finished.
        const adapter = makeReferenceAdapter({ scriptWords: ["a", "b"] });
        return yield* runTurnInSlices({
          adapter,
          startOptions: { sessionId: "s1", source: SOURCE },
          turnId: "t1",
          prompt: "hi",
          budget: { maxEvents: 4 },
        });
      }),
    );
    expect(run.result.finishReason).toBe("stop");
    expect(run.slices).toBe(1); // did not spuriously suspend on the boundary
  });
});
