import {
  canonicalJson,
  fleetRunScope,
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
export const FLEET_RUN_EXECUTION_ACK_SCHEMA =
  "openagents.pylon.fleet_run_execution_ack.v1" as const
export const FLEET_RUN_AUTHORITY_CREATE_MUTATION_REF =
  "system:sarah_fleet_run_authority.create.v1" as const
export const FLEET_RUN_AUTHORITY_EXECUTION_MUTATION_REF =
  "system:sarah_fleet_run_authority.execution.v1" as const
export const FLEET_RUN_PYLON_FRESHNESS_MS = 5 * 60 * 1_000

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
const ExecutionEventRef = S.Trim.check(
  S.isMinLength(32),
  S.isMaxLength(180),
  S.isPattern(/^event\.pylon\.fleet_run\.[A-Za-z0-9_.:-]+$/u),
)
const ExecutionSequence = S.Int.check(
  S.isGreaterThanOrEqualTo(1),
  S.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)
const ExecutionPublicRef = S.Trim.check(
  S.isMinLength(1),
  S.isMaxLength(180),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:/#-]*$/u),
)
const AccountRefHash = S.Trim.check(
  S.isPattern(/^account\.pylon\.(?:codex|claude_agent|grok)\.[a-f0-9]{6,64}$/u),
)
const BlockerRef = S.Trim.check(
  S.isMinLength(3),
  S.isMaxLength(180),
  S.isPattern(/^blocker\.[A-Za-z0-9][A-Za-z0-9._:/#-]*$/u),
)

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
  blockerRefs: S.Array(BlockerRef),
})
const FleetRunWorkTerminalExecutionEventBase = S.Struct({
  ...FleetRunExecutionEventBase.fields,
  kind: S.Literal("work_terminal"),
  unitRef: PlanUnitRef,
  workClaimRef: ExecutionPublicRef,
  workerKind: S.Literals(["codex", "claude", "grok"]),
  blockerRefs: S.Array(BlockerRef),
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
  FleetRunUnprovenWorkTerminalExecutionEvent,
  FleetRunProvenFailedWorkTerminalExecutionEvent,
])
export type FleetRunWorkTerminalExecutionEvent =
  typeof FleetRunWorkTerminalExecutionEvent.Type
export const FleetRunTerminalExecutionEvent = S.Struct({
  ...FleetRunExecutionEventBase.fields,
  kind: S.Literal("run_terminal"),
  terminalState: S.Literals(["completed", "failed", "stopped"]),
  blockerRefs: S.Array(BlockerRef),
})
export const FleetRunExecutionEvent = S.Union([
  FleetRunStartedExecutionEvent,
  FleetRunWorkProgressExecutionEvent,
  FleetRunWorkTerminalExecutionEvent,
  FleetRunTerminalExecutionEvent,
])
export type FleetRunExecutionEvent = typeof FleetRunExecutionEvent.Type

export const FleetRunExecutionBatch = S.Struct({
  schema: S.Literal(FLEET_RUN_EXECUTION_BATCH_SCHEMA),
  claimRef: TrimmedClaimRef,
  events: S.Array(FleetRunExecutionEvent),
})
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
  blockerRefs: S.Array(BlockerRef),
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
  FleetRunUnprovenWorkUnitCloseout,
  FleetRunProvenFailedWorkUnitCloseout,
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
    if (event.blockerRefs.length > 32) {
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
    if (event.kind === "work_progress") {
      if (
        (event.accountRefHash !== undefined &&
          !accountHashMatchesWorker(event.workerKind, event.accountRefHash)) ||
        event.blockerRefs.length > 32
      ) {
        throw invalidRequest()
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
  S.fromJsonString(S.Array(ExecutionPublicRef)),
)
const blockerRefsFromJson = S.decodeUnknownSync(
  S.fromJsonString(S.Array(BlockerRef)),
)

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
    ORDER BY unit_ref
  `
  const closeouts = rows.map(closeoutFromRow)
  const activeRows: Array<{ count: string | number | bigint }> = await sql`
    SELECT count(DISTINCT event.unit_ref) AS count
    FROM sarah_fleet_run_execution_events AS event
    LEFT JOIN sarah_fleet_run_work_unit_closeouts AS closeout
      ON closeout.run_ref = event.run_ref AND closeout.unit_ref = event.unit_ref
    WHERE event.run_ref = ${record.runRef}
      AND event.event_kind = 'work_progress'
      AND closeout.unit_ref IS NULL
  `
  const activeAssignments = safeStoredSequence(activeRows[0]?.count ?? 0)
  return decodeUnknown(FleetRunExecutionProjection, {
    state: record.execution.state,
    lastSequence: record.execution.lastSequence,
    counters: {
      workUnitsTotal: workUnitsFrom(record.request.workSource).length,
      activeAssignments,
      acceptedAssignments: closeouts.filter(
        (closeout) => closeout.terminalState === "accepted",
      ).length,
      failedAssignments: closeouts.filter(
        (closeout) => closeout.terminalState === "failed",
      ).length,
      staleAssignments: closeouts.filter(
        (closeout) => closeout.terminalState === "stale",
      ).length,
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
           depends_on_refs_json)
        VALUES
          (${record.runRef}, ${record.ownerUserId}, ${unitIndex},
           ${unit.unitRef}, ${unit.issueRef}, ${unit.title},
           ${canonicalJson(unit.dependsOn)})
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
        (event.kind === "work_progress" || event.kind === "work_terminal"
          ? event.unitRef
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
  event: FleetRunWorkTerminalExecutionEvent,
): FleetRunWorkUnitCloseout =>
  decodeUnknown(FleetRunWorkUnitCloseout, {
    unitRef: event.unitRef,
    workClaimRef: event.workClaimRef,
    workerKind: event.workerKind,
    terminalState: event.terminalState,
    ...("assignmentRef" in event
      ? {
          assignmentRef: event.assignmentRef,
          accountRefHash: event.accountRefHash,
          closeoutRef: event.closeoutRef,
          usageEvidence: event.usageEvidence,
        }
      : {}),
    blockerRefs: event.blockerRefs,
    observedAt: event.observedAt,
    eventRef: event.eventRef,
  })

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
    event.kind === "work_progress" || event.kind === "work_terminal"
      ? event.unitRef
      : null
  await sql`
    INSERT INTO sarah_fleet_run_execution_events
      (run_ref, sequence, event_ref, owner_user_id, pylon_ref,
       intake_claim_ref, event_kind, unit_ref, event_json, observed_at,
       recorded_at)
    VALUES
      (${input.runRef}, ${event.sequence}, ${event.eventRef},
       ${input.ownerUserId}, ${input.pylonRef}, ${input.batch.claimRef},
       ${event.kind}, ${unitRef}, ${canonicalJson(event)},
       ${event.observedAt}, ${nowIso})
  `
}

const insertTerminalCloseout = async (
  sql: SqlTag,
  runRef: string,
  event: FleetRunWorkTerminalExecutionEvent,
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
        ORDER BY unit_ref
      `
    const closeouts = new Map(
      storedCloseoutRows.map((row) => {
        const closeout = closeoutFromRow(row)
        return [closeout.unitRef, closeout] as const
      }),
    )
    let executionState = initialRecord.execution.state
    let acceptedThroughSequence = initialRecord.execution.lastSequence
    let executionStartedAt = initialRecord.execution.startedAt
    let storedEventCount = 0
    let duplicateEventCount = 0

    for (const event of input.batch.events) {
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
        executionStartedAt = event.observedAt
      } else if (
        event.kind === "work_progress" ||
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
        if (closeouts.has(event.unitRef)) {
          throw fixedError(
            "idempotency_conflict",
            "fleet run work unit already has terminal evidence",
            { runRef: input.runRef },
          )
        }
      }

      await insertExecutionEvent(writer.sql, input, event, nowIso)
      if (event.kind === "work_terminal") {
        closeouts.set(
          event.unitRef,
          await insertTerminalCloseout(writer.sql, input.runRef, event),
        )
      } else if (event.kind === "run_terminal") {
        if (
          event.terminalState === "completed" &&
          (closeouts.size !== knownUnits.size ||
            [...closeouts.values()].some(
              (closeout) => closeout.terminalState !== "accepted",
            ))
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
