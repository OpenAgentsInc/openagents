import {
  canonicalJson,
  ClientGroupId,
  ClientId,
  EntityId,
  EntityType,
  KHALA_SYNC_PROTOCOL_VERSION,
  MutationEnvelope,
  MutationId,
  MutationResult,
  MutatorName,
  personalScope,
  PushRequest,
  SyncSchemaVersion,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { KhalaSyncStorageError } from "./errors.js"
import {
  defineMutator,
  executePush,
  makeMutatorRegistry,
  type MutatorRegistry,
} from "./push-engine.js"
import { bootstrap, logPage } from "./read-service.js"
import type { SyncSql } from "./sql.js"

/**
 * Khala Sync fleet-burst load harness (KS-9.1, #8310).
 *
 * Reproduces the June 28-29 failure shape (docs/fable/
 * 2026-07-04-database-alternatives-and-postgres-sync-engine.md §1.3:
 * ~20 concurrent workers, assignment-accept bursts, per-turn ingest of
 * ~10-11 statements, plus public-counter reads) against the Khala Sync
 * substrate — at a configurable multiple of that shape.
 *
 * Two modes:
 *
 * - `substrate`: runs `executePush` / `logPage` / `bootstrap` in-process
 *   against a Postgres URL (local throwaway server or the staging Cloud SQL
 *   database over TLS). This exercises the exact engine transaction shape
 *   the Worker runs, minus the HTTP/Hyperdrive hop.
 * - `http`: drives the deployed Worker routes (`POST /api/sync/push`,
 *   `GET /api/sync/log`) with a real agent bearer token — the full
 *   route + Hyperdrive path.
 *
 * Every writer is one client group pushing `sync.debugEcho`-shaped batches
 * into its own personal scope (`scope.user.loadtest.<runId>.wN` in
 * substrate mode; the token user's own personal scope in http mode).
 * Readers run concurrent log catch-up + periodic bootstrap reads, plus
 * public-counter-style single-row reads (the KS-6.3/#8304 projection scope
 * `scope.public.tokens-served` when it exists, otherwise a single-row
 * scope-counter read).
 *
 * This is a LOAD TOOL: wall-clock (`Date.now`/`performance.now`) is used
 * deliberately for latency measurement — none of it feeds production logic.
 */

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

export type LoadTestMode = "substrate" | "http"

export interface LoadTestConfig {
  readonly mode: LoadTestMode
  /** Direct Postgres URL (substrate mode; also enables cleanup). */
  readonly databaseUrl: string | undefined
  /** Worker origin, e.g. https://openagents-staging.openagents.workers.dev */
  readonly baseUrl: string | undefined
  /** Agent bearer token (http mode). Never printed. */
  readonly token: string | undefined
  /** TLS posture for the direct Postgres connection. */
  readonly ssl: "require" | "verify" | "disable"
  /** Concurrent writer clients (each = one client group). */
  readonly workers: number
  /** Target pushes per second PER WORKER (closed loop, paced). */
  readonly pushesPerSecond: number
  /** Mutation envelopes per push batch. */
  readonly batchSize: number
  /** Concurrent readers (log catch-up + bootstrap + counter reads). */
  readonly readers: number
  /** Reader poll interval in ms. */
  readonly readIntervalMs: number
  /** Run duration in seconds. */
  readonly durationSec: number
  /** Max pooled Postgres connections (substrate mode). */
  readonly pool: number
  /** Namespaces all synthetic rows; used by cleanup. [A-Za-z0-9-] only. */
  readonly runId: string
  /** Delete synthetic rows after the run (needs databaseUrl). */
  readonly cleanup: boolean
  /** Write the JSON report here (in addition to stdout summary). */
  readonly jsonOut: string | undefined
}

export interface LoadTestConfigError {
  readonly error: string
}

const DEFAULTS: {
  readonly batchSize: number
  readonly durationSec: number
  readonly pool: number
  readonly pushesPerSecond: number
  readonly readIntervalMs: number
  readonly readers: number
  readonly workers: number
} = {
  batchSize: 2,
  durationSec: 300,
  pool: 16,
  pushesPerSecond: 2,
  readIntervalMs: 1_000,
  readers: 10,
  workers: 40,
}

const RUN_ID_PATTERN = /^[A-Za-z0-9-]+$/

export const defaultRunId = (now: Date = new Date()): string =>
  now
    .toISOString()
    .replace(/[:.]/g, "")
    .replace(/-/g, "")
    .slice(0, 15)

const positiveInt = (name: string, raw: string): number | LoadTestConfigError => {
  const value = Number(raw)
  if (!Number.isSafeInteger(value) || value < 1) {
    return { error: `${name} must be a positive integer, got ${raw}` }
  }
  return value
}

/**
 * Parse CLI flags (+ env fallbacks) into a config. Flags accept both
 * `--flag=value` and `--flag value`. Returns `{ error }` instead of
 * throwing so the CLI can print usage.
 *
 * Env fallbacks: `KHALA_LOAD_DATABASE_URL`, `KHALA_LOAD_BASE_URL`,
 * `KHALA_LOAD_TOKEN` (token is env-only by design — never a flag, so it
 * cannot leak into shell history or process listings).
 */
export const parseLoadTestArgs = (
  argv: ReadonlyArray<string>,
  env: Record<string, string | undefined> = {},
): LoadTestConfig | LoadTestConfigError => {
  let mode: LoadTestMode | undefined
  let databaseUrl = env["KHALA_LOAD_DATABASE_URL"]
  let baseUrl = env["KHALA_LOAD_BASE_URL"]
  const token = env["KHALA_LOAD_TOKEN"]
  let ssl: LoadTestConfig["ssl"] = "require"
  let workers = DEFAULTS.workers
  let pushesPerSecond = DEFAULTS.pushesPerSecond
  let batchSize = DEFAULTS.batchSize
  let readers = DEFAULTS.readers
  let readIntervalMs = DEFAULTS.readIntervalMs
  let durationSec = DEFAULTS.durationSec
  let pool = DEFAULTS.pool
  let runId = defaultRunId()
  let cleanup = true
  let jsonOut: string | undefined

  const flags: Array<{ name: string; value: string }> = []
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]
    if (arg === undefined) continue
    if (!arg.startsWith("--")) {
      return { error: `unexpected argument ${JSON.stringify(arg)}` }
    }
    const eq = arg.indexOf("=")
    if (eq !== -1) {
      flags.push({ name: arg.slice(0, eq), value: arg.slice(eq + 1) })
      continue
    }
    if (arg === "--no-cleanup" || arg === "--cleanup") {
      flags.push({ name: arg, value: "" })
      continue
    }
    const next = argv[++i]
    if (next === undefined) {
      return { error: `${arg} requires a value` }
    }
    flags.push({ name: arg, value: next })
  }

  for (const { name, value } of flags) {
    switch (name) {
      case "--mode": {
        if (value !== "substrate" && value !== "http") {
          return { error: `--mode must be substrate or http, got ${value}` }
        }
        mode = value
        break
      }
      case "--database-url":
        databaseUrl = value
        break
      case "--base-url":
        baseUrl = value
        break
      case "--ssl": {
        if (value !== "require" && value !== "verify" && value !== "disable") {
          return { error: `--ssl must be require, verify, or disable` }
        }
        ssl = value
        break
      }
      case "--workers": {
        const parsed = positiveInt("--workers", value)
        if (typeof parsed !== "number") return parsed
        workers = parsed
        break
      }
      case "--pushes-per-second": {
        const parsed = Number(value)
        if (!Number.isFinite(parsed) || parsed <= 0) {
          return { error: `--pushes-per-second must be a positive number` }
        }
        pushesPerSecond = parsed
        break
      }
      case "--batch": {
        const parsed = positiveInt("--batch", value)
        if (typeof parsed !== "number") return parsed
        batchSize = parsed
        break
      }
      case "--readers": {
        const parsed = Number(value)
        if (!Number.isSafeInteger(parsed) || parsed < 0) {
          return { error: `--readers must be a non-negative integer` }
        }
        readers = parsed
        break
      }
      case "--read-interval-ms": {
        const parsed = positiveInt("--read-interval-ms", value)
        if (typeof parsed !== "number") return parsed
        readIntervalMs = parsed
        break
      }
      case "--duration-sec": {
        const parsed = positiveInt("--duration-sec", value)
        if (typeof parsed !== "number") return parsed
        durationSec = parsed
        break
      }
      case "--pool": {
        const parsed = positiveInt("--pool", value)
        if (typeof parsed !== "number") return parsed
        pool = parsed
        break
      }
      case "--run-id": {
        if (!RUN_ID_PATTERN.test(value)) {
          return { error: "--run-id must match [A-Za-z0-9-]+" }
        }
        runId = value
        break
      }
      case "--cleanup":
        cleanup = true
        break
      case "--no-cleanup":
        cleanup = false
        break
      case "--json-out":
        jsonOut = value
        break
      default:
        return { error: `unknown flag ${name}` }
    }
  }

  if (mode === undefined) {
    return { error: "--mode is required (substrate | http)" }
  }
  if (mode === "substrate" && (databaseUrl === undefined || databaseUrl === "")) {
    return {
      error:
        "substrate mode needs --database-url (or KHALA_LOAD_DATABASE_URL)",
    }
  }
  if (mode === "http") {
    if (baseUrl === undefined || baseUrl === "") {
      return { error: "http mode needs --base-url (or KHALA_LOAD_BASE_URL)" }
    }
    if (token === undefined || token === "") {
      return { error: "http mode needs KHALA_LOAD_TOKEN (env only, never a flag)" }
    }
  }

  return {
    baseUrl,
    batchSize,
    cleanup,
    databaseUrl,
    durationSec,
    jsonOut,
    mode,
    pool,
    pushesPerSecond,
    readIntervalMs,
    readers,
    runId,
    ssl,
    token,
    workers,
  }
}

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

