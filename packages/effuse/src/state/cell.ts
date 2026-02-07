/**
 * StateCell - Reactive state primitive
 */

import { Effect, Option, Queue, Ref, Scope, Stream } from "effect"

export type Eq<A> = (x: A, y: A) => boolean

const defaultEq: Eq<unknown> = Object.is

const makeView = <A>(
  get: Effect.Effect<A, never>,
  changes: Stream.Stream<A, never>
): StateCellView<A> => ({
  get,
  changes,
  computed: <B>(f: (a: A) => B, eqB?: Eq<B>) =>
    (() => {
      const eqDerived = (eqB ?? (defaultEq as Eq<B>)) as Eq<B>
      const derivedGet = get.pipe(Effect.map(f))

      // Do not emit the derived value unless it changes, starting from the current value
      // at subscription time (baseline).
      const derivedChanges = Stream.fromEffect(derivedGet).pipe(
        Stream.flatMap((baseline) =>
          changes.pipe(
            Stream.map(f),
            Stream.mapAccum(baseline, (prev, next) => {
              if (eqDerived(prev, next)) {
                return [prev, Option.none<B>()] as const
              }
              return [next, Option.some(next)] as const
            }),
            Stream.filterMap((o) => o)
          )
        )
      )

      return makeView(derivedGet, derivedChanges)
    })(),
  filtered: (pred) => changes.pipe(Stream.filter(pred)),
})

export interface StateCellView<A> {
  /** Read current value */
  readonly get: Effect.Effect<A, never>

  /** Stream of state changes */
  readonly changes: Stream.Stream<A, never>

  /**
   * Derived read-only view of this cell that emits only when the derived value changes.
   */
  readonly computed: <B>(f: (a: A) => B, eq?: Eq<B>) => StateCellView<B>

  /**
   * Stream view that emits only values matching the predicate.
   */
  readonly filtered: (pred: (a: A) => boolean) => Stream.Stream<A, never>
}

export interface StateCell<A> extends StateCellView<A> {
  /** Replace value (triggers re-render) */
  readonly set: (value: A) => Effect.Effect<void, never>

  /** Transform value (triggers re-render) */
  readonly update: (f: (current: A) => A) => Effect.Effect<void, never>

  /**
   * Set the cell's equality function used to dedupe change emissions.
   *
   * This is intended for component-local cells, and mutates the cell instance.
   */
  readonly withEq: (eq: Eq<A>) => StateCell<A>

  /**
   * Coalesce change emissions within the provided Effect into a single notification.
   *
   * If the final value equals the value at batch start (under `eq`), no notification is emitted.
   * Flush happens on success, failure, or interruption.
   */
  readonly batch: <B, E, R>(effect: Effect.Effect<B, E, R>) => Effect.Effect<B, E, R>
}

export const makeCell = <A>(
  initial: A
): Effect.Effect<StateCell<A>, never, Scope.Scope> =>
  Effect.gen(function* () {
    const ref = yield* Ref.make(initial)
    const queue = yield* Queue.unbounded<A>()

    // Shutdown queue when scope closes
    yield* Effect.addFinalizer(() => Queue.shutdown(queue))

    // Mutable equality function (component-local use)
    let eq: Eq<A> = defaultEq as Eq<A>

    const batchDepth = yield* Ref.make(0)
    const batchStart = yield* Ref.make<A | null>(null)
    const batchDirty = yield* Ref.make(false)

    const view = makeView(Ref.get(ref), Stream.fromQueue(queue))

    const emitIfNotBatching = (value: A) =>
      Effect.gen(function* () {
        const depth = yield* Ref.get(batchDepth)
        if (depth > 0) {
          yield* Ref.set(batchDirty, true)
          return
        }
        yield* Queue.offer(queue, value)
      })

    const set: StateCell<A>["set"] = (value) =>
      Effect.gen(function* () {
        const changed = yield* Ref.modify(ref, (current) => {
          if (eq(current, value)) {
            return [Option.none<A>(), current] as const
          }
          return [Option.some(value), value] as const
        })

        if (Option.isNone(changed)) return
        yield* emitIfNotBatching(changed.value)
      })

    const update: StateCell<A>["update"] = (f) =>
      Effect.gen(function* () {
        const changed = yield* Ref.modify(ref, (current) => {
          const next = f(current)
          if (eq(current, next)) {
            return [Option.none<A>(), current] as const
          }
          return [Option.some(next), next] as const
        })

        if (Option.isNone(changed)) return
        yield* emitIfNotBatching(changed.value)
      })

    const beginBatch = Effect.uninterruptible(
      Effect.gen(function* () {
        const depth0 = yield* Ref.get(batchDepth)
        if (depth0 === 0) {
          const start = yield* Ref.get(ref)
          yield* Ref.set(batchStart, start)
          yield* Ref.set(batchDirty, false)
        }
        yield* Ref.set(batchDepth, depth0 + 1)
      })
    )

    const endBatch = Effect.uninterruptible(
      Effect.gen(function* () {
        const depth0 = yield* Ref.get(batchDepth)
        const depth1 = Math.max(0, depth0 - 1)
        yield* Ref.set(batchDepth, depth1)

        if (depth1 > 0) return

        const dirty = yield* Ref.get(batchDirty)
        const start = yield* Ref.get(batchStart)

        yield* Ref.set(batchDirty, false)
        yield* Ref.set(batchStart, null)

        if (!dirty) return
        if (start === null) return

        const current = yield* Ref.get(ref)
        if (eq(start, current)) return

        yield* Queue.offer(queue, current)
      })
    )

    const batch: StateCell<A>["batch"] = (effect) =>
      Effect.uninterruptibleMask((restore) =>
        Effect.gen(function* () {
          yield* beginBatch
          return yield* restore(effect).pipe(Effect.ensuring(endBatch))
        })
      )

    const cell: StateCell<A> = {
      ...view,
      set,
      update,
      withEq: (nextEq) => {
        eq = nextEq
        return cell
      },
      batch,
    }

    return cell
  })
