import { Data, Schema as S } from "effect";

import {
  CandidateRef,
  ProviderTurnRef,
  TurnProviderRef,
  TurnRequestRef,
  TurnThreadRef,
  type TurnLifecycleState,
  type TurnRefusalReason,
} from "@openagentsinc/agent-runtime-schema";

/**
 * AFS-01 UI-neutral turn state machine.
 *
 * This module owns the deterministic turn lifecycle transition. It is pure: it
 * imports no platform API, no provider SDK, no store driver, and no UI. Every
 * turn adapter — the in-memory test store and the Desktop transition adapter —
 * folds the same transitions and must reach the same terminal record. The
 * shared corpus below is the single conformance witness both adapters run.
 *
 * The record is the canonical driver-neutral turn state. A store persists it. A
 * projection derives a safe card from it. Neither owns the transition rule.
 */

/** Typed constructors for the frozen branded turn references. */
export const turnRequestRef = S.decodeUnknownSync(TurnRequestRef);
export const turnThreadRef = S.decodeUnknownSync(TurnThreadRef);
export const turnProviderRef = S.decodeUnknownSync(TurnProviderRef);
export const providerTurnRef = S.decodeUnknownSync(ProviderTurnRef);
export const candidateRef = S.decodeUnknownSync(CandidateRef);

/**
 * A turn transition. It is the only vocabulary the state machine folds. Each
 * transition is advisory input the host derives from policy, provider, or owner
 * intent. A transition never carries provider credentials, raw content, or UI.
 */
export type TurnTransition = Data.TaggedEnum<{
  /** The host began deriving a route decision. */
  RouteStarted: Record<never, never>;
  /** The host admitted a route; the selected and effective lanes are bound. */
  RouteAdmitted: { readonly selected: TurnProviderRef; readonly effective: TurnProviderRef };
  /** The filtered candidate set was empty; the turn fails closed with input kept. */
  RouteClosed: Record<never, never>;
  /** The admitted provider started its turn; the provider turn ref is bound. */
  ProviderStarted: { readonly providerTurnRef: ProviderTurnRef };
  /** A bounded progress event arrived from the provider stream. */
  Progress: Record<never, never>;
  /** The provider produced an admitted terminal candidate. */
  Completed: { readonly candidateRef: CandidateRef };
  /** A typed refusal. The user input is kept; no silent provider change occurs. */
  Refused: { readonly reason: TurnRefusalReason };
  /**
   * A provider or host failure after dispatch. It carries a bounded, public-safe
   * reason label (for example `session_failed`), never raw provider output.
   */
  Failed: { readonly reason: string };
  /** The owner or host cancelled the turn. */
  Cancelled: Record<never, never>;
}>;

export const TurnTransition = Data.taggedEnum<TurnTransition>();

/**
 * A generation-fenced transition envelope. Every event names the generation it
 * belongs to. The state machine drops an event whose generation does not match
 * the active turn record, so a late event from a superseded provider run can
 * never mutate the active turn.
 */
export interface TurnEvent {
  readonly generation: number;
  readonly transition: TurnTransition;
}

/** The canonical, driver-neutral turn state record. */
export interface TurnStateRecord {
  readonly requestRef: TurnRequestRef;
  readonly threadRef: TurnThreadRef;
  readonly state: TurnLifecycleState;
  readonly generation: number;
  readonly providerTurnRef: ProviderTurnRef | null;
  readonly selected: TurnProviderRef | null;
  readonly effective: TurnProviderRef | null;
  readonly candidateRef: CandidateRef | null;
  readonly refusalReason: TurnRefusalReason | null;
  /**
   * The bounded, public-safe reason a turn reached the `failed` state. It is set
   * only on a `Failed` transition and is null otherwise. It never carries raw
   * provider output, a command, a path, or a token.
   */
  readonly failureReason: string | null;
  readonly progressCount: number;
}

const TERMINAL_STATES: ReadonlySet<TurnLifecycleState> = new Set<TurnLifecycleState>([
  "completed",
  "refused",
  "failed",
  "cancelled",
]);

/** True when a state accepts no further transition. */
export const isTerminalTurnState = (state: TurnLifecycleState): boolean => TERMINAL_STATES.has(state);

/** The initial accepted record for a newly admitted turn at generation zero. */
export const initialTurnState = (
  requestRef: TurnRequestRef,
  threadRef: TurnThreadRef,
): TurnStateRecord => ({
  requestRef,
  threadRef,
  state: "accepted",
  generation: 0,
  providerTurnRef: null,
  selected: null,
  effective: null,
  candidateRef: null,
  refusalReason: null,
  failureReason: null,
  progressCount: 0,
});