/** `sorted` ascending; p in [0,100]. Nearest-rank percentile. */
export const percentile = (
  sorted: ReadonlyArray<number>,
  p: number,
): number => {
  if (sorted.length === 0) return 0
  const rank = Math.ceil((p / 100) * sorted.length)
  const index = Math.min(sorted.length - 1, Math.max(0, rank - 1))
  return sorted[index] ?? 0
}

export interface OpSummary {
  readonly count: number
  readonly errors: number
  readonly p50Ms: number
  readonly p95Ms: number
  readonly p99Ms: number
  readonly maxMs: number
  readonly meanMs: number
  /** Successful ops per second over the measured window. */
  readonly throughputPerSec: number
}

export interface LoadTestReport {
  readonly mode: LoadTestMode
  readonly runId: string
  readonly startedAtIso: string
  readonly durationSec: number
  readonly workers: number
  readonly readers: number
  readonly batchSize: number
  readonly targetPushesPerSecondPerWorker: number
  readonly ops: Record<string, OpSummary>
  /** errorKind -> count, across all op kinds. */
  readonly errorTaxonomy: Record<string, number>
  /**
   * Write→read visibility lag (ms): reader observation time minus the
   * writer's pre-push stamp for newly observed entities. Includes push
   * latency + reader poll interval by construction.
   */
  readonly deltaVisibility: OpSummary | undefined
  readonly notes: ReadonlyArray<string>
  readonly cleanup: CleanupResult | undefined
}

