/**
 * CFG-9/CFG-3 (#8524, #8518): Postgres-authoritative OpenAuth storage.
 *
 * Serves the OpenAuth issuer's `StorageAdapter` from the EXISTING
 * `openauth_storage` Postgres twin (khala-sync identity/auth domain,
 * #8362) instead of D1. The twin has been dual-written + backfilled from
 * production D1, so signing/encryption keys and live refresh tokens are
 * already present — flipping authority here preserves existing sessions.
 *
 * Activated via `OPENAUTH_STORAGE_AUTHORITY=postgres`
 * (identity-auth-domain-store.ts `makeOpenAuthStorageForEnv`). This is the
 * Cloud Run monolith's default posture: D1 WRITES are dead account-wide
 * (Cloudflare error 7500, storage limit exceeded — observed live on
 * 2026-07-06), so a D1-authoritative issuer cannot mint sessions at all.
 *
 * Table shape (khala-sync identity domain): key text PK, value_json text,
 * expires_at bigint (epoch ms, nullable), updated_at text (ISO).
 *
 * The postgres.js client is CACHED per connection string and never ended:
 * this adapter only runs on the long-lived Bun monolith (the flag is unset
 * on the Worker), where a persistent max:1 `prepare:false` client is the
 * cheap, transaction-mode-safe choice.
 */

import {
  type StorageAdapter,
  joinKey,
  splitKey,
} from '@openauthjs/openauth/storage/storage'

import { safeJsonRecord } from '../json-boundary'
import {
  type OpenAuthStorageRuntime,
  systemOpenAuthStorageRuntime,
} from './openauth-storage'

type Row = Readonly<{
  key: string
  value_json: string
  expires_at: number | string | null
}>

/** Minimal structural postgres.js tagged-template surface we need. */
export type OpenAuthSql = (
  strings: TemplateStringsArray,
  ...values: ReadonlyArray<unknown>
) => Promise<ReadonlyArray<Row>>

const clientCache = new Map<string, Promise<OpenAuthSql>>()

const defaultAcquireSql = (connectionString: string): Promise<OpenAuthSql> => {
  const cached = clientCache.get(connectionString)
  if (cached !== undefined) return cached

  const made = (async () => {
    const mod = (await import('postgres')) as unknown as {
      default: (
        connectionString: string,
        options: Record<string, unknown>,
      ) => OpenAuthSql
    }
    return mod.default(connectionString, {
      connect_timeout: 10,
      max: Number(process.env['OPENAUTH_STORAGE_POOL_MAX'] ?? 3),
      prepare: false,
    })
  })()

  clientCache.set(connectionString, made)
  return made
}

const expiresAtMs = (value: number | string | null): number | null =>
  value === null ? null : Number(value)

export const makePostgresOpenAuthStorage = (
  connectionString: string,
  runtime: OpenAuthStorageRuntime = systemOpenAuthStorageRuntime,
  acquireSql: (
    connectionString: string,
  ) => Promise<OpenAuthSql> = defaultAcquireSql,
): StorageAdapter => {
  const withSql = async <A>(run: (sql: OpenAuthSql) => Promise<A>): Promise<A> =>
    run(await acquireSql(connectionString))

  return {
    get: async key => {
      const storageKey = joinKey(key)
      return withSql(async sql => {
        const rows = await sql`
          SELECT key, value_json, expires_at
          FROM openauth_storage
          WHERE key = ${storageKey}
        `
        const row = rows[0]
        if (row === undefined) return undefined

        const expiry = expiresAtMs(row.expires_at)
        if (expiry !== null && expiry <= runtime.nowMs()) {
          await sql`DELETE FROM openauth_storage WHERE key = ${storageKey}`
          return undefined
        }

        return safeJsonRecord(row.value_json)
      })
    },

    remove: async key => {
      const storageKey = joinKey(key)
      await withSql(
        sql => sql`DELETE FROM openauth_storage WHERE key = ${storageKey}`,
      )
    },

    scan: async function* (prefix) {
      const like = `${joinKey([...prefix, ''])}%`
      const now = runtime.nowMs()
      const rows = await withSql(
        sql => sql`
          SELECT key, value_json, expires_at
          FROM openauth_storage
          WHERE key LIKE ${like}
            AND (expires_at IS NULL OR expires_at > ${now})
          ORDER BY key
        `,
      )
      for (const row of rows) {
        const parsed = safeJsonRecord(row.value_json)
        if (parsed !== undefined) {
          yield [splitKey(row.key), parsed] as [Array<string>, unknown]
        }
      }
    },

    set: async (key, value: unknown, expiry) => {
      const storageKey = joinKey(key)
      const valueJson = JSON.stringify(value)
      const expiresAt = expiry?.getTime() ?? null
      const updatedAt = runtime.nowIso()
      await withSql(
        sql => sql`
          INSERT INTO openauth_storage (key, value_json, expires_at, updated_at)
          VALUES (${storageKey}, ${valueJson}, ${expiresAt}, ${updatedAt})
          ON CONFLICT (key) DO UPDATE SET
            value_json = EXCLUDED.value_json,
            expires_at = EXCLUDED.expires_at,
            updated_at = EXCLUDED.updated_at
        `,
      )
    },
  }
}