/**
 * Increment the generation of a live turn. The host calls this when a new run
 * supersedes an in-flight run (a retry or an owner cancel). Every event stamped
 * with the previous generation is fenced out after this bump.
 */
export const bumpTurnGeneration = (record: TurnStateRecord): TurnStateRecord => ({
  ...record,
  generation: record.generation + 1,
});

export type TurnTransitionRejection = "already_terminal" | "stale_generation" | "illegal_transition";

export type TurnTransitionOutcome =
  | { readonly ok: true; readonly record: TurnStateRecord }
  | { readonly ok: false; readonly reason: TurnTransitionRejection };

const reject = (reason: TurnTransitionRejection): TurnTransitionOutcome => ({ ok: false, reason });
const accept = (record: TurnStateRecord): TurnTransitionOutcome => ({ ok: true, record });

/**
 * Fold one generation-fenced event into a turn record. The function is total and
 * deterministic:
 *
 * - A generation mismatch is fenced (`stale_generation`) and never mutates state.
 * - A terminal record accepts nothing further (`already_terminal`).
 * - An out-of-order transition is rejected (`illegal_transition`).
 *
 * Only a legal, in-generation, non-terminal transition returns a new record.
 */
export const applyTurnEvent = (record: TurnStateRecord, event: TurnEvent): TurnTransitionOutcome => {
  if (event.generation !== record.generation) return reject("stale_generation");
  if (isTerminalTurnState(record.state)) return reject("already_terminal");

  return TurnTransition.$match(event.transition, {
    RouteStarted: () =>
      record.state === "accepted" ? accept({ ...record, state: "routing" }) : reject("illegal_transition"),
    RouteAdmitted: ({ selected, effective }) =>
      record.state === "routing"
        ? accept({ ...record, state: "dispatching", selected, effective })
        : reject("illegal_transition"),
    RouteClosed: () =>
      record.state === "routing"
        ? accept({ ...record, state: "refused", refusalReason: "route_closed_no_candidate" })
        : reject("illegal_transition"),
    ProviderStarted: ({ providerTurnRef: ref }) =>
      record.state === "dispatching"
        ? accept({ ...record, state: "streaming", providerTurnRef: ref })
        : reject("illegal_transition"),
    Progress: () =>
      record.state === "streaming"
        ? accept({ ...record, progressCount: record.progressCount + 1 })
        : reject("illegal_transition"),
    Completed: ({ candidateRef: ref }) =>
      record.state === "streaming"
        ? accept({ ...record, state: "completed", candidateRef: ref })
        : reject("illegal_transition"),
    Refused: ({ reason }) =>
      record.state === "accepted" ||
      record.state === "routing" ||
      record.state === "dispatching" ||
      record.state === "streaming"
        ? accept({ ...record, state: "refused", refusalReason: reason })
        : reject("illegal_transition"),
    Failed: ({ reason }) =>
      record.state === "dispatching" || record.state === "streaming"
        ? accept({ ...record, state: "failed", failureReason: reason })
        : reject("illegal_transition"),
    Cancelled: () => accept({ ...record, state: "cancelled" }),
  });
};

/**
 * Fold a full event sequence, ignoring fenced and illegal events, and return the
 * final record. This is exactly how an adapter replays a turn: it never crashes
 * on a stale or out-of-order event, it simply keeps the last valid record.
 */
export const foldTurnEvents = (
  initial: TurnStateRecord,
  events: ReadonlyArray<TurnEvent>,
): TurnStateRecord =>
  events.reduce((record, event) => {
    const outcome = applyTurnEvent(record, event);
    return outcome.ok ? outcome.record : record;
  }, initial);

/** One conformance scenario every turn adapter must reproduce identically. */
export interface TurnStateCorpusScenario {
  readonly name: string;
  readonly events: ReadonlyArray<TurnEvent>;
  readonly expected: {
    readonly state: TurnLifecycleState;
    readonly generation: number;
    readonly progressCount: number;
    readonly hasCandidate: boolean;
    readonly refusalReason: TurnRefusalReason | null;
    readonly failureReason?: string | null;
  };
}

const SELECTED = turnProviderRef("provider.codex.1");
const EFFECTIVE = turnProviderRef("provider.codex.1");
const PROVIDER_TURN = providerTurnRef("providerturn.codex.1");
const CANDIDATE = candidateRef("candidate.codex.1");