export class MetricsRecorder {
  private readonly latencies = new Map<string, Array<number>>()
  private readonly errorCounts = new Map<string, number>()
  private readonly opErrors = new Map<string, number>()

  record(op: string, ms: number, ok: boolean, errorKind?: string): void {
    if (ok) {
      let bucket = this.latencies.get(op)
      if (bucket === undefined) {
        bucket = []
        this.latencies.set(op, bucket)
      }
      bucket.push(ms)
      return
    }
    this.opErrors.set(op, (this.opErrors.get(op) ?? 0) + 1)
    const kind = errorKind ?? "unknown"
    this.errorCounts.set(kind, (this.errorCounts.get(kind) ?? 0) + 1)
  }

  summary(op: string, windowSec: number): OpSummary {
    const raw = this.latencies.get(op) ?? []
    const sorted = [...raw].sort((a, b) => a - b)
    const count = sorted.length
    const errors = this.opErrors.get(op) ?? 0
    const sum = sorted.reduce((a, b) => a + b, 0)
    return {
      count,
      errors,
      maxMs: count === 0 ? 0 : (sorted[count - 1] ?? 0),
      meanMs: count === 0 ? 0 : round2(sum / count),
      p50Ms: round2(percentile(sorted, 50)),
      p95Ms: round2(percentile(sorted, 95)),
      p99Ms: round2(percentile(sorted, 99)),
      throughputPerSec: windowSec <= 0 ? 0 : round2(count / windowSec),
    }
  }

  opNames(): ReadonlyArray<string> {
    return [
      ...new Set([...this.latencies.keys(), ...this.opErrors.keys()]),
    ].sort()
  }

  taxonomy(): Record<string, number> {
    return Object.fromEntries(
      [...this.errorCounts.entries()].sort((a, b) => b[1] - a[1]),
    )
  }
}

const round2 = (value: number): number => Math.round(value * 100) / 100

// ---------------------------------------------------------------------------
// The load mutator (substrate mode)
// ---------------------------------------------------------------------------

export const LOAD_ECHO_MUTATOR_NAME = "sync.debugEcho"
export const LOAD_ECHO_ENTITY_TYPE = "sync_debug_echo"

interface LoadEchoArgs {
  readonly scope: string
  readonly entityId: string
  readonly echo: string
}

