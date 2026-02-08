import { Deferred, Effect, Fiber, Ref, Stream } from "effect";
import * as Chunk from "effect/Chunk";
import { describe, expect, it } from "vitest";

import { api } from "../../convex/_generated/api";
import { makeTestConvexKit } from "../harness/convex";

describe("apps/web TestConvexKit", () => {
  it("supports subscribeQuery updates (in-process, wipeable)", async () => {
    const kit = makeTestConvexKit();

    const threadId = "thread-1";
    const anonKey = "anon-1";

    await Effect.runPromise(
      kit.service.mutation(api.autopilot.threads.ensureAnonThread, { threadId, anonKey } as any),
    );

    const stream = kit.service.subscribeQuery(api.autopilot.messages.getThreadSnapshot, {
      threadId,
      anonKey,
      maxMessages: 200,
      maxParts: 5000,
    } as any);

    const effect = Effect.gen(function* () {
      const firstSeen = yield* Deferred.make<void>();
      const didSignal = yield* Ref.make(false);

      const stream2 = stream.pipe(
        Stream.tap(() =>
          Effect.gen(function* () {
            const already = yield* Ref.get(didSignal);
            if (already) return;
            yield* Ref.set(didSignal, true);
            yield* Deferred.succeed(firstSeen, void 0);
          }),
        ),
        Stream.take(2),
      );

      const fiber = yield* Effect.fork(Stream.runCollect(stream2));

      // Wait for the initial value to be observed before mutating to avoid
      // racing with the first snapshot.
      yield* Deferred.await(firstSeen);

      // Trigger an update while the subscription is active.
      yield* kit.service.mutation(api.autopilot.messages.createRun, { threadId, anonKey, text: "hi" } as any);

      const chunk = yield* Fiber.join(fiber);
      return Chunk.toReadonlyArray(chunk) as any[];
    });

    const values = await Effect.runPromise(effect);
    expect(values.length).toBe(2);

    const first = values[0];
    const second = values[1];

    expect(Array.isArray(first.messages)).toBe(true);
    expect(first.messages.length).toBe(1); // welcome message seeded by ensureAnonThread

    expect(Array.isArray(second.messages)).toBe(true);
    // welcome + user + assistant streaming
    expect(second.messages.length).toBeGreaterThanOrEqual(3);
    expect(second.messages.some((m: any) => m?.role === "user")).toBe(true);
    expect(second.messages.some((m: any) => m?.role === "assistant")).toBe(true);

    // Wipe is deterministic and isolated.
    kit.reset();
    expect(kit.db.__tables.messages.length).toBe(0);
    expect(kit.db.__tables.threads.length).toBe(0);
  });
});
