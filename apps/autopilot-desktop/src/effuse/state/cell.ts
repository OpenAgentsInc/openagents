/**
 * StateCell - Reactive state primitive
 */

import { Effect, Queue, Ref, Scope, Stream } from "effect"

export interface StateCell<A> {
  /** Read current value */
  readonly get: Effect.Effect<A, never>

  /** Replace value (triggers re-render) */
  readonly set: (value: A) => Effect.Effect<void, never>

  /** Transform value (triggers re-render) */
  readonly update: (f: (current: A) => A) => Effect.Effect<void, never>

  /** Stream of state changes */
  readonly changes: Stream.Stream<A, never>
}

export const makeCell = <A>(
  initial: A
): Effect.Effect<StateCell<A>, never, Scope.Scope> =>
  Effect.gen(function* () {
    const ref = yield* Ref.make(initial)
    const queue = yield* Queue.unbounded<A>()

    // Shutdown queue when scope closes
    yield* Effect.addFinalizer(() => Queue.shutdown(queue))

    const cell: StateCell<A> = {
      get: Ref.get(ref),

      set: (value: A) =>
        Effect.gen(function* () {
          yield* Ref.set(ref, value)
          yield* Queue.offer(queue, value)
        }),

      update: (f: (current: A) => A) =>
        Effect.gen(function* () {
          const newValue = yield* Ref.updateAndGet(ref, f)
          yield* Queue.offer(queue, newValue)
        }),

      changes: Stream.fromQueue(queue),
    }

    return cell
  })
