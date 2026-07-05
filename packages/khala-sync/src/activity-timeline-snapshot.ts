import { Schema as S } from "effect"

/**
 * Public activity-timeline stored-snapshot entity contract (KS-6.7b, #8421;
 * SPEC §2.1 `scope.public.<channel>`, §7 invariant 8/9).
 *
 * The public activity timeline (`GET /api/public/activity-timeline`) live-
 * merges SEVEN separate source stores every request (Pylon registrations/
 * presence, training run/window/lease/verification authority, settlement
 * receipts, inference receipts, forum topics/posts, Artanis admin ticks, and
 * Pylon capacity funnel snapshots) — see
 * `apps/openagents.com/workers/api/src/public-activity-timeline.ts`. Unlike
 * the KS-6.7 tokens-served-aggregates group, none of these seven stores
 * share a single clean write-site hook (no shared ledger insert, no shared
 * "one function creates this row" call site) that a KS-6.1/KS-6.3-style
 * event-driven producer could tap without instrumenting seven independent
 * write paths across five-plus modules.
 *
 * So this projection is REBUILD-ON-CRON, not event-sourced: a scheduled
 * Worker tick (`apps/openagents.com/workers/api/src/
 * khala-sync-public-activity-timeline.ts`) periodically re-runs the EXACT
 * SAME live-merge function
 * (`buildPublicActivityTimelineRawSnapshot`) against the real D1 stores and
 * stores its full output (every event the live merge would currently
 * return, not a partial subset of the seven domains) as ONE snapshot here.
 * Because the stored post-image is exactly what the live merge already
 * computes, this projection covers ALL seven source domains identically —
 * there is no shallow/partial-coverage subset to flag.
 *
 * MANY EVENTS, ONE ENTITY (single-row snapshot, mirrors the settled-feed
 * summary / gym-run-progress "one entity per scope" shape): the whole
 * bounded recent-event window rides ONE upsert keyed by the constant
 * `ACTIVITY_TIMELINE_SNAPSHOT_ENTITY_ID`, so a read is a single changelog
 * lookup, not a fan-out over many rows.
 *
 * PUBLIC-SAFE BY CONSTRUCTION: every event embedded here has already passed
 * the live merge's own `assertPublicActivityTimelineEventSafe` gate (raw/
 * private material scan, cursor-consistency, receipt-source requirements)
 * before it is ever handed to this projection — this module re-validates
 * shape only (bounded literals, arrays of strings, ISO timestamps), the same
 * "decode-then-encode is defense in depth" discipline used by every other
 * KS-6.x projection.
 *
 * This module is deliberately self-contained (imports only `effect`) so it
 * can be re-exported from ./index without a module cycle — same rule as
 * ./fleet, ./public-counter, ./settled-feed, ./gym, and ./tokens-served-mix.
 * It intentionally re-declares the event/source-lag shape rather than
 * depending on `@openagentsinc/public-activity-timeline` (that package is a
 * separate leaf; duplicating the bounded shape here keeps khala-sync's own
 * "no cross-feature-package dependency" precedent intact).
 */

// ---------------------------------------------------------------------------
// Entity type name (changelog `entityType` value) + scope channel id
// ---------------------------------------------------------------------------

/** The `<channel>` segment of the scope: `scope.public.activity-timeline`. */
export const ACTIVITY_TIMELINE_SNAPSHOT_CHANNEL_ID = "activity-timeline"

export const ACTIVITY_TIMELINE_SNAPSHOT_ENTITY_TYPE =
  "activity_timeline_snapshot"

/** Single-row snapshot: the whole bounded recent-event window, one entity. */
export const ACTIVITY_TIMELINE_SNAPSHOT_ENTITY_ID = "current"

// ---------------------------------------------------------------------------
// Bounded field primitives (mirrors packages/public-activity-timeline's
// literal sets; kept in lockstep manually — see module doc for why this is
// duplicated rather than imported).
// ---------------------------------------------------------------------------

