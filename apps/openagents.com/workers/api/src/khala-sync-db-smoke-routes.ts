// Khala Sync Hyperdrive connectivity smoke (KS-0.2, #8284).
//
// Admin-bearer-gated internal route proving a round-trip parameterized query
// from the deployed Worker to the Khala Sync Cloud SQL Postgres instance
// THROUGH the `KHALA_SYNC_DB` Hyperdrive binding (`docs/khala-sync/SPEC.md`
// §4). It runs exactly two bounded single-statement queries via postgres.js:
//
//   1. `SELECT 1 AS ok` — basic round trip.
//   2. `SELECT count(*) FROM pg_catalog.pg_tables WHERE tablename LIKE $1`
//      with `khala_sync_%` — proves parameterized queries work and reports
//      how many Khala Sync tables the connected database currently has.
//
// TRANSACTION-MODE SAFE (SPEC §4 constraint): Hyperdrive pools in transaction
// mode, so this route uses single statements only — no LISTEN/NOTIFY, no
// session `PREPARE`, no advisory locks, no SET/session state. The postgres.js
// client is created with `prepare: false` (unnamed statements only) and
// `max: 1`, and is ALWAYS ended (`sql.end()`) in a finally block so a smoke
// never leaks a pooled connection.
//
// AUTH: same admin bearer mechanism as the other operator smoke routes
// (`requireAdminApiToken`, injected as `requireOperator` — mirrors
// `handlePylonFabricSmoke`). Not part of the public OpenAPI surface.
//
// PUBLIC SAFETY: responses never include the connection string, host, IP,
// port, user, password, or database name. Failure reasons are redacted
// (IPv4/IPv6-looking substrings and URL credentials stripped) and bounded.
//
// The real postgres.js client is dynamically imported ONLY when no
// `makeSqlClient` is injected (i.e. inside a deployed Worker with
// `nodejs_compat`); unit tests inject a fake client so CI never needs a
// database or the driver's Node compat surface.

import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { currentEpochMillis } from './runtime-primitives'

type HttpResponse = globalThis.Response

export const KHALA_SYNC_DB_SMOKE_PATH = '/api/internal/khala-sync/db-smoke'
export const KHALA_SYNC_DB_SMOKE_ROUTE_REF =
  'route.internal.khala_sync.db_smoke.v0_1'

/** The exact statements this smoke is allowed to run (bounded, read-only). */
export const KHALA_SYNC_DB_SMOKE_SELECT_ONE = 'SELECT 1 AS ok'
export const KHALA_SYNC_DB_SMOKE_TABLE_COUNT =
  'SELECT count(*)::int AS khala_sync_tables FROM pg_catalog.pg_tables WHERE tablename LIKE $1'
export const KHALA_SYNC_TABLE_PREFIX_PATTERN = 'khala_sync_%'

/** Minimal single-statement query surface this smoke drives. */
export type KhalaSyncSmokeSqlClient = Readonly<{
  /** Run ONE parameterized statement; returns the result rows. */
  query: (
    text: string,
    params: ReadonlyArray<string>,
  ) => Promise<ReadonlyArray<Record<string, unknown>>>
  /** Release the underlying connection(s). Always called, even on error. */
  end: () => Promise<void>
}>

export type MakeKhalaSyncSmokeSqlClient = (
  connectionString: string,
) => Promise<KhalaSyncSmokeSqlClient>

/** The Hyperdrive binding slice this route reads (`env.KHALA_SYNC_DB`). */
export type KhalaSyncHyperdriveBinding = Readonly<{
  connectionString: string
}>

export type KhalaSyncDbSmokeDependencies = Readonly<{
  /** Same admin bearer predicate the other operator smokes use. */
  requireOperator: () => Promise<boolean>
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  /**
   * Injectable client factory. Default: dynamic import of `postgres`
   * (postgres.js), Worker-runtime only. Tests inject a fake — no network,
   * no database.
   */
  makeSqlClient?: MakeKhalaSyncSmokeSqlClient | undefined
  nowMs?: (() => number) | undefined
}>

/**
 * Redact anything that could leak connection details from an error message:
 * URL-embedded credentials, IPv4/IPv6-looking substrings, and host:port
 * pairs. Bounded to keep failure payloads small.
 */
