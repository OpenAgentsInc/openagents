import { createHash } from "node:crypto"
import type { ControlSessionSpawnCommand } from "./control-sessions.js"
import {
  runSessionsExec,
  type ApprovalPolicy,
  type ApprovalPolicyCallback,
  type SessionsExecControl,
  type SessionsExecOptions,
  type SessionsExecResult,
} from "./sessions-exec.js"

const RESULT_SCHEMA = "openagents.pylon.sessions_batch_result.v0.1"

export type SessionsBatchTask = {
  id: string
  objective: string
  verify?: string[]
  worktreePath?: string
  timeoutSeconds?: number
}

export type SessionsBatchOptions = {
  adapter: "codex" | "claude_agent"
  approvalAudit?: SessionsExecOptions["approvalAudit"]
  approvalPolicy?: ApprovalPolicyCallback
  concurrency?: number
  deadlineMs?: number
  lane?: ControlSessionSpawnCommand["lane"]
  now?: () => number
  onApproval?: ApprovalPolicy
  pollIntervalMs?: number
  repoRef?: ControlSessionSpawnCommand["repoRef"]
  sleep?: (ms: number) => Promise<void>
  tasks: SessionsBatchTask[]
  timeoutSeconds?: number
  verify: string[]
  worktreePath?: string
}

export type SessionsBatchTaskResult = {
  id: string
  objectiveRef: string
  ok: boolean
  result: SessionsExecResult
}

export type SessionsBatchResult = {
  schema: typeof RESULT_SCHEMA
  ok: boolean
  taskCount: number
  concurrency: number
  results: SessionsBatchTaskResult[]
  failures: Array<{
    id: string
    outcome: SessionsExecResult["outcome"]
    sessionRef: string
    errorClass: string | null
    errorDigestRef: string | null
  }>
}

function objectiveRef(id: string, objective: string): string {
  return `digest.pylon.sessions_batch.objective.${
    createHash("sha256").update(`${id}:${objective}`).digest("hex").slice(0, 24)
  }`
}

function failedTaskResult(input: {
  adapter: "codex" | "claude_agent"
  error: unknown
  now: () => number
  startedAt: number
}): SessionsExecResult {
  const digest = createHash("sha256")
    .update(input.error instanceof Error ? input.error.message : String(input.error))
    .digest("hex")
    .slice(0, 24)
  return {
    schema: "openagents.pylon.sessions_exec_result.v0.1",
    ok: false,
    outcome: "failed",
    sessionRef: "",
    adapter: input.adapter,
    state: "failed",
    resultSummary: null,
    resultRef: null,
    artifactRef: null,
    changeset: null,
    verify: null,
    errorClass: "driver_error",
    errorDigestRef: `digest.pylon.sessions_batch.error.${digest}`,
    pendingApprovals: [],
    autoApprovals: [],
    startedAt: null,
    completedAt: null,
    driver: { elapsedMs: input.now() - input.startedAt, polls: 0, timedOut: false },
  }
}

function positiveInteger(value: unknown): number | null {
  return typeof value === "number" && Number.isInteger(value) && value > 0 ? value : null
}

function stringArray(value: unknown): string[] | null {
  if (!Array.isArray(value) || value.length === 0) return null
  const entries = value.filter((entry): entry is string => typeof entry === "string" && entry.trim().length > 0)
  return entries.length === value.length ? entries.map((entry) => entry.trim()) : null
}