const decodeLoadEchoArgs = (argsJson: string): LoadEchoArgs => {
  const raw: unknown = JSON.parse(argsJson)
  if (
    typeof raw !== "object" ||
    raw === null ||
    typeof (raw as { scope?: unknown }).scope !== "string" ||
    typeof (raw as { entityId?: unknown }).entityId !== "string" ||
    (raw as { entityId: string }).entityId.length === 0 ||
    typeof (raw as { echo?: unknown }).echo !== "string"
  ) {
    throw new Error("loadtest echo args must be { scope, entityId, echo }")
  }
  return raw as LoadEchoArgs
}

/**
 * Same contract as the Worker's `sync.debugEcho` (khala-sync-mutators.ts):
 * one changelog upsert into the CALLER'S OWN personal scope, guarded
 * before any write. Substrate mode registers this locally because the
 * Worker registry lives in the openagents.com app, not this package.
 */
export const makeLoadEchoRegistry = (): MutatorRegistry =>
  makeMutatorRegistry([
    defineMutator<LoadEchoArgs>({
      decodeArgs: decodeLoadEchoArgs,
      execute: async (args, ctx) => {
        const ownScope = personalScope(ctx.userId)
        if (args.scope !== String(ownScope)) {
          return new MutationResult({
            errorCode: "unauthorized_scope",
            errorMessageSafe:
              "loadtest echo may only write the caller's personal scope",
            mutationId: ctx.mutationId,
            status: "rejected",
          })
        }
        await ctx.writer.appendChange({
          entityId: EntityId.make(args.entityId),
          entityType: EntityType.make(LOAD_ECHO_ENTITY_TYPE),
          mutationRef: ctx.mutationRef,
          op: "upsert",
          postImage: { echo: args.echo, entityId: args.entityId, scope: args.scope },
          scope: ownScope,
        })
        return new MutationResult({
          mutationId: ctx.mutationId,
          status: "applied",
        })
      },
      name: MutatorName.make(LOAD_ECHO_MUTATOR_NAME),
    }),
  ])

// ---------------------------------------------------------------------------
// Synthetic identity naming (cleanup keys off these prefixes)
// ---------------------------------------------------------------------------

export const loadUserId = (runId: string, worker: number): string =>
  `loadtest.${runId}.w${worker}`

export const loadScope = (runId: string, worker: number): SyncScope =>
  personalScope(loadUserId(runId, worker))

export const loadClientGroupId = (runId: string, worker: number): string =>
  `cg-loadtest-${runId}-w${worker}`

export const loadScopeLikePattern = (runId: string): string =>
  `scope.user.loadtest.${runId}.%`

export const loadClientGroupLikePattern = (runId: string): string =>
  `cg-loadtest-${runId}-%`

/** The KS-6.3 (#8304) public projection scope, when it exists. */
export const PUBLIC_COUNTER_SCOPE = "scope.public.tokens-served"

// ---------------------------------------------------------------------------
// Run orchestration
// ---------------------------------------------------------------------------

const sleep = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms))

const errorKindOf = (error: unknown): string => {
  if (error instanceof KhalaSyncStorageError) return `storage_${error.reason}`
  if (error instanceof Error) {
    const name = error.constructor.name
    return name === "Error" ? (error.message.slice(0, 60) || "error") : name
  }
  return "unknown"
}

interface WriterState {
  readonly worker: number
  nextMutationId: number
  pushes: number
}

const schemaVersion = SyncSchemaVersion.make(1)

const makeEnvelopes = (
  state: WriterState,
  scope: string,
  batchSize: number,
): Array<MutationEnvelope> => {
  const envelopes: Array<MutationEnvelope> = []
  for (let i = 0; i < batchSize; i++) {
    const mutationId = state.nextMutationId++
    envelopes.push(
      new MutationEnvelope({
        argsJson: canonicalJson({
          echo: canonicalJson({ push: state.pushes, sentAtMs: Date.now() }),
          entityId: `e-${state.worker}-${mutationId % 50}`,
          scope,
        }),
        mutationId: MutationId.make(mutationId),
        name: MutatorName.make(LOAD_ECHO_MUTATOR_NAME),
      }),
    )
  }
  state.pushes++
  return envelopes
}

