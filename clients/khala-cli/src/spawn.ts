import { createHash, randomUUID } from "node:crypto"
import { existsSync } from "node:fs"
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"

import { khalaHome, runKhalaCodexTask, type KhalaCodexRunResult } from "./codex.js"
import { DEFAULT_BASE_URL } from "./types.js"

export const KHALA_SPAWN_RUN_SCHEMA = "openagents.khala.spawn_run.v0.1"
export const KHALA_SPAWN_WORKER_SCHEMA = "openagents.khala.spawn_worker.v0.1"
export const KHALA_SPAWN_RUNS_SCHEMA = "openagents.khala.spawn_runs.v0.1"
export const KHALA_SPAWN_CANCEL_SCHEMA = "openagents.khala.spawn_cancel.v0.1"

export type KhalaSpawnStrategy = "auto" | "local" | "pylon"
export type KhalaSpawnResolvedStrategy = "local_codex_threads" | "pylon_codex_assignments"
export type KhalaSpawnState = "planned" | "running" | "completed" | "cancelled" | "failed"
export type KhalaSpawnWorkerState =
  | "queued"
  | "starting"
  | "running"
  | "accepted"
  | "rejected"
  | "cancelled"
  | "failed"

export type KhalaSpawnWorker = {
  readonly schema: typeof KHALA_SPAWN_WORKER_SCHEMA
  readonly workerRef: string
  readonly runRef: string
  readonly slotIndex: number
  readonly objective: string
  readonly state: KhalaSpawnWorkerState
  readonly assignmentRef?: string | undefined
  readonly durableRequestId?: string | undefined
  readonly pylonRef?: string | undefined
  readonly accountRefHash?: string | undefined
  readonly localWorktree?: string | undefined
  readonly sessionRef?: string | undefined
  readonly proofRef?: string | undefined
  readonly blockerRefs: readonly string[]
  readonly commandCount?: number | undefined
  readonly editedFileCount?: number | undefined
  readonly error?: string | undefined
  readonly resultText?: string | undefined
  readonly startedAt?: string | undefined
  readonly completedAt?: string | undefined
}

export type KhalaSpawnRun = {
  readonly schema: typeof KHALA_SPAWN_RUN_SCHEMA
  readonly runRef: string
  readonly objective: string
  readonly requestedCount: number
  readonly maxParallel: number
  readonly requestedStrategy: KhalaSpawnStrategy
  readonly strategy: KhalaSpawnResolvedStrategy
  readonly state: KhalaSpawnState
  readonly createdAt: string
  readonly updatedAt: string
  readonly workers: readonly KhalaSpawnWorker[]
}

export type KhalaSpawnRunsProjection = {
  readonly schema: typeof KHALA_SPAWN_RUNS_SCHEMA
  readonly runs: readonly KhalaSpawnRun[]
}

export type KhalaSpawnCancelProjection = {
  readonly schema: typeof KHALA_SPAWN_CANCEL_SCHEMA
  readonly ok: true
  readonly targetRef: string
  readonly cancelledWorkers: readonly string[]
  readonly runRefs: readonly string[]
}

export type KhalaSpawnRunOptions = {
  readonly baseUrl?: string | undefined
  readonly branch?: string | undefined
  readonly commit?: string | undefined
  readonly count: number
  readonly cwd: string
  readonly env?: Record<string, string | undefined> | undefined
  readonly fixture?: boolean | undefined
  readonly maxParallel?: number | undefined
  readonly mcpCaller?: KhalaSpawnMcpCaller | undefined
  readonly objective: string
  readonly pylonRef?: string | undefined
  readonly repo?: string | undefined
  readonly strategy?: KhalaSpawnStrategy | undefined
  readonly timeoutMs?: number | undefined
  readonly token?: string | undefined
  readonly verify?: string | undefined
  readonly workflow?: KhalaSpawnWorkflow | undefined
  readonly runner?: KhalaSpawnWorkerRunner | undefined
  readonly workspaceFactory?: KhalaSpawnWorkspaceFactory | undefined
  readonly now?: (() => Date) | undefined
  readonly onEvent?: ((event: KhalaSpawnLifecycleEvent) => void | Promise<void>) | undefined
}

export type KhalaSpawnWorkflow = "cloud_coding_session" | "codex_agent_task"

export type KhalaSpawnMcpToolName = "khala.spawn" | "khala.spawnStatus"

export type KhalaSpawnMcpCaller = (input: {
  readonly args: Record<string, unknown>
  readonly baseUrl: string
  readonly env: Record<string, string | undefined>
  readonly token: string
  readonly tool: KhalaSpawnMcpToolName
}) => Promise<unknown>

export type KhalaSpawnWorkerRunner = (input: {
  readonly signal: AbortSignal
  readonly worker: KhalaSpawnWorker
  readonly workspace: string
  readonly prompt: string
  readonly env: Record<string, string | undefined>
  readonly timeoutMs?: number | undefined
}) => Promise<KhalaCodexRunResult>

export type KhalaSpawnWorkspaceFactory = (input: {
  readonly cwd: string
  readonly runRef: string
  readonly workerRef: string
  readonly env: Record<string, string | undefined>
}) => Promise<string>