export const ActivityTimelineSnapshotEventKind = S.Literals([
  "pylon_registered",
  "pylon_heartbeat",
  "wallet_ready",
  "assignment_ready",
  "window_opened",
  "window_closed",
  "work_claimed",
  "trace_submitted",
  "verification_queued",
  "verification_verified",
  "verification_rejected",
  "khala_inference_served",
  "settlement_recorded",
  "real_bitcoin_moved",
  "forum_topic_created",
  "forum_posted",
  "artanis_tick",
  "capacity_snapshot",
  "projection_gap",
])
export type ActivityTimelineSnapshotEventKind =
  typeof ActivityTimelineSnapshotEventKind.Type

export const ActivityTimelineSnapshotSourceKind = S.Literals([
  "pylon_api",
  "pylon_presence",
  "training_window",
  "training_trace",
  "training_verification",
  "inference_receipt",
  "settlement_receipt",
  "forum",
  "artanis",
  "capacity_funnel",
  "projection_gap",
])
export type ActivityTimelineSnapshotSourceKind =
  typeof ActivityTimelineSnapshotSourceKind.Type

export const ActivityTimelineSnapshotSourceLagStatus = S.Literals([
  "current",
  "stale",
  "unavailable",
  "projection_gap",
])
export type ActivityTimelineSnapshotSourceLagStatus =
  typeof ActivityTimelineSnapshotSourceLagStatus.Type

/** Bounded public-safe ref/label string (already-sanitized refs only). */
export const ActivityTimelineSnapshotRef = S.String.check(S.isMaxLength(400))

// ---------------------------------------------------------------------------
// activity_timeline_event_snapshot — one entry per merged public event
// ---------------------------------------------------------------------------

export class ActivityTimelineSnapshotEventEntity extends S.Class<ActivityTimelineSnapshotEventEntity>(
  "ActivityTimelineSnapshotEventEntity",
)({
  actorRef: S.optional(ActivityTimelineSnapshotRef),
  amountSats: S.optional(S.Number),
  blockerRefs: S.Array(ActivityTimelineSnapshotRef),
  caveatRefs: S.Array(ActivityTimelineSnapshotRef),
  cursor: S.String,
  eventRef: S.String,
  kind: ActivityTimelineSnapshotEventKind,
  realBitcoinMoved: S.optional(S.Boolean),
  refs: S.Array(ActivityTimelineSnapshotRef),
  runRef: S.optional(ActivityTimelineSnapshotRef),
  sourceKind: ActivityTimelineSnapshotSourceKind,
  sourceRefs: S.Array(ActivityTimelineSnapshotRef),
  state: S.optional(S.String.check(S.isMaxLength(200))),
  targetRef: S.optional(ActivityTimelineSnapshotRef),
  text: S.String.check(S.isMaxLength(2_000)),
  ts: S.String,
  windowRef: S.optional(ActivityTimelineSnapshotRef),
}) {}

export class ActivityTimelineSnapshotSourceLagEntity extends S.Class<ActivityTimelineSnapshotSourceLagEntity>(
  "ActivityTimelineSnapshotSourceLagEntity",
)({
  blockerRefs: S.Array(ActivityTimelineSnapshotRef),
  caveatRefs: S.Array(ActivityTimelineSnapshotRef),
  lagSeconds: S.NullOr(S.Number),
  latestSourceEventAt: S.NullOr(S.String),
  maxStalenessSeconds: S.Number,
  observedAt: S.String,
  sourceKind: ActivityTimelineSnapshotSourceKind,
  sourceRefs: S.Array(ActivityTimelineSnapshotRef),
  status: ActivityTimelineSnapshotSourceLagStatus,
}) {}

// ---------------------------------------------------------------------------
// activity_timeline_snapshot — the single stored entity (entityId = "current")
// ---------------------------------------------------------------------------

export class ActivityTimelineSnapshotEntity extends S.Class<ActivityTimelineSnapshotEntity>(
  "ActivityTimelineSnapshotEntity",
)({
  events: S.Array(ActivityTimelineSnapshotEventEntity),
  generatedAt: S.String,
  sourceLag: S.Array(ActivityTimelineSnapshotSourceLagEntity),
}) {}

// ---------------------------------------------------------------------------
// Boundary codecs
// ---------------------------------------------------------------------------

export const decodeActivityTimelineSnapshotEntity = S.decodeUnknownSync(
  ActivityTimelineSnapshotEntity,
)
export const encodeActivityTimelineSnapshotEntity = S.encodeSync(
  ActivityTimelineSnapshotEntity,
)
