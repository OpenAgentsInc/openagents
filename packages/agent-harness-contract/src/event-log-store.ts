import { Effect, Schema as S } from "effect";
import type { HarnessStreamEvent } from "./stream.ts";

/**
 * Persistence port for the durable harness event log. One turn's events form an
 * ordered, append-only, strictly-increasing-by-`sequence` list. The port is the
 * seam a real backend implements: HARN-02 ships the in-memory reference, the
 * desktop local-turn journal implements it where lanes actually stream, and the
 * managed-sandbox event store (whose opaque `BoxProjectionCursorSchema` cursors
 * are the same concept) implements it for cloud sessions.
 */
export class HarnessEventLogError extends S.TaggedErrorClass<HarnessEventLogError>()(
  "AgentHarness.EventLogError",
  {
    operation: S.String,
    turnId: S.String,
    detail: S.optionalKey(S.String),
    cause: S.optionalKey(S.Defect()),
  },
) {}

export interface HarnessEventLogStore {
  /**
   * Append one event. The store rejects a non-increasing `sequence` for the
   * turn (duplicate or out-of-order) with `HarnessEventLogError` — this is what
   * makes replay duplicate-free.
   */
  readonly append: (event: HarnessStreamEvent) => Effect.Effect<void, HarnessEventLogError>;

  /** Events for `turnId` with `sequence > fromCursor`, ascending. */
  readonly read: (params: {
    readonly turnId: string;
    readonly fromCursor: number;
  }) => Effect.Effect<ReadonlyArray<HarnessStreamEvent>, HarnessEventLogError>;

  /** The greatest `sequence` stored for `turnId`, or `-1` when empty. */
  readonly lastCursor: (params: {
    readonly turnId: string;
  }) => Effect.Effect<number, HarnessEventLogError>;

  /**
   * Record that the turn was re-driven from persisted state at `atCursor`: the
   * tail after `atCursor` is recomputed, not attached. Stored explicitly so a
   * consumer can distinguish a lossless attach from a recomputed tail.
   */
  readonly recordRerunBoundary: (params: {
    readonly turnId: string;
    readonly atCursor: number;
  }) => Effect.Effect<void, HarnessEventLogError>;

  /** The recorded rerun-boundary cursors for `turnId`, ascending. */
  readonly rerunBoundaries: (params: {
    readonly turnId: string;
  }) => Effect.Effect<ReadonlyArray<number>, HarnessEventLogError>;
}

interface TurnLog {
  events: Array<HarnessStreamEvent>;
  boundaries: Array<number>;
}

/**
 * In-memory reference store. Correct and deterministic for tests. Not durable
 * across process death by itself — the HARN-02 "process death" test proves
 * recovery by handing the SAME store instance to a fresh {@link HarnessEventLog}
 * runtime, which is exactly what a real durable backend guarantees.
 */
export const makeInMemoryEventLogStore = (): HarnessEventLogStore => {
  const turns = new Map<string, TurnLog>();
  const forTurn = (turnId: string): TurnLog => {
    const existing = turns.get(turnId);
    if (existing !== undefined) return existing;
    const created: TurnLog = { events: [], boundaries: [] };
    turns.set(turnId, created);
    return created;
  };

  return {
    append: (event) =>
      Effect.gen(function* () {
        const turnId = event.turnId;
        const log = forTurn(turnId);
        const last = log.events.length === 0 ? -1 : log.events[log.events.length - 1]!.sequence;
        if (event.sequence <= last) {
          return yield* Effect.fail(
            new HarnessEventLogError({
              operation: "append",
              turnId,
              detail: `non-increasing sequence ${event.sequence} <= ${last}`,
            }),
          );
        }
        log.events.push(event);
      }),

    read: ({ turnId, fromCursor }) =>
      Effect.sync(() => {
        const log = turns.get(turnId);
        if (log === undefined) return [];
        return log.events.filter((e) => e.sequence > fromCursor);
      }),

    lastCursor: ({ turnId }) =>
      Effect.sync(() => {
        const log = turns.get(turnId);
        if (log === undefined || log.events.length === 0) return -1;
        return log.events[log.events.length - 1]!.sequence;
      }),

    recordRerunBoundary: ({ turnId, atCursor }) =>
      Effect.sync(() => {
        const log = forTurn(turnId);
        log.boundaries.push(atCursor);
        log.boundaries.sort((a, b) => a - b);
      }),

    rerunBoundaries: ({ turnId }) =>
      Effect.sync(() => {
        const log = turns.get(turnId);
        return log === undefined ? [] : [...log.boundaries];
      }),
  };
};
