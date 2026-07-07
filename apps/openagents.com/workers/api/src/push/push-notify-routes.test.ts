import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  handlePushNotificationPreferencesRequest,
  handlePushNotifyEventsRequest,
  PUSH_NOTIFICATION_PREFERENCES_PATH,
  PUSH_NOTIFY_EVENTS_PATH,
  type PushNotifyRouteDependencies,
} from './push-notify-routes'
import { registerPushDeviceToken, type PushDeviceTokenDb } from './push-device-tokens'
import type { MobileAccessRevocationStore } from '../auth/mobile-session'
import { makeLedgerSqliteDb } from '../test/payments-ledger-sqlite'

type FakeEnv = Readonly<{ db: PushDeviceTokenDb }>
type FakeUser = Readonly<{ userId: string }>

// CFG-4 Domain 4 (#8519): both push tables are Postgres-authoritative behind
// the `PaymentsLedgerDb` seam; tests back them with the SQLite ledger adapter.
const PUSH_TABLES_SQLITE_SCHEMA = `
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
CREATE TABLE push_notification_preferences (
  user_id TEXT PRIMARY KEY,
  push_enabled INTEGER NOT NULL DEFAULT 1 CHECK (push_enabled IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
`

const makeEnv = (): FakeEnv => ({ db: makeLedgerSqliteDb(PUSH_TABLES_SQLITE_SCHEMA) })

const fakeAuthStorage = (): MobileAccessRevocationStore =>
  ({ get: async () => null, put: async () => {} }) as unknown as MobileAccessRevocationStore

const ctx = {} as ExecutionContext

const makeDependencies = (
  input: Readonly<{ adminOk: boolean; sessionUserId: string | undefined }>,
): PushNotifyRouteDependencies<FakeEnv, FakeUser> => ({
  authStorage: () => fakeAuthStorage(),
  db: env => env.db,
  fetchImpl: async () =>
    new Response(JSON.stringify({ data: [{ id: 'ticket-1', status: 'ok' }] }), { status: 200 }),
  nowIso: () => '2026-07-06T00:00:00.000Z',
  requireAdminApiToken: async () => input.adminOk,
  requireUserBearerSession: async () =>
    input.sessionUserId === undefined ? undefined : { user: { userId: input.sessionUserId } },
  userIdFromSession: session => session.user.userId,
})

describe('handlePushNotifyEventsRequest — auth gating', () => {
  test('rejects without a valid admin bearer', async () => {
    const env = makeEnv()
    const response = await Effect.runPromise(
      handlePushNotifyEventsRequest(
        makeDependencies({ adminOk: false, sessionUserId: undefined }),
        new Request(`https://openagents.com${PUSH_NOTIFY_EVENTS_PATH}`, {
          body: JSON.stringify({ kind: 'turn_completed', ownerUserId: 'user-1', threadId: 'thread-1' }),
          method: 'POST',
        }),
        env,
      ),
    )
    expect(response.status).toBe(401)
  })

  test('rejects a non-POST method', async () => {
    const env = makeEnv()
    const response = await Effect.runPromise(
      handlePushNotifyEventsRequest(
        makeDependencies({ adminOk: true, sessionUserId: undefined }),
        new Request(`https://openagents.com${PUSH_NOTIFY_EVENTS_PATH}`, { method: 'GET' }),
        env,
      ),
    )
    expect(response.status).toBe(405)
  })

  test('rejects an invalid/missing kind', async () => {
    const env = makeEnv()
    const response = await Effect.runPromise(
      handlePushNotifyEventsRequest(
        makeDependencies({ adminOk: true, sessionUserId: undefined }),
        new Request(`https://openagents.com${PUSH_NOTIFY_EVENTS_PATH}`, {
          body: JSON.stringify({ kind: 'not_a_real_kind', ownerUserId: 'user-1', threadId: 'thread-1' }),
          method: 'POST',
        }),
        env,
      ),
    )
    expect(response.status).toBe(400)
  })
})

