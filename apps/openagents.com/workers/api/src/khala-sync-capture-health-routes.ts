// Khala Sync capture liveness probe + alerting signal (#8556).
//
// Background: capture ran as an unsupervised daemon; a silent stall killed
// all realtime for ~32h with a 20k backlog and nothing paged. #8554 moved
// capture onto the always-on `khala-capture` Cloud Run service
// (`--min-instances 1 --max-instances 1 --no-cpu-throttling`), so a *crash*
// now auto-restarts. This module is the missing MONITORING layer: it detects
// a *stuck-but-running* or degraded capture (process alive, checkpoints not
// advancing while a backlog exists) and surfaces it loudly so an alert pages.
//
// THE LIVENESS SIGNAL (docs/khala-sync/RUNBOOK.md "Capture daemon operation"):
// capture advances `khala_sync_capture_checkpoints.updated_at` ONLY after the
// hub 2xx-acknowledges a batch. So the true stall signal is BOTH of:
//   - there is undelivered work: SUM(scopes.last_version - checkpoint) > 0
//   - checkpoints are not advancing: now() - max(updated_at) > threshold
// A quiet system (no writes, no backlog) is intentionally NOT an alert even
// though `updated_at` is old — there is simply nothing to push. A healthy
// busy system keeps `updated_at` fresh (seconds), so it is not stale either.
// Only "backlog present AND not draining" fires.
//
// TWO CONSUMERS OF THE SAME EVALUATION:
//   1. `handleKhalaSyncCaptureHealth` — `GET /api/internal/khala-sync/
//      capture-health` (admin bearer). Typed snapshot for operators / an
//      uptime probe. Never leaks connection details.
//   2. `runKhalaSyncCaptureStalenessProbe` — called from the Worker
//      `scheduled()` per-minute task table. On a stale verdict it emits ONE
//      single-line structured Cloud Logging entry
//      (`jsonPayload.event="khala_sync_capture_stale"`, severity WARNING); a
//      log-based metric counts it and a Cloud Monitoring alert policy pages
//      the owner. Healthy / idle ticks are silent (no metric samples).
//
// TRANSACTION-MODE SAFE (SPEC §4): one bounded single-statement read via the
// `KHALA_SYNC_DB` Hyperdrive/direct-URL binding, reusing the db-smoke client
// factory (`prepare:false`, `max:1`, always `end()`ed). No LISTEN/NOTIFY, no
// session state. `now()` is read from Postgres in the SAME statement so the
// staleness delta is immune to monolith<->DB clock skew.

import { Effect } from 'effect'

import {
  type KhalaSyncSmokeSqlClient,
  type MakeKhalaSyncSmokeSqlClient,
  type KhalaSyncHyperdriveBinding,
  defaultMakeSqlClient,
  redactConnectionDetails,
} from './khala-sync-db-smoke-routes'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

type HttpResponse = globalThis.Response

export const KHALA_SYNC_CAPTURE_HEALTH_PATH =
  '/api/internal/khala-sync/capture-health'
export const KHALA_SYNC_CAPTURE_HEALTH_ROUTE_REF =
  'route.internal.khala_sync.capture_health.v0_1'

/**
 * The structured event emitted (single line) when the probe finds a stall.
 * This exact string is the log-based-metric filter key
 * (`jsonPayload.event="khala_sync_capture_stale"`).
 */
export const KHALA_SYNC_CAPTURE_STALE_EVENT = 'khala_sync_capture_stale'

/**
 * Staleness threshold: capture normally advances `updated_at` every few
 * seconds (poll interval default 5s). 120s = ~24 missed poll cycles WHILE a
 * backlog exists — comfortably past transient blips, far below the 32h
 * incident. Overridable per-call for the route/tests.
 */
export const KHALA_SYNC_CAPTURE_HEALTH_DEFAULT_THRESHOLD_MS = 120_000