export function parseSessionsBatchTasks(input: unknown): SessionsBatchTask[] {
  if (!Array.isArray(input)) throw new Error("sessions batch tasks must be a JSON array")
  if (input.length === 0) throw new Error("sessions batch tasks must not be empty")
  return input.map((entry, index) => {
    const fallbackId = `task-${index + 1}`
    if (typeof entry === "string") {
      const objective = entry.trim()
      if (objective.length === 0) throw new Error(`sessions batch ${fallbackId} objective is empty`)
      return { id: fallbackId, objective }
    }
    if (entry === null || typeof entry !== "object") {
      throw new Error(`sessions batch ${fallbackId} must be a string or object`)
    }
    const record = entry as Record<string, unknown>
    const objective = typeof record.objective === "string" ? record.objective.trim() : ""
    if (objective.length === 0) throw new Error(`sessions batch ${fallbackId} requires objective`)
    const rawId = typeof record.id === "string" && record.id.trim().length > 0 ? record.id.trim() : fallbackId
    if (!/^[A-Za-z0-9_.:-]{1,80}$/.test(rawId)) throw new Error(`sessions batch ${fallbackId} id is invalid`)
    const verify = record.verify === undefined ? undefined : stringArray(record.verify)
    if (verify === null) {
      throw new Error(`sessions batch ${rawId} verify must be a non-empty string array`)
    }
    const timeoutSeconds =
      record.timeoutSeconds === undefined ? undefined : positiveInteger(record.timeoutSeconds)
    if (timeoutSeconds === null) {
      throw new Error(`sessions batch ${rawId} timeoutSeconds must be a positive integer`)
    }
    const worktreePath =
      typeof record.worktreePath === "string" && record.worktreePath.trim().length > 0
        ? record.worktreePath.trim()
        : undefined
    return {
      id: rawId,
      objective,
      ...(verify === undefined ? {} : { verify }),
      ...(worktreePath === undefined ? {} : { worktreePath }),
      ...(timeoutSeconds === undefined ? {} : { timeoutSeconds }),
    }
  })
}

export async function runSessionsBatch(
  control: SessionsExecControl,
  options: SessionsBatchOptions,
): Promise<SessionsBatchResult> {
  const concurrency = Math.max(1, Math.min(options.concurrency ?? 2, options.tasks.length || 1))
  const results = new Array<SessionsBatchTaskResult | undefined>(options.tasks.length)
  let next = 0

  const worker = async () => {
    for (;;) {
      const index = next
      next += 1
      const task = options.tasks[index]
      if (task === undefined) return
      const now = options.now ?? (() => Date.now())
      const startedAt = now()
      let result: SessionsExecResult
      try {
        result = await runSessionsExec(control, {
          adapter: options.adapter,
          ...(options.lane === undefined ? {} : { lane: options.lane }),
          objective: task.objective,
          verify: task.verify ?? options.verify,
          ...(options.repoRef ? { repoRef: options.repoRef } : {}),
          ...(options.repoRef === undefined && (task.worktreePath ?? options.worktreePath)
            ? { worktreePath: task.worktreePath ?? options.worktreePath }
            : {}),
          timeoutSeconds: task.timeoutSeconds ?? options.timeoutSeconds,
          onApproval: options.onApproval,
          ...(options.approvalPolicy ? { approvalPolicy: options.approvalPolicy } : {}),
          ...(options.approvalAudit ? { approvalAudit: options.approvalAudit } : {}),
          ...(options.pollIntervalMs === undefined ? {} : { pollIntervalMs: options.pollIntervalMs }),
          ...(options.deadlineMs === undefined ? {} : { deadlineMs: options.deadlineMs }),
          ...(options.now === undefined ? {} : { now: options.now }),
          ...(options.sleep === undefined ? {} : { sleep: options.sleep }),
        })
      } catch (error) {
        result = failedTaskResult({ adapter: options.adapter, error, now, startedAt })
      }
      results[index] = {
        id: task.id,
        objectiveRef: objectiveRef(task.id, task.objective),
        ok: result.ok,
        result,
      }
    }
  }

  await Promise.all(Array.from({ length: concurrency }, () => worker()))
  const settled = results.filter((entry): entry is SessionsBatchTaskResult => entry !== undefined)
  const failures = settled
    .filter((entry) => !entry.ok)
    .map((entry) => ({
      id: entry.id,
      outcome: entry.result.outcome,
      sessionRef: entry.result.sessionRef,
      errorClass: entry.result.errorClass,
      errorDigestRef: entry.result.errorDigestRef,
    }))
  return {
    schema: RESULT_SCHEMA,
    ok: failures.length === 0,
    taskCount: options.tasks.length,
    concurrency,
    results: settled,
    failures,
  }
}
