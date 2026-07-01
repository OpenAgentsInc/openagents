import type { Database } from "bun:sqlite"
import { createHash } from "node:crypto"
import { existsSync } from "node:fs"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { Schema as S } from "effect"
import type { PylonPaths } from "../state.js"
import {
  isAgentRunnerKind,
  normalizeAgentRunnerKind,
  type AgentRunnerKind,
  type LegacyAgentRunnerKind,
} from "../agent-runner-registry.js"

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

export const FleetRunStopConditionSchema = S.Literals(["backlog_empty", "target_reached", "manual_stop"])
export type FleetRunStopCondition = typeof FleetRunStopConditionSchema.Type

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

export function decodeFleetRun(input: unknown): FleetRun {
  const run = S.decodeUnknownSync(FleetRunSchema)(input)
  if (!run.runRef.trim()) throw new Error("fleet run runRef is required")
  if (!run.objective.trim()) throw new Error("fleet run objective is required")
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
  return kind === "generic" ? "generic" : normalizeAgentRunnerKind(kind)
}

export function isStoredOrchestrationRunnerKind(kind: string): kind is StoredRunnerKind {
  return kind === "generic" || kind === "claude" || isAgentRunnerKind(kind)
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

const virtualHeadProjectionRef = (input: { repo: string; branch: string; taskId: string; branchFrom: string }): string => {
  const digest = createHash("sha256")
    .update(`${input.repo}\0${input.branch}\0${input.taskId}\0${input.branchFrom}`)
    .digest("hex")
    .slice(0, 20)
  return `virtual-head.${digest}`
}

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
             SET status = 'dispatched', current_task_id = $taskId, updated_at = $at
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

export const fleetRunOwnerLocalStatePath = (paths: Pick<PylonPaths, "home">): string =>
  join(paths.home, "fleet-runs.json")

export async function loadFleetRunOwnerLocalState(
  paths: Pick<PylonPaths, "home">,
): Promise<FleetRunOwnerLocalState> {
  const path = fleetRunOwnerLocalStatePath(paths)
  if (!existsSync(path)) return { schema: FLEET_RUN_OWNER_LOCAL_STATE_SCHEMA, runs: [] }
  const parsed: unknown = JSON.parse(await readFile(path, "utf8"))
  const state = S.decodeUnknownSync(FleetRunOwnerLocalStateSchema)(parsed)
  return { ...state, runs: state.runs.map(decodeFleetRun) }
}

export async function saveFleetRunOwnerLocalState(
  paths: Pick<PylonPaths, "home">,
  state: FleetRunOwnerLocalState,
): Promise<void> {
  const decoded = S.decodeUnknownSync(FleetRunOwnerLocalStateSchema)(state)
  const validated = { ...decoded, runs: decoded.runs.map(decodeFleetRun) }
  await mkdir(paths.home, { recursive: true })
  await writeFile(fleetRunOwnerLocalStatePath(paths), `${JSON.stringify(validated, null, 2)}\n`, { mode: 0o600 })
}

export async function syncFleetRunsToOwnerLocalState(
  store: PylonOrchestrationStore,
  paths: Pick<PylonPaths, "home">,
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
  paths: Pick<PylonPaths, "home">,
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
