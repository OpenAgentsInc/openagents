import { describe, expect, test } from "vite-plus/test"
import { Cause, Deferred, Effect, Exit, Fiber, PubSub, Scope } from "effect"

import {
  awaitPipelineSignal,
  makeDrainableWorker,
  makePipelineSignalBus,
  type PipelineSignalBus,
} from "./index.js"

type TestSignal =
  | Readonly<{ kind: "test.item_settled"; item: number }>
  | Readonly<{ kind: "test.item_failed"; item: number; reason: string }>

const isFailed = (signal: TestSignal): signal is TestSignal & { kind: "test.item_failed" } =>
  signal.kind === "test.item_failed"

// Deterministic scheduler flush: lets forked fibers run without wall-clock
// sleeps or poll loops.
const flushScheduler = Effect.gen(function* () {
  for (let i = 0; i < 8; i += 1) yield* Effect.yieldNow
})

describe("makeDrainableWorker", () => {
  test("drain settles only after queued and in-flight work completes", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const processed: number[] = []
          const gate = yield* Deferred.make<void>()
          const worker = yield* makeDrainableWorker((item: number) =>
            Effect.gen(function* () {
              yield* Deferred.await(gate)
              processed.push(item)
            }),
          )

          expect(yield* worker.enqueue(1)).toBe(true)
          expect(yield* worker.enqueue(2)).toBe(true)

          const drainFiber = yield* Effect.forkScoped(worker.drain)
          yield* flushScheduler
          // The first item is in flight (blocked on the gate); drain must not
          // have settled.
          expect(drainFiber.pollUnsafe()).toBeUndefined()
          expect(processed).toEqual([])

          yield* Deferred.succeed(gate, undefined)
          yield* Fiber.join(drainFiber)
          expect(processed).toEqual([1, 2])
        }),
      ),
    )
  })

  test("items enqueued during an active drain are settled before drain resolves", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const processed: number[] = []
          const firstItemStarted = yield* Deferred.make<void>()
          const gate = yield* Deferred.make<void>()
          const worker = yield* makeDrainableWorker((item: number) =>
            Effect.gen(function* () {
              yield* Deferred.succeed(firstItemStarted, undefined)
              yield* Deferred.await(gate)
              processed.push(item)
            }),
          )

          yield* worker.enqueue(1)
          const drainFiber = yield* Effect.forkScoped(worker.drain)
          yield* Deferred.await(firstItemStarted)

          // Concurrent enqueue while drain is already being awaited.
          yield* worker.enqueue(2)
          yield* worker.enqueue(3)
          yield* flushScheduler
          expect(drainFiber.pollUnsafe()).toBeUndefined()

          yield* Deferred.succeed(gate, undefined)
          yield* Fiber.join(drainFiber)
          expect(processed).toEqual([1, 2, 3])
        }),
      ),
    )
  })

  test("failure-path items publish failure signals and never hang drain", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const bus: PipelineSignalBus<TestSignal> = yield* makePipelineSignalBus<TestSignal>()
          const subscription = yield* bus.subscribe
          const worker = yield* makeDrainableWorker(
            (item: number) =>
              item % 2 === 0
                ? Effect.fail(`item ${item} rejected`)
                : bus.publish({ kind: "test.item_settled", item }).pipe(Effect.asVoid),
            {
              onFailure: ({ cause, item }) =>
                bus
                  .publish({
                    kind: "test.item_failed",
                    item,
                    reason: String(Cause.squash(cause)),
                  })
                  .pipe(Effect.asVoid),
            },
          )

          yield* worker.enqueue(1)
          yield* worker.enqueue(2)
          yield* worker.enqueue(3)
          yield* worker.drain

          const failed = yield* awaitPipelineSignal(subscription, isFailed)
          expect(failed).toEqual({
            kind: "test.item_failed",
            item: 2,
            reason: "item 2 rejected",
          })

          // The loop survived the failure: later items still processed and a
          // fresh enqueue after drain also settles.
          yield* worker.enqueue(5)
          yield* worker.drain
        }),
      ),
    )
  })

  test("defects in process are contained by onFailure and drain still settles", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const defects: string[] = []
          const worker = yield* makeDrainableWorker(
            (item: number) => (item === 1 ? Effect.die("boom") : Effect.void),
            {
              onFailure: ({ item }) =>
                Effect.sync(() => {
                  defects.push(`item ${item} failed`)
                }),
            },
          )
          yield* worker.enqueue(1)
          yield* worker.enqueue(2)
          yield* worker.drain
          expect(defects).toEqual(["item 1 failed"])
        }),
      ),
    )
  })

  test("concurrent drains all settle at the same quiescent point", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const gate = yield* Deferred.make<void>()
          const processed: number[] = []
          const worker = yield* makeDrainableWorker((item: number) =>
            Effect.gen(function* () {
              yield* Deferred.await(gate)
              processed.push(item)
            }),
          )
          yield* worker.enqueue(1)
          yield* worker.enqueue(2)

          const drains = yield* Effect.forkScoped(
            Effect.all([worker.drain, worker.drain, worker.drain], { concurrency: "unbounded" }),
          )
          yield* flushScheduler
          expect(drains.pollUnsafe()).toBeUndefined()

          yield* Deferred.succeed(gate, undefined)
          yield* Fiber.join(drains)
          expect(processed).toEqual([1, 2])

          // drain on an already-idle worker settles immediately.
          yield* worker.drain
        }),
      ),
    )
  })

  test("scope close shuts the queue down: late enqueues are rejected and drain stays settled", async () => {
    const scope = Effect.runSync(Scope.make())
    const worker = await Effect.runPromise(
      makeDrainableWorker((_: number) => Effect.void).pipe(
        Effect.provideService(Scope.Scope, scope),
      ),
    )
    await Effect.runPromise(worker.enqueue(1).pipe(Effect.andThen(worker.drain)))

    await Effect.runPromise(Scope.close(scope, Exit.void))

    // Queue is shut down: the item is rejected, drain state is untouched, and
    // drain settles instead of hanging on an item nobody will process.
    expect(await Effect.runPromise(worker.enqueue(2))).toBe(false)
    await Effect.runPromise(worker.drain)
  })
})

