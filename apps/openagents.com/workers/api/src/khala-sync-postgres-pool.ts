/**
 * Shared postgres.js client acquisition for the Khala Sync Postgres seams.
 *
 * WHY THIS EXISTS (incident 2026-07-08): every driver seam in this Worker used
 * to construct a FRESH `postgres(connectionString, { max: 1 })` client per D1
 * operation / per request and `end()` it immediately after. Under Cloudflare
 * Workers + Hyperdrive that is correct — each invocation is isolated and
 * Hyperdrive pools upstream, and a Worker isolate cannot reuse a socket across
 * request contexts. But on the **Cloud Run monolith** (a long-lived Bun
 * process, `--concurrency 80`, up to 4 instances) that pattern opens a brand
 * new raw connection to Cloud SQL for every single statement. At ~80 concurrent
 * DB-heavy requests it blew past Cloud Run's hard limit of **100 connections
 * per instance** to `khala-sync-pg`, surfacing as
 * `Exceeded maximum of 100 connections per instance` and intermittent 500/503
 * on DB-heavy routes (pylon heartbeat, forum, credits, …).
 *
 * THE FIX: on the long-lived server runtime, construct ONE pool-backed
 * postgres.js client per `(connectionString, variant)` at first use and REUSE
 * it across every request and statement. postgres.js's `max` option makes the
 * client pool-backed natively; `.end()` on the returned handle is a no-op so
 * the callers' existing per-operation teardown does not tear down the shared
 * pool. Idle connections are released by `idle_timeout`, so the pool shrinks
 * back down when load subsides.
 *
 * `variant` keys the cache alongside the DSN so seams with materially different
 * driver options (e.g. the D1 adapter's int8→Number `types` parser vs. the raw
 * sync-engine client's default string int8) never share a connection with the
 * wrong parsing behavior.
 *
 * On Cloudflare Workers (detected via `navigator.userAgent`) the legacy
 * per-acquire fresh-client-with-real-teardown path is preserved unchanged.
 */

/** The structural postgres.js `default` export: `postgres(url, options)`. */
export type PostgresJsFactory = (
  connectionString: string,
  options: Record<string, unknown>,
) => PostgresJsClientLike

/** The minimal postgres.js client surface the callers cast to their own
 * seam shapes (`unsafe`, `begin`, tagged-template `sql`). */
export type PostgresJsClientLike = {
  end: (options?: { timeout?: number }) => Promise<void>
}

export type SharedPostgresHandle<Client extends PostgresJsClientLike> =
  Readonly<{
    /** The underlying postgres.js client (pooled on the server runtime). */
    sql: Client
    /** Release the client. No-op for the shared server pool; a real teardown
     * for the per-acquire Workers client. Always safe to call. */
    end: () => Promise<void>
  }>

export type AcquireSharedPostgresArgs = Readonly<{
  connectionString: string
  /** Stable pool identity: seams with different `options` MUST use different
   * variants so they never share a pooled connection. */
  variant: string
  /** Driver options for THIS seam. `max` is injected by this helper (the pool
   * size on the server; forced to 1 on Workers) — do not set it here. */
  options: Record<string, unknown>
  /** Test seam: inject a fake `postgres(...)` factory. Default: dynamic import
   * of `postgres`. */
  createClient?: PostgresJsFactory
  /** Test/override seam: force the runtime. Default: auto-detect. */
  runtime?: 'workers' | 'server'
}>

/** Cloudflare Workers sets `navigator.userAgent === 'Cloudflare-Workers'`. Bun
 * (Cloud Run) and Node report something else or `undefined`, so this is
 * `false` on the long-lived server. */
const detectRuntime = (): 'workers' | 'server' => {
  const ua =
    typeof navigator !== 'undefined'
      ? (navigator as { userAgent?: string }).userAgent
      : undefined
  return ua === 'Cloudflare-Workers' ? 'workers' : 'server'
}

const DEFAULT_POOL_MAX = 10

/** Server pool size per `(connectionString, variant)`. Overridable via
 * `KHALA_SYNC_PG_POOL_MAX`. Keep `variants × poolMax × maxInstances` well under
 * both Cloud Run's 100-connections-per-instance cap and Cloud SQL's
 * `max_connections`. */
export const resolveServerPoolMax = (
  processEnv: Record<string, string | undefined> = typeof process !==
  'undefined'
    ? process.env
    : {},
): number => {
  const raw = processEnv['KHALA_SYNC_PG_POOL_MAX']
  if (raw === undefined || raw.length === 0) return DEFAULT_POOL_MAX
  const parsed = Number(raw)
  if (!Number.isFinite(parsed) || parsed < 1) return DEFAULT_POOL_MAX
  return Math.floor(parsed)
}

let defaultCreateClient: PostgresJsFactory | undefined
const loadDefaultCreateClient = async (): Promise<PostgresJsFactory> => {
  if (defaultCreateClient === undefined) {
    const mod = (await import('postgres')) as unknown as {
      default: PostgresJsFactory
    }
    defaultCreateClient = mod.default
  }
  return defaultCreateClient
}

/** Process-lifetime pool cache, keyed by `${connectionString}::${variant}`. */
const serverPools = new Map<string, PostgresJsClientLike>()

/** Test-only: drop the cached server pools so a fresh test can assert
 * construction/reuse from a clean slate. */
export const __resetSharedPostgresPoolsForTests = (): void => {
  serverPools.clear()
}

/** Test-only: how many distinct pools are currently cached. */
export const __sharedPostgresPoolCountForTests = (): number => serverPools.size

const NOOP_END = async (): Promise<void> => undefined

/**
 * Acquire a postgres.js client for a Khala Sync Postgres seam.
 *
 * - Server runtime (Cloud Run / Bun): returns a SHARED pool-backed client,
 *   memoized per `(connectionString, variant)`, with a no-op `end()`.
 * - Workers runtime: returns a FRESH `max: 1` client with a real `end()`,
 *   preserving the pre-incident Hyperdrive-era discipline.
 */
export const acquireSharedPostgresClient = async <
  Client extends PostgresJsClientLike,
>(
  args: AcquireSharedPostgresArgs,
): Promise<SharedPostgresHandle<Client>> => {
  const createClient = args.createClient ?? (await loadDefaultCreateClient())
  const runtime = args.runtime ?? detectRuntime()

  if (runtime === 'workers') {
    const sql = createClient(args.connectionString, {
      ...args.options,
      max: 1,
    }) as Client
    return {
      end: () => sql.end({ timeout: 5 }),
      sql,
    }
  }

  const key = `${args.connectionString}::${args.variant}`
  const existing = serverPools.get(key)
  if (existing !== undefined) {
    return { end: NOOP_END, sql: existing as Client }
  }

  const sql = createClient(args.connectionString, {
    // Sensible pool defaults; the caller's own `options` win except for `max`,
    // which this helper owns.
    idle_timeout: 20,
    max_lifetime: 60 * 30,
    ...args.options,
    max: resolveServerPoolMax(),
  }) as Client
  serverPools.set(key, sql)
  return { end: NOOP_END, sql }
}
