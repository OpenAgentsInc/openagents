/**
 * Postgres KvStore backend (migrations/0001_oa_infra_kv.sql).
 *
 * Single table `oa_infra_kv (key text PK, value text, expires_at
 * timestamptz)`. Expiry is LAZY on the read path: `get` deletes-and-misses
 * expired rows in one statement, so no background sweeper is required for
 * correctness. Operators may still bulk-reap with `sweepExpired`.
 */
import { Effect, Layer } from "effect"
import { KvStore, KvStoreBackendError, type KvStoreShape } from "./kv-store.ts"
import { OaInfraSql } from "./sql.ts"
import type { SQL } from "bun"

const BACKEND = "postgres"

const tryPg = <A>(operation: string, run: () => Promise<A>) =>
  Effect.tryPromise({
    try: run,
    catch: (cause) => new KvStoreBackendError({ backend: BACKEND, operation, cause }),
  })

export const makePostgresKvStore = (sql: SQL): KvStoreShape => {
  const get = (key: string) =>
    tryPg("get", async () => {
      // Lazy expiry: reap the expired row and report a miss atomically.
      const rows: Array<{ value: string; expired: boolean }> = await sql`
        WITH reaped AS (
          DELETE FROM oa_infra_kv
          WHERE key = ${key} AND expires_at IS NOT NULL AND expires_at <= now()
          RETURNING key
        )
        SELECT value, (key IN (SELECT key FROM reaped)) AS expired
        FROM oa_infra_kv
        WHERE key = ${key}
      `
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

  return { get, put, delete: del }
}

/** Bulk-reap expired rows (optional maintenance; correctness never needs it). */
export const sweepExpired = (
  sql: SQL,
): Effect.Effect<number, KvStoreBackendError> =>
  tryPg("sweepExpired", async () => {
    const rows: Array<{ key: string }> = await sql`
      DELETE FROM oa_infra_kv
      WHERE expires_at IS NOT NULL AND expires_at <= now()
      RETURNING key
    `
    return rows.length
  })

/** Postgres KvStore Layer; requires `OaInfraSql` (see src/sql.ts). */
export const layerPostgres: Layer.Layer<KvStore, never, OaInfraSql> = Layer.effect(
  KvStore,
  Effect.gen(function* () {
    const { sql } = yield* OaInfraSql
    return makePostgresKvStore(sql)
  }),
)