describe("makePipelineSignalBus", () => {
  test("subscribers observe typed signals published after subscription", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const bus = yield* makePipelineSignalBus<TestSignal>()
          const subscription = yield* bus.subscribe
          yield* bus.publish({ kind: "test.item_settled", item: 7 })
          expect(yield* PubSub.take(subscription)).toEqual({
            kind: "test.item_settled",
            item: 7,
          })
        }),
      ),
    )
  })

  test("publishUnsafe delivers from synchronous boundaries", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const bus = yield* makePipelineSignalBus<TestSignal>()
          const subscription = yield* bus.subscribe
          const accepted = bus.publishUnsafe({ kind: "test.item_failed", item: 9, reason: "sync" })
          expect(accepted).toBe(true)
          const failed = yield* awaitPipelineSignal(subscription, isFailed)
          expect(failed.item).toBe(9)
        }),
      ),
    )
  })

  test("awaitPipelineSignal skips non-matching signals deterministically", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const bus = yield* makePipelineSignalBus<TestSignal>()
          const subscription = yield* bus.subscribe
          yield* bus.publish({ kind: "test.item_settled", item: 1 })
          yield* bus.publish({ kind: "test.item_settled", item: 2 })
          yield* bus.publish({ kind: "test.item_failed", item: 3, reason: "third" })
          const failed = yield* awaitPipelineSignal(subscription, isFailed)
          expect(failed).toEqual({ kind: "test.item_failed", item: 3, reason: "third" })
        }),
      ),
    )
  })
})
