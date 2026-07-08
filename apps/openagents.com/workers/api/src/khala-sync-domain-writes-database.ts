// #8515 D1 evacuation — the shared WRITES-cutover lever for the adapter-safe,
// non-money Khala Sync domains (training, sites, gym/evals, CRM, agent
// runtime, supervision).
//
// The proven CFG-4 (#8519) lever, generalized: instead of hand-writing a
// Postgres store per domain, hand the domain's EXISTING D1-API store factory a
// `D1Database`-SHAPED handle backed by Cloud SQL Postgres
// (`makePostgresD1Database`). The store's inline SQL — `INSERT … ON CONFLICT`,
// `UPDATE`, `.batch()` — then runs UNCHANGED on Postgres, and `.batch()`
// becomes ONE Postgres transaction. This moves BOTH reads and writes for that
// domain off the 401-dead D1 HTTP bridge in a single seam.
//
// Each domain gates on `KHALA_SYNC_<DOMAIN>_WRITES` (parsed here). Posture
// mirrors the pylon dispatch cutover (`pylonDispatchFlagsFromEnv`): the safe
// default after the Cloudflare exit is `postgres`, and the ONLY way back to
// dead D1 is an explicit `d1` — a typo must never silently route authoritative
// writes onto the 401 bridge. When the `KHALA_SYNC_DB` binding is absent
// (tests without a binding), the handle is `undefined` and the caller degrades
// to plain D1 — identical to the pre-cutover behavior.
//
// The adapter only translates `?`→`$n`, `IS ?`, and `INSERT OR IGNORE`; it
// THROWS on `INSERT OR REPLACE` and does not rewrite
// `datetime()/strftime()/json_extract()/julianday()`. Only domains whose SQL
// avoids those are cut through this lever.

import { acquireSharedPostgresClient } from './khala-sync-postgres-pool'
import type { KhalaSyncHyperdriveBinding } from './khala-sync-push-routes'
import {
  makePostgresD1Database,
  type PostgresD1Client,
} from './postgres-d1-adapter'

// ---------------------------------------------------------------------------
// Flag
// ---------------------------------------------------------------------------

export type KhalaSyncWritesMode = 'd1' | 'postgres'

/**
 * Parse a `KHALA_SYNC_<DOMAIN>_WRITES` var. Default `postgres` (#8515: D1 is
 * dead account-wide); only an explicit `d1` restores the D1 write authority —
 * the inverse typo-posture of the `reads` flags on purpose.
 */
export const parseKhalaSyncWritesMode = (
  raw: string | undefined,
): KhalaSyncWritesMode =>
  raw?.trim().toLowerCase() === 'd1' ? 'd1' : 'postgres'

// ---------------------------------------------------------------------------
// Postgres client (int8 -> JS number, transaction-mode-safe)
// ---------------------------------------------------------------------------

/**
 * The int8-parsing postgres.js client the D1 adapter runs on, matching the
 * CFG-4 product-state discipline (`prepare: false`, `max: 1`, int8 oid 20
 * parsed to a JS number so bigint twin columns — counts, msat, text-ISO
 * timestamps, all < 2^53 on these domains — read back in the numeric shape D1
 * returned). Tests inject their own client instead.
 */
export const defaultMakeKhalaSyncDomainD1Client = async (
  connectionString: string,
): Promise<PostgresD1Client> => {
  // On Cloud Run this reuses the shared 'd1-bigint' pool (same int8→Number
  // parser as the Khala Code product-state D1 client), instead of opening a
  // fresh connection per D1 statement.
  const { sql, end } = await acquireSharedPostgresClient({
    connectionString,
    options: {
      connect_timeout: 10,
      prepare: false,
      types: {
        bigint: {
          from: [20],
          parse: (value: string) => Number(value),
          serialize: (value: number | bigint) => value.toString(),
          to: 20,
        },
      },
    },
    variant: 'd1-bigint',
  })
  return {
    end,
    sql: sql as unknown as PostgresD1Client['sql'],
  }
}

// ---------------------------------------------------------------------------
// Handle
// ---------------------------------------------------------------------------

export type KhalaSyncWritesDatabaseEnv = Readonly<{
  KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
}>

export type MakeKhalaSyncWritesDatabaseOptions = Readonly<{
  /** Injectable adapter client factory (tests). Default: the int8 postgres.js
   * client above. */
  makeD1Client?:
    | ((connectionString: string) => Promise<PostgresD1Client>)
    | undefined
}>

/**
 * Build the Postgres-backed, D1-shaped write/read handle for a domain, or
 * `undefined` when the `KHALA_SYNC_DB` binding is absent (the caller then
 * falls back to `openAgentsDatabase(env)` — plain D1). The returned handle is
 * a drop-in `D1Database` the domain's existing store factories consume
 * unchanged.
 */
export const makeKhalaSyncWritesDatabase = (
  env: KhalaSyncWritesDatabaseEnv,
  options: MakeKhalaSyncWritesDatabaseOptions = {},
): D1Database | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeD1Client =
    options.makeD1Client ?? defaultMakeKhalaSyncDomainD1Client
  return makePostgresD1Database({
    acquireSql: () => makeD1Client(connectionString),
  })
}
