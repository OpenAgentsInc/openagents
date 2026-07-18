import { Schema as S } from "effect";

/**
 * MOB-FA-02 (#8994): typed durable Pause/Resume/Stop intents dispatched from
 * OpenAgents mobile toward a Desktop-owned `FullAutoRun`.
 *
 * Desktop is not always running or reachable when the phone wants to act
 * (asleep, offline, owner away from the Mac). Per the issue's explicit
 * architectural steer, this is deliberately SERVER-MEDIATED and
 * eventually-consistent, mirroring how #8981's Desktop->server projection
 * publish already rides a periodic heartbeat rather than a live socket:
 *
 *  1. Mobile POSTs a `FullAutoRunControlIntent` to the server
 *     (`/api/full-auto-runs/control-intents`); the server durably records it
 *     `status: "pending"` and returns immediately -- never a synchronous
 *     round trip to Desktop.
 *  2. Desktop's existing FullAutoRun action-context loop (the same one that
 *     backs the loopback control API and the owner UI's IPC, extended by
 *     `full-auto-run-control-intent-consumer.ts`) polls the SAME endpoint on
 *     its next heartbeat tick, applies the intent through
 *     `full-auto-run-actions.ts` with `actor: "mobile"`, and POSTs the typed
 *     outcome (`applied` or `rejected`, with a bounded reason) back.
 *  3. Mobile polls for that outcome and renders pending vs. applied honestly
 *     -- never completing the UI from optimistic/notification state (the
 *     issue's non-negotiable boundary).
 *
 * This is a NEW typed vocabulary, not a reuse of `KhalaFleetIntent`
 * (`@openagentsinc/khala-fleet-intents`): that package's `fleet_run_control`
 * variant is bound to Sarah FleetRun refs (`FleetSteeringRunRef`'s
 * `fleet_run.sarah.*` pattern) and delivered through the Khala Sync
 * mutator/changelog machinery Pylon polls. FullAutoRun is a different
 * domain (Desktop's single-thread objective/lifecycle model, already served
 * through the bespoke `/api/full-auto-runs` push/pull route from #8981) --
 * this module is the sibling mutation vocabulary for that SAME route family,
 * per the issue's explicit "extend the existing endpoint or add a sibling
 * mutation endpoint following the same pattern" guidance.
 */
export const FULL_AUTO_RUN_CONTROL_INTENT_SCHEMA = "full_auto_run.control_intent.v1" as const;

export const FullAutoRunControlAction = S.Literals(["pause", "resume", "stop"]);
export type FullAutoRunControlAction = typeof FullAutoRunControlAction.Type;

export const fullAutoRunControlActions: ReadonlyArray<FullAutoRunControlAction> = [
  "pause",
  "resume",
  "stop",
];

/** v1: mobile is the only phone-class dispatch surface for this intent. */
export const FullAutoRunControlIntentSurface = S.Literals(["mobile"]);
export type FullAutoRunControlIntentSurface = typeof FullAutoRunControlIntentSurface.Type;

export const FullAutoRunControlIntentStatus = S.Literals(["pending", "applied", "rejected"]);
export type FullAutoRunControlIntentStatus = typeof FullAutoRunControlIntentStatus.Type;

/**
 * Bounded, typed rejection vocabulary -- mirrors the error codes
 * `full-auto-run-actions.ts`'s `pauseFullAutoRunAction`/`resumeFullAutoRunAction`/
 * `stopFullAutoRunAction` already return, so a rejected intent always carries
 * an honest, specific reason rather than a generic failure.
 */
export const FullAutoRunControlRejectionReason = S.Literals([
  "run_not_found",
  "illegal_transition",
  "workspace_mismatch",
  "lane_not_eligible",
  "desktop_unreachable",
  "storage_unavailable",
]);
export type FullAutoRunControlRejectionReason = typeof FullAutoRunControlRejectionReason.Type;

const PublicRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(160),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
);
const FullAutoRunControlRunRef = S.String.check(S.isMinLength(1), S.isMaxLength(180));
const FullAutoRunControlTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/u),
);
const FullAutoRunControlLifecycleState = S.Literals([
  "draft",
  "running",
  "pausing",
  "paused",
  "retrying",
  "stalled",
  "completed",
  "failed",
  "stopped",
  "cap_reached",
]);

/** The durable record: mobile's dispatch request plus whatever outcome
 * Desktop has (or has not yet) reported. */
export const FullAutoRunControlIntent = S.Struct({
  schema: S.Literal(FULL_AUTO_RUN_CONTROL_INTENT_SCHEMA),
  intentId: PublicRef,
  idempotencyKey: PublicRef,
  runRef: FullAutoRunControlRunRef,
  action: FullAutoRunControlAction,
  surface: FullAutoRunControlIntentSurface,
  createdAt: FullAutoRunControlTimestamp,
  status: FullAutoRunControlIntentStatus,
  appliedAt: S.NullOr(FullAutoRunControlTimestamp),
  rejectionReason: S.NullOr(FullAutoRunControlRejectionReason),
  resultLifecycleState: S.NullOr(FullAutoRunControlLifecycleState),
});
export type FullAutoRunControlIntent = typeof FullAutoRunControlIntent.Type;

export const decodeFullAutoRunControlIntent = (value: unknown): FullAutoRunControlIntent =>
  S.decodeUnknownSync(FullAutoRunControlIntent)(value, { onExcessProperty: "error" });

/** The mobile -> server dispatch request body (POST). Mobile mints
 * `intentId`/`idempotencyKey` client-side so a retried POST after a dropped
 * response is idempotent rather than double-dispatching. */
export const FullAutoRunControlIntentDispatchRequest = S.Struct({
  intentId: PublicRef,
  idempotencyKey: PublicRef,
  runRef: FullAutoRunControlRunRef,
  action: FullAutoRunControlAction,
});
export type FullAutoRunControlIntentDispatchRequest =
  typeof FullAutoRunControlIntentDispatchRequest.Type;

/** The Desktop -> server outcome report body (POST). */
export const FullAutoRunControlIntentOutcomeReport = S.Struct({
  intentId: PublicRef,
  status: S.Literals(["applied", "rejected"]),
  rejectionReason: S.optional(FullAutoRunControlRejectionReason),
  resultLifecycleState: S.optional(FullAutoRunControlLifecycleState),
});
export type FullAutoRunControlIntentOutcomeReport =
  typeof FullAutoRunControlIntentOutcomeReport.Type;

export const FullAutoRunControlIntentListEnvelope = S.Struct({
  schema: S.Literal(FULL_AUTO_RUN_CONTROL_INTENT_SCHEMA),
  intents: S.Array(FullAutoRunControlIntent),
});
export type FullAutoRunControlIntentListEnvelope = typeof FullAutoRunControlIntentListEnvelope.Type;
