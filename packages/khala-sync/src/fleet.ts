import { Schema as S } from "effect"

/**
 * Fleet cockpit entity contracts (KS-6.1; SPEC §2.1 `scope.fleet_run.<id>`).
 *
 * These are the entity post-image shapes that ride inside
 * `ChangelogEntry.postImageJson` for fleet-run scopes — the server-side
 * projection of fleet run / worker / assignment / account state that the
 * Khala Code cockpit consumes.
 *
 * PUBLIC-SAFE BY CONSTRUCTION (SPEC §7 invariant 9): every field is either a
 * closed literal set, a bounded public-safe ref, or an ISO timestamp. No
 * tokens, prompts, local paths, emails, or raw account identity may appear
 * here; account identity rides ONLY as the existing public hash refs
 * (`account.<lane>.<hex-digest>`). The ref patterns below structurally
 * exclude `@` (emails), `/` (filesystem paths), and whitespace, so a raw
 * secret cannot even decode into these shapes. The projector in
 * `@openagentsinc/khala-sync-server` additionally allowlist-maps raw rows
 * into these schemas (never spreads) before anything is serialized.
 *
 * This module is deliberately self-contained (imports only `effect`) so it
 * can be re-exported from ./index without a module cycle. Entity-type names
 * are exported as plain strings; brand them with `EntityType.make(...)` at
 * append call sites.
 */

// ---------------------------------------------------------------------------
// Entity type names (changelog `entityType` values)
// ---------------------------------------------------------------------------

export const FLEET_RUN_ENTITY_TYPE = "fleet_run"
export const FLEET_WORKER_ENTITY_TYPE = "fleet_worker"
export const FLEET_ASSIGNMENT_ENTITY_TYPE = "fleet_assignment"
export const FLEET_ACCOUNT_ENTITY_TYPE = "fleet_account"

export const FLEET_ENTITY_TYPES = [
  FLEET_RUN_ENTITY_TYPE,
  FLEET_WORKER_ENTITY_TYPE,
  FLEET_ASSIGNMENT_ENTITY_TYPE,
  FLEET_ACCOUNT_ENTITY_TYPE,
] as const

// ---------------------------------------------------------------------------
// Bounded public-safe field primitives
// ---------------------------------------------------------------------------

/**
 * A public-safe structured ref: dot/colon/dash-separated identifier
 * segments. Excludes `@`, `/`, and whitespace by construction, so emails,
 * filesystem paths, and URLs cannot decode into ref-typed fields.
 */
export const FleetPublicRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)

/**
 * A public-safe hashed account ref (`account.<lane...>.<hex-digest>`), the
 * ONLY permitted form of account identity in fleet post-images — matching
 * the existing Pylon convention (e.g. `account.pylon.codex.<24-hex>`).
 * Never a raw account ref, email, or home path.
 */
export const FleetAccountRefHash = S.String.check(
  S.isMaxLength(256),
  S.isPattern(/^account\.[a-z][a-z0-9_.-]*\.[0-9a-f]{8,64}$/),
)

/** ISO-8601 UTC timestamp string (same shape the wire contracts use). */
export const FleetIsoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
)

/**
 * A bounded lower_snake_case classification token (statuses that evolve
 * with the source system, closeout classes, rate-limit classes).
 */
export const FleetClassToken = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(64),
  S.isPattern(/^[a-z][a-z0-9_]*$/),
)

/**
 * A public-safe issue ref (`#8302`, `OpenAgentsInc/openagents#8302`). The
 * single-slash owner/repo form is allowed; `@`, whitespace, and `://` are
 * structurally excluded.
 */
export const FleetIssueRef = S.String.check(
  S.isMaxLength(128),
  S.isPattern(/^([A-Za-z0-9][A-Za-z0-9._-]*\/[A-Za-z0-9][A-Za-z0-9._-]*)?#\d+$/),
)

const boundedCount = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
  S.isLessThanOrEqualTo(1_000_000),
)

// ---------------------------------------------------------------------------
// fleet_run
// ---------------------------------------------------------------------------

/** Fleet run lifecycle (mirrors the Pylon orchestration store's state set). */
export const FleetRunStatus = S.Literals([
  "draft",
  "running",
  "paused",
  "draining",
  "stopped",
  "completed",
])
export type FleetRunStatus = typeof FleetRunStatus.Type

export const FleetWorkerKind = S.Literals(["codex", "claude", "auto"])
export type FleetWorkerKind = typeof FleetWorkerKind.Type

export class FleetRunCounters extends S.Class<FleetRunCounters>(
  "FleetRunCounters",
)({
  workUnitsTotal: boundedCount,
  activeAssignments: boundedCount,
  completedAssignments: boundedCount,
  failedAssignments: boundedCount,
  blockedAssignments: boundedCount,
}) {}