export type KhalaSpawnLifecycleEvent = {
  readonly runRef: string
  readonly workerRef?: string | undefined
  readonly state: KhalaSpawnState | KhalaSpawnWorkerState
  readonly message: string
  readonly observedAt: string
}

const DEFAULT_MAX_COUNT = 10
const PYLON_MAX_COUNT = 20
const DEFAULT_TIMEOUT_MS = 10 * 60 * 1000
const TERMINAL_WORKER_STATES = new Set<KhalaSpawnWorkerState>([
  "accepted",
  "rejected",
  "cancelled",
  "failed",
])
const runWriteQueues = new Map<string, Promise<KhalaSpawnRun>>()

export function boundedSpawnCount(value: number, label = "count"): number {
  return boundedSpawnCountFor(value, label, DEFAULT_MAX_COUNT, "local supervised workers")
}

function boundedPylonSpawnCount(value: number, label = "count"): number {
  return boundedSpawnCountFor(value, label, PYLON_MAX_COUNT, "Pylon supervised workers")
}

function boundedSpawnCountFor(value: number, label: string, max: number, surface: string): number {
  if (!Number.isFinite(value) || value <= 0 || Math.floor(value) !== value) {
    throw new Error(`khala spawn ${label} must be a positive integer`)
  }
  if (value > max) {
    throw new Error(`khala spawn ${label} is capped at ${max} for ${surface}`)
  }
  return value
}

export function cleanSpawnObjective(value: string): string {
  const objective = value.trim()
  if (objective.length < 3 || objective.length > 8_000) {
    throw new Error("khala spawn objective must be 3-8000 characters")
  }
  return objective
}

export function spawnRoot(env: Record<string, string | undefined> = Bun.env): string {
  return join(khalaHome(env), "spawn")
}

export function spawnRunDir(env: Record<string, string | undefined>, runRef: string): string {
  return join(spawnRoot(env), "runs", safeFilename(runRef))
}

export async function runKhalaSpawn(options: KhalaSpawnRunOptions): Promise<KhalaSpawnRun> {
  const env = options.env ?? Bun.env
  const requestedStrategy = options.strategy ?? "auto"
  if (requestedStrategy === "pylon") {
    return runPylonKhalaSpawn(options, env)
  }
  const count = boundedSpawnCount(options.count)
  const maxParallel = boundedSpawnCount(Math.min(options.maxParallel ?? count, count), "max-parallel")
  const objective = cleanSpawnObjective(options.objective)
  const strategy: KhalaSpawnResolvedStrategy = "local_codex_threads"
  const now = options.now ?? (() => new Date())
  const createdAt = now().toISOString()
  const runRef = stableRef("khala_spawn", `${createdAt}:${objective}:${randomUUID()}`, 12)
  const workers = Array.from({ length: count }, (_, index): KhalaSpawnWorker => ({
    schema: KHALA_SPAWN_WORKER_SCHEMA,
    blockerRefs: [],
    objective: workerObjective(objective, index + 1, count),
    runRef,
    slotIndex: index + 1,
    state: "queued",
    workerRef: `${runRef}.worker.${String(index + 1).padStart(2, "0")}`,
  }))
  let run: KhalaSpawnRun = {
    schema: KHALA_SPAWN_RUN_SCHEMA,
    createdAt,
    maxParallel,
    objective,
    requestedCount: count,
    requestedStrategy,
    runRef,
    state: "planned",
    strategy,
    updatedAt: createdAt,
    workers,
  }
  await writeSpawnRun(env, run)
  await emit(options, { runRef, state: "planned", message: `planned ${count} Khala worker(s)`, observedAt: createdAt })

  run = updateRun(run, { state: "running", updatedAt: now().toISOString() })
  await writeSpawnRun(env, run)
  await emit(options, { runRef, state: "running", message: "spawn supervisor started", observedAt: run.updatedAt })

  let nextIndex = 0
  const startNext = async (): Promise<void> => {
    const worker = run.workers[nextIndex]
    nextIndex += 1
    if (worker === undefined) return
    run = await runOneWorker({
      env,
      now,
      options,
      run,
      worker,
    })
  }
  const lanes = Array.from({ length: Math.min(maxParallel, workers.length) }, () => (async () => {
    while (nextIndex < workers.length) {
      if (await spawnCancelled(env, runRef)) {
        const pending = run.workers.filter(worker => !TERMINAL_WORKER_STATES.has(worker.state))
        run = updateRun(run, {
          state: "cancelled",
          updatedAt: now().toISOString(),
          workers: run.workers.map(worker =>
            pending.some(candidate => candidate.workerRef === worker.workerRef)
              ? updateWorker(worker, {
                  blockerRefs: [...worker.blockerRefs, "blocker.khala_spawn.cancelled"],
                  completedAt: now().toISOString(),
                  state: "cancelled",
                })
              : worker,
          ),
        })
        await writeSpawnRun(env, run)
        return
      }
      await startNext()
    }
  })())
  await Promise.all(lanes)

  try {
    run = await latestRunWrite(env, run)
    const finalState = await spawnCancelled(env, run.runRef) ? "cancelled" : finalRunState(run.workers)
    run = updateRun(run, { state: finalState, updatedAt: now().toISOString() })
    await writeSpawnRun(env, run)
    await emit(options, {
      runRef,
      state: finalState,
      message: `spawn supervisor finished with ${acceptedWorkerCount(run)} accepted worker(s)`,
      observedAt: run.updatedAt,
    })
    return run
  } finally {
    runWriteQueues.delete(spawnRunDir(env, run.runRef))
  }
}

