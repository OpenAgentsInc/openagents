/**
 * Effuse StateCell
 *
 * A reactive state cell that notifies subscribers on change.
 * Built on Effect.Ref with Stream-based change notifications.
 */

import { Effect, Ref, Stream, Queue, Scope } from "effect"

/**
 * A reactive state cell that can be read, written, and subscribed to.
 */
export interface StateCell<A> {
  /** Get current value */
  readonly get: Effect.Effect<A, never>
  /** Set new value */
  readonly set: (value: A) => Effect.Effect<void, never>
  /** Update with function */
  readonly update: (f: (current: A) => A) => Effect.Effect<void, never>
  /** Stream of value changes (emits on every set/update) */
  readonly changes: Stream.Stream<A, never>
}

/**
 * Create a new StateCell with an initial value.
 *
 * The cell is scoped - when the scope closes, the change stream ends.
 *
 * @example
 * ```typescript
 * const program = Effect.gen(function* () {
 *   const counter = yield* makeCell(0)
 *
 *   // Subscribe to changes
 *   yield* pipe(
 *     counter.changes,
 *     Stream.tap(value => Effect.log(`Counter: ${value}`)),
 *     Stream.runDrain,
 *     Effect.forkScoped
 *   )
 *
 *   // Modify state
 *   yield* counter.set(5)
 *   yield* counter.update(n => n + 1)
 *
 *   const value = yield* counter.get
 *   // value === 6
 * })
 * ```
 */
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

/**
 * Create a read-only view of a StateCell.
 */
export const readonly = <A>(cell: StateCell<A>): Readonly<StateCell<A>> => ({
  get: cell.get,
  set: () => Effect.die("Cannot set a readonly cell"),
  update: () => Effect.die("Cannot update a readonly cell"),
  changes: cell.changes,
})