/**
 * Single bounded read. Joins every scope to its checkpoint (LEFT JOIN — an
 * absent checkpoint behaves as version 0, the KS-2.2 scope-start watermark)
 * and returns, in one row:
 *   - `db_now_epoch`            — Postgres now(), seconds (skew-free delta)
 *   - `max_updated_at_epoch`    — freshest checkpoint push, seconds (NULL when
 *                                 NO checkpoint has ever been written)
 *   - `versions_undelivered`    — total backlog across scopes (>= 0)
 *   - `scopes_behind`           — count of scopes with lag > 0
 *   - `checkpoint_count`        — checkpoint rows present
 */
export const KHALA_SYNC_CAPTURE_HEALTH_QUERY = `
SELECT
  EXTRACT(EPOCH FROM now())::double precision AS db_now_epoch,
  EXTRACT(EPOCH FROM max(c.updated_at))::double precision AS max_updated_at_epoch,
  COALESCE(
    SUM(GREATEST(s.last_version - COALESCE(c.pushed_through_version, 0), 0)),
    0
  )::bigint AS versions_undelivered,
  COALESCE(
    SUM(CASE WHEN s.last_version > COALESCE(c.pushed_through_version, 0) THEN 1 ELSE 0 END),
    0
  )::bigint AS scopes_behind,
  COUNT(c.scope)::bigint AS checkpoint_count
FROM khala_sync_scopes s
LEFT JOIN khala_sync_capture_checkpoints c USING (scope)
`.trim()

export type CaptureHealthStatus = 'healthy' | 'stale'

/** Pure inputs to the threshold decision — no DB, no clock, fully testable. */
export type CaptureHealthInput = Readonly<{
  /** Reference "now" in epoch ms (Postgres now() in the live path). */
  nowMs: number
  /** Freshest checkpoint `updated_at` in epoch ms; NULL = no checkpoints. */
  maxUpdatedAtMs: number | null
  /** Total undelivered versions across scopes (>= 0). */
  versionsUndelivered: number
  /** Scopes with lag > 0. */
  scopesBehind: number
  /** Checkpoint rows present. */
  checkpointCount: number
  /** Staleness threshold in ms. */
  thresholdMs: number
}>

export type CaptureHealthSnapshot = Readonly<{
  status: CaptureHealthStatus
  /** now - max(updated_at), clamped >= 0; NULL when no checkpoint exists. */
  stalenessMs: number | null
  versionsUndelivered: number
  scopesBehind: number
  checkpointCount: number
  thresholdMs: number
}>

/**
 * The typed threshold decision. A stall requires BOTH a backlog AND
 * non-advancing checkpoints:
 *
 *   - No backlog (`versionsUndelivered === 0`)  -> healthy, ALWAYS. Quiet on
 *     an idle system even if `updated_at` is ancient.
 *   - Backlog present + NO checkpoint at all    -> stale (capture has never
 *     pushed while work is waiting).
 *   - Backlog present + staleness > threshold   -> stale.
 *   - Backlog present + staleness <= threshold  -> healthy (actively draining).
 */
export const evaluateCaptureHealth = (
  input: CaptureHealthInput,
): CaptureHealthSnapshot => {
  const stalenessMs =
    input.maxUpdatedAtMs === null
      ? null
      : Math.max(0, input.nowMs - input.maxUpdatedAtMs)

  const hasBacklog = input.versionsUndelivered > 0
  const notAdvancing =
    stalenessMs === null || stalenessMs > input.thresholdMs

  const status: CaptureHealthStatus =
    hasBacklog && notAdvancing ? 'stale' : 'healthy'

  return {
    checkpointCount: input.checkpointCount,
    scopesBehind: input.scopesBehind,
    stalenessMs,
    status,
    thresholdMs: input.thresholdMs,
    versionsUndelivered: input.versionsUndelivered,
  }
}

const readNumber = (
  rows: ReadonlyArray<Record<string, unknown>>,
  column: string,
): number | null => {
  const raw = rows[0]?.[column]
  if (raw === null || raw === undefined) {
    return null
  }
  const value =
    typeof raw === 'number'
      ? raw
      : typeof raw === 'bigint'
        ? Number(raw)
        : typeof raw === 'string'
          ? Number(raw)
          : Number.NaN
  return Number.isFinite(value) ? value : null
}

