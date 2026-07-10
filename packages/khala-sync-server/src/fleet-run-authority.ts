import {
  canonicalJson,
  decodeFleetApprovalEntity,
  decodeFleetAttemptEntity,
  decodeFleetWorkUnitEntity,
  FleetAttemptExactUsageEvidence,
  FleetAttemptMarginalCostClass,
  FleetAttemptNotMeasuredUsageEvidence,
  fleetRunScope,
  type FleetAttemptEntity,
  type FleetApprovalEntity,
  type FleetWorkUnitEntity,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import {
  FleetWorkerKind as FleetWorkerKindSchema,
  type FleetWorkerKind,
} from "@openagentsinc/khala-fleet-intents"
import { Clock, Effect, Layer, Schema as S } from "effect"
import * as Context from "effect/Context"

import {
  appendFleetEntityChange,
  ensureScopeOwner,
  fleetRunPostImage,
} from "./fleet-projection.js"
import { withSyncTransaction } from "./outbox-writer.js"
import type { SqlTag, SyncSql } from "./sql.js"

export const FLEET_RUN_AUTHORITY_REQUEST_SCHEMA =
  "sarah.coding_fleet_start.request.v1" as const
export const FLEET_RUN_AUTHORITY_RECORD_SCHEMA =
  "openagents.sarah.fleet_run_authority.v1" as const
export const FLEET_RUN_INTAKE_CLAIM_SCHEMA =
  "openagents.sarah.fleet_run_intake_claim.v1" as const
export const FLEET_RUN_EXECUTION_BATCH_SCHEMA =
  "openagents.pylon.fleet_run_execution_batch.v1" as const
export const FLEET_RUN_EXECUTION_EVENT_SCHEMA =
  "openagents.pylon.fleet_run_execution_event.v1" as const
export const FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2 =
  "openagents.pylon.fleet_run_execution_batch.v2" as const
export const FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2 =
  "openagents.pylon.fleet_run_execution_event.v2" as const
export const FLEET_RUN_EXECUTION_ACK_SCHEMA =
  "openagents.pylon.fleet_run_execution_ack.v1" as const
export const FLEET_RUN_AUTHORITY_CREATE_MUTATION_REF =
  "system:sarah_fleet_run_authority.create.v1" as const
export const FLEET_RUN_AUTHORITY_EXECUTION_MUTATION_REF =
  "system:sarah_fleet_run_authority.execution.v1" as const
export const FLEET_RUN_PYLON_FRESHNESS_MS = 5 * 60 * 1_000
export const FLEET_RUN_EXECUTION_MAX_FUTURE_SKEW_MS = 5 * 60 * 1_000

const PublicOwnerRef = S.Trim.check(
  S.isMinLength(3),
  S.isMaxLength(160),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
)
const PublicPylonRef = S.Trim.check(
  S.isMinLength(3),
  S.isMaxLength(120),
  S.isPattern(/^[a-z0-9][a-z0-9._:-]*$/u),
)
const IdempotencyKey = S.Trim.check(
  S.isMinLength(8),
  S.isMaxLength(120),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
)
const RepositorySlug = S.Trim.check(
  S.isMinLength(1),
  S.isMaxLength(120),
  S.isPattern(/^[A-Za-z0-9_.-]+$/u),
)
const RepositoryBranch = S.Trim.check(
  S.isMinLength(1),
  S.isMaxLength(120),
  S.isPattern(/^[A-Za-z0-9._/-]+$/u),
)
const RepositoryCommit = S.Trim.check(S.isPattern(/^[0-9a-fA-F]{40}$/u))
const Objective = S.Trim.check(S.isMinLength(8), S.isMaxLength(1_000))
const VerifierCommand = S.Trim.check(S.isMinLength(3), S.isMaxLength(240))
const PublicRef = S.Trim.check(
  S.isMinLength(1),
  S.isMaxLength(160),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:/#-]*$/u),
)
const PlanUnitRef = S.Trim.check(
  S.isMinLength(1),
  S.isMaxLength(160),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
)
const PlanUnitTitle = S.Trim.check(S.isMinLength(1), S.isMaxLength(160))
const RunRef = S.String.check(S.isPattern(/^fleet_run\.sarah\.[0-9a-f]{20}$/u))
const TrimmedRunRef = S.Trim.check(
  S.isPattern(/^fleet_run\.sarah\.[0-9a-f]{20}$/u),
)
const ClaimRef = S.String.check(
  S.isPattern(/^claim\.sarah_fleet_run\.[0-9a-f]{24}$/u),
)
const TrimmedClaimRef = S.Trim.check(
  S.isPattern(/^claim\.sarah_fleet_run\.[0-9a-f]{24}$/u),
)
const FleetRunScope = S.String.check(
  S.isPattern(/^scope\.fleet_run\.fleet_run\.sarah\.[0-9a-f]{20}$/u),
)
const IsoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/u),
)
const ExecutionEventRef = S.String.check(
  S.isPattern(/^event\.pylon\.fleet_run\.[0-9a-f]{24}$/u),
)
const ExecutionSequence = S.Int.check(
  S.isGreaterThanOrEqualTo(1),
  S.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)
