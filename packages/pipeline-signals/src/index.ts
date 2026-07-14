/**
 * Deterministic completion signals for async package pipelines.
 *
 * Two primitives (openagents issue #8782, from the T3 Code teardown):
 *
 * - `makeDrainableWorker` wraps the queue-backed worker pattern and exposes
 *   `drain`, which settles when the queue is empty AND all in-flight work has
 *   completed. Tests and orchestration await `drain` instead of sleeping or
 *   polling internal state.
 * - `makePipelineSignalBus` is a typed milestone bus (Effect PubSub).
 *   Pipelines publish schema-tagged milestone signals; tests and orchestration
 *   subscribe first, run the pipeline, then await the exact milestone.
 *
 * Naming contract: these are *pipeline signals* — deterministic
 * test/orchestration synchronization events. They are explicitly NOT the
 * user-facing evidence "receipts" vocabulary used by Blueprint/Cloud
 * (`resource_usage_receipt.v1`, action receipts, payout receipts). Never
 * surface a pipeline signal as user-facing evidence.
 */
import { Effect, PubSub, Scope, TxQueue, TxRef } from "effect"
import type { Cause } from "effect"

/**
 * A queue-backed worker whose completion is deterministically observable.
 */
export interface DrainableWorker<A> {
  /**
   * Enqueue a work item and track it for `drain`. Resolves `false` when the
   * worker's scope has closed (the queue is shut down) and the item was
   * rejected; drain state is only updated for accepted items.
   */
  readonly enqueue: (item: A) => Effect.Effect<boolean>
  /**
   * Settles when the queue is empty and no accepted item is still being
   * processed. Safe to await from multiple fibers concurrently.
   */
  readonly drain: Effect.Effect<void>
}

export type DrainableWorkerOptions<A, E> = Readonly<{
  /**
   * Runs when processing an item fails (typed failure or defect). The worker
   * loop keeps running and `drain` still settles; use this to publish a
   * failure milestone on a `PipelineSignalBus`.
   */
  onFailure?: (input: Readonly<{ cause: Cause.Cause<E>; item: A }>) => Effect.Effect<void>
}>

/**
 * Create a drainable worker that processes items from an unbounded queue.
 *
 * The worker fiber is forked into the current scope and interrupted when the
 * scope closes; a finalizer shuts the queue down so late `enqueue` calls
 * resolve `false` instead of hanging `drain`.
 */
export const makeDrainableWorker = <A, E = never, R = never>(
  process: (item: A) => Effect.Effect<void, E, R>,
  options: DrainableWorkerOptions<A, E> = {},
): Effect.Effect<DrainableWorker<A>, never, Scope.Scope | R> =>
  Effect.gen(function* () {
    const queue = yield* Effect.acquireRelease(TxQueue.unbounded<A>(), TxQueue.shutdown)
    const outstanding = yield* TxRef.make(0)

    const settleItem = (item: A) =>
      process(item).pipe(
        Effect.catchCause(cause => options.onFailure?.({ cause, item }) ?? Effect.void),
        Effect.ensuring(Effect.tx(TxRef.update(outstanding, count => count - 1))),
      )

    yield* TxQueue.take(queue).pipe(
      Effect.flatMap(settleItem),
      Effect.forever,
      Effect.forkScoped,
    )

    const drain: DrainableWorker<A>["drain"] = Effect.tx(
      TxRef.get(outstanding).pipe(
        Effect.flatMap(count => (count > 0 ? Effect.txRetry : Effect.void)),
      ),
    )

    const enqueue: DrainableWorker<A>["enqueue"] = item =>
      Effect.tx(
        TxQueue.offer(queue, item).pipe(
          Effect.tap(accepted =>
            accepted ? TxRef.update(outstanding, count => count + 1) : Effect.void,
          ),
        ),
      )

    return { drain, enqueue } satisfies DrainableWorker<A>
  })

/**
 * A typed milestone bus for one pipeline. Subscribe BEFORE triggering the
 * pipeline work: PubSub subscriptions only observe signals published after
 * the subscription exists.
 */
export interface PipelineSignalBus<S> {
  /** Publish one milestone signal from Effect code. */
  readonly publish: (signal: S) => Effect.Effect<boolean>
  /**
   * Publish one milestone signal from a synchronous boundary (event handlers,
   * promise executors) that cannot yield an Effect.
   */
  readonly publishUnsafe: (signal: S) => boolean
  /** Scoped subscription; closed with the subscribing scope. */
  readonly subscribe: Effect.Effect<PubSub.Subscription<S>, never, Scope.Scope>
}

/** Create an unbounded typed milestone bus backed by Effect PubSub. */
export const makePipelineSignalBus = <S>(): Effect.Effect<PipelineSignalBus<S>> =>
  Effect.gen(function* () {
    const pubsub = yield* PubSub.unbounded<S>()
    return {
      publish: signal => PubSub.publish(pubsub, signal),
      publishUnsafe: signal => PubSub.publishUnsafe(pubsub, signal),
      subscribe: PubSub.subscribe(pubsub),
    } satisfies PipelineSignalBus<S>
  })

/**
 * Await the first signal on a subscription matching `predicate`, discarding
 * non-matching signals. Deterministic replacement for sleep/poll loops.
 */
export const awaitPipelineSignal = <S, T extends S>(
  subscription: PubSub.Subscription<S>,
  predicate: (signal: S) => signal is T,
): Effect.Effect<T> =>
  Effect.gen(function* () {
    while (true) {
      const signal = yield* PubSub.take(subscription)
      if (predicate(signal)) return signal
    }
  })
