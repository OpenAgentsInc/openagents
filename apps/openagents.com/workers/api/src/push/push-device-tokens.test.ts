import { describe, expect, test } from 'vitest'

import {
  listActivePushDeviceTokensForUser,
  listPushDeviceTokensForUser,
  registerPushDeviceToken,
  removePushDeviceTokensByExpoToken,
  unregisterPushDeviceToken,
  type PushDeviceTokenDb,
} from './push-device-tokens'
import { makeLedgerSqliteDb } from '../test/payments-ledger-sqlite'

// CFG-4 Domain 4 (#8519): the push registry is Postgres-authoritative behind
// the `PaymentsLedgerDb` seam; tests back it with the SQLite ledger adapter
// (the same `assertPortableLedgerSql` dialect guard the credits domain uses).
export const PUSH_DEVICE_TOKENS_SQLITE_SCHEMA = `
CREATE TABLE push_device_tokens (
  user_id TEXT NOT NULL,
  device_id TEXT NOT NULL,
  expo_push_token TEXT NOT NULL,
  platform TEXT NOT NULL CHECK (platform IN ('ios', 'android')),
  access_token_revocation_key TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, device_id)
);
`

const makeDb = (): PushDeviceTokenDb => makeLedgerSqliteDb(PUSH_DEVICE_TOKENS_SQLITE_SCHEMA)

const fakeRevocationStore = () => {
  const revoked = new Set<string>()
  return {
    revoke: (key: string) => revoked.add(key),
    store: {
      get: async (key: string) => (revoked.has(key) ? '1' : null),
      put: async () => {},
    } as unknown as import('../auth/mobile-session').MobileAccessRevocationStore,
  }
}

describe('registerPushDeviceToken', () => {
  test('inserts a fresh row keyed per user+device', async () => {
    const db = makeDb()
    const row = await registerPushDeviceToken(db, {
      accessToken: 'access-token-1',
      deviceId: 'device-1',
      expoPushToken: 'ExponentPushToken[abc]',
      nowIso: '2026-07-05T00:00:00.000Z',
      platform: 'ios',
      userId: 'user-1',
    })

    expect(row).toEqual({
      createdAt: '2026-07-05T00:00:00.000Z',
      deviceId: 'device-1',
      expoPushToken: 'ExponentPushToken[abc]',
      platform: 'ios',
      updatedAt: '2026-07-05T00:00:00.000Z',
      userId: 'user-1',
    })
  })

  test('re-registering the SAME device upserts (never duplicates rows)', async () => {
    const db = makeDb()
    await registerPushDeviceToken(db, {
      accessToken: 'access-token-1',
      deviceId: 'device-1',
      expoPushToken: 'ExponentPushToken[old]',
      nowIso: '2026-07-05T00:00:00.000Z',
      platform: 'ios',
      userId: 'user-1',
    })
    await registerPushDeviceToken(db, {
      accessToken: 'access-token-2',
      deviceId: 'device-1',
      expoPushToken: 'ExponentPushToken[new]',
      nowIso: '2026-07-05T01:00:00.000Z',
      platform: 'ios',
      userId: 'user-1',
    })

    const rows = await listPushDeviceTokensForUser(db, 'user-1')
    expect(rows).toHaveLength(1)
    expect(rows[0]?.expoPushToken).toBe('ExponentPushToken[new]')
    expect(rows[0]?.updatedAt).toBe('2026-07-05T01:00:00.000Z')
  })

  test('two distinct devices for the same user both persist', async () => {
    const db = makeDb()
    await registerPushDeviceToken(db, {
      accessToken: 'a1',
      deviceId: 'device-1',
      expoPushToken: 'token-1',
      nowIso: '2026-07-05T00:00:00.000Z',
      platform: 'ios',
      userId: 'user-1',
    })
    await registerPushDeviceToken(db, {
      accessToken: 'a2',
      deviceId: 'device-2',
      expoPushToken: 'token-2',
      nowIso: '2026-07-05T00:00:00.000Z',
      platform: 'android',
      userId: 'user-1',
    })

    const rows = await listPushDeviceTokensForUser(db, 'user-1')
    expect(rows).toHaveLength(2)
  })
})

