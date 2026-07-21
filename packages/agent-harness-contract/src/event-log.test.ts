import { Deferred, Effect, Fiber, Ref, Stream } from "effect";
import type { KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema";
import { describe, expect, test } from "vite-plus/test";

import { buildTextDelta, buildTurnFinished, buildTurnStarted } from "./event-builder.ts";
import { makeHarnessEventLog } from "./event-log.ts";
import { HarnessEventLogError, makeInMemoryEventLogStore } from "./event-log-store.ts";
import type { HarnessStreamEvent } from "./stream.ts";

const SOURCE: KhalaRuntimeSource = { lane: "test_fixture" };

/** A full scripted turn: turn.started, N text.delta, turn.finished. */
const scriptTurn = (
  turnId: string,
  words: ReadonlyArray<string>,
  startSeq = 0,
): ReadonlyArray<HarnessStreamEvent> => {
  const events: Array<HarnessStreamEvent> = [];
  let seq = startSeq;
  events.push(buildTurnStarted({ turnId, threadId: "s1", sequence: seq++, source: SOURCE }));
  for (const word of words) {
    events.push(
      buildTextDelta({
        turnId,
        threadId: "s1",
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
      threadId: "s1",
      sequence: seq++,
      source: SOURCE,
      finishReason: "stop",
    }),
  );
  return events;
};

const seqs = (events: ReadonlyArray<HarnessStreamEvent>) => events.map((e) => e.sequence);

describe("event log — durable replay", () => {
  test("replays the tail from a cursor after process death (fresh runtime, same store)", async () => {
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const store = makeInMemoryEventLogStore();
        const script = scriptTurn("t1", ["a", "b", "c"]); // sequences 0..4

        // Producer runtime appends the whole turn, then "dies".
        const producer = yield* makeHarnessEventLog(store);
        yield* Effect.forEach(script, (e) => producer.appendEvent(e));

        // A different process rehydrates over the SAME store and replays from 1.
        const recovered = yield* makeHarnessEventLog(store);
        const tail = yield* Stream.runCollect(recovered.replay({ turnId: "t1", fromCursor: 1 }));
        const last = yield* recovered.lastCursor({ turnId: "t1" });
        return { tail, last };
      }),
    );

    // Exactly the events after cursor 1, contiguous, no gap, no duplicate.
    expect(seqs(outcome.tail)).toEqual([2, 3, 4]);
    expect(new Set(seqs(outcome.tail)).size).toBe(outcome.tail.length);
    expect(outcome.last).toBe(4);
  });

  test("a full replay from -1 returns the entire turn once", async () => {
    const all = await Effect.runPromise(
      Effect.gen(function* () {
        const store = makeInMemoryEventLogStore();
        const log = yield* makeHarnessEventLog(store);
        const script = scriptTurn("t1", ["x", "y"]); // sequences 0..3
        yield* Effect.forEach(script, (e) => log.appendEvent(e));
        return yield* Stream.runCollect(log.replay({ turnId: "t1", fromCursor: -1 }));
      }),
    );
    expect(seqs(all)).toEqual([0, 1, 2, 3]);
  });

  test("a duplicate or out-of-order sequence is rejected (dup-free guarantee)", async () => {
    const exit = await Effect.runPromiseExit(
      Effect.gen(function* () {
        const store = makeInMemoryEventLogStore();
        const log = yield* makeHarnessEventLog(store);
        const [e0] = scriptTurn("t1", []); // just turn.started at seq 0
        yield* log.appendEvent(e0!);
        // Re-append the same sequence 0 — must be rejected.
        yield* log.appendEvent(e0!);
      }),
    );
    expect(exit._tag).toBe("Failure");
  });
});

describe("event log — rerun boundary visibility", () => {
  test("records and reports a rerun boundary so a recomputed tail is distinguishable", async () => {
    const boundaries = await Effect.runPromise(
      Effect.gen(function* () {
        const store = makeInMemoryEventLogStore();
        const log = yield* makeHarnessEventLog(store);
        yield* Effect.forEach(scriptTurn("t1", ["a"]), (e) => log.appendEvent(e));
        // The turn was re-driven from cursor 1 (lossy continue).
        yield* log.markRerunBoundary({ turnId: "t1", atCursor: 1 });
        return yield* log.rerunBoundaries({ turnId: "t1" });
      }),
    );
    expect(boundaries).toEqual([1]);
  });
});

