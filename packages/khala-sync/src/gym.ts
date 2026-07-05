import { Schema as S } from "effect"

/**
 * Gym / Harbor live run-progress public entity contract (KS-6.5, #8415;
 * SPEC §2.1 `scope.public.<channel>`, §7 invariant 8/9).
 *
 * This is the khala-sync mirror of the Worker's ALREADY public-safe
 * `openagents.gym.run_progress.v1` projection
 * (`GymRunProgressPublicProjection` in
 * `apps/openagents.com/workers/api/src/inference/gym/run-progress.ts`): the
 * `/gym` follow-along panel's live per-run progress card. A `web_authorized`
 * run publishes counts/denominators/timings/public-safe serving-profile
 * refs; a `local_only` run degrades to an honest "awaiting authorization"
 * marker with no live numbers — never invented data.
 *
 * MANY ENTITIES, ONE SCOPE: unlike the single-row tokens-served counter
 * (`./public-counter.ts`), a Gym feed tracks MANY concurrently-running
 * benchmark jobs. All runs ride ONE shared public scope
 * (`scope.public.gym-run-progress`, mirroring the legacy sync-worker room's
 * fixed `network` feed id), keyed by `entityId = runRef` — same
 * multi-entity-per-scope shape as `scope.fleet_run.<id>` (./fleet.ts), minus
 * the owner-gating (this scope is PUBLIC: every authenticated caller reads
 * it per `resolveScopeRead`'s `public` arm — see khala-sync-server's
 * scope-auth.ts).
 *
 * PUBLIC-SAFE BY CONSTRUCTION (SPEC §7 invariant 9): every field is either a
 * closed literal set, a bounded public-safe ref, a bounded label string, or
 * an ISO timestamp. No prompt, response, log, trajectory, key, or private
 * endpoint material can decode into this shape — mirroring the Worker's own
 * `checkGymRunProgressPublicSafety` tripwire, which the projector re-checks
 * as defense in depth (`GYM_RUN_PROGRESS_POST_IMAGE_FORBIDDEN_PATTERN` in
 * `@openagentsinc/khala-sync-server`).
 *
 * This module is deliberately self-contained (imports only `effect`) so it
 * can be re-exported from ./index without a module cycle — same rule as
 * ./fleet and ./public-counter.
 */

// ---------------------------------------------------------------------------
// Entity type name (changelog `entityType` value)
// ---------------------------------------------------------------------------

export const GYM_RUN_PROGRESS_ENTITY_TYPE = "gym_run_progress"

/** The shared public channel all Gym runs ride (the `<channel>` in `scope.public.<channel>`). */
export const GYM_RUN_PROGRESS_PUBLIC_CHANNEL = "gym-run-progress"

// ---------------------------------------------------------------------------
// Bounded public-safe field primitives
// ---------------------------------------------------------------------------

/**
 * A public-safe structured ref: dot/colon/dash-separated identifier
 * segments. Excludes `@`, `/`, and whitespace by construction (same shape
 * as `FleetPublicRef`), so emails, filesystem paths, and URLs cannot decode
 * into ref-typed fields.
 */
export const GymPublicRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)

/**
 * A public-safe model ref, which may carry ONE `namespace/name` segment
 * (e.g. `openagents/khala`, `openagents/glm-5.2-reap-504b` — the Harbor
 * Terminal-Bench model id vocabulary). At most one slash; still excludes
 * `@`, whitespace, and any additional path depth.
 */
export const GymModelRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._-]*(\/[A-Za-z0-9][A-Za-z0-9._-]*)?$/),
)

/** A bounded free-text public label (attribution / display strings). */
export const GymPublicLabel = S.String.check(
  S.isMaxLength(256),
)

/** ISO-8601 UTC timestamp string (same shape the wire contracts use). */
export const GymIsoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
)

const boundedCount = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
  S.isLessThanOrEqualTo(1_000_000),
)

const maybeBoundedNumber = S.NullOr(
  S.Number.check(S.isGreaterThanOrEqualTo(0)),
)