/** Extract writer send-stamps from freshly observed post-images. */
const visibilityLagsFromEntries = (
  entries: ReadonlyArray<{ readonly postImageJson?: string | undefined }>,
  nowMs: number,
): Array<number> => {
  const lags: Array<number> = []
  for (const entry of entries) {
    if (entry.postImageJson === undefined) continue
    try {
      const image: unknown = JSON.parse(entry.postImageJson)
      const echoRaw = (image as { echo?: unknown }).echo
      if (typeof echoRaw !== "string") continue
      const echo: unknown = JSON.parse(echoRaw)
      const sentAtMs = (echo as { sentAtMs?: unknown }).sentAtMs
      if (typeof sentAtMs === "number" && Number.isFinite(sentAtMs)) {
        lags.push(Math.max(0, nowMs - sentAtMs))
      }
    } catch {
      // Not a loadtest post-image — skip.
    }
  }
  return lags
}

export interface SubstrateRunDeps {
  /** Pooled root SQL handle (postgres.js / Bun SQL). */
  readonly sql: SyncSql
}

export const runSubstrateLoad = async (
  config: LoadTestConfig,
  deps: SubstrateRunDeps,
): Promise<LoadTestReport> => {
  const { sql } = deps
  const registry = makeLoadEchoRegistry()
  const metrics = new MetricsRecorder()
  const visibilityLags: Array<number> = []
  const notes: Array<string> = []
  const startedAt = new Date()
  const endAtMs = startedAt.getTime() + config.durationSec * 1_000
  const pushIntervalMs = 1_000 / config.pushesPerSecond

  // Probe the KS-6.3 public projection scope once, up front.
  let counterScope: string = PUBLIC_COUNTER_SCOPE
  const counterRows: Array<{ scope: string }> = await sql`
    SELECT scope FROM khala_sync_scopes WHERE scope = ${PUBLIC_COUNTER_SCOPE}
  `
  if (counterRows.length === 0) {
    counterScope = String(loadScope(config.runId, 0))
    notes.push(
      `public projection scope ${PUBLIC_COUNTER_SCOPE} absent (KS-6.3 not ` +
        `landed on this database) — counter reads use a single-row ` +
        `khala_sync_scopes read on a loadtest scope instead`,
    )
  }

  const writer = async (workerIndex: number): Promise<void> => {
    const state: WriterState = { nextMutationId: 1, pushes: 0, worker: workerIndex }
    const userId = loadUserId(config.runId, workerIndex)
    const scope = String(loadScope(config.runId, workerIndex))
    const clientGroupId = ClientGroupId.make(
      loadClientGroupId(config.runId, workerIndex),
    )
    const clientId = ClientId.make("c-1")
    // Stagger start so 40 workers do not phase-lock their pacing.
    await sleep(Math.random() * pushIntervalMs)
    while (Date.now() < endAtMs) {
      const request = new PushRequest({
        clientGroupId,
        clientId,
        mutations: makeEnvelopes(state, scope, config.batchSize),
        protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
        schemaVersion,
      })
      const t0 = performance.now()
      try {
        const response = await executePush({ registry, request, sql, userId })
        const elapsed = performance.now() - t0
        const rejected = response.results.filter((r) => r.status === "rejected")
        if (rejected.length > 0) {
          metrics.record(
            "push",
            elapsed,
            false,
            `rejected_${rejected[0]?.errorCode ?? "unknown"}`,
          )
        } else {
          metrics.record("push", elapsed, true)
        }
      } catch (error) {
        metrics.record("push", performance.now() - t0, false, errorKindOf(error))
        // Back off briefly on storage failure so a dying database is not
        // hammered in a tight loop.
        await sleep(250)
      }
      const spent = performance.now() - t0
      if (spent < pushIntervalMs) await sleep(pushIntervalMs - spent)
    }
  }

  const reader = async (readerIndex: number): Promise<void> => {
    // Round-robin scope assignment over the writer scopes.
    const scope = loadScope(config.runId, readerIndex % config.workers)
    let cursor = 0
    let tick = 0
    await sleep(Math.random() * config.readIntervalMs)
    while (Date.now() < endAtMs) {
      const tickStart = performance.now()
      // 1. Log catch-up (delta read).
      const t0 = performance.now()
      try {
        const page = await logPage(sql, { afterVersion: cursor, scope })
        metrics.record("log_read", performance.now() - t0, true)
        const nowMs = Date.now()
        visibilityLags.push(
          ...visibilityLagsFromEntries(
            page.entries.map((e) => ({
              postImageJson: e.postImageJson,
            })),
            nowMs,
          ),
        )
        cursor = Number(page.nextCursor)
      } catch (error) {
        metrics.record("log_read", performance.now() - t0, false, errorKindOf(error))
      }
      // 2. Public-counter-style single-row read every tick.
      const t1 = performance.now()
      try {
        await sql`
          SELECT last_version FROM khala_sync_scopes WHERE scope = ${counterScope}
        `
        metrics.record("counter_read", performance.now() - t1, true)
      } catch (error) {
        metrics.record(
          "counter_read",
          performance.now() - t1,
          false,
          errorKindOf(error),
        )
      }
      // 3. Bootstrap snapshot (first page) every 10th tick.
      if (tick % 10 === 0) {
        const t2 = performance.now()
        try {
          await bootstrap(sql, { scope })
          metrics.record("bootstrap", performance.now() - t2, true)
        } catch (error) {
          metrics.record(
            "bootstrap",
            performance.now() - t2,
            false,
            errorKindOf(error),
          )
        }
      }
      tick++
      const spent = performance.now() - tickStart
      if (spent < config.readIntervalMs) {
        await sleep(config.readIntervalMs - spent)
      }
    }
  }

  const tasks: Array<Promise<void>> = []
  for (let w = 0; w < config.workers; w++) tasks.push(writer(w))
  for (let r = 0; r < config.readers; r++) tasks.push(reader(r))
  await Promise.all(tasks)

  const windowSec = (Date.now() - startedAt.getTime()) / 1_000
  return buildReport(config, metrics, visibilityLags, notes, startedAt, windowSec)
}

