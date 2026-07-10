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
export const FLEET_WORK_UNIT_ENTITY_TYPE = "fleet_work_unit"
export const FLEET_ATTEMPT_ENTITY_TYPE = "fleet_attempt"
export const FLEET_ACCOUNT_ENTITY_TYPE = "fleet_account"
export const FLEET_INBOX_FLAG_ENTITY_TYPE = "fleet_inbox_flag"
// MH-6 (#8585): the projected post-images the three MH-0 typed steering
// intents (`khala.fleet_intent.v1`) drive — a pending-approval card and a
// body-free steer receipt.
export const FLEET_APPROVAL_ENTITY_TYPE = "fleet_approval"
export const FLEET_STEER_ENTITY_TYPE = "fleet_steer"
export const FLEET_COMMAND_OUTCOME_ENTITY_TYPE = "fleet_command_outcome"

export const FLEET_ENTITY_TYPES = [
  FLEET_RUN_ENTITY_TYPE,
  FLEET_WORKER_ENTITY_TYPE,
  FLEET_ASSIGNMENT_ENTITY_TYPE,
  FLEET_WORK_UNIT_ENTITY_TYPE,
  FLEET_ATTEMPT_ENTITY_TYPE,
  FLEET_ACCOUNT_ENTITY_TYPE,
  FLEET_INBOX_FLAG_ENTITY_TYPE,
  FLEET_APPROVAL_ENTITY_TYPE,
  FLEET_STEER_ENTITY_TYPE,
  FLEET_COMMAND_OUTCOME_ENTITY_TYPE,
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

// Kept in lockstep with `@openagentsinc/khala-fleet-intents` `FleetWorkerKind`
// (MH-0 #8581). `grok` is additive: pre-multi-harness post-images that only
// ever carried codex/claude/auto still decode unchanged.
export const FleetWorkerKind = S.Literals(["codex", "claude", "grok", "auto"])
export type FleetWorkerKind = typeof FleetWorkerKind.Type

// A concrete coding harness that actually executes work (no `auto`), mirroring
// `FleetHarnessKind` in `@openagentsinc/khala-fleet-intents`. Rides on
// `fleet_worker` cards so the cockpit can show a per-harness badge (MH-6/MH-7).
export const FleetHarnessKind = S.Literals(["codex", "claude", "grok"])
export type FleetHarnessKind = typeof FleetHarnessKind.Type

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

/**
 * Worker slot lifecycle (mirrors the Pylon dispatch-context status set,
 * plus the operator-desired `paused` state written by `fleet.pauseWorker` —
 * KS-3.2 #8292; additive, so pre-#8292 post-images still decode).
 */
export const FleetWorkerPhase = S.Literals([
  "idle",
  "dispatched",
  "completed",
  "failed",
  "blocked",
  "circuit_broken",
  "paused",
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
  /**
   * Which concrete harness backs this worker (MH-6): the per-harness badge on
   * the cockpit's worker card. Optional so pre-multi-harness `fleet_worker`
   * post-images still decode (the account lane in `accountRefHash` was the
   * only harness signal before).
   */
  harnessKind: S.optionalKey(FleetHarnessKind),
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
// fleet_work_unit / fleet_attempt (FC-3 #8639)
// ---------------------------------------------------------------------------

const boundedPublicRefs = S.Array(FleetPublicRef).check(S.isMaxLength(64))
const boundedBlockerRefs = S.Array(FleetPublicRef).check(S.isMaxLength(32))
const nonEmptyEvidenceRefs = S.Array(FleetPublicRef).check(
  S.isMinLength(1),
  S.isMaxLength(64),
)

/**
 * A work unit is the stable plan identity. Attempts may be retried, but this
 * entity remains keyed by the original `unitRef` and points to the latest and
 * (only when fully proven) accepted attempt.
 */
export const FleetWorkUnitState = S.Literals([
  "planned",
  "running",
  "verification_pending",
  "succeeded",
  "failed",
  "stale",
])
export type FleetWorkUnitState = typeof FleetWorkUnitState.Type

const FleetWorkUnitEntityFields = {
  workUnitRef: FleetPublicRef,
  issueRef: S.NullOr(FleetIssueRef),
  dependsOnRefs: boundedPublicRefs,
  state: FleetWorkUnitState,
  latestAttemptRef: S.NullOr(FleetPublicRef),
  acceptedAttemptRef: S.NullOr(FleetPublicRef),
  updatedAt: FleetIsoTimestamp,
} as const

export const FleetWorkUnitEntity = S.Struct(FleetWorkUnitEntityFields).pipe(
  S.check(
    S.makeFilter(
      (entity) =>
        (entity.state === "planned"
          ? entity.latestAttemptRef === null &&
            entity.acceptedAttemptRef === null
          : entity.latestAttemptRef !== null) &&
        (entity.state === "succeeded"
          ? entity.acceptedAttemptRef === entity.latestAttemptRef
          : entity.acceptedAttemptRef === null),
      {
        message:
          "fleet work-unit state and attempt pointers must be coherent",
      },
    ),
  ),
)
export type FleetWorkUnitEntity = typeof FleetWorkUnitEntity.Type

export const FleetAttemptState = S.Literals([
  "running",
  "evidence_pending",
  "succeeded",
  "failed",
  "stale",
])
export type FleetAttemptState = typeof FleetAttemptState.Type

export const FleetAttemptProgressClass = S.Literals([
  "active",
  "blocked",
  "terminal",
])
export type FleetAttemptProgressClass = typeof FleetAttemptProgressClass.Type

export const FleetAttemptVerification = S.Union([
  S.Struct({ truth: S.Literal("pending") }),
  S.Struct({ truth: S.Literal("not_reported") }),
  S.Struct({
    truth: S.Literal("passed"),
    verifierRef: FleetPublicRef,
    evidenceRefs: nonEmptyEvidenceRefs,
  }),
  S.Struct({
    truth: S.Literal("failed"),
    verifierRef: S.NullOr(FleetPublicRef),
    evidenceRefs: boundedPublicRefs,
  }),
])
export type FleetAttemptVerification = typeof FleetAttemptVerification.Type

// Kept exactly in lockstep with
// `@openagentsinc/khala-fleet-intents` `MarginalCostClass`. Capacity custody
// (`owner_local`) is deliberately separate: owner-local subscriptions are not
// free, and absent economics must remain `not_measured`.
export const FleetAttemptMarginalCostClass = S.Literals([
  "free",
  "subscription",
  "api_metered",
  "not_measured",
])
export type FleetAttemptMarginalCostClass =
  typeof FleetAttemptMarginalCostClass.Type

const attemptUsageCount = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
  S.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)
const positiveAttemptUsageCount = S.Number.check(
  S.isInt(),
  S.isGreaterThan(0),
  S.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)
const nonEmptyAttemptUsageRefs = S.Array(FleetPublicRef).check(
  S.isMinLength(1),
  S.isMaxLength(100),
)

export const FleetAttemptExactUsageEvidence = S.Struct({
  schema: S.Literal("openagents.pylon.fleet_run_usage_evidence.v1"),
  truth: S.Literal("exact"),
  harnessKind: S.Literals(["codex", "claude"]),
  evidenceRef: FleetPublicRef,
  assignmentRef: FleetPublicRef,
  pylonRef: S.String.check(
    S.isMinLength(3),
    S.isMaxLength(120),
    S.isPattern(/^[a-z0-9][a-z0-9._:-]*$/),
  ),
  provider: S.Literals([
    "pylon-codex-own-capacity",
    "pylon-claude-own-capacity",
  ]),
  model: S.Literals(["openagents/pylon-codex", "openagents/pylon-claude"]),
  demandKind: S.Literal("own_capacity"),
  demandSource: S.Literal("khala_coding_delegation"),
  inputTokens: attemptUsageCount,
  outputTokens: attemptUsageCount,
  reasoningTokens: attemptUsageCount,
  cacheReadTokens: attemptUsageCount,
  totalTokens: positiveAttemptUsageCount,
  tokenRows: positiveAttemptUsageCount,
  tokenUsageRefs: nonEmptyAttemptUsageRefs,
  proofRefs: nonEmptyAttemptUsageRefs,
  closeoutChecklistRefs: nonEmptyAttemptUsageRefs,
  proofChecklistRefs: nonEmptyAttemptUsageRefs,
}).pipe(
  S.check(
    S.makeFilter(
      (usage) =>
        usage.totalTokens === usage.inputTokens + usage.outputTokens &&
        usage.reasoningTokens <= usage.outputTokens &&
        usage.cacheReadTokens <= usage.inputTokens &&
        usage.tokenUsageRefs.length >= Math.min(usage.tokenRows, 100) &&
        (usage.harnessKind === "codex"
          ? usage.provider === "pylon-codex-own-capacity" &&
            usage.model === "openagents/pylon-codex"
          : usage.provider === "pylon-claude-own-capacity" &&
            usage.model === "openagents/pylon-claude"),
      { message: "exact FleetRun usage evidence must be internally coherent" },
    ),
  ),
)
export type FleetAttemptExactUsageEvidence =
  typeof FleetAttemptExactUsageEvidence.Type

export const FleetAttemptNotMeasuredUsageEvidence = S.Struct({
  schema: S.Literal("openagents.pylon.fleet_run_usage_evidence.v1"),
  truth: S.Literal("not_measured"),
  harnessKind: S.Literal("grok"),
  evidenceRef: FleetPublicRef,
  assignmentRef: FleetPublicRef,
  receiptRef: FleetPublicRef,
  tokenUsageRefs: S.Array(FleetPublicRef).check(S.isMaxLength(0)),
  caveatRefs: nonEmptyAttemptUsageRefs,
})
export type FleetAttemptNotMeasuredUsageEvidence =
  typeof FleetAttemptNotMeasuredUsageEvidence.Type

export const FleetAttemptUsageEvidence = S.Union([
  S.Struct({
    truth: S.Literal("pending"),
  }),
  FleetAttemptExactUsageEvidence,
  FleetAttemptNotMeasuredUsageEvidence,
])
export type FleetAttemptUsageEvidence = typeof FleetAttemptUsageEvidence.Type

const FleetAttemptEntityFields = {
  /** The canonical attempt identity: exactly the Pylon `workClaimRef`. */
  attemptRef: FleetPublicRef,
  workUnitRef: FleetPublicRef,
  intakeClaimRef: S.String.check(
    S.isPattern(/^claim\.sarah_fleet_run\.[0-9a-f]{24}$/),
  ),
  pylonRef: S.String.check(
    S.isMinLength(3),
    S.isMaxLength(120),
    S.isPattern(/^[a-z0-9][a-z0-9._:-]*$/),
  ),
  workerKind: FleetHarnessKind,
  state: FleetAttemptState,
  progressClass: FleetAttemptProgressClass,
  /** Optional graph edge only. It is never the attempt identity. */
  assignmentRef: S.NullOr(FleetPublicRef),
  accountRefHash: S.NullOr(FleetAccountRefHash),
  capacityClass: S.Literal("owner_local"),
  marginalCostClass: FleetAttemptMarginalCostClass,
  verification: FleetAttemptVerification,
  artifactRefs: boundedPublicRefs,
  proofRefs: boundedPublicRefs,
  authorityReceiptRefs: boundedPublicRefs,
  closeoutRef: S.NullOr(FleetPublicRef),
  usageEvidence: FleetAttemptUsageEvidence,
  blockerRefs: boundedBlockerRefs,
  lastEventRef: S.String.check(
    S.isPattern(/^event\.pylon\.fleet_run\.[0-9a-f]{24}$/),
  ),
  startedAt: FleetIsoTimestamp,
  /** Server receipt clock used by freshness UI. */
  lastObservedAt: FleetIsoTimestamp,
  /** Pylon-reported clock retained for audit only, never freshness. */
  remoteObservedAt: FleetIsoTimestamp,
  terminalAt: S.NullOr(FleetIsoTimestamp),
  updatedAt: FleetIsoTimestamp,
} as const

export const FleetAttemptEntity = S.Struct(FleetAttemptEntityFields).pipe(
  S.check(
    S.makeFilter(
      (entity) => {
        if (
          entity.accountRefHash !== null &&
          !entity.accountRefHash.startsWith(
            `account.pylon.${
              entity.workerKind === "claude"
                ? "claude_agent"
                : entity.workerKind
            }.`,
          )
        ) {
          return false
        }
        if (entity.state === "running") {
          return entity.progressClass !== "terminal" &&
            entity.verification.truth === "pending" &&
            entity.closeoutRef === null &&
            entity.terminalAt === null &&
            entity.artifactRefs.length === 0 &&
            entity.proofRefs.length === 0 &&
            entity.authorityReceiptRefs.length === 0 &&
            entity.usageEvidence.truth === "pending"
        }
        if (entity.progressClass !== "terminal" || entity.terminalAt === null) {
          return false
        }
        if (entity.state === "evidence_pending") {
          return entity.verification.truth === "not_reported" &&
            entity.closeoutRef !== null &&
            entity.artifactRefs.length === 0 &&
            entity.proofRefs.length === 0 &&
            entity.authorityReceiptRefs.length === 0 &&
            entity.usageEvidence.truth === "pending" &&
            entity.blockerRefs.length === 0
        }
        if (entity.state === "succeeded") {
          return entity.verification.truth === "passed" &&
            entity.closeoutRef !== null &&
            entity.artifactRefs.length > 0 &&
            entity.proofRefs.length > 0 &&
            entity.authorityReceiptRefs.length > 0 &&
            entity.usageEvidence.truth !== "pending" &&
            entity.blockerRefs.length === 0
        }
        return entity.verification.truth !== "pending" &&
          entity.blockerRefs.length > 0
      },
      { message: "fleet attempt state and evidence must be coherent" },
    ),
  ),
)
export type FleetAttemptEntity = typeof FleetAttemptEntity.Type

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
 * existing public hash ref. `provider` and the `capacity*` slot counts are
 * bounded, non-identifying scalars (which CLI backs the account, how many
 * dispatch slots it has) — safe to sync alongside the hashed ref; they
 * carry no raw account ref, email, or local path.
 */
export class FleetAccountEntity extends S.Class<FleetAccountEntity>(
  "FleetAccountEntity",
)({
  accountRefHash: FleetAccountRefHash,
  readiness: FleetAccountReadiness,
  /** Rate-limit classification token (e.g. `five_hour_window`). */
  rateLimitClass: S.optionalKey(FleetClassToken),
  /** Which CLI backs this account (e.g. `codex`, `claude`). */
  provider: S.optionalKey(FleetClassToken),
  capacityAvailable: S.optionalKey(boundedCount),
  capacityBusy: S.optionalKey(boundedCount),
  capacityQueued: S.optionalKey(boundedCount),
  updatedAt: FleetIsoTimestamp,
}) {}

// ---------------------------------------------------------------------------
// fleet_inbox_flag
// ---------------------------------------------------------------------------

export const FleetInboxFlagStatus = S.Literals(["open", "acknowledged"])
export type FleetInboxFlagStatus = typeof FleetInboxFlagStatus.Type

/**
 * One inbox/attention flag on a fleet run (KS-3.2 #8292) — the synced
 * counterpart of the cockpit's attention items (run blocked, cooldown,
 * claim expired, …). `entityId` is `flagRef`. `kind` is a bounded
 * lower_snake_case classification token (e.g. `run_blocked`,
 * `cooldown_all_accounts`); flag PRODUCERS are a follow-up projection lane,
 * but `fleet.acknowledgeInboxFlag` acks ride this entity today so operator
 * acknowledgments are durable and converge across cockpit clients.
 */
export class FleetInboxFlagEntity extends S.Class<FleetInboxFlagEntity>(
  "FleetInboxFlagEntity",
)({
  flagRef: FleetPublicRef,
  kind: FleetClassToken,
  status: FleetInboxFlagStatus,
  openedAt: S.optionalKey(FleetIsoTimestamp),
  acknowledgedAt: S.optionalKey(FleetIsoTimestamp),
  updatedAt: FleetIsoTimestamp,
}) {}

// ---------------------------------------------------------------------------
// fleet_approval (MH-6 #8585)
// ---------------------------------------------------------------------------

/**
 * A pending tool/approval gate on a worker, and its resolution. The desktop
 * authority projects `pending` when a worker blocks on a tool that needs a
 * human allow/deny; the mobile `approval_decision` intent flips it to
 * `allowed`/`denied`. PUBLIC-SAFE: only refs and a bounded tool class token —
 * never the tool's arguments, prompt, or output.
 */
export const FleetApprovalStatus = S.Literals(["pending", "allowed", "denied"])
export type FleetApprovalStatus = typeof FleetApprovalStatus.Type

const FleetApprovalEntityFields = {
  approvalRef: FleetPublicRef,
  status: FleetApprovalStatus,
  /** Exact run/attempt binding. All fields in this group are present together. */
  runRef: S.optionalKey(FleetPublicRef),
  workUnitRef: S.optionalKey(FleetPublicRef),
  attemptRef: S.optionalKey(FleetPublicRef),
  assignmentRef: S.optionalKey(S.NullOr(FleetPublicRef)),
  /** The worker slot blocked on this approval. */
  workerId: S.optionalKey(FleetPublicRef),
  accountRefHash: S.optionalKey(S.NullOr(FleetAccountRefHash)),
  requestEventRef: S.optionalKey(FleetPublicRef),
  /** Bounded tool classification (e.g. `bash`, `write_file`); never args. */
  toolClass: S.optionalKey(FleetClassToken),
  openedAt: S.optionalKey(FleetIsoTimestamp),
  /** When the allow/deny decision landed. */
  decidedAt: S.optionalKey(FleetIsoTimestamp),
  updatedAt: FleetIsoTimestamp,
} as const

/**
 * Legacy approval post-images remain decodable without exact binding fields.
 * Once any new binding field is present, every binding edge and the original
 * public-safe request metadata must be present. Nullable edges are explicit
 * keys, preventing absence from being confused with a known-null value.
 */
export const FleetApprovalEntity = S.Struct(FleetApprovalEntityFields).pipe(
  S.check(
    S.makeFilter(
      (entity) => {
        const bindingKeys = [
          "runRef",
          "workUnitRef",
          "attemptRef",
          "assignmentRef",
          "accountRefHash",
          "requestEventRef",
        ] as const
        const hasAnyBinding = bindingKeys.some((key) => key in entity)
        if (!hasAnyBinding) return true
        return (
          bindingKeys.every((key) => key in entity) &&
          "workerId" in entity &&
          "toolClass" in entity &&
          "openedAt" in entity
        )
      },
      { message: "fleet approval exact binding must be complete" },
    ),
  ),
)
export type FleetApprovalEntity = typeof FleetApprovalEntity.Type
export type FleetBoundApprovalEntity = FleetApprovalEntity &
  Readonly<{
    runRef: string
    workUnitRef: string
    attemptRef: string
    assignmentRef: string | null
    workerId: string
    accountRefHash: string | null
    requestEventRef: string
    toolClass: string
    openedAt: string
  }>

export const fleetApprovalHasExactBinding = (
  entity: FleetApprovalEntity,
): entity is FleetBoundApprovalEntity => "runRef" in entity

// ---------------------------------------------------------------------------
// fleet_steer (MH-6 #8585)
// ---------------------------------------------------------------------------

/**
 * A body-free receipt that a steer message was dispatched at an in-flight
 * worker/turn. The message body itself is NEVER projected here (it can carry
 * arbitrary text); only the steer's ref, an optional target worker/turn ref,
 * and whether a body was carried inline vs. by opaque ref ride in the
 * post-image, so the cockpit can show "steer delivered" without leaking text.
 */
export class FleetSteerEntity extends S.Class<FleetSteerEntity>(
  "FleetSteerEntity",
)({
  steerRef: FleetPublicRef,
  /** The worker/turn the steer targeted, when the intent named one. */
  targetRef: S.optionalKey(FleetPublicRef),
  /** How the body travelled: `inline` (public-unsafe, stored elsewhere) or `ref`. */
  bodyCarrier: S.Literals(["inline", "ref", "none"]),
  createdAt: FleetIsoTimestamp,
  updatedAt: FleetIsoTimestamp,
}) {}

// ---------------------------------------------------------------------------
// fleet_command_outcome (FC-3 #8639)
// ---------------------------------------------------------------------------

/**
 * The Pylon delivery disposition for one typed Sarah fleet command. This is
 * deliberately distinct from the command's effective result: a command can
 * be accepted for local follow-up without yet changing the fleet projection.
 */
export const FleetCommandDeliveryOutcome = S.Literals([
  "applied",
  "queued_follow_up",
  "skipped_stale",
  "rejected",
  "failed",
])
export type FleetCommandDeliveryOutcome =
  typeof FleetCommandDeliveryOutcome.Type

export const FleetCommandKind = S.Literals([
  "fleet_run_control",
  "approval_decision",
  "steer_message",
  "worker_selection",
])
export type FleetCommandKind = typeof FleetCommandKind.Type

/**
 * An exact effective state that this Sync transaction projected. `null` means
 * no effective state was claimed (including queued, rejected, failed, and
 * stale deliveries). Steering and worker-selection commands cannot claim an
 * effective result until their executor posts a terminal, state-bound ACK.
 */
export const FleetCommandEffectiveOutcome = S.Literals([
  "running",
  "paused",
  "draining",
  "stopped",
  "allowed",
  "denied",
  "steer_delivered",
])
export type FleetCommandEffectiveOutcome =
  typeof FleetCommandEffectiveOutcome.Type

export const FleetCommandCompletionOutcome = S.Literals([
  "applied",
  "skipped_stale",
  "rejected",
  "failed",
])
export type FleetCommandCompletionOutcome =
  typeof FleetCommandCompletionOutcome.Type

/**
 * Body-free, reconnect-safe command receipt. It contains no prompt, steer
 * body, reason text, session handle, local path, or raw account identity.
 * `entityId` is `intentId`, giving retries one stable upsert identity.
 *
 * An immediately applied run/approval command uses its content-bound
 * `outcomeRef` as `completionRef`. A command first recorded as
 * `queued_follow_up` keeps that delivery disposition and original outcome ref
 * when an executor later completes it; that later upsert fills
 * `effectiveOutcome`, `completedAt`, and a distinct content-bound
 * `completion.pylon.fleet_steering.*` receipt. Delivery history is therefore
 * never rewritten to imply that a queued command was immediate.
 */
export const FleetCommandCompletionRef = S.String.check(
  S.isPattern(
    /^(outcome|completion)\.pylon\.fleet_steering\.[a-f0-9]{24}$/,
  ),
)
export type FleetCommandCompletionRef = typeof FleetCommandCompletionRef.Type

export const FleetCommandOutcomeRef = S.String.check(
  S.isPattern(/^outcome\.pylon\.fleet_steering\.[a-f0-9]{24}$/),
)
export type FleetCommandOutcomeRef = typeof FleetCommandOutcomeRef.Type

const FleetCommandOutcomeEntityFields = {
  intentId: FleetPublicRef,
  seq: S.Number.check(
    S.isInt(),
    S.isGreaterThanOrEqualTo(1),
    S.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
  ),
  kind: FleetCommandKind,
  /** A public-safe target ref only; unsafe/absent optional targets become null. */
  targetRef: S.NullOr(FleetPublicRef),
  deliveryOutcome: FleetCommandDeliveryOutcome,
  completionOutcome: S.NullOr(FleetCommandCompletionOutcome),
  effectiveOutcome: S.NullOr(FleetCommandEffectiveOutcome),
  completionRef: S.NullOr(FleetCommandCompletionRef),
  completedAt: S.NullOr(FleetIsoTimestamp),
  outcomeRef: FleetCommandOutcomeRef,
  /** Pylon's observation clock. */
  observedAt: FleetIsoTimestamp,
  /** Authoritative server receipt clock. */
  recordedAt: FleetIsoTimestamp,
  updatedAt: FleetIsoTimestamp,
} as const

const fleetCommandEffectiveMatchesKind = (
  kind: FleetCommandKind,
  effective: FleetCommandEffectiveOutcome,
): boolean =>
  kind === "fleet_run_control"
    ? effective === "running" ||
      effective === "paused" ||
      effective === "draining" ||
      effective === "stopped"
    : kind === "approval_decision"
      ? effective === "allowed" || effective === "denied"
      : kind === "steer_message"
        ? effective === "steer_delivered"
        : false

export const FleetCommandOutcomeEntity = S.Struct(
  FleetCommandOutcomeEntityFields,
).pipe(
  S.check(
    S.makeFilter(
      (entity) => {
        const hasCompletionOutcome = entity.completionOutcome !== null
        const hasCompletionRef = entity.completionRef !== null
        const hasCompletedAt = entity.completedAt !== null
        const completionTupleCoherent =
          hasCompletionOutcome === hasCompletionRef &&
          hasCompletionRef === hasCompletedAt
        if (!completionTupleCoherent) return false

        if (entity.deliveryOutcome === "applied") {
          return entity.completionOutcome === "applied" &&
            entity.completionRef === entity.outcomeRef &&
            entity.effectiveOutcome !== null &&
            fleetCommandEffectiveMatchesKind(
              entity.kind,
              entity.effectiveOutcome,
            )
        }

        if (entity.deliveryOutcome === "queued_follow_up") {
          if (entity.completionOutcome === null) {
            return entity.effectiveOutcome === null
          }
          if (entity.completionOutcome === "applied") {
            return entity.completionRef?.startsWith(
              "completion.pylon.fleet_steering.",
            ) === true &&
              entity.effectiveOutcome !== null &&
              fleetCommandEffectiveMatchesKind(
                entity.kind,
                entity.effectiveOutcome,
              )
          }
          return entity.completionRef?.startsWith(
            "completion.pylon.fleet_steering.",
          ) === true && entity.effectiveOutcome === null
        }

        return entity.completionOutcome === null &&
          entity.effectiveOutcome === null
      },
      {
        message:
          "fleet command delivery, completion, and effective outcomes must be coherent",
      },
    ),
  ),
)
export type FleetCommandOutcomeEntity =
  typeof FleetCommandOutcomeEntity.Type

// ---------------------------------------------------------------------------
// Fleet operator intents (KS-3.2 #8292)
// ---------------------------------------------------------------------------

/**
 * Durable operator intent vocabulary — kept in lockstep with the
 * `khala_sync_fleet_intents` CHECK constraint (khala-sync-server
 * migrations 0004/0005) and the fleet mutator set.
 */
export const FleetIntentKind = S.Literals([
  "set_desired_slots",
  "pause",
  "resume",
  "pause_worker",
  "resume_worker",
  "acknowledge_inbox_flag",
  "stop",
])
export type FleetIntentKind = typeof FleetIntentKind.Type

/**
 * One durable operator intent row as served by the intent-consumption
 * seam (`readPendingFleetIntents` in `@openagentsinc/khala-sync-server`
 * and the Worker's admin-guarded
 * `GET /api/internal/khala-sync/fleet-intents` route). NOT a sync-protocol
 * message — it is the polling contract for enforcement loops (the Pylon
 * supervisor), shared here so producer and consumer decode one schema.
 * `id` is the monotonic identity column and the poller's resume watermark.
 */
export class FleetIntentRow extends S.Class<FleetIntentRow>("FleetIntentRow")({
  id: S.Number,
  scope: S.String,
  runId: S.String,
  intent: FleetIntentKind,
  desiredSlots: S.NullOr(S.Number),
  workerId: S.NullOr(S.String),
  flagRef: S.NullOr(S.String),
  requestedByUserId: S.String,
  mutationRef: S.String,
  createdAt: S.String,
}) {}

export const decodeFleetIntentRow = S.decodeUnknownSync(FleetIntentRow)
export const encodeFleetIntentRow = S.encodeSync(FleetIntentRow)

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
export const decodeFleetWorkUnitEntity = (input: unknown) =>
  S.decodeUnknownSync(FleetWorkUnitEntity)(input, {
    onExcessProperty: "error",
  })
export const encodeFleetWorkUnitEntity = S.encodeSync(FleetWorkUnitEntity)
export const decodeFleetAttemptEntity = (input: unknown) =>
  S.decodeUnknownSync(FleetAttemptEntity)(input, {
    onExcessProperty: "error",
  })
export const encodeFleetAttemptEntity = S.encodeSync(FleetAttemptEntity)
export const decodeFleetAccountEntity = S.decodeUnknownSync(FleetAccountEntity)
export const encodeFleetAccountEntity = S.encodeSync(FleetAccountEntity)
export const decodeFleetInboxFlagEntity = S.decodeUnknownSync(
  FleetInboxFlagEntity,
)
export const encodeFleetInboxFlagEntity = S.encodeSync(FleetInboxFlagEntity)
export const decodeFleetApprovalEntity = (input: unknown) =>
  S.decodeUnknownSync(FleetApprovalEntity)(input, {
    onExcessProperty: "error",
  })
export const encodeFleetApprovalEntity = S.encodeSync(FleetApprovalEntity)
export const decodeFleetSteerEntity = S.decodeUnknownSync(FleetSteerEntity)
export const encodeFleetSteerEntity = S.encodeSync(FleetSteerEntity)
export const decodeFleetCommandOutcomeEntity = S.decodeUnknownSync(
  FleetCommandOutcomeEntity,
)
export const encodeFleetCommandOutcomeEntity = S.encodeSync(
  FleetCommandOutcomeEntity,
)