describe("event log — live attach", () => {
  test("attach replays the persisted tail then follows new events with no gap or duplicate", async () => {
    const collected = await Effect.runPromise(
      Effect.gen(function* () {
        const store = makeInMemoryEventLogStore();
        const log = yield* makeHarnessEventLog(store);

        // Persist the first two events (seq 0, 1).
        yield* log.appendEvent(
          buildTurnStarted({
            turnId: "t1",
            threadId: "s1",
            sequence: 0,
            source: SOURCE,
          }),
        );
        yield* log.appendEvent(
          buildTextDelta({
            turnId: "t1",
            threadId: "s1",
            sequence: 1,
            source: SOURCE,
            messageId: "m",
            text: "a",
          }),
        );

        // Attach from cursor 0 (want seq 1 onward): replay 1, then live 2..3.
        const sink = yield* Ref.make<ReadonlyArray<number>>([]);
        const wantCount = 3; // seq 1, 2, 3
        const done = yield* Deferred.make<void>();
        const fiber = yield* Effect.forkChild(
          log.attach({ turnId: "t1", fromCursor: 0, consumerClass: "renderer" }).pipe(
            Stream.runForEach((event) =>
              Effect.gen(function* () {
                const next = yield* Ref.updateAndGet(sink, (xs) => [...xs, event.sequence]);
                if (next.length >= wantCount) {
                  yield* Deferred.succeed(done, undefined);
                }
              }),
            ),
          ),
        );

        // Append two more live events after the attach is running.
        yield* log.appendEvent(
          buildTextDelta({
            turnId: "t1",
            threadId: "s1",
            sequence: 2,
            source: SOURCE,
            messageId: "m",
            text: "b",
          }),
        );
        yield* log.appendEvent(
          buildTurnFinished({
            turnId: "t1",
            threadId: "s1",
            sequence: 3,
            source: SOURCE,
            finishReason: "stop",
          }),
        );

        yield* Deferred.await(done);
        yield* Fiber.interrupt(fiber);
        return yield* Ref.get(sink);
      }),
    );

    // Replayed 1, then followed 2 and 3 — contiguous, exactly once each.
    expect(collected).toEqual([1, 2, 3]);
    expect(new Set(collected).size).toBe(collected.length);
  });

  test("single-flight: a newer attach for the same (turn, class) supersedes the older", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const store = makeInMemoryEventLogStore();
        const log = yield* makeHarnessEventLog(store);
        yield* log.appendEvent(
          buildTurnStarted({
            turnId: "t1",
            threadId: "s1",
            sequence: 0,
            source: SOURCE,
          }),
        );

        const sinkA = yield* Ref.make<ReadonlyArray<number>>([]);
        const sinkB = yield* Ref.make<ReadonlyArray<number>>([]);
        const bGot = yield* Deferred.make<void>();
        const aReplayed = yield* Deferred.make<void>();

        // Attach A (renderer). It replays seq 0 and then follows. A registers in
        // the single-flight table before it emits, so receiving seq 0 proves A
        // is the active subscriber — deterministic, no sleep needed.
        const fiberA = yield* Effect.forkChild(
          log.attach({ turnId: "t1", fromCursor: -1, consumerClass: "renderer" }).pipe(
            Stream.runForEach((e) =>
              Effect.gen(function* () {
                yield* Ref.update(sinkA, (xs) => [...xs, e.sequence]);
                yield* Deferred.succeed(aReplayed, undefined);
              }),
            ),
          ),
        );
        yield* Deferred.await(aReplayed);

        // Attach B for the SAME (turn, class) — supersedes A.
        const fiberB = yield* Effect.forkChild(
          log.attach({ turnId: "t1", fromCursor: 0, consumerClass: "renderer" }).pipe(
            Stream.runForEach((e) =>
              Effect.gen(function* () {
                yield* Ref.update(sinkB, (xs) => [...xs, e.sequence]);
                yield* Deferred.succeed(bGot, undefined);
              }),
            ),
          ),
        );

        // A should be interrupted by the supersede; await its fiber.
        yield* Fiber.await(fiberA);

        // Publish a new event; only B (the live subscriber) should receive it.
        yield* log.appendEvent(
          buildTextDelta({
            turnId: "t1",
            threadId: "s1",
            sequence: 1,
            source: SOURCE,
            messageId: "m",
            text: "b",
          }),
        );
        yield* Deferred.await(bGot);
        yield* Fiber.interrupt(fiberB);

        return {
          a: yield* Ref.get(sinkA),
          b: yield* Ref.get(sinkB),
        };
      }),
    );

    // A saw the replay (seq 0) but was superseded before the live event.
    expect(result.a).toEqual([0]);
    // B received the live event 1 that A did not.
    expect(result.b).toContain(1);
  });
});

describe("HarnessEventLogError", () => {
  test("is tagged for matching", () => {
    const err = new HarnessEventLogError({ operation: "append", turnId: "t1" });
    expect(err._tag).toBe("AgentHarness.EventLogError");
  });
});