/**
 * Run the query and fold rows into the pure evaluation. Separated so the DB
 * shape parsing is covered once and the caller only handles the client
 * lifecycle. Throws on a malformed result (caller surfaces it as 503 / a
 * probe error) rather than silently reporting "healthy".
 */
export const captureHealthFromRows = (
  rows: ReadonlyArray<Record<string, unknown>>,
  thresholdMs: number,
): CaptureHealthSnapshot => {
  const dbNowEpoch = readNumber(rows, 'db_now_epoch')
  if (dbNowEpoch === null) {
    throw new Error('capture-health query returned no db_now_epoch')
  }
  const maxUpdatedEpoch = readNumber(rows, 'max_updated_at_epoch')
  const versionsUndelivered = readNumber(rows, 'versions_undelivered') ?? 0
  const scopesBehind = readNumber(rows, 'scopes_behind') ?? 0
  const checkpointCount = readNumber(rows, 'checkpoint_count') ?? 0

  return evaluateCaptureHealth({
    checkpointCount,
    maxUpdatedAtMs: maxUpdatedEpoch === null ? null : maxUpdatedEpoch * 1000,
    nowMs: dbNowEpoch * 1000,
    scopesBehind,
    thresholdMs,
    versionsUndelivered,
  })
}

export type KhalaSyncCaptureHealthDependencies = Readonly<{
  /** Same admin bearer predicate the other operator smokes use. */
  requireOperator: () => Promise<boolean>
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  /** Injectable client factory (default: db-smoke's postgres.js factory). */
  makeSqlClient?: MakeKhalaSyncSmokeSqlClient | undefined
  /** Staleness threshold override (default 120s). */
  thresholdMs?: number | undefined
}>

/**
 * `GET /api/internal/khala-sync/capture-health` — admin bearer only.
 *
 * Success: `{ ok: true, status, stalenessMs, versionsUndelivered,
 * scopesBehind, checkpointCount, thresholdMs, routeRef }`. `ok` reflects that
 * the PROBE ran — `status` carries the health verdict ('healthy' | 'stale').
 * Binding absent: honest `{ ok:false, reason }` (HTTP 200). Query failure:
 * `{ ok:false, error, reason }` HTTP 503 with a redacted reason.
 */
export const handleKhalaSyncCaptureHealth = (
  request: Request,
  deps: KhalaSyncCaptureHealthDependencies,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    if (!(await deps.requireOperator())) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    if (
      deps.binding === undefined ||
      typeof deps.binding.connectionString !== 'string' ||
      deps.binding.connectionString.length === 0
    ) {
      return noStoreJsonResponse({
        ok: false,
        reason:
          'KHALA_SYNC_DB binding is absent (KHALA_SYNC_DATABASE_URL unset). ' +
          'Deploy with the direct Cloud SQL URL to enable the capture probe.',
        routeRef: KHALA_SYNC_CAPTURE_HEALTH_ROUTE_REF,
      })
    }

    const thresholdMs =
      deps.thresholdMs ?? KHALA_SYNC_CAPTURE_HEALTH_DEFAULT_THRESHOLD_MS

    let sql: KhalaSyncSmokeSqlClient | undefined
    try {
      const makeSqlClient = deps.makeSqlClient ?? defaultMakeSqlClient
      sql = await makeSqlClient(deps.binding.connectionString)
      const rows = await sql.query(KHALA_SYNC_CAPTURE_HEALTH_QUERY, [])
      const snapshot = captureHealthFromRows(rows, thresholdMs)
      return noStoreJsonResponse({
        ok: true,
        routeRef: KHALA_SYNC_CAPTURE_HEALTH_ROUTE_REF,
        ...snapshot,
      })
    } catch (error) {
      return noStoreJsonResponse(
        {
          error: 'khala_sync_capture_health_failed',
          ok: false,
          reason: redactConnectionDetails(
            error instanceof Error ? error.message : String(error),
          ),
          routeRef: KHALA_SYNC_CAPTURE_HEALTH_ROUTE_REF,
        },
        { status: 503 },
      )
    } finally {
      if (sql !== undefined) {
        try {
          await sql.end()
        } catch {
          // best-effort teardown; never mask the real result.
        }
      }
    }
  })