export const redactConnectionDetails = (message: string): string =>
  message
    .replace(/postgres(ql)?:\/\/[^\s'"]+/gi, '[redacted-dsn]')
    .replace(/\b\d{1,3}(\.\d{1,3}){3}(:\d+)?\b/g, '[redacted-address]')
    .replace(/\b(?:[0-9a-f]{0,4}:){2,7}[0-9a-f]{0,4}\b/gi, '[redacted-address]')
    .slice(0, 300)

const defaultMakeSqlClient: MakeKhalaSyncSmokeSqlClient = async (
  connectionString,
) => {
  const mod = (await import('postgres')) as unknown as {
    default: (
      connectionString: string,
      options: Record<string, unknown>,
    ) => {
      unsafe: (
        text: string,
        params: ReadonlyArray<string>,
      ) => Promise<ReadonlyArray<Record<string, unknown>>>
      end: (options?: { timeout?: number }) => Promise<void>
    }
  }

  // Transaction-mode-safe client: one connection, unnamed statements only
  // (`prepare: false`), no session state. `fetch_types: false` skips the
  // connect-time type-fetch query — this smoke only reads ints.
  const sql = mod.default(connectionString, {
    connect_timeout: 10,
    fetch_types: false,
    max: 1,
    prepare: false,
  })

  return {
    end: () => sql.end({ timeout: 5 }),
    query: (text, params) => sql.unsafe(text, params),
  }
}

const readCount = (
  rows: ReadonlyArray<Record<string, unknown>>,
  column: string,
): number => {
  const raw = rows[0]?.[column]
  const value =
    typeof raw === 'number'
      ? raw
      : typeof raw === 'string'
        ? Number.parseInt(raw, 10)
        : typeof raw === 'bigint'
          ? Number(raw)
          : Number.NaN
  return Number.isFinite(value) ? value : Number.NaN
}

/**
 * `GET /api/internal/khala-sync/db-smoke` — admin bearer only.
 *
 * Success: `{ ok: true, khalaSyncTables, latencyMs, routeRef }`.
 * Binding absent: honest `{ ok: false, reason }` (HTTP 200) so the operator
 * sees the enablement gap instead of an opaque 500 (mirrors the
 * cf-browser-smoke convention). Query/connect failures: `{ ok: false,
 * error, reason }` with HTTP 503 and redacted reason.
 */
export const handleKhalaSyncDbSmoke = (
  request: Request,
  deps: KhalaSyncDbSmokeDependencies,
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
          'Hyperdrive binding (env.KHALA_SYNC_DB) is absent. Add the ' +
          '`hyperdrive` binding to wrangler.jsonc and deploy; the binding ' +
          'only exists inside a deployed Worker.',
        routeRef: KHALA_SYNC_DB_SMOKE_ROUTE_REF,
      })
    }

    const nowMs = deps.nowMs ?? currentEpochMillis
    const makeSqlClient = deps.makeSqlClient ?? defaultMakeSqlClient

    let sql: KhalaSyncSmokeSqlClient | undefined
    const startedAt = nowMs()
    try {
      sql = await makeSqlClient(deps.binding.connectionString)

      const okRows = await sql.query(KHALA_SYNC_DB_SMOKE_SELECT_ONE, [])
      const okValue = readCount(okRows, 'ok')
      if (okValue !== 1) {
        return noStoreJsonResponse(
          {
            error: 'khala_sync_db_smoke_failed',
            ok: false,
            reason: 'SELECT 1 round trip returned an unexpected result.',
            routeRef: KHALA_SYNC_DB_SMOKE_ROUTE_REF,
          },
          { status: 503 },
        )
      }

      const tableRows = await sql.query(KHALA_SYNC_DB_SMOKE_TABLE_COUNT, [
        KHALA_SYNC_TABLE_PREFIX_PATTERN,
      ])
      const khalaSyncTables = readCount(tableRows, 'khala_sync_tables')
      if (Number.isNaN(khalaSyncTables)) {
        return noStoreJsonResponse(
          {
            error: 'khala_sync_db_smoke_failed',
            ok: false,
            reason: 'khala_sync_% table count returned an unexpected shape.',
            routeRef: KHALA_SYNC_DB_SMOKE_ROUTE_REF,
          },
          { status: 503 },
        )
      }

      return noStoreJsonResponse({
        khalaSyncTables,
        latencyMs: Math.max(0, nowMs() - startedAt),
        ok: true,
        routeRef: KHALA_SYNC_DB_SMOKE_ROUTE_REF,
      })
    } catch (error) {
      return noStoreJsonResponse(
        {
          error: 'khala_sync_db_smoke_failed',
          ok: false,
          reason: redactConnectionDetails(
            error instanceof Error ? error.message : String(error),
          ),
          routeRef: KHALA_SYNC_DB_SMOKE_ROUTE_REF,
        },
        { status: 503 },
      )
    } finally {
      if (sql !== undefined) {
        try {
          await sql.end()
        } catch {
          // best-effort teardown: never mask the real result with a close
          // error; the `max: 1` client is dropped with the isolate anyway.
        }
      }
    }
  })