async function runPylonKhalaSpawn(
  options: KhalaSpawnRunOptions,
  env: Record<string, string | undefined>,
): Promise<KhalaSpawnRun> {
  const count = boundedPylonSpawnCount(options.count)
  const maxParallel = boundedPylonSpawnCount(Math.min(options.maxParallel ?? count, count), "max-parallel")
  const objective = cleanSpawnObjective(options.objective)
  const now = options.now ?? (() => new Date())
  const createdAt = now().toISOString()
  const projection = decodePylonSpawnProjection(await callKhalaSpawnMcpTool(options, env, "khala.spawn", {
    count,
    maxParallel,
    objective,
    workflow: options.workflow ?? "codex_agent_task",
    ...(options.branch === undefined ? {} : { branch: options.branch }),
    ...(options.commit === undefined ? {} : { commit: options.commit }),
    ...(options.fixture === undefined ? {} : { fixture: options.fixture }),
    ...(options.pylonRef === undefined ? {} : { targetPylonRef: options.pylonRef }),
    ...(options.repo === undefined ? {} : { repo: options.repo }),
    ...(options.verify === undefined ? {} : { verify: options.verify }),
  }))
  const run = pylonRunFromProjection({
    count,
    createdAt,
    maxParallel,
    objective,
    projection,
    requestedStrategy: options.strategy ?? "pylon",
  })
  await writeSpawnRun(env, run)
  for (const worker of run.workers) {
    await appendWorkerEvent(env, run.runRef, worker.workerRef, {
      assignmentRef: worker.assignmentRef,
      durableRequestId: worker.durableRequestId,
      observedAt: run.updatedAt,
      pylonRef: worker.pylonRef,
      state: worker.state,
      workerRef: worker.workerRef,
    })
  }
  await emit(options, {
    message: `pylon spawn dispatched ${projection.assignedCount} of ${count} requested worker(s)`,
    observedAt: run.updatedAt,
    runRef: run.runRef,
    state: run.state,
  })
  return run
}

export async function refreshKhalaPylonSpawnRun(input: {
  readonly baseUrl?: string | undefined
  readonly env?: Record<string, string | undefined> | undefined
  readonly mcpCaller?: KhalaSpawnMcpCaller | undefined
  readonly run: KhalaSpawnRun
  readonly token?: string | undefined
}): Promise<KhalaSpawnRun> {
  if (input.run.strategy !== "pylon_codex_assignments") return input.run
  const env = input.env ?? Bun.env
  const projection = decodePylonSpawnStatusProjection(await callKhalaSpawnMcpTool({
    baseUrl: input.baseUrl,
    env,
    mcpCaller: input.mcpCaller,
    token: input.token,
  }, env, "khala.spawnStatus", { spawnRef: input.run.runRef }))
  const updatedAt = new Date().toISOString()
  const statusByAssignment = new Map(
    projection.children
      .filter(child => child.assignmentRef !== undefined)
      .map(child => [child.assignmentRef!, child]),
  )
  const workers = input.run.workers.map(worker => {
    const status = worker.assignmentRef === undefined
      ? undefined
      : statusByAssignment.get(worker.assignmentRef)
    if (status === undefined) return worker
    const state = pylonAssignmentStateToWorkerState(status.state)
    return updateWorker(worker, {
      blockerRefs: state === "rejected" || state === "failed"
        ? [...new Set([...worker.blockerRefs, "blocker.khala_spawn.pylon_assignment_not_accepted"])]
        : worker.blockerRefs,
      completedAt: TERMINAL_WORKER_STATES.has(state) ? worker.completedAt ?? updatedAt : worker.completedAt,
      durableRequestId: status.durableRequestId ?? worker.durableRequestId,
      pylonRef: status.pylonRef ?? worker.pylonRef,
      resultText: pylonWorkerResultText(status),
      state,
    })
  })
  const run = updateRun(input.run, {
    state: projection.state === "accepted"
      ? "completed"
      : projection.state === "rejected"
        ? "failed"
        : finalRunState(workers),
    updatedAt,
    workers,
  })
  await writeSpawnRun(env, run)
  return run
}

type PylonSpawnChildProjection = {
  readonly assignmentRef?: string | undefined
  readonly durableRequestId?: string | undefined
  readonly durableStreamUrl?: string | undefined
  readonly error?: string | undefined
  readonly ok: boolean
  readonly pylonRef?: string | undefined
  readonly reason?: string | undefined
  readonly slotIndex?: number | undefined
  readonly state?: string | undefined
  readonly workerRef?: string | undefined
}

type PylonSpawnProjection = {
  readonly assignedCount: number
  readonly blockerRefs: readonly string[]
  readonly children: readonly PylonSpawnChildProjection[]
  readonly ok: boolean
  readonly requestedCount: number
  readonly spawnRef: string
}

type PylonSpawnStatusChildProjection = {
  readonly assignmentRef?: string | undefined
  readonly durableRequestId?: string | undefined
  readonly pylonRef?: string | undefined
  readonly state?: string | undefined
}

