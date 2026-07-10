import type { Database } from "bun:sqlite"
import { createHash } from "node:crypto"
import { existsSync } from "node:fs"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { Schema as S } from "effect"
import {
  PYLON_AGENT_RUNNER_CONTROL_VERBS,
  PYLON_AGENT_RUNNER_STATUS_EVENT_SCHEMA_VERSION,
  type AgentRunnerControlVerb,
  type AgentRunnerNeutralState,
  type AgentRunnerStatusEvent,
  type AgentRunnerStatusHistoryEntry,
} from "../agent-status-reporter.js"
import {
  PYLON_DISPATCH_BREAKER_SCHEMA,
  classifyPylonDispatchFailure,
  normalizePylonDispatchFailureLane,
  pylonDispatchBreakerIsActive,
  pylonDispatchBreakerScopeKey,
  type PylonDispatchBreakerSnapshot,
  type PylonDispatchFailureClassification,
  type PylonDispatchFailureInput,
  type PylonDispatchFailureLane,
} from "../dispatch-failure-taxonomy.js"
import {
  FleetRunWorkSourceDescriptorSchema,
  decodeFleetRunWorkSourceDescriptor,
  type FleetRunWorkSourceDescriptor,
} from "./fleet-run-work-source.js"

export const ORCHESTRATION_SCHEMA_VERSION = 12

export type OrchestrationTaskStatus =
  | "pending"
  | "ready"
  | "dispatched"
  | "completed"
  | "failed"
  | "blocked"

export type OrchestrationTaskSpec = {
  title: string
  prompt: string
  runnerKind?: OrchestrationRunnerKind
  verifyCommand?: string
  repo?: string
  branch?: string
  baseCommit?: string
  issueRef?: string
  fleetRunRef?: string
}

export type OrchestrationTask = {
  id: string
  parentId: string | null
  threadId: string
  spec: OrchestrationTaskSpec
  status: OrchestrationTaskStatus
  deps: string[]
  result: string | null
  createdAt: string
  updatedAt: string
}

export type PublicOrchestrationTask = {
  id: string
  parentId: string | null
  threadId: string
  status: OrchestrationTaskStatus
  deps: string[]
  runnerKind: OrchestrationRunnerKind | null
  repo: string | null
  branch: string | null
  baseCommit: string | null
  issueRef: string | null
  fleetRunRef: string | null
  createdAt: string
  updatedAt: string
}

export const FLEET_RUN_SCHEMA = "openagents.khala_code.fleet_run.v1" as const
export const FLEET_RUN_OWNER_LOCAL_STATE_SCHEMA = "openagents.khala_code.fleet_runs.owner_local.v1" as const
export const FLEET_RUN_AUTHORITY_BINDING_SCHEMA =
  "openagents.pylon.fleet_run_authority_binding.v1" as const
export const WORK_CLAIM_SCHEMA = "openagents.khala_code.work_claim.v1" as const

export const FleetRunWorkSourceSchema = S.Literals(["github_backlog", "issue_list", "fixture", "plan_dag"])
export type FleetRunWorkSource = typeof FleetRunWorkSourceSchema.Type

export const FleetRunWorkerKindSchema = S.Literals(["codex", "claude", "grok", "auto"])
export type FleetRunWorkerKind = typeof FleetRunWorkerKindSchema.Type

export const FleetRunDispatchKindSchema = S.Literals(["handoff", "supervised_dispatch"])
export type FleetRunDispatchKind = typeof FleetRunDispatchKindSchema.Type

export const FleetRunStateSchema = S.Literals([
  "draft",
  "running",
  "paused",
  "draining",
  "stopped",
  "completed",
])
export type FleetRunState = typeof FleetRunStateSchema.Type

export const FleetRunControlVerbSchema = S.Literals(["pause", "resume", "drain", "stop"])
export type FleetRunControlVerb = typeof FleetRunControlVerbSchema.Type

export const FleetRunStopConditionSchema = S.Literals(["backlog_empty", "target_reached", "manual_stop"])
export type FleetRunStopCondition = typeof FleetRunStopConditionSchema.Type

export type AgentRunnerKind = "claude_agent" | "codex"
export type LegacyAgentRunnerKind = "claude"

export const FleetRunRefillPolicySchema = S.Struct({
  maxPerAccount: S.Number,
  cooldownAware: S.Boolean,
  stopCondition: FleetRunStopConditionSchema,
})
export type FleetRunRefillPolicy = typeof FleetRunRefillPolicySchema.Type

export const FleetRunCountersSchema = S.Struct({
  workUnitsTotal: S.Number,
  activeAssignments: S.Number,
  completedAssignments: S.Number,
  failedAssignments: S.Number,
  blockedAssignments: S.Number,
})
export type FleetRunCounters = typeof FleetRunCountersSchema.Type

/**
 * Durable import/accept journal for one server-authoritative Sarah FleetRun.
 *
 * This is metadata on the canonical local run record, not another run or work
 * claim registry. The server remains authoritative for its intake lease and
 * the existing `pylon_orchestration_work_claims` table remains authoritative
 * for executable work-unit claims. Keeping the last server claim ref here is
 * what makes an accept response lost across process restart safely replayable.
 */
export const FleetRunAuthorityBindingSchema = S.Struct({
  schema: S.Literal(FLEET_RUN_AUTHORITY_BINDING_SCHEMA),
  source: S.Literal("sarah_authority"),
  authorityFingerprint: S.String.check(S.isPattern(/^[0-9a-f]{64}$/u)),
  claimRef: S.String.check(
    S.isPattern(/^claim\.sarah_fleet_run\.[0-9a-f]{24}$/u),
  ),
  pylonRef: S.String.check(
    S.isPattern(/^[a-z0-9][a-z0-9._:-]{2,119}$/u),
  ),
  targetPreference: S.Literals(["owner_local", "auto"]),
  phase: S.Literals(["imported", "accepted"]),
})
export type FleetRunAuthorityBinding =
  typeof FleetRunAuthorityBindingSchema.Type

export const FleetRunSchema = S.Struct({
  schema: S.Literal(FLEET_RUN_SCHEMA),
  runRef: S.String,
  objective: S.String,
  workSource: FleetRunWorkSourceSchema,
  workSourceDescriptor: S.optional(FleetRunWorkSourceDescriptorSchema),
  authorityBinding: S.optional(FleetRunAuthorityBindingSchema),
  targetConcurrency: S.Number,
  workerKind: FleetRunWorkerKindSchema,
  refillPolicy: FleetRunRefillPolicySchema,
  state: FleetRunStateSchema,
  stateSource: S.optional(S.Literals(["operator", "reconcile"])),
  dispatchKind: FleetRunDispatchKindSchema,
  dagTracked: S.Boolean,
  startedAt: S.NullOr(S.String),
  counters: FleetRunCountersSchema,
  createdAt: S.String,
  updatedAt: S.String,
})
export type FleetRun = typeof FleetRunSchema.Type

/**
 * Explicit owner-local permission for the standing node to keep one durable
 * FleetRun active across node restarts. This row deliberately contains refs
 * and the armed bit only: transport config, credentials, local paths, prompts,
 * and executor diagnostics never enter the orchestration authority.
 */
export type FleetRunActivation = {
  pylonRef: string
  runRef: string
  armed: boolean
}

/**
 * One public-safe execution event waiting for acknowledgement from the
 * server-authoritative Sarah FleetRun projection.
 *
 * The local SQLite sequence is the replay cursor. `eventJson` is already
 * schema-encoded by the FleetRun execution reporter; this store treats it as
 * opaque bytes while binding it to the exact accepted intake claim. A caller
 * cannot enqueue against an unaccepted or different authority binding.
 */
export type FleetRunExecutionOutboxEntry = {
  runRef: string
  claimRef: string
  sequence: number
  eventRef: string
  eventJson: string
  deliveryBatchRef: string | null
  createdAt: string
  deliveredAt: string | null
}

export type EnqueueFleetRunExecutionOutboxInput = {
  runRef: string
  claimRef: string
  eventRef: string
  /** Encode the final schema event after the store assigns its gapless sequence. */
  eventJsonForSequence: (sequence: number) => string
  now?: Date
}

export type ReserveFleetRunExecutionOutboxBatchInput = {
  runRef: string
  claimRef: string
  firstSequence: number
  lastSequence: number
  deliveryBatchRef: string
}

export type FleetRunSteeringOutcomeStatus =
  | "applied"
  | "queued_follow_up"
  | "skipped_stale"
  | "rejected"
  | "failed"

/**
 * Durable, public-safe receipt for one Sarah steering intent applied by this
 * accepted Pylon. The raw intent and steer body deliberately do not live in
 * this row: only their content digest crosses the exactly-once boundary.
 */
export type FleetRunSteeringOutcomeRecord = {
  readonly pylonRef: string
  readonly runRef: string
  readonly claimRef: string
  readonly seq: number
  readonly intentId: string
  readonly intentKind: string
  readonly intentDigest: string
  readonly outcome: FleetRunSteeringOutcomeStatus
  readonly outcomeRef: string
  readonly observedAt: string
}

export type FleetRunSteeringOutcomeOutboxEntry =
  FleetRunSteeringOutcomeRecord & {
    readonly deliveredAt: string | null
  }

export type FleetRunSteeringQueuedFollowUp = {
  readonly pylonRef: string
  readonly runRef: string
  readonly claimRef: string
  readonly seq: number
  readonly intentId: string
  readonly workUnitRef: string | null
  readonly workClaimRef: string | null
  readonly assignmentRef: string | null
  readonly targetRef: string | null
  readonly intentKind: "fleet_run_control" | "approval_decision" | "steer_message"
  readonly approvalRef: string | null
  readonly decision: "allow" | "deny" | null
  readonly residualRefs: readonly string[]
  /** Owner-private local material. Never project this through an ACK. */
  readonly body: string | null
  readonly bodyRef: string | null
  readonly createdAt: string
  readonly state: "queued" | "dispatching" | "applied" | "failed" | "stale"
  readonly attemptCount: number
  readonly nextAttemptAt: string
  readonly lastAttemptAt: string | null
  readonly dispatchLeaseExpiresAt: string | null
  readonly dispatchLeaseToken: string | null
  readonly leaseGeneration: number
  readonly lastFailureRef: string | null
  readonly completionRef: string | null
  readonly completedAt: string | null
}

export type FleetRunSteeringApprovalBinding = {
  readonly approvalRef: string
  readonly pylonRef: string
  readonly runRef: string
  readonly claimRef: string
  readonly workUnitRef: string
  readonly workClaimRef: string
  readonly assignmentRef: string
  /** Null only for bindings written before schema v11; such rows are never projected. */
  readonly workerKind: "codex" | "claude" | "grok" | null
  /** Stable public-safe worker identity, distinct from the private account ref. */
  readonly workerRef: string | null
  readonly accountRefHash: string | null
  readonly toolClass: string | null
  readonly state: "pending" | "resolved"
  readonly decision: "allow" | "deny" | null
  readonly resolutionState: "applied" | "failed" | "stale" | null
  readonly createdAt: string
  readonly resolvedAt: string | null
  readonly completionRef: string | null
}

/** Body-free terminal receipt. Owner-private steer bodies never enter this row. */
export type FleetRunSteeringFollowUpCompletion = {
  readonly pylonRef: string
  readonly runRef: string
  readonly claimRef: string
  readonly seq: number
  readonly intentId: string
  readonly intentKind: FleetRunSteeringQueuedFollowUp["intentKind"]
  readonly state: "applied" | "failed" | "stale"
  readonly workUnitRef: string | null
  readonly workClaimRef: string | null
  readonly assignmentRef: string | null
  readonly approvalRef: string | null
  readonly completionRef: string
  readonly completedAt: string
  readonly failureRef: string | null
  readonly deliveredAt: string | null
}

export type ApplyFleetRunSteeringIntentInput = {
  readonly pylonRef: string
  readonly runRef: string
  readonly claimRef: string
  readonly seq: number
  readonly intentId: string
  readonly intentKind: string
  readonly intentDigest: string
  readonly observedAt: Date
  readonly outcomeRefFor: (
    outcome: FleetRunSteeringOutcomeStatus,
    observedAt: string,
  ) => string
}

export type FleetRunSteeringApplication = {
  readonly outcome: FleetRunSteeringOutcomeStatus
  readonly queuedFollowUp?: Omit<
    FleetRunSteeringQueuedFollowUp,
    | "pylonRef"
    | "runRef"
    | "claimRef"
    | "seq"
    | "intentId"
    | "createdAt"
    | "state"
    | "attemptCount"
    | "nextAttemptAt"
    | "lastAttemptAt"
    | "dispatchLeaseExpiresAt"
    | "dispatchLeaseToken"
    | "leaseGeneration"
    | "lastFailureRef"
    | "completionRef"
    | "completedAt"
  > | undefined
}

export const FleetRunOwnerLocalStateSchema = S.Struct({
  schema: S.Literal(FLEET_RUN_OWNER_LOCAL_STATE_SCHEMA),
  runs: S.Array(FleetRunSchema),
})
export type FleetRunOwnerLocalState = typeof FleetRunOwnerLocalStateSchema.Type

export const WorkClaimStateSchema = S.Literals([
  "claimed",
  "in_progress",
  "closeout",
  "released",
  "expired",
])
export type WorkClaimState = typeof WorkClaimStateSchema.Type
export type LiveWorkClaimState = Extract<WorkClaimState, "claimed" | "in_progress" | "closeout">
export const WorkClaimMarginalCostClassSchema = S.Literals([
  "free",
  "subscription",
  "api_metered",
  "not_measured",
])
export type WorkClaimMarginalCostClass =
  typeof WorkClaimMarginalCostClassSchema.Type

export const WorkClaimSchema = S.Struct({
  schema: S.Literal(WORK_CLAIM_SCHEMA),
  claimRef: S.String,
  workUnitRef: S.String,
  runRef: S.String,
  assignmentRef: S.NullOr(S.String),
  workerAccountRef: S.String,
  marginalCostClass: S.optional(WorkClaimMarginalCostClassSchema),
  state: WorkClaimStateSchema,
  ttl: S.Number,
  claimedAt: S.String,
  expiresAt: S.String,
  updatedAt: S.String,
})
export type WorkClaim = typeof WorkClaimSchema.Type

export type CreateWorkClaimInput = {
  claimRef: string
  workUnitRef: string
  runRef: string
  assignmentRef?: string | null
  workerAccountRef: string
  marginalCostClass?: WorkClaimMarginalCostClass | undefined
  ttl: number
  now?: Date
}

export type ReconcileWorkClaimsInput = {
  now?: Date
  workerHeartbeatTtlMs?: number
}

/**
 * Per-intent enforcement outcome vocabulary (KS-3.2 #8332):
 *
 * - `applied` — the intent changed (or already matched) supervisor state
 * - `skipped_stale` — honestly not applicable anymore (unknown run/worker,
 *   terminal run, transition no longer valid); recorded, never retried
 * - `failed` — the application step itself errored; recorded with a bounded
 *   public-safe detail so the loop is never wedged by one bad intent
 */
export type FleetIntentOutcomeStatus = "applied" | "skipped_stale" | "failed"

/**
 * One durably recorded fleet-intent application outcome. `intentId` is the
 * monotonic `khala_sync_fleet_intents.id`; the primary key on it is the
 * exactly-once guard under route redelivery.
 */
export type FleetIntentOutcomeRecord = {
  intentId: number
  scope: string
  runRef: string
  intent: string
  outcome: FleetIntentOutcomeStatus
  detail: string | null
  mutationRef: string
  recordedAt: string
}

/**
 * Runtime control-intent dispatch outcome (#8388) — the exactly-once guard
 * for the Pylon-side `runtime.*` dispatch consumer
 * (`./runtime-intent-enforcement.ts`), mirroring
 * `FleetIntentOutcomeStatus` above. `intentId` is the client-minted
 * `KhalaRuntimeControlIntent.intentId` (a text ref, not a numeric id —
 * unlike fleet intents, the resumable ordering key is the separate `seq`
 * watermark).
 */
export type RuntimeIntentOutcomeStatus = "applied" | "skipped_stale" | "failed"

export type RuntimeIntentOutcomeRecord = {
  intentId: string
  threadId: string
  turnId: string | null
  kind: string
  outcome: RuntimeIntentOutcomeStatus
  detail: string | null
  recordedAt: string
}

export type RecordRuntimeIntentOutcomeInput = {
  intentId: string
  threadId: string
  turnId?: string | null
  kind: string
  outcome: RuntimeIntentOutcomeStatus
  detail?: string | null
  now?: Date
}

export type RecordFleetIntentOutcomeInput = {
  intentId: number
  scope: string
  runRef: string
  intent: string
  outcome: FleetIntentOutcomeStatus
  detail?: string | null
  mutationRef: string
  now?: Date
}

export type CreateFleetRunInput = {
  runRef: string
  objective: string
  workSource: FleetRunWorkSource
  workSourceDescriptor?: FleetRunWorkSourceDescriptor
  authorityBinding?: FleetRunAuthorityBinding
  targetConcurrency: number
  workerKind: FleetRunWorkerKind
  refillPolicy?: Partial<FleetRunRefillPolicy>
  state?: FleetRunState
  dispatchKind?: FleetRunDispatchKind
  startedAt?: Date | string | null
  counters?: Partial<FleetRunCounters>
  now?: Date
}

export type VirtualHead = {
  repo: string
  branch: string
  baseCommit: string
  projectedHead: string
  pendingTaskIds: string[]
  createdAt: string
  updatedAt: string
}

export type VirtualHeadReservation = {
  repo: string
  branch: string
  taskId: string
  branchFrom: string
  projectedHead: string
  pendingTaskIds: string[]
}

export type DispatchContextStatus =
  | "idle"
  | "dispatched"
  | "completed"
  | "failed"
  | "blocked"
  | "circuit_broken"

// `grok_cli` is the Axis-B coding-worker harness kind (MH-4). It is not an
// AgentRunnerKind in the codex/claude Pylon registry — Grok workers dispatch
// via the desktop/local headless executor — but durable fleet contexts and
// task specs still need to label the runner honestly.
export type OrchestrationRunnerKind = AgentRunnerKind | "generic" | "grok_cli"
type StoredRunnerKind = OrchestrationRunnerKind | LegacyAgentRunnerKind

export type DispatchContext = {
  id: string
  assigneeHandle: string
  runnerKind: OrchestrationRunnerKind
  lane: PylonDispatchFailureLane
  accountRefHash: string | null
  worktreeId: string | null
  worktreePath: string | null
  status: DispatchContextStatus
  currentTaskId: string | null
  failureCount: number
  lastHeartbeatAt: string | null
  baseBehindBy: number
  maxConcurrentSlots: number
  /**
   * Operator-level slot gate (KS-3.2 #8332): a paused context is refused by
   * `dispatchEligibility` until an operator resumes it. Enforced from the
   * durable `pause_worker` / `resume_worker` fleet intents.
   */
  paused: boolean
  createdAt: string
  updatedAt: string
}

export type PublicDispatchContext = Omit<DispatchContext, "worktreePath"> & {
  worktreePath: null
}

export type AgentRunnerStatusRetentionState = "live" | "retained"

export type OrchestrationAgentRunnerStatusEntry = AgentRunnerStatusEvent & {
  schemaVersion: typeof PYLON_AGENT_RUNNER_STATUS_EVENT_SCHEMA_VERSION
  retentionState: AgentRunnerStatusRetentionState
  retainedAt: string | null
}

export type IngestAgentRunnerStatusEventInput = {
  event: AgentRunnerStatusEvent
  now?: Date
  historyLimit?: number
}

export type DecayAgentRunnerStatusesInput = {
  now?: Date
  staleAfterMs?: number
}

export type OrchestrationMessageKind =
  | "dispatch"
  | "worker_done"
  | "heartbeat"
  | "escalation"
  | "decision_gate"

export type OrchestrationMessage = {
  id: string
  threadId: string
  taskId: string | null
  dispatchContextId: string | null
  kind: OrchestrationMessageKind
  body: string
  createdAt: string
}

export type CreateTaskInput = {
  id: string
  parentId?: string | null
  threadId?: string
  spec: OrchestrationTaskSpec
  deps?: readonly string[]
  status?: OrchestrationTaskStatus
  now?: Date
}

export type CreateDispatchContextInput = {
  id: string
  assigneeHandle: string
  runnerKind?: OrchestrationRunnerKind | LegacyAgentRunnerKind
  lane?: PylonDispatchFailureLane
  accountRefHash?: string | null
  worktreeId?: string | null
  worktreePath?: string | null
  maxConcurrentSlots?: number
  lastHeartbeatAt?: Date | null
  baseBehindBy?: number
  now?: Date
}

export type RecordWorkerHeartbeatInput = {
  contextId: string
  taskId?: string | null
  at?: Date
  baseBehindBy?: number
  status?: DispatchContextStatus
  body?: string
}

export type RecordWorkerDoneInput = {
  contextId: string
  taskId: string
  status: Extract<OrchestrationTaskStatus, "completed" | "failed" | "blocked">
  result?: string | null
  body?: string
  failure?: PylonDispatchFailureInput
  maxFailures?: number
  now?: Date
}

type SqliteDatabase = Pick<Database, "exec" | "query" | "run">
type PylonHomePaths = { home: string }

const iso = (date: Date = new Date()): string => date.toISOString()

const DEFAULT_FLEET_RUN_COUNTERS: FleetRunCounters = {
  workUnitsTotal: 0,
  activeAssignments: 0,
  completedAssignments: 0,
  failedAssignments: 0,
  blockedAssignments: 0,
}

const DEFAULT_FLEET_RUN_REFILL_POLICY: FleetRunRefillPolicy = {
  maxPerAccount: 1,
  cooldownAware: true,
  stopCondition: "backlog_empty",
}

const FLEET_RUN_ACTIVATION_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,180}$/u
const FLEET_RUN_PROJECTED_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,179}$/u
const FLEET_RUN_APPROVAL_TOOL_CLASS_PATTERN = /^[a-z][a-z0-9_]{0,63}$/u
const FLEET_RUN_ACCOUNT_REF_HASH_PATTERN =
  /^account\.pylon\.(?:codex|claude_agent|grok)\.[a-f0-9]{24}$/u
const FLEET_RUN_EXECUTION_EVENT_REF_PATTERN =
  /^event\.pylon\.fleet_run\.[a-f0-9]{24}$/u
const FLEET_RUN_EXECUTION_BATCH_REF_PATTERN =
  /^batch\.pylon\.fleet_run\.[a-f0-9]{24}$/u
const FLEET_RUN_EXECUTION_EVENT_MAX_BYTES = 64 * 1_024

const parseJsonArray = (value: string): string[] => {
  const parsed: unknown = JSON.parse(value)
  return Array.isArray(parsed) ? parsed.filter((entry): entry is string => typeof entry === "string") : []
}

const parseControlVerbs = (value: string): AgentRunnerControlVerb[] =>
  parseJsonArray(value).filter((verb): verb is AgentRunnerControlVerb =>
    PYLON_AGENT_RUNNER_CONTROL_VERBS.includes(verb as AgentRunnerControlVerb),
  )

const parseStateHistory = (value: string): AgentRunnerStatusHistoryEntry[] => {
  const parsed: unknown = JSON.parse(value)
  if (!Array.isArray(parsed)) return []
  return parsed.flatMap((entry): AgentRunnerStatusHistoryEntry[] => {
    if (typeof entry !== "object" || entry === null) return []
    const candidate = entry as Partial<AgentRunnerStatusHistoryEntry>
    return typeof candidate.state === "string" && typeof candidate.stateStartedAt === "string"
      ? [{ state: candidate.state as AgentRunnerNeutralState, stateStartedAt: candidate.stateStartedAt }]
      : []
  })
}

const parsePendingTaskIds = (value: string): string[] => [...new Set(parseJsonArray(value))]

const parseSpec = (value: string): OrchestrationTaskSpec => {
  const parsed: unknown = JSON.parse(value)
  if (typeof parsed !== "object" || parsed === null) throw new Error("invalid orchestration task spec")
  const spec = parsed as Omit<OrchestrationTaskSpec, "runnerKind"> & { runnerKind?: StoredRunnerKind }
  const { runnerKind, ...rest } = spec
  return {
    ...rest,
    ...(runnerKind === undefined ? {} : { runnerKind: normalizeOrchestrationRunnerKind(runnerKind) }),
  }
}

const normalizeTaskSpec = (spec: OrchestrationTaskSpec): OrchestrationTaskSpec => ({
  ...spec,
  ...(spec.runnerKind === undefined ? {} : { runnerKind: normalizeOrchestrationRunnerKind(spec.runnerKind) }),
})

const assertWholePositive = (field: string, value: number): void => {
  if (!Number.isInteger(value) || value < 1) throw new Error(`fleet run ${field} must be a positive integer`)
}

const assertWholeNonNegative = (field: string, value: number): void => {
  if (!Number.isInteger(value) || value < 0) throw new Error(`fleet run ${field} must be a non-negative integer`)
}