// ---------------------------------------------------------------------------
// HTTP mode
// ---------------------------------------------------------------------------

const HTTP_TIMEOUT_MS = 30_000

const fetchJson = async (
  url: string,
  token: string,
  init: { method: string; body?: string },
): Promise<{ status: number; body: unknown }> => {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), HTTP_TIMEOUT_MS)
  try {
    const response = await fetch(url, {
      ...(init.body === undefined ? {} : { body: init.body }),
      headers: {
        authorization: `Bearer ${token}`,
        ...(init.body === undefined ? {} : { "content-type": "application/json" }),
      },
      method: init.method,
      signal: controller.signal,
    })
    let body: unknown
    try {
      body = await response.json()
    } catch {
      body = undefined
    }
    return { body, status: response.status }
  } finally {
    clearTimeout(timer)
  }
}

/**
 * One authenticated probe: GET /api/agents/me. Returns the linked user id
 * on success, or an error string when the token does not authenticate
 * against this base URL (the caller should skip http mode honestly).
 */
export const probeHttpIdentity = async (
  baseUrl: string,
  token: string,
): Promise<{ userId: string } | { error: string }> => {
  try {
    const { body, status } = await fetchJson(
      `${baseUrl}/api/agents/me`,
      token,
      { method: "GET" },
    )
    if (status !== 200) {
      return { error: `GET /api/agents/me returned ${status}` }
    }
    const userId = (
      body as { agent?: { user?: { id?: unknown } } }
    )?.agent?.user?.id
    if (typeof userId !== "string" || userId.length === 0) {
      return { error: "GET /api/agents/me returned no agent.user.id" }
    }
    return { userId }
  } catch (error) {
    return { error: `identity probe failed: ${errorKindOf(error)}` }
  }
}