type PylonSpawnStatusProjection = {
  readonly children: readonly PylonSpawnStatusChildProjection[]
  readonly spawnRef: string
  readonly state: "active" | "accepted" | "rejected"
}

async function callKhalaSpawnMcpTool(
  options: Pick<KhalaSpawnRunOptions, "baseUrl" | "env" | "mcpCaller" | "token">,
  env: Record<string, string | undefined>,
  tool: KhalaSpawnMcpToolName,
  args: Record<string, unknown>,
): Promise<unknown> {
  const baseUrl = options.baseUrl ?? env.KHALA_BASE_URL ?? DEFAULT_BASE_URL
  const token = options.token?.trim() || env.OPENAGENTS_AGENT_TOKEN?.trim()
  if (token === undefined || token.length === 0) {
    throw new Error("khala spawn --strategy pylon requires OPENAGENTS_AGENT_TOKEN, `khala login`, or --token")
  }
  const caller = options.mcpCaller ?? remoteKhalaSpawnMcpCaller
  return caller({ args, baseUrl, env, token, tool })
}

const remoteKhalaSpawnMcpCaller: KhalaSpawnMcpCaller = async input => {
  const response = await fetch(new URL("/api/mcp", normalizedBaseUrl(input.baseUrl)), {
    body: JSON.stringify({
      id: `khala-cli.${input.tool}.${randomUUID()}`,
      jsonrpc: "2.0",
      method: "tools/call",
      params: {
        arguments: input.args,
        name: input.tool,
      },
    }),
    headers: {
      accept: "application/json",
      authorization: `Bearer ${input.token}`,
      "content-type": "application/json",
    },
    method: "POST",
  })
  const text = await response.text()
  if (!response.ok) {
    throw new Error(`remote ${input.tool} failed (${response.status}): ${text.trim() || response.statusText}`)
  }
  const envelope = parseJsonRecord(text, `remote ${input.tool} response`)
  const error = recordValue(envelope, "error")
  if (isRecord(error)) {
    throw new Error(stringValue(error, "message") ?? `remote ${input.tool} returned an error`)
  }
  const result = recordValue(envelope, "result")
  if (!isRecord(result)) {
    throw new Error(`remote ${input.tool} returned no result`)
  }
  return structuredMcpContent(result, input.tool)
}

function structuredMcpContent(result: Record<string, unknown>, tool: string): unknown {
  if ("structuredContent" in result) return result.structuredContent
  const content = arrayValue(result, "content")
  const text = content
    .map(item => isRecord(item) && item.type === "text" ? stringValue(item, "text") : undefined)
    .find(item => item !== undefined)
  if (text !== undefined) {
    try {
      return JSON.parse(text)
    } catch {
      if (result.isError === true) throw new Error(text)
      return { text }
    }
  }
  if (result.isError === true) {
    throw new Error(`remote ${tool} returned an error`)
  }
  return result
}

function decodePylonSpawnProjection(value: unknown): PylonSpawnProjection {
  const record = expectRecord(value, "khala.spawn output")
  const spawnRef = requiredStringValue(record, "spawnRef")
  return {
    assignedCount: integerValue(record, "assignedCount", 0),
    blockerRefs: stringArrayValue(record, "blockerRefs"),
    children: arrayValue(record, "children").map(decodePylonSpawnChildProjection),
    ok: record.ok === true,
    requestedCount: integerValue(record, "requestedCount", 0),
    spawnRef,
  }
}

function decodePylonSpawnChildProjection(value: unknown): PylonSpawnChildProjection {
  const record = expectRecord(value, "khala.spawn child")
  return {
    assignmentRef: stringValue(record, "assignmentRef"),
    durableRequestId: stringValue(record, "durableRequestId"),
    durableStreamUrl: stringValue(record, "durableStreamUrl"),
    error: stringValue(record, "error"),
    ok: record.ok === true,
    pylonRef: stringValue(record, "pylonRef"),
    reason: stringValue(record, "reason"),
    slotIndex: optionalIntegerValue(record, "slotIndex"),
    state: stringValue(record, "state"),
    workerRef: stringValue(record, "workerRef"),
  }
}

function decodePylonSpawnStatusProjection(value: unknown): PylonSpawnStatusProjection {
  const record = expectRecord(value, "khala.spawnStatus output")
  const state = stringValue(record, "state")
  return {
    children: arrayValue(record, "children").map(child => {
      const childRecord = expectRecord(child, "khala.spawnStatus child")
      return {
        assignmentRef: stringValue(childRecord, "assignmentRef"),
        durableRequestId: childRecord.durableRequestId === null ? undefined : stringValue(childRecord, "durableRequestId"),
        pylonRef: stringValue(childRecord, "pylonRef"),
        state: stringValue(childRecord, "state"),
      }
    }),
    spawnRef: requiredStringValue(record, "spawnRef"),
    state: state === "accepted" || state === "rejected" ? state : "active",
  }
}

