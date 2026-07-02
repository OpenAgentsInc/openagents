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

export const ORCHESTRATION_SCHEMA_VERSION = 2

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
export const WORK_CLAIM_SCHEMA = "openagents.khala_code.work_claim.v1" as const

export const FleetRunWorkSourceSchema = S.Literals(["github_backlog", "issue_list", "fixture"])
export type FleetRunWorkSource = typeof FleetRunWorkSourceSchema.Type

export const FleetRunWorkerKindSchema = S.Literals(["codex", "claude", "auto"])
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

export const FleetRunSchema = S.Struct({
  schema: S.Literal(FLEET_RUN_SCHEMA),
  runRef: S.String,
  objective: S.String,
  workSource: FleetRunWorkSourceSchema,
  targetConcurrency: S.Number,
  workerKind: FleetRunWorkerKindSchema,
  refillPolicy: FleetRunRefillPolicySchema,
  state: FleetRunStateSchema,
  dispatchKind: FleetRunDispatchKindSchema,
  dagTracked: S.Boolean,
  startedAt: S.NullOr(S.String),
  counters: FleetRunCountersSchema,
  createdAt: S.String,
  updatedAt: S.String,
})
export type FleetRun = typeof FleetRunSchema.Type

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

export const WorkClaimSchema = S.Struct({
  schema: S.Literal(WORK_CLAIM_SCHEMA),
  claimRef: S.String,
  workUnitRef: S.String,
  runRef: S.String,
  assignmentRef: S.NullOr(S.String),
  workerAccountRef: S.String,
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
  ttl: number
  now?: Date
}

export type ReconcileWorkClaimsInput = {
  now?: Date
  workerHeartbeatTtlMs?: number
}

export type CreateFleetRunInput = {
  runRef: string
  objective: string
  workSource: FleetRunWorkSource
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

export type OrchestrationRunnerKind = AgentRunnerKind | "generic"
type StoredRunnerKind = OrchestrationRunnerKind | LegacyAgentRunnerKind

export type DispatchContext = {
  id: string
  assigneeHandle: string
  runnerKind: OrchestrationRunnerKind
  worktreeId: string | null
  worktreePath: string | null
  status: DispatchContextStatus
  currentTaskId: string | null
  failureCount: number
  lastHeartbeatAt: string | null
  baseBehindBy: number
  maxConcurrentSlots: number
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
  return run
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
  return kind === "generic" || kind === "claude" || kind === "claude_agent" || kind === "codex"
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
  worktree_id: string | null
  worktree_path: string | null
  status: DispatchContextStatus
  current_task_id: string | null
  failure_count: number
  last_heartbeat_at: string | null
  base_behind_by: number
  max_concurrent_slots: number
  created_at: string
  updated_at: string
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

type WorkClaimRow = {
  claim_ref: string
  work_unit_ref: string
  run_ref: string
  assignment_ref: string | null
  worker_account_ref: string
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
  worktreeId: row.worktree_id,
  worktreePath: row.worktree_path,
  status: row.status,
  currentTaskId: row.current_task_id,
  failureCount: row.failure_count,
  lastHeartbeatAt: row.last_heartbeat_at,
  baseBehindBy: row.base_behind_by,
  maxConcurrentSlots: row.max_concurrent_slots,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
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

const workClaimFromRow = (row: WorkClaimRow): WorkClaim => decodeWorkClaim({
  schema: WORK_CLAIM_SCHEMA,
  claimRef: row.claim_ref,
  workUnitRef: row.work_unit_ref,
  runRef: row.run_ref,
  assignmentRef: row.assignment_ref,
  workerAccountRef: row.worker_account_ref,
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
        runner_kind TEXT NOT NULL CHECK (runner_kind IN ('codex', 'claude_agent', 'claude', 'generic')),
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
        worker_kind TEXT NOT NULL CHECK (worker_kind IN ('codex', 'claude', 'auto')),
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        started_at TEXT
      );
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_fleet_runs_state
        ON pylon_orchestration_fleet_runs(state, updated_at);
      CREATE INDEX IF NOT EXISTS idx_pylon_orchestration_fleet_runs_dispatch
        ON pylon_orchestration_fleet_runs(dispatch_kind, updated_at);
      CREATE TABLE IF NOT EXISTS pylon_orchestration_work_claims (
        claim_ref TEXT PRIMARY KEY,
        work_unit_ref TEXT NOT NULL,
        run_ref TEXT NOT NULL,
        assignment_ref TEXT,
        worker_account_ref TEXT NOT NULL,
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
    `)
    this.db
      .query("INSERT OR REPLACE INTO pylon_orchestration_meta (key, value) VALUES ('schema_version', $version)")
      .run({ $version: String(ORCHESTRATION_SCHEMA_VERSION) })
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

  updateFleetRunState(runRef: string, state: FleetRunState, now: Date = new Date()): FleetRun {
    const current = this.getFleetRun(runRef)
    if (current === null) throw new Error(`unknown fleet run: ${runRef}`)
    return this.upsertFleetRun({
      ...current,
      state,
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
    return this.upsertFleetRun({ ...run, state, counters, updatedAt: iso(now) })
  }

  reconcileFleetRuns(now: Date = new Date()): FleetRun[] {
    return this.listFleetRuns().map((run) => this.reconcileFleetRun(run.runRef, now))
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
            (claim_ref, work_unit_ref, run_ref, assignment_ref, worker_account_ref, state,
             ttl_ms, claimed_at, expires_at, updated_at)
          VALUES
            ($claimRef, $workUnitRef, $runRef, $assignmentRef, $workerAccountRef, $state,
             $ttl, $claimedAt, $expiresAt, $updatedAt)
        `)
        .run({
          $claimRef: claim.claimRef,
          $workUnitRef: claim.workUnitRef,
          $runRef: claim.runRef,
          $assignmentRef: claim.assignmentRef,
          $workerAccountRef: claim.workerAccountRef,
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
    this.db
      .query(`
        INSERT INTO pylon_orchestration_dispatch_contexts
          (id, assignee_handle, runner_kind, worktree_id, worktree_path, status, current_task_id,
           failure_count, last_heartbeat_at, base_behind_by, max_concurrent_slots, created_at, updated_at)
        VALUES
          ($id, $assigneeHandle, $runnerKind, $worktreeId, $worktreePath, 'idle', NULL,
           0, $lastHeartbeatAt, $baseBehindBy, $maxConcurrentSlots, $createdAt, $updatedAt)
      `)
      .run({
        $id: input.id,
        $assigneeHandle: input.assigneeHandle,
        $runnerKind: normalizeOrchestrationRunnerKind(input.runnerKind ?? "generic"),
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

  recordDispatchFailure(id: string, maxFailures = 3, now: Date = new Date()): DispatchContext {
    const current = this.getDispatchContext(id)
    if (current === null) throw new Error(`unknown dispatch context: ${id}`)
    const failureCount = current.failureCount + 1
    const status: DispatchContextStatus = failureCount >= maxFailures ? "circuit_broken" : "idle"
    const at = iso(now)
    this.db.run("BEGIN IMMEDIATE")
    try {
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
    const contextStatus: DispatchContextStatus =
      input.status === "failed" && failureCount >= maxFailures ? "circuit_broken" : "idle"
    const body = input.body ?? `worker_done ${input.taskId} ${input.status}`

    this.db.run("BEGIN IMMEDIATE")
    try {
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