/**
 * One fleet run — the cockpit's root entity. `entityId` is the run id (the
 * `<fleetRunId>` segment of the scope).
 */
export class FleetRunEntity extends S.Class<FleetRunEntity>("FleetRunEntity")({
  runId: FleetPublicRef,
  status: FleetRunStatus,
  /** Operator-desired concurrent worker slots (0 = fully drained). */
  desiredSlots: S.Number.check(
    S.isInt(),
    S.isGreaterThanOrEqualTo(0),
    S.isLessThanOrEqualTo(1024),
  ),
  workerKind: FleetWorkerKind,
  startedAt: S.NullOr(FleetIsoTimestamp),
  counters: FleetRunCounters,
  updatedAt: FleetIsoTimestamp,
}) {}

// ---------------------------------------------------------------------------
// fleet_worker
// ---------------------------------------------------------------------------

/** Worker slot lifecycle (mirrors the Pylon dispatch-context status set). */
export const FleetWorkerPhase = S.Literals([
  "idle",
  "dispatched",
  "completed",
  "failed",
  "blocked",
  "circuit_broken",
])
export type FleetWorkerPhase = typeof FleetWorkerPhase.Type

/**
 * One worker slot inside a fleet run. `workerId` is the public-safe
 * dispatch-context ref; account identity is the hashed ref ONLY.
 */
export class FleetWorkerEntity extends S.Class<FleetWorkerEntity>(
  "FleetWorkerEntity",
)({
  workerId: FleetPublicRef,
  phase: FleetWorkerPhase,
  assignmentRef: S.optionalKey(FleetPublicRef),
  accountRefHash: S.optionalKey(FleetAccountRefHash),
  lastProgressAt: S.optionalKey(FleetIsoTimestamp),
  updatedAt: FleetIsoTimestamp,
}) {}

// ---------------------------------------------------------------------------
// fleet_assignment
// ---------------------------------------------------------------------------

/**
 * One assignment as seen by the cockpit — the PUBLIC-SAFE slice of the
 * Worker's assignment record: refs, lifecycle status, and closeout class.
 * Deliberately NOT the full assignment projection (no lease internals, no
 * coding-assignment payload — that JSON can carry prompts and workspace
 * paths and must never reach a fleet post-image).
 */
export class FleetAssignmentEntity extends S.Class<FleetAssignmentEntity>(
  "FleetAssignmentEntity",
)({
  assignmentRef: FleetPublicRef,
  issueRef: S.optionalKey(FleetIssueRef),
  /**
   * Assignment lifecycle status as a bounded token (the source state set
   * evolves with the Pylon API; e.g. `offered`, `accepted`, `running`,
   * `closeout_submitted`, `accepted_work`, `rejected`, `stale`).
   */
  status: FleetClassToken,
  /** Terminal classification once closed (e.g. `accepted_work`, `rejected`). */
  closeoutClass: S.optionalKey(FleetClassToken),
  updatedAt: FleetIsoTimestamp,
}) {}

// ---------------------------------------------------------------------------
// fleet_account
// ---------------------------------------------------------------------------

export const FleetAccountReadiness = S.Literals([
  "ready",
  "cooldown",
  "unavailable",
  "unknown",
])
export type FleetAccountReadiness = typeof FleetAccountReadiness.Type

/**
 * One provider account's dispatch readiness, identified ONLY by its
 * existing public hash ref.
 */
export class FleetAccountEntity extends S.Class<FleetAccountEntity>(
  "FleetAccountEntity",
)({
  accountRefHash: FleetAccountRefHash,
  readiness: FleetAccountReadiness,
  /** Rate-limit classification token (e.g. `five_hour_window`). */
  rateLimitClass: S.optionalKey(FleetClassToken),
  updatedAt: FleetIsoTimestamp,
}) {}

// ---------------------------------------------------------------------------
// Boundary codecs
// ---------------------------------------------------------------------------

export const decodeFleetRunEntity = S.decodeUnknownSync(FleetRunEntity)
export const encodeFleetRunEntity = S.encodeSync(FleetRunEntity)
export const decodeFleetWorkerEntity = S.decodeUnknownSync(FleetWorkerEntity)
export const encodeFleetWorkerEntity = S.encodeSync(FleetWorkerEntity)
export const decodeFleetAssignmentEntity = S.decodeUnknownSync(
  FleetAssignmentEntity,
)
export const encodeFleetAssignmentEntity = S.encodeSync(FleetAssignmentEntity)
export const decodeFleetAccountEntity = S.decodeUnknownSync(FleetAccountEntity)
export const encodeFleetAccountEntity = S.encodeSync(FleetAccountEntity)