function pylonRunFromProjection(input: {
  readonly count: number
  readonly createdAt: string
  readonly maxParallel: number
  readonly objective: string
  readonly projection: PylonSpawnProjection
  readonly requestedStrategy: KhalaSpawnStrategy
}): KhalaSpawnRun {
  const childBySlot = new Map<number, PylonSpawnChildProjection>()
  input.projection.children.forEach((child, index) => {
    const slot = child.slotIndex === undefined ? index + 1 : child.slotIndex + 1
    childBySlot.set(slot, child)
  })
  const workers = Array.from({ length: input.count }, (_, index): KhalaSpawnWorker => {
    const slotIndex = index + 1
    const child = childBySlot.get(slotIndex)
    if (child === undefined) {
      return pylonMissingWorker({
        blockerRefs: input.projection.blockerRefs,
        objective: input.objective,
        runRef: input.projection.spawnRef,
        slotIndex,
        total: input.count,
      })
    }
    return pylonWorkerFromChild({
      child,
      objective: input.objective,
      runRef: input.projection.spawnRef,
      slotIndex,
      total: input.count,
    })
  })
  return {
    schema: KHALA_SPAWN_RUN_SCHEMA,
    createdAt: input.createdAt,
    maxParallel: input.maxParallel,
    objective: input.objective,
    requestedCount: input.count,
    requestedStrategy: input.requestedStrategy,
    runRef: input.projection.spawnRef,
    state: input.projection.ok ? finalRunState(workers) : finalRunState(workers),
    strategy: "pylon_codex_assignments",
    updatedAt: input.createdAt,
    workers,
  }
}

function pylonWorkerFromChild(input: {
  readonly child: PylonSpawnChildProjection
  readonly objective: string
  readonly runRef: string
  readonly slotIndex: number
  readonly total: number
}): KhalaSpawnWorker {
  const state = input.child.ok
    ? pylonAssignmentStateToWorkerState(input.child.state)
    : "rejected"
  const blockerRefs = [
    ...(input.child.ok ? [] : ["blocker.khala_spawn.pylon_assignment_not_started"]),
    ...(input.child.error === undefined ? [] : [`blocker.khala_mcp.spawn.${input.child.error}`]),
  ]
  return {
    schema: KHALA_SPAWN_WORKER_SCHEMA,
    assignmentRef: input.child.assignmentRef,
    blockerRefs,
    completedAt: TERMINAL_WORKER_STATES.has(state) ? new Date().toISOString() : undefined,
    durableRequestId: input.child.durableRequestId,
    error: input.child.error ?? input.child.reason,
    objective: workerObjective(input.objective, input.slotIndex, input.total),
    pylonRef: input.child.pylonRef,
    resultText: pylonWorkerResultText(input.child),
    runRef: input.runRef,
    slotIndex: input.slotIndex,
    startedAt: input.child.ok ? new Date().toISOString() : undefined,
    state,
    workerRef: input.child.workerRef ?? `${input.runRef}.worker.${String(input.slotIndex).padStart(2, "0")}`,
  }
}

function pylonMissingWorker(input: {
  readonly blockerRefs: readonly string[]
  readonly objective: string
  readonly runRef: string
  readonly slotIndex: number
  readonly total: number
}): KhalaSpawnWorker {
  return {
    schema: KHALA_SPAWN_WORKER_SCHEMA,
    blockerRefs: input.blockerRefs.length === 0
      ? ["blocker.khala_mcp.spawn.capacity_shortfall"]
      : input.blockerRefs,
    completedAt: new Date().toISOString(),
    objective: workerObjective(input.objective, input.slotIndex, input.total),
    runRef: input.runRef,
    slotIndex: input.slotIndex,
    state: "rejected",
    workerRef: `${input.runRef}.worker.${String(input.slotIndex).padStart(2, "0")}`,
  }
}

function pylonAssignmentStateToWorkerState(state: string | undefined): KhalaSpawnWorkerState {
  if (state === "accepted") return "accepted"
  if (state === "cancelled") return "cancelled"
  if (state === "blocked" || state === "rejected" || state === "stale") return "rejected"
  return "running"
}

function pylonWorkerResultText(child: {
  readonly assignmentRef?: string | undefined
  readonly durableRequestId?: string | undefined
  readonly pylonRef?: string | undefined
  readonly state?: string | undefined
}): string {
  return [
    `pylon assignment state: ${child.state ?? "unknown"}`,
    ...(child.assignmentRef === undefined ? [] : [`assignment: ${child.assignmentRef}`]),
    ...(child.durableRequestId === undefined ? [] : [`durable request: ${child.durableRequestId}`]),
    ...(child.pylonRef === undefined ? [] : [`pylon: ${child.pylonRef}`]),
  ].join("\n")
}

function normalizedBaseUrl(baseUrl: string): string {
  return baseUrl.replace(/\/+$/, "")
}

function parseJsonRecord(text: string, label: string): Record<string, unknown> {
  try {
    return expectRecord(JSON.parse(text), label)
  } catch (error) {
    throw new Error(`${label} was not valid JSON: ${error instanceof Error ? error.message : String(error)}`)
  }
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!isRecord(value)) throw new Error(`${label} was not an object`)
  return value
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function recordValue(record: Record<string, unknown>, key: string): unknown {
  return record[key]
}

function arrayValue(record: Record<string, unknown>, key: string): readonly unknown[] {
  const value = record[key]
  return Array.isArray(value) ? value : []
}

