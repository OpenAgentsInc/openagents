import { Effect, Ref, Stream } from "effect";
import type { AgentHarness, HarnessStartOptions } from "./adapter.ts";
import type { HarnessEventLog } from "./event-log.ts";
import type { HarnessContinuationState } from "./lifecycle-state.ts";
import type { HarnessSession, HarnessTurnResult } from "./session.ts";
import type { HarnessCursor, HarnessStreamEvent } from "./stream.ts";

/**
 * Intra-turn slice runner — the HARN-06 durability payoff of HARN-01 (the
 * suspend/continue contract) and HARN-02 (the durable event log).
 *
 * A long turn is run as a chain of short, time-boxed slices. Each slice consumes
 * a bounded number of events; if the turn is not finished when the budget is
 * spent, the session is suspended NON-destructively at the exact cursor, and the
 * next slice resumes from `continueFrom` with no gap and no duplicate. This is
 * what lets a multi-hour turn survive short-lived process invocations (a durable
 * workflow step, an `oa-dev --restart` generation swap) and what gives Full Auto
 * cursor-exact liveness.
 *
 * The budget here is an EVENT COUNT rather than wall-clock time. That keeps the
 * runner deterministic and testable without a clock, and maps cleanly onto a
 * real time budget in the desktop wiring (the same suspend/continue calls, gated
 * on elapsed time instead of a counter).
 */
export interface HarnessSliceBudget {
  /** Maximum stream events to consume in one slice before suspending. */
  readonly maxEvents: number;
}

export type HarnessSliceStatus = "completed" | "suspended";

export interface HarnessSliceOutcome {
  readonly status: HarnessSliceStatus;
  /** The cursor of the last event delivered in this slice. */
  readonly cursor: HarnessCursor;
  /** The kind of the last event delivered — the liveness signal Full Auto reads. */
  readonly lastEventKind?: string;
  /** Present when the turn finished within the slice. */
  readonly result?: HarnessTurnResult;
  /** Present when the slice suspended a still-running turn. */
  readonly continuation?: HarnessContinuationState;
}

const NO_CURSOR = -1;

/**
 * Run one time-boxed slice of an already-started prompt control. Consumes up to
 * `budget.maxEvents` events. If the turn's stream completes within the budget,
 * the slice is `completed`. Otherwise the session is suspended and the slice is
 * `suspended`, carrying the `continueFrom` the next slice re-enters with.
 */
export const runHarnessSlice = (params: {
  readonly session: HarnessSession;
  readonly control: {
    readonly events: Stream.Stream<HarnessStreamEvent, unknown>;
    readonly done: Effect.Effect<HarnessTurnResult, unknown>;
  };
  readonly budget: HarnessSliceBudget;
  /** Optional durable log: every delivered event is appended for replay. */
  readonly eventLog?: HarnessEventLog;
}): Effect.Effect<HarnessSliceOutcome, unknown> =>
  Effect.gen(function* () {
    const { session, control, budget, eventLog } = params;
    const cursorRef = yield* Ref.make<HarnessCursor>(NO_CURSOR);
    const lastKindRef = yield* Ref.make<string | undefined>(undefined);
    const deliveredRef = yield* Ref.make(0);

    // Consume at most `maxEvents` from the turn stream, recording cursor/kind and
    // (optionally) persisting each event to the durable log.
    yield* control.events.pipe(
      Stream.take(budget.maxEvents),
      Stream.runForEach((event) =>
        Effect.gen(function* () {
          yield* Ref.set(cursorRef, event.sequence);
          yield* Ref.set(lastKindRef, event.kind);
          yield* Ref.update(deliveredRef, (n) => n + 1);
          if (eventLog !== undefined) {
            // Best-effort persistence: a re-driven (lossy) slice may re-emit an
            // already-persisted sequence, which the log rejects. Ignore that so
            // the slice loop stays fail-soft, exactly as the desktop trace/token
            // ingest is fail-soft around the coding task.
            yield* eventLog.appendEvent(event).pipe(Effect.catch(() => Effect.void));
          }
        }),
      ),
    );

    const cursor = yield* Ref.get(cursorRef);
    const lastEventKind = yield* Ref.get(lastKindRef);
    const delivered = yield* Ref.get(deliveredRef);

    // The turn completed within this slice when either the stream produced fewer
    // events than the budget (it ended), or the last delivered event is a turn
    // terminal (`turn.finished` / `turn.interrupted`). Checking the terminal kind
    // handles the exact-boundary case where the budget lands precisely on the
    // last event — a count-only test would wrongly suspend an already-finished
    // turn.
    const endedEarly = delivered < budget.maxEvents;
    const lastWasTerminal =
      lastEventKind === "turn.finished" || lastEventKind === "turn.interrupted";
    if (endedEarly || lastWasTerminal) {
      const result = yield* control.done;
      return {
        status: "completed",
        cursor,
        ...(lastEventKind === undefined ? {} : { lastEventKind }),
        result,
      };
    }

    // Budget spent with the turn still live: suspend at the exact cursor.
    const continuation = yield* session.suspendTurn();
    return {
      status: "suspended",
      cursor,
      ...(lastEventKind === undefined ? {} : { lastEventKind }),
      continuation,
    };
  });

/**
 * Drive a whole prompt turn to completion as a chain of slices, re-entering the
 * session from `continueFrom` after every suspension. Each continuation may
 * happen in a fresh session (a different process): the loop calls
 * `adapter.start({ continueFrom })` then `continueTurn`, exactly as a durable
 * workflow step or a restart would. Returns the final turn result plus the slice
 * count, and (when an event log is supplied) every event is persisted in order.
 */
export const runTurnInSlices = (params: {
  readonly adapter: AgentHarness;
  readonly startOptions: HarnessStartOptions;
  readonly turnId: string;
  readonly prompt: string;
  readonly budget: HarnessSliceBudget;
  readonly eventLog?: HarnessEventLog;
  /** Safety bound on slices so a stuck turn cannot loop forever. */
  readonly maxSlices?: number;
}): Effect.Effect<{ readonly result: HarnessTurnResult; readonly slices: number }, unknown> =>
  Effect.gen(function* () {
    const { adapter, startOptions, turnId, prompt, budget, eventLog } = params;
    const maxSlices = params.maxSlices ?? 1000;

    // First slice: fresh session + prompt turn.
    const firstSession = yield* adapter.start(startOptions);
    const firstControl = yield* firstSession.promptTurn({ turnId, prompt });
    let outcome: HarnessSliceOutcome = yield* runHarnessSlice({
      session: firstSession,
      control: firstControl,
      budget,
      ...(eventLog === undefined ? {} : { eventLog }),
    });
    let slices = 1;

    // Subsequent slices: re-enter from the continuation and continue the turn.
    while (outcome.status === "suspended") {
      if (slices >= maxSlices) {
        return yield* Effect.fail(new Error(`runTurnInSlices exceeded ${maxSlices} slices`));
      }
      const continuation = outcome.continuation!;
      const session = yield* adapter.start({
        ...startOptions,
        continueFrom: continuation,
      });
      const control = yield* session.continueTurn({});
      outcome = yield* runHarnessSlice({
        session,
        control,
        budget,
        ...(eventLog === undefined ? {} : { eventLog }),
      });
      slices += 1;
    }

    return { result: outcome.result!, slices };
  });