describe('handlePushNotifyEventsRequest — dispatch', () => {
  test('sends to every active device for the owner and reports sent count', async () => {
    const env = makeEnv()
    await registerPushDeviceToken(env.db, {
      accessToken: 'a1',
      deviceId: 'device-1',
      expoPushToken: 'token-1',
      nowIso: '2026-07-06T00:00:00.000Z',
      platform: 'ios',
      userId: 'user-1',
    })

    const response = await Effect.runPromise(
      handlePushNotifyEventsRequest(
        makeDependencies({ adminOk: true, sessionUserId: undefined }),
        new Request(`https://openagents.com${PUSH_NOTIFY_EVENTS_PATH}`, {
          body: JSON.stringify({ kind: 'turn_completed', ownerUserId: 'user-1', threadId: 'thread-1' }),
          method: 'POST',
        }),
        env,
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { ok: boolean; sent: number; suppressedByPreference: boolean }
    expect(body).toEqual({ invalidatedTokens: [], ok: true, sent: 1, suppressedByPreference: false })
  })

  test('a user with zero registered devices sends nothing (no error)', async () => {
    const env = makeEnv()
    const response = await Effect.runPromise(
      handlePushNotifyEventsRequest(
        makeDependencies({ adminOk: true, sessionUserId: undefined }),
        new Request(`https://openagents.com${PUSH_NOTIFY_EVENTS_PATH}`, {
          body: JSON.stringify({ kind: 'turn_completed', ownerUserId: 'user-with-no-devices', threadId: 'thread-1' }),
          method: 'POST',
        }),
        env,
      ),
    )
    const body = (await response.json()) as { sent: number }
    expect(body.sent).toBe(0)
  })

  test('respects the user\'s disabled preference — suppresses the send entirely', async () => {
    const env = makeEnv()
    await registerPushDeviceToken(env.db, {
      accessToken: 'a1',
      deviceId: 'device-1',
      expoPushToken: 'token-1',
      nowIso: '2026-07-06T00:00:00.000Z',
      platform: 'ios',
      userId: 'user-1',
    })
    // Disable via the preference route first.
    await Effect.runPromise(
      handlePushNotificationPreferencesRequest(
        makeDependencies({ adminOk: true, sessionUserId: 'user-1' }),
        new Request(`https://openagents.com${PUSH_NOTIFICATION_PREFERENCES_PATH}`, {
          body: JSON.stringify({ pushEnabled: false }),
          method: 'PUT',
        }),
        env,
        ctx,
      ),
    )

    const response = await Effect.runPromise(
      handlePushNotifyEventsRequest(
        makeDependencies({ adminOk: true, sessionUserId: undefined }),
        new Request(`https://openagents.com${PUSH_NOTIFY_EVENTS_PATH}`, {
          body: JSON.stringify({ kind: 'turn_completed', ownerUserId: 'user-1', threadId: 'thread-1' }),
          method: 'POST',
        }),
        env,
      ),
    )
    const body = (await response.json()) as { sent: number; suppressedByPreference: boolean }
    expect(body.sent).toBe(0)
    expect(body.suppressedByPreference).toBe(true)
  })
})

describe('handlePushNotificationPreferencesRequest', () => {
  test('401s without a valid mobile bearer session', async () => {
    const env = makeEnv()
    const response = await Effect.runPromise(
      handlePushNotificationPreferencesRequest(
        makeDependencies({ adminOk: false, sessionUserId: undefined }),
        new Request(`https://openagents.com${PUSH_NOTIFICATION_PREFERENCES_PATH}`, { method: 'GET' }),
        env,
        ctx,
      ),
    )
    expect(response.status).toBe(401)
  })

  test('GET defaults to enabled when never written', async () => {
    const env = makeEnv()
    const response = await Effect.runPromise(
      handlePushNotificationPreferencesRequest(
        makeDependencies({ adminOk: false, sessionUserId: 'user-1' }),
        new Request(`https://openagents.com${PUSH_NOTIFICATION_PREFERENCES_PATH}`, { method: 'GET' }),
        env,
        ctx,
      ),
    )
    const body = (await response.json()) as { preference: { pushEnabled: boolean } }
    expect(body.preference.pushEnabled).toBe(true)
  })

  test('PUT persists the toggle, and a later GET reflects it', async () => {
    const env = makeEnv()
    const deps = makeDependencies({ adminOk: false, sessionUserId: 'user-1' })

    await Effect.runPromise(
      handlePushNotificationPreferencesRequest(
        deps,
        new Request(`https://openagents.com${PUSH_NOTIFICATION_PREFERENCES_PATH}`, {
          body: JSON.stringify({ pushEnabled: false }),
          method: 'PUT',
        }),
        env,
        ctx,
      ),
    )

    const response = await Effect.runPromise(
      handlePushNotificationPreferencesRequest(
        deps,
        new Request(`https://openagents.com${PUSH_NOTIFICATION_PREFERENCES_PATH}`, { method: 'GET' }),
        env,
        ctx,
      ),
    )
    const body = (await response.json()) as { preference: { pushEnabled: boolean } }
    expect(body.preference.pushEnabled).toBe(false)
  })

  test('rejects a non-boolean pushEnabled on PUT', async () => {
    const env = makeEnv()
    const response = await Effect.runPromise(
      handlePushNotificationPreferencesRequest(
        makeDependencies({ adminOk: false, sessionUserId: 'user-1' }),
        new Request(`https://openagents.com${PUSH_NOTIFICATION_PREFERENCES_PATH}`, {
          body: JSON.stringify({ pushEnabled: 'yes' }),
          method: 'PUT',
        }),
        env,
        ctx,
      ),
    )
    expect(response.status).toBe(400)
  })

  test('an unsupported method is 405', async () => {
    const env = makeEnv()
    const response = await Effect.runPromise(
      handlePushNotificationPreferencesRequest(
        makeDependencies({ adminOk: false, sessionUserId: 'user-1' }),
        new Request(`https://openagents.com${PUSH_NOTIFICATION_PREFERENCES_PATH}`, { method: 'DELETE' }),
        env,
        ctx,
      ),
    )
    expect(response.status).toBe(405)
  })
})