const assertNonEmpty = (kind: string, field: string, value: string): void => {
  if (!value.trim()) throw new Error(`${kind} ${field} is required`)
}

export const assertFleetRunControlTransition = (
  state: FleetRunState,
  verb: FleetRunControlVerb,
): void => {
  const allowed: Record<FleetRunControlVerb, readonly FleetRunState[]> = {
    drain: ["running", "paused"],
    pause: ["running"],
    resume: ["paused"],
    stop: ["draft", "running", "paused", "draining"],
  }
  if (!allowed[verb].includes(state)) {
    throw new Error(`fleetRunControl cannot ${verb} a ${state} fleet run`)
  }
}

/**
 * A closed fleet run may be automatically reopened by the supervisor only
 * when reconciliation closed it (all created tasks terminal while the
 * planner backlog still holds uncreated units). Operator lifecycle
 * decisions — pause, drain, stop — are authority and are never auto-revived.
 * Legacy rows without a stateSource keep the historical auto-revive
 * behavior for completed/stopped states.
 */
export const isAutoRevivableFleetRun = (
  run: Pick<FleetRun, "state" | "stateSource">,
): boolean =>
  (run.state === "completed" || run.state === "stopped") && run.stateSource !== "operator"

const fleetRunControlState = (verb: FleetRunControlVerb): FleetRunState => {
  switch (verb) {
    case "pause":
      return "paused"
    case "resume":
      return "running"
    case "drain":
      return "draining"
    case "stop":
      return "stopped"
  }
}

const LIVE_WORK_CLAIM_STATES: readonly LiveWorkClaimState[] = ["claimed", "in_progress", "closeout"]

const isLiveWorkClaimState = (state: WorkClaimState): state is LiveWorkClaimState =>
  LIVE_WORK_CLAIM_STATES.includes(state as LiveWorkClaimState)

export function decodeFleetRun(input: unknown): FleetRun {
  const rawDescriptor = typeof input === "object" && input !== null
    ? (input as { workSourceDescriptor?: unknown }).workSourceDescriptor
    : undefined
  const decodedDescriptor = rawDescriptor === undefined
    ? undefined
    : decodeFleetRunWorkSourceDescriptor(rawDescriptor)
  const run = S.decodeUnknownSync(FleetRunSchema)(input)
  assertNonEmpty("fleet run", "runRef", run.runRef)
  assertNonEmpty("fleet run", "objective", run.objective)
  assertWholePositive("targetConcurrency", run.targetConcurrency)
  assertWholePositive("refillPolicy.maxPerAccount", run.refillPolicy.maxPerAccount)
  for (const [key, value] of Object.entries(run.counters) as Array<[keyof FleetRunCounters, number]>) {
    assertWholeNonNegative(`counters.${key}`, value)
  }
  if (run.dispatchKind === "handoff" && run.dagTracked) {
    throw new Error("fleet run handoff records must not be DAG-tracked")
  }
  if (run.dispatchKind === "supervised_dispatch" && !run.dagTracked) {
    throw new Error("fleet run supervised dispatch records must be DAG-tracked")
  }
  if (run.startedAt !== null && Number.isNaN(Date.parse(run.startedAt))) {
    throw new Error("fleet run startedAt must be ISO-compatible or null")
  }
  if (Number.isNaN(Date.parse(run.createdAt)) || Number.isNaN(Date.parse(run.updatedAt))) {
    throw new Error("fleet run timestamps must be ISO-compatible")
  }
  if (decodedDescriptor !== undefined && decodedDescriptor.kind !== run.workSource) {
    throw new Error("fleet run workSource must match workSourceDescriptor.kind")
  }
  return decodedDescriptor === undefined ? run : { ...run, workSourceDescriptor: decodedDescriptor }
}

export function decodeWorkClaim(input: unknown): WorkClaim {
  const claim = S.decodeUnknownSync(WorkClaimSchema)(input)
  assertNonEmpty("work claim", "claimRef", claim.claimRef)
  assertNonEmpty("work claim", "workUnitRef", claim.workUnitRef)
  assertNonEmpty("work claim", "runRef", claim.runRef)
  assertNonEmpty("work claim", "workerAccountRef", claim.workerAccountRef)
  assertWholePositive("claim ttl", claim.ttl)
  for (const field of ["claimedAt", "expiresAt", "updatedAt"] as const) {
    if (Number.isNaN(Date.parse(claim[field]))) throw new Error(`work claim ${field} must be ISO-compatible`)
  }
  if (Date.parse(claim.expiresAt) <= Date.parse(claim.claimedAt)) {
    throw new Error("work claim expiresAt must be after claimedAt")
  }
  return claim
}

export function buildWorkClaim(input: CreateWorkClaimInput): WorkClaim {
  const now = input.now ?? new Date()
  const claimedAt = iso(now)
  const expiresAt = iso(new Date(now.getTime() + input.ttl))
  return decodeWorkClaim({
    schema: WORK_CLAIM_SCHEMA,
    claimRef: input.claimRef,
    workUnitRef: input.workUnitRef,
    runRef: input.runRef,
    assignmentRef: input.assignmentRef ?? null,
    workerAccountRef: input.workerAccountRef,
    marginalCostClass: input.marginalCostClass ?? "not_measured",
    state: "claimed",
    ttl: input.ttl,
    claimedAt,
    expiresAt,
    updatedAt: claimedAt,
  })
}

export function buildFleetRun(input: CreateFleetRunInput): FleetRun {
  const now = input.now ?? new Date()
  const state = input.state ?? "draft"
  const dispatchKind = input.dispatchKind ?? "supervised_dispatch"
  const run: FleetRun = {
    schema: FLEET_RUN_SCHEMA,
    runRef: input.runRef,
    objective: input.objective,
    workSource: input.workSource,
    ...(input.workSourceDescriptor === undefined
      ? {}
      : { workSourceDescriptor: input.workSourceDescriptor }),
    ...(input.authorityBinding === undefined
      ? {}
      : { authorityBinding: input.authorityBinding }),
    targetConcurrency: input.targetConcurrency,
    workerKind: input.workerKind,
    refillPolicy: {
      ...DEFAULT_FLEET_RUN_REFILL_POLICY,
      ...input.refillPolicy,
    },
    state,
    dispatchKind,
    dagTracked: dispatchKind === "supervised_dispatch",
    startedAt: input.startedAt === undefined
      ? state === "running" ? iso(now) : null
      : input.startedAt instanceof Date ? iso(input.startedAt) : input.startedAt,
    counters: {
      ...DEFAULT_FLEET_RUN_COUNTERS,
      ...input.counters,
    },
    createdAt: iso(now),
    updatedAt: iso(now),
  }
  return decodeFleetRun(run)
}

const fleetRunFromJson = (value: string): FleetRun => decodeFleetRun(JSON.parse(value))

export function normalizeOrchestrationRunnerKind(
  kind: OrchestrationRunnerKind | LegacyAgentRunnerKind,
): OrchestrationRunnerKind {
  if (kind === "claude") return "claude_agent"
  return kind
}

export function isStoredOrchestrationRunnerKind(kind: string): kind is StoredRunnerKind {
  return (
    kind === "generic" ||
    kind === "claude" ||
    kind === "claude_agent" ||
    kind === "codex" ||
    kind === "grok_cli"
  )
}

type TaskRow = {
  id: string
  parent_id: string | null
  thread_id: string
  spec_json: string
  status: OrchestrationTaskStatus
  deps_json: string
  result_json: string | null
  created_at: string
  updated_at: string
}

type DispatchContextRow = {
  id: string
  assignee_handle: string
  runner_kind: StoredRunnerKind
  lane?: string | null
  account_ref_hash?: string | null
  worktree_id: string | null
  worktree_path: string | null
  status: DispatchContextStatus
  current_task_id: string | null
  failure_count: number
  last_heartbeat_at: string | null
  base_behind_by: number
  max_concurrent_slots: number
  paused?: number | null
  created_at: string
  updated_at: string
}

type DispatchBreakerRow = {
  scope_key: string
  lane: string
  account_ref_hash: string | null
  context_id: string | null
  failure_kind: PylonDispatchBreakerSnapshot["failureKind"]
  reason: PylonDispatchBreakerSnapshot["reason"]
  blocker_refs_json: string
  failure_count: number
  first_observed_at: string
  last_observed_at: string
  cooldown_until: string | null
  source_digest_ref: string
}

type AgentRunnerStatusRow = {
  event_ref: string
  runner_ref: string
  runner_kind: string
  state: AgentRunnerNeutralState
  state_started_at: string
  updated_at: string
  assignment_ref: string | null
  task_id: string | null
  dispatch_context_id: string | null
  pylon_ref: string | null
  worktree_ref: string | null
  capability_refs_json: string
  supported_control_verbs_json: string
  refs_json: string
  blocker_refs_json: string
  state_history_json: string
  retention_state: AgentRunnerStatusRetentionState
  retained_at: string | null
}

type VirtualHeadRow = {
  repo: string
  branch: string
  base_commit: string
  projected_head: string
  pending_task_ids_json: string
  created_at: string
  updated_at: string
}

type FleetRunRow = {
  run_ref: string
  record_json: string
  state: FleetRunState
  dispatch_kind: FleetRunDispatchKind
  worker_kind: FleetRunWorkerKind
  created_at: string
  updated_at: string
  started_at: string | null
}

type FleetRunActivationRow = {
  pylon_ref: string
  run_ref: string
  armed: number
}

type FleetRunExecutionOutboxRow = {
  run_ref: string
  claim_ref: string
  sequence: number
  event_ref: string
  event_json: string
  delivery_batch_ref?: string | null
  created_at: string
  delivered_at: string | null
}

type FleetRunSteeringOutcomeRow = {
  pylon_ref: string
  run_ref: string
  claim_ref: string
  seq: number
  intent_id: string
  intent_kind: string
  intent_digest: string
  outcome: FleetRunSteeringOutcomeStatus
  outcome_ref: string
  observed_at: string
}

type FleetRunSteeringOutcomeOutboxRow = FleetRunSteeringOutcomeRow & {
  delivered_at: string | null
}

type FleetRunSteeringQueuedFollowUpRow = {
  pylon_ref: string
  run_ref: string
  claim_ref: string
  seq: number
  intent_id: string
  work_unit_ref: string | null
  work_claim_ref: string | null
  assignment_ref: string | null
  target_ref: string | null
  intent_kind: "fleet_run_control" | "approval_decision" | "steer_message"
  approval_ref: string | null
  decision: "allow" | "deny" | null
  residual_refs_json: string
  body: string | null
  body_ref: string | null
  created_at: string
  state: FleetRunSteeringQueuedFollowUp["state"]
  attempt_count: number
  next_attempt_at: string
  last_attempt_at: string | null
  dispatch_lease_expires_at: string | null
  dispatch_lease_token: string | null
  lease_generation: number
  last_failure_ref: string | null
  completion_ref: string | null
  completed_at: string | null
}

type FleetRunSteeringApprovalBindingRow = {
  approval_ref: string
  pylon_ref: string
  run_ref: string
  claim_ref: string
  work_unit_ref: string
  work_claim_ref: string
  assignment_ref: string
  worker_kind: FleetRunSteeringApprovalBinding["workerKind"]
  worker_ref: string | null
  account_ref_hash: string | null
  tool_class: string | null
  state: FleetRunSteeringApprovalBinding["state"]
  decision: FleetRunSteeringApprovalBinding["decision"]
  resolution_state: FleetRunSteeringApprovalBinding["resolutionState"]
  created_at: string
  resolved_at: string | null
  completion_ref: string | null
}

type FleetRunSteeringFollowUpCompletionRow = {
  pylon_ref: string
  run_ref: string
  claim_ref: string
  seq: number
  intent_id: string
  intent_kind: FleetRunSteeringQueuedFollowUp["intentKind"]
  state: FleetRunSteeringFollowUpCompletion["state"]
  work_unit_ref: string | null
  work_claim_ref: string | null
  assignment_ref: string | null
  approval_ref: string | null
  completion_ref: string
  completed_at: string
  failure_ref: string | null
  delivered_at: string | null
}

type WorkClaimRow = {
  claim_ref: string
  work_unit_ref: string
  run_ref: string
  assignment_ref: string | null
  worker_account_ref: string
  marginal_cost_class?: WorkClaimMarginalCostClass | null
  state: WorkClaimState
  ttl_ms: number
  claimed_at: string
  expires_at: string
  updated_at: string
}

type MessageRow = {
  id: string
  thread_id: string
  task_id: string | null
  dispatch_context_id: string | null
  kind: OrchestrationMessageKind
  body: string
  created_at: string
}

type FleetIntentOutcomeRow = {
  intent_id: number
  scope: string
  run_ref: string
  intent: string
  outcome: FleetIntentOutcomeStatus
  detail: string | null
  mutation_ref: string
  recorded_at: string
}

const fleetIntentOutcomeFromRow = (row: FleetIntentOutcomeRow): FleetIntentOutcomeRecord => ({
  intentId: row.intent_id,
  scope: row.scope,
  runRef: row.run_ref,
  intent: row.intent,
  outcome: row.outcome,
  detail: row.detail,
  mutationRef: row.mutation_ref,
  recordedAt: row.recorded_at,
})

type RuntimeIntentOutcomeRow = {
  intent_id: string
  thread_id: string
  turn_id: string | null
  kind: string
  outcome: RuntimeIntentOutcomeStatus
  detail: string | null
  recorded_at: string
}

const runtimeIntentOutcomeFromRow = (row: RuntimeIntentOutcomeRow): RuntimeIntentOutcomeRecord => ({
  intentId: row.intent_id,
  threadId: row.thread_id,
  turnId: row.turn_id,
  kind: row.kind,
  outcome: row.outcome,
  detail: row.detail,
  recordedAt: row.recorded_at,
})