export const runHttpLoad = async (
  config: LoadTestConfig,
): Promise<LoadTestReport> => {
  const baseUrl = config.baseUrl ?? ""
  const token = config.token ?? ""
  const metrics = new MetricsRecorder()
  const visibilityLags: Array<number> = []
  const notes: Array<string> = []

  const identity = await probeHttpIdentity(baseUrl, token)
  if ("error" in identity) {
    throw new Error(`http mode auth probe failed: ${identity.error}`)
  }
  // All http-mode writers act as the token user: each gets its OWN client
  // group, all writing the token user's personal scope (sync.debugEcho is
  // guarded to the caller's own scope).
  const scope = String(personalScope(identity.userId))
  notes.push(
    `http mode: ${config.workers} client groups as one agent user, ` +
      `all writing that user's personal scope`,
  )

  const startedAt = new Date()
  const endAtMs = startedAt.getTime() + config.durationSec * 1_000
  const pushIntervalMs = 1_000 / config.pushesPerSecond

  const writer = async (workerIndex: number): Promise<void> => {
    const state: WriterState = { nextMutationId: 1, pushes: 0, worker: workerIndex }
    const clientGroupId = loadClientGroupId(config.runId, workerIndex)
    await sleep(Math.random() * pushIntervalMs)
    while (Date.now() < endAtMs) {
      const body = JSON.stringify({
        clientGroupId,
        clientId: "c-1",
        mutations: makeEnvelopes(state, scope, config.batchSize).map((m) => ({
          argsJson: m.argsJson,
          mutationId: Number(m.mutationId),
          name: String(m.name),
        })),
        protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
        schemaVersion: 1,
      })
      const t0 = performance.now()
      try {
        const { body: response, status } = await fetchJson(
          `${baseUrl}/api/sync/push`,
          token,
          { body, method: "POST" },
        )
        const elapsed = performance.now() - t0
        if (status === 200) {
          const results =
            (response as { results?: Array<{ status?: string; errorCode?: string }> })
              ?.results ?? []
          const rejected = results.find((r) => r.status === "rejected")
          if (rejected !== undefined) {
            metrics.record(
              "push",
              elapsed,
              false,
              `rejected_${rejected.errorCode ?? "unknown"}`,
            )
          } else {
            metrics.record("push", elapsed, true)
          }
        } else {
          const code = (response as { code?: string })?.code ?? "no_body"
          metrics.record("push", elapsed, false, `http_${status}_${code}`)
          if (status === 503 || status === 429) await sleep(500)
        }
      } catch (error) {
        metrics.record("push", performance.now() - t0, false, errorKindOf(error))
        await sleep(250)
      }
      const spent = performance.now() - t0
      if (spent < pushIntervalMs) await sleep(pushIntervalMs - spent)
    }
  }

  const reader = async (): Promise<void> => {
    let cursor = 0
    await sleep(Math.random() * config.readIntervalMs)
    while (Date.now() < endAtMs) {
      const tickStart = performance.now()
      const t0 = performance.now()
      try {
        const { body, status } = await fetchJson(
          `${baseUrl}/api/sync/log?scope=${encodeURIComponent(scope)}&cursor=${cursor}`,
          token,
          { method: "GET" },
        )
        const elapsed = performance.now() - t0
        if (status === 200) {
          metrics.record("log_read", elapsed, true)
          const page = body as {
            nextCursor?: number
            entries?: Array<{ postImageJson?: string }>
          }
          visibilityLags.push(
            ...visibilityLagsFromEntries(page.entries ?? [], Date.now()),
          )
          if (typeof page.nextCursor === "number") cursor = page.nextCursor
        } else {
          const code = (body as { code?: string })?.code ?? "no_body"
          metrics.record("log_read", elapsed, false, `http_${status}_${code}`)
        }
      } catch (error) {
        metrics.record("log_read", performance.now() - t0, false, errorKindOf(error))
      }
      const spent = performance.now() - tickStart
      if (spent < config.readIntervalMs) {
        await sleep(config.readIntervalMs - spent)
      }
    }
  }

  const tasks: Array<Promise<void>> = []
  for (let w = 0; w < config.workers; w++) tasks.push(writer(w))
  for (let r = 0; r < config.readers; r++) tasks.push(reader())
  await Promise.all(tasks)

  const windowSec = (Date.now() - startedAt.getTime()) / 1_000
  return buildReport(config, metrics, visibilityLags, notes, startedAt, windowSec)
}

// ---------------------------------------------------------------------------
// Report assembly + formatting
// ---------------------------------------------------------------------------

const buildReport = (
  config: LoadTestConfig,
  metrics: MetricsRecorder,
  visibilityLags: ReadonlyArray<number>,
  notes: ReadonlyArray<string>,
  startedAt: Date,
  windowSec: number,
): LoadTestReport => {
  const ops: Record<string, OpSummary> = {}
  for (const op of metrics.opNames()) {
    ops[op] = metrics.summary(op, windowSec)
  }
  let deltaVisibility: OpSummary | undefined
  if (visibilityLags.length > 0) {
    const sorted = [...visibilityLags].sort((a, b) => a - b)
    deltaVisibility = {
      count: sorted.length,
      errors: 0,
      maxMs: sorted[sorted.length - 1] ?? 0,
      meanMs: round2(sorted.reduce((a, b) => a + b, 0) / sorted.length),
      p50Ms: round2(percentile(sorted, 50)),
      p95Ms: round2(percentile(sorted, 95)),
      p99Ms: round2(percentile(sorted, 99)),
      throughputPerSec: round2(sorted.length / windowSec),
    }
  }
  return {
    batchSize: config.batchSize,
    cleanup: undefined,
    deltaVisibility,
    durationSec: round2(windowSec),
    errorTaxonomy: metrics.taxonomy(),
    mode: config.mode,
    notes,
    ops,
    readers: config.readers,
    runId: config.runId,
    startedAtIso: startedAt.toISOString(),
    targetPushesPerSecondPerWorker: config.pushesPerSecond,
    workers: config.workers,
  }
}

const summaryLine = (name: string, s: OpSummary): string =>
  `  ${name.padEnd(14)} n=${String(s.count).padStart(6)} ` +
  `err=${String(s.errors).padStart(4)} ` +
  `p50=${s.p50Ms}ms p95=${s.p95Ms}ms p99=${s.p99Ms}ms ` +
  `max=${s.maxMs.toFixed(0)}ms mean=${s.meanMs}ms ` +
  `rate=${s.throughputPerSec}/s`