function stringValue(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key]
  return typeof value === "string" && value.trim().length > 0 ? value : undefined
}

function requiredStringValue(record: Record<string, unknown>, key: string): string {
  const value = stringValue(record, key)
  if (value === undefined) throw new Error(`${key} is required`)
  return value
}

function optionalIntegerValue(record: Record<string, unknown>, key: string): number | undefined {
  const value = record[key]
  if (typeof value === "number" && Number.isFinite(value)) return Math.trunc(value)
  if (typeof value === "string" && value.trim().length > 0) {
    const parsed = Number.parseInt(value, 10)
    return Number.isFinite(parsed) ? parsed : undefined
  }
  return undefined
}

function integerValue(record: Record<string, unknown>, key: string, fallback: number): number {
  return optionalIntegerValue(record, key) ?? fallback
}

function stringArrayValue(record: Record<string, unknown>, key: string): readonly string[] {
  return arrayValue(record, key).filter((value): value is string => typeof value === "string")
}

export async function listKhalaSpawnRuns(
  env: Record<string, string | undefined> = Bun.env,
): Promise<KhalaSpawnRunsProjection> {
  const dir = join(spawnRoot(env), "runs")
  await mkdir(dir, { recursive: true })
  const runs: KhalaSpawnRun[] = []
  for (const name of await readdir(dir)) {
    const runPath = join(dir, name, "run.json")
    try {
      runs.push(decodeRun(JSON.parse(await readFile(runPath, "utf8"))))
    } catch {
      // Ignore malformed local state; it is not useful operator evidence.
    }
  }
  runs.sort((left, right) => right.createdAt.localeCompare(left.createdAt))
  return { schema: KHALA_SPAWN_RUNS_SCHEMA, runs }
}

export async function readKhalaSpawnRun(
  env: Record<string, string | undefined>,
  runRef: string,
): Promise<KhalaSpawnRun> {
  return decodeRun(JSON.parse(await readFile(join(spawnRunDir(env, runRef), "run.json"), "utf8")))
}

export async function readKhalaSpawnWorker(
  env: Record<string, string | undefined>,
  workerRef: string,
): Promise<KhalaSpawnWorker> {
  const listing = await listKhalaSpawnRuns(env)
  for (const run of listing.runs) {
    const worker = run.workers.find(candidate => candidate.workerRef === workerRef)
    if (worker !== undefined) return worker
  }
  throw new Error(`Khala worker not found: ${workerRef}`)
}

export async function cancelKhalaSpawn(
  env: Record<string, string | undefined>,
  targetRef: string,
): Promise<KhalaSpawnCancelProjection> {
  const listing = await listKhalaSpawnRuns(env)
  const touchedRuns: string[] = []
  const cancelledWorkers: string[] = []
  for (const run of listing.runs) {
    const matchesRun = run.runRef === targetRef
    const matchesWorker = run.workers.some(worker => worker.workerRef === targetRef)
    if (!matchesRun && !matchesWorker) continue
    touchedRuns.push(run.runRef)
    await writeCancelRequest(env, run.runRef, matchesRun ? undefined : targetRef)
    const nextWorkers = run.workers.map(worker => {
      const shouldCancel = matchesRun || worker.workerRef === targetRef
      if (!shouldCancel || TERMINAL_WORKER_STATES.has(worker.state)) return worker
      cancelledWorkers.push(worker.workerRef)
      return updateWorker(worker, {
        blockerRefs: [...worker.blockerRefs, "blocker.khala_spawn.cancelled"],
        completedAt: new Date().toISOString(),
        state: "cancelled",
      })
    })
    await writeSpawnRun(env, updateRun(run, {
      state: matchesRun && cancelledWorkers.length > 0 ? "cancelled" : finalRunState(nextWorkers),
      updatedAt: new Date().toISOString(),
      workers: nextWorkers,
    }))
  }
  if (touchedRuns.length === 0) {
    throw new Error(`Khala spawn target not found: ${targetRef}`)
  }
  return {
    schema: KHALA_SPAWN_CANCEL_SCHEMA,
    cancelledWorkers,
    ok: true,
    runRefs: touchedRuns,
    targetRef,
  }
}

export function summarizeSpawnRun(run: KhalaSpawnRun): string {
  const counts = run.workers.reduce<Record<KhalaSpawnWorkerState, number>>((out, worker) => {
    out[worker.state] += 1
    return out
  }, {
    accepted: 0,
    cancelled: 0,
    failed: 0,
    queued: 0,
    rejected: 0,
    running: 0,
    starting: 0,
  })
  const lines = [
    `run: ${run.runRef}`,
    `state: ${run.state}`,
    `strategy: ${run.strategy}`,
    `workers: ${run.workers.length} (${counts.accepted} accepted, ${counts.failed} failed, ${counts.cancelled} cancelled, ${counts.running + counts.starting + counts.queued} active/queued)`,
  ]
  for (const worker of run.workers) {
    const suffix = worker.sessionRef === undefined ? "" : ` session=${worker.sessionRef}`
    lines.push(`  ${worker.slotIndex}. ${worker.workerRef} - ${worker.state}${suffix}`)
  }
  return lines.join("\n")
}

