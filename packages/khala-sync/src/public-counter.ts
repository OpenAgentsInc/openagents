import { Schema as S } from "effect"

/**
 * Public-counter entity contract (KS-6.3, #8304; SPEC §2.1
 * `scope.public.<channel>`, §7 invariant 8).
 *
 * A public counter is the projected value of an exact-row aggregate (today:
 * the "Khala Tokens Served" headline, the running SUM over
 * `token_usage_events`). The ingest path bumps the projected counter and
 * appends this post-image to the counter's public scope; the public route
 * serves the projection; a reconcile job proves projection == SUM(exact
 * rows) and an admin repair action realigns it with an audit note. The sync
 * path never invents counter deltas — every increment is keyed to one exact
 * source ledger row's idempotency key.
 *
 * PUBLIC-SAFE BY CONSTRUCTION (SPEC §7 invariant 9): a bounded counter id,
 * a non-negative integer total, and an ISO timestamp. No per-user, account,
 * provider, model, prompt, or payment material can decode into this shape.
 *
 * This module is deliberately self-contained (imports only `effect`) so it
 * can be re-exported from ./index without a module cycle — same rule as
 * ./fleet.
 */

// ---------------------------------------------------------------------------
// Entity type name (changelog `entityType` value)
// ---------------------------------------------------------------------------

export const PUBLIC_COUNTER_ENTITY_TYPE = "public_counter"

/**
 * The tokens-served counter id — also the `<channel>` segment of its scope
 * (`scope.public.tokens-served`) and its `entityId` in the changelog.
 */
export const TOKENS_SERVED_COUNTER_ID = "tokens-served"

// ---------------------------------------------------------------------------
// Bounded field primitives
// ---------------------------------------------------------------------------

/** Bounded lowercase kebab-case counter id (`tokens-served`). */
export const PublicCounterId = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(64),
  S.isPattern(/^[a-z][a-z0-9-]*$/),
)

/** ISO-8601 UTC timestamp string (same shape the wire contracts use). */
export const PublicCounterIsoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
)

/**
 * The counter total: a non-negative safe integer. The Postgres column is
 * `bigint`, but the projected value must stay within JS safe-integer range
 * to ride JSON post-images honestly; the tokens-served total is orders of
 * magnitude below 2^53.
 */
export const PublicCounterTotal = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
)

// ---------------------------------------------------------------------------
// public_counter entity
// ---------------------------------------------------------------------------

/**
 * One public counter's post-image. `entityId` is the counter id (the
 * `<channel>` segment of the scope).
 */
export class PublicCounterEntity extends S.Class<PublicCounterEntity>(
  "PublicCounterEntity",
)({
  counterId: PublicCounterId,
  total: PublicCounterTotal,
  /** Observed-at of the newest source row applied; null before any event. */
  lastEventAt: S.NullOr(PublicCounterIsoTimestamp),
}) {}

// ---------------------------------------------------------------------------
// Boundary codecs
// ---------------------------------------------------------------------------

export const decodePublicCounterEntity = S.decodeUnknownSync(PublicCounterEntity)
export const encodePublicCounterEntity = S.encodeSync(PublicCounterEntity)
