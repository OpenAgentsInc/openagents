/**
 * CFG-9 (#8524): `AUTH_STORAGE` — a `KVNamespace`-compatible adapter over
 * the owned Postgres `oa_infra_kv` table (CFG-2/CFG-3, oa-infra
 * migrations/0001_oa_infra_kv.sql), replacing the Cloudflare KV binding.
 *
 * Same table + semantics as `@openagentsinc/oa-infra/kv-store-postgres`
 * (lazy expiry on the read path) so the CFG-3 KvStore layer and this
 * Workers-shaped adapter see one storage. Implements the subset the worker
 * uses (`get`/`put(expirationTtl)`/`delete`) plus `list`/`getWithMetadata`
 * for API completeness.
 */

import type { SQL } from 'bun'

export type KvKeyPrefix = string

export const makePostgresKvNamespace = (
  sql: SQL,
  options: Readonly<{ keyPrefix?: KvKeyPrefix }> = {},
): KVNamespace => {
  const prefix = options.keyPrefix ?? ''
  const k = (key: string) => `${prefix}${key}`

  const readValue = async (key: string): Promise<string | null> => {
    const rows: Array<{ value: string; expired: boolean }> = await sql`
      WITH reaped AS (
        DELETE FROM oa_infra_kv
        WHERE key = ${k(key)} AND expires_at IS NOT NULL AND expires_at <= now()
        RETURNING key
      )
      SELECT value, (key IN (SELECT key FROM reaped)) AS expired
      FROM oa_infra_kv
      WHERE key = ${k(key)}
    `
    const row = rows[0]
    if (row === undefined || row.expired) return null
    return row.value
  }

  const decode = (value: string | null, type: string | undefined): unknown => {
    if (value === null) return null
    switch (type) {
      case 'json':
        return JSON.parse(value)
      case 'arrayBuffer':
        return new TextEncoder().encode(value).buffer
      case 'stream':
        return new Response(value).body
      default:
        return value
    }
  }

  const namespace = {
    delete: async (key: string): Promise<void> => {
      await sql`DELETE FROM oa_infra_kv WHERE key = ${k(key)}`
    },
    get: async (
      key: string,
      typeOrOptions?: string | { type?: string },
    ): Promise<unknown> => {
      const type =
        typeof typeOrOptions === 'string' ? typeOrOptions : typeOrOptions?.type
      return decode(await readValue(key), type)
    },
    getWithMetadata: async (
      key: string,
      typeOrOptions?: string | { type?: string },
    ): Promise<{ cacheStatus: null; metadata: null; value: unknown }> => {
      const type =
        typeof typeOrOptions === 'string' ? typeOrOptions : typeOrOptions?.type
      return {
        cacheStatus: null,
        metadata: null,
        value: decode(await readValue(key), type),
      }
    },
    list: async (
      listOptions: Readonly<{
        cursor?: string
        limit?: number
        prefix?: string
      }> = {},
    ) => {
      const limit = Math.min(Math.max(listOptions.limit ?? 1000, 1), 1000)
      const like = `${k(listOptions.prefix ?? '')}%`
      const after = listOptions.cursor ?? ''
      const rows: Array<{ key: string; expires_at: string | null }> =
        await sql`
          SELECT key, expires_at
          FROM oa_infra_kv
          WHERE key LIKE ${like}
            AND key > ${after}
            AND (expires_at IS NULL OR expires_at > now())
          ORDER BY key
          LIMIT ${limit + 1}
        `
      const page = rows.slice(0, limit)
      const listComplete = rows.length <= limit
      return {
        cacheStatus: null,
        keys: page.map(row => ({
          expiration:
            row.expires_at === null
              ? undefined
              : Math.floor(new Date(row.expires_at).getTime() / 1000),
          name: row.key.slice(prefix.length),
        })),
        list_complete: listComplete,
        ...(listComplete
          ? {}
          : { cursor: page[page.length - 1]?.key ?? '' }),
      }
    },
    put: async (
      key: string,
      value: string | ArrayBuffer | ArrayBufferView | ReadableStream,
      putOptions: Readonly<{
        expiration?: number
        expirationTtl?: number
      }> = {},
    ): Promise<void> => {
      const text =
        typeof value === 'string'
          ? value
          : value instanceof ReadableStream
            ? await new Response(value).text()
            : new TextDecoder().decode(
                value instanceof ArrayBuffer ? value : (value.buffer as ArrayBuffer),
              )

      const ttlMs =
        putOptions.expirationTtl !== undefined
          ? putOptions.expirationTtl * 1000
          : putOptions.expiration !== undefined
            ? putOptions.expiration * 1000 - Date.now()
            : null

      await sql`
        INSERT INTO oa_infra_kv (key, value, expires_at, updated_at)
        VALUES (
          ${k(key)},
          ${text},
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
    },
  }

  return namespace as unknown as KVNamespace
}