async function runOneWorker(input: {
  readonly env: Record<string, string | undefined>
  readonly now: () => Date
  readonly options: KhalaSpawnRunOptions
  readonly run: KhalaSpawnRun
  readonly worker: KhalaSpawnWorker
}): Promise<KhalaSpawnRun> {
  const { env, now, options, worker } = input
  let run = input.run
  if (await spawnCancelled(env, run.runRef, worker.workerRef)) {
    return await persistWorkerUpdate(env, run, updateWorker(worker, {
      blockerRefs: [...worker.blockerRefs, "blocker.khala_spawn.cancelled"],
      completedAt: now().toISOString(),
      state: "cancelled",
    }))
  }
  const startingAt = now().toISOString()
  const workspaceFactory = options.workspaceFactory ?? createLocalWorkerWorkspace
  let workspace: string
  try {
    workspace = await workspaceFactory({
      cwd: options.cwd,
      env,
      runRef: run.runRef,
      workerRef: worker.workerRef,
    })
  } catch (error) {
    return await persistWorkerUpdate(env, run, updateWorker(worker, {
      blockerRefs: ["blocker.khala_spawn.workspace_unavailable"],
      completedAt: now().toISOString(),
      error: error instanceof Error ? error.message : String(error),
      startedAt: startingAt,
      state: "failed",
    }))
  }

  let nextWorker = updateWorker(worker, {
    localWorktree: workspace,
    startedAt: startingAt,
    state: "starting",
  })
  run = await persistWorkerUpdate(env, run, nextWorker)
  await emit(options, {
    message: "worker starting",
    observedAt: startingAt,
    runRef: run.runRef,
    state: "starting",
    workerRef: worker.workerRef,
  })
  nextWorker = updateWorker(nextWorker, { state: "running" })
  run = await persistWorkerUpdate(env, run, nextWorker)

  const abort = new AbortController()
  const cancelTimer = setInterval(() => {
    void spawnCancelled(env, run.runRef, worker.workerRef)
      .then(cancelled => {
        if (cancelled) abort.abort()
      })
      .catch(() => {})
  }, 500)
  cancelTimer.unref?.()

  try {
    const runner = options.runner ?? runCodexSpawnWorker
    const result = await runner({
      env,
      prompt: workerPrompt(run, nextWorker),
      signal: abort.signal,
      timeoutMs: options.timeoutMs ?? DEFAULT_TIMEOUT_MS,
      worker: nextWorker,
      workspace,
    })
    const completedAt = now().toISOString()
    return await persistWorkerUpdate(env, run, updateWorker(nextWorker, {
      commandCount: result.commandCount,
      completedAt,
      editedFileCount: result.editedFileCount,
      resultText: result.text,
      sessionRef: result.sessionRef ?? undefined,
      state: "accepted",
    }))
  } catch (error) {
    const completedAt = now().toISOString()
    const cancelled = abort.signal.aborted || await spawnCancelled(env, run.runRef, worker.workerRef).catch(() => false)
    return await persistWorkerUpdate(env, run, updateWorker(nextWorker, {
      blockerRefs: [cancelled ? "blocker.khala_spawn.cancelled" : "blocker.khala_spawn.worker_failed"],
      completedAt,
      error: error instanceof Error ? error.message : String(error),
      state: cancelled ? "cancelled" : "failed",
    }))
  } finally {
    clearInterval(cancelTimer)
  }
}

async function runCodexSpawnWorker(input: Parameters<KhalaSpawnWorkerRunner>[0]): Promise<KhalaCodexRunResult> {
  return runKhalaCodexTask({
    cwd: input.workspace,
    env: input.env,
    prompt: input.prompt,
    signal: input.signal,
    timeoutMs: input.timeoutMs,
  })
}

async function persistWorkerUpdate(
  env: Record<string, string | undefined>,
  run: KhalaSpawnRun,
  worker: KhalaSpawnWorker,
): Promise<KhalaSpawnRun> {
  const key = spawnRunDir(env, run.runRef)
  const previous = runWriteQueues.get(key) ?? Promise.resolve(run)
  const nextPromise = previous.then(async latest => {
    let current = latest
    try {
      current = await readKhalaSpawnRun(env, run.runRef)
    } catch {
      current = latest
    }
    const next = updateRun(current, {
      updatedAt: new Date().toISOString(),
      workers: current.workers.map(candidate =>
        candidate.workerRef === worker.workerRef ? worker : candidate,
      ),
    })
    await writeSpawnRun(env, next)
    await appendWorkerEvent(env, next.runRef, worker.workerRef, {
      observedAt: next.updatedAt,
      state: worker.state,
      workerRef: worker.workerRef,
    })
    return next
  })
  runWriteQueues.set(key, nextPromise.catch(() => run))
  return nextPromise
}

async function latestRunWrite(
  env: Record<string, string | undefined>,
  run: KhalaSpawnRun,
): Promise<KhalaSpawnRun> {
  const queued = runWriteQueues.get(spawnRunDir(env, run.runRef))
  if (queued === undefined) return run
  return queued.catch(() => run)
}