const taskFromRow = (row: TaskRow): OrchestrationTask => ({
  id: row.id,
  parentId: row.parent_id,
  threadId: row.thread_id,
  spec: parseSpec(row.spec_json),
  status: row.status,
  deps: parseJsonArray(row.deps_json),
  result: row.result_json,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

export const publicOrchestrationTaskFrom = (task: OrchestrationTask): PublicOrchestrationTask => ({
  id: task.id,
  parentId: task.parentId,
  threadId: task.threadId,
  status: task.status,
  deps: task.deps,
  runnerKind: task.spec.runnerKind ?? null,
  repo: task.spec.repo ?? null,
  branch: task.spec.branch ?? null,
  baseCommit: task.spec.baseCommit ?? null,
  issueRef: task.spec.issueRef ?? null,
  fleetRunRef: task.spec.fleetRunRef ?? null,
  createdAt: task.createdAt,
  updatedAt: task.updatedAt,
})

const contextFromRow = (row: DispatchContextRow): DispatchContext => ({
  id: row.id,
  assigneeHandle: row.assignee_handle,
  runnerKind: normalizeOrchestrationRunnerKind(row.runner_kind),
  lane: normalizePylonDispatchFailureLane(row.lane ?? row.runner_kind),
  accountRefHash: row.account_ref_hash ?? null,
  worktreeId: row.worktree_id,
  worktreePath: row.worktree_path,
  status: row.status,
  currentTaskId: row.current_task_id,
  failureCount: row.failure_count,
  lastHeartbeatAt: row.last_heartbeat_at,
  baseBehindBy: row.base_behind_by,
  maxConcurrentSlots: row.max_concurrent_slots,
  paused: (row.paused ?? 0) === 1,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const dispatchBreakerFromRow = (row: DispatchBreakerRow): PylonDispatchBreakerSnapshot => ({
  schema: PYLON_DISPATCH_BREAKER_SCHEMA,
  scopeKey: row.scope_key,
  lane: normalizePylonDispatchFailureLane(row.lane),
  accountRefHash: row.account_ref_hash,
  contextId: row.context_id,
  failureKind: row.failure_kind,
  reason: row.reason,
  blockerRefs: parseJsonArray(row.blocker_refs_json),
  failureCount: row.failure_count,
  firstObservedAt: row.first_observed_at,
  lastObservedAt: row.last_observed_at,
  cooldownUntil: row.cooldown_until,
  sourceDigestRef: row.source_digest_ref,
})

export const publicDispatchContextFrom = (context: DispatchContext): PublicDispatchContext => ({
  ...context,
  worktreePath: null,
})

const virtualHeadFromRow = (row: VirtualHeadRow): VirtualHead => ({
  repo: row.repo,
  branch: row.branch,
  baseCommit: row.base_commit,
  projectedHead: row.projected_head,
  pendingTaskIds: parsePendingTaskIds(row.pending_task_ids_json),
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const messageFromRow = (row: MessageRow): OrchestrationMessage => ({
  id: row.id,
  threadId: row.thread_id,
  taskId: row.task_id,
  dispatchContextId: row.dispatch_context_id,
  kind: row.kind,
  body: row.body,
  createdAt: row.created_at,
})

const fleetRunExecutionOutboxEntryFromRow = (
  row: FleetRunExecutionOutboxRow,
): FleetRunExecutionOutboxEntry => ({
  runRef: row.run_ref,
  claimRef: row.claim_ref,
  sequence: row.sequence,
  eventRef: row.event_ref,
  eventJson: row.event_json,
  deliveryBatchRef: row.delivery_batch_ref ?? null,
  createdAt: row.created_at,
  deliveredAt: row.delivered_at,
})

const fleetRunSteeringOutcomeFromRow = (
  row: FleetRunSteeringOutcomeRow,
): FleetRunSteeringOutcomeRecord => ({
  pylonRef: row.pylon_ref,
  runRef: row.run_ref,
  claimRef: row.claim_ref,
  seq: row.seq,
  intentId: row.intent_id,
  intentKind: row.intent_kind,
  intentDigest: row.intent_digest,
  outcome: row.outcome,
  outcomeRef: row.outcome_ref,
  observedAt: row.observed_at,
})

const fleetRunSteeringOutcomeOutboxFromRow = (
  row: FleetRunSteeringOutcomeOutboxRow,
): FleetRunSteeringOutcomeOutboxEntry => ({
  ...fleetRunSteeringOutcomeFromRow(row),
  deliveredAt: row.delivered_at,
})

const fleetRunSteeringQueuedFollowUpFromRow = (
  row: FleetRunSteeringQueuedFollowUpRow,
): FleetRunSteeringQueuedFollowUp => ({
  pylonRef: row.pylon_ref,
  runRef: row.run_ref,
  claimRef: row.claim_ref,
  seq: row.seq,
  intentId: row.intent_id,
  workUnitRef: row.work_unit_ref,
  workClaimRef: row.work_claim_ref,
  assignmentRef: row.assignment_ref,
  targetRef: row.target_ref,
  intentKind: row.intent_kind,
  approvalRef: row.approval_ref,
  decision: row.decision,
  residualRefs: parseJsonArray(row.residual_refs_json),
  body: row.body,
  bodyRef: row.body_ref,
  createdAt: row.created_at,
  state: row.state,
  attemptCount: row.attempt_count,
  nextAttemptAt: row.next_attempt_at,
  lastAttemptAt: row.last_attempt_at,
  dispatchLeaseExpiresAt: row.dispatch_lease_expires_at,
  dispatchLeaseToken: row.dispatch_lease_token,
  leaseGeneration: row.lease_generation,
  lastFailureRef: row.last_failure_ref,
  completionRef: row.completion_ref,
  completedAt: row.completed_at,
})

const fleetRunSteeringApprovalBindingFromRow = (
  row: FleetRunSteeringApprovalBindingRow,
): FleetRunSteeringApprovalBinding => ({
  approvalRef: row.approval_ref,
  pylonRef: row.pylon_ref,
  runRef: row.run_ref,
  claimRef: row.claim_ref,
  workUnitRef: row.work_unit_ref,
  workClaimRef: row.work_claim_ref,
  assignmentRef: row.assignment_ref,
  workerKind: row.worker_kind,
  workerRef: row.worker_ref,
  accountRefHash: row.account_ref_hash,
  toolClass: row.tool_class,
  state: row.state,
  decision: row.decision,
  resolutionState: row.resolution_state,
  createdAt: row.created_at,
  resolvedAt: row.resolved_at,
  completionRef: row.completion_ref,
})

const fleetRunSteeringFollowUpCompletionFromRow = (
  row: FleetRunSteeringFollowUpCompletionRow,
): FleetRunSteeringFollowUpCompletion => ({
  pylonRef: row.pylon_ref,
  runRef: row.run_ref,
  claimRef: row.claim_ref,
  seq: row.seq,
  intentId: row.intent_id,
  intentKind: row.intent_kind,
  state: row.state,
  workUnitRef: row.work_unit_ref,
  workClaimRef: row.work_claim_ref,
  assignmentRef: row.assignment_ref,
  approvalRef: row.approval_ref,
  completionRef: row.completion_ref,
  completedAt: row.completed_at,
  failureRef: row.failure_ref,
  deliveredAt: row.delivered_at,
})

const workClaimFromRow = (row: WorkClaimRow): WorkClaim => decodeWorkClaim({
  schema: WORK_CLAIM_SCHEMA,
  claimRef: row.claim_ref,
  workUnitRef: row.work_unit_ref,
  runRef: row.run_ref,
  assignmentRef: row.assignment_ref,
  workerAccountRef: row.worker_account_ref,
  marginalCostClass: row.marginal_cost_class ?? "not_measured",
  state: row.state,
  ttl: row.ttl_ms,
  claimedAt: row.claimed_at,
  expiresAt: row.expires_at,
  updatedAt: row.updated_at,
})

const virtualHeadProjectionRef = (input: { repo: string; branch: string; taskId: string; branchFrom: string }): string => {
  const digest = createHash("sha256")
    .update(`${input.repo}\0${input.branch}\0${input.taskId}\0${input.branchFrom}`)
    .digest("hex")
    .slice(0, 20)
  return `virtual-head.${digest}`
}

// Only the live-claim-per-unit index may translate to a "unit busy" null;
// any other constraint failure (e.g. a claimRef primary-key collision) is a
// caller bug and must surface, not masquerade as contention.
const isLiveClaimUniqueConstraintError = (error: unknown): boolean =>
  error instanceof Error &&
  error.message.includes("UNIQUE constraint failed") &&
  (error.message.includes("idx_pylon_orchestration_work_claims_live_unit") ||
    error.message.includes("pylon_orchestration_work_claims.work_unit_ref"))

const stablePublicRef = (prefix: string, value: string): string =>
  value.startsWith(`${prefix}.`)
    ? value
    : `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`

const rollingStateHistory = (
  previous: ReadonlyArray<AgentRunnerStatusHistoryEntry>,
  state: AgentRunnerNeutralState,
  stateStartedAt: string,
  limit: number,
): AgentRunnerStatusHistoryEntry[] => {
  const history = [...previous]
  const last = history.at(-1)
  if (last === undefined || last.state !== state || last.stateStartedAt !== stateStartedAt) {
    history.push({ state, stateStartedAt })
  }
  return history.slice(-Math.max(1, limit))
}

const publicRefArray = (prefix: string, values: ReadonlyArray<string> | undefined): string[] =>
  [...new Set((values ?? []).map((value) => stablePublicRef(prefix, value)))].slice(0, 64)

const publicStatusEventFrom = (
  event: AgentRunnerStatusEvent,
  previous: OrchestrationAgentRunnerStatusEntry | null,
  historyLimit: number,
): AgentRunnerStatusEvent => {
  const stateChanged = previous === null || previous.state !== event.state
  const stateStartedAt = stateChanged ? event.stateStartedAt : previous.stateStartedAt
  const previousHistory = event.stateHistory ?? previous?.stateHistory ?? []
  return {
    eventRef: stablePublicRef("event.public.pylon.runner_status", event.eventRef),
    runnerRef: stablePublicRef("runner.public.pylon", event.runnerRef),
    runnerKind: event.runnerKind,
    state: event.state,
    stateStartedAt,
    updatedAt: event.updatedAt,
    ...(event.assignmentRef === undefined
      ? {}
      : { assignmentRef: stablePublicRef("assignment.public.pylon", event.assignmentRef) }),
    ...(event.taskId === undefined ? {} : { taskId: stablePublicRef("task.public.pylon", event.taskId) }),
    ...(event.dispatchContextId === undefined
      ? {}
      : { dispatchContextId: stablePublicRef("dispatch-context.public.pylon", event.dispatchContextId) }),
    ...(event.pylonRef === undefined ? {} : { pylonRef: stablePublicRef("pylon.public", event.pylonRef) }),
    ...(event.worktreeRef === undefined
      ? event.worktreeId === undefined
        ? {}
        : { worktreeRef: stablePublicRef("worktree.public.pylon", event.worktreeId) }
      : { worktreeRef: stablePublicRef("worktree.public.pylon", event.worktreeRef) }),
    capabilityRefs: publicRefArray("capability.public.pylon", event.capabilityRefs),
    supportedControlVerbs: (event.supportedControlVerbs ?? PYLON_AGENT_RUNNER_CONTROL_VERBS)
      .filter((verb): verb is AgentRunnerControlVerb =>
        PYLON_AGENT_RUNNER_CONTROL_VERBS.includes(verb as AgentRunnerControlVerb),
      ),
    refs: publicRefArray("ref.public.pylon", event.refs),
    blockerRefs: publicRefArray("blocker.public.pylon", event.blockerRefs),
    stateHistory: rollingStateHistory(previousHistory, event.state, stateStartedAt, historyLimit),
  }
}

const statusEntryFromRow = (row: AgentRunnerStatusRow): OrchestrationAgentRunnerStatusEntry => ({
  schemaVersion: PYLON_AGENT_RUNNER_STATUS_EVENT_SCHEMA_VERSION,
  eventRef: row.event_ref,
  runnerRef: row.runner_ref,
  runnerKind: row.runner_kind,
  state: row.state,
  stateStartedAt: row.state_started_at,
  updatedAt: row.updated_at,
  ...(row.assignment_ref === null ? {} : { assignmentRef: row.assignment_ref }),
  ...(row.task_id === null ? {} : { taskId: row.task_id }),
  ...(row.dispatch_context_id === null ? {} : { dispatchContextId: row.dispatch_context_id }),
  ...(row.pylon_ref === null ? {} : { pylonRef: row.pylon_ref }),
  ...(row.worktree_ref === null ? {} : { worktreeRef: row.worktree_ref }),
  capabilityRefs: parseJsonArray(row.capability_refs_json),
  supportedControlVerbs: parseControlVerbs(row.supported_control_verbs_json),
  refs: parseJsonArray(row.refs_json),
  blockerRefs: parseJsonArray(row.blocker_refs_json),
  stateHistory: parseStateHistory(row.state_history_json),
  retentionState: row.retention_state,
  retainedAt: row.retained_at,
})

const dispatchStatusForNeutralState = (state: AgentRunnerNeutralState): DispatchContextStatus => {
  if (state === "blocked") return "blocked"
  if (state === "failed") return "failed"
  if (state === "done") return "completed"
  if (state === "offline") return "circuit_broken"
  if (state === "working" || state === "waiting") return "dispatched"
  return "idle"
}

const shouldDecayRunnerState = (state: AgentRunnerNeutralState): boolean =>
  state === "queued" || state === "working" || state === "waiting"

export class PylonOrchestrationStore {
  constructor(private readonly db: SqliteDatabase) {}

  migrate(): void {
    this.db.exec(`
      CREATE TABLE IF NOT EXISTS pylon_orchestration_meta (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
      );
      CREATE TABLE IF NOT EXISTS pylon_orchestration_tasks (
        id TEXT PRIMARY KEY,
        parent_id TEXT,
        thread_id TEXT NOT NULL,
        spec_json TEXT NOT NULL,
        status TEXT NOT NULL CHECK (status IN ('pending', 'ready', 'dispatched', 'completed', 'failed', 'blocked')),
        deps_json TEXT NOT NULL,
        result_json TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_tasks_status
        ON pylon_orchestration_tasks(status, updated_at);
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_tasks_thread
        ON pylon_orchestration_tasks(thread_id);
      CREATE TABLE IF NOT EXISTS pylon_orchestration_dispatch_contexts (
        id TEXT PRIMARY KEY,
        assignee_handle TEXT NOT NULL,
        runner_kind TEXT NOT NULL CHECK (runner_kind IN ('codex', 'claude_agent', 'claude', 'generic', 'grok_cli')),
        lane TEXT,
        account_ref_hash TEXT,
        worktree_id TEXT,
        worktree_path TEXT,
        status TEXT NOT NULL CHECK (status IN ('idle', 'dispatched', 'completed', 'failed', 'blocked', 'circuit_broken')),
        current_task_id TEXT,
        failure_count INTEGER NOT NULL DEFAULT 0,
        last_heartbeat_at TEXT,
        base_behind_by INTEGER NOT NULL DEFAULT 0,
        max_concurrent_slots INTEGER NOT NULL DEFAULT 1,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_contexts_status
        ON pylon_orchestration_dispatch_contexts(status, updated_at);
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_contexts_worktree
        ON pylon_orchestration_dispatch_contexts(worktree_id);
      CREATE TABLE IF NOT EXISTS pylon_orchestration_dispatch_breakers (
        scope_key TEXT PRIMARY KEY,
        lane TEXT NOT NULL,
        account_ref_hash TEXT,
        context_id TEXT,
        failure_kind TEXT NOT NULL CHECK (failure_kind IN ('permanent', 'transient')),
        reason TEXT NOT NULL,
        blocker_refs_json TEXT NOT NULL,
        failure_count INTEGER NOT NULL DEFAULT 1,
        first_observed_at TEXT NOT NULL,
        last_observed_at TEXT NOT NULL,
        cooldown_until TEXT,
        source_digest_ref TEXT NOT NULL
      );
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_dispatch_breakers_cooldown
        ON pylon_orchestration_dispatch_breakers(failure_kind, cooldown_until);
      CREATE TABLE IF NOT EXISTS pylon_orchestration_runner_statuses (
        event_ref TEXT PRIMARY KEY,
        runner_ref TEXT NOT NULL,
        runner_kind TEXT NOT NULL,
        state TEXT NOT NULL CHECK (state IN ('idle', 'queued', 'working', 'waiting', 'blocked', 'done', 'failed', 'offline')),
        state_started_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        assignment_ref TEXT,
        task_id TEXT,
        dispatch_context_id TEXT,
        pylon_ref TEXT,
        worktree_ref TEXT,
        capability_refs_json TEXT NOT NULL,
        supported_control_verbs_json TEXT NOT NULL,
        refs_json TEXT NOT NULL,
        blocker_refs_json TEXT NOT NULL,
        state_history_json TEXT NOT NULL,
        retention_state TEXT NOT NULL CHECK (retention_state IN ('live', 'retained')),
        retained_at TEXT
      );
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_runner_statuses_live
        ON pylon_orchestration_runner_statuses(runner_ref, retention_state, updated_at);
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_runner_statuses_state
        ON pylon_orchestration_runner_statuses(state, retention_state, updated_at);
      CREATE TABLE IF NOT EXISTS pylon_orchestration_messages (
        id TEXT PRIMARY KEY,
        thread_id TEXT NOT NULL,
        task_id TEXT,
        dispatch_context_id TEXT,
        kind TEXT NOT NULL CHECK (kind IN ('dispatch', 'worker_done', 'heartbeat', 'escalation', 'decision_gate')),
        body TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_messages_thread
        ON pylon_orchestration_messages(thread_id, created_at);
      CREATE TABLE IF NOT EXISTS pylon_orchestration_virtual_heads (
        repo TEXT NOT NULL,
        branch TEXT NOT NULL,
        base_commit TEXT NOT NULL,
        projected_head TEXT NOT NULL,
        pending_task_ids_json TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        PRIMARY KEY (repo, branch)
      );
      CREATE TABLE IF NOT EXISTS pylon_orchestration_fleet_runs (
        run_ref TEXT PRIMARY KEY,
        record_json TEXT NOT NULL,
        state TEXT NOT NULL CHECK (state IN ('draft', 'running', 'paused', 'draining', 'stopped', 'completed')),
        dispatch_kind TEXT NOT NULL CHECK (dispatch_kind IN ('handoff', 'supervised_dispatch')),
        worker_kind TEXT NOT NULL CHECK (worker_kind IN ('codex', 'claude', 'grok', 'auto')),
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        started_at TEXT
      );
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_fleet_runs_state
        ON pylon_orchestration_fleet_runs(state, updated_at);
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_fleet_runs_dispatch
        ON pylon_orchestration_fleet_runs(dispatch_kind, updated_at);
      CREATE TABLE IF NOT EXISTS pylon_orchestration_fleet_run_activations (
        pylon_ref TEXT NOT NULL,
        run_ref TEXT NOT NULL,
        armed INTEGER NOT NULL CHECK (armed IN (0, 1)),
        PRIMARY KEY (pylon_ref, run_ref)
      );
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_fleet_run_activations_armed
        ON pylon_orchestration_fleet_run_activations(pylon_ref, armed, run_ref);
      CREATE TABLE IF NOT EXISTS pylon_orchestration_fleet_run_execution_outbox (
        run_ref TEXT NOT NULL,
        claim_ref TEXT NOT NULL,
        sequence INTEGER NOT NULL CHECK (sequence BETWEEN 1 AND 9007199254740991),
        event_ref TEXT NOT NULL UNIQUE,
        event_json TEXT NOT NULL,
        delivery_batch_ref TEXT,
        created_at TEXT NOT NULL,
        delivered_at TEXT,
        PRIMARY KEY (run_ref, sequence)
      );
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_fleet_run_execution_pending
        ON pylon_orchestration_fleet_run_execution_outbox(run_ref, delivered_at, sequence);
      CREATE TABLE IF NOT EXISTS pylon_orchestration_fleet_run_steering_watermarks (
        pylon_ref TEXT NOT NULL,
        run_ref TEXT NOT NULL,
        claim_ref TEXT NOT NULL,
        after_seq INTEGER NOT NULL CHECK (after_seq BETWEEN 0 AND 9007199254740991),
        updated_at TEXT NOT NULL,
        PRIMARY KEY (pylon_ref, run_ref, claim_ref)
      );
      CREATE TABLE IF NOT EXISTS pylon_orchestration_fleet_run_steering_outcomes (
        pylon_ref TEXT NOT NULL,
        run_ref TEXT NOT NULL,
        claim_ref TEXT NOT NULL,
        seq INTEGER NOT NULL CHECK (seq BETWEEN 1 AND 9007199254740991),
        intent_id TEXT NOT NULL,
        intent_kind TEXT NOT NULL,
        intent_digest TEXT NOT NULL,
        outcome TEXT NOT NULL CHECK (outcome IN ('applied', 'queued_follow_up', 'skipped_stale', 'rejected', 'failed')),
        outcome_ref TEXT NOT NULL UNIQUE,
        observed_at TEXT NOT NULL,
        PRIMARY KEY (pylon_ref, run_ref, claim_ref, seq, intent_id)
      );
      CREATE UNIQUE INDEX IF NOT EXISTS idx_pylon_orchestration_fleet_run_steering_seq
        ON pylon_orchestration_fleet_run_steering_outcomes(pylon_ref, run_ref, claim_ref, seq);
      CREATE UNIQUE INDEX IF NOT EXISTS idx_pylon_orchestration_fleet_run_steering_intent
        ON pylon_orchestration_fleet_run_steering_outcomes(pylon_ref, run_ref, claim_ref, intent_id);
      CREATE TABLE IF NOT EXISTS pylon_orchestration_fleet_run_steering_outbox (
        pylon_ref TEXT NOT NULL,
        run_ref TEXT NOT NULL,
        claim_ref TEXT NOT NULL,
        seq INTEGER NOT NULL CHECK (seq BETWEEN 1 AND 9007199254740991),
        intent_id TEXT NOT NULL,
        intent_kind TEXT NOT NULL,
        intent_digest TEXT NOT NULL,
        outcome TEXT NOT NULL CHECK (outcome IN ('applied', 'queued_follow_up', 'skipped_stale', 'rejected', 'failed')),
        outcome_ref TEXT NOT NULL UNIQUE,
        observed_at TEXT NOT NULL,
        delivered_at TEXT,
        PRIMARY KEY (pylon_ref, run_ref, claim_ref, seq, intent_id)
      );
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_fleet_run_steering_outbox_pending
        ON pylon_orchestration_fleet_run_steering_outbox(pylon_ref, run_ref, claim_ref, delivered_at, seq);
      CREATE TABLE IF NOT EXISTS pylon_orchestration_fleet_run_steering_follow_ups (
        pylon_ref TEXT NOT NULL,
        run_ref TEXT NOT NULL,
        claim_ref TEXT NOT NULL,
        seq INTEGER NOT NULL CHECK (seq BETWEEN 1 AND 9007199254740991),
        intent_id TEXT NOT NULL,
        work_unit_ref TEXT,
        work_claim_ref TEXT,
        assignment_ref TEXT,
        target_ref TEXT,
        intent_kind TEXT NOT NULL CHECK (intent_kind IN ('fleet_run_control', 'approval_decision', 'steer_message')),
        approval_ref TEXT,
        decision TEXT CHECK (decision IS NULL OR decision IN ('allow', 'deny')),
        residual_refs_json TEXT NOT NULL,
        body TEXT,
        body_ref TEXT,
        created_at TEXT NOT NULL,
        state TEXT NOT NULL DEFAULT 'queued' CHECK (state IN ('queued', 'dispatching', 'applied', 'failed', 'stale')),
        attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count BETWEEN 0 AND 9007199254740991),
        next_attempt_at TEXT NOT NULL DEFAULT '1970-01-01T00:00:00.000Z',
        last_attempt_at TEXT,
        dispatch_lease_expires_at TEXT,
        dispatch_lease_token TEXT,
        lease_generation INTEGER NOT NULL DEFAULT 0 CHECK (lease_generation BETWEEN 0 AND 9007199254740991),
        last_failure_ref TEXT,
        completion_ref TEXT,
        completed_at TEXT,
        PRIMARY KEY (pylon_ref, run_ref, claim_ref, seq, intent_id)
      );
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_fleet_run_steering_follow_up_target
        ON pylon_orchestration_fleet_run_steering_follow_ups(run_ref, work_claim_ref, assignment_ref, seq);
      CREATE TABLE IF NOT EXISTS pylon_orchestration_fleet_run_steering_approval_bindings (
        approval_ref TEXT PRIMARY KEY,
        pylon_ref TEXT NOT NULL,
        run_ref TEXT NOT NULL,
        claim_ref TEXT NOT NULL,
        work_unit_ref TEXT NOT NULL,
        work_claim_ref TEXT NOT NULL,
        assignment_ref TEXT NOT NULL,
        worker_kind TEXT NOT NULL CHECK (worker_kind IN ('codex', 'claude', 'grok')),
        worker_ref TEXT NOT NULL,
        account_ref_hash TEXT NOT NULL,
        tool_class TEXT NOT NULL,
        state TEXT NOT NULL CHECK (state IN ('pending', 'resolved')),
        decision TEXT CHECK (decision IS NULL OR decision IN ('allow', 'deny')),
        resolution_state TEXT CHECK (resolution_state IS NULL OR resolution_state IN ('applied', 'failed', 'stale')),
        created_at TEXT NOT NULL,
        resolved_at TEXT,
        completion_ref TEXT
      );
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_fleet_run_steering_approval_attempt
        ON pylon_orchestration_fleet_run_steering_approval_bindings(
          pylon_ref, run_ref, claim_ref, work_claim_ref, assignment_ref, state
        );
      CREATE TABLE IF NOT EXISTS pylon_orchestration_fleet_run_steering_completion_outbox (
        pylon_ref TEXT NOT NULL,
        run_ref TEXT NOT NULL,
        claim_ref TEXT NOT NULL,
        seq INTEGER NOT NULL CHECK (seq BETWEEN 1 AND 9007199254740991),
        intent_id TEXT NOT NULL,
        intent_kind TEXT NOT NULL CHECK (intent_kind IN ('fleet_run_control', 'approval_decision', 'steer_message')),
        state TEXT NOT NULL CHECK (state IN ('applied', 'failed', 'stale')),
        work_unit_ref TEXT,
        work_claim_ref TEXT,
        assignment_ref TEXT,
        approval_ref TEXT,
        completion_ref TEXT NOT NULL UNIQUE,
        completed_at TEXT NOT NULL,
        failure_ref TEXT,
        delivered_at TEXT,
        PRIMARY KEY (pylon_ref, run_ref, claim_ref, seq, intent_id)
      );
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_fleet_run_steering_completion_pending
        ON pylon_orchestration_fleet_run_steering_completion_outbox(
          pylon_ref, run_ref, claim_ref, delivered_at, seq
        );
      CREATE TABLE IF NOT EXISTS pylon_orchestration_work_claims (
        claim_ref TEXT PRIMARY KEY,
        work_unit_ref TEXT NOT NULL,
        run_ref TEXT NOT NULL,
        assignment_ref TEXT,
        worker_account_ref TEXT NOT NULL,
        marginal_cost_class TEXT NOT NULL DEFAULT 'not_measured'
          CHECK (marginal_cost_class IN ('free', 'subscription', 'api_metered', 'not_measured')),
        state TEXT NOT NULL CHECK (state IN ('claimed', 'in_progress', 'closeout', 'released', 'expired')),
        ttl_ms INTEGER NOT NULL CHECK (ttl_ms > 0),
        claimed_at TEXT NOT NULL,
        expires_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );
      CREATE UNIQUE INDEX IF NOT EXISTS idx_pylon_orchestration_work_claims_live_unit
        ON pylon_orchestration_work_claims(work_unit_ref)
        WHERE state IN ('claimed', 'in_progress', 'closeout');
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_work_claims_expiry
        ON pylon_orchestration_work_claims(state, expires_at);
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_work_claims_run
        ON pylon_orchestration_work_claims(run_ref, updated_at);
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_work_claims_worker
        ON pylon_orchestration_work_claims(worker_account_ref, state);
      CREATE TABLE IF NOT EXISTS pylon_orchestration_fleet_intent_outcomes (
        intent_id INTEGER PRIMARY KEY,
        scope TEXT NOT NULL,
        run_ref TEXT NOT NULL,
        intent TEXT NOT NULL,
        outcome TEXT NOT NULL CHECK (outcome IN ('applied', 'skipped_stale', 'failed')),
        detail TEXT,
        mutation_ref TEXT NOT NULL,
        recorded_at TEXT NOT NULL
      );
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_fleet_intent_outcomes_run
        ON pylon_orchestration_fleet_intent_outcomes(run_ref, intent_id);
      CREATE TABLE IF NOT EXISTS pylon_orchestration_runtime_intent_outcomes (
        intent_id TEXT PRIMARY KEY,
        thread_id TEXT NOT NULL,
        turn_id TEXT,
        kind TEXT NOT NULL,
        outcome TEXT NOT NULL CHECK (outcome IN ('applied', 'skipped_stale', 'failed')),
        detail TEXT,
        recorded_at TEXT NOT NULL
      );
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_runtime_intent_outcomes_thread
        ON pylon_orchestration_runtime_intent_outcomes(thread_id, intent_id);
    `)
    this.ensureDispatchContextBreakerColumns()
    this.ensureDispatchContextPausedColumn()
    this.ensureDispatchContextRunnerKindAllowsGrokCli()
    this.ensureFleetRunWorkerKindAllowsGrok()
    this.ensureFleetRunExecutionOutboxBatchColumn()
    this.ensureWorkClaimMarginalCostClassColumn()
    this.ensureFleetRunSteeringFollowUpColumns()
    this.db
      .query("INSERT OR REPLACE INTO pylon_orchestration_meta (key, value) VALUES ('schema_version', $version)")
      .run({ $version: String(ORCHESTRATION_SCHEMA_VERSION) })
  }

  private tableColumnNames(table: string): Set<string> {
    const rows = this.db.query(`PRAGMA table_info(${table})`).all() as Array<{ name?: unknown }>
    return new Set(rows.flatMap(row => typeof row.name === "string" ? [row.name] : []))
  }

  private ensureDispatchContextBreakerColumns(): void {
    const contextColumns = this.tableColumnNames("pylon_orchestration_dispatch_contexts")
    if (!contextColumns.has("lane")) {
      this.db.exec("ALTER TABLE pylon_orchestration_dispatch_contexts ADD COLUMN lane TEXT")
    }
    if (!contextColumns.has("account_ref_hash")) {
      this.db.exec("ALTER TABLE pylon_orchestration_dispatch_contexts ADD COLUMN account_ref_hash TEXT")
    }

    const breakerColumns = this.tableColumnNames("pylon_orchestration_dispatch_breakers")
    if (!breakerColumns.has("lane")) {
      this.db.exec("ALTER TABLE pylon_orchestration_dispatch_breakers ADD COLUMN lane TEXT NOT NULL DEFAULT 'default'")
    }
    if (!breakerColumns.has("account_ref_hash")) {
      this.db.exec("ALTER TABLE pylon_orchestration_dispatch_breakers ADD COLUMN account_ref_hash TEXT")
    }

    this.db.exec(`
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_contexts_account_lane
        ON pylon_orchestration_dispatch_contexts(account_ref_hash, lane);
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_dispatch_breakers_lane_account
        ON pylon_orchestration_dispatch_breakers(lane, account_ref_hash);
    `)
  }

  private ensureDispatchContextPausedColumn(): void {
    const contextColumns = this.tableColumnNames("pylon_orchestration_dispatch_contexts")
    if (!contextColumns.has("paused")) {
      this.db.exec("ALTER TABLE pylon_orchestration_dispatch_contexts ADD COLUMN paused INTEGER NOT NULL DEFAULT 0")
    }
  }

  private ensureFleetRunExecutionOutboxBatchColumn(): void {
    const outboxColumns = this.tableColumnNames(
      "pylon_orchestration_fleet_run_execution_outbox",
    )
    if (!outboxColumns.has("delivery_batch_ref")) {
      this.db.exec(`
        ALTER TABLE pylon_orchestration_fleet_run_execution_outbox
        ADD COLUMN delivery_batch_ref TEXT
      `)
    }
    this.db.exec(`
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_fleet_run_execution_batch
        ON pylon_orchestration_fleet_run_execution_outbox(run_ref, delivery_batch_ref, sequence)
    `)
  }

  private ensureWorkClaimMarginalCostClassColumn(): void {
    const columns = this.tableColumnNames("pylon_orchestration_work_claims")
    if (!columns.has("marginal_cost_class")) {
      this.db.exec(`
        ALTER TABLE pylon_orchestration_work_claims
        ADD COLUMN marginal_cost_class TEXT NOT NULL DEFAULT 'not_measured'
          CHECK (marginal_cost_class IN ('free', 'subscription', 'api_metered', 'not_measured'))
      `)
    }
  }

  private ensureFleetRunSteeringFollowUpColumns(): void {
    const table = "pylon_orchestration_fleet_run_steering_follow_ups"
    const columns = this.tableColumnNames(table)
    const additions = [
      ["state", "TEXT NOT NULL DEFAULT 'queued' CHECK (state IN ('queued', 'dispatching', 'applied', 'failed', 'stale'))"],
      ["attempt_count", "INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count BETWEEN 0 AND 9007199254740991)"],
      ["next_attempt_at", "TEXT NOT NULL DEFAULT '1970-01-01T00:00:00.000Z'"],
      ["last_attempt_at", "TEXT"],
      ["dispatch_lease_expires_at", "TEXT"],
      ["dispatch_lease_token", "TEXT"],
      ["lease_generation", "INTEGER NOT NULL DEFAULT 0 CHECK (lease_generation BETWEEN 0 AND 9007199254740991)"],
      ["last_failure_ref", "TEXT"],
      ["completion_ref", "TEXT"],
      ["completed_at", "TEXT"],
    ] as const
    for (const [column, definition] of additions) {
      if (!columns.has(column)) {
        this.db.exec(`ALTER TABLE ${table} ADD COLUMN ${column} ${definition}`)
      }
    }
    this.db.exec(`
      UPDATE ${table}
         SET next_attempt_at = created_at
       WHERE next_attempt_at = '1970-01-01T00:00:00.000Z';
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_fleet_run_steering_follow_up_dispatch
        ON ${table}(pylon_ref, run_ref, claim_ref, state, next_attempt_at, seq);
    `)
    const approvalColumns = this.tableColumnNames(
      "pylon_orchestration_fleet_run_steering_approval_bindings",
    )
    if (!approvalColumns.has("resolution_state")) {
      this.db.exec(`
        ALTER TABLE pylon_orchestration_fleet_run_steering_approval_bindings
        ADD COLUMN resolution_state TEXT
          CHECK (resolution_state IS NULL OR resolution_state IN ('applied', 'failed', 'stale'))
      `)
    }
    const approvalIdentityColumns = [
      ["worker_kind", "TEXT CHECK (worker_kind IS NULL OR worker_kind IN ('codex', 'claude', 'grok'))"],
      ["worker_ref", "TEXT"],
      ["account_ref_hash", "TEXT"],
      ["tool_class", "TEXT"],
    ] as const
    for (const [column, definition] of approvalIdentityColumns) {
      if (!approvalColumns.has(column)) {
        // Historical approval bindings predate exact worker/tool identity.
        // Keep those columns nullable on upgrade and exclude incomplete rows
        // from replay instead of inventing identity after the fact.
        this.db.exec(`
          ALTER TABLE pylon_orchestration_fleet_run_steering_approval_bindings
          ADD COLUMN ${column} ${definition}
        `)
      }
    }
  }

  /**
   * MH-4: expand the dispatch-context `runner_kind` CHECK to include `grok_cli`.
   * SQLite cannot ALTER a CHECK constraint, so existing DBs (pre schema v3)
   * rebuild the table once. Fresh installs already get the expanded CHECK from
   * CREATE TABLE IF NOT EXISTS above.
   */
  private ensureDispatchContextRunnerKindAllowsGrokCli(): void {
    const rawVersion = this.getMeta("schema_version")
    const version = rawVersion === null ? 0 : Number(rawVersion)
    if (Number.isFinite(version) && version >= 3) return

    // Probe: if the live CHECK already accepts grok_cli, skip rebuild.
    try {
      this.db.exec("SAVEPOINT probe_grok_cli_runner_kind")
      this.db
        .query(`
          INSERT INTO pylon_orchestration_dispatch_contexts
            (id, assignee_handle, runner_kind, status, failure_count, base_behind_by,
             max_concurrent_slots, created_at, updated_at)
          VALUES
            ('__probe_grok_cli__', 'probe', 'grok_cli', 'idle', 0, 0, 1,
             '1970-01-01T00:00:00.000Z', '1970-01-01T00:00:00.000Z')
        `)
        .run()
      this.db.exec("ROLLBACK TO probe_grok_cli_runner_kind")
      this.db.exec("RELEASE probe_grok_cli_runner_kind")
      return
    } catch {
      try {
        this.db.exec("ROLLBACK TO probe_grok_cli_runner_kind")
        this.db.exec("RELEASE probe_grok_cli_runner_kind")
      } catch {
        // ignore nested rollback failures
      }
    }

    const columns = this.tableColumnNames("pylon_orchestration_dispatch_contexts")
    const hasLane = columns.has("lane")
    const hasAccountRefHash = columns.has("account_ref_hash")
    const hasPaused = columns.has("paused")

    this.db.exec(`
      CREATE TABLE pylon_orchestration_dispatch_contexts_v3 (
        id TEXT PRIMARY KEY,
        assignee_handle TEXT NOT NULL,
        runner_kind TEXT NOT NULL CHECK (runner_kind IN ('codex', 'claude_agent', 'claude', 'generic', 'grok_cli')),
        lane TEXT,
        account_ref_hash TEXT,
        worktree_id TEXT,
        worktree_path TEXT,
        status TEXT NOT NULL CHECK (status IN ('idle', 'dispatched', 'completed', 'failed', 'blocked', 'circuit_broken')),
        current_task_id TEXT,
        failure_count INTEGER NOT NULL DEFAULT 0,
        last_heartbeat_at TEXT,
        base_behind_by INTEGER NOT NULL DEFAULT 0,
        max_concurrent_slots INTEGER NOT NULL DEFAULT 1,
        paused INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );
      INSERT INTO pylon_orchestration_dispatch_contexts_v3 (
        id, assignee_handle, runner_kind, lane, account_ref_hash, worktree_id, worktree_path,
        status, current_task_id, failure_count, last_heartbeat_at, base_behind_by,
        max_concurrent_slots, paused, created_at, updated_at
      )
      SELECT
        id,
        assignee_handle,
        runner_kind,
        ${hasLane ? "lane" : "NULL"},
        ${hasAccountRefHash ? "account_ref_hash" : "NULL"},
        worktree_id,
        worktree_path,
        status,
        current_task_id,
        failure_count,
        last_heartbeat_at,
        base_behind_by,
        max_concurrent_slots,
        ${hasPaused ? "paused" : "0"},
        created_at,
        updated_at
      FROM pylon_orchestration_dispatch_contexts;
      DROP TABLE pylon_orchestration_dispatch_contexts;
      ALTER TABLE pylon_orchestration_dispatch_contexts_v3
        RENAME TO pylon_orchestration_dispatch_contexts;
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_contexts_status
        ON pylon_orchestration_dispatch_contexts(status, updated_at);
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_contexts_worktree
        ON pylon_orchestration_dispatch_contexts(worktree_id);
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_contexts_account_lane
        ON pylon_orchestration_dispatch_contexts(account_ref_hash, lane);
    `)
  }

  /**
   * FC-5: expand the durable FleetRun worker_kind CHECK to include Grok.
   *
   * Fresh databases already have the current constraint. Existing Pylon homes
   * can predate Grok support while carrying a newer meta version from unrelated
   * additive migrations, so probe the actual constraint instead of trusting the
   * coarse schema-version marker. SQLite cannot ALTER a CHECK constraint; an
   * atomic table rebuild preserves every existing run before remote intake can
   * import a Grok-only Sarah FleetRun.
   */
  private ensureFleetRunWorkerKindAllowsGrok(): void {
    try {
      this.db.exec("SAVEPOINT probe_fleet_run_grok_worker_kind")
      this.db
        .query(`
          INSERT INTO pylon_orchestration_fleet_runs
            (run_ref, record_json, state, dispatch_kind, worker_kind,
             created_at, updated_at, started_at)
          VALUES
            ('__probe_fleet_run_grok__', '{}', 'draft', 'supervised_dispatch',
             'grok', '1970-01-01T00:00:00.000Z',
             '1970-01-01T00:00:00.000Z', NULL)
        `)
        .run()
      this.db.exec("ROLLBACK TO probe_fleet_run_grok_worker_kind")
      this.db.exec("RELEASE probe_fleet_run_grok_worker_kind")
      return
    } catch {
      try {
        this.db.exec("ROLLBACK TO probe_fleet_run_grok_worker_kind")
        this.db.exec("RELEASE probe_fleet_run_grok_worker_kind")
      } catch {
        // Ignore probe rollback failures; the atomic rebuild below is decisive.
      }
    }

    this.db.exec("SAVEPOINT migrate_fleet_run_grok_worker_kind")
    try {
      this.db.exec(`
        CREATE TABLE pylon_orchestration_fleet_runs_v12 (
          run_ref TEXT PRIMARY KEY,
          record_json TEXT NOT NULL,
          state TEXT NOT NULL CHECK (state IN ('draft', 'running', 'paused', 'draining', 'stopped', 'completed')),
          dispatch_kind TEXT NOT NULL CHECK (dispatch_kind IN ('handoff', 'supervised_dispatch')),
          worker_kind TEXT NOT NULL CHECK (worker_kind IN ('codex', 'claude', 'grok', 'auto')),
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          started_at TEXT
        );
        INSERT INTO pylon_orchestration_fleet_runs_v12 (
          run_ref, record_json, state, dispatch_kind, worker_kind,
          created_at, updated_at, started_at
        )
        SELECT
          run_ref, record_json, state, dispatch_kind, worker_kind,
          created_at, updated_at, started_at
        FROM pylon_orchestration_fleet_runs;
        DROP TABLE pylon_orchestration_fleet_runs;
        ALTER TABLE pylon_orchestration_fleet_runs_v12
          RENAME TO pylon_orchestration_fleet_runs;
        CREATE INDEX idx_pylon_orchestration_fleet_runs_state
          ON pylon_orchestration_fleet_runs(state, updated_at);
        CREATE INDEX idx_pylon_orchestration_fleet_runs_dispatch
          ON pylon_orchestration_fleet_runs(dispatch_kind, updated_at);
      `)
      this.db.exec("RELEASE migrate_fleet_run_grok_worker_kind")
    } catch (error) {
      try {
        this.db.exec("ROLLBACK TO migrate_fleet_run_grok_worker_kind")
        this.db.exec("RELEASE migrate_fleet_run_grok_worker_kind")
      } catch {
        // Preserve the original migration failure.
      }
      throw error
    }
  }

  createFleetRun(input: CreateFleetRunInput): FleetRun {
    const run = buildFleetRun(input)
    return this.upsertFleetRun(run)
  }

  upsertFleetRun(input: FleetRun, now?: Date): FleetRun {
    const current = decodeFleetRun(input)
    const updatedAt = now === undefined ? current.updatedAt : iso(now)
    const run = decodeFleetRun({ ...current, updatedAt })
    this.db
      .query(`
        INSERT INTO pylon_orchestration_fleet_runs
          (run_ref, record_json, state, dispatch_kind, worker_kind, created_at, updated_at, started_at)
        VALUES
          ($runRef, $recordJson, $state, $dispatchKind, $workerKind, $createdAt, $updatedAt, $startedAt)
        ON CONFLICT(run_ref) DO UPDATE SET
          record_json = excluded.record_json,
          state = excluded.state,
          dispatch_kind = excluded.dispatch_kind,
          worker_kind = excluded.worker_kind,
          updated_at = excluded.updated_at,
          started_at = excluded.started_at
      `)
      .run({
        $runRef: run.runRef,
        $recordJson: JSON.stringify(run),
        $state: run.state,
        $dispatchKind: run.dispatchKind,
        $workerKind: run.workerKind,
        $createdAt: run.createdAt,
        $updatedAt: run.updatedAt,
        $startedAt: run.startedAt,
      })
    const stored = this.getFleetRun(run.runRef)
    if (stored === null) throw new Error(`failed to persist fleet run ${run.runRef}`)
    return stored
  }

  getFleetRun(runRef: string): FleetRun | null {
    const row = this.db
      .query("SELECT * FROM pylon_orchestration_fleet_runs WHERE run_ref = $runRef")
      .get({ $runRef: runRef }) as FleetRunRow | null
    return row === null ? null : fleetRunFromJson(row.record_json)
  }

  listFleetRuns(state?: FleetRunState): FleetRun[] {
    const rows = state === undefined
      ? this.db.query("SELECT * FROM pylon_orchestration_fleet_runs ORDER BY created_at ASC").all()
      : this.db
        .query("SELECT * FROM pylon_orchestration_fleet_runs WHERE state = $state ORDER BY created_at ASC")
        .all({ $state: state })
    return (rows as FleetRunRow[]).map((row) => fleetRunFromJson(row.record_json))
  }

  setFleetRunActivation(input: FleetRunActivation): FleetRunActivation {
    if (
      !FLEET_RUN_ACTIVATION_REF_PATTERN.test(input.pylonRef) ||
      !FLEET_RUN_ACTIVATION_REF_PATTERN.test(input.runRef)
    ) {
      throw new Error("fleet run activation refs must be bounded public-safe refs")
    }
    this.db
      .query(`
        INSERT INTO pylon_orchestration_fleet_run_activations
          (pylon_ref, run_ref, armed)
        VALUES
          ($pylonRef, $runRef, $armed)
        ON CONFLICT(pylon_ref, run_ref) DO UPDATE SET
          armed = excluded.armed
      `)
      .run({
        $pylonRef: input.pylonRef,
        $runRef: input.runRef,
        $armed: input.armed ? 1 : 0,
      })
    return { ...input }
  }

  getFleetRunActivation(pylonRef: string, runRef: string): FleetRunActivation | null {
    const row = this.db
      .query(`
        SELECT pylon_ref, run_ref, armed
          FROM pylon_orchestration_fleet_run_activations
         WHERE pylon_ref = $pylonRef AND run_ref = $runRef
      `)
      .get({ $pylonRef: pylonRef, $runRef: runRef }) as FleetRunActivationRow | null
    return row === null
      ? null
      : { pylonRef: row.pylon_ref, runRef: row.run_ref, armed: row.armed === 1 }
  }

  listFleetRunActivations(pylonRef: string): FleetRunActivation[] {
    const rows = this.db
      .query(`
        SELECT pylon_ref, run_ref, armed
          FROM pylon_orchestration_fleet_run_activations
         WHERE pylon_ref = $pylonRef
         ORDER BY run_ref ASC
      `)
      .all({ $pylonRef: pylonRef }) as FleetRunActivationRow[]
    return rows.map((row) => ({
      pylonRef: row.pylon_ref,
      runRef: row.run_ref,
      armed: row.armed === 1,
    }))
  }

  enqueueFleetRunExecutionOutbox(
    input: EnqueueFleetRunExecutionOutboxInput,
  ): FleetRunExecutionOutboxEntry {
    if (
      !FLEET_RUN_ACTIVATION_REF_PATTERN.test(input.runRef) ||
      !/^claim\.sarah_fleet_run\.[0-9a-f]{24}$/u.test(input.claimRef) ||
      !FLEET_RUN_EXECUTION_EVENT_REF_PATTERN.test(input.eventRef)
    ) {
      throw new Error("fleet run execution outbox refs are invalid")
    }
    const createdAt = iso(input.now)
    this.db.run("BEGIN IMMEDIATE")
    try {
      // The authority read belongs under the same write lock as sequence
      // allocation. Otherwise another connection could replace or revoke the
      // accepted binding after the check but before this append commits.
      const run = this.getFleetRun(input.runRef)
      if (
        run?.authorityBinding?.phase !== "accepted" ||
        run.authorityBinding.claimRef !== input.claimRef
      ) {
        throw new Error("fleet run execution outbox requires the exact accepted authority claim")
      }
      const existing = this.db
        .query(`
          SELECT run_ref, claim_ref, sequence, event_ref, event_json, delivery_batch_ref,
                 created_at, delivered_at
            FROM pylon_orchestration_fleet_run_execution_outbox
           WHERE event_ref = $eventRef
        `)
        .get({ $eventRef: input.eventRef }) as FleetRunExecutionOutboxRow | null
      if (existing !== null) {
        const expectedJson = input.eventJsonForSequence(existing.sequence)
        if (
          existing.run_ref !== input.runRef ||
          existing.claim_ref !== input.claimRef ||
          existing.event_json !== expectedJson
        ) {
          throw new Error("fleet run execution event ref was reused with conflicting bytes")
        }
        this.db.run("COMMIT")
        return fleetRunExecutionOutboxEntryFromRow(existing)
      }
      const next = this.db
        .query(`
          SELECT COALESCE(MAX(sequence), 0) + 1 AS sequence
            FROM pylon_orchestration_fleet_run_execution_outbox
           WHERE run_ref = $runRef
        `)
        .get({ $runRef: input.runRef }) as { sequence?: unknown } | null
      const sequence = Number(next?.sequence)
      if (!Number.isSafeInteger(sequence) || sequence < 1) {
        throw new Error("fleet run execution outbox sequence is invalid")
      }
      const eventJson = input.eventJsonForSequence(sequence)
      const byteLength = new TextEncoder().encode(eventJson).byteLength
      if (
        byteLength < 2 ||
        byteLength > FLEET_RUN_EXECUTION_EVENT_MAX_BYTES ||
        eventJson.includes("\u0000")
      ) {
        throw new Error("fleet run execution event bytes are invalid")
      }
      this.db
        .query(`
          INSERT INTO pylon_orchestration_fleet_run_execution_outbox
            (run_ref, claim_ref, sequence, event_ref, event_json,
             created_at, delivered_at)
          VALUES
            ($runRef, $claimRef, $sequence, $eventRef, $eventJson,
             $createdAt, NULL)
        `)
        .run({
          $runRef: input.runRef,
          $claimRef: input.claimRef,
          $sequence: sequence,
          $eventRef: input.eventRef,
          $eventJson: eventJson,
          $createdAt: createdAt,
        })
      const inserted = this.db
        .query(`
          SELECT run_ref, claim_ref, sequence, event_ref, event_json, delivery_batch_ref,
                 created_at, delivered_at
            FROM pylon_orchestration_fleet_run_execution_outbox
           WHERE run_ref = $runRef AND sequence = $sequence
        `)
        .get({ $runRef: input.runRef, $sequence: sequence }) as FleetRunExecutionOutboxRow | null
      if (inserted === null) {
        throw new Error("fleet run execution outbox insert was not retained")
      }
      this.db.run("COMMIT")
      return fleetRunExecutionOutboxEntryFromRow(inserted)
    } catch (error) {
      this.db.run("ROLLBACK")
      throw error
    }
  }

  /**
   * Freeze one retry boundary before a network append begins.
   *
   * A response may be lost after the server commits. Persisting this boundary
   * means later enqueues cannot expand the retried request and thereby change
   * its idempotency key or bytes. Only the oldest pending, gapless prefix may
   * be reserved.
   */
  reserveFleetRunExecutionOutboxBatch(
    input: ReserveFleetRunExecutionOutboxBatchInput,
  ): FleetRunExecutionOutboxEntry[] {
    const count = input.lastSequence - input.firstSequence + 1
    if (
      !FLEET_RUN_ACTIVATION_REF_PATTERN.test(input.runRef) ||
      !/^claim\.sarah_fleet_run\.[0-9a-f]{24}$/u.test(input.claimRef) ||
      !FLEET_RUN_EXECUTION_BATCH_REF_PATTERN.test(input.deliveryBatchRef) ||
      !Number.isSafeInteger(input.firstSequence) ||
      !Number.isSafeInteger(input.lastSequence) ||
      input.firstSequence < 1 ||
      input.lastSequence < input.firstSequence ||
      !Number.isSafeInteger(count) ||
      count > 64
    ) {
      throw new Error("fleet run execution batch reservation is invalid")
    }

    this.db.run("BEGIN IMMEDIATE")
    try {
      const run = this.getFleetRun(input.runRef)
      if (
        run?.authorityBinding?.phase !== "accepted" ||
        run.authorityBinding.claimRef !== input.claimRef
      ) {
        throw new Error("fleet run execution outbox requires the exact accepted authority claim")
      }
      const head = this.db
        .query(`
          SELECT MIN(sequence) AS sequence
            FROM pylon_orchestration_fleet_run_execution_outbox
           WHERE run_ref = $runRef AND delivered_at IS NULL
        `)
        .get({ $runRef: input.runRef }) as { sequence?: unknown } | null
      if (Number(head?.sequence) !== input.firstSequence) {
        throw new Error("fleet run execution batch must reserve the oldest pending prefix")
      }
      const rows = this.db
        .query(`
          SELECT run_ref, claim_ref, sequence, event_ref, event_json, delivery_batch_ref,
                 created_at, delivered_at
            FROM pylon_orchestration_fleet_run_execution_outbox
           WHERE run_ref = $runRef
             AND sequence BETWEEN $firstSequence AND $lastSequence
           ORDER BY sequence ASC
        `)
        .all({
          $runRef: input.runRef,
          $firstSequence: input.firstSequence,
          $lastSequence: input.lastSequence,
        }) as FleetRunExecutionOutboxRow[]
      if (
        rows.length !== count ||
        rows.some((row, index) =>
          row.sequence !== input.firstSequence + index ||
          row.claim_ref !== input.claimRef ||
          row.delivered_at !== null ||
          (row.delivery_batch_ref !== null &&
            row.delivery_batch_ref !== undefined &&
            row.delivery_batch_ref !== input.deliveryBatchRef)
        )
      ) {
        throw new Error("fleet run execution batch prefix is not reservable")
      }
      this.db
        .query(`
          UPDATE pylon_orchestration_fleet_run_execution_outbox
             SET delivery_batch_ref = $deliveryBatchRef
           WHERE run_ref = $runRef
             AND claim_ref = $claimRef
             AND sequence BETWEEN $firstSequence AND $lastSequence
             AND delivered_at IS NULL
             AND (delivery_batch_ref IS NULL OR delivery_batch_ref = $deliveryBatchRef)
        `)
        .run({
          $runRef: input.runRef,
          $claimRef: input.claimRef,
          $firstSequence: input.firstSequence,
          $lastSequence: input.lastSequence,
          $deliveryBatchRef: input.deliveryBatchRef,
        })
      const reserved = this.db
        .query(`
          SELECT run_ref, claim_ref, sequence, event_ref, event_json, delivery_batch_ref,
                 created_at, delivered_at
            FROM pylon_orchestration_fleet_run_execution_outbox
           WHERE run_ref = $runRef
             AND sequence BETWEEN $firstSequence AND $lastSequence
           ORDER BY sequence ASC
        `)
        .all({
          $runRef: input.runRef,
          $firstSequence: input.firstSequence,
          $lastSequence: input.lastSequence,
        }) as FleetRunExecutionOutboxRow[]
      if (
        reserved.length !== count ||
        reserved.some(row => row.delivery_batch_ref !== input.deliveryBatchRef)
      ) {
        throw new Error("fleet run execution batch reservation was not retained")
      }
      this.db.run("COMMIT")
      return reserved.map(fleetRunExecutionOutboxEntryFromRow)
    } catch (error) {
      this.db.run("ROLLBACK")
      throw error
    }
  }

  listFleetRunExecutionOutbox(
    runRef: string,
    options: { pendingOnly?: boolean; limit?: number } = {},
  ): FleetRunExecutionOutboxEntry[] {
    if (!FLEET_RUN_ACTIVATION_REF_PATTERN.test(runRef)) {
      throw new Error("fleet run execution outbox run ref is invalid")
    }
    const limit = Math.max(1, Math.min(64, Math.trunc(options.limit ?? 64)))
    const rows = options.pendingOnly === false
      ? this.db
        .query(`
          SELECT run_ref, claim_ref, sequence, event_ref, event_json, delivery_batch_ref,
                 created_at, delivered_at
            FROM pylon_orchestration_fleet_run_execution_outbox
           WHERE run_ref = $runRef
           ORDER BY sequence ASC
           LIMIT $limit
        `)
        .all({ $runRef: runRef, $limit: limit })
      : this.db
        .query(`
          SELECT run_ref, claim_ref, sequence, event_ref, event_json, delivery_batch_ref,
                 created_at, delivered_at
            FROM pylon_orchestration_fleet_run_execution_outbox
           WHERE run_ref = $runRef AND delivered_at IS NULL
           ORDER BY sequence ASC
           LIMIT $limit
        `)
        .all({ $runRef: runRef, $limit: limit })
    return (rows as FleetRunExecutionOutboxRow[]).map(
      fleetRunExecutionOutboxEntryFromRow,
    )
  }

  markFleetRunExecutionOutboxDelivered(
    runRef: string,
    claimRef: string,
    acceptedThroughSequence: number,
    now: Date = new Date(),
  ): number {
    if (
      !FLEET_RUN_ACTIVATION_REF_PATTERN.test(runRef) ||
      !/^claim\.sarah_fleet_run\.[0-9a-f]{24}$/u.test(claimRef) ||
      !Number.isSafeInteger(acceptedThroughSequence) ||
      acceptedThroughSequence < 0
    ) {
      throw new Error("fleet run execution acknowledgement is invalid")
    }
    const result = this.db
      .query(`
        UPDATE pylon_orchestration_fleet_run_execution_outbox
           SET delivered_at = COALESCE(delivered_at, $deliveredAt)
         WHERE run_ref = $runRef
           AND claim_ref = $claimRef
           AND sequence <= $acceptedThroughSequence
      `)
      .run({
        $runRef: runRef,
        $claimRef: claimRef,
        $acceptedThroughSequence: acceptedThroughSequence,
        $deliveredAt: iso(now),
      })
    return Number(result.changes)
  }

  getFleetRunSteeringWatermark(
    pylonRef: string,
    runRef: string,
    claimRef: string,
  ): number {
    const row = this.db
      .query(`
        SELECT after_seq
          FROM pylon_orchestration_fleet_run_steering_watermarks
         WHERE pylon_ref = $pylonRef
           AND run_ref = $runRef
           AND claim_ref = $claimRef
      `)
      .get({ $pylonRef: pylonRef, $runRef: runRef, $claimRef: claimRef }) as
        | { after_seq?: unknown }
        | null
    const value = Number(row?.after_seq ?? 0)
    return Number.isSafeInteger(value) && value >= 0 ? value : 0
  }

  getFleetRunSteeringOutcome(input: {
    readonly pylonRef: string
    readonly runRef: string
    readonly claimRef: string
    readonly seq: number
    readonly intentId: string
  }): FleetRunSteeringOutcomeRecord | null {
    const row = this.db
      .query(`
        SELECT pylon_ref, run_ref, claim_ref, seq, intent_id, intent_kind,
               intent_digest, outcome, outcome_ref, observed_at
          FROM pylon_orchestration_fleet_run_steering_outcomes
         WHERE pylon_ref = $pylonRef
           AND run_ref = $runRef
           AND claim_ref = $claimRef
           AND (seq = $seq OR intent_id = $intentId)
         ORDER BY seq ASC
         LIMIT 1
      `)
      .get({
        $pylonRef: input.pylonRef,
        $runRef: input.runRef,
        $claimRef: input.claimRef,
        $seq: input.seq,
        $intentId: input.intentId,
      }) as FleetRunSteeringOutcomeRow | null
    return row === null ? null : fleetRunSteeringOutcomeFromRow(row)
  }

  /**
   * Apply one decoded steering intent under the same SQLite write lock that
   * records its outcome, advances its run+claim watermark, and appends its
   * retryable ACK. A redelivery returns the original content-bound outcome
   * without invoking `apply` again.
   */
  applyFleetRunSteeringIntent(
    input: ApplyFleetRunSteeringIntentInput,
    apply: () => FleetRunSteeringApplication,
  ): { readonly recorded: boolean; readonly outcome: FleetRunSteeringOutcomeRecord } {
    if (
      !FLEET_RUN_ACTIVATION_REF_PATTERN.test(input.pylonRef) ||
      !FLEET_RUN_ACTIVATION_REF_PATTERN.test(input.runRef) ||
      !/^claim\.sarah_fleet_run\.[0-9a-f]{24}$/u.test(input.claimRef) ||
      !Number.isSafeInteger(input.seq) ||
      input.seq < 1 ||
      !FLEET_RUN_ACTIVATION_REF_PATTERN.test(input.intentId) ||
      !FLEET_RUN_ACTIVATION_REF_PATTERN.test(input.intentKind) ||
      !/^[0-9a-f]{64}$/u.test(input.intentDigest) ||
      Number.isNaN(input.observedAt.getTime())
    ) {
      throw new Error("fleet run steering application refs are invalid")
    }
    const observedAt = iso(input.observedAt)
    this.db.run("BEGIN IMMEDIATE")
    try {
      const run = this.getFleetRun(input.runRef)
      if (
        run?.authorityBinding?.phase !== "accepted" ||
        run.authorityBinding.claimRef !== input.claimRef ||
        run.authorityBinding.pylonRef !== input.pylonRef
      ) {
        throw new Error("fleet run steering requires the exact accepted authority claim")
      }

      const existing = this.getFleetRunSteeringOutcome(input)
      if (existing !== null) {
        if (
          existing.seq !== input.seq ||
          existing.intentId !== input.intentId ||
          existing.intentKind !== input.intentKind ||
          existing.intentDigest !== input.intentDigest ||
          existing.outcomeRef !== input.outcomeRefFor(
            existing.outcome,
            existing.observedAt,
          )
        ) {
          throw new Error("fleet run steering sequence or intent ref was reused with conflicting bytes")
        }
        this.db.run("COMMIT")
        return { outcome: existing, recorded: false }
      }

      const watermark = this.getFleetRunSteeringWatermark(
        input.pylonRef,
        input.runRef,
        input.claimRef,
      )
      if (input.seq <= watermark) {
        throw new Error("fleet run steering watermark has no matching durable outcome")
      }
      const pendingAcks = this.db
        .query(`
          SELECT COUNT(*) AS count
            FROM pylon_orchestration_fleet_run_steering_outbox
           WHERE pylon_ref = $pylonRef
             AND run_ref = $runRef
             AND claim_ref = $claimRef
             AND delivered_at IS NULL
        `)
        .get({
          $pylonRef: input.pylonRef,
          $runRef: input.runRef,
          $claimRef: input.claimRef,
        }) as { count?: unknown } | null
      if (Number(pendingAcks?.count ?? 0) >= 128) {
        throw new Error("fleet run steering acknowledgement backpressure")
      }

      const application = apply()
      const allowedOutcomes: ReadonlySet<string> = new Set([
        "applied",
        "queued_follow_up",
        "skipped_stale",
        "rejected",
        "failed",
      ])
      if (!allowedOutcomes.has(application.outcome)) {
        throw new Error("fleet run steering outcome is invalid")
      }
      if (
        (application.outcome === "queued_follow_up") !==
          (application.queuedFollowUp !== undefined)
      ) {
        throw new Error("fleet run steering follow-up does not match its outcome")
      }
      const outcomeRef = input.outcomeRefFor(application.outcome, observedAt)
      if (!FLEET_RUN_ACTIVATION_REF_PATTERN.test(outcomeRef)) {
        throw new Error("fleet run steering outcome ref is invalid")
      }

      this.db
        .query(`
          INSERT INTO pylon_orchestration_fleet_run_steering_outcomes
            (pylon_ref, run_ref, claim_ref, seq, intent_id, intent_kind,
             intent_digest, outcome, outcome_ref, observed_at)
          VALUES
            ($pylonRef, $runRef, $claimRef, $seq, $intentId, $intentKind,
             $intentDigest, $outcome, $outcomeRef, $observedAt)
        `)
        .run({
          $pylonRef: input.pylonRef,
          $runRef: input.runRef,
          $claimRef: input.claimRef,
          $seq: input.seq,
          $intentId: input.intentId,
          $intentKind: input.intentKind,
          $intentDigest: input.intentDigest,
          $outcome: application.outcome,
          $outcomeRef: outcomeRef,
          $observedAt: observedAt,
        })
      this.db
        .query(`
          INSERT INTO pylon_orchestration_fleet_run_steering_outbox
            (pylon_ref, run_ref, claim_ref, seq, intent_id, intent_kind,
             intent_digest, outcome, outcome_ref, observed_at, delivered_at)
          VALUES
            ($pylonRef, $runRef, $claimRef, $seq, $intentId, $intentKind,
             $intentDigest, $outcome, $outcomeRef, $observedAt, NULL)
        `)
        .run({
          $pylonRef: input.pylonRef,
          $runRef: input.runRef,
          $claimRef: input.claimRef,
          $seq: input.seq,
          $intentId: input.intentId,
          $intentKind: input.intentKind,
          $intentDigest: input.intentDigest,
          $outcome: application.outcome,
          $outcomeRef: outcomeRef,
          $observedAt: observedAt,
        })

      if (application.queuedFollowUp !== undefined) {
        const queued = application.queuedFollowUp
        const queuedCount = this.db
          .query(`
            SELECT COUNT(*) AS count
              FROM pylon_orchestration_fleet_run_steering_follow_ups
             WHERE pylon_ref = $pylonRef
               AND run_ref = $runRef
               AND claim_ref = $claimRef
               AND state IN ('queued', 'dispatching')
          `)
          .get({
            $pylonRef: input.pylonRef,
            $runRef: input.runRef,
            $claimRef: input.claimRef,
          }) as { count?: unknown } | null
        if (Number(queuedCount?.count ?? 0) >= 128) {
          throw new Error("fleet run steering private follow-up backpressure")
        }
        const pendingCompletions = this.db.query(`
          SELECT COUNT(*) AS count
            FROM pylon_orchestration_fleet_run_steering_completion_outbox
           WHERE pylon_ref = $pylonRef AND run_ref = $runRef AND claim_ref = $claimRef
             AND delivered_at IS NULL
        `).get({
          $pylonRef: input.pylonRef,
          $runRef: input.runRef,
          $claimRef: input.claimRef,
        }) as { count?: unknown } | null
        if (Number(pendingCompletions?.count ?? 0) >= 128) {
          throw new Error("fleet run steering completion acknowledgement backpressure")
        }
        const bodyBytes = new TextEncoder().encode(queued.body ?? "").byteLength
        const targetRefsValid = queued.intentKind === "fleet_run_control"
          ? queued.workUnitRef === null &&
            queued.workClaimRef === null &&
            queued.assignmentRef === null &&
            queued.targetRef === null &&
            queued.residualRefs.length > 0 &&
            queued.residualRefs.length % 2 === 0
          : queued.workUnitRef !== null &&
            queued.workClaimRef !== null &&
            queued.assignmentRef !== null &&
            queued.targetRef !== null &&
            FLEET_RUN_ACTIVATION_REF_PATTERN.test(queued.workUnitRef) &&
            FLEET_RUN_ACTIVATION_REF_PATTERN.test(queued.workClaimRef) &&
            FLEET_RUN_ACTIVATION_REF_PATTERN.test(queued.assignmentRef) &&
            FLEET_RUN_ACTIVATION_REF_PATTERN.test(queued.targetRef) &&
            queued.residualRefs.length === 0
        const intentFieldsValid = queued.intentKind === "fleet_run_control"
          ? queued.approvalRef === null && queued.decision === null &&
            queued.body === null && queued.bodyRef === null
          : queued.intentKind === "approval_decision"
            ? queued.approvalRef !== null &&
              FLEET_RUN_ACTIVATION_REF_PATTERN.test(queued.approvalRef) &&
              (queued.decision === "allow" || queued.decision === "deny") &&
              queued.body === null && queued.bodyRef === null
            : queued.approvalRef === null && queued.decision === null
        if (
          !targetRefsValid ||
          !intentFieldsValid ||
          queued.residualRefs.length > 128 ||
          queued.residualRefs.some(ref => !FLEET_RUN_ACTIVATION_REF_PATTERN.test(ref)) ||
          bodyBytes > 16 * 1_024 ||
          (queued.bodyRef !== null &&
            !FLEET_RUN_ACTIVATION_REF_PATTERN.test(queued.bodyRef))
        ) {
          throw new Error("fleet run steering queued follow-up is invalid")
        }
        this.db
          .query(`
            INSERT INTO pylon_orchestration_fleet_run_steering_follow_ups
              (pylon_ref, run_ref, claim_ref, seq, intent_id, work_unit_ref,
               work_claim_ref, assignment_ref, target_ref, intent_kind,
               approval_ref, decision, residual_refs_json, body, body_ref, created_at,
               state, attempt_count, next_attempt_at)
            VALUES
              ($pylonRef, $runRef, $claimRef, $seq, $intentId, $workUnitRef,
               $workClaimRef, $assignmentRef, $targetRef, $intentKind,
               $approvalRef, $decision, $residualRefsJson, $body, $bodyRef, $createdAt,
               'queued', 0, $createdAt)
          `)
          .run({
            $pylonRef: input.pylonRef,
            $runRef: input.runRef,
            $claimRef: input.claimRef,
            $seq: input.seq,
            $intentId: input.intentId,
            $workUnitRef: queued.workUnitRef,
            $workClaimRef: queued.workClaimRef,
            $assignmentRef: queued.assignmentRef,
            $targetRef: queued.targetRef,
            $intentKind: queued.intentKind,
            $approvalRef: queued.approvalRef,
            $decision: queued.decision,
            $residualRefsJson: JSON.stringify(queued.residualRefs),
            $body: queued.body,
            $bodyRef: queued.bodyRef,
            $createdAt: observedAt,
          })
      }

      this.db
        .query(`
          INSERT INTO pylon_orchestration_fleet_run_steering_watermarks
            (pylon_ref, run_ref, claim_ref, after_seq, updated_at)
          VALUES ($pylonRef, $runRef, $claimRef, $afterSeq, $updatedAt)
          ON CONFLICT(pylon_ref, run_ref, claim_ref) DO UPDATE SET
            after_seq = MAX(after_seq, excluded.after_seq),
            updated_at = excluded.updated_at
        `)
        .run({
          $pylonRef: input.pylonRef,
          $runRef: input.runRef,
          $claimRef: input.claimRef,
          $afterSeq: input.seq,
          $updatedAt: observedAt,
        })

      const stored = this.getFleetRunSteeringOutcome(input)
      if (stored === null) {
        throw new Error("fleet run steering outcome was not retained")
      }
      this.db.run("COMMIT")
      return { outcome: stored, recorded: true }
    } catch (error) {
      this.db.run("ROLLBACK")
      throw error
    }
  }

  listFleetRunSteeringOutcomeOutbox(input: {
    readonly pylonRef: string
    readonly runRef: string
    readonly claimRef: string
    readonly pendingOnly?: boolean
    readonly limit?: number
  }): FleetRunSteeringOutcomeOutboxEntry[] {
    const limit = Math.max(1, Math.min(100, Math.trunc(input.limit ?? 100)))
    const rows = this.db
      .query(`
        SELECT pylon_ref, run_ref, claim_ref, seq, intent_id, intent_kind,
               intent_digest, outcome, outcome_ref, observed_at, delivered_at
          FROM pylon_orchestration_fleet_run_steering_outbox
         WHERE pylon_ref = $pylonRef
           AND run_ref = $runRef
           AND claim_ref = $claimRef
           ${input.pendingOnly === false ? "" : "AND delivered_at IS NULL"}
         ORDER BY seq ASC
         LIMIT $limit
      `)
      .all({
        $pylonRef: input.pylonRef,
        $runRef: input.runRef,
        $claimRef: input.claimRef,
        $limit: limit,
      }) as FleetRunSteeringOutcomeOutboxRow[]
    return rows.map(fleetRunSteeringOutcomeOutboxFromRow)
  }

  markFleetRunSteeringOutcomeOutboxDelivered(
    entries: ReadonlyArray<Pick<
      FleetRunSteeringOutcomeOutboxEntry,
      "pylonRef" | "runRef" | "claimRef" | "seq" | "intentId" | "outcomeRef"
    >>,
    now: Date = new Date(),
  ): number {
    if (entries.length === 0) return 0
    const deliveredAt = iso(now)
    let changed = 0
    this.db.run("BEGIN IMMEDIATE")
    try {
      for (const entry of entries) {
        const before = this.db.query(`
          SELECT delivered_at
            FROM pylon_orchestration_fleet_run_steering_outbox
           WHERE pylon_ref = $pylonRef
             AND run_ref = $runRef
             AND claim_ref = $claimRef
             AND seq = $seq
             AND intent_id = $intentId
             AND outcome_ref = $outcomeRef
        `).get({
          $pylonRef: entry.pylonRef,
          $runRef: entry.runRef,
          $claimRef: entry.claimRef,
          $seq: entry.seq,
          $intentId: entry.intentId,
          $outcomeRef: entry.outcomeRef,
        }) as { delivered_at: string | null } | null
        if (before === null) continue
        this.db
          .query(`
            UPDATE pylon_orchestration_fleet_run_steering_outbox
               SET delivered_at = COALESCE(delivered_at, $deliveredAt)
             WHERE pylon_ref = $pylonRef
               AND run_ref = $runRef
               AND claim_ref = $claimRef
               AND seq = $seq
               AND intent_id = $intentId
               AND outcome_ref = $outcomeRef
          `)
          .run({
            $pylonRef: entry.pylonRef,
            $runRef: entry.runRef,
            $claimRef: entry.claimRef,
            $seq: entry.seq,
            $intentId: entry.intentId,
            $outcomeRef: entry.outcomeRef,
            $deliveredAt: deliveredAt,
          })
        const after = this.db.query(`
          SELECT delivered_at
            FROM pylon_orchestration_fleet_run_steering_outbox
           WHERE pylon_ref = $pylonRef
             AND run_ref = $runRef
             AND claim_ref = $claimRef
             AND seq = $seq
             AND intent_id = $intentId
             AND outcome_ref = $outcomeRef
        `).get({
          $pylonRef: entry.pylonRef,
          $runRef: entry.runRef,
          $claimRef: entry.claimRef,
          $seq: entry.seq,
          $intentId: entry.intentId,
          $outcomeRef: entry.outcomeRef,
        }) as { delivered_at: string | null } | null
        if (after?.delivered_at === null || after === null) {
          throw new Error("fleet run steering outcome delivery was not retained")
        }
        if (before.delivered_at === null) changed += 1
      }
      this.db.run("COMMIT")
      return changed
    } catch (error) {
      this.db.run("ROLLBACK")
      throw error
    }
  }

  listFleetRunSteeringQueuedFollowUps(input: {
    readonly pylonRef: string
    readonly runRef: string
    readonly claimRef: string
    readonly includeTerminal?: boolean
  }): FleetRunSteeringQueuedFollowUp[] {
    const rows = this.db
      .query(`
        SELECT pylon_ref, run_ref, claim_ref, seq, intent_id, work_unit_ref,
               work_claim_ref, assignment_ref, target_ref, intent_kind,
               approval_ref, decision, residual_refs_json, body, body_ref, created_at,
               state, attempt_count, next_attempt_at, last_attempt_at,
               dispatch_lease_expires_at, dispatch_lease_token, lease_generation,
               last_failure_ref, completion_ref, completed_at
          FROM pylon_orchestration_fleet_run_steering_follow_ups
         WHERE pylon_ref = $pylonRef
           AND run_ref = $runRef
           AND claim_ref = $claimRef
           ${input.includeTerminal === true ? "" : "AND state IN ('queued', 'dispatching')"}
         ORDER BY seq ASC
      `)
      .all({
        $pylonRef: input.pylonRef,
        $runRef: input.runRef,
        $claimRef: input.claimRef,
      }) as FleetRunSteeringQueuedFollowUpRow[]
    return rows.map(fleetRunSteeringQueuedFollowUpFromRow)
  }

  getFleetRunSteeringApprovalBinding(
    approvalRef: string,
  ): FleetRunSteeringApprovalBinding | null {
    const row = this.db.query(`
      SELECT approval_ref, pylon_ref, run_ref, claim_ref, work_unit_ref,
             work_claim_ref, assignment_ref, worker_kind, worker_ref,
             account_ref_hash, tool_class, state, decision, resolution_state,
             created_at, resolved_at, completion_ref
        FROM pylon_orchestration_fleet_run_steering_approval_bindings
       WHERE approval_ref = $approvalRef
    `).get({ $approvalRef: approvalRef }) as FleetRunSteeringApprovalBindingRow | null
    return row === null ? null : fleetRunSteeringApprovalBindingFromRow(row)
  }

  listFleetRunSteeringApprovalBindings(input: {
    readonly pylonRef: string
    readonly runRef: string
    readonly claimRef: string
    readonly pendingOnly?: boolean
  }): FleetRunSteeringApprovalBinding[] {
    if (
      !FLEET_RUN_ACTIVATION_REF_PATTERN.test(input.pylonRef) ||
      !FLEET_RUN_ACTIVATION_REF_PATTERN.test(input.runRef) ||
      !FLEET_RUN_ACTIVATION_REF_PATTERN.test(input.claimRef)
    ) throw new Error("fleet run steering approval binding scope is invalid")
    const rows = this.db.query(`
      SELECT approval_ref, pylon_ref, run_ref, claim_ref, work_unit_ref,
             work_claim_ref, assignment_ref, worker_kind, worker_ref,
             account_ref_hash, tool_class, state, decision, resolution_state,
             created_at, resolved_at, completion_ref
        FROM pylon_orchestration_fleet_run_steering_approval_bindings
       WHERE pylon_ref = $pylonRef
         AND run_ref = $runRef
         AND claim_ref = $claimRef
         ${input.pendingOnly === false ? "" : "AND state = 'pending'"}
       ORDER BY created_at ASC, approval_ref ASC
    `).all({
      $pylonRef: input.pylonRef,
      $runRef: input.runRef,
      $claimRef: input.claimRef,
    }) as FleetRunSteeringApprovalBindingRow[]
    return rows.map(fleetRunSteeringApprovalBindingFromRow)
  }

  /** Bind a pending approval to one exact live attempt. Replays are byte-identical. */
  bindFleetRunSteeringApproval(input: {
    readonly approvalRef: string
    readonly pylonRef: string
    readonly runRef: string
    readonly claimRef: string
    readonly workUnitRef: string
    readonly workClaimRef: string
    readonly assignmentRef: string
    readonly workerKind: "codex" | "claude" | "grok"
    readonly workerRef: string
    readonly accountRefHash: string
    readonly toolClass: string
    readonly now?: Date
  }): { readonly binding: FleetRunSteeringApprovalBinding; readonly created: boolean } {
    const authorityRefs = [
      input.pylonRef,
      input.runRef,
      input.claimRef,
      input.workUnitRef,
      input.workClaimRef,
    ]
    const projectedRefs = [
      input.approvalRef,
      input.assignmentRef,
      input.workerRef,
      input.accountRefHash,
      input.toolClass,
    ]
    const now = input.now ?? new Date()
    const provider = input.workerKind === "claude" ? "claude_agent" : input.workerKind
    if (
      authorityRefs.some(ref => !FLEET_RUN_ACTIVATION_REF_PATTERN.test(ref)) ||
      projectedRefs.some(ref => !FLEET_RUN_PROJECTED_REF_PATTERN.test(ref)) ||
      !FLEET_RUN_ACCOUNT_REF_HASH_PATTERN.test(input.accountRefHash) ||
      !input.accountRefHash.startsWith(`account.pylon.${provider}.`) ||
      !FLEET_RUN_APPROVAL_TOOL_CLASS_PATTERN.test(input.toolClass) ||
      Number.isNaN(now.getTime())
    ) {
      throw new Error("fleet run steering approval binding is invalid")
    }
    this.db.run("BEGIN IMMEDIATE")
    try {
      const existing = this.getFleetRunSteeringApprovalBinding(input.approvalRef)
      if (existing !== null) {
        if (
          existing.pylonRef !== input.pylonRef ||
          existing.runRef !== input.runRef ||
          existing.claimRef !== input.claimRef ||
          existing.workUnitRef !== input.workUnitRef ||
          existing.workClaimRef !== input.workClaimRef ||
          existing.assignmentRef !== input.assignmentRef ||
          existing.workerKind !== input.workerKind ||
          existing.workerRef !== input.workerRef ||
          existing.accountRefHash !== input.accountRefHash ||
          existing.toolClass !== input.toolClass
        ) {
          throw new Error("fleet run steering approval ref was rebound")
        }
        this.db.run("COMMIT")
        return { binding: existing, created: false }
      }
      const run = this.getFleetRun(input.runRef)
      const workClaim = this.getWorkClaim(input.workClaimRef)
      const expectedAccountRefHash = workClaim === null
        ? null
        : `account.pylon.${provider}.${createHash("sha256")
          .update(`${provider}:${workClaim.workerAccountRef}`)
          .digest("hex")
          .slice(0, 24)}`
      if (
        run?.authorityBinding?.phase !== "accepted" ||
        run.authorityBinding.pylonRef !== input.pylonRef ||
        run.authorityBinding.claimRef !== input.claimRef ||
        workClaim === null ||
        workClaim.runRef !== input.runRef ||
        workClaim.workUnitRef !== input.workUnitRef ||
        expectedAccountRefHash !== input.accountRefHash ||
        (workClaim.assignmentRef !== null && workClaim.assignmentRef !== input.assignmentRef) ||
        (workClaim.state !== "claimed" && workClaim.state !== "in_progress")
      ) {
        throw new Error("fleet run steering approval requires an exact live attempt")
      }
      if (workClaim.assignmentRef === null) {
        this.db.query(`
          UPDATE pylon_orchestration_work_claims
             SET assignment_ref = $assignmentRef, updated_at = $updatedAt
           WHERE claim_ref = $workClaimRef
             AND assignment_ref IS NULL
             AND state IN ('claimed', 'in_progress')
        `).run({
          $workClaimRef: input.workClaimRef,
          $assignmentRef: input.assignmentRef,
          $updatedAt: iso(now),
        })
        const assigned = this.getWorkClaim(input.workClaimRef)
        if (
          assigned === null ||
          assigned.assignmentRef !== input.assignmentRef ||
          (assigned.state !== "claimed" && assigned.state !== "in_progress")
        ) {
          throw new Error("fleet run steering approval lost its exact live attempt")
        }
      }
      this.db.query(`
        INSERT INTO pylon_orchestration_fleet_run_steering_approval_bindings
          (approval_ref, pylon_ref, run_ref, claim_ref, work_unit_ref,
           work_claim_ref, assignment_ref, worker_kind, worker_ref,
           account_ref_hash, tool_class, state, decision, resolution_state,
           created_at, resolved_at, completion_ref)
        VALUES
          ($approvalRef, $pylonRef, $runRef, $claimRef, $workUnitRef,
           $workClaimRef, $assignmentRef, $workerKind, $workerRef,
           $accountRefHash, $toolClass, 'pending', NULL, NULL, $createdAt, NULL, NULL)
      `).run({
        $approvalRef: input.approvalRef,
        $pylonRef: input.pylonRef,
        $runRef: input.runRef,
        $claimRef: input.claimRef,
        $workUnitRef: input.workUnitRef,
        $workClaimRef: input.workClaimRef,
        $assignmentRef: input.assignmentRef,
        $workerKind: input.workerKind,
        $workerRef: input.workerRef,
        $accountRefHash: input.accountRefHash,
        $toolClass: input.toolClass,
        $createdAt: iso(now),
      })
      const binding = this.getFleetRunSteeringApprovalBinding(input.approvalRef)
      if (binding === null) throw new Error("fleet run steering approval binding was not retained")
      this.db.run("COMMIT")
      return { binding, created: true }
    } catch (error) {
      this.db.run("ROLLBACK")
      throw error
    }
  }

  getFleetRunSteeringFollowUp(input: {
    readonly pylonRef: string
    readonly runRef: string
    readonly claimRef: string
    readonly seq: number
    readonly intentId: string
  }): FleetRunSteeringQueuedFollowUp | null {
    const row = this.db.query(`
      SELECT pylon_ref, run_ref, claim_ref, seq, intent_id, work_unit_ref,
             work_claim_ref, assignment_ref, target_ref, intent_kind,
             approval_ref, decision, residual_refs_json, body, body_ref, created_at,
             state, attempt_count, next_attempt_at, last_attempt_at,
             dispatch_lease_expires_at, dispatch_lease_token, lease_generation,
             last_failure_ref, completion_ref, completed_at
        FROM pylon_orchestration_fleet_run_steering_follow_ups
       WHERE pylon_ref = $pylonRef AND run_ref = $runRef AND claim_ref = $claimRef
         AND seq = $seq AND intent_id = $intentId
    `).get({
      $pylonRef: input.pylonRef,
      $runRef: input.runRef,
      $claimRef: input.claimRef,
      $seq: input.seq,
      $intentId: input.intentId,
    }) as FleetRunSteeringQueuedFollowUpRow | null
    return row === null ? null : fleetRunSteeringQueuedFollowUpFromRow(row)
  }

  /** Lease exactly one oldest due follow-up. Expired dispatch leases recover after restart. */
  acquireFleetRunSteeringFollowUp(input: {
    readonly pylonRef: string
    readonly runRef: string
    readonly claimRef: string
    readonly now?: Date
    readonly leaseMs?: number
  }): FleetRunSteeringQueuedFollowUp | null {
    const now = input.now ?? new Date()
    const leaseMs = input.leaseMs ?? 30_000
    if (Number.isNaN(now.getTime()) || !Number.isInteger(leaseMs) || leaseMs < 1_000 || leaseMs > 300_000) {
      throw new Error("fleet run steering follow-up lease is invalid")
    }
    const at = iso(now)
    const leaseExpiresAt = iso(new Date(now.getTime() + leaseMs))
    this.db.run("BEGIN IMMEDIATE")
    try {
      this.db.query(`
        UPDATE pylon_orchestration_fleet_run_steering_follow_ups
           SET state = 'queued', dispatch_lease_expires_at = NULL,
               dispatch_lease_token = NULL,
               next_attempt_at = $at,
               last_failure_ref = COALESCE(last_failure_ref,
                 'blocker.pylon.fleet_steering.dispatch_interrupted')
         WHERE pylon_ref = $pylonRef AND run_ref = $runRef AND claim_ref = $claimRef
           AND state = 'dispatching' AND dispatch_lease_expires_at <= $at
      `).run({
        $pylonRef: input.pylonRef,
        $runRef: input.runRef,
        $claimRef: input.claimRef,
        $at: at,
      })
      const candidate = this.db.query(`
        SELECT seq, intent_id, state, next_attempt_at, lease_generation
          FROM pylon_orchestration_fleet_run_steering_follow_ups
         WHERE pylon_ref = $pylonRef AND run_ref = $runRef AND claim_ref = $claimRef
           AND state IN ('queued', 'dispatching')
         ORDER BY seq ASC
         LIMIT 1
      `).get({
        $pylonRef: input.pylonRef,
        $runRef: input.runRef,
        $claimRef: input.claimRef,
      }) as {
        seq: number
        intent_id: string
        state: "queued" | "dispatching"
        next_attempt_at: string
        lease_generation: number
      } | null
      // True head-of-line ordering: a live lease or backed-off oldest intent
      // fences every later sequence until the oldest intent is terminal.
      if (
        candidate === null ||
        candidate.state === "dispatching" ||
        candidate.next_attempt_at > at
      ) {
        this.db.run("COMMIT")
        return null
      }
      const leaseGeneration = candidate.lease_generation + 1
      if (!Number.isSafeInteger(leaseGeneration) || leaseGeneration < 1) {
        throw new Error("fleet run steering follow-up lease generation overflow")
      }
      const leaseToken = `lease.pylon.fleet_steering.${createHash("sha256")
        .update(JSON.stringify({
          pylonRef: input.pylonRef,
          runRef: input.runRef,
          claimRef: input.claimRef,
          seq: candidate.seq,
          intentId: candidate.intent_id,
          leaseGeneration,
          acquiredAt: at,
        }))
        .digest("hex")
        .slice(0, 24)}`
      this.db.query(`
        UPDATE pylon_orchestration_fleet_run_steering_follow_ups
           SET state = 'dispatching', attempt_count = attempt_count + 1,
               last_attempt_at = $at, dispatch_lease_expires_at = $leaseExpiresAt,
               dispatch_lease_token = $leaseToken, lease_generation = $leaseGeneration
         WHERE pylon_ref = $pylonRef AND run_ref = $runRef AND claim_ref = $claimRef
           AND seq = $seq AND intent_id = $intentId AND state = 'queued'
           AND lease_generation = $previousLeaseGeneration
      `).run({
        $pylonRef: input.pylonRef,
        $runRef: input.runRef,
        $claimRef: input.claimRef,
        $seq: candidate.seq,
        $intentId: candidate.intent_id,
        $at: at,
        $leaseExpiresAt: leaseExpiresAt,
        $leaseToken: leaseToken,
        $leaseGeneration: leaseGeneration,
        $previousLeaseGeneration: candidate.lease_generation,
      })
      const leased = this.getFleetRunSteeringFollowUp({
        pylonRef: input.pylonRef,
        runRef: input.runRef,
        claimRef: input.claimRef,
        seq: candidate.seq,
        intentId: candidate.intent_id,
      })
      if (
        leased?.state !== "dispatching" ||
        leased.dispatchLeaseToken !== leaseToken ||
        leased.leaseGeneration !== leaseGeneration
      ) throw new Error("fleet run steering follow-up lease was lost")
      this.db.run("COMMIT")
      return leased
    } catch (error) {
      this.db.run("ROLLBACK")
      throw error
    }
  }

  retryFleetRunSteeringFollowUp(input: {
    readonly followUp: FleetRunSteeringQueuedFollowUp
    readonly nextAttemptAt: Date
    readonly failureRef: string
  }): FleetRunSteeringQueuedFollowUp {
    if (
      Number.isNaN(input.nextAttemptAt.getTime()) ||
      !FLEET_RUN_ACTIVATION_REF_PATTERN.test(input.failureRef) ||
      input.followUp.dispatchLeaseToken === null ||
      !FLEET_RUN_ACTIVATION_REF_PATTERN.test(input.followUp.dispatchLeaseToken) ||
      !Number.isSafeInteger(input.followUp.leaseGeneration) ||
      input.followUp.leaseGeneration < 1
    ) throw new Error("fleet run steering follow-up retry is invalid")
    const changed = this.db.query(`
      UPDATE pylon_orchestration_fleet_run_steering_follow_ups
         SET state = 'queued', next_attempt_at = $nextAttemptAt,
             dispatch_lease_expires_at = NULL, dispatch_lease_token = NULL,
             last_failure_ref = $failureRef
       WHERE pylon_ref = $pylonRef AND run_ref = $runRef AND claim_ref = $claimRef
         AND seq = $seq AND intent_id = $intentId AND state = 'dispatching'
         AND lease_generation = $leaseGeneration AND dispatch_lease_token = $leaseToken
    `).run({
      $pylonRef: input.followUp.pylonRef,
      $runRef: input.followUp.runRef,
      $claimRef: input.followUp.claimRef,
      $seq: input.followUp.seq,
      $intentId: input.followUp.intentId,
      $nextAttemptAt: iso(input.nextAttemptAt),
      $failureRef: input.failureRef,
      $leaseGeneration: input.followUp.leaseGeneration,
      $leaseToken: input.followUp.dispatchLeaseToken,
    })
    if (Number(changed.changes) !== 1) throw new Error("fleet run steering follow-up retry lost its lease")
    const updated = this.getFleetRunSteeringFollowUp(input.followUp)
    if (updated === null) throw new Error("fleet run steering follow-up retry was not retained")
    return updated
  }

  completeFleetRunSteeringFollowUp(input: {
    readonly followUp: FleetRunSteeringQueuedFollowUp
    readonly state: FleetRunSteeringFollowUpCompletion["state"]
    readonly completionRef: string
    readonly failureRef?: string | null
    readonly completedAt?: Date
  }): FleetRunSteeringFollowUpCompletion {
    const completedAt = input.completedAt ?? new Date()
    const failureRef = input.failureRef ?? null
    if (
      Number.isNaN(completedAt.getTime()) ||
      !FLEET_RUN_ACTIVATION_REF_PATTERN.test(input.completionRef) ||
      (failureRef !== null && !FLEET_RUN_ACTIVATION_REF_PATTERN.test(failureRef)) ||
      (input.state === "applied" && failureRef !== null) ||
      input.followUp.dispatchLeaseToken === null ||
      !FLEET_RUN_ACTIVATION_REF_PATTERN.test(input.followUp.dispatchLeaseToken) ||
      !Number.isSafeInteger(input.followUp.leaseGeneration) ||
      input.followUp.leaseGeneration < 1
    ) throw new Error("fleet run steering follow-up completion is invalid")
    const at = iso(completedAt)
    this.db.run("BEGIN IMMEDIATE")
    try {
      const current = this.getFleetRunSteeringFollowUp(input.followUp)
      if (current === null) throw new Error("unknown fleet run steering follow-up")
      if (
        current.dispatchLeaseToken !== input.followUp.dispatchLeaseToken ||
        current.leaseGeneration !== input.followUp.leaseGeneration
      ) throw new Error("fleet run steering follow-up lease fence was lost")
      if (current.state === "applied" || current.state === "failed" || current.state === "stale") {
        if (
          current.state !== input.state ||
          current.completionRef !== input.completionRef ||
          current.completedAt !== at ||
          current.lastFailureRef !== failureRef
        ) {
          throw new Error("fleet run steering follow-up was completed differently")
        }
      } else {
        if (current.state !== "dispatching") throw new Error("fleet run steering follow-up is not leased")
        this.db.query(`
          UPDATE pylon_orchestration_fleet_run_steering_follow_ups
             SET state = $state, dispatch_lease_expires_at = NULL,
                 completion_ref = $completionRef, completed_at = $completedAt,
                 last_failure_ref = $failureRef
           WHERE pylon_ref = $pylonRef AND run_ref = $runRef AND claim_ref = $claimRef
             AND seq = $seq AND intent_id = $intentId AND state = 'dispatching'
             AND lease_generation = $leaseGeneration AND dispatch_lease_token = $leaseToken
        `).run({
          $pylonRef: current.pylonRef,
          $runRef: current.runRef,
          $claimRef: current.claimRef,
          $seq: current.seq,
          $intentId: current.intentId,
          $state: input.state,
          $completionRef: input.completionRef,
          $completedAt: at,
          $failureRef: failureRef,
          $leaseGeneration: current.leaseGeneration,
          $leaseToken: current.dispatchLeaseToken,
        })
        const transitioned = this.getFleetRunSteeringFollowUp(current)
        if (
          transitioned === null ||
          transitioned.state !== input.state ||
          transitioned.completionRef !== input.completionRef ||
          transitioned.completedAt !== at ||
          transitioned.lastFailureRef !== failureRef
        ) {
          throw new Error("fleet run steering follow-up completion lost its lease")
        }
        if (current.intentKind === "approval_decision") {
          if (current.approvalRef === null || current.decision === null) {
            throw new Error("fleet run steering approval follow-up lost its decision")
          }
          const binding = this.getFleetRunSteeringApprovalBinding(current.approvalRef)
          if (
            binding === null || binding.state !== "pending" ||
            binding.pylonRef !== current.pylonRef || binding.runRef !== current.runRef ||
            binding.claimRef !== current.claimRef || binding.workUnitRef !== current.workUnitRef ||
            binding.workClaimRef !== current.workClaimRef || binding.assignmentRef !== current.assignmentRef
          ) throw new Error("fleet run steering approval binding is no longer exact")
          this.db.query(`
            UPDATE pylon_orchestration_fleet_run_steering_approval_bindings
               SET state = 'resolved', decision = $decision,
                   resolution_state = $resolutionState,
                   resolved_at = $resolvedAt, completion_ref = $completionRef
             WHERE approval_ref = $approvalRef AND state = 'pending'
          `).run({
            $approvalRef: current.approvalRef,
            $decision: current.decision,
            $resolutionState: input.state,
            $resolvedAt: at,
            $completionRef: input.completionRef,
          })
        }
        this.db.query(`
          INSERT INTO pylon_orchestration_fleet_run_steering_completion_outbox
            (pylon_ref, run_ref, claim_ref, seq, intent_id, intent_kind, state,
             work_unit_ref, work_claim_ref, assignment_ref, approval_ref,
             completion_ref, completed_at, failure_ref, delivered_at)
          VALUES
            ($pylonRef, $runRef, $claimRef, $seq, $intentId, $intentKind, $state,
             $workUnitRef, $workClaimRef, $assignmentRef, $approvalRef,
             $completionRef, $completedAt, $failureRef, NULL)
        `).run({
          $pylonRef: current.pylonRef,
          $runRef: current.runRef,
          $claimRef: current.claimRef,
          $seq: current.seq,
          $intentId: current.intentId,
          $intentKind: current.intentKind,
          $state: input.state,
          $workUnitRef: current.workUnitRef,
          $workClaimRef: current.workClaimRef,
          $assignmentRef: current.assignmentRef,
          $approvalRef: current.approvalRef,
          $completionRef: input.completionRef,
          $completedAt: at,
          $failureRef: failureRef,
        })
      }
      const row = this.db.query(`
        SELECT pylon_ref, run_ref, claim_ref, seq, intent_id, intent_kind, state,
               work_unit_ref, work_claim_ref, assignment_ref, approval_ref,
               completion_ref, completed_at, failure_ref, delivered_at
          FROM pylon_orchestration_fleet_run_steering_completion_outbox
         WHERE completion_ref = $completionRef
      `).get({ $completionRef: input.completionRef }) as FleetRunSteeringFollowUpCompletionRow | null
      if (row === null) throw new Error("fleet run steering completion was not retained")
      this.db.run("COMMIT")
      return fleetRunSteeringFollowUpCompletionFromRow(row)
    } catch (error) {
      this.db.run("ROLLBACK")
      throw error
    }
  }

  listFleetRunSteeringFollowUpCompletionOutbox(input: {
    readonly pylonRef: string
    readonly runRef: string
    readonly claimRef: string
    readonly pendingOnly?: boolean
    readonly limit?: number
  }): FleetRunSteeringFollowUpCompletion[] {
    const limit = Math.max(1, Math.min(100, Math.trunc(input.limit ?? 100)))
    const rows = this.db.query(`
      SELECT pylon_ref, run_ref, claim_ref, seq, intent_id, intent_kind, state,
             work_unit_ref, work_claim_ref, assignment_ref, approval_ref,
             completion_ref, completed_at, failure_ref, delivered_at
        FROM pylon_orchestration_fleet_run_steering_completion_outbox
       WHERE pylon_ref = $pylonRef AND run_ref = $runRef AND claim_ref = $claimRef
         ${input.pendingOnly === false ? "" : "AND delivered_at IS NULL"}
       ORDER BY seq ASC
       LIMIT $limit
    `).all({
      $pylonRef: input.pylonRef,
      $runRef: input.runRef,
      $claimRef: input.claimRef,
      $limit: limit,
    }) as FleetRunSteeringFollowUpCompletionRow[]
    return rows.map(fleetRunSteeringFollowUpCompletionFromRow)
  }

  markFleetRunSteeringFollowUpCompletionsDelivered(
    entries: ReadonlyArray<Pick<FleetRunSteeringFollowUpCompletion, "completionRef">>,
    now: Date = new Date(),
  ): number {
    if (entries.length === 0) return 0
    if (Number.isNaN(now.getTime())) throw new Error("fleet run steering completion delivery clock is invalid")
    let changed = 0
    this.db.run("BEGIN IMMEDIATE")
    try {
      for (const entry of entries) {
        const before = this.db.query(`
          SELECT delivered_at
            FROM pylon_orchestration_fleet_run_steering_completion_outbox
           WHERE completion_ref = $completionRef
        `).get({
          $completionRef: entry.completionRef,
        }) as { delivered_at: string | null } | null
        if (before === null) continue
        this.db.query(`
          UPDATE pylon_orchestration_fleet_run_steering_completion_outbox
             SET delivered_at = COALESCE(delivered_at, $deliveredAt)
           WHERE completion_ref = $completionRef
        `).run({
          $completionRef: entry.completionRef,
          $deliveredAt: iso(now),
        })
        const after = this.db.query(`
          SELECT delivered_at
            FROM pylon_orchestration_fleet_run_steering_completion_outbox
           WHERE completion_ref = $completionRef
        `).get({
          $completionRef: entry.completionRef,
        }) as { delivered_at: string | null } | null
        if (after?.delivered_at === null || after === null) {
          throw new Error("fleet run steering completion delivery was not retained")
        }
        if (before.delivered_at === null) changed += 1
      }
      this.db.run("COMMIT")
      return changed
    } catch (error) {
      this.db.run("ROLLBACK")
      throw error
    }
  }

  updateFleetRunState(
    runRef: string,
    state: FleetRunState,
    now: Date = new Date(),
    stateSource: "operator" | "reconcile" = "operator",
  ): FleetRun {
    const current = this.getFleetRun(runRef)
    if (current === null) throw new Error(`unknown fleet run: ${runRef}`)
    return this.upsertFleetRun({
      ...current,
      state,
      stateSource,
      startedAt: state === "running" && current.startedAt === null ? iso(now) : current.startedAt,
      updatedAt: iso(now),
    })
  }

  controlFleetRun(
    runRef: string,
    verb: FleetRunControlVerb,
    now: Date = new Date(),
  ): { previousState: FleetRunState; run: FleetRun } {
    const current = this.getFleetRun(runRef)
    if (current === null) throw new Error(`unknown fleet run: ${runRef}`)
    assertFleetRunControlTransition(current.state, verb)
    return {
      previousState: current.state,
      run: this.updateFleetRunState(runRef, fleetRunControlState(verb), now),
    }
  }

  reconcileFleetRun(runRef: string, now: Date = new Date()): FleetRun {
    const run = this.getFleetRun(runRef)
    if (run === null) throw new Error(`unknown fleet run: ${runRef}`)
    const tasks = this.listTasks().filter((task) => task.spec.fleetRunRef === runRef)
    const counters: FleetRunCounters = {
      workUnitsTotal: tasks.length,
      activeAssignments: tasks.filter((task) => task.status === "dispatched").length,
      completedAssignments: tasks.filter((task) => task.status === "completed").length,
      failedAssignments: tasks.filter((task) => task.status === "failed").length,
      blockedAssignments: tasks.filter((task) => task.status === "blocked").length,
    }
    const terminalCount = counters.completedAssignments + counters.failedAssignments + counters.blockedAssignments
    const shouldClose =
      tasks.length > 0 &&
      terminalCount === tasks.length &&
      counters.activeAssignments === 0 &&
      (run.state === "running" || run.state === "draining")
    const state: FleetRunState = shouldClose && counters.failedAssignments === 0 && counters.blockedAssignments === 0
      ? "completed"
      : shouldClose ? "stopped" : run.state
    // Closing a running run is a machine decision and may be undone by the
    // supervisor when planner backlog remains. Closing a draining run is the
    // completion of an operator drain: keep operator provenance so the closed
    // run is never auto-revived (#7975).
    const stateSource = shouldClose
      ? (run.state === "draining" ? ("operator" as const) : ("reconcile" as const))
      : run.stateSource
    const projectionChanged =
      state !== run.state ||
      stateSource !== run.stateSource ||
      counters.workUnitsTotal !== run.counters.workUnitsTotal ||
      counters.activeAssignments !== run.counters.activeAssignments ||
      counters.completedAssignments !== run.counters.completedAssignments ||
      counters.failedAssignments !== run.counters.failedAssignments ||
      counters.blockedAssignments !== run.counters.blockedAssignments
    return this.upsertFleetRun({
      ...run,
      state,
      ...(stateSource === undefined ? {} : { stateSource }),
      counters,
      // Stable unchanged rows make terminal observations idempotent across
      // standing-loop ticks; state/counter transitions still advance time.
      updatedAt: projectionChanged ? iso(now) : run.updatedAt,
    })
  }

  reconcileFleetRuns(now: Date = new Date()): FleetRun[] {
    return this.listFleetRuns().map((run) => this.reconcileFleetRun(run.runRef, now))
  }

  private getMeta(key: string): string | null {
    const row = this.db
      .query("SELECT value FROM pylon_orchestration_meta WHERE key = $key")
      .get({ $key: key }) as { value: string } | null
    return row?.value ?? null
  }

  private setMeta(key: string, value: string): void {
    this.db
      .query("INSERT OR REPLACE INTO pylon_orchestration_meta (key, value) VALUES ($key, $value)")
      .run({ $key: key, $value: value })
  }

  private fleetIntentWatermarkKey(scope?: string): string {
    return scope === undefined ? "fleet_intents_watermark" : `fleet_intents_watermark.${scope}`
  }

  /**
   * Persisted fleet-intent consumption watermark (KS-3.2 #8332): the highest
   * `khala_sync_fleet_intents.id` this supervisor has consumed, resumed
   * across restarts. One global watermark by default; a per-scope key when
   * the enforcement loop polls a single fleet scope.
   */
  getFleetIntentWatermark(scope?: string): number {
    const raw = this.getMeta(this.fleetIntentWatermarkKey(scope))
    if (raw === null) return 0
    const value = Number(raw)
    return Number.isInteger(value) && value >= 0 ? value : 0
  }

  setFleetIntentWatermark(after: number, scope?: string): void {
    if (!Number.isInteger(after) || after < 0) {
      throw new Error("fleet intent watermark must be a non-negative integer")
    }
    this.setMeta(this.fleetIntentWatermarkKey(scope), String(after))
  }

  /**
   * Operator desired-slots cap (KS-3.2 #8332), the durable overlay written
   * by `set_desired_slots` intents. Kept separate from
   * `FleetRun.targetConcurrency` because the supervisor heartbeat re-derives
   * targetConcurrency from local account capacity on every beat; the cap
   * survives those beats and bounds the effective slots.
   */
  getFleetRunDesiredSlotsCap(runRef: string): number | null {
    const raw = this.getMeta(`fleet_intent_desired_slots_cap.${runRef}`)
    if (raw === null) return null
    const value = Number(raw)
    return Number.isInteger(value) && value >= 0 ? value : null
  }

  setFleetRunDesiredSlotsCap(runRef: string, cap: number | null): void {
    const key = `fleet_intent_desired_slots_cap.${runRef}`
    if (cap === null) {
      this.db.query("DELETE FROM pylon_orchestration_meta WHERE key = $key").run({ $key: key })
      return
    }
    if (!Number.isInteger(cap) || cap < 0) {
      throw new Error("fleet run desired-slots cap must be a non-negative integer")
    }
    this.setMeta(key, String(cap))
  }

  /**
   * The slots the supervisor loop should actually dispatch at: 0 when the
   * run is paused/draining/stopped/completed (or unknown), else the local
   * capacity (`targetConcurrency`) bounded by the operator cap.
   */
  effectiveFleetRunDesiredSlots(runRef: string): number {
    const run = this.getFleetRun(runRef)
    if (run === null) return 0
    if (run.state !== "running" && run.state !== "draft") return 0
    const cap = this.getFleetRunDesiredSlotsCap(runRef)
    return cap === null ? run.targetConcurrency : Math.min(run.targetConcurrency, cap)
  }

  setDispatchContextPaused(id: string, paused: boolean, now: Date = new Date()): DispatchContext {
    const current = this.getDispatchContext(id)
    if (current === null) throw new Error(`unknown dispatch context: ${id}`)
    this.db
      .query(`
        UPDATE pylon_orchestration_dispatch_contexts
           SET paused = $paused,
               updated_at = $now
         WHERE id = $id
      `)
      .run({ $id: id, $paused: paused ? 1 : 0, $now: iso(now) })
    return this.getDispatchContext(id) ?? current
  }

  getFleetIntentOutcome(intentId: number): FleetIntentOutcomeRecord | null {
    const row = this.db
      .query("SELECT * FROM pylon_orchestration_fleet_intent_outcomes WHERE intent_id = $intentId")
      .get({ $intentId: intentId }) as FleetIntentOutcomeRow | null
    return row === null ? null : fleetIntentOutcomeFromRow(row)
  }

  listFleetIntentOutcomes(input: { runRef?: string } = {}): FleetIntentOutcomeRecord[] {
    const rows = input.runRef === undefined
      ? this.db
        .query("SELECT * FROM pylon_orchestration_fleet_intent_outcomes ORDER BY intent_id ASC")
        .all()
      : this.db
        .query(`
          SELECT * FROM pylon_orchestration_fleet_intent_outcomes
           WHERE run_ref = $runRef
           ORDER BY intent_id ASC
        `)
        .all({ $runRef: input.runRef })
    return (rows as FleetIntentOutcomeRow[]).map(fleetIntentOutcomeFromRow)
  }

  /**
   * Record one intent application outcome exactly once. A second record for
   * the same `intentId` (route redelivery, restart replay) is refused and
   * the FIRST recorded outcome is returned with `recorded: false` — callers
   * must treat that as "already applied, do not re-apply".
   */
  recordFleetIntentOutcome(
    input: RecordFleetIntentOutcomeInput,
  ): { recorded: boolean; outcome: FleetIntentOutcomeRecord } {
    const existing = this.getFleetIntentOutcome(input.intentId)
    if (existing !== null) return { outcome: existing, recorded: false }
    this.db
      .query(`
        INSERT OR IGNORE INTO pylon_orchestration_fleet_intent_outcomes
          (intent_id, scope, run_ref, intent, outcome, detail, mutation_ref, recorded_at)
        VALUES
          ($intentId, $scope, $runRef, $intent, $outcome, $detail, $mutationRef, $recordedAt)
      `)
      .run({
        $intentId: input.intentId,
        $scope: input.scope,
        $runRef: input.runRef,
        $intent: input.intent,
        $outcome: input.outcome,
        $detail: input.detail ?? null,
        $mutationRef: input.mutationRef,
        $recordedAt: iso(input.now),
      })
    const stored = this.getFleetIntentOutcome(input.intentId)
    if (stored === null) throw new Error(`failed to record fleet intent outcome ${input.intentId}`)
    return { outcome: stored, recorded: true }
  }

  /**
   * Persisted runtime control-intent consumption watermark (#8388): the
   * highest `khala_sync_runtime_control_intents.seq` this Pylon's runtime
   * dispatch consumer has consumed, resumed across restarts. Mirrors
   * `getFleetIntentWatermark`/`setFleetIntentWatermark` above.
   */
  getRuntimeIntentWatermark(scope?: string): number {
    const raw = this.getMeta(this.runtimeIntentWatermarkKey(scope))
    if (raw === null) return 0
    const value = Number(raw)
    return Number.isInteger(value) && value >= 0 ? value : 0
  }

  setRuntimeIntentWatermark(after: number, scope?: string): void {
    if (!Number.isInteger(after) || after < 0) {
      throw new Error("runtime intent watermark must be a non-negative integer")
    }
    this.setMeta(this.runtimeIntentWatermarkKey(scope), String(after))
  }

  private runtimeIntentWatermarkKey(scope?: string): string {
    return scope === undefined ? "runtime_intents_watermark" : `runtime_intents_watermark.${scope}`
  }

  /**
   * Last known Codex SDK thread id for a Khala Sync chat thread (#8388
   * follow-up: cross-turn continuity). Captured from the SDK's own
   * `thread.started` event the first time a `turn.start` dispatch runs for
   * this Khala thread, and reused on the NEXT dispatch (an ordinary
   * `turn.start`, or a Pylon-authored follow-up turn seeded from a queued
   * `message.append`) via `Codex#resumeThread(id)` so the conversation
   * keeps its context instead of starting fresh every turn. Best-effort
   * only: if the account that resumes differs from the one that created
   * the thread (each account has an isolated `~/.codex`-equivalent home),
   * the resume attempt fails cleanly and the dispatch reports a normal
   * `turn.finished` error — never a crash — so a stale/mismatched mapping
   * is safe to keep around.
   */
  getRuntimeCodexThreadId(threadId: string): string | null {
    return this.getMeta(this.runtimeCodexThreadIdKey(threadId))
  }

  setRuntimeCodexThreadId(threadId: string, codexThreadId: string): void {
    this.setMeta(this.runtimeCodexThreadIdKey(threadId), codexThreadId)
  }

  private runtimeCodexThreadIdKey(threadId: string): string {
    return `runtime_codex_thread_id.${threadId}`
  }

  /**
   * The Claude Agent SDK analogue of `getRuntimeCodexThreadId`/
   * `setRuntimeCodexThreadId` (#8404): the SDK's own `session_id` (present on
   * every `SDKMessage`, captured from the first message of a `claude_pylon`
   * dispatch) so a LATER turn in the same Khala Sync thread can pass it back
   * as `options.resume` and keep the model's prior context. Same best-effort
   * contract as the Codex thread id: if the account resuming differs from the
   * one that created the session (isolated per-account `CLAUDE_CONFIG_DIR`
   * homes), the resume attempt fails cleanly into a normal `turn.finished`
   * error, never a crash.
   */
  getRuntimeClaudeSessionId(threadId: string): string | null {
    return this.getMeta(this.runtimeClaudeSessionIdKey(threadId))
  }

  setRuntimeClaudeSessionId(threadId: string, claudeSessionId: string): void {
    this.setMeta(this.runtimeClaudeSessionIdKey(threadId), claudeSessionId)
  }

  private runtimeClaudeSessionIdKey(threadId: string): string {
    return `runtime_claude_session_id.${threadId}`
  }

  /**
   * Pinned dispatch account for a Khala Sync thread (#8410 follow-up:
   * thread-resume account affinity). Codex (`Codex#resumeThread`) and Claude
   * (`options.resume`) sessions are account-specific — resuming under a
   * DIFFERENT account than the one that created the thread/session fails
   * cleanly (never a crash, but loses conversation context). Once a Khala
   * thread's first `turn.start` dispatch picks an account,
   * `handleTurnStart` pins that SAME account for every later dispatch to the
   * thread, as long as the pinned account is still in the real dispatch-ready
   * set (see `candidateAccountsFromRegistry`'s real per-account readiness
   * check) — round-robin fairness ACROSS different threads is unaffected;
   * only a single thread's account choice becomes sticky. If the pinned
   * account becomes unhealthy (revoked/rate-limited/quota-exhausted),
   * `handleTurnStart` falls through to ordinary `selectDispatchAccount`
   * round-robin and RE-PINS to whichever account gets picked next — a thread
   * is never wedged on a dead account, it just loses continuity for that one
   * turn (an honest, logged trade-off, not a crash).
   */
  getRuntimeDispatchAccountRefHash(threadId: string): string | null {
    return this.getMeta(this.runtimeDispatchAccountRefHashKey(threadId))
  }

  setRuntimeDispatchAccountRefHash(threadId: string, accountRefHash: string): void {
    this.setMeta(this.runtimeDispatchAccountRefHashKey(threadId), accountRefHash)
  }

  private runtimeDispatchAccountRefHashKey(threadId: string): string {
    return `runtime_dispatch_account_ref_hash.${threadId}`
  }

  getRuntimeIntentOutcome(intentId: string): RuntimeIntentOutcomeRecord | null {
    const row = this.db
      .query("SELECT * FROM pylon_orchestration_runtime_intent_outcomes WHERE intent_id = $intentId")
      .get({ $intentId: intentId }) as RuntimeIntentOutcomeRow | null
    return row === null ? null : runtimeIntentOutcomeFromRow(row)
  }

  listRuntimeIntentOutcomes(input: { threadId?: string } = {}): RuntimeIntentOutcomeRecord[] {
    const rows = input.threadId === undefined
      ? this.db
        .query("SELECT * FROM pylon_orchestration_runtime_intent_outcomes ORDER BY recorded_at ASC")
        .all()
      : this.db
        .query(`
          SELECT * FROM pylon_orchestration_runtime_intent_outcomes
           WHERE thread_id = $threadId
           ORDER BY recorded_at ASC
        `)
        .all({ $threadId: input.threadId })
    return (rows as RuntimeIntentOutcomeRow[]).map(runtimeIntentOutcomeFromRow)
  }

  /**
   * Record one runtime control-intent dispatch outcome exactly once. A
   * second record for the same `intentId` (route redelivery, restart
   * replay) is refused and the FIRST recorded outcome is returned with
   * `recorded: false` — callers must treat that as "already dispatched, do
   * not re-dispatch" (mirrors `recordFleetIntentOutcome`).
   */
  recordRuntimeIntentOutcome(
    input: RecordRuntimeIntentOutcomeInput,
  ): { recorded: boolean; outcome: RuntimeIntentOutcomeRecord } {
    const existing = this.getRuntimeIntentOutcome(input.intentId)
    if (existing !== null) return { outcome: existing, recorded: false }
    this.db
      .query(`
        INSERT OR IGNORE INTO pylon_orchestration_runtime_intent_outcomes
          (intent_id, thread_id, turn_id, kind, outcome, detail, recorded_at)
        VALUES
          ($intentId, $threadId, $turnId, $kind, $outcome, $detail, $recordedAt)
      `)
      .run({
        $intentId: input.intentId,
        $threadId: input.threadId,
        $turnId: input.turnId ?? null,
        $kind: input.kind,
        $outcome: input.outcome,
        $detail: input.detail ?? null,
        $recordedAt: iso(input.now),
      })
    const stored = this.getRuntimeIntentOutcome(input.intentId)
    if (stored === null) throw new Error(`failed to record runtime intent outcome ${input.intentId}`)
    return { outcome: stored, recorded: true }
  }

  tryClaimWorkUnit(input: CreateWorkClaimInput): WorkClaim | null {
    const claim = buildWorkClaim(input)
    const at = iso(input.now)
    this.db.run("BEGIN IMMEDIATE")
    try {
      this.expireWorkClaims(input.now)
      this.db
        .query(`
          INSERT INTO pylon_orchestration_work_claims
            (claim_ref, work_unit_ref, run_ref, assignment_ref, worker_account_ref,
             marginal_cost_class, state,
             ttl_ms, claimed_at, expires_at, updated_at)
          VALUES
            ($claimRef, $workUnitRef, $runRef, $assignmentRef, $workerAccountRef,
             $marginalCostClass, $state,
             $ttl, $claimedAt, $expiresAt, $updatedAt)
        `)
        .run({
          $claimRef: claim.claimRef,
          $workUnitRef: claim.workUnitRef,
          $runRef: claim.runRef,
          $assignmentRef: claim.assignmentRef,
          $workerAccountRef: claim.workerAccountRef,
          $marginalCostClass: claim.marginalCostClass ?? "not_measured",
          $state: claim.state,
          $ttl: claim.ttl,
          $claimedAt: claim.claimedAt,
          $expiresAt: claim.expiresAt,
          $updatedAt: at,
        })
      this.db.run("COMMIT")
    } catch (error) {
      this.db.run("ROLLBACK")
      if (isLiveClaimUniqueConstraintError(error)) return null
      throw error
    }
    return this.getWorkClaim(claim.claimRef)
  }

  getWorkClaim(claimRef: string): WorkClaim | null {
    const row = this.db
      .query("SELECT * FROM pylon_orchestration_work_claims WHERE claim_ref = $claimRef")
      .get({ $claimRef: claimRef }) as WorkClaimRow | null
    return row === null ? null : workClaimFromRow(row)
  }

  getLiveWorkClaim(workUnitRef: string, now: Date = new Date()): WorkClaim | null {
    this.expireWorkClaims(now)
    const row = this.db
      .query(`
        SELECT * FROM pylon_orchestration_work_claims
         WHERE work_unit_ref = $workUnitRef
           AND state IN ('claimed', 'in_progress', 'closeout')
         ORDER BY claimed_at DESC
         LIMIT 1
      `)
      .get({ $workUnitRef: workUnitRef }) as WorkClaimRow | null
    return row === null ? null : workClaimFromRow(row)
  }

  listWorkClaims(input: { state?: WorkClaimState; runRef?: string } = {}): WorkClaim[] {
    const clauses: string[] = []
    const params: Record<string, string> = {}
    if (input.state !== undefined) {
      clauses.push("state = $state")
      params.$state = input.state
    }
    if (input.runRef !== undefined) {
      clauses.push("run_ref = $runRef")
      params.$runRef = input.runRef
    }
    const where = clauses.length === 0 ? "" : ` WHERE ${clauses.join(" AND ")}`
    const rows = this.db
      .query(`SELECT * FROM pylon_orchestration_work_claims${where} ORDER BY claimed_at ASC, claim_ref ASC`)
      .all(params) as WorkClaimRow[]
    return rows.map(workClaimFromRow)
  }

  listLiveWorkClaims(now: Date = new Date()): WorkClaim[] {
    this.expireWorkClaims(now)
    const rows = this.db
      .query(`
        SELECT * FROM pylon_orchestration_work_claims
         WHERE state IN ('claimed', 'in_progress', 'closeout')
         ORDER BY claimed_at ASC, claim_ref ASC
      `)
      .all() as WorkClaimRow[]
    return rows.map(workClaimFromRow)
  }

  updateWorkClaimState(claimRef: string, state: WorkClaimState, now: Date = new Date()): WorkClaim {
    const current = this.getWorkClaim(claimRef)
    if (current === null) throw new Error(`unknown work claim: ${claimRef}`)
    if (!isLiveWorkClaimState(current.state) && isLiveWorkClaimState(state)) {
      throw new Error(`cannot revive terminal work claim: ${claimRef}`)
    }
    this.db
      .query(`
        UPDATE pylon_orchestration_work_claims
           SET state = $state,
               updated_at = $updatedAt
         WHERE claim_ref = $claimRef
      `)
      .run({ $claimRef: claimRef, $state: state, $updatedAt: iso(now) })
    const updated = this.getWorkClaim(claimRef)
    if (updated === null) throw new Error(`failed to update work claim: ${claimRef}`)
    return updated
  }

  releaseWorkClaim(claimRef: string, now: Date = new Date()): WorkClaim {
    return this.updateWorkClaimState(claimRef, "released", now)
  }

  updateWorkClaimAssignmentRef(claimRef: string, assignmentRef: string | null, now: Date = new Date()): WorkClaim {
    const current = this.getWorkClaim(claimRef)
    if (current === null) throw new Error(`unknown work claim: ${claimRef}`)
    if (current.assignmentRef !== null && assignmentRef !== current.assignmentRef) {
      throw new Error(`cannot rebind work claim assignment: ${claimRef}`)
    }
    this.db
      .query(`
        UPDATE pylon_orchestration_work_claims
           SET assignment_ref = $assignmentRef,
               updated_at = $updatedAt
         WHERE claim_ref = $claimRef
      `)
      .run({ $claimRef: claimRef, $assignmentRef: assignmentRef, $updatedAt: iso(now) })
    const updated = this.getWorkClaim(claimRef)
    if (updated === null) throw new Error(`failed to update work claim assignment: ${claimRef}`)
    return updated
  }

  refreshLiveWorkClaim(workUnitRef: string, now: Date = new Date()): WorkClaim | null {
    const current = this.getLiveWorkClaim(workUnitRef, now)
    if (current === null) return null
    this.db
      .query(`
        UPDATE pylon_orchestration_work_claims
           SET expires_at = $expiresAt,
               updated_at = $updatedAt
         WHERE claim_ref = $claimRef
           AND state IN ('claimed', 'in_progress', 'closeout')
      `)
      .run({
        $claimRef: current.claimRef,
        $expiresAt: iso(new Date(now.getTime() + current.ttl)),
        $updatedAt: iso(now),
      })
    return this.getWorkClaim(current.claimRef)
  }

  releaseLiveWorkClaim(workUnitRef: string, now: Date = new Date()): WorkClaim | null {
    const current = this.getLiveWorkClaim(workUnitRef, now)
    if (current === null) return null
    return this.releaseWorkClaim(current.claimRef, now)
  }

  expireWorkClaims(now: Date = new Date()): WorkClaim[] {
    const at = iso(now)
    const rows = this.db
      .query(`
        SELECT * FROM pylon_orchestration_work_claims
         WHERE state IN ('claimed', 'in_progress', 'closeout')
           AND expires_at <= $now
         ORDER BY expires_at ASC, claim_ref ASC
      `)
      .all({ $now: at }) as WorkClaimRow[]
    if (rows.length === 0) return []
    this.db
      .query(`
        UPDATE pylon_orchestration_work_claims
           SET state = 'expired',
               updated_at = $now
         WHERE state IN ('claimed', 'in_progress', 'closeout')
           AND expires_at <= $now
      `)
      .run({ $now: at })
    return rows.map((row) => workClaimFromRow({ ...row, state: "expired", updated_at: at }))
  }

  reconcileWorkClaims(input: ReconcileWorkClaimsInput = {}): { expired: WorkClaim[]; released: WorkClaim[] } {
    const now = input.now ?? new Date()
    const expired = this.expireWorkClaims(now)
    const heartbeatTtlMs = input.workerHeartbeatTtlMs ?? 5 * 60 * 1000
    const freshAfter = now.getTime() - heartbeatTtlMs
    const released: WorkClaim[] = []

    this.db.run("BEGIN IMMEDIATE")
    try {
      for (const claim of this.listLiveWorkClaims(now)) {
        const row = this.db
          .query(`
            SELECT * FROM pylon_orchestration_dispatch_contexts
             WHERE assignee_handle = $workerAccountRef
             ORDER BY updated_at DESC
             LIMIT 1
          `)
          .get({ $workerAccountRef: claim.workerAccountRef }) as DispatchContextRow | null
        const context = row === null ? null : contextFromRow(row)
        const heartbeatAt = context?.lastHeartbeatAt === null || context?.lastHeartbeatAt === undefined
          ? null
          : Date.parse(context.lastHeartbeatAt)
        const workerDead =
          context === null ||
          context.status === "circuit_broken" ||
          heartbeatAt === null ||
          Number.isNaN(heartbeatAt) ||
          heartbeatAt <= freshAfter
        if (!workerDead) continue
        this.db
          .query(`
            UPDATE pylon_orchestration_work_claims
               SET state = 'released',
                   updated_at = $now
             WHERE claim_ref = $claimRef
               AND state IN ('claimed', 'in_progress', 'closeout')
          `)
          .run({ $claimRef: claim.claimRef, $now: iso(now) })
        released.push({ ...claim, state: "released", updatedAt: iso(now) })
      }
      this.db.run("COMMIT")
    } catch (error) {
      this.db.run("ROLLBACK")
      throw error
    }

    return { expired, released }
  }

  createTask(input: CreateTaskInput): OrchestrationTask {
    const now = iso(input.now)
    const deps = [...(input.deps ?? [])]
    const status = input.status ?? (deps.length === 0 ? "ready" : "pending")
    const threadId = input.threadId ?? input.id
    this.db
      .query(`
        INSERT INTO pylon_orchestration_tasks
          (id, parent_id, thread_id, spec_json, status, deps_json, result_json, created_at, updated_at)
        VALUES
          ($id, $parentId, $threadId, $spec, $status, $deps, NULL, $createdAt, $updatedAt)
      `)
      .run({
        $id: input.id,
        $parentId: input.parentId ?? null,
        $threadId: threadId,
        $spec: JSON.stringify(normalizeTaskSpec(input.spec)),
        $status: status,
        $deps: JSON.stringify(deps),
        $createdAt: now,
        $updatedAt: now,
      })
    const task = this.getTask(input.id)
    if (task === null) throw new Error(`failed to create orchestration task ${input.id}`)
    return task
  }

  getTask(id: string): OrchestrationTask | null {
    const row = this.db.query("SELECT * FROM pylon_orchestration_tasks WHERE id = $id").get({ $id: id }) as TaskRow | null
    return row === null ? null : taskFromRow(row)
  }

  listTasks(status?: OrchestrationTaskStatus): OrchestrationTask[] {
    const rows = status === undefined
      ? this.db.query("SELECT * FROM pylon_orchestration_tasks ORDER BY created_at ASC").all()
      : this.db
        .query("SELECT * FROM pylon_orchestration_tasks WHERE status = $status ORDER BY created_at ASC")
        .all({ $status: status })
    return (rows as TaskRow[]).map(taskFromRow)
  }

  updateTaskSpec(id: string, spec: OrchestrationTaskSpec, now: Date = new Date()): OrchestrationTask {
    this.db
      .query("UPDATE pylon_orchestration_tasks SET spec_json = $spec, updated_at = $now WHERE id = $id")
      .run({ $id: id, $spec: JSON.stringify(normalizeTaskSpec(spec)), $now: iso(now) })
    const task = this.getTask(id)
    if (task === null) throw new Error(`unknown orchestration task: ${id}`)
    return task
  }

  completeTask(id: string, result: string | null = null, now: Date = new Date()): void {
    this.db
      .query("UPDATE pylon_orchestration_tasks SET status = 'completed', result_json = $result, updated_at = $now WHERE id = $id")
      .run({ $id: id, $result: result, $now: iso(now) })
    this.releaseVirtualHeadTask(id, now)
    this.promoteReadyTasks(now)
  }

  promoteReadyTasks(now: Date = new Date()): string[] {
    const completed = new Set(this.listTasks("completed").map((task) => task.id))
    const promoted: string[] = []
    for (const task of this.listTasks("pending")) {
      if (!task.deps.every((dep) => completed.has(dep))) continue
      this.db
        .query("UPDATE pylon_orchestration_tasks SET status = 'ready', updated_at = $now WHERE id = $id")
        .run({ $id: task.id, $now: iso(now) })
      promoted.push(task.id)
    }
    return promoted
  }

  createDispatchContext(input: CreateDispatchContextInput): DispatchContext {
    const now = iso(input.now)
    const runnerKind = normalizeOrchestrationRunnerKind(input.runnerKind ?? "generic")
    const lane = input.lane ?? normalizePylonDispatchFailureLane(runnerKind)
    this.db
      .query(`
        INSERT INTO pylon_orchestration_dispatch_contexts
          (id, assignee_handle, runner_kind, lane, account_ref_hash, worktree_id, worktree_path, status, current_task_id,
           failure_count, last_heartbeat_at, base_behind_by, max_concurrent_slots, created_at, updated_at)
        VALUES
          ($id, $assigneeHandle, $runnerKind, $lane, $accountRefHash, $worktreeId, $worktreePath, 'idle', NULL,
           0, $lastHeartbeatAt, $baseBehindBy, $maxConcurrentSlots, $createdAt, $updatedAt)
      `)
      .run({
        $id: input.id,
        $assigneeHandle: input.assigneeHandle,
        $runnerKind: runnerKind,
        $lane: lane,
        $accountRefHash: input.accountRefHash ?? null,
        $worktreeId: input.worktreeId ?? null,
        $worktreePath: input.worktreePath ?? null,
        $lastHeartbeatAt: input.lastHeartbeatAt === undefined ? null : input.lastHeartbeatAt === null ? null : iso(input.lastHeartbeatAt),
        $baseBehindBy: input.baseBehindBy ?? 0,
        $maxConcurrentSlots: input.maxConcurrentSlots ?? 1,
        $createdAt: now,
        $updatedAt: now,
      })
    const context = this.getDispatchContext(input.id)
    if (context === null) throw new Error(`failed to create dispatch context ${input.id}`)
    return context
  }

  getDispatchContext(id: string): DispatchContext | null {
    const row = this.db
      .query("SELECT * FROM pylon_orchestration_dispatch_contexts WHERE id = $id")
      .get({ $id: id }) as DispatchContextRow | null
    return row === null ? null : contextFromRow(row)
  }

  listDispatchContexts(status?: DispatchContextStatus): DispatchContext[] {
    const rows = status === undefined
      ? this.db.query("SELECT * FROM pylon_orchestration_dispatch_contexts ORDER BY created_at ASC").all()
      : this.db
        .query("SELECT * FROM pylon_orchestration_dispatch_contexts WHERE status = $status ORDER BY created_at ASC")
        .all({ $status: status })
    return (rows as DispatchContextRow[]).map(contextFromRow)
  }

  getDispatchBreaker(scopeKey: string): PylonDispatchBreakerSnapshot | null {
    const row = this.db
      .query("SELECT * FROM pylon_orchestration_dispatch_breakers WHERE scope_key = $scopeKey")
      .get({ $scopeKey: scopeKey }) as DispatchBreakerRow | null
    return row === null ? null : dispatchBreakerFromRow(row)
  }

  listDispatchBreakers(): PylonDispatchBreakerSnapshot[] {
    const rows = this.db
      .query("SELECT * FROM pylon_orchestration_dispatch_breakers ORDER BY last_observed_at DESC, scope_key ASC")
      .all() as DispatchBreakerRow[]
    return rows.map(dispatchBreakerFromRow)
  }

  listActiveDispatchBreakers(now: Date = new Date()): PylonDispatchBreakerSnapshot[] {
    return this.listDispatchBreakers().filter(breaker => pylonDispatchBreakerIsActive(breaker, now))
  }

  getActiveDispatchBreakerForContext(
    context: DispatchContext,
    now: Date = new Date(),
  ): PylonDispatchBreakerSnapshot | null {
    const breaker = this.getDispatchBreaker(pylonDispatchBreakerScopeKey({
      accountRefHash: context.accountRefHash,
      contextId: context.id,
      lane: context.lane,
    }))
    return breaker !== null && pylonDispatchBreakerIsActive(breaker, now) ? breaker : null
  }

  recordDispatchBreakerFailure(input: {
    accountRefHash?: string | null
    classification: PylonDispatchFailureClassification
    contextId?: string | null
    lane: PylonDispatchFailureLane
    now?: Date
  }): PylonDispatchBreakerSnapshot {
    const now = input.now ?? new Date()
    const at = iso(now)
    const scopeKey = pylonDispatchBreakerScopeKey({
      accountRefHash: input.accountRefHash ?? null,
      contextId: input.contextId ?? null,
      lane: input.lane,
    })
    const current = this.getDispatchBreaker(scopeKey)
    const failureCount = (current?.failureCount ?? 0) + 1
    const cooldownUntil =
      input.classification.failureKind === "transient" && input.classification.cooldownMs !== null
        ? iso(new Date(now.getTime() + input.classification.cooldownMs * Math.min(failureCount, 6)))
        : null
    const blockerRefs = [
      input.classification.blockerRef,
      ...(input.classification.failureKind === "permanent"
        ? ["blocker.pylon.dispatch.permanent_breaker"]
        : ["blocker.pylon.dispatch.cooldown_active"]),
    ]
    this.db
      .query(`
        INSERT INTO pylon_orchestration_dispatch_breakers
          (scope_key, lane, account_ref_hash, context_id, failure_kind, reason,
           blocker_refs_json, failure_count, first_observed_at, last_observed_at,
           cooldown_until, source_digest_ref)
        VALUES
          ($scopeKey, $lane, $accountRefHash, $contextId, $failureKind, $reason,
           $blockerRefs, $failureCount, $firstObservedAt, $lastObservedAt,
           $cooldownUntil, $sourceDigestRef)
        ON CONFLICT(scope_key) DO UPDATE SET
          failure_kind = excluded.failure_kind,
          reason = excluded.reason,
          blocker_refs_json = excluded.blocker_refs_json,
          failure_count = excluded.failure_count,
          last_observed_at = excluded.last_observed_at,
          cooldown_until = excluded.cooldown_until,
          source_digest_ref = excluded.source_digest_ref
      `)
      .run({
        $scopeKey: scopeKey,
        $lane: input.lane,
        $accountRefHash: input.accountRefHash ?? null,
        $contextId: input.contextId ?? null,
        $failureKind: input.classification.failureKind,
        $reason: input.classification.reason,
        $blockerRefs: JSON.stringify(blockerRefs),
        $failureCount: failureCount,
        $firstObservedAt: current?.firstObservedAt ?? at,
        $lastObservedAt: at,
        $cooldownUntil: cooldownUntil,
        $sourceDigestRef: input.classification.sourceDigestRef,
      })
    const stored = this.getDispatchBreaker(scopeKey)
    if (stored === null) throw new Error(`failed to record dispatch breaker ${scopeKey}`)
    return stored
  }

  getAgentRunnerStatusEvent(eventRef: string): OrchestrationAgentRunnerStatusEntry | null {
    const row = this.db
      .query("SELECT * FROM pylon_orchestration_runner_statuses WHERE event_ref = $eventRef")
      .get({ $eventRef: eventRef }) as AgentRunnerStatusRow | null
    return row === null ? null : statusEntryFromRow(row)
  }

  getLiveAgentRunnerStatusByRunnerRef(runnerRef: string): OrchestrationAgentRunnerStatusEntry | null {
    const publicRunnerRef = stablePublicRef("runner.public.pylon", runnerRef)
    const row = this.db
      .query(`
        SELECT * FROM pylon_orchestration_runner_statuses
         WHERE runner_ref = $runnerRef AND retention_state = 'live'
         ORDER BY updated_at DESC, event_ref DESC
         LIMIT 1
      `)
      .get({ $runnerRef: publicRunnerRef }) as AgentRunnerStatusRow | null
    return row === null ? null : statusEntryFromRow(row)
  }

  listAgentRunnerStatusEvents(input: {
    retentionState?: AgentRunnerStatusRetentionState
    runnerRef?: string
  } = {}): OrchestrationAgentRunnerStatusEntry[] {
    const runnerRef = input.runnerRef === undefined ? undefined : stablePublicRef("runner.public.pylon", input.runnerRef)
    if (input.retentionState === undefined && runnerRef === undefined) {
      return (this.db
        .query("SELECT * FROM pylon_orchestration_runner_statuses ORDER BY updated_at ASC, event_ref ASC")
        .all() as AgentRunnerStatusRow[]).map(statusEntryFromRow)
    }
    if (input.retentionState !== undefined && runnerRef === undefined) {
      return (this.db
        .query(`
          SELECT * FROM pylon_orchestration_runner_statuses
           WHERE retention_state = $retentionState
           ORDER BY updated_at ASC, event_ref ASC
        `)
        .all({ $retentionState: input.retentionState }) as AgentRunnerStatusRow[]).map(statusEntryFromRow)
    }
    if (input.retentionState === undefined && runnerRef !== undefined) {
      return (this.db
        .query(`
          SELECT * FROM pylon_orchestration_runner_statuses
           WHERE runner_ref = $runnerRef
           ORDER BY updated_at ASC, event_ref ASC
        `)
        .all({ $runnerRef: runnerRef }) as AgentRunnerStatusRow[]).map(statusEntryFromRow)
    }
    if (input.retentionState === undefined || runnerRef === undefined) return []
    const rows = this.db
      .query(`
        SELECT * FROM pylon_orchestration_runner_statuses
         WHERE runner_ref = $runnerRef AND retention_state = $retentionState
         ORDER BY updated_at ASC, event_ref ASC
      `)
      .all({ $runnerRef: runnerRef, $retentionState: input.retentionState })
    return (rows as AgentRunnerStatusRow[]).map(statusEntryFromRow)
  }

  ingestAgentRunnerStatusEvent(input: IngestAgentRunnerStatusEventInput): OrchestrationAgentRunnerStatusEntry {
    const now = input.now ?? new Date(input.event.updatedAt)
    const at = iso(now)
    const historyLimit = input.historyLimit ?? 20
    const existing = this.getAgentRunnerStatusEvent(stablePublicRef("event.public.pylon.runner_status", input.event.eventRef))
    if (existing !== null) return existing
    const previous = this.getLiveAgentRunnerStatusByRunnerRef(input.event.runnerRef)
    const event = publicStatusEventFrom(input.event, previous, historyLimit)

    this.db.run("BEGIN IMMEDIATE")
    try {
      this.db
        .query(`
          UPDATE pylon_orchestration_runner_statuses
             SET retention_state = 'retained',
                 retained_at = $retainedAt
           WHERE runner_ref = $runnerRef AND retention_state = 'live'
        `)
        .run({ $runnerRef: event.runnerRef, $retainedAt: at })
      this.db
        .query(`
          INSERT INTO pylon_orchestration_runner_statuses
            (event_ref, runner_ref, runner_kind, state, state_started_at, updated_at,
             assignment_ref, task_id, dispatch_context_id, pylon_ref, worktree_ref,
             capability_refs_json, supported_control_verbs_json, refs_json,
             blocker_refs_json, state_history_json, retention_state, retained_at)
          VALUES
            ($eventRef, $runnerRef, $runnerKind, $state, $stateStartedAt, $updatedAt,
             $assignmentRef, $taskId, $dispatchContextId, $pylonRef, $worktreeRef,
             $capabilityRefs, $supportedControlVerbs, $refs, $blockerRefs,
             $stateHistory, 'live', NULL)
        `)
        .run({
          $eventRef: event.eventRef,
          $runnerRef: event.runnerRef,
          $runnerKind: event.runnerKind,
          $state: event.state,
          $stateStartedAt: event.stateStartedAt,
          $updatedAt: event.updatedAt,
          $assignmentRef: event.assignmentRef ?? null,
          $taskId: event.taskId ?? null,
          $dispatchContextId: event.dispatchContextId ?? null,
          $pylonRef: event.pylonRef ?? null,
          $worktreeRef: event.worktreeRef ?? null,
          $capabilityRefs: JSON.stringify(event.capabilityRefs ?? []),
          $supportedControlVerbs: JSON.stringify(event.supportedControlVerbs ?? []),
          $refs: JSON.stringify(event.refs ?? []),
          $blockerRefs: JSON.stringify(event.blockerRefs ?? []),
          $stateHistory: JSON.stringify(event.stateHistory ?? []),
        })
      if (input.event.dispatchContextId !== undefined && this.getDispatchContext(input.event.dispatchContextId) !== null) {
        const contextStatus = dispatchStatusForNeutralState(input.event.state)
        const taskId = contextStatus === "dispatched" ? input.event.taskId ?? null : null
        this.db
          .query(`
            UPDATE pylon_orchestration_dispatch_contexts
               SET status = $status,
                   current_task_id = $taskId,
                   last_heartbeat_at = $updatedAt,
                   updated_at = $updatedAt
             WHERE id = $contextId
          `)
          .run({
            $contextId: input.event.dispatchContextId,
            $status: contextStatus,
            $taskId: taskId,
            $updatedAt: input.event.updatedAt,
          })
      }
      this.db.run("COMMIT")
    } catch (error) {
      this.db.run("ROLLBACK")
      throw error
    }

    const stored = this.getAgentRunnerStatusEvent(event.eventRef)
    if (stored === null) throw new Error(`failed to ingest agent runner status event ${event.eventRef}`)
    return stored
  }

  decayAgentRunnerStatuses(input: DecayAgentRunnerStatusesInput = {}): OrchestrationAgentRunnerStatusEntry[] {
    const now = input.now ?? new Date()
    const staleAfterMs = input.staleAfterMs ?? 5 * 60 * 1000
    const decayed: OrchestrationAgentRunnerStatusEntry[] = []
    for (const live of this.listAgentRunnerStatusEvents({ retentionState: "live" })) {
      if (!shouldDecayRunnerState(live.state)) continue
      if (now.getTime() - Date.parse(live.updatedAt) < staleAfterMs) continue
      decayed.push(this.ingestAgentRunnerStatusEvent({
        event: {
          eventRef: `${live.eventRef}.decay.${now.getTime()}`,
          runnerRef: live.runnerRef,
          runnerKind: live.runnerKind,
          state: "idle",
          stateStartedAt: iso(now),
          updatedAt: iso(now),
          ...(live.supportedControlVerbs === undefined ? {} : { supportedControlVerbs: live.supportedControlVerbs }),
          ...(live.refs === undefined ? {} : { refs: live.refs }),
          ...(live.stateHistory === undefined ? {} : { stateHistory: live.stateHistory }),
        },
        now,
      }))
    }
    return decayed
  }

  recordHeartbeat(id: string, input: { at?: Date; baseBehindBy?: number; status?: DispatchContextStatus } = {}): DispatchContext {
    const current = this.getDispatchContext(id)
    if (current === null) throw new Error(`unknown dispatch context: ${id}`)
    const status = current.status === "circuit_broken" ? "circuit_broken" : input.status ?? null
    this.db
      .query(`
        UPDATE pylon_orchestration_dispatch_contexts
           SET last_heartbeat_at = $at,
               base_behind_by = COALESCE($baseBehindBy, base_behind_by),
               status = COALESCE($status, status),
               updated_at = $at
         WHERE id = $id
      `)
      .run({ $id: id, $at: iso(input.at), $baseBehindBy: input.baseBehindBy ?? null, $status: status })
    return this.getDispatchContext(id) ?? current
  }

  recordWorkerHeartbeat(input: RecordWorkerHeartbeatInput): OrchestrationMessage {
    const at = input.at ?? new Date()
    const context = this.getDispatchContext(input.contextId)
    if (context === null) throw new Error(`unknown dispatch context: ${input.contextId}`)
    const taskId = input.taskId ?? context.currentTaskId
    const task = taskId === null ? null : this.getTask(taskId)
    const threadId = task?.threadId ?? taskId ?? context.id
    const body = input.body ?? `heartbeat ${context.assigneeHandle}${taskId === null ? "" : ` on ${taskId}`}`
    const id = `message.${threadId}.${context.id}.heartbeat.${at.getTime()}`

    this.db.run("BEGIN IMMEDIATE")
    try {
      this.recordHeartbeat(input.contextId, {
        at,
        ...(input.baseBehindBy === undefined ? {} : { baseBehindBy: input.baseBehindBy }),
        ...(input.status === undefined ? {} : { status: input.status }),
      })
      this.appendMessage({
        id,
        threadId,
        taskId,
        dispatchContextId: context.id,
        kind: "heartbeat",
        body,
        now: at,
      })
      this.db.run("COMMIT")
    } catch (error) {
      this.db.run("ROLLBACK")
      throw error
    }

    const message = this.getMessage(id)
    if (message === null) throw new Error(`failed to record orchestration heartbeat message ${id}`)
    return message
  }

  markDispatched(taskId: string, contextId: string, now: Date = new Date()): void {
    const at = iso(now)
    this.db.run("BEGIN IMMEDIATE")
    try {
      this.db
        .query("UPDATE pylon_orchestration_tasks SET status = 'dispatched', updated_at = $at WHERE id = $taskId")
        .run({ $taskId: taskId, $at: at })
      this.db
        .query(`
          UPDATE pylon_orchestration_dispatch_contexts
             SET status = 'dispatched',
                 current_task_id = $taskId,
                 last_heartbeat_at = $at,
                 updated_at = $at
           WHERE id = $contextId
        `)
        .run({ $contextId: contextId, $taskId: taskId, $at: at })
      this.db.run("COMMIT")
    } catch (error) {
      this.db.run("ROLLBACK")
      throw error
    }
  }

  getVirtualHead(repo: string, branch: string): VirtualHead | null {
    const row = this.db
      .query("SELECT * FROM pylon_orchestration_virtual_heads WHERE repo = $repo AND branch = $branch")
      .get({ $repo: repo, $branch: branch }) as VirtualHeadRow | null
    return row === null ? null : virtualHeadFromRow(row)
  }

  listVirtualHeads(): VirtualHead[] {
    const rows = this.db
      .query("SELECT * FROM pylon_orchestration_virtual_heads ORDER BY repo ASC, branch ASC")
      .all() as VirtualHeadRow[]
    return rows.map(virtualHeadFromRow)
  }

  seedVirtualHead(input: { repo: string; branch: string; baseCommit: string; projectedHead?: string; now?: Date }): VirtualHead {
    const now = iso(input.now)
    const projectedHead = input.projectedHead ?? input.baseCommit
    this.db
      .query(`
        INSERT INTO pylon_orchestration_virtual_heads
          (repo, branch, base_commit, projected_head, pending_task_ids_json, created_at, updated_at)
        VALUES
          ($repo, $branch, $baseCommit, $projectedHead, '[]', $createdAt, $updatedAt)
        ON CONFLICT(repo, branch) DO UPDATE SET
          projected_head = excluded.projected_head,
          updated_at = excluded.updated_at
      `)
      .run({
        $repo: input.repo,
        $branch: input.branch,
        $baseCommit: input.baseCommit,
        $projectedHead: projectedHead,
        $createdAt: now,
        $updatedAt: now,
      })
    const virtualHead = this.getVirtualHead(input.repo, input.branch)
    if (virtualHead === null) throw new Error(`failed to seed virtual head for ${input.repo}#${input.branch}`)
    return virtualHead
  }

  reserveVirtualHeadForTask(taskId: string, now: Date = new Date()): VirtualHeadReservation | null {
    const task = this.getTask(taskId)
    if (task === null) throw new Error(`unknown orchestration task: ${taskId}`)
    const { repo, branch, baseCommit } = task.spec
    if (repo === undefined || branch === undefined || baseCommit === undefined) return null

    const at = iso(now)
    this.db.run("BEGIN IMMEDIATE")
    try {
      const current =
        this.getVirtualHead(repo, branch) ??
        this.seedVirtualHead({ repo, branch, baseCommit, now })
      const branchFrom = current.projectedHead
      const projectedHead = virtualHeadProjectionRef({ repo, branch, taskId, branchFrom })
      const pendingTaskIds = [...new Set([...current.pendingTaskIds, taskId])]
      this.db
        .query(`
          UPDATE pylon_orchestration_virtual_heads
             SET projected_head = $projectedHead,
                 pending_task_ids_json = $pendingTaskIds,
                 updated_at = $updatedAt
           WHERE repo = $repo AND branch = $branch
        `)
        .run({
          $repo: repo,
          $branch: branch,
          $projectedHead: projectedHead,
          $pendingTaskIds: JSON.stringify(pendingTaskIds),
          $updatedAt: at,
        })
      this.updateTaskSpec(taskId, { ...task.spec, baseCommit: branchFrom }, now)
      this.db.run("COMMIT")
      return { repo, branch, taskId, branchFrom, projectedHead, pendingTaskIds }
    } catch (error) {
      this.db.run("ROLLBACK")
      throw error
    }
  }

  releaseVirtualHeadTask(taskId: string, now: Date = new Date()): void {
    const at = iso(now)
    for (const virtualHead of this.listVirtualHeads()) {
      if (!virtualHead.pendingTaskIds.includes(taskId)) continue
      const pendingTaskIds = virtualHead.pendingTaskIds.filter((id) => id !== taskId)
      this.db
        .query(`
          UPDATE pylon_orchestration_virtual_heads
             SET pending_task_ids_json = $pendingTaskIds,
                 updated_at = $updatedAt
           WHERE repo = $repo AND branch = $branch
        `)
        .run({
          $repo: virtualHead.repo,
          $branch: virtualHead.branch,
          $pendingTaskIds: JSON.stringify(pendingTaskIds),
          $updatedAt: at,
        })
    }
  }

  releaseDispatchContext(id: string, status: DispatchContextStatus = "idle", now: Date = new Date()): void {
    this.db
      .query(`
        UPDATE pylon_orchestration_dispatch_contexts
           SET status = $status, current_task_id = NULL, updated_at = $now
         WHERE id = $id
      `)
      .run({ $id: id, $status: status, $now: iso(now) })
  }

  recordDispatchFailure(
    id: string,
    maxFailures = 3,
    now: Date = new Date(),
    failure?: PylonDispatchFailureInput,
  ): DispatchContext {
    const current = this.getDispatchContext(id)
    if (current === null) throw new Error(`unknown dispatch context: ${id}`)
    const failureCount = current.failureCount + 1
    const classification = failure === undefined ? null : classifyPylonDispatchFailure(failure)
    const status: DispatchContextStatus =
      classification?.failureKind === "permanent" || failureCount >= maxFailures
        ? "circuit_broken"
        : "idle"
    const at = iso(now)
    this.db.run("BEGIN IMMEDIATE")
    try {
      if (classification !== null) {
        this.recordDispatchBreakerFailure({
          accountRefHash: current.accountRefHash,
          classification,
          contextId: current.id,
          lane: current.lane,
          now,
        })
      }
      if (current.currentTaskId !== null) {
        this.db
          .query("UPDATE pylon_orchestration_tasks SET status = 'failed', updated_at = $now WHERE id = $taskId")
          .run({ $taskId: current.currentTaskId, $now: at })
      }
      this.db
        .query(`
          UPDATE pylon_orchestration_dispatch_contexts
             SET failure_count = $failureCount,
                 status = $status,
                 current_task_id = NULL,
                 updated_at = $now
           WHERE id = $id
        `)
        .run({ $id: id, $failureCount: failureCount, $status: status, $now: at })
      if (current.currentTaskId !== null) {
        this.releaseVirtualHeadTask(current.currentTaskId, now)
      }
      this.db.run("COMMIT")
    } catch (error) {
      this.db.run("ROLLBACK")
      throw error
    }
    return this.getDispatchContext(id) ?? current
  }

  recordWorkerDone(input: RecordWorkerDoneInput): DispatchContext {
    const now = input.now ?? new Date()
    const at = iso(now)
    const task = this.getTask(input.taskId)
    if (task === null) throw new Error(`unknown orchestration task: ${input.taskId}`)
    const context = this.getDispatchContext(input.contextId)
    if (context === null) throw new Error(`unknown dispatch context: ${input.contextId}`)
    if (context.currentTaskId !== input.taskId) {
      throw new Error(`dispatch context ${input.contextId} is not assigned to task ${input.taskId}`)
    }

    const maxFailures = input.maxFailures ?? 3
    const failureCount = input.status === "failed" ? context.failureCount + 1 : 0
    const classification = input.status === "failed" && input.failure !== undefined
      ? classifyPylonDispatchFailure(input.failure)
      : null
    const contextStatus: DispatchContextStatus =
      input.status === "failed" &&
        (classification?.failureKind === "permanent" || failureCount >= maxFailures)
        ? "circuit_broken"
        : "idle"
    const body = input.body ?? `worker_done ${input.taskId} ${input.status}`

    this.db.run("BEGIN IMMEDIATE")
    try {
      if (classification !== null) {
        this.recordDispatchBreakerFailure({
          accountRefHash: context.accountRefHash,
          classification,
          contextId: context.id,
          lane: context.lane,
          now,
        })
      }
      this.db
        .query(`
          UPDATE pylon_orchestration_tasks
             SET status = $status,
                 result_json = $result,
                 updated_at = $now
           WHERE id = $taskId
        `)
        .run({
          $taskId: input.taskId,
          $status: input.status,
          $result: input.result ?? null,
          $now: at,
        })
      this.db
        .query(`
          UPDATE pylon_orchestration_dispatch_contexts
             SET status = $contextStatus,
                 current_task_id = NULL,
                 failure_count = $failureCount,
                 updated_at = $now
           WHERE id = $contextId
        `)
        .run({
          $contextId: input.contextId,
          $contextStatus: contextStatus,
          $failureCount: failureCount,
          $now: at,
        })
      this.releaseVirtualHeadTask(input.taskId, now)
      this.appendMessage({
        id: `message.${input.taskId}.${input.contextId}.worker_done.${now.getTime()}`,
        threadId: task.threadId,
        taskId: input.taskId,
        dispatchContextId: input.contextId,
        kind: "worker_done",
        body,
        now,
      })
      this.db.run("COMMIT")
    } catch (error) {
      this.db.run("ROLLBACK")
      throw error
    }

    if (input.status === "completed") {
      this.promoteReadyTasks(now)
    }
    return this.getDispatchContext(input.contextId) ?? context
  }

  appendMessage(input: {
    id: string
    threadId: string
    taskId?: string | null
    dispatchContextId?: string | null
    kind: OrchestrationMessageKind
    body: string
    now?: Date
  }): void {
    this.db
      .query(`
        INSERT INTO pylon_orchestration_messages
          (id, thread_id, task_id, dispatch_context_id, kind, body, created_at)
        VALUES
          ($id, $threadId, $taskId, $dispatchContextId, $kind, $body, $createdAt)
      `)
      .run({
        $id: input.id,
        $threadId: input.threadId,
        $taskId: input.taskId ?? null,
        $dispatchContextId: input.dispatchContextId ?? null,
        $kind: input.kind,
        $body: input.body,
        $createdAt: iso(input.now),
      })
  }

  getMessage(id: string): OrchestrationMessage | null {
    const row = this.db
      .query("SELECT * FROM pylon_orchestration_messages WHERE id = $id")
      .get({ $id: id }) as MessageRow | null
    return row === null ? null : messageFromRow(row)
  }

  listMessages(threadId?: string): OrchestrationMessage[] {
    const rows = threadId === undefined
      ? this.db.query("SELECT * FROM pylon_orchestration_messages ORDER BY created_at ASC, id ASC").all()
      : this.db
        .query("SELECT * FROM pylon_orchestration_messages WHERE thread_id = $threadId ORDER BY created_at ASC, id ASC")
        .all({ $threadId: threadId })
    return (rows as MessageRow[]).map(messageFromRow)
  }

  publicSnapshot(): { tasks: PublicOrchestrationTask[]; dispatchContexts: PublicDispatchContext[] } {
    return {
      tasks: this.listTasks().map(publicOrchestrationTaskFrom),
      dispatchContexts: this.listDispatchContexts().map(publicDispatchContextFrom),
    }
  }
}

export const fleetRunOwnerLocalStatePath = (paths: PylonHomePaths): string =>
  join(paths.home, "fleet-runs.json")

export async function loadFleetRunOwnerLocalState(
  paths: PylonHomePaths,
): Promise<FleetRunOwnerLocalState> {
  const path = fleetRunOwnerLocalStatePath(paths)
  if (!existsSync(path)) return { schema: FLEET_RUN_OWNER_LOCAL_STATE_SCHEMA, runs: [] }
  const parsed: unknown = JSON.parse(await readFile(path, "utf8"))
  const state = S.decodeUnknownSync(FleetRunOwnerLocalStateSchema)(parsed)
  return { ...state, runs: state.runs.map(decodeFleetRun) }
}

export async function saveFleetRunOwnerLocalState(
  paths: PylonHomePaths,
  state: FleetRunOwnerLocalState,
): Promise<void> {
  const decoded = S.decodeUnknownSync(FleetRunOwnerLocalStateSchema)(state)
  const validated = { ...decoded, runs: decoded.runs.map(decodeFleetRun) }
  await mkdir(paths.home, { recursive: true })
  await writeFile(fleetRunOwnerLocalStatePath(paths), `${JSON.stringify(validated, null, 2)}\n`, { mode: 0o600 })
}

export async function syncFleetRunsToOwnerLocalState(
  store: PylonOrchestrationStore,
  paths: PylonHomePaths,
): Promise<FleetRunOwnerLocalState> {
  const state: FleetRunOwnerLocalState = {
    schema: FLEET_RUN_OWNER_LOCAL_STATE_SCHEMA,
    runs: store.listFleetRuns(),
  }
  await saveFleetRunOwnerLocalState(paths, state)
  return state
}

export async function reconcileFleetRunsFromOwnerLocalState(
  store: PylonOrchestrationStore,
  paths: PylonHomePaths,
  input: { now?: Date } = {},
): Promise<FleetRun[]> {
  const localState = await loadFleetRunOwnerLocalState(paths)
  for (const run of localState.runs) {
    store.upsertFleetRun(run, input.now)
  }
  const reconciled = store.reconcileFleetRuns(input.now)
  await syncFleetRunsToOwnerLocalState(store, paths)
  return reconciled
}

export const createPylonOrchestrationStore = (db: SqliteDatabase): PylonOrchestrationStore => {
  const store = new PylonOrchestrationStore(db)
  store.migrate()
  return store
}