const ExecutionPublicRef = S.String.check(
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,179}$/u),
)
const ProjectedExecutionPublicRef = S.String.check(
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]{0,179}$/u),
)
const AccountRefHash = S.String.check(
  S.isPattern(
    /^account\.pylon\.(?:codex|claude_agent|grok)\.[a-f0-9]{24}$/u,
  ),
)
const ApprovalToolClass = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(64),
  S.isPattern(/^[a-z][a-z0-9_]*$/u),
)
const BlockerRef = S.String.check(
  S.isPattern(/^blocker\.[A-Za-z0-9][A-Za-z0-9._:-]{0,171}$/u),
)
const LegacyBlockerRef = S.String.check(
  S.isPattern(/^blocker\.[A-Za-z0-9][A-Za-z0-9._:/#-]{0,171}$/u),
)
const projectedExecutionPublicRefPattern =
  /^[A-Za-z0-9][A-Za-z0-9._:-]{0,179}$/u
const projectedBlockerRefPattern =
  /^blocker\.[A-Za-z0-9][A-Za-z0-9._:-]{0,171}$/u

export const FleetRunTargetPreference = S.Literals([
  "owner_local",
  "managed_cloud",
  "auto",
])
export type FleetRunTargetPreference = typeof FleetRunTargetPreference.Type

export const FleetRunRepositoryPin = S.Struct({
  owner: RepositorySlug,
  name: RepositorySlug,
  branch: RepositoryBranch,
  commit: RepositoryCommit,
})
export type FleetRunRepositoryPin = typeof FleetRunRepositoryPin.Type

// FC-1B persists only immediately executable verifier authority. A future
// verifier-ref form needs a typed resolver before it may cross this boundary;
// storing an unresolved ref would create a durable run Pylon cannot execute.
export const FleetRunVerifier = S.Struct({
  kind: S.Literal("command"),
  command: VerifierCommand,
})
export type FleetRunVerifier = typeof FleetRunVerifier.Type

export const FleetRunIssueListSource = S.Struct({
  kind: S.Literal("issue_list"),
  issueRefs: S.Array(S.Trim.check(S.isMinLength(2), S.isMaxLength(160))),
})
export type FleetRunIssueListSource = typeof FleetRunIssueListSource.Type

export const FleetRunPlanUnit = S.Struct({
  unitRef: PlanUnitRef,
  title: PlanUnitTitle,
  dependsOn: S.optionalKey(S.Array(PlanUnitRef)),
})
export type FleetRunPlanUnit = typeof FleetRunPlanUnit.Type

export const FleetRunPlanDagSource = S.Struct({
  kind: S.Literal("plan_dag"),
  planRef: PublicRef,
  units: S.Array(FleetRunPlanUnit),
})
export type FleetRunPlanDagSource = typeof FleetRunPlanDagSource.Type

export const FleetRunWorkSource = S.Union([
  FleetRunIssueListSource,
  FleetRunPlanDagSource,
])
export type FleetRunWorkSource = typeof FleetRunWorkSource.Type

export const FleetRunWorkerPolicy = S.Struct({
  workerKind: FleetWorkerKindSchema,
  targetPreference: FleetRunTargetPreference,
})
export type FleetRunWorkerPolicy = typeof FleetRunWorkerPolicy.Type

export const FleetRunAuthorityStartRequest = S.Struct({
  schema: S.optionalKey(S.Literal(FLEET_RUN_AUTHORITY_REQUEST_SCHEMA)),
  objective: Objective,
  repository: FleetRunRepositoryPin,
  verifier: FleetRunVerifier,
  workSource: FleetRunWorkSource,
  workerPolicy: FleetRunWorkerPolicy,
  targetConcurrency: S.Int.check(
    S.isGreaterThanOrEqualTo(1),
    S.isLessThanOrEqualTo(8),
  ),
  idempotencyKey: IdempotencyKey,
})
export type FleetRunAuthorityStartRequest =
  typeof FleetRunAuthorityStartRequest.Type

export const FleetRunAuthorityStartInput = S.Struct({
  ownerUserId: PublicOwnerRef,
  request: FleetRunAuthorityStartRequest,
})
export type FleetRunAuthorityStartInput =
  typeof FleetRunAuthorityStartInput.Type

export const FleetRunAuthorityObserveInput = S.Struct({
  ownerUserId: PublicOwnerRef,
  runRef: TrimmedRunRef,
})
export type FleetRunAuthorityObserveInput =
  typeof FleetRunAuthorityObserveInput.Type

export const FleetRunAuthorityClaimInput = S.Struct({
  ownerUserId: PublicOwnerRef,
  pylonRef: PublicPylonRef,
  runRef: S.optionalKey(TrimmedRunRef),
  claimIdempotencyKey: IdempotencyKey,
  leaseDurationMs: S.Int.check(
    S.isGreaterThanOrEqualTo(5_000),
    S.isLessThanOrEqualTo(5 * 60 * 1_000),
  ),
})
export type FleetRunAuthorityClaimInput =
  typeof FleetRunAuthorityClaimInput.Type

export const FleetRunAuthorityAcceptClaimInput = S.Struct({
  ownerUserId: PublicOwnerRef,
  pylonRef: PublicPylonRef,
  runRef: TrimmedRunRef,
  claimRef: TrimmedClaimRef,
})
export type FleetRunAuthorityAcceptClaimInput =
  typeof FleetRunAuthorityAcceptClaimInput.Type

export const FleetRunExecutionState = S.Literals([
  "pending",
  "running",
  "completed",
  "failed",
  "stopped",
])
export type FleetRunExecutionState = typeof FleetRunExecutionState.Type

export const FleetRunExecutionCounters = S.Struct({
  workUnitsTotal: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  activeAssignments: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  acceptedAssignments: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  failedAssignments: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  staleAssignments: S.Int.check(S.isGreaterThanOrEqualTo(0)),
})
export type FleetRunExecutionCounters = typeof FleetRunExecutionCounters.Type

export const FleetRunExactUsageEvidence = S.Struct({
  truth: S.Literal("exact"),
  tokenUsageRefs: S.Array(ExecutionPublicRef),
})
export const FleetRunNotMeasuredUsageEvidence = S.Struct({
  truth: S.Literal("not_measured"),
  tokenUsageRefs: S.Array(ExecutionPublicRef),
})
export const FleetRunUsageEvidence = S.Union([
  FleetRunExactUsageEvidence,
  FleetRunNotMeasuredUsageEvidence,
])
export type FleetRunUsageEvidence = typeof FleetRunUsageEvidence.Type

export const FleetRunUsageEvidenceV2 = S.Union([
  FleetAttemptExactUsageEvidence,
  FleetAttemptNotMeasuredUsageEvidence,
])
export type FleetRunUsageEvidenceV2 = typeof FleetRunUsageEvidenceV2.Type

const FleetRunExecutionEventBase = S.Struct({
  schema: S.Literal(FLEET_RUN_EXECUTION_EVENT_SCHEMA),
  sequence: ExecutionSequence,
  eventRef: ExecutionEventRef,
  observedAt: IsoTimestamp,
})

export const FleetRunStartedExecutionEvent = S.Struct({
  ...FleetRunExecutionEventBase.fields,
  kind: S.Literal("run_started"),
})
export const FleetRunWorkProgressExecutionEvent = S.Struct({
  ...FleetRunExecutionEventBase.fields,
  kind: S.Literal("work_progress"),
  unitRef: PlanUnitRef,
  workClaimRef: ExecutionPublicRef,
  assignmentRef: S.optionalKey(ExecutionPublicRef),
  workerKind: S.Literals(["codex", "claude", "grok"]),
  accountRefHash: S.optionalKey(AccountRefHash),
  blockerRefs: S.Array(LegacyBlockerRef),
})
const FleetRunWorkTerminalExecutionEventBase = S.Struct({
  ...FleetRunExecutionEventBase.fields,
  kind: S.Literal("work_terminal"),
  unitRef: PlanUnitRef,
  workClaimRef: ExecutionPublicRef,
  workerKind: S.Literals(["codex", "claude", "grok"]),
  blockerRefs: S.Array(LegacyBlockerRef),
})
export const FleetRunAcceptedWorkTerminalExecutionEvent = S.Struct({
  ...FleetRunWorkTerminalExecutionEventBase.fields,
  terminalState: S.Literal("accepted"),
  assignmentRef: ExecutionPublicRef,
  accountRefHash: AccountRefHash,
  closeoutRef: ExecutionPublicRef,
  usageEvidence: FleetRunUsageEvidence,
})
export const FleetRunUnprovenWorkTerminalExecutionEvent = S.Struct({
  ...FleetRunWorkTerminalExecutionEventBase.fields,
  terminalState: S.Literals(["failed", "stale"]),
})
export const FleetRunProvenFailedWorkTerminalExecutionEvent = S.Struct({
  ...FleetRunWorkTerminalExecutionEventBase.fields,
  terminalState: S.Literals(["failed", "stale"]),
  assignmentRef: ExecutionPublicRef,
  accountRefHash: AccountRefHash,
  closeoutRef: ExecutionPublicRef,
  usageEvidence: FleetRunUsageEvidence,
})
export const FleetRunWorkTerminalExecutionEvent = S.Union([
  FleetRunAcceptedWorkTerminalExecutionEvent,
  FleetRunProvenFailedWorkTerminalExecutionEvent,
  FleetRunUnprovenWorkTerminalExecutionEvent,
])
export type FleetRunWorkTerminalExecutionEvent =
  typeof FleetRunWorkTerminalExecutionEvent.Type
export const FleetRunTerminalExecutionEvent = S.Struct({
  ...FleetRunExecutionEventBase.fields,
  kind: S.Literal("run_terminal"),
  terminalState: S.Literals(["completed", "failed", "stopped"]),
  blockerRefs: S.Array(LegacyBlockerRef),
})
export const FleetRunExecutionEventV1 = S.Union([
  FleetRunStartedExecutionEvent,
  FleetRunWorkProgressExecutionEvent,
  FleetRunWorkTerminalExecutionEvent,
  FleetRunTerminalExecutionEvent,
])
export type FleetRunExecutionEventV1 = typeof FleetRunExecutionEventV1.Type

const FleetRunExecutionEventV2Base = S.Struct({
  schema: S.Literal(FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2),
  sequence: ExecutionSequence,
  eventRef: ExecutionEventRef,
  observedAt: IsoTimestamp,
})
const FleetRunExecutionV2WorkFields = {
  unitRef: PlanUnitRef,
  workClaimRef: ProjectedExecutionPublicRef,
  assignmentRef: S.optionalKey(ProjectedExecutionPublicRef),
  workerKind: S.Literals(["codex", "claude", "grok"]),
  accountRefHash: S.optionalKey(AccountRefHash),
  marginalCostClass: S.optionalKey(FleetAttemptMarginalCostClass),
  blockerRefs: S.Array(BlockerRef),
} as const
export const FleetRunStartedExecutionEventV2 = S.Struct({
  ...FleetRunExecutionEventV2Base.fields,
  kind: S.Literal("run_started"),
})
export const FleetRunWorkProgressExecutionEventV2 = S.Struct({
  ...FleetRunExecutionEventV2Base.fields,
  kind: S.Literal("work_progress"),
  ...FleetRunExecutionV2WorkFields,
})
export const FleetRunApprovalRequestedExecutionEventV2 = S.Struct({
  ...FleetRunExecutionEventV2Base.fields,
  kind: S.Literal("approval_requested"),
  unitRef: PlanUnitRef,
  workClaimRef: ProjectedExecutionPublicRef,
  assignmentRef: S.optionalKey(ProjectedExecutionPublicRef),
  workerKind: S.Literals(["codex", "claude", "grok"]),
  workerRef: ProjectedExecutionPublicRef,
  accountRefHash: S.optionalKey(AccountRefHash),
  approvalRef: ProjectedExecutionPublicRef,
  toolClass: ApprovalToolClass,
  blockerRefs: S.Array(BlockerRef).check(S.isMinLength(1), S.isMaxLength(32)),
})
export const FleetRunVerifiedEvidenceV2 = S.Struct({
  truth: S.Literal("passed"),
  verifierRef: ProjectedExecutionPublicRef,
  evidenceRefs: S.Array(ProjectedExecutionPublicRef).check(
    S.isMinLength(1),
    S.isMaxLength(64),
  ),
})
export const FleetRunAcceptedWorkTerminalExecutionEventV2 = S.Struct({
  ...FleetRunExecutionEventV2Base.fields,
  kind: S.Literal("work_terminal"),
  ...FleetRunExecutionV2WorkFields,
  terminalState: S.Literal("accepted"),
  assignmentRef: ProjectedExecutionPublicRef,
  accountRefHash: AccountRefHash,
  closeoutRef: ProjectedExecutionPublicRef,
  verification: FleetRunVerifiedEvidenceV2,
  artifactRefs: S.Array(ProjectedExecutionPublicRef).check(
    S.isMinLength(1),
    S.isMaxLength(64),
  ),
  proofRefs: S.Array(ProjectedExecutionPublicRef).check(
    S.isMinLength(1),
    S.isMaxLength(64),
  ),
  authorityReceiptRefs: S.Array(ProjectedExecutionPublicRef).check(
    S.isMinLength(1),
    S.isMaxLength(64),
  ),
  usageEvidence: FleetRunUsageEvidenceV2,
})
export const FleetRunFailedWorkTerminalExecutionEventV2 = S.Struct({
  ...FleetRunExecutionEventV2Base.fields,
  kind: S.Literal("work_terminal"),
  ...FleetRunExecutionV2WorkFields,
  terminalState: S.Literals(["failed", "stale"]),
  closeoutRef: S.optionalKey(ProjectedExecutionPublicRef),
  verification: S.optionalKey(
    S.Struct({
      truth: S.Literal("failed"),
      verifierRef: S.optionalKey(ProjectedExecutionPublicRef),
      evidenceRefs: S.Array(ProjectedExecutionPublicRef).check(
        S.isMaxLength(64),
      ),
    }),
  ),
  artifactRefs: S.optionalKey(
    S.Array(ProjectedExecutionPublicRef).check(S.isMaxLength(64)),
  ),
  proofRefs: S.optionalKey(
    S.Array(ProjectedExecutionPublicRef).check(S.isMaxLength(64)),
  ),
  authorityReceiptRefs: S.optionalKey(
    S.Array(ProjectedExecutionPublicRef).check(S.isMaxLength(64)),
  ),
  usageEvidence: S.optionalKey(FleetRunUsageEvidenceV2),
})
export const FleetRunTerminalExecutionEventV2 = S.Struct({
  ...FleetRunExecutionEventV2Base.fields,
  kind: S.Literal("run_terminal"),
  terminalState: S.Literals(["completed", "failed", "stopped"]),
  blockerRefs: S.Array(BlockerRef),
})
export const FleetRunExecutionEventV2 = S.Union([
  FleetRunStartedExecutionEventV2,
  FleetRunWorkProgressExecutionEventV2,
  FleetRunApprovalRequestedExecutionEventV2,
  FleetRunAcceptedWorkTerminalExecutionEventV2,
  FleetRunFailedWorkTerminalExecutionEventV2,
  FleetRunTerminalExecutionEventV2,
])
export type FleetRunExecutionEventV2 = typeof FleetRunExecutionEventV2.Type

export const FleetRunExecutionEvent = S.Union([
  FleetRunExecutionEventV1,
  FleetRunExecutionEventV2,
])
export type FleetRunExecutionEvent = typeof FleetRunExecutionEvent.Type
type FleetRunAnyWorkTerminalExecutionEvent = Extract<
  FleetRunExecutionEvent,
  { readonly kind: "work_terminal" }
>

export const FleetRunExecutionBatchV1 = S.Struct({
  schema: S.Literal(FLEET_RUN_EXECUTION_BATCH_SCHEMA),
  claimRef: TrimmedClaimRef,
  events: S.Array(FleetRunExecutionEventV1),
})
export const FleetRunExecutionBatchV2 = S.Struct({
  schema: S.Literal(FLEET_RUN_EXECUTION_BATCH_SCHEMA_V2),
  claimRef: TrimmedClaimRef,
  events: S.Array(FleetRunExecutionEventV2),
})
export const FleetRunExecutionBatch = S.Union([
  FleetRunExecutionBatchV1,
  FleetRunExecutionBatchV2,
])
export type FleetRunExecutionBatch = typeof FleetRunExecutionBatch.Type

export const FleetRunAuthorityAppendExecutionInput = S.Struct({
  ownerUserId: PublicOwnerRef,
  pylonRef: PublicPylonRef,
  runRef: TrimmedRunRef,
  batch: FleetRunExecutionBatch,
})
export type FleetRunAuthorityAppendExecutionInput =
  typeof FleetRunAuthorityAppendExecutionInput.Type

const FleetRunWorkUnitCloseoutBase = S.Struct({
  unitRef: PlanUnitRef,
  workClaimRef: ExecutionPublicRef,
  workerKind: S.Literals(["codex", "claude", "grok"]),
  blockerRefs: S.Array(LegacyBlockerRef),
  observedAt: IsoTimestamp,
  eventRef: ExecutionEventRef,
})
export const FleetRunAcceptedWorkUnitCloseout = S.Struct({
  ...FleetRunWorkUnitCloseoutBase.fields,
  terminalState: S.Literal("accepted"),
  assignmentRef: ExecutionPublicRef,
  accountRefHash: AccountRefHash,
  closeoutRef: ExecutionPublicRef,
  usageEvidence: FleetRunUsageEvidence,
})
export const FleetRunUnprovenWorkUnitCloseout = S.Struct({
  ...FleetRunWorkUnitCloseoutBase.fields,
  terminalState: S.Literals(["failed", "stale"]),
})
export const FleetRunProvenFailedWorkUnitCloseout = S.Struct({
  ...FleetRunWorkUnitCloseoutBase.fields,
  terminalState: S.Literals(["failed", "stale"]),
  assignmentRef: ExecutionPublicRef,
  accountRefHash: AccountRefHash,
  closeoutRef: ExecutionPublicRef,
  usageEvidence: FleetRunUsageEvidence,
})
export const FleetRunWorkUnitCloseout = S.Union([
  FleetRunAcceptedWorkUnitCloseout,
  FleetRunProvenFailedWorkUnitCloseout,
  FleetRunUnprovenWorkUnitCloseout,
])
export type FleetRunWorkUnitCloseout = typeof FleetRunWorkUnitCloseout.Type

export const FleetRunExecutionProjection = S.Struct({
  state: FleetRunExecutionState,
  lastSequence: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  counters: FleetRunExecutionCounters,
  startedAt: S.NullOr(IsoTimestamp),
  updatedAt: S.NullOr(IsoTimestamp),
  closeouts: S.Array(FleetRunWorkUnitCloseout),
})
export type FleetRunExecutionProjection =
  typeof FleetRunExecutionProjection.Type

export const FleetRunExecutionAck = S.Struct({
  schema: S.Literal(FLEET_RUN_EXECUTION_ACK_SCHEMA),
  runRef: RunRef,
  claimRef: ClaimRef,
  acceptedThroughSequence: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  storedEventCount: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  duplicateEventCount: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  execution: FleetRunExecutionProjection,
})
export type FleetRunExecutionAck = typeof FleetRunExecutionAck.Type

export const FleetRunAuthorityRecord = S.Struct({
  schema: S.Literal(FLEET_RUN_AUTHORITY_RECORD_SCHEMA),
  runRef: RunRef,
  scope: FleetRunScope,
  ownerUserId: PublicOwnerRef,
  requestFingerprint: S.String.check(S.isPattern(/^[0-9a-f]{64}$/u)),
  status: S.Literals(["pending_executor", "claimed_by_pylon"]),
  request: FleetRunAuthorityStartRequest,
  execution: FleetRunExecutionProjection,
  createdAt: IsoTimestamp,
  updatedAt: IsoTimestamp,
})
export type FleetRunAuthorityRecord = typeof FleetRunAuthorityRecord.Type

export const FleetRunIntakeClaim = S.Struct({
  schema: S.Literal(FLEET_RUN_INTAKE_CLAIM_SCHEMA),
  claimRef: ClaimRef,
  runRef: RunRef,
  ownerUserId: PublicOwnerRef,
  pylonRef: PublicPylonRef,
  claimIdempotencyKey: IdempotencyKey,
  state: S.Literals(["claimed", "accepted"]),
  leaseExpiresAt: IsoTimestamp,
  createdAt: IsoTimestamp,
  updatedAt: IsoTimestamp,
})
export type FleetRunIntakeClaim = typeof FleetRunIntakeClaim.Type

export type FleetRunAuthorityStartResult = Readonly<{
  duplicate: boolean
  record: FleetRunAuthorityRecord
}>

export type FleetRunAuthorityObserveResult = Readonly<{
  record: FleetRunAuthorityRecord
}>

export type FleetRunAuthorityClaimResult = Readonly<{
  duplicate: boolean
  claim: FleetRunIntakeClaim
  run: FleetRunAuthorityRecord
}>

export type FleetRunAuthorityAcceptClaimResult = Readonly<{
  duplicate: boolean
  claim: FleetRunIntakeClaim
  run: FleetRunAuthorityRecord
}>

export type FleetRunAuthorityAppendExecutionResult = Readonly<{
  ack: FleetRunExecutionAck
  record: FleetRunAuthorityRecord
}>

export class FleetRunAuthorityError extends S.TaggedErrorClass<FleetRunAuthorityError>()(
  "FleetRunAuthorityError",
  {
    kind: S.Literals([
      "invalid_request",
      "idempotency_conflict",
      "run_not_found",
      "pylon_not_authorized",
      "pylon_unavailable",
      "claim_conflict",
      "claim_not_found",
      "claim_expired",
      "storage_unavailable",
    ]),
    reason: S.String,
    runRef: S.optionalKey(S.String),
    pylonRef: S.optionalKey(S.String),
  },
) {}

export type FleetRunAuthorityRepositoryShape = Readonly<{
  start: (
    input: unknown,
  ) => Effect.Effect<FleetRunAuthorityStartResult, FleetRunAuthorityError>
  observe: (
    input: unknown,
  ) => Effect.Effect<FleetRunAuthorityObserveResult, FleetRunAuthorityError>
  claim: (
    input: unknown,
  ) => Effect.Effect<FleetRunAuthorityClaimResult, FleetRunAuthorityError>
  acceptClaim: (
    input: unknown,
  ) => Effect.Effect<FleetRunAuthorityAcceptClaimResult, FleetRunAuthorityError>
  appendExecutionEvents?: (
    input: unknown,
  ) => Effect.Effect<
    FleetRunAuthorityAppendExecutionResult,
    FleetRunAuthorityError
  >
}>

export class FleetRunAuthorityRepository extends Context.Service<
  FleetRunAuthorityRepository,
  FleetRunAuthorityRepositoryShape
>()("@openagentsinc/khala-sync-server/FleetRunAuthorityRepository") {}

type FleetRunRequestRow = Readonly<{
  run_ref: string
  owner_user_id: string
  idempotency_key: string
  request_fingerprint: string
  request_json: string
  status: "pending_executor" | "claimed_by_pylon"
  target_preference: FleetRunTargetPreference
  worker_kind: FleetWorkerKind
  target_concurrency: number
  execution_state: FleetRunExecutionState
  execution_last_sequence: string | number | bigint
  execution_started_at: string | null
  execution_updated_at: string | null
  created_at: string
  updated_at: string
}>

type FleetRunLeaseRow = Readonly<{
  run_ref: string
  claim_ref: string
  owner_user_id: string
  pylon_ref: string
  claim_idempotency_key: string
  claim_fingerprint: string
  state: "claimed" | "accepted" | "released"
  lease_expires_at: string
  created_at: string
  updated_at: string
}>

type PylonRegistrationRow = Readonly<{
  owner_agent_user_id: string
  status: string
  latest_heartbeat_at: string | null
  latest_heartbeat_status: string | null
}>

type FleetRunWorkUnitRow = Readonly<{
  unitRef: string
  issueRef: string | null
  title: string | null
  dependsOn: ReadonlyArray<string>
}>

type StoredFleetRunWorkUnitRow = Readonly<{
  run_ref: string
  owner_user_id: string
  unit_index: number
  unit_ref: string
  issue_ref: string | null
  title: string | null
  depends_on_refs_json: string
}>

type FleetRunExecutionEventRow = Readonly<{
  run_ref: string
  sequence: string | number | bigint
  event_ref: string
  owner_user_id: string
  pylon_ref: string
  intake_claim_ref: string
  event_kind: FleetRunExecutionEvent["kind"]
  unit_ref: string | null
  work_claim_ref: string | null
  event_json: string
  observed_at: string
  recorded_at: string
}>

type FleetRunWorkUnitCloseoutRow = Readonly<{
  run_ref: string
  unit_ref: string
  work_claim_ref: string
  assignment_ref: string | null
  worker_kind: "codex" | "claude" | "grok"
  account_ref_hash: string | null
  terminal_state: "accepted" | "failed" | "stale"
  closeout_ref: string | null
  usage_truth: "exact" | "not_measured" | null
  token_usage_refs_json: string | null
  blocker_refs_json: string
  observed_at: string
  event_ref: string
}>

type FleetRunAttemptRow = Readonly<{
  run_ref: string
  attempt_ref: string
  work_unit_ref: string
  owner_user_id: string
  intake_claim_ref: string
  pylon_ref: string
  worker_kind: "codex" | "claude" | "grok"
  state: "running" | "evidence_pending" | "succeeded" | "failed" | "stale"
  progress_class: "active" | "blocked" | "terminal"
  assignment_ref: string | null
  account_ref_hash: string | null
  capacity_class: "owner_local"
  marginal_cost_class: "free" | "subscription" | "api_metered" | "not_measured"
  verification_json: string
  artifact_refs_json: string
  proof_refs_json: string
  authority_receipt_refs_json: string
  closeout_ref: string | null
  usage_json: string
  usage_truth: "pending" | "exact" | "not_measured"
  usage_evidence_ref: string | null
  usage_provider: string | null
  usage_model: string | null
  usage_demand_kind: string | null
  usage_demand_source: string | null
  usage_input_tokens: string | number | bigint | null
  usage_output_tokens: string | number | bigint | null
  usage_reasoning_tokens: string | number | bigint | null
  usage_cache_read_tokens: string | number | bigint | null
  usage_total_tokens: string | number | bigint | null
  usage_token_rows: string | number | bigint | null
  token_usage_refs_json: string
  blocker_refs_json: string
  last_event_ref: string
  first_remote_observed_at: string
  remote_observed_at: string
  last_observed_at: string
  started_at: string
  terminal_at: string | null
  updated_at: string
}>

const PRIVATE_MATERIAL_PATTERN =
  /(?:^|[\s"'])\/(?:Users|private|home)\/|(?:^|[\s"'])~\/|OPENAGENTS_AGENT_TOKEN|(?:API|AUTH|SECRET|TOKEN|PASSWORD|PRIVATE)_?KEY|BEGIN [A-Z ]*PRIVATE KEY/iu
// This is the same bounded checkout contract enforced by Pylon's durable
// workspace materializer. Keep the intake authority at least as strict so a
// request cannot become newly ambiguous or shell-shaped between persistence
// and owner-local import.
const GIT_BRANCH_NAME_PATTERN =
  /^(?!-)(?!refs\/)(?!.*(?:^|\/)\.)(?!.*(?:^|\/)$)(?!.*\.\.)(?!.*@\{)(?!.*\/\/)(?!.*\.lock(?:\/|$))(?!.*\.$)[A-Za-z0-9][A-Za-z0-9._/-]{0,119}$/iu
const PLACEHOLDER_COMMIT_PATTERN = /^(?:0{40}|1{40})$/u
const VERIFICATION_COMMAND_ARG_PATTERN = /^[A-Za-z0-9_./:=@+-]{1,120}$/u
const UNSAFE_VERIFICATION_COMMAND_ARG_PATTERN =
  /(^|[._/:=@+-])(access[_-]?token|bearer|cookie|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?(key|repo)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(command|content|invoice|payment|payload|prompt|repo|runner|state)|secret|seed[_-]?phrase|ssh:|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)([._/:=@+-]|$)|\bsk-[A-Za-z0-9_-]{16,}\b|\bln(?:bc|tb|bcrt)[A-Za-z0-9]{20,}\b/iu
const ACTIVE_PYLON_HEARTBEAT_STATUSES = new Set([
  "available",
  "healthy",
  "idle",
  "online",
  "ready",
])

const invalidRequest = (): FleetRunAuthorityError =>
  new FleetRunAuthorityError({
    kind: "invalid_request",
    reason: "fleet run request failed validation",
  })

const fixedError = (
  kind: FleetRunAuthorityError["kind"],
  reason: string,
  refs: Readonly<{ runRef?: string; pylonRef?: string }> = {},
): FleetRunAuthorityError =>
  new FleetRunAuthorityError({ kind, reason, ...refs })

const authorityErrorFromUnknown = (error: unknown): FleetRunAuthorityError =>
  error instanceof FleetRunAuthorityError
    ? error
    : fixedError("storage_unavailable", "fleet run authority is unavailable")

const decodeUnknown = <A>(schema: S.Decoder<A>, input: unknown): A => {
  try {
    return S.decodeUnknownSync(schema)(input, { onExcessProperty: "error" })
  } catch {
    throw invalidRequest()
  }
}

const assertNoPrivateMaterial = (input: unknown): void => {
  if (PRIVATE_MATERIAL_PATTERN.test(canonicalJson(input))) {
    throw invalidRequest()
  }
}

const validIsoTimestamp = (value: string): boolean => {
  const parsed = new Date(value)
  return Number.isFinite(parsed.getTime()) && parsed.toISOString() === value
}

const accountHashMatchesWorker = (
  workerKind: "codex" | "claude" | "grok",
  accountRefHash: string,
): boolean =>
  accountRefHash.startsWith(
    `account.pylon.${workerKind === "claude" ? "claude_agent" : workerKind}.`,
  )

const refsAreUnique = (refs: ReadonlyArray<string>): boolean =>
  new Set(refs).size === refs.length

export const decodeFleetRunExecutionBatch = (
  input: unknown,
): FleetRunExecutionBatch => {
  const decoded = decodeUnknown(FleetRunExecutionBatch, input)
  if (
    decoded.events.length < 1 ||
    decoded.events.length > 64 ||
    new TextEncoder().encode(canonicalJson(decoded)).byteLength > 256 * 1_024 ||
    new Set(decoded.events.map((event) => event.eventRef)).size !==
      decoded.events.length
  ) {
    throw invalidRequest()
  }
  decoded.events.forEach((event, index) => {
    if (
      !validIsoTimestamp(event.observedAt) ||
      (index > 0 && event.sequence !== decoded.events[index - 1]!.sequence + 1)
    ) {
      throw invalidRequest()
    }
    if (event.kind === "run_started") {
      return
    }
    if (
      event.schema === FLEET_RUN_EXECUTION_EVENT_SCHEMA &&
      (event.blockerRefs.some(
        (blockerRef) => !projectedBlockerRefPattern.test(blockerRef),
      ) ||
        ("workClaimRef" in event &&
          !projectedExecutionPublicRefPattern.test(event.workClaimRef)) ||
        ("assignmentRef" in event &&
          event.assignmentRef !== undefined &&
          !projectedExecutionPublicRefPattern.test(event.assignmentRef)) ||
        ("closeoutRef" in event &&
          event.closeoutRef !== undefined &&
          !projectedExecutionPublicRefPattern.test(event.closeoutRef)))
    ) {
      throw invalidRequest()
    }
    if (event.blockerRefs.length > 32 || !refsAreUnique(event.blockerRefs)) {
      throw invalidRequest()
    }
    if (event.kind === "run_terminal") {
      if (
        (event.terminalState === "completed" && event.blockerRefs.length > 0) ||
        (event.terminalState === "failed" && event.blockerRefs.length < 1)
      ) {
        throw invalidRequest()
      }
      return
    }
    if (event.kind === "work_progress" || event.kind === "approval_requested") {
      if (
        (event.accountRefHash !== undefined &&
          !accountHashMatchesWorker(event.workerKind, event.accountRefHash)) ||
        event.blockerRefs.length > 32 ||
        (event.kind === "approval_requested" && event.blockerRefs.length < 1)
      ) {
        throw invalidRequest()
      }
      return
    }
    if (event.schema === FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2) {
      const terminalEvidenceGroups = event.kind === "work_terminal"
        ? [
            event.verification?.evidenceRefs ?? [],
            event.artifactRefs ?? [],
            event.proofRefs ?? [],
            event.authorityReceiptRefs ?? [],
          ]
        : []
      if (
        (event.accountRefHash !== undefined &&
          !accountHashMatchesWorker(event.workerKind, event.accountRefHash)) ||
        (event.terminalState === "accepted" && event.blockerRefs.length > 0) ||
        (event.terminalState !== "accepted" && event.blockerRefs.length < 1) ||
        terminalEvidenceGroups.some((refs) => !refsAreUnique(refs))
      ) {
        throw invalidRequest()
      }
      if (event.usageEvidence !== undefined) {
        const usage = event.usageEvidence
        const tokenRefs = usage.tokenUsageRefs
        const refGroups =
          usage.truth === "exact"
            ? [
                usage.tokenUsageRefs,
                usage.proofRefs,
                usage.closeoutChecklistRefs,
                usage.proofChecklistRefs,
              ]
            : [usage.caveatRefs]
        if (
          event.workerKind !== usage.harnessKind ||
          event.assignmentRef !== usage.assignmentRef ||
          tokenRefs.length > 100 ||
          (usage.truth === "exact" && tokenRefs.length < 1) ||
          (usage.truth === "not_measured" && tokenRefs.length > 0) ||
          (event.workerKind === "grok" && usage.truth !== "not_measured") ||
          (event.workerKind !== "grok" && usage.truth !== "exact") ||
          refGroups.some((refs) => new Set(refs).size !== refs.length)
        ) {
          throw invalidRequest()
        }
      }
      return
    }
    const proofPresent = "assignmentRef" in event
    if (!proofPresent) {
      if (event.blockerRefs.length < 1) {
        throw invalidRequest()
      }
      return
    }
    const tokenRefs = event.usageEvidence.tokenUsageRefs
    if (
      !accountHashMatchesWorker(event.workerKind, event.accountRefHash) ||
      tokenRefs.length > 100 ||
      (event.usageEvidence.truth === "exact" && tokenRefs.length < 1) ||
      (event.usageEvidence.truth === "not_measured" && tokenRefs.length > 0) ||
      (event.workerKind === "grok" &&
        event.usageEvidence.truth !== "not_measured") ||
      (event.workerKind !== "grok" && event.usageEvidence.truth !== "exact") ||
      (event.terminalState === "accepted" && event.blockerRefs.length > 0) ||
      (event.terminalState !== "accepted" && event.blockerRefs.length < 1)
    ) {
      throw invalidRequest()
    }
  })
  assertNoPrivateMaterial(decoded)
  return decoded
}

const normalizeIssueRef = (
  raw: string,
  repository: FleetRunRepositoryPin,
): string => {
  const value = raw.trim()
  const shortMatch = value.match(/^#?([1-9]\d*)$/u)
  if (shortMatch?.[1] !== undefined) {
    return `#${shortMatch[1]}`
  }
  const urlMatch = value.match(
    /^https:\/\/github\.com\/([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+)\/issues\/([1-9]\d*)$/u,
  )
  if (
    urlMatch?.[1] === undefined ||
    urlMatch[2] === undefined ||
    urlMatch[3] === undefined ||
    urlMatch[1].toLowerCase() !== repository.owner.toLowerCase() ||
    urlMatch[2].toLowerCase() !== repository.name.toLowerCase()
  ) {
    throw invalidRequest()
  }
  return `#${urlMatch[3]}`
}

const normalizeVerifier = (verifier: FleetRunVerifier): FleetRunVerifier => {
  const args = verifier.command.split(/\s+/u).filter(Boolean)
  if (
    args.length < 1 ||
    args.length > 20 ||
    args.some(
      (arg) =>
        !VERIFICATION_COMMAND_ARG_PATTERN.test(arg) ||
        arg.includes("..") ||
        arg.startsWith("/") ||
        UNSAFE_VERIFICATION_COMMAND_ARG_PATTERN.test(arg),
    )
  ) {
    throw invalidRequest()
  }
  return { kind: "command", command: args.join(" ") }
}

const validatePlanDag = (
  source: FleetRunPlanDagSource,
): FleetRunPlanDagSource => {
  if (source.units.length < 1 || source.units.length > 25) {
    throw invalidRequest()
  }
  const byRef = new Map(source.units.map((unit) => [unit.unitRef, unit]))
  if (byRef.size !== source.units.length) {
    throw invalidRequest()
  }
  const normalizedUnits = source.units.map((unit) => {
    const dependsOn = [...new Set(unit.dependsOn ?? [])]
    if (
      dependsOn.length !== (unit.dependsOn ?? []).length ||
      dependsOn.includes(unit.unitRef) ||
      dependsOn.some((ref) => !byRef.has(ref))
    ) {
      throw invalidRequest()
    }
    return { unitRef: unit.unitRef, title: unit.title, dependsOn }
  })
  const normalizedByRef = new Map(
    normalizedUnits.map((unit) => [unit.unitRef, unit]),
  )
  const visiting = new Set<string>()
  const visited = new Set<string>()
  const visit = (unitRef: string): void => {
    if (visited.has(unitRef)) {
      return
    }
    if (visiting.has(unitRef)) {
      throw invalidRequest()
    }
    visiting.add(unitRef)
    normalizedByRef.get(unitRef)?.dependsOn.forEach(visit)
    visiting.delete(unitRef)
    visited.add(unitRef)
  }
  normalizedUnits.forEach((unit) => visit(unit.unitRef))
  return { kind: "plan_dag", planRef: source.planRef, units: normalizedUnits }
}

export const decodeFleetRunAuthorityStartRequest = (
  input: unknown,
): FleetRunAuthorityStartRequest => {
  const decoded = decodeUnknown(FleetRunAuthorityStartRequest, input)
  if (
    !GIT_BRANCH_NAME_PATTERN.test(decoded.repository.branch) ||
    PLACEHOLDER_COMMIT_PATTERN.test(decoded.repository.commit)
  ) {
    throw invalidRequest()
  }
  const workSource: FleetRunWorkSource =
    decoded.workSource.kind === "issue_list"
      ? (() => {
          if (
            decoded.workSource.issueRefs.length < 1 ||
            decoded.workSource.issueRefs.length > 25
          ) {
            throw invalidRequest()
          }
          const issueRefs = decoded.workSource.issueRefs.map((issueRef) =>
            normalizeIssueRef(issueRef, decoded.repository),
          )
          if (new Set(issueRefs).size !== issueRefs.length) {
            throw invalidRequest()
          }
          return { kind: "issue_list", issueRefs }
        })()
      : validatePlanDag(decoded.workSource)
  const normalized: FleetRunAuthorityStartRequest = {
    schema: FLEET_RUN_AUTHORITY_REQUEST_SCHEMA,
    objective: decoded.objective,
    repository: {
      owner: decoded.repository.owner,
      name: decoded.repository.name,
      branch: decoded.repository.branch,
      commit: decoded.repository.commit.toLowerCase(),
    },
    verifier: normalizeVerifier(decoded.verifier),
    workSource,
    workerPolicy: decoded.workerPolicy,
    targetConcurrency: decoded.targetConcurrency,
    idempotencyKey: decoded.idempotencyKey,
  }
  assertNoPrivateMaterial(normalized)
  return normalized
}

const workUnitsFrom = (
  source: FleetRunWorkSource,
): ReadonlyArray<FleetRunWorkUnitRow> =>
  source.kind === "issue_list"
    ? source.issueRefs.map((issueRef) => ({
        unitRef: `issue.${issueRef.slice(1)}`,
        issueRef,
        title: null,
        dependsOn: [],
      }))
    : source.units.map((unit) => ({
        unitRef: unit.unitRef,
        issueRef: null,
        title: unit.title,
        dependsOn: unit.dependsOn ?? [],
      }))

const sha256Hex = async (input: unknown): Promise<string> => {
  const bytes = new TextEncoder().encode(canonicalJson(input))
  const digest = await crypto.subtle.digest("SHA-256", bytes)
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("")
}

const expectedExecutionEventRef = async (
  runRef: string,
  claimRef: string,
  event: FleetRunExecutionEvent,
): Promise<string> => {
  const {
    eventRef: _eventRef,
    sequence: _sequence,
    ...eventWithoutSequenceAndEventRef
  } = event
  return `event.pylon.fleet_run.${(
    await sha256Hex({
      runRef,
      claimRef,
      event: eventWithoutSequenceAndEventRef,
    })
  ).slice(0, 24)}`
}

const assertExecutionEventContentBinding = async (
  runRef: string,
  claimRef: string,
  event: FleetRunExecutionEvent,
): Promise<void> => {
  if (event.eventRef !== (await expectedExecutionEventRef(runRef, claimRef, event))) {
    throw fixedError(
      "invalid_request",
      "fleet run execution event ref does not match its canonical content",
      { runRef },
    )
  }
}

const runRefFor = async (
  ownerUserId: string,
  idempotencyKey: string,
): Promise<string> =>
  `fleet_run.sarah.${(
    await sha256Hex({
      schema: "openagents.sarah.fleet_run_ref.v1",
      ownerUserId,
      idempotencyKey,
    })
  ).slice(0, 20)}`

const claimRefFor = async (
  runRef: string,
  pylonRef: string,
  claimIdempotencyKey: string,
): Promise<string> =>
  `claim.sarah_fleet_run.${(
    await sha256Hex({
      schema: "openagents.sarah.fleet_run_claim_ref.v1",
      runRef,
      pylonRef,
      claimIdempotencyKey,
    })
  ).slice(0, 24)}`

const requestFromJson = S.decodeUnknownSync(
  S.fromJsonString(FleetRunAuthorityStartRequest),
)

const safeStoredSequence = (value: string | number | bigint): number => {
  const parsed = typeof value === "number" ? value : Number(value)
  if (!Number.isSafeInteger(parsed) || parsed < 0) {
    throw fixedError(
      "storage_unavailable",
      "fleet run execution sequence failed integrity validation",
    )
  }
  return parsed
}

const recordFromRow = async (
  row: FleetRunRequestRow,
): Promise<FleetRunAuthorityRecord> => {
  let request: FleetRunAuthorityStartRequest
  try {
    request = decodeFleetRunAuthorityStartRequest(
      requestFromJson(row.request_json),
    )
  } catch {
    throw fixedError(
      "storage_unavailable",
      "fleet run authority record failed integrity validation",
      { runRef: row.run_ref },
    )
  }
  const fingerprint = await sha256Hex(request)
  const expectedRunRef = await runRefFor(row.owner_user_id, row.idempotency_key)
  if (
    row.run_ref !== expectedRunRef ||
    row.request_fingerprint !== fingerprint ||
    row.request_json !== canonicalJson(request) ||
    row.idempotency_key !== request.idempotencyKey ||
    row.target_preference !== request.workerPolicy.targetPreference ||
    row.worker_kind !== request.workerPolicy.workerKind ||
    row.target_concurrency !== request.targetConcurrency
  ) {
    throw fixedError(
      "storage_unavailable",
      "fleet run authority record failed integrity validation",
      { runRef: row.run_ref },
    )
  }
  try {
    return decodeUnknown(FleetRunAuthorityRecord, {
      schema: FLEET_RUN_AUTHORITY_RECORD_SCHEMA,
      runRef: row.run_ref,
      scope: fleetRunScope(row.run_ref),
      ownerUserId: row.owner_user_id,
      requestFingerprint: fingerprint,
      status: row.status,
      request,
      execution: {
        state: row.execution_state,
        lastSequence: safeStoredSequence(row.execution_last_sequence),
        counters: {
          workUnitsTotal: workUnitsFrom(request.workSource).length,
          activeAssignments: 0,
          acceptedAssignments: 0,
          failedAssignments: 0,
          staleAssignments: 0,
        },
        startedAt: row.execution_started_at,
        updatedAt: row.execution_updated_at,
        closeouts: [],
      },
      createdAt: row.created_at,
      updatedAt: row.updated_at,
    })
  } catch {
    throw fixedError(
      "storage_unavailable",
      "fleet run authority record failed integrity validation",
      { runRef: row.run_ref },
    )
  }
}

const claimFromRow = (row: FleetRunLeaseRow): FleetRunIntakeClaim => {
  try {
    const claim = decodeUnknown(FleetRunIntakeClaim, {
      schema: FLEET_RUN_INTAKE_CLAIM_SCHEMA,
      claimRef: row.claim_ref,
      runRef: row.run_ref,
      ownerUserId: row.owner_user_id,
      pylonRef: row.pylon_ref,
      claimIdempotencyKey: row.claim_idempotency_key,
      state: row.state,
      leaseExpiresAt: row.lease_expires_at,
      createdAt: row.created_at,
      updatedAt: row.updated_at,
    })
    if (
      Date.parse(claim.leaseExpiresAt) <= Date.parse(claim.createdAt) ||
      Date.parse(claim.updatedAt) < Date.parse(claim.createdAt)
    ) {
      throw invalidRequest()
    }
    return claim
  } catch {
    throw fixedError(
      "storage_unavailable",
      "fleet run intake claim failed integrity validation",
      { runRef: row.run_ref, pylonRef: row.pylon_ref },
    )
  }
}

const dependsOnFromJson = S.decodeUnknownSync(
  S.fromJsonString(S.Array(PlanUnitRef)),
)

const assertStoredWorkUnits = async (
  sql: SqlTag,
  record: FleetRunAuthorityRecord,
): Promise<void> => {
  const stored: Array<StoredFleetRunWorkUnitRow> = await sql`
    SELECT run_ref, owner_user_id, unit_index, unit_ref, issue_ref, title,
           depends_on_refs_json
    FROM sarah_fleet_run_work_units
    WHERE run_ref = ${record.runRef}
    ORDER BY unit_index
  `
  let decoded: ReadonlyArray<FleetRunWorkUnitRow>
  try {
    decoded = stored.map((row, unitIndex) => {
      if (
        row.run_ref !== record.runRef ||
        row.owner_user_id !== record.ownerUserId ||
        row.unit_index !== unitIndex
      ) {
        throw invalidRequest()
      }
      return {
        unitRef: row.unit_ref,
        issueRef: row.issue_ref,
        title: row.title,
        dependsOn: dependsOnFromJson(row.depends_on_refs_json),
      }
    })
  } catch {
    throw fixedError(
      "storage_unavailable",
      "fleet run work units failed integrity validation",
      { runRef: record.runRef },
    )
  }
  if (
    canonicalJson(decoded) !==
    canonicalJson(workUnitsFrom(record.request.workSource))
  ) {
    throw fixedError(
      "storage_unavailable",
      "fleet run work units failed integrity validation",
      { runRef: record.runRef },
    )
  }
}

const executionRefsFromJson = S.decodeUnknownSync(
  S.fromJsonString(S.Array(ProjectedExecutionPublicRef)),
)
const blockerRefsFromJson = S.decodeUnknownSync(
  S.fromJsonString(S.Array(BlockerRef)),
)

const parseCanonicalJson = (value: string): unknown => {
  const decoded: unknown = JSON.parse(value)
  if (canonicalJson(decoded) !== value) throw invalidRequest()
  return decoded
}

const attemptEntityFromRow = (row: FleetRunAttemptRow): FleetAttemptEntity => {
  try {
    const artifactRefs = executionRefsFromJson(row.artifact_refs_json)
    const proofRefs = executionRefsFromJson(row.proof_refs_json)
    const authorityReceiptRefs = executionRefsFromJson(
      row.authority_receipt_refs_json,
    )
    const tokenUsageRefs = executionRefsFromJson(row.token_usage_refs_json)
    const blockerRefs = blockerRefsFromJson(row.blocker_refs_json)
    const verification = parseCanonicalJson(row.verification_json)
    const usageEvidence = parseCanonicalJson(row.usage_json)
    const entity = decodeFleetAttemptEntity({
      attemptRef: row.attempt_ref,
      workUnitRef: row.work_unit_ref,
      intakeClaimRef: row.intake_claim_ref,
      pylonRef: row.pylon_ref,
      workerKind: row.worker_kind,
      state: row.state,
      progressClass: row.progress_class,
      assignmentRef: row.assignment_ref,
      accountRefHash: row.account_ref_hash,
      capacityClass: row.capacity_class,
      marginalCostClass: row.marginal_cost_class,
      verification,
      artifactRefs,
      proofRefs,
      authorityReceiptRefs,
      closeoutRef: row.closeout_ref,
      usageEvidence,
      blockerRefs,
      lastEventRef: row.last_event_ref,
      startedAt: row.started_at,
      lastObservedAt: row.last_observed_at,
      remoteObservedAt: row.remote_observed_at,
      terminalAt: row.terminal_at,
      updatedAt: row.updated_at,
    })
    if (
      canonicalJson(artifactRefs) !== row.artifact_refs_json ||
      canonicalJson(proofRefs) !== row.proof_refs_json ||
      canonicalJson(authorityReceiptRefs) !==
        row.authority_receipt_refs_json ||
      canonicalJson(tokenUsageRefs) !== row.token_usage_refs_json ||
      canonicalJson(blockerRefs) !== row.blocker_refs_json ||
      entity.usageEvidence.truth !== row.usage_truth ||
      canonicalJson(entity.usageEvidence) !== row.usage_json ||
      (entity.usageEvidence.truth === "pending"
        ? row.usage_evidence_ref !== null ||
          row.usage_provider !== null ||
          row.usage_model !== null ||
          row.usage_demand_kind !== null ||
          row.usage_demand_source !== null ||
          row.usage_input_tokens !== null ||
          row.usage_output_tokens !== null ||
          row.usage_reasoning_tokens !== null ||
          row.usage_cache_read_tokens !== null ||
          row.usage_total_tokens !== null ||
          row.usage_token_rows !== null ||
          tokenUsageRefs.length !== 0
        : entity.usageEvidence.evidenceRef !== row.usage_evidence_ref ||
          (entity.usageEvidence.truth === "exact"
            ? entity.usageEvidence.provider !== row.usage_provider ||
              entity.usageEvidence.model !== row.usage_model ||
              entity.usageEvidence.demandKind !== row.usage_demand_kind ||
              entity.usageEvidence.demandSource !== row.usage_demand_source ||
              entity.usageEvidence.inputTokens !==
                safeStoredSequence(row.usage_input_tokens ?? -1) ||
              entity.usageEvidence.outputTokens !==
                safeStoredSequence(row.usage_output_tokens ?? -1) ||
              entity.usageEvidence.reasoningTokens !==
                safeStoredSequence(row.usage_reasoning_tokens ?? -1) ||
              entity.usageEvidence.cacheReadTokens !==
                safeStoredSequence(row.usage_cache_read_tokens ?? -1) ||
              entity.usageEvidence.totalTokens !==
                safeStoredSequence(row.usage_total_tokens ?? -1) ||
              entity.usageEvidence.tokenRows !==
                safeStoredSequence(row.usage_token_rows ?? -1) ||
              canonicalJson(entity.usageEvidence.tokenUsageRefs) !==
                row.token_usage_refs_json
            : row.usage_provider !== null ||
              row.usage_model !== null ||
              row.usage_demand_kind !== null ||
              row.usage_demand_source !== null ||
              row.usage_input_tokens !== null ||
              row.usage_output_tokens !== null ||
              row.usage_reasoning_tokens !== null ||
              row.usage_cache_read_tokens !== null ||
              row.usage_total_tokens !== null ||
              row.usage_token_rows !== null ||
              tokenUsageRefs.length !== 0))
    ) {
      throw invalidRequest()
    }
    return entity
  } catch {
    throw fixedError(
      "storage_unavailable",
      "fleet run attempt failed integrity validation",
      { runRef: row.run_ref },
    )
  }
}

const workUnitEntityFromStorage = async (
  sql: SqlTag,
  runRef: string,
  unitRef: string,
): Promise<FleetWorkUnitEntity> => {
  const rows: Array<{
    unit_ref: string
    issue_ref: string | null
    depends_on_refs_json: string
    state: FleetWorkUnitEntity["state"]
    latest_attempt_ref: string | null
    accepted_attempt_ref: string | null
    updated_at: string
  }> = await sql`
    SELECT unit_ref, issue_ref, depends_on_refs_json, state,
           latest_attempt_ref, accepted_attempt_ref, updated_at
    FROM sarah_fleet_run_work_units
    WHERE run_ref = ${runRef} AND unit_ref = ${unitRef}
  `
  const row = rows[0]
  if (row === undefined) {
    throw fixedError(
      "storage_unavailable",
      "fleet run work-unit projection is unavailable",
      { runRef },
    )
  }
  try {
    const dependsOnRefs = dependsOnFromJson(row.depends_on_refs_json)
    if (canonicalJson(dependsOnRefs) !== row.depends_on_refs_json) {
      throw invalidRequest()
    }
    return decodeFleetWorkUnitEntity({
      workUnitRef: row.unit_ref,
      issueRef: row.issue_ref,
      dependsOnRefs,
      state: row.state,
      latestAttemptRef: row.latest_attempt_ref,
      acceptedAttemptRef: row.accepted_attempt_ref,
      updatedAt: row.updated_at,
    })
  } catch {
    throw fixedError(
      "storage_unavailable",
      "fleet run work-unit projection failed integrity validation",
      { runRef },
    )
  }
}

const closeoutFromRow = (
  row: FleetRunWorkUnitCloseoutRow,
): FleetRunWorkUnitCloseout => {
  try {
    const proofFields = [
      row.assignment_ref,
      row.account_ref_hash,
      row.closeout_ref,
      row.usage_truth,
      row.token_usage_refs_json,
    ]
    const proofPresent = proofFields.every((value) => value !== null)
    if (!proofPresent && proofFields.some((value) => value !== null)) {
      throw invalidRequest()
    }
    const tokenUsageRefs = proofPresent
      ? executionRefsFromJson(row.token_usage_refs_json!)
      : undefined
    const blockerRefs = blockerRefsFromJson(row.blocker_refs_json)
    const closeout = decodeUnknown(FleetRunWorkUnitCloseout, {
      unitRef: row.unit_ref,
      workClaimRef: row.work_claim_ref,
      workerKind: row.worker_kind,
      terminalState: row.terminal_state,
      ...(proofPresent
        ? {
            assignmentRef: row.assignment_ref!,
            accountRefHash: row.account_ref_hash!,
            closeoutRef: row.closeout_ref!,
            usageEvidence: {
              truth: row.usage_truth!,
              tokenUsageRefs: tokenUsageRefs!,
            },
          }
        : {}),
      blockerRefs,
      observedAt: row.observed_at,
      eventRef: row.event_ref,
    })
    const decodedProofPresent = "assignmentRef" in closeout
    if (
      decodedProofPresent !== proofPresent ||
      closeout.blockerRefs.length > 32 ||
      (closeout.terminalState === "accepted" &&
        closeout.blockerRefs.length > 0) ||
      (closeout.terminalState !== "accepted" &&
        closeout.blockerRefs.length < 1) ||
      (decodedProofPresent &&
        (!accountHashMatchesWorker(
          closeout.workerKind,
          closeout.accountRefHash,
        ) ||
          (closeout.workerKind === "grok" &&
            closeout.usageEvidence.truth !== "not_measured") ||
          (closeout.workerKind !== "grok" &&
            closeout.usageEvidence.truth !== "exact") ||
          (closeout.usageEvidence.truth === "exact" &&
            closeout.usageEvidence.tokenUsageRefs.length < 1) ||
          (closeout.usageEvidence.truth === "not_measured" &&
            closeout.usageEvidence.tokenUsageRefs.length > 0))) ||
      (proofPresent &&
        canonicalJson(tokenUsageRefs) !== row.token_usage_refs_json) ||
      canonicalJson(blockerRefs) !== row.blocker_refs_json
    ) {
      throw invalidRequest()
    }
    return closeout
  } catch {
    throw fixedError(
      "storage_unavailable",
      "fleet run closeout failed integrity validation",
      { runRef: row.run_ref },
    )
  }
}

const executionProjectionFromStorage = async (
  sql: SqlTag,
  record: FleetRunAuthorityRecord,
): Promise<FleetRunExecutionProjection> => {
  const rows: Array<FleetRunWorkUnitCloseoutRow> = await sql`
    SELECT * FROM sarah_fleet_run_work_unit_closeouts
    WHERE run_ref = ${record.runRef}
    ORDER BY unit_ref, observed_at, work_claim_ref
  `
  const closeouts = rows.map(closeoutFromRow)
  const attemptCounts: Array<{
    active: string | number | bigint
    failed: string | number | bigint
    stale: string | number | bigint
  }> = await sql`
    SELECT
      count(*) FILTER (WHERE state = 'running') AS active,
      count(*) FILTER (WHERE state = 'failed') AS failed,
      count(*) FILTER (WHERE state = 'stale') AS stale
    FROM sarah_fleet_run_attempts
    WHERE run_ref = ${record.runRef}
  `
  const succeededRows: Array<{ count: string | number | bigint }> = await sql`
    SELECT count(*) AS count FROM sarah_fleet_run_work_units
    WHERE run_ref = ${record.runRef} AND state = 'succeeded'
  `
  const counts = attemptCounts[0]
  return decodeUnknown(FleetRunExecutionProjection, {
    state: record.execution.state,
    lastSequence: record.execution.lastSequence,
    counters: {
      workUnitsTotal: workUnitsFrom(record.request.workSource).length,
      activeAssignments: safeStoredSequence(counts?.active ?? 0),
      acceptedAssignments: safeStoredSequence(succeededRows[0]?.count ?? 0),
      failedAssignments: safeStoredSequence(counts?.failed ?? 0),
      staleAssignments: safeStoredSequence(counts?.stale ?? 0),
    },
    startedAt: record.execution.startedAt,
    updatedAt: record.execution.updatedAt,
    closeouts,
  })
}

const recordWithExecutionFromRow = async (
  sql: SqlTag,
  row: FleetRunRequestRow,
): Promise<FleetRunAuthorityRecord> => {
  const record = await recordFromRow(row)
  return decodeUnknown(FleetRunAuthorityRecord, {
    ...record,
    execution: await executionProjectionFromStorage(sql, record),
  })
}

const insertWorkUnits = async (
  sql: SqlTag,
  record: FleetRunAuthorityRecord,
): Promise<void> => {
  const units = workUnitsFrom(record.request.workSource)
  await Promise.all(
    units.map(
      (unit, unitIndex) =>
        sql`
        INSERT INTO sarah_fleet_run_work_units
          (run_ref, owner_user_id, unit_index, unit_ref, issue_ref, title,
           depends_on_refs_json, state, updated_at)
        VALUES
          (${record.runRef}, ${record.ownerUserId}, ${unitIndex},
           ${unit.unitRef}, ${unit.issueRef}, ${unit.title},
           ${canonicalJson(unit.dependsOn)}, 'planned', ${record.createdAt})
      `,
    ),
  )
}

const createFleetRun = async (
  sql: SyncSql,
  input: FleetRunAuthorityStartInput,
  nowIso: string,
): Promise<FleetRunAuthorityStartResult> => {
  const request = decodeFleetRunAuthorityStartRequest(input.request)
  const runRef = await runRefFor(input.ownerUserId, request.idempotencyKey)
  const requestFingerprint = await sha256Hex(request)
  const requestJson = canonicalJson(request)
  return withSyncTransaction(sql, async (writer) => {
    const inserted: Array<FleetRunRequestRow> = await writer.sql`
      INSERT INTO sarah_fleet_run_requests
        (run_ref, owner_user_id, idempotency_key, request_fingerprint,
         request_json, status, target_preference, worker_kind,
         target_concurrency, created_at, updated_at)
      VALUES
        (${runRef}, ${input.ownerUserId}, ${request.idempotencyKey},
         ${requestFingerprint}, ${requestJson}, 'pending_executor',
         ${request.workerPolicy.targetPreference},
         ${request.workerPolicy.workerKind}, ${request.targetConcurrency},
         ${nowIso}, ${nowIso})
      ON CONFLICT DO NOTHING
      RETURNING *
    `
    const insertedRow = inserted[0]
    if (insertedRow === undefined) {
      const existing: Array<FleetRunRequestRow> = await writer.sql`
        SELECT * FROM sarah_fleet_run_requests
        WHERE owner_user_id = ${input.ownerUserId}
          AND idempotency_key = ${request.idempotencyKey}
      `
      const existingRow = existing[0]
      if (
        existingRow === undefined ||
        existingRow.request_fingerprint !== requestFingerprint
      ) {
        throw fixedError(
          "idempotency_conflict",
          "fleet run idempotency key is already bound to another request",
        )
      }
      const existingRecord = await recordWithExecutionFromRow(
        writer.sql,
        existingRow,
      )
      await assertStoredWorkUnits(writer.sql, existingRecord)
      return { duplicate: true, record: existingRecord }
    }

    const record = await recordFromRow(insertedRow)
    await insertWorkUnits(writer.sql, record)
    const scope: SyncScope = fleetRunScope(runRef)
    const owner = await ensureScopeOwner(writer.sql, scope, input.ownerUserId)
    if (owner !== input.ownerUserId) {
      throw fixedError(
        "idempotency_conflict",
        "fleet run scope is already owned by another request",
      )
    }
    await appendFleetEntityChange(
      writer,
      runRef,
      {
        kind: "fleet_run",
        op: "upsert",
        entity: fleetRunPostImage({
          runRef,
          state: "draft",
          targetConcurrency: request.targetConcurrency,
          workerKind: request.workerPolicy.workerKind,
          startedAt: null,
          counters: {
            workUnitsTotal: workUnitsFrom(request.workSource).length,
          },
          updatedAt: nowIso,
        }),
      },
      FLEET_RUN_AUTHORITY_CREATE_MUTATION_REF,
    )
    for (const unit of workUnitsFrom(request.workSource)) {
      await appendFleetEntityChange(
        writer,
        runRef,
        {
          kind: "fleet_work_unit",
          op: "upsert",
          entity: decodeFleetWorkUnitEntity({
            workUnitRef: unit.unitRef,
            issueRef: unit.issueRef,
            dependsOnRefs: unit.dependsOn,
            state: "planned",
            latestAttemptRef: null,
            acceptedAttemptRef: null,
            updatedAt: nowIso,
          }),
        },
        FLEET_RUN_AUTHORITY_CREATE_MUTATION_REF,
      )
    }
    return { duplicate: false, record }
  })
}

const observeFleetRun = async (
  sql: SyncSql,
  input: FleetRunAuthorityObserveInput,
): Promise<FleetRunAuthorityObserveResult> =>
  sql.begin(async (tx) => {
    const rows: Array<FleetRunRequestRow> = await tx`
      SELECT * FROM sarah_fleet_run_requests
      WHERE run_ref = ${input.runRef}
        AND owner_user_id = ${input.ownerUserId}
    `
    const row = rows[0]
    if (row === undefined) {
      throw fixedError(
        "run_not_found",
        "fleet run was not found for this owner",
        { runRef: input.runRef },
      )
    }
    const record = await recordWithExecutionFromRow(tx, row)
    await assertStoredWorkUnits(tx, record)
    return { record }
  })

const requireLinkedPylon = async (
  sql: SqlTag,
  input: Pick<FleetRunAuthorityClaimInput, "ownerUserId" | "pylonRef">,
): Promise<PylonRegistrationRow> => {
  const rows: Array<PylonRegistrationRow> = await sql`
    SELECT owner_agent_user_id, status, latest_heartbeat_at,
           latest_heartbeat_status
    FROM pylon_registrations
    WHERE pylon_ref = ${input.pylonRef} AND archived_at IS NULL
  `
  const row = rows[0]
  if (row === undefined) {
    throw fixedError(
      "pylon_not_authorized",
      "Pylon is not linked to this owner",
      { pylonRef: input.pylonRef },
    )
  }
  const directOwner = row.owner_agent_user_id === input.ownerUserId
  const linkedRows: Array<{ linked: boolean }> = directOwner
    ? []
    : await sql`
        SELECT true AS linked
        FROM openauth_agent_links
        WHERE openauth_user_id = ${input.ownerUserId}
          AND agent_user_id = ${row.owner_agent_user_id}
          AND status = 'active'
          AND revoked_at IS NULL
        LIMIT 1
      `
  if (!directOwner && linkedRows[0]?.linked !== true) {
    throw fixedError(
      "pylon_not_authorized",
      "Pylon is not linked to this owner",
      { pylonRef: input.pylonRef },
    )
  }
  return row
}

const requireClaimablePylon = async (
  sql: SqlTag,
  input: Pick<FleetRunAuthorityClaimInput, "ownerUserId" | "pylonRef">,
  nowMs: number,
): Promise<void> => {
  const row = await requireLinkedPylon(sql, input)
  const heartbeatMs =
    row.latest_heartbeat_at === null
      ? Number.NaN
      : Date.parse(row.latest_heartbeat_at)
  const heartbeatAge = nowMs - heartbeatMs
  if (
    row.status !== "active" ||
    !ACTIVE_PYLON_HEARTBEAT_STATUSES.has(
      row.latest_heartbeat_status?.trim().toLowerCase() ?? "",
    ) ||
    !Number.isFinite(heartbeatAge) ||
    heartbeatAge < 0 ||
    heartbeatAge > FLEET_RUN_PYLON_FRESHNESS_MS
  ) {
    throw fixedError(
      "pylon_unavailable",
      "Pylon is not active and heartbeat-fresh",
      { pylonRef: input.pylonRef },
    )
  }
}

const readClaimReplay = async (
  sql: SqlTag,
  input: FleetRunAuthorityClaimInput,
  claimFingerprint: string,
  nowMs: number,
): Promise<FleetRunAuthorityClaimResult | null> => {
  const rows: Array<FleetRunLeaseRow> = await sql`
    SELECT * FROM sarah_fleet_run_intake_leases
    WHERE owner_user_id = ${input.ownerUserId}
      AND pylon_ref = ${input.pylonRef}
      AND claim_idempotency_key = ${input.claimIdempotencyKey}
    FOR UPDATE
  `
  const row = rows[0]
  if (row === undefined) {
    return null
  }
  if (row.claim_fingerprint !== claimFingerprint) {
    throw fixedError(
      "idempotency_conflict",
      "claim idempotency key is already bound to another request",
      { pylonRef: input.pylonRef },
    )
  }
  if (row.state === "accepted") {
    throw fixedError(
      "claim_conflict",
      "fleet run intake claim was already accepted",
      { runRef: row.run_ref, pylonRef: row.pylon_ref },
    )
  }
  if (row.state !== "claimed" || Date.parse(row.lease_expires_at) <= nowMs) {
    throw fixedError(
      "claim_expired",
      "fleet run intake claim is no longer active",
      { runRef: row.run_ref, pylonRef: row.pylon_ref },
    )
  }
  const runRows: Array<FleetRunRequestRow> = await sql`
    SELECT * FROM sarah_fleet_run_requests
    WHERE run_ref = ${row.run_ref} AND owner_user_id = ${input.ownerUserId}
  `
  const runRow = runRows[0]
  if (runRow === undefined) {
    throw fixedError(
      "storage_unavailable",
      "fleet run claim points to a missing authority record",
      { runRef: row.run_ref },
    )
  }
  const run = await recordFromRow(runRow)
  await assertStoredWorkUnits(sql, run)
  return {
    duplicate: true,
    claim: claimFromRow(row),
    run,
  }
}

const selectClaimableRun = async (
  sql: SqlTag,
  input: FleetRunAuthorityClaimInput,
  nowIso: string,
): Promise<FleetRunRequestRow> => {
  const rows: Array<FleetRunRequestRow> =
    input.runRef === undefined
      ? await sql`
          SELECT request.*
          FROM sarah_fleet_run_requests AS request
          LEFT JOIN sarah_fleet_run_intake_leases AS lease
            ON lease.run_ref = request.run_ref
           AND lease.state = 'claimed'
           AND lease.lease_expires_at > ${nowIso}
          WHERE request.owner_user_id = ${input.ownerUserId}
            AND request.status = 'pending_executor'
            AND request.target_preference IN ('owner_local', 'auto')
            AND lease.run_ref IS NULL
          ORDER BY request.created_at, request.run_ref
          LIMIT 1
          FOR UPDATE OF request SKIP LOCKED
        `
      : await sql`
          SELECT * FROM sarah_fleet_run_requests
          WHERE run_ref = ${input.runRef}
            AND owner_user_id = ${input.ownerUserId}
            AND status = 'pending_executor'
            AND target_preference IN ('owner_local', 'auto')
          FOR UPDATE
        `
  const row = rows[0]
  if (row === undefined) {
    throw fixedError(
      "run_not_found",
      "no claimable fleet run exists for this owner",
      input.runRef === undefined ? {} : { runRef: input.runRef },
    )
  }
  return row
}

const claimFleetRun = async (
  sql: SyncSql,
  input: FleetRunAuthorityClaimInput,
  nowMs: number,
): Promise<FleetRunAuthorityClaimResult> => {
  const nowIso = new Date(nowMs).toISOString()
  const leaseExpiresAt = new Date(nowMs + input.leaseDurationMs).toISOString()
  const claimFingerprint = await sha256Hex({
    schema: "openagents.sarah.fleet_run_claim_request.v1",
    ownerUserId: input.ownerUserId,
    pylonRef: input.pylonRef,
    runRef: input.runRef ?? null,
    claimIdempotencyKey: input.claimIdempotencyKey,
    leaseDurationMs: input.leaseDurationMs,
  })
  return sql.begin(async (tx) => {
    await requireClaimablePylon(tx, input, nowMs)
    const replay = await readClaimReplay(tx, input, claimFingerprint, nowMs)
    if (replay !== null) {
      return replay
    }
    const runRow = await selectClaimableRun(tx, input, nowIso)
    const run = await recordFromRow(runRow)
    await assertStoredWorkUnits(tx, run)
    const leaseRows: Array<FleetRunLeaseRow> = await tx`
      SELECT * FROM sarah_fleet_run_intake_leases
      WHERE run_ref = ${run.runRef}
      FOR UPDATE
    `
    const existing = leaseRows[0]
    if (
      existing !== undefined &&
      existing.state === "claimed" &&
      Date.parse(existing.lease_expires_at) > nowMs
    ) {
      throw fixedError(
        "claim_conflict",
        "fleet run already has an active intake claim",
        { runRef: run.runRef },
      )
    }
    const claimRef = await claimRefFor(
      run.runRef,
      input.pylonRef,
      input.claimIdempotencyKey,
    )
    const written: Array<FleetRunLeaseRow> =
      existing === undefined
        ? await tx`
            INSERT INTO sarah_fleet_run_intake_leases
              (run_ref, claim_ref, owner_user_id, pylon_ref,
               claim_idempotency_key, claim_fingerprint, state,
               lease_expires_at, created_at, updated_at)
            VALUES
              (${run.runRef}, ${claimRef}, ${input.ownerUserId},
               ${input.pylonRef}, ${input.claimIdempotencyKey},
               ${claimFingerprint}, 'claimed', ${leaseExpiresAt},
               ${nowIso}, ${nowIso})
            RETURNING *
          `
        : await tx`
            UPDATE sarah_fleet_run_intake_leases
            SET claim_ref = ${claimRef},
                owner_user_id = ${input.ownerUserId},
                pylon_ref = ${input.pylonRef},
                claim_idempotency_key = ${input.claimIdempotencyKey},
                claim_fingerprint = ${claimFingerprint},
                state = 'claimed',
                lease_expires_at = ${leaseExpiresAt},
                created_at = ${nowIso},
                updated_at = ${nowIso}
            WHERE run_ref = ${run.runRef}
            RETURNING *
          `
    const row = written[0]
    if (row === undefined) {
      throw fixedError(
        "storage_unavailable",
        "fleet run intake claim was not persisted",
        { runRef: run.runRef },
      )
    }
    return { duplicate: false, claim: claimFromRow(row), run }
  })
}

const acceptFleetRunClaim = async (
  sql: SyncSql,
  input: FleetRunAuthorityAcceptClaimInput,
  nowMs: number,
): Promise<FleetRunAuthorityAcceptClaimResult> => {
  const nowIso = new Date(nowMs).toISOString()
  return sql.begin(async (tx) => {
    await requireClaimablePylon(tx, input, nowMs)
    const leaseRows: Array<FleetRunLeaseRow> = await tx`
      SELECT * FROM sarah_fleet_run_intake_leases
      WHERE run_ref = ${input.runRef}
        AND claim_ref = ${input.claimRef}
        AND owner_user_id = ${input.ownerUserId}
        AND pylon_ref = ${input.pylonRef}
      FOR UPDATE
    `
    const lease = leaseRows[0]
    if (lease === undefined) {
      throw fixedError(
        "claim_not_found",
        "fleet run intake claim was not found",
        { runRef: input.runRef, pylonRef: input.pylonRef },
      )
    }
    const runRows: Array<FleetRunRequestRow> = await tx`
      SELECT * FROM sarah_fleet_run_requests
      WHERE run_ref = ${input.runRef}
        AND owner_user_id = ${input.ownerUserId}
      FOR UPDATE
    `
    const runRow = runRows[0]
    if (runRow === undefined) {
      throw fixedError(
        "storage_unavailable",
        "fleet run claim points to a missing authority record",
        { runRef: input.runRef },
      )
    }
    const run = await recordFromRow(runRow)
    await assertStoredWorkUnits(tx, run)
    if (lease.state === "accepted") {
      if (run.status !== "claimed_by_pylon") {
        throw fixedError(
          "storage_unavailable",
          "fleet run claim acceptance state is inconsistent",
          { runRef: input.runRef },
        )
      }
      return {
        duplicate: true,
        claim: claimFromRow(lease),
        run: await recordWithExecutionFromRow(tx, runRow),
      }
    }
    if (
      lease.state !== "claimed" ||
      Date.parse(lease.lease_expires_at) <= nowMs
    ) {
      throw fixedError(
        "claim_expired",
        "fleet run intake claim is no longer active",
        { runRef: input.runRef, pylonRef: input.pylonRef },
      )
    }
    if (run.status !== "pending_executor") {
      throw fixedError(
        "claim_conflict",
        "fleet run is no longer pending Pylon intake",
        { runRef: input.runRef },
      )
    }
    const acceptedRows: Array<FleetRunLeaseRow> = await tx`
      UPDATE sarah_fleet_run_intake_leases
      SET state = 'accepted', updated_at = ${nowIso}
      WHERE run_ref = ${input.runRef} AND claim_ref = ${input.claimRef}
      RETURNING *
    `
    const claimedRunRows: Array<FleetRunRequestRow> = await tx`
      UPDATE sarah_fleet_run_requests
      SET status = 'claimed_by_pylon', updated_at = ${nowIso}
      WHERE run_ref = ${input.runRef} AND status = 'pending_executor'
      RETURNING *
    `
    const accepted = acceptedRows[0]
    const claimedRun = claimedRunRows[0]
    if (accepted === undefined || claimedRun === undefined) {
      throw fixedError(
        "storage_unavailable",
        "fleet run intake claim acceptance was not persisted",
        { runRef: input.runRef },
      )
    }
    return {
      duplicate: false,
      claim: claimFromRow(accepted),
      run: await recordFromRow(claimedRun),
    }
  })
}

const executionEventFromRow = (
  row: FleetRunExecutionEventRow,
): FleetRunExecutionEvent => {
  try {
    const event = S.decodeUnknownSync(S.fromJsonString(FleetRunExecutionEvent))(
      row.event_json,
    )
    if (
      row.run_ref === "" ||
      safeStoredSequence(row.sequence) !== event.sequence ||
      row.event_ref !== event.eventRef ||
      row.event_kind !== event.kind ||
      row.unit_ref !==
        (event.kind === "work_progress" ||
        event.kind === "approval_requested" ||
        event.kind === "work_terminal"
          ? event.unitRef
          : null) ||
      row.work_claim_ref !==
        (event.kind === "work_progress" ||
        event.kind === "approval_requested" ||
        event.kind === "work_terminal"
          ? event.workClaimRef
          : null) ||
      row.observed_at !== event.observedAt ||
      canonicalJson(event) !== row.event_json
    ) {
      throw invalidRequest()
    }
    return event
  } catch {
    throw fixedError(
      "storage_unavailable",
      "fleet run execution event failed integrity validation",
      { runRef: row.run_ref },
    )
  }
}

const closeoutForEvent = (
  event: FleetRunAnyWorkTerminalExecutionEvent,
): FleetRunWorkUnitCloseout => {
  const proof =
    event.schema === FLEET_RUN_EXECUTION_EVENT_SCHEMA
      ? "assignmentRef" in event
        ? {
            assignmentRef: event.assignmentRef,
            accountRefHash: event.accountRefHash,
            closeoutRef: event.closeoutRef,
            usageEvidence: {
              truth: event.usageEvidence.truth,
              tokenUsageRefs: event.usageEvidence.tokenUsageRefs,
            },
          }
        : {}
      : event.assignmentRef !== undefined &&
          event.accountRefHash !== undefined &&
          event.closeoutRef !== undefined &&
          event.usageEvidence !== undefined
        ? {
            assignmentRef: event.assignmentRef,
            accountRefHash: event.accountRefHash,
            closeoutRef: event.closeoutRef,
            usageEvidence: {
              truth: event.usageEvidence.truth,
              tokenUsageRefs: event.usageEvidence.tokenUsageRefs,
            },
          }
        : {}
  return decodeUnknown(FleetRunWorkUnitCloseout, {
    unitRef: event.unitRef,
    workClaimRef: event.workClaimRef,
    workerKind: event.workerKind,
    terminalState: event.terminalState,
    ...proof,
    blockerRefs: event.blockerRefs,
    observedAt: event.observedAt,
    eventRef: event.eventRef,
  })
}

const executionStateForSync = (
  state: FleetRunExecutionState,
): "draft" | "running" | "stopped" | "completed" =>
  state === "pending"
    ? "draft"
    : state === "running"
      ? "running"
      : state === "completed"
        ? "completed"
        : "stopped"

const insertExecutionEvent = async (
  sql: SqlTag,
  input: FleetRunAuthorityAppendExecutionInput,
  event: FleetRunExecutionEvent,
  nowIso: string,
): Promise<void> => {
  const unitRef =
    event.kind === "work_progress" ||
    event.kind === "approval_requested" ||
    event.kind === "work_terminal"
      ? event.unitRef
      : null
  const workClaimRef =
    event.kind === "work_progress" ||
    event.kind === "approval_requested" ||
    event.kind === "work_terminal"
      ? event.workClaimRef
      : null
  await sql`
    INSERT INTO sarah_fleet_run_execution_events
      (run_ref, sequence, event_ref, owner_user_id, pylon_ref,
       intake_claim_ref, event_kind, unit_ref, work_claim_ref, event_json,
       observed_at, recorded_at)
    VALUES
      (${input.runRef}, ${event.sequence}, ${event.eventRef},
       ${input.ownerUserId}, ${input.pylonRef}, ${input.batch.claimRef},
       ${event.kind}, ${unitRef}, ${workClaimRef}, ${canonicalJson(event)},
       ${event.observedAt}, ${nowIso})
  `
}

const projectAttemptForEvent = async (
  sql: SqlTag,
  input: FleetRunAuthorityAppendExecutionInput,
  event: Extract<
    FleetRunExecutionEvent,
    {
      readonly kind:
        | "work_progress"
        | "approval_requested"
        | "work_terminal"
    }
  >,
  nowIso: string,
): Promise<Readonly<{
  attempt: FleetAttemptEntity
  workUnit: FleetWorkUnitEntity
}>> => {
  const existingRows: Array<FleetRunAttemptRow> = await sql`
    SELECT * FROM sarah_fleet_run_attempts
    WHERE run_ref = ${input.runRef} AND attempt_ref = ${event.workClaimRef}
    FOR UPDATE
  `
  const existingRow = existingRows[0]
  const existing =
    existingRow === undefined ? undefined : attemptEntityFromRow(existingRow)
  if (event.kind === "approval_requested" && existing === undefined) {
    throw fixedError(
      "claim_conflict",
      "fleet approval requires an active exact work attempt",
      { runRef: input.runRef },
    )
  }
  if (
    existing !== undefined &&
    (existingRow?.owner_user_id !== input.ownerUserId ||
      existing.workUnitRef !== event.unitRef ||
      existing.workerKind !== event.workerKind ||
      existing.intakeClaimRef !== input.batch.claimRef ||
      existing.pylonRef !== input.pylonRef)
  ) {
    throw fixedError(
      "idempotency_conflict",
      "fleet run work claim is already bound to another attempt",
      { runRef: input.runRef },
    )
  }
  if (existing !== undefined && existing.state !== "running") {
    throw fixedError(
      "idempotency_conflict",
      "fleet run attempt already has terminal evidence",
      { runRef: input.runRef },
    )
  }

  const incomingAssignmentRef =
    "assignmentRef" in event ? (event.assignmentRef ?? null) : null
  const incomingAccountRefHash =
    "accountRefHash" in event ? (event.accountRefHash ?? null) : null
  const incomingMarginalCostClass =
    "marginalCostClass" in event
      ? (event.marginalCostClass ?? "not_measured")
      : "not_measured"
  if (
    (existing?.assignmentRef !== null &&
      existing?.assignmentRef !== undefined &&
      incomingAssignmentRef !== null &&
      existing.assignmentRef !== incomingAssignmentRef) ||
    (existing?.accountRefHash !== null &&
      existing?.accountRefHash !== undefined &&
      incomingAccountRefHash !== null &&
      existing.accountRefHash !== incomingAccountRefHash) ||
    (existing !== undefined &&
      existing.marginalCostClass !== "not_measured" &&
      incomingMarginalCostClass !== "not_measured" &&
      existing.marginalCostClass !== incomingMarginalCostClass)
  ) {
    throw fixedError(
      "idempotency_conflict",
      "fleet run attempt evidence changed an established graph edge",
      { runRef: input.runRef },
    )
  }
  const assignmentRef = existing?.assignmentRef ?? incomingAssignmentRef
  const accountRefHash = existing?.accountRefHash ?? incomingAccountRefHash
  const marginalCostClass =
    existing?.marginalCostClass === "not_measured"
      ? incomingMarginalCostClass
      : (existing?.marginalCostClass ?? incomingMarginalCostClass)

  let state: FleetAttemptEntity["state"] = "running"
  let progressClass: FleetAttemptEntity["progressClass"] =
    event.blockerRefs.length > 0 ? "blocked" : "active"
  let verification: FleetAttemptEntity["verification"] = { truth: "pending" }
  let artifactRefs: ReadonlyArray<string> = []
  let proofRefs: ReadonlyArray<string> = []
  let authorityReceiptRefs: ReadonlyArray<string> = []
  let closeoutRef: string | null = null
  let usageEvidence: FleetAttemptEntity["usageEvidence"] = {
    truth: "pending",
  }
  let terminalAt: string | null = null
  if (event.kind === "work_terminal") {
    progressClass = "terminal"
    terminalAt = nowIso
    if (event.schema === FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2) {
      if (event.terminalState === "accepted") {
        if (
          event.usageEvidence.truth === "exact" &&
          event.usageEvidence.pylonRef !== input.pylonRef
        ) {
          throw fixedError(
            "invalid_request",
            "fleet run usage evidence names another Pylon",
            { runRef: input.runRef, pylonRef: input.pylonRef },
          )
        }
        state = "succeeded"
        verification = event.verification
        artifactRefs = event.artifactRefs
        proofRefs = event.proofRefs
        authorityReceiptRefs = event.authorityReceiptRefs
        closeoutRef = event.closeoutRef
        usageEvidence = event.usageEvidence
      } else {
        state = event.terminalState
        verification =
          event.verification === undefined
            ? { truth: "not_reported" }
            : {
                truth: "failed",
                verifierRef: event.verification.verifierRef ?? null,
                evidenceRefs: event.verification.evidenceRefs,
              }
        artifactRefs = event.artifactRefs ?? []
        proofRefs = event.proofRefs ?? []
        authorityReceiptRefs = event.authorityReceiptRefs ?? []
        closeoutRef = event.closeoutRef ?? null
        if (
          event.usageEvidence?.truth === "exact" &&
          event.usageEvidence.pylonRef !== input.pylonRef
        ) {
          throw fixedError(
            "invalid_request",
            "fleet run usage evidence names another Pylon",
            { runRef: input.runRef, pylonRef: input.pylonRef },
          )
        }
        usageEvidence = event.usageEvidence ?? { truth: "pending" }
      }
    } else if (event.terminalState === "accepted") {
      // v1 did not carry verifier/artifact/proof/authority receipts. Preserve
      // its closeout and usage honestly, but do not promote it to succeeded.
      state = "evidence_pending"
      verification = { truth: "not_reported" }
      closeoutRef = event.closeoutRef
    } else {
      state = event.terminalState
      verification = { truth: "not_reported" }
      if ("closeoutRef" in event) {
        closeoutRef = event.closeoutRef
      }
    }
  }

  const startedAt = existing?.startedAt ?? nowIso
  const attempt = decodeFleetAttemptEntity({
    attemptRef: event.workClaimRef,
    workUnitRef: event.unitRef,
    intakeClaimRef: input.batch.claimRef,
    pylonRef: input.pylonRef,
    workerKind: event.workerKind,
    state,
    progressClass,
    assignmentRef,
    accountRefHash,
    capacityClass: "owner_local",
    marginalCostClass,
    verification,
    artifactRefs,
    proofRefs,
    authorityReceiptRefs,
    closeoutRef,
    usageEvidence,
    blockerRefs: event.blockerRefs,
    lastEventRef: event.eventRef,
    startedAt,
    lastObservedAt: nowIso,
    remoteObservedAt: event.observedAt,
    terminalAt,
    updatedAt: nowIso,
  })
  const exactUsage =
    attempt.usageEvidence.truth === "exact" ? attempt.usageEvidence : undefined
  const measuredUsage =
    attempt.usageEvidence.truth === "pending"
      ? undefined
      : attempt.usageEvidence
  const inserted: Array<FleetRunAttemptRow> = await sql`
    INSERT INTO sarah_fleet_run_attempts
      (run_ref, attempt_ref, work_unit_ref, owner_user_id, intake_claim_ref,
       pylon_ref, worker_kind, state, progress_class, assignment_ref,
       account_ref_hash, capacity_class, marginal_cost_class,
       verification_json, artifact_refs_json, proof_refs_json,
       authority_receipt_refs_json, closeout_ref, usage_json, usage_truth,
       usage_evidence_ref, usage_provider, usage_model, usage_demand_kind,
       usage_demand_source, usage_input_tokens, usage_output_tokens,
       usage_reasoning_tokens, usage_cache_read_tokens, usage_total_tokens,
       usage_token_rows, token_usage_refs_json, blocker_refs_json,
       last_event_ref, first_remote_observed_at, remote_observed_at,
       last_observed_at, started_at, terminal_at, updated_at)
    VALUES
      (${input.runRef}, ${attempt.attemptRef}, ${attempt.workUnitRef},
       ${input.ownerUserId}, ${input.batch.claimRef}, ${input.pylonRef},
       ${attempt.workerKind}, ${attempt.state}, ${attempt.progressClass},
       ${attempt.assignmentRef}, ${attempt.accountRefHash},
       ${attempt.capacityClass}, ${attempt.marginalCostClass},
       ${canonicalJson(attempt.verification)},
       ${canonicalJson(attempt.artifactRefs)},
       ${canonicalJson(attempt.proofRefs)},
       ${canonicalJson(attempt.authorityReceiptRefs)},
       ${attempt.closeoutRef}, ${canonicalJson(attempt.usageEvidence)},
       ${attempt.usageEvidence.truth}, ${measuredUsage?.evidenceRef ?? null},
       ${exactUsage?.provider ?? null}, ${exactUsage?.model ?? null},
       ${exactUsage?.demandKind ?? null}, ${exactUsage?.demandSource ?? null},
       ${exactUsage?.inputTokens ?? null}, ${exactUsage?.outputTokens ?? null},
       ${exactUsage?.reasoningTokens ?? null},
       ${exactUsage?.cacheReadTokens ?? null},
       ${exactUsage?.totalTokens ?? null}, ${exactUsage?.tokenRows ?? null},
       ${canonicalJson(exactUsage?.tokenUsageRefs ?? [])},
       ${canonicalJson(attempt.blockerRefs)}, ${attempt.lastEventRef},
       ${existingRow?.first_remote_observed_at ?? event.observedAt},
       ${attempt.remoteObservedAt}, ${attempt.lastObservedAt},
       ${attempt.startedAt}, ${attempt.terminalAt},
       ${attempt.updatedAt})
    ON CONFLICT (run_ref, attempt_ref) DO UPDATE SET
      state = EXCLUDED.state,
      progress_class = EXCLUDED.progress_class,
      assignment_ref = EXCLUDED.assignment_ref,
      account_ref_hash = EXCLUDED.account_ref_hash,
      marginal_cost_class = EXCLUDED.marginal_cost_class,
      verification_json = EXCLUDED.verification_json,
      artifact_refs_json = EXCLUDED.artifact_refs_json,
      proof_refs_json = EXCLUDED.proof_refs_json,
      authority_receipt_refs_json = EXCLUDED.authority_receipt_refs_json,
      closeout_ref = EXCLUDED.closeout_ref,
      usage_json = EXCLUDED.usage_json,
      usage_truth = EXCLUDED.usage_truth,
      usage_evidence_ref = EXCLUDED.usage_evidence_ref,
      usage_provider = EXCLUDED.usage_provider,
      usage_model = EXCLUDED.usage_model,
      usage_demand_kind = EXCLUDED.usage_demand_kind,
      usage_demand_source = EXCLUDED.usage_demand_source,
      usage_input_tokens = EXCLUDED.usage_input_tokens,
      usage_output_tokens = EXCLUDED.usage_output_tokens,
      usage_reasoning_tokens = EXCLUDED.usage_reasoning_tokens,
      usage_cache_read_tokens = EXCLUDED.usage_cache_read_tokens,
      usage_total_tokens = EXCLUDED.usage_total_tokens,
      usage_token_rows = EXCLUDED.usage_token_rows,
      token_usage_refs_json = EXCLUDED.token_usage_refs_json,
      blocker_refs_json = EXCLUDED.blocker_refs_json,
      last_event_ref = EXCLUDED.last_event_ref,
      remote_observed_at = EXCLUDED.remote_observed_at,
      last_observed_at = EXCLUDED.last_observed_at,
      terminal_at = EXCLUDED.terminal_at,
      updated_at = EXCLUDED.updated_at
    RETURNING *
  `
  const storedAttempt = inserted[0]
  if (storedAttempt === undefined) {
    throw fixedError(
      "storage_unavailable",
      "fleet run attempt was not persisted",
      { runRef: input.runRef },
    )
  }
  const projectedAttempt = attemptEntityFromRow(storedAttempt)
  if (canonicalJson(projectedAttempt) !== canonicalJson(attempt)) {
    throw fixedError(
      "storage_unavailable",
      "fleet run attempt projection changed during persistence",
      { runRef: input.runRef },
    )
  }

  const workUnitState: FleetWorkUnitEntity["state"] =
    attempt.state === "running"
      ? "running"
      : attempt.state === "evidence_pending"
        ? "verification_pending"
        : attempt.state
  await sql`
    UPDATE sarah_fleet_run_work_units
    SET state = ${workUnitState},
        latest_attempt_ref = ${attempt.attemptRef},
        accepted_attempt_ref = ${
          attempt.state === "succeeded" ? attempt.attemptRef : null
        },
        updated_at = ${nowIso}
    WHERE run_ref = ${input.runRef} AND unit_ref = ${event.unitRef}
  `
  const workUnit = await workUnitEntityFromStorage(
    sql,
    input.runRef,
    event.unitRef,
  )
  return { attempt: projectedAttempt, workUnit }
}

const projectApprovalRequestedForEvent = async (
  sql: SqlTag,
  input: FleetRunAuthorityAppendExecutionInput,
  event: typeof FleetRunApprovalRequestedExecutionEventV2.Type,
  attempt: FleetAttemptEntity,
  nowIso: string,
): Promise<FleetApprovalEntity> => {
  const rows: Array<{ scope: string; post_image_json: string | object }> =
    await sql`
      SELECT scope, post_image_json
      FROM khala_sync_changelog
      WHERE entity_type = 'fleet_approval'
        AND entity_id = ${event.approvalRef}
        AND op = 'upsert'
      ORDER BY committed_at DESC, version DESC
      LIMIT 1
      FOR UPDATE
    `
  const stored = rows[0]
  const raw = stored?.post_image_json
  if (raw !== undefined) {
    let current: FleetApprovalEntity
    try {
      current = decodeFleetApprovalEntity(
        typeof raw === "string" ? JSON.parse(raw) : raw,
      )
    } catch {
      throw fixedError(
        "storage_unavailable",
        "fleet approval projection failed integrity validation",
        { runRef: input.runRef },
      )
    }
    if (
      current.status !== "pending" ||
      stored?.scope !== fleetRunScope(input.runRef) ||
      !("runRef" in current) ||
      current.runRef !== input.runRef ||
      current.workUnitRef !== event.unitRef ||
      current.attemptRef !== event.workClaimRef ||
      current.assignmentRef !== attempt.assignmentRef ||
      current.workerId !== event.workerRef ||
      current.accountRefHash !== attempt.accountRefHash ||
      current.requestEventRef !== event.eventRef ||
      current.toolClass !== event.toolClass
    ) {
      throw fixedError(
        "idempotency_conflict",
        "fleet approval ref is already bound to another exact attempt",
        { runRef: input.runRef },
      )
    }
    return current
  }
  return decodeFleetApprovalEntity({
    approvalRef: event.approvalRef,
    status: "pending",
    runRef: input.runRef,
    workUnitRef: event.unitRef,
    attemptRef: event.workClaimRef,
    assignmentRef: attempt.assignmentRef,
    workerId: event.workerRef,
    accountRefHash: attempt.accountRefHash,
    requestEventRef: event.eventRef,
    toolClass: event.toolClass,
    openedAt: nowIso,
    updatedAt: nowIso,
  })
}

const insertTerminalCloseout = async (
  sql: SqlTag,
  runRef: string,
  event: FleetRunAnyWorkTerminalExecutionEvent,
): Promise<FleetRunWorkUnitCloseout> => {
  const closeout = closeoutForEvent(event)
  const proof = "assignmentRef" in closeout ? closeout : undefined
  await sql`
    INSERT INTO sarah_fleet_run_work_unit_closeouts
      (run_ref, unit_ref, work_claim_ref, assignment_ref, worker_kind,
       account_ref_hash, terminal_state, closeout_ref, usage_truth,
       token_usage_refs_json, blocker_refs_json, observed_at, event_ref)
    VALUES
      (${runRef}, ${closeout.unitRef}, ${closeout.workClaimRef},
       ${proof?.assignmentRef ?? null}, ${closeout.workerKind},
       ${proof?.accountRefHash ?? null}, ${closeout.terminalState},
       ${proof?.closeoutRef ?? null}, ${proof?.usageEvidence.truth ?? null},
       ${
         proof === undefined
           ? null
           : canonicalJson(proof.usageEvidence.tokenUsageRefs)
       },
       ${canonicalJson(closeout.blockerRefs)}, ${closeout.observedAt},
       ${closeout.eventRef})
  `
  return closeout
}

const appendFleetRunExecutionEvents = async (
  sql: SyncSql,
  input: FleetRunAuthorityAppendExecutionInput,
  nowMs: number,
): Promise<FleetRunAuthorityAppendExecutionResult> => {
  const nowIso = new Date(nowMs).toISOString()
  return withSyncTransaction(sql, async (writer) => {
    // Approval refs are global public identities even though their post-images
    // live in run-scoped Sync logs. Acquire every requested ref in one stable
    // global order before taking a run, lease, scope, or projection row lock.
    // This both closes the concurrent first-writer race and prevents two
    // cross-run batches with reverse approval order from deadlocking.
    const approvalRefs = [
      ...new Set(
        input.batch.events
          .filter((event) => event.kind === "approval_requested")
          .map((event) => event.approvalRef),
      ),
    ].sort()
    for (const approvalRef of approvalRefs) {
      await writer.sql`
        SELECT pg_advisory_xact_lock(hashtextextended(${approvalRef}, 0))
      `
    }
    await requireLinkedPylon(writer.sql, input)
    const runRows: Array<FleetRunRequestRow> = await writer.sql`
      SELECT * FROM sarah_fleet_run_requests
      WHERE run_ref = ${input.runRef}
        AND owner_user_id = ${input.ownerUserId}
      FOR UPDATE
    `
    const initialRow = runRows[0]
    if (initialRow === undefined) {
      throw fixedError(
        "run_not_found",
        "fleet run was not found for this owner",
        { runRef: input.runRef },
      )
    }
    const initialRecord = await recordFromRow(initialRow)
    await assertStoredWorkUnits(writer.sql, initialRecord)
    if (initialRecord.status !== "claimed_by_pylon") {
      throw fixedError(
        "claim_conflict",
        "fleet run has no accepted Pylon intake claim",
        { runRef: input.runRef, pylonRef: input.pylonRef },
      )
    }
    const leaseRows: Array<FleetRunLeaseRow> = await writer.sql`
      SELECT * FROM sarah_fleet_run_intake_leases
      WHERE run_ref = ${input.runRef}
        AND claim_ref = ${input.batch.claimRef}
        AND owner_user_id = ${input.ownerUserId}
        AND pylon_ref = ${input.pylonRef}
      FOR UPDATE
    `
    const lease = leaseRows[0]
    if (lease === undefined || lease.state !== "accepted") {
      throw fixedError(
        "claim_not_found",
        "accepted fleet run intake claim was not found",
        { runRef: input.runRef, pylonRef: input.pylonRef },
      )
    }

    const knownUnits = new Set(
      workUnitsFrom(initialRecord.request.workSource).map(
        (unit) => unit.unitRef,
      ),
    )
    const storedCloseoutRows: Array<FleetRunWorkUnitCloseoutRow> =
      await writer.sql`
        SELECT * FROM sarah_fleet_run_work_unit_closeouts
        WHERE run_ref = ${input.runRef}
        ORDER BY unit_ref, observed_at, work_claim_ref
      `
    const closeouts = storedCloseoutRows.map(closeoutFromRow)
    const closeoutsByClaim = new Map(
      closeouts.map((closeout) => [closeout.workClaimRef, closeout] as const),
    )
    const succeededUnitRows: Array<{ unit_ref: string }> = await writer.sql`
      SELECT unit_ref FROM sarah_fleet_run_work_units
      WHERE run_ref = ${input.runRef} AND state = 'succeeded'
    `
    const acceptedUnitRefs = new Set(
      succeededUnitRows.map((row) => row.unit_ref),
    )
    const storedWorkEventRows: Array<FleetRunExecutionEventRow> =
      await writer.sql`
        SELECT * FROM sarah_fleet_run_execution_events
        WHERE run_ref = ${input.runRef} AND work_claim_ref IS NOT NULL
        ORDER BY sequence
      `
    const claimBindings = new Map<
      string,
      Readonly<{
        unitRef: string
        workerKind: "codex" | "claude" | "grok"
      }>
    >()
    for (const row of storedWorkEventRows) {
      const event = executionEventFromRow(row)
      if (
        (event.kind !== "work_progress" &&
          event.kind !== "approval_requested" &&
          event.kind !== "work_terminal") ||
        row.owner_user_id !== input.ownerUserId ||
        row.pylon_ref !== input.pylonRef ||
        row.intake_claim_ref !== input.batch.claimRef
      ) {
        throw fixedError(
          "storage_unavailable",
          "fleet run execution attempt binding failed integrity validation",
          { runRef: input.runRef },
        )
      }
      const binding = claimBindings.get(event.workClaimRef)
      if (
        binding !== undefined &&
        (binding.unitRef !== event.unitRef ||
          binding.workerKind !== event.workerKind)
      ) {
        throw fixedError(
          "storage_unavailable",
          "fleet run execution attempt binding failed integrity validation",
          { runRef: input.runRef },
        )
      }
      claimBindings.set(event.workClaimRef, {
        unitRef: event.unitRef,
        workerKind: event.workerKind,
      })
    }
    let executionState = initialRecord.execution.state
    let acceptedThroughSequence = initialRecord.execution.lastSequence
    let executionStartedAt = initialRecord.execution.startedAt
    let storedEventCount = 0
    let duplicateEventCount = 0

    for (const event of input.batch.events) {
      await assertExecutionEventContentBinding(
        input.runRef,
        input.batch.claimRef,
        event,
      )
      const existingRows: Array<FleetRunExecutionEventRow> = await writer.sql`
        SELECT * FROM sarah_fleet_run_execution_events
        WHERE run_ref = ${input.runRef} AND sequence = ${event.sequence}
      `
      const existing = existingRows[0]
      if (existing !== undefined) {
        const stored = executionEventFromRow(existing)
        if (
          existing.owner_user_id !== input.ownerUserId ||
          existing.pylon_ref !== input.pylonRef ||
          existing.intake_claim_ref !== input.batch.claimRef ||
          canonicalJson(stored) !== canonicalJson(event)
        ) {
          throw fixedError(
            "idempotency_conflict",
            "fleet run execution sequence is already bound to another event",
            { runRef: input.runRef },
          )
        }
        duplicateEventCount += 1
        continue
      }
      const reusedRefRows: Array<FleetRunExecutionEventRow> = await writer.sql`
        SELECT * FROM sarah_fleet_run_execution_events
        WHERE event_ref = ${event.eventRef}
      `
      if (reusedRefRows[0] !== undefined) {
        throw fixedError(
          "idempotency_conflict",
          "fleet run execution event ref is already bound to another sequence",
          { runRef: input.runRef },
        )
      }
      const observedMs = Date.parse(event.observedAt)
      if (
        observedMs > nowMs + FLEET_RUN_EXECUTION_MAX_FUTURE_SKEW_MS ||
        observedMs <
          Date.parse(initialRecord.createdAt) -
            FLEET_RUN_EXECUTION_MAX_FUTURE_SKEW_MS
      ) {
        throw fixedError(
          "invalid_request",
          "fleet run execution observation clock failed freshness validation",
          { runRef: input.runRef },
        )
      }
      if (event.sequence !== acceptedThroughSequence + 1) {
        throw fixedError(
          "claim_conflict",
          "fleet run execution sequence is not contiguous",
          { runRef: input.runRef },
        )
      }
      if (
        executionState === "completed" ||
        executionState === "failed" ||
        executionState === "stopped"
      ) {
        throw fixedError(
          "claim_conflict",
          "fleet run execution is already terminal",
          { runRef: input.runRef },
        )
      }
      if (
        acceptedThroughSequence === 0 &&
        (event.sequence !== 1 || event.kind !== "run_started")
      ) {
        throw fixedError(
          "claim_conflict",
          "fleet run execution must begin with run_started",
          { runRef: input.runRef },
        )
      }
      if (event.kind === "run_started") {
        if (executionState !== "pending") {
          throw fixedError(
            "claim_conflict",
            "fleet run execution was already started",
            { runRef: input.runRef },
          )
        }
        executionState = "running"
        executionStartedAt = nowIso
      } else if (
        event.kind === "work_progress" ||
        event.kind === "approval_requested" ||
        event.kind === "work_terminal"
      ) {
        if (!knownUnits.has(event.unitRef)) {
          throw fixedError(
            "invalid_request",
            "fleet run execution event names an unknown work unit",
            { runRef: input.runRef },
          )
        }
        if (executionState !== "running") {
          throw fixedError(
            "claim_conflict",
            "fleet run execution is not running",
            { runRef: input.runRef },
          )
        }
        if (closeoutsByClaim.has(event.workClaimRef)) {
          throw fixedError(
            "idempotency_conflict",
            "fleet run work attempt already has terminal evidence",
            { runRef: input.runRef },
          )
        }
        if (acceptedUnitRefs.has(event.unitRef)) {
          throw fixedError(
            "idempotency_conflict",
            "fleet run work unit already has accepted terminal evidence",
            { runRef: input.runRef },
          )
        }
        const binding = claimBindings.get(event.workClaimRef)
        if (
          binding !== undefined &&
          (binding.unitRef !== event.unitRef ||
            binding.workerKind !== event.workerKind)
        ) {
          throw fixedError(
            "idempotency_conflict",
            "fleet run work claim is already bound to another attempt",
            { runRef: input.runRef },
          )
        }
        claimBindings.set(event.workClaimRef, {
          unitRef: event.unitRef,
          workerKind: event.workerKind,
        })
      }

      await insertExecutionEvent(writer.sql, input, event, nowIso)
      if (
        event.kind === "work_progress" ||
        event.kind === "approval_requested" ||
        event.kind === "work_terminal"
      ) {
        const projection = await projectAttemptForEvent(
          writer.sql,
          input,
          event,
          nowIso,
        )
        await appendFleetEntityChange(
          writer,
          input.runRef,
          {
            kind: "fleet_attempt",
            op: "upsert",
            entity: projection.attempt,
          },
          FLEET_RUN_AUTHORITY_EXECUTION_MUTATION_REF,
        )
        if (event.kind === "approval_requested") {
          const approval = await projectApprovalRequestedForEvent(
            writer.sql,
            input,
            event,
            projection.attempt,
            nowIso,
          )
          await appendFleetEntityChange(
            writer,
            input.runRef,
            {
              kind: "fleet_approval",
              op: "upsert",
              entity: approval,
            },
            FLEET_RUN_AUTHORITY_EXECUTION_MUTATION_REF,
          )
        }
        await appendFleetEntityChange(
          writer,
          input.runRef,
          {
            kind: "fleet_work_unit",
            op: "upsert",
            entity: projection.workUnit,
          },
          FLEET_RUN_AUTHORITY_EXECUTION_MUTATION_REF,
        )
      }
      if (event.kind === "work_terminal") {
        const closeout = await insertTerminalCloseout(
          writer.sql,
          input.runRef,
          event,
        )
        closeouts.push(closeout)
        closeoutsByClaim.set(closeout.workClaimRef, closeout)
        if (
          closeout.terminalState === "accepted" &&
          event.schema === FLEET_RUN_EXECUTION_EVENT_SCHEMA_V2
        ) {
          acceptedUnitRefs.add(closeout.unitRef)
        }
      } else if (event.kind === "run_terminal") {
        if (
          event.terminalState === "completed" &&
          acceptedUnitRefs.size !== knownUnits.size
        ) {
          throw fixedError(
            "claim_conflict",
            "fleet run cannot complete without accepted terminal evidence for every work unit",
            { runRef: input.runRef },
          )
        }
        executionState = event.terminalState
      }
      acceptedThroughSequence = event.sequence
      storedEventCount += 1
    }

    let currentRow = initialRow
    if (storedEventCount > 0) {
      const updatedRows: Array<FleetRunRequestRow> = await writer.sql`
        UPDATE sarah_fleet_run_requests
        SET execution_state = ${executionState},
            execution_last_sequence = ${acceptedThroughSequence},
            execution_started_at = ${executionStartedAt},
            execution_updated_at = ${nowIso},
            updated_at = ${nowIso}
        WHERE run_ref = ${input.runRef}
          AND owner_user_id = ${input.ownerUserId}
        RETURNING *
      `
      const updated = updatedRows[0]
      if (updated === undefined) {
        throw fixedError(
          "storage_unavailable",
          "fleet run execution projection was not persisted",
          { runRef: input.runRef },
        )
      }
      currentRow = updated
    }
    const record = await recordWithExecutionFromRow(writer.sql, currentRow)
    if (storedEventCount > 0) {
      const owner = await ensureScopeOwner(
        writer.sql,
        fleetRunScope(input.runRef),
        input.ownerUserId,
      )
      if (owner !== input.ownerUserId) {
        throw fixedError(
          "pylon_not_authorized",
          "fleet run scope is owned by another user",
          { runRef: input.runRef, pylonRef: input.pylonRef },
        )
      }
      await appendFleetEntityChange(
        writer,
        input.runRef,
        {
          kind: "fleet_run",
          op: "upsert",
          entity: fleetRunPostImage({
            runRef: input.runRef,
            state: executionStateForSync(record.execution.state),
            targetConcurrency: record.request.targetConcurrency,
            workerKind: record.request.workerPolicy.workerKind,
            startedAt: record.execution.startedAt,
            counters: {
              workUnitsTotal: record.execution.counters.workUnitsTotal,
              activeAssignments: record.execution.counters.activeAssignments,
              completedAssignments:
                record.execution.counters.acceptedAssignments,
              failedAssignments: record.execution.counters.failedAssignments,
              blockedAssignments: record.execution.counters.staleAssignments,
            },
            updatedAt: record.execution.updatedAt ?? nowIso,
          }),
        },
        FLEET_RUN_AUTHORITY_EXECUTION_MUTATION_REF,
      )
    }
    const ack = decodeUnknown(FleetRunExecutionAck, {
      schema: FLEET_RUN_EXECUTION_ACK_SCHEMA,
      runRef: input.runRef,
      claimRef: input.batch.claimRef,
      acceptedThroughSequence,
      storedEventCount,
      duplicateEventCount,
      execution: record.execution,
    })
    return { ack, record }
  })
}

export type MakeFleetRunAuthorityRepositoryOptions = Readonly<{
  sql: SyncSql
  now?: Effect.Effect<number>
}>

export const makeFleetRunAuthorityRepository = (
  options: MakeFleetRunAuthorityRepositoryOptions,
): FleetRunAuthorityRepositoryShape &
  Readonly<{
    appendExecutionEvents: NonNullable<
      FleetRunAuthorityRepositoryShape["appendExecutionEvents"]
    >
  }> => {
  const now = options.now ?? Clock.currentTimeMillis
  const start = Effect.fn("FleetRunAuthorityRepository.start")(
    (rawInput: unknown) =>
      Effect.gen(function* () {
        const input = yield* Effect.try({
          try: () => {
            const decoded = decodeUnknown(FleetRunAuthorityStartInput, rawInput)
            return {
              ownerUserId: decoded.ownerUserId,
              request: decodeFleetRunAuthorityStartRequest(decoded.request),
            }
          },
          catch: authorityErrorFromUnknown,
        })
        const nowMs = yield* now
        return yield* Effect.tryPromise({
          try: () =>
            createFleetRun(options.sql, input, new Date(nowMs).toISOString()),
          catch: authorityErrorFromUnknown,
        })
      }),
  )
  const claim = Effect.fn("FleetRunAuthorityRepository.claim")(
    (rawInput: unknown) =>
      Effect.gen(function* () {
        const input = yield* Effect.try({
          try: () => decodeUnknown(FleetRunAuthorityClaimInput, rawInput),
          catch: authorityErrorFromUnknown,
        })
        const nowMs = yield* now
        return yield* Effect.tryPromise({
          try: () => claimFleetRun(options.sql, input, nowMs),
          catch: authorityErrorFromUnknown,
        })
      }),
  )
  const observe = Effect.fn("FleetRunAuthorityRepository.observe")(
    (rawInput: unknown) =>
      Effect.gen(function* () {
        const input = yield* Effect.try({
          try: () => decodeUnknown(FleetRunAuthorityObserveInput, rawInput),
          catch: authorityErrorFromUnknown,
        })
        return yield* Effect.tryPromise({
          try: () => observeFleetRun(options.sql, input),
          catch: authorityErrorFromUnknown,
        })
      }),
  )
  const acceptClaim = Effect.fn("FleetRunAuthorityRepository.acceptClaim")(
    (rawInput: unknown) =>
      Effect.gen(function* () {
        const input = yield* Effect.try({
          try: () => decodeUnknown(FleetRunAuthorityAcceptClaimInput, rawInput),
          catch: authorityErrorFromUnknown,
        })
        const nowMs = yield* now
        return yield* Effect.tryPromise({
          try: () => acceptFleetRunClaim(options.sql, input, nowMs),
          catch: authorityErrorFromUnknown,
        })
      }),
  )
  const appendExecutionEvents = Effect.fn(
    "FleetRunAuthorityRepository.appendExecutionEvents",
  )((rawInput: unknown) =>
    Effect.gen(function* () {
      const input = yield* Effect.try({
        try: () => {
          const decoded = decodeUnknown(
            FleetRunAuthorityAppendExecutionInput,
            rawInput,
          )
          return {
            ownerUserId: decoded.ownerUserId,
            pylonRef: decoded.pylonRef,
            runRef: decoded.runRef,
            batch: decodeFleetRunExecutionBatch(decoded.batch),
          }
        },
        catch: authorityErrorFromUnknown,
      })
      const nowMs = yield* now
      return yield* Effect.tryPromise({
        try: () => appendFleetRunExecutionEvents(options.sql, input, nowMs),
        catch: authorityErrorFromUnknown,
      })
    }),
  )
  return { start, observe, claim, acceptClaim, appendExecutionEvents }
}

export const fleetRunAuthorityRepositoryLayer = (
  options: MakeFleetRunAuthorityRepositoryOptions,
) =>
  Layer.succeed(
    FleetRunAuthorityRepository,
    makeFleetRunAuthorityRepository(options),
  )

export const publicFleetRunAuthorityRecord = (
  record: FleetRunAuthorityRecord,
) => ({
  runRef: record.runRef,
  scope: record.scope,
  status: record.status,
  objective: record.request.objective,
  repository: record.request.repository,
  verifier: record.request.verifier,
  workSource: record.request.workSource,
  workerPolicy: record.request.workerPolicy,
  targetConcurrency: record.request.targetConcurrency,
  execution: record.execution,
  createdAt: record.createdAt,
  updatedAt: record.updatedAt,
  privateMaterialExcluded: true as const,
})