export type CaptureStalenessProbeDependencies = Readonly<{
  binding: KhalaSyncHyperdriveBinding | undefined
  makeSqlClient?: MakeKhalaSyncSmokeSqlClient | undefined
  thresholdMs?: number | undefined
  /**
   * Emit ONE single-line structured log. Default writes JSON to stdout so
   * Cloud Run parses it into `jsonPayload` with a recognized `severity`
   * field — the exact signal a log-based metric + alert policy consume.
   */
  emitStructuredLog?: ((line: Record<string, unknown>) => void) | undefined
  /** Emit a warning when the probe itself cannot run (query/connect error). */
  emitProbeError?: ((message: string) => void) | undefined
}>

const defaultEmitStructuredLog = (line: Record<string, unknown>): void => {
  // Single line so Cloud Logging captures it as ONE entry with jsonPayload.
  // eslint-disable-next-line no-console
  console.log(JSON.stringify(line))
}

/**
 * Per-minute scheduled probe. Queries capture liveness and, on a `stale`
 * verdict, emits the single structured `khala_sync_capture_stale` warning
 * that the log-based metric + alert policy page on. Healthy / idle ticks are
 * silent. Fail-soft: a probe error is logged (so a broken probe is itself
 * visible) but never throws into the shared `scheduled()` task batch.
 * Returns the snapshot (or `null` when it could not run) for callers/tests.
 */
export const runKhalaSyncCaptureStalenessProbe = async (
  deps: CaptureStalenessProbeDependencies,
): Promise<CaptureHealthSnapshot | null> => {
  const emitStructuredLog = deps.emitStructuredLog ?? defaultEmitStructuredLog
  const emitProbeError =
    deps.emitProbeError ??
    ((message: string) =>
      defaultEmitStructuredLog({
        event: 'khala_sync_capture_probe_error',
        message,
        severity: 'WARNING',
      }))

  if (
    deps.binding === undefined ||
    typeof deps.binding.connectionString !== 'string' ||
    deps.binding.connectionString.length === 0
  ) {
    // No binding configured (e.g. local/tests): nothing to probe, stay silent.
    return null
  }

  const thresholdMs =
    deps.thresholdMs ?? KHALA_SYNC_CAPTURE_HEALTH_DEFAULT_THRESHOLD_MS

  let sql: KhalaSyncSmokeSqlClient | undefined
  try {
    const makeSqlClient = deps.makeSqlClient ?? defaultMakeSqlClient
    sql = await makeSqlClient(deps.binding.connectionString)
    const rows = await sql.query(KHALA_SYNC_CAPTURE_HEALTH_QUERY, [])
    const snapshot = captureHealthFromRows(rows, thresholdMs)

    if (snapshot.status === 'stale') {
      emitStructuredLog({
        checkpointCount: snapshot.checkpointCount,
        event: KHALA_SYNC_CAPTURE_STALE_EVENT,
        message:
          'khala-capture appears stalled: undelivered backlog is not draining',
        scopesBehind: snapshot.scopesBehind,
        severity: 'WARNING',
        stalenessMs: snapshot.stalenessMs,
        thresholdMs: snapshot.thresholdMs,
        versionsUndelivered: snapshot.versionsUndelivered,
      })
    }
    return snapshot
  } catch (error) {
    emitProbeError(
      redactConnectionDetails(
        error instanceof Error ? error.message : String(error),
      ),
    )
    return null
  } finally {
    if (sql !== undefined) {
      try {
        await sql.end()
      } catch {
        // best-effort teardown.
      }
    }
  }
}
