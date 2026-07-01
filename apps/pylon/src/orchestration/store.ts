import type { Database } from "bun:sqlite"
import { createHash } from "node:crypto"
import {
  isAgentRunnerKind,
  normalizeAgentRunnerKind,
  type AgentRunnerKind,
  type LegacyAgentRunnerKind,
} from "../agent-runner-registry.js"

export const ORCHESTRATION_SCHEMA_VERSION = 1

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
  createdAt: string
  updatedAt: string
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
    `)
    this.db
      .query("INSERT OR REPLACE INTO pylon_orchestration_meta (key, value) VALUES ('schema_version', $version)")
      .run({ $version: String(ORCHESTRATION_SCHEMA_VERSION) })
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

export const createPylonOrchestrationStore = (db: SqliteDatabase): PylonOrchestrationStore => {
  const store = new PylonOrchestrationStore(db)
  store.migrate()
  return store
}