async function createLocalWorkerWorkspace(input: {
  readonly cwd: string
  readonly runRef: string
  readonly workerRef: string
  readonly env: Record<string, string | undefined>
}): Promise<string> {
  const root = resolve(input.cwd)
  const workspace = join(spawnRoot(input.env), "worktrees", safeFilename(input.runRef), safeFilename(input.workerRef))
  await mkdir(dirname(workspace), { recursive: true })
  const rev = Bun.spawn(["git", "rev-parse", "--is-inside-work-tree"], {
    cwd: root,
    stderr: "ignore",
    stdout: "ignore",
  })
  if (await rev.exited !== 0) return root
  if (existsSync(workspace)) return workspace
  const proc = Bun.spawn(["git", "worktree", "add", "--detach", workspace, "HEAD"], {
    cwd: root,
    stderr: "pipe",
    stdout: "ignore",
  })
  const stderr = await new Response(proc.stderr).text()
  const exitCode = await proc.exited
  if (exitCode !== 0) {
    throw new Error(`git worktree add failed (${exitCode}): ${stderr.trim()}`)
  }
  return workspace
}

async function writeSpawnRun(env: Record<string, string | undefined>, run: KhalaSpawnRun): Promise<void> {
  const dir = spawnRunDir(env, run.runRef)
  await mkdir(join(dir, "workers"), { recursive: true })
  await writeFile(join(dir, "run.json"), `${JSON.stringify(run, null, 2)}\n`)
}

async function appendWorkerEvent(
  env: Record<string, string | undefined>,
  runRef: string,
  workerRef: string,
  event: Record<string, unknown>,
): Promise<void> {
  const path = join(spawnRunDir(env, runRef), "workers", `${safeFilename(workerRef)}.jsonl`)
  await mkdir(dirname(path), { recursive: true })
  const previous = existsSync(path) ? await readFile(path, "utf8") : ""
  await writeFile(path, `${previous}${JSON.stringify(event)}\n`)
}

async function writeCancelRequest(
  env: Record<string, string | undefined>,
  runRef: string,
  workerRef?: string,
): Promise<void> {
  const path = cancelPath(env, runRef, workerRef)
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, `${JSON.stringify({
    cancelledAt: new Date().toISOString(),
    runRef,
    ...(workerRef === undefined ? {} : { workerRef }),
  }, null, 2)}\n`)
}

async function spawnCancelled(
  env: Record<string, string | undefined>,
  runRef: string,
  workerRef?: string,
): Promise<boolean> {
  if (existsSync(cancelPath(env, runRef))) return true
  if (workerRef !== undefined && existsSync(cancelPath(env, runRef, workerRef))) return true
  return false
}

function cancelPath(env: Record<string, string | undefined>, runRef: string, workerRef?: string): string {
  return workerRef === undefined
    ? join(spawnRunDir(env, runRef), "cancel.json")
    : join(spawnRunDir(env, runRef), "workers", `${safeFilename(workerRef)}.cancel.json`)
}

function workerObjective(objective: string, slotIndex: number, count: number): string {
  return [
    `Worker ${slotIndex}/${count}.`,
    objective,
    "Work independently and return a concise summary, findings or changes, evidence, blockers, and suggested next step.",
  ].join(" ")
}

function workerPrompt(run: KhalaSpawnRun, worker: KhalaSpawnWorker): string {
  return [
    `You are Khala child worker ${worker.slotIndex}/${run.requestedCount}.`,
    `You are part of spawn run ${run.runRef}.`,
    "Work independently on this objective:",
    worker.objective,
    "",
    "Return:",
    "1. Summary",
    "2. Findings or changes",
    "3. Evidence",
    "4. Blockers",
    "5. Suggested next step",
    "",
    "Do not coordinate through hidden shared state. Do not claim another worker's result.",
    "Keep private data, tokens, local auth paths, raw prompts, and raw shell output out of public summaries.",
  ].join("\n")
}

function finalRunState(workers: readonly KhalaSpawnWorker[]): KhalaSpawnState {
  if (workers.some(worker => worker.state === "running" || worker.state === "starting" || worker.state === "queued")) {
    return "running"
  }
  if (workers.length > 0 && workers.every(worker => worker.state === "cancelled")) return "cancelled"
  if (workers.some(worker => worker.state === "accepted")) return "completed"
  return "failed"
}

function acceptedWorkerCount(run: KhalaSpawnRun): number {
  return run.workers.filter(worker => worker.state === "accepted").length
}

function updateRun(run: KhalaSpawnRun, patch: Partial<KhalaSpawnRun>): KhalaSpawnRun {
  return { ...run, ...patch }
}

function updateWorker(worker: KhalaSpawnWorker, patch: Partial<KhalaSpawnWorker>): KhalaSpawnWorker {
  return { ...worker, ...patch }
}

function decodeRun(value: unknown): KhalaSpawnRun {
  const run = value as KhalaSpawnRun
  if (run?.schema !== KHALA_SPAWN_RUN_SCHEMA || typeof run.runRef !== "string" || !Array.isArray(run.workers)) {
    throw new Error("Invalid Khala spawn run record")
  }
  return run
}

function stableRef(prefix: string, value: string, length = 24): string {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, length)}`
}

function safeFilename(value: string): string {
  return value.replace(/[^a-zA-Z0-9._-]+/g, "_")
}

async function emit(options: KhalaSpawnRunOptions, event: KhalaSpawnLifecycleEvent): Promise<void> {
  await options.onEvent?.(event)
}
