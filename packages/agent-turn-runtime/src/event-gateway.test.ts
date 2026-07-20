import { Effect, Stream } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { makeTurnEventGateway } from "./event-gateway.js";

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> => Effect.runPromise(Effect.scoped(effect));

describe("turn event gateway", () => {
  test("a burst beyond capacity is bounded and counted, never queued without limit", () =>
    run(
      Effect.gen(function* () {
        const gateway = yield* makeTurnEventGateway<number>(4);
        const results: boolean[] = [];
        for (let index = 0; index < 10; index += 1) {
          results.push(yield* gateway.offer(0, index));
        }
        const depth = yield* gateway.depth;
        const dropped = yield* gateway.droppedCount;
        expect(depth).toBeLessThanOrEqual(4);
        expect(results.filter((accepted) => accepted).length).toBe(4);
        expect(dropped).toBe(6);
      }),
    ));

  test("an offer from a superseded generation is fenced and never reaches the queue", () =>
    run(
      Effect.gen(function* () {
        const gateway = yield* makeTurnEventGateway<number>(8, 0);
        yield* gateway.offer(0, 1);
        yield* gateway.setGeneration(1);
        const fenced = yield* gateway.offer(0, 99);
        expect(fenced).toBe(false);
        expect(yield* gateway.fencedCount).toBe(1);
        expect(yield* gateway.depth).toBe(1);
      }),
    ));

  test("the stream drains accepted in-generation items in order", () =>
    run(
      Effect.gen(function* () {
        const gateway = yield* makeTurnEventGateway<number>(8);
        yield* gateway.offer(0, 10);
        yield* gateway.offer(0, 20);
        yield* gateway.offer(0, 30);
        const drained = yield* gateway.stream.pipe(Stream.take(3), Stream.runCollect);
        expect([...drained]).toEqual([10, 20, 30]);
      }),
    ));
});