const gen0 = (transition: TurnTransition): TurnEvent => ({ generation: 0, transition });

/**
 * The shared state-transition corpus. The in-memory store adapter and the
 * Desktop journal adapter both fold every scenario and must reach the same
 * expected record. It exercises complete, refuse, fail, cancel, route-closed,
 * progress bounds, illegal ordering, and generation fencing.
 */
export const TURN_STATE_TRANSITION_CORPUS: ReadonlyArray<TurnStateCorpusScenario> = [
  {
    name: "completes deterministically",
    events: [
      gen0(TurnTransition.RouteStarted()),
      gen0(TurnTransition.RouteAdmitted({ selected: SELECTED, effective: EFFECTIVE })),
      gen0(TurnTransition.ProviderStarted({ providerTurnRef: PROVIDER_TURN })),
      gen0(TurnTransition.Progress()),
      gen0(TurnTransition.Progress()),
      gen0(TurnTransition.Completed({ candidateRef: CANDIDATE })),
    ],
    expected: { state: "completed", generation: 0, progressCount: 2, hasCandidate: true, refusalReason: null },
  },
  {
    name: "fails after dispatch",
    events: [
      gen0(TurnTransition.RouteStarted()),
      gen0(TurnTransition.RouteAdmitted({ selected: SELECTED, effective: EFFECTIVE })),
      gen0(TurnTransition.ProviderStarted({ providerTurnRef: PROVIDER_TURN })),
      gen0(TurnTransition.Failed({ reason: "session_failed" })),
    ],
    expected: {
      state: "failed",
      generation: 0,
      progressCount: 0,
      hasCandidate: false,
      refusalReason: null,
      failureReason: "session_failed",
    },
  },
  {
    name: "refuses on malformed output without dispatch",
    events: [
      gen0(TurnTransition.RouteStarted()),
      gen0(TurnTransition.RouteAdmitted({ selected: SELECTED, effective: EFFECTIVE })),
      gen0(TurnTransition.Refused({ reason: "malformed_output" })),
    ],
    expected: { state: "refused", generation: 0, progressCount: 0, hasCandidate: false, refusalReason: "malformed_output" },
  },
  {
    name: "fails closed when no candidate is admitted",
    events: [gen0(TurnTransition.RouteStarted()), gen0(TurnTransition.RouteClosed())],
    expected: {
      state: "refused",
      generation: 0,
      progressCount: 0,
      hasCandidate: false,
      refusalReason: "route_closed_no_candidate",
    },
  },
  {
    name: "cancels a streaming turn",
    events: [
      gen0(TurnTransition.RouteStarted()),
      gen0(TurnTransition.RouteAdmitted({ selected: SELECTED, effective: EFFECTIVE })),
      gen0(TurnTransition.ProviderStarted({ providerTurnRef: PROVIDER_TURN })),
      gen0(TurnTransition.Cancelled()),
    ],
    expected: { state: "cancelled", generation: 0, progressCount: 0, hasCandidate: false, refusalReason: null },
  },
  {
    name: "ignores an illegal completed before dispatch",
    events: [
      gen0(TurnTransition.RouteStarted()),
      gen0(TurnTransition.Completed({ candidateRef: CANDIDATE })),
      gen0(TurnTransition.RouteAdmitted({ selected: SELECTED, effective: EFFECTIVE })),
      gen0(TurnTransition.ProviderStarted({ providerTurnRef: PROVIDER_TURN })),
      gen0(TurnTransition.Completed({ candidateRef: CANDIDATE })),
    ],
    expected: { state: "completed", generation: 0, progressCount: 0, hasCandidate: true, refusalReason: null },
  },
  {
    name: "fences a late event from a superseded generation",
    events: [
      gen0(TurnTransition.RouteStarted()),
      gen0(TurnTransition.RouteAdmitted({ selected: SELECTED, effective: EFFECTIVE })),
      gen0(TurnTransition.ProviderStarted({ providerTurnRef: PROVIDER_TURN })),
      // The next event belongs to a stale run and must be fenced out entirely.
      { generation: 0, transition: TurnTransition.Completed({ candidateRef: CANDIDATE }) },
      { generation: 1, transition: TurnTransition.Progress() },
    ],
    // The generation-0 Completed applied; the generation-1 Progress was fenced.
    expected: { state: "completed", generation: 0, progressCount: 0, hasCandidate: true, refusalReason: null },
  },
];
