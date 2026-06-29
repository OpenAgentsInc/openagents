import {
  type StorageAdapter,
  joinKey,
  splitKey,
} from '@openauthjs/openauth/storage/storage'

import { safeJsonRecord } from '../json-boundary'
import { currentEpochMillis, currentIsoTimestamp } from '../runtime-primitives'

export type OpenAuthStorageRuntime = Readonly<{
  nowIso: () => string
  nowMs: () => number
}>

export const systemOpenAuthStorageRuntime: OpenAuthStorageRuntime = {
  nowIso: currentIsoTimestamp,
  nowMs: currentEpochMillis,
}

export const makeD1Storage = (
  db: D1Database,
  runtime: OpenAuthStorageRuntime = systemOpenAuthStorageRuntime,
): StorageAdapter => ({
  get: async key => {
    const storageKey = joinKey(key)
    const row = await db
      .prepare(
        `SELECT value_json, expires_at
         FROM openauth_storage
         WHERE key = ?`,
      )
      .bind(storageKey)
      .first<Readonly<{ value_json: string; expires_at: number | null }>>()

    if (row === null) {
      return undefined
    }

    if (row.expires_at !== null && row.expires_at <= runtime.nowMs()) {
      await db
        .prepare(`DELETE FROM openauth_storage WHERE key = ?`)
        .bind(storageKey)
        .run()

      return undefined
    }

    return safeJsonRecord(row.value_json)
  },

  set: async (key, value: unknown, expiry) => {
    await db
      .prepare(
        `INSERT INTO openauth_storage
          (key, value_json, expires_at, updated_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET
          value_json = excluded.value_json,
          expires_at = excluded.expires_at,
          updated_at = excluded.updated_at`,
      )
      .bind(
        joinKey(key),
        JSON.stringify(value),
        expiry?.getTime() ?? null,
        runtime.nowIso(),
      )
      .run()
  },

  remove: async key => {
    await db
      .prepare(`DELETE FROM openauth_storage WHERE key = ?`)
      .bind(joinKey(key))
      .run()
  },

  scan: async function* (prefix) {
    const now = runtime.nowMs()
    const rows = await db
      .prepare(
        `SELECT key, value_json, expires_at
         FROM openauth_storage
         WHERE key LIKE ?
           AND (expires_at IS NULL OR expires_at > ?)
         ORDER BY key`,
      )
      .bind(`${joinKey([...prefix, ''])}%`, now)
      .all<
        Readonly<{ key: string; value_json: string; expires_at: number | null }>
      >()

    for (const row of rows.results) {
      const parsed = safeJsonRecord(row.value_json)

      if (parsed !== undefined) {
        yield [splitKey(row.key), parsed]
      }
    }
  },
})
