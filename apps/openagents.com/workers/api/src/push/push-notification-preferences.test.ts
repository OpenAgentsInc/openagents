import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import {
  readPushNotificationPreference,
  writePushNotificationPreference,
} from './push-notification-preferences'

type Row = Record<string, unknown>

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []
  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}
  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
    return this
  }
  async first<T = Row>(): Promise<T | null> {
    const row = this.db.prepare(this.sql).get(...(this.bound as never[]))
    return (row ?? null) as T | null
  }
  async all<T = Row>(): Promise<{ results: Array<T> }> {
    return { results: this.db.prepare(this.sql).all(...(this.bound as never[])) as Array<T> }
  }
  async run(): Promise<{ meta: { changes: number }; success: true }> {
    const result = this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { meta: { changes: Number(result.changes) }, success: true }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}
  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

const schema = `
CREATE TABLE push_notification_preferences (
  user_id TEXT PRIMARY KEY,
  push_enabled INTEGER NOT NULL DEFAULT 1 CHECK (push_enabled IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
`

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(schema)
  return new SqliteD1(raw) as unknown as D1Database
}

describe('readPushNotificationPreference', () => {
  test('defaults to enabled when no row exists (opt-out, not opt-in)', async () => {
    const db = makeDb()
    const preference = await readPushNotificationPreference(db, 'user-1')
    expect(preference).toEqual({ pushEnabled: true, updatedAt: '', userId: 'user-1' })
  })
})

describe('writePushNotificationPreference', () => {
  test('persists and reads back a disabled preference', async () => {
    const db = makeDb()
    await writePushNotificationPreference(db, {
      nowIso: '2026-07-06T00:00:00.000Z',
      pushEnabled: false,
      userId: 'user-1',
    })
    const preference = await readPushNotificationPreference(db, 'user-1')
    expect(preference).toEqual({
      pushEnabled: false,
      updatedAt: '2026-07-06T00:00:00.000Z',
      userId: 'user-1',
    })
  })

  test('re-enabling upserts rather than erroring', async () => {
    const db = makeDb()
    await writePushNotificationPreference(db, {
      nowIso: '2026-07-06T00:00:00.000Z',
      pushEnabled: false,
      userId: 'user-1',
    })
    await writePushNotificationPreference(db, {
      nowIso: '2026-07-06T01:00:00.000Z',
      pushEnabled: true,
      userId: 'user-1',
    })
    expect((await readPushNotificationPreference(db, 'user-1')).pushEnabled).toBe(true)
  })

  test('preferences are scoped per user', async () => {
    const db = makeDb()
    await writePushNotificationPreference(db, {
      nowIso: '2026-07-06T00:00:00.000Z',
      pushEnabled: false,
      userId: 'user-1',
    })
    expect((await readPushNotificationPreference(db, 'user-2')).pushEnabled).toBe(true)
  })
})
