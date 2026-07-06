/**
 * Postgres KvStore backend (migrations/0001_oa_infra_kv.sql,
 * 0004_oa_infra_kv_key_prefix.sql).
 *
 * Single table `oa_infra_kv (key text PK, value text, expires_at
 * timestamptz)`. Expiry is LAZY on the read path: `get` deletes-and-misses
 * expired rows in one statement, so no background sweeper is required for
 * correctness. Operators may still bulk-reap with `sweepExpired`.
 *
 * DRIVER SEAM (CFG-3, issue #8518): the implementation is written against
 * the structural `KvSql` tagged-template type below instead of Bun's `SQL`
 * class, so the SAME module backs both runtimes we care about — the Bun
 * monolith (Bun `SQL` via `layerPostgres` in kv-store-postgres-layer.ts)
 * and the `openagents.com` Worker (a postgres.js client over the khala-sync
 * Hyperdrive path; see the Worker's `auth/auth-kv.ts`). Both drivers
 * expose the same `` sql`...` `` call shape; only parameterized
 * interpolation is used, never driver-specific helpers. This module MUST
 * stay free of `bun`/node-only imports — the Worker typechecks it under
 * `@cloudflare/workers-types`; the Bun-SQL Layer lives in
 * kv-store-postgres-layer.ts for that reason.
 */
import { Effect } from "effect"
import { KvStoreBackendError, type KvStoreShape } from "./kv-store.ts"

const BACKEND = "postgres"

/**
 * Minimal structural tagged-template SQL client: what Bun's `SQL` and
 * postgres.js both satisfy for the plain parameterized statements this
 * module issues.
 */
export type KvSql = (
  strings: TemplateStringsArray,
  ...values: ReadonlyArray<unknown>
) => PromiseLike<unknown>

const tryPg = <A>(operation: string, run: () => Promise<A>) =>
  Effect.tryPromise({
    try: run,
    catch: (cause) => new KvStoreBackendError({ backend: BACKEND, operation, cause }),
  })

/**
 * Escape LIKE pattern metacharacters so a caller-supplied prefix is always
 * matched LITERALLY (`listPrefix` contract). Postgres's default LIKE escape
 * character is backslash.
 */
const escapeLikePrefix = (prefix: string): string =>
  prefix.replace(/([\\%_])/g, "\\$1")

export const makePostgresKvStore = (sql: KvSql): KvStoreShape => {
  const get = (key: string) =>
    tryPg("get", async () => {
      // Lazy expiry: reap the expired row and report a miss atomically.
      const rows = (await sql`
        WITH reaped AS (
          DELETE FROM oa_infra_kv
          WHERE key = ${key} AND expires_at IS NOT NULL AND expires_at <= now()
          RETURNING key
        )
        SELECT value, (key IN (SELECT key FROM reaped)) AS expired
        FROM oa_infra_kv
        WHERE key = ${key}
      `) as ReadonlyArray<{ value: string; expired: boolean }>
      const row = rows[0]
      if (row === undefined || row.expired) return null
      return row.value
    })

  const put = (key: string, value: string, options?: { readonly ttlMs?: number }) =>
    tryPg("put", async () => {
      const ttlMs = options?.ttlMs ?? null
      await sql`
        INSERT INTO oa_infra_kv (key, value, expires_at, updated_at)
        VALUES (
          ${key},
          ${value},
          CASE
            WHEN ${ttlMs}::bigint IS NULL THEN NULL
            ELSE now() + make_interval(secs => ${ttlMs}::bigint / 1000.0)
          END,
          now()
        )
        ON CONFLICT (key) DO UPDATE SET
          value = EXCLUDED.value,
          expires_at = EXCLUDED.expires_at,
          updated_at = EXCLUDED.updated_at
      `
    })

  const del = (key: string) =>
    tryPg("delete", async () => {
      await sql`DELETE FROM oa_infra_kv WHERE key = ${key}`
    })

  const listPrefix = (prefix: string) =>
    tryPg("listPrefix", async () => {
      const rows = (await sql`
        SELECT key, value
        FROM oa_infra_kv
        WHERE key LIKE ${`${escapeLikePrefix(prefix)}%`}
          AND (expires_at IS NULL OR expires_at > now())
        ORDER BY key
      `) as ReadonlyArray<{ key: string; value: string }>
      return rows.map((row) => ({ key: row.key, value: row.value }))
    })

  return { get, put, delete: del, listPrefix }
}

/** Bulk-reap expired rows (optional maintenance; correctness never needs it). */
export const sweepExpired = (
  sql: KvSql,
): Effect.Effect<number, KvStoreBackendError> =>
  tryPg("sweepExpired", async () => {
    const rows = (await sql`
      DELETE FROM oa_infra_kv
      WHERE expires_at IS NOT NULL AND expires_at <= now()
      RETURNING key
    `) as ReadonlyArray<{ key: string }>
    return rows.length
  })
