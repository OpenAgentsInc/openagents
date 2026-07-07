import { describe, expect, test } from 'vitest'

import {
  readPushNotificationPreference,
  writePushNotificationPreference,
  type PushNotificationPreferenceDb,
} from './push-notification-preferences'
import { makeLedgerSqliteDb } from '../test/payments-ledger-sqlite'

// CFG-4 Domain 4 (#8519): Postgres-authoritative behind the `PaymentsLedgerDb`
// seam; tests back it with the SQLite ledger adapter.
export const PUSH_NOTIFICATION_PREFERENCES_SQLITE_SCHEMA = `
CREATE TABLE push_notification_preferences (
  user_id TEXT PRIMARY KEY,
  push_enabled INTEGER NOT NULL DEFAULT 1 CHECK (push_enabled IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
`

const makeDb = (): PushNotificationPreferenceDb =>
  makeLedgerSqliteDb(PUSH_NOTIFICATION_PREFERENCES_SQLITE_SCHEMA)

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