export const formatHumanSummary = (report: LoadTestReport): string => {
  const lines: Array<string> = [
    `khala-sync load test — mode=${report.mode} runId=${report.runId}`,
    `  started=${report.startedAtIso} duration=${report.durationSec}s ` +
      `writers=${report.workers} readers=${report.readers} ` +
      `batch=${report.batchSize} target=${report.targetPushesPerSecondPerWorker} push/s/worker`,
  ]
  for (const [name, s] of Object.entries(report.ops)) {
    lines.push(summaryLine(name, s))
  }
  if (report.deltaVisibility !== undefined) {
    lines.push(summaryLine("visibility", report.deltaVisibility))
    lines.push(
      "  (visibility = write-stamp → reader-observation; includes push latency + poll interval)",
    )
  }
  const taxonomy = Object.entries(report.errorTaxonomy)
  lines.push(
    taxonomy.length === 0
      ? "  errors: none"
      : `  errors: ${taxonomy.map(([k, v]) => `${k}=${v}`).join(" ")}`,
  )
  for (const note of report.notes) lines.push(`  note: ${note}`)
  if (report.cleanup !== undefined) {
    lines.push(
      `  cleanup: changelog=${report.cleanup.changelogRows} ` +
        `scopes=${report.cleanup.scopeRows} mutations=${report.cleanup.mutationRows} ` +
        `clientState=${report.cleanup.clientStateRows} ` +
        `captureCheckpoints=${report.cleanup.captureCheckpointRows} ` +
        `scopeOwners=${report.cleanup.scopeOwnerRows}`,
    )
  }
  return lines.join("\n")
}

// ---------------------------------------------------------------------------
// Cleanup (plain DELETE by runId prefix — documented in the report)
// ---------------------------------------------------------------------------

export interface CleanupResult {
  readonly changelogRows: number
  readonly scopeRows: number
  readonly mutationRows: number
  readonly clientStateRows: number
  readonly captureCheckpointRows: number
  readonly scopeOwnerRows: number
}

const countOf = (rows: Array<{ n: string | number | bigint }>): number =>
  Number(rows[0]?.n ?? 0)

/**
 * Delete every synthetic row this run created, keyed on the runId
 * prefixes (`scope.user.loadtest.<runId>.%` / `cg-loadtest-<runId>-%`).
 * Plain DELETEs — the volumes are bounded by the run itself and the
 * loadtest scopes are disjoint from all real scopes by construction.
 */
export const cleanupLoadTestRows = async (
  sql: SyncSql,
  runId: string,
): Promise<CleanupResult> => {
  const scopePattern = loadScopeLikePattern(runId)
  const groupPattern = loadClientGroupLikePattern(runId)

  const changelog = await sql`
    WITH deleted AS (
      DELETE FROM khala_sync_changelog WHERE scope LIKE ${scopePattern}
      RETURNING 1
    ) SELECT count(*)::bigint AS n FROM deleted
  `
  const checkpoints = await sql`
    WITH deleted AS (
      DELETE FROM khala_sync_capture_checkpoints WHERE scope LIKE ${scopePattern}
      RETURNING 1
    ) SELECT count(*)::bigint AS n FROM deleted
  `
  const owners = await sql`
    WITH deleted AS (
      DELETE FROM khala_sync_scope_owners WHERE scope LIKE ${scopePattern}
      RETURNING 1
    ) SELECT count(*)::bigint AS n FROM deleted
  `
  const scopes = await sql`
    WITH deleted AS (
      DELETE FROM khala_sync_scopes WHERE scope LIKE ${scopePattern}
      RETURNING 1
    ) SELECT count(*)::bigint AS n FROM deleted
  `
  const mutations = await sql`
    WITH deleted AS (
      DELETE FROM khala_sync_mutations WHERE client_group_id LIKE ${groupPattern}
      RETURNING 1
    ) SELECT count(*)::bigint AS n FROM deleted
  `
  const clientState = await sql`
    WITH deleted AS (
      DELETE FROM khala_sync_client_state WHERE client_group_id LIKE ${groupPattern}
      RETURNING 1
    ) SELECT count(*)::bigint AS n FROM deleted
  `

  return {
    captureCheckpointRows: countOf(checkpoints),
    changelogRows: countOf(changelog),
    clientStateRows: countOf(clientState),
    mutationRows: countOf(mutations),
    scopeOwnerRows: countOf(owners),
    scopeRows: countOf(scopes),
  }
}
