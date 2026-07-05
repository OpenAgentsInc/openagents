import { Schema as S } from "effect"

/**
 * Public settled-feed entity contracts (KS-6.4, #8414; SPEC §2.1
 * `scope.public.<channel>`, §7 invariant 9).
 *
 * The live settled feed (openagents #5311) streams ONE public-safe "settled"
 * event per settled Bitcoin leg as real settlements land, plus a running
 * summary (latest event + cumulative totals). This module is the khala-sync
 * projection shape for that feed: `SettledFeedEventEntity` is one event's
 * post-image (`entityId` = the event's stable `eventRef`); the running
 * totals it carries (`totalSettledCount`/`totalSettledSats`) are computed
 * UPSTREAM from the real settlement ledger before the event reaches this
 * contract, so the projection never invents a delta — it only replicates an
 * already-authoritative post-image (unlike the tokens-served public counter,
 * which increments its own stored total; see ./public-counter.ts).
 * `SettledFeedSummaryEntity` is the single latest-state summary row
 * (`entityId` = `SETTLED_FEED_SUMMARY_ENTITY_ID`).
 *
 * PUBLIC-SAFE BY CONSTRUCTION (SPEC §7 invariant 9): bounded refs, a bounded
 * actor literal, non-negative integer amounts/totals, and ISO timestamps.
 * No raw `spark1…` address, invoice, preimage, wallet material, or secret
 * can decode into this shape.
 *
 * This module is deliberately self-contained (imports only `effect`) so it
 * can be re-exported from ./index without a module cycle — same rule as
 * ./fleet and ./public-counter.
 */

// ---------------------------------------------------------------------------
// Entity type names (changelog `entityType` values) + scope channel id
// ---------------------------------------------------------------------------

export const SETTLED_FEED_EVENT_ENTITY_TYPE = "settled_feed_event"
export const SETTLED_FEED_SUMMARY_ENTITY_TYPE = "settled_feed_summary"

/** The `<channel>` segment of the scope: `scope.public.settled-feed`. */
export const SETTLED_FEED_CHANNEL_ID = "settled-feed"

/** The summary entity's fixed `entityId` in the changelog. */
export const SETTLED_FEED_SUMMARY_ENTITY_ID = "summary"

// ---------------------------------------------------------------------------
// Bounded field primitives
// ---------------------------------------------------------------------------

/**
 * Bounded public-safe ref: an ASCII token starting with an alphanumeric,
 * then alphanumerics plus `._:-`, up to 256 chars. Structurally refuses
 * whitespace, emails, filesystem paths, and payment-material shapes (which
 * all require characters this pattern excludes) — same discipline as the
 * fleet contracts' `PUBLIC_REF_PATTERN`.
 */
export const SettledFeedRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]{0,255}$/),
)

/** ISO-8601 UTC timestamp string (same shape the wire contracts use). */
export const SettledFeedIsoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
)

/** Non-negative safe-integer amount/total (sats and counts are never negative). */
export const SettledFeedNonNegativeInt = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
)

export const SettledFeedActor = S.Literals(["worker", "validator"])
export type SettledFeedActor = typeof SettledFeedActor.Type

// ---------------------------------------------------------------------------
// settled_feed_event entity
// ---------------------------------------------------------------------------

/**
 * One settled event's post-image. `entityId` is the event's `eventRef`.
 * Carries the running cumulative totals AFTER this event (computed upstream
 * from the real settlement ledger — see the module doc).
 */
export class SettledFeedEventEntity extends S.Class<SettledFeedEventEntity>(
  "SettledFeedEventEntity",
)({
  amountSats: SettledFeedNonNegativeInt,
  challengeRef: SettledFeedRef,
  /** Public contributor digest ref (e.g. `pylon.worker.orrery`). */
  contributorRef: SettledFeedRef,
  eventRef: SettledFeedRef,
  party: SettledFeedActor,
  runRef: SettledFeedRef,
  settledAt: SettledFeedIsoTimestamp,
  totalSettledCount: SettledFeedNonNegativeInt,
  totalSettledSats: SettledFeedNonNegativeInt,
  windowRef: S.NullOr(SettledFeedRef),
}) {}

// ---------------------------------------------------------------------------
// settled_feed_summary entity
// ---------------------------------------------------------------------------

/**
 * The feed's single latest-state summary. `entityId` is always
 * {@link SETTLED_FEED_SUMMARY_ENTITY_ID}.
 */
export class SettledFeedSummaryEntity extends S.Class<SettledFeedSummaryEntity>(
  "SettledFeedSummaryEntity",
)({
  latestEventRef: S.NullOr(SettledFeedRef),
  latestSettledAt: S.NullOr(SettledFeedIsoTimestamp),
  totalSettledCount: SettledFeedNonNegativeInt,
  totalSettledSats: SettledFeedNonNegativeInt,
  updatedAt: SettledFeedIsoTimestamp,
}) {}

// ---------------------------------------------------------------------------
// Boundary codecs
// ---------------------------------------------------------------------------

export const decodeSettledFeedEventEntity = S.decodeUnknownSync(
  SettledFeedEventEntity,
)
export const encodeSettledFeedEventEntity = S.encodeSync(
  SettledFeedEventEntity,
)
export const decodeSettledFeedSummaryEntity = S.decodeUnknownSync(
  SettledFeedSummaryEntity,
)
export const encodeSettledFeedSummaryEntity = S.encodeSync(
  SettledFeedSummaryEntity,
)