describe('unregisterPushDeviceToken', () => {
  test('removes the row and reports removed:true; a second call reports removed:false', async () => {
    const db = makeDb()
    await registerPushDeviceToken(db, {
      accessToken: 'a1',
      deviceId: 'device-1',
      expoPushToken: 'token-1',
      nowIso: '2026-07-05T00:00:00.000Z',
      platform: 'ios',
      userId: 'user-1',
    })

    const first = await unregisterPushDeviceToken(db, { deviceId: 'device-1', userId: 'user-1' })
    expect(first.removed).toBe(true)
    expect(await listPushDeviceTokensForUser(db, 'user-1')).toHaveLength(0)

    const second = await unregisterPushDeviceToken(db, { deviceId: 'device-1', userId: 'user-1' })
    expect(second.removed).toBe(false)
  })

  test('never removes another user\'s device with the same device id', async () => {
    const db = makeDb()
    await registerPushDeviceToken(db, {
      accessToken: 'a1',
      deviceId: 'shared-device-id',
      expoPushToken: 'token-1',
      nowIso: '2026-07-05T00:00:00.000Z',
      platform: 'ios',
      userId: 'user-1',
    })
    await registerPushDeviceToken(db, {
      accessToken: 'a2',
      deviceId: 'shared-device-id',
      expoPushToken: 'token-2',
      nowIso: '2026-07-05T00:00:00.000Z',
      platform: 'ios',
      userId: 'user-2',
    })

    await unregisterPushDeviceToken(db, { deviceId: 'shared-device-id', userId: 'user-1' })

    expect(await listPushDeviceTokensForUser(db, 'user-1')).toHaveLength(0)
    expect(await listPushDeviceTokensForUser(db, 'user-2')).toHaveLength(1)
  })
})

describe('listActivePushDeviceTokensForUser — pruned on auth revocation', () => {
  test('a row whose access token was revoked is pruned and excluded', async () => {
    const db = makeDb()
    const revocation = fakeRevocationStore()

    await registerPushDeviceToken(db, {
      accessToken: 'still-live-token',
      deviceId: 'device-live',
      expoPushToken: 'token-live',
      nowIso: '2026-07-05T00:00:00.000Z',
      platform: 'ios',
      userId: 'user-1',
    })
    await registerPushDeviceToken(db, {
      accessToken: 'revoked-token',
      deviceId: 'device-revoked',
      expoPushToken: 'token-revoked',
      nowIso: '2026-07-05T00:00:00.000Z',
      platform: 'ios',
      userId: 'user-1',
    })

    // Simulate sign-out: the SAME hashing scheme mobile-session.ts uses.
    const { mobileRevokedAccessKey } = await import('../auth/mobile-session')
    revocation.revoke(await mobileRevokedAccessKey('revoked-token'))

    const active = await listActivePushDeviceTokensForUser(db, revocation.store, 'user-1')
    expect(active.map(row => row.deviceId)).toEqual(['device-live'])

    // The pruned row is actually deleted, not just filtered in memory.
    expect(await listPushDeviceTokensForUser(db, 'user-1')).toHaveLength(1)
  })
})

describe('removePushDeviceTokensByExpoToken', () => {
  test('removes every row carrying a given Expo push token, across users/devices', async () => {
    const db = makeDb()
    await registerPushDeviceToken(db, {
      accessToken: 'a1',
      deviceId: 'device-1',
      expoPushToken: 'shared-token',
      nowIso: '2026-07-05T00:00:00.000Z',
      platform: 'ios',
      userId: 'user-1',
    })
    await registerPushDeviceToken(db, {
      accessToken: 'a2',
      deviceId: 'device-2',
      expoPushToken: 'shared-token',
      nowIso: '2026-07-05T00:00:00.000Z',
      platform: 'android',
      userId: 'user-2',
    })

    const removed = await removePushDeviceTokensByExpoToken(db, 'shared-token')
    expect(removed).toBe(2)
    expect(await listPushDeviceTokensForUser(db, 'user-1')).toHaveLength(0)
    expect(await listPushDeviceTokensForUser(db, 'user-2')).toHaveLength(0)
  })
})