const unitFraction = S.Number.check(
  S.isGreaterThanOrEqualTo(0),
  S.isLessThanOrEqualTo(1),
)

const maybeUnitFraction = S.NullOr(unitFraction)

export const GymRunPhase = S.Literals([
  "queued",
  "running",
  "completed",
  "cancelled",
  "errored",
])
export type GymRunPhase = typeof GymRunPhase.Type

// ---------------------------------------------------------------------------
// gym_run_progress — published (`web_authorized`) shape
// ---------------------------------------------------------------------------

export class GymRunProgressCountsEntity extends S.Class<GymRunProgressCountsEntity>(
  "GymRunProgressCountsEntity",
)({
  officialDenominator: boundedCount,
  completed: boundedCount,
  completedPassed: boundedCount,
  completedFailed: boundedCount,
  running: boundedCount,
  pending: boundedCount,
  error: boundedCount,
  cancelled: boundedCount,
}) {}

export class GymRunProgressTokensEntity extends S.Class<GymRunProgressTokensEntity>(
  "GymRunProgressTokensEntity",
)({
  promptTokens: maybeBoundedNumber,
  completionTokens: maybeBoundedNumber,
  totalTokens: maybeBoundedNumber,
}) {}

export class GymRunProgressProfileEntity extends S.Class<GymRunProgressProfileEntity>(
  "GymRunProgressProfileEntity",
)({
  profileRef: GymPublicRef,
  publicLabel: GymPublicLabel,
  model: GymModelRef,
  attribution: GymPublicLabel,
  hardwareProfile: GymPublicRef,
  contextWindowTokens: boundedCount,
}) {}

/**
 * One `web_authorized` run's live progress — the full public projection
 * (counts, pass-rate over completed, token counts, freshness).
 */
export class GymRunProgressPublishedEntity extends S.Class<GymRunProgressPublishedEntity>(
  "GymRunProgressPublishedEntity",
)({
  runRef: GymPublicRef,
  jobRef: GymPublicRef,
  configId: GymPublicRef,
  agent: GymPublicRef,
  profile: GymRunProgressProfileEntity,
  phase: GymRunPhase,
  publication: S.Literal("web_authorized"),
  decisionGrade: S.Literal(false),
  inProgress: S.Boolean,
  counts: GymRunProgressCountsEntity,
  passRateOverCompleted: maybeUnitFraction,
  completionFraction: unitFraction,
  tokens: GymRunProgressTokensEntity,
  elapsedMs: maybeBoundedNumber,
  lastUpdatedAt: GymIsoTimestamp,
  caveatRefs: S.Array(GymPublicRef),
  blockerRefs: S.Array(GymPublicRef),
}) {}

// ---------------------------------------------------------------------------
// gym_run_progress — degraded (`local_only`) shape
// ---------------------------------------------------------------------------

/**
 * A `local_only` run's honest "awaiting web authorization" marker. NO live
 * counts — a partial/unauthorized run must never publish invented numbers.
 */
export class GymRunProgressUnpublishedEntity extends S.Class<GymRunProgressUnpublishedEntity>(
  "GymRunProgressUnpublishedEntity",
)({
  runRef: GymPublicRef,
  publication: S.Literal("local_only"),
  decisionGrade: S.Literal(false),
  inProgress: S.Boolean,
  blockerRefs: S.Array(GymPublicRef),
  lastUpdatedAt: GymIsoTimestamp,
}) {}

// ---------------------------------------------------------------------------
// Union + boundary codecs
// ---------------------------------------------------------------------------

export const GymRunProgressEntity = S.Union([
  GymRunProgressPublishedEntity,
  GymRunProgressUnpublishedEntity,
])
export type GymRunProgressEntity = typeof GymRunProgressEntity.Type

export const decodeGymRunProgressEntity = S.decodeUnknownSync(
  GymRunProgressEntity,
)
export const encodeGymRunProgressEntity = S.encodeSync(GymRunProgressEntity)
