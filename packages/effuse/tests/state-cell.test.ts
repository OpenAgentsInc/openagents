import { describe, expect, it } from "vitest"
import { Effect, Stream } from "effect"
import { makeCell } from "../src/index.ts"

describe("StateCell (contract)", () => {
  it("dedupes change emissions with default equality", async () => {
    const seen: number[] = []

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const cell = yield* makeCell(0)

          yield* cell.changes
            .pipe(
              Stream.tap((n) => Effect.sync(() => void seen.push(n))),
              Stream.runDrain,
              Effect.forkScoped
            )

          yield* cell.set(0) // no-op
          yield* cell.update((n) => n) // no-op
          yield* cell.set(1)
          yield* cell.set(1) // no-op
          yield* cell.update((n) => n + 1)

          // Give the consumer fiber a tick to drain the queue.
          yield* Effect.sleep("1 millis")
        })
      )
    )

    expect(seen).toEqual([1, 2])
  })

  it("computed view emits only when the derived value changes", async () => {
    const seen: number[] = []

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const cell = yield* makeCell({ count: 0, other: 0 })
          const count = cell.computed((s) => s.count)

          yield* count.changes
            .pipe(
              Stream.tap((n) => Effect.sync(() => void seen.push(n))),
              Stream.runDrain,
              Effect.forkScoped
            )

          // Ensure the stream fiber starts before we enqueue updates.
          yield* Effect.yieldNow()

          yield* cell.update((s) => ({ ...s, other: s.other + 1 }))
          yield* cell.update((s) => ({ ...s, count: s.count + 1 }))
          yield* cell.update((s) => ({ ...s, other: s.other + 1 }))
          yield* cell.update((s) => ({ ...s, count: s.count })) // no-op

          yield* Effect.sleep("1 millis")
        })
      )
    )

    expect(seen).toEqual([1])
  })

  it("batch coalesces multiple updates into a single emission", async () => {
    const seen: number[] = []

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const cell = yield* makeCell(0)

          yield* cell.changes
            .pipe(
              Stream.tap((n) => Effect.sync(() => void seen.push(n))),
              Stream.runDrain,
              Effect.forkScoped
            )

          yield* cell.batch(
            Effect.gen(function* () {
              yield* cell.set(1)
              yield* cell.set(2)
              yield* cell.set(2) // no-op
            })
          )

          // A batch that ends where it started should emit nothing.
          yield* cell.batch(
            Effect.gen(function* () {
              yield* cell.set(3)
              yield* cell.set(2)
            })
          )

          yield* Effect.sleep("1 millis")
        })
      )
    )

    expect(seen).toEqual([2])
  })
})
