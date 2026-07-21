import { Deferred, Effect, PubSub, Ref, Stream } from "effect";
import type { HarnessEventLogError, HarnessEventLogStore } from "./event-log-store.ts";
import type { HarnessCursor, HarnessStreamEvent } from "./stream.ts";

/**
 * Durable, cursor-exact event log — the HARN-02 runtime. It is the mechanism
 * under renderer reconnect, Full Auto reconcile, restart-resume (FA-REL-01),
 * and mobile supervision: one seq-cursor log with attach / replay / rerun,
 * generic across adapters.
 *
 * `replay` is the crash-recovery path: a finite pull of persisted events from a
 * cursor. The producer need not still be alive. `attach` adds live-follow with
 * single-flight per (turn, consumer class): a newer attach supersedes the older
 * so a reconnecting consumer never double-runs.
 */
export interface HarnessEventLog {
  /**
   * Persist one event and fan it out to live followers. Fails if the store
   * rejects a non-increasing sequence (duplicate / out-of-order) for the turn.
   */
  readonly appendEvent: (event: HarnessStreamEvent) => Effect.Effect<void, HarnessEventLogError>;

  /**
   * Finite replay of persisted events for `turnId` with `sequence > fromCursor`,
   * ascending. Crash recovery: reads only the durable store, so it works after
   * the producing runtime is gone.
   */
  readonly replay: (params: {
    readonly turnId: string;
    readonly fromCursor: number;
  }) => Stream.Stream<HarnessStreamEvent, HarnessEventLogError>;

  /**
   * Live attach: replay the persisted tail from `fromCursor`, then follow new
   * events, with no gap and no duplicate at the seam. Single-flight per
   * `(turnId, consumerClass)` — starting another attach for the same pair ends
   * this one cleanly.
   */
  readonly attach: (params: {
    readonly turnId: string;
    readonly fromCursor: number;
    readonly consumerClass: string;
  }) => Stream.Stream<HarnessStreamEvent, HarnessEventLogError>;

  /** Greatest stored `sequence` for `turnId`, or `-1` when empty. */
  readonly lastCursor: (params: {
    readonly turnId: string;
  }) => Effect.Effect<HarnessCursor, HarnessEventLogError>;

  /**
   * Record that the turn was re-driven from persisted state at `atCursor`. The
   * tail after `atCursor` is recomputed, not attached — recorded explicitly so a
   * consumer distinguishes a lossless attach from a recomputed tail.
   */
  readonly markRerunBoundary: (params: {
    readonly turnId: string;
    readonly atCursor: number;
  }) => Effect.Effect<void, HarnessEventLogError>;

  /** The recorded rerun-boundary cursors for `turnId`, ascending. */
  readonly rerunBoundaries: (params: {
    readonly turnId: string;
  }) => Effect.Effect<ReadonlyArray<HarnessCursor>, HarnessEventLogError>;
}

/**
 * Build a {@link HarnessEventLog} over a {@link HarnessEventLogStore}. Handing a
 * fresh runtime the SAME store instance models process death: the new runtime
 * replays everything the old one persisted.
 */
export const makeHarnessEventLog = (store: HarnessEventLogStore): Effect.Effect<HarnessEventLog> =>
  Effect.gen(function* () {
    // Per-turn live fan-out.
    const pubsubs = yield* Ref.make(new Map<string, PubSub.PubSub<HarnessStreamEvent>>());
    // Active single-flight subscriber per `${turnId}:${consumerClass}`.
    const active = yield* Ref.make(new Map<string, Deferred.Deferred<void>>());

    const pubsubFor = (turnId: string) =>
      Effect.gen(function* () {
        const map = yield* Ref.get(pubsubs);
        const existing = map.get(turnId);
        if (existing !== undefined) return existing;
        const created = yield* PubSub.unbounded<HarnessStreamEvent>();
        yield* Ref.update(pubsubs, (m) => new Map(m).set(turnId, created));
        return created;
      });

    const appendEvent = (event: HarnessStreamEvent) =>
      Effect.gen(function* () {
        yield* store.append(event);
        const ps = yield* pubsubFor(event.turnId);
        yield* PubSub.publish(ps, event);
      });

    const replay = (params: { turnId: string; fromCursor: number }) =>
      Stream.unwrap(store.read(params).pipe(Effect.map((events) => Stream.fromIterable(events))));

    const attach = (params: { turnId: string; fromCursor: number; consumerClass: string }) =>
      // `Stream.unwrap` excludes `Scope` from the result, so the subscription
      // acquired below lives exactly as long as the returned stream.
      Stream.unwrap(
        Effect.gen(function* () {
          const { turnId, fromCursor, consumerClass } = params;
          const ps = yield* pubsubFor(turnId);

          // Subscribe BEFORE reading the store so an event appended during the
          // read lands in the live queue and is delivered exactly once.
          const subscription = yield* PubSub.subscribe(ps);

          // Single-flight: supersede any prior attach for this (turn, class).
          const key = `${turnId}:${consumerClass}`;
          const superseded = yield* Deferred.make<void>();
          const prior = (yield* Ref.get(active)).get(key);
          if (prior !== undefined) {
            yield* Deferred.succeed(prior, undefined);
          }
          yield* Ref.update(active, (m) => new Map(m).set(key, superseded));

          const persisted = yield* store.read({ turnId, fromCursor });
          const lastPersisted =
            persisted.length === 0 ? fromCursor : persisted[persisted.length - 1]!.sequence;

          const live = Stream.fromSubscription(subscription).pipe(
            Stream.filter((event) => event.sequence > lastPersisted),
          );

          return Stream.concat(Stream.fromIterable(persisted), live).pipe(
            Stream.interruptWhen(Deferred.await(superseded)),
          );
        }),
      );

    const log: HarnessEventLog = {
      appendEvent,
      replay,
      attach,
      lastCursor: (params) => store.lastCursor(params),
      markRerunBoundary: (params) => store.recordRerunBoundary(params),
      rerunBoundaries: (params) => store.rerunBoundaries(params),
    };
    return log;
  });
