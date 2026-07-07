import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  handlePushDeviceTokensRequest,
  PUSH_DEVICE_TOKENS_PATH,
  type PushDeviceTokenRouteDependencies,
} from './push-device-token-routes'
import { listPushDeviceTokensForUser, type PushDeviceTokenDb } from './push-device-tokens'
import { makeLedgerSqliteDb } from '../test/payments-ledger-sqlite'

type FakeEnv = Readonly<{ db: PushDeviceTokenDb }>
type FakeUser = Readonly<{ userId: string }>

// CFG-4 Domain 4 (#8519): Postgres-authoritative behind the `PaymentsLedgerDb`
// seam; tests back it with the SQLite ledger adapter.
const PUSH_DEVICE_TOKENS_SQLITE_SCHEMA = `
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

const makeEnv = (): FakeEnv => ({ db: makeLedgerSqliteDb(PUSH_DEVICE_TOKENS_SQLITE_SCHEMA) })

const makeDependencies = (
  sessionUserId: string | undefined,
): PushDeviceTokenRouteDependencies<FakeEnv, FakeUser> => ({
  db: env => env.db,
  nowIso: () => '2026-07-05T00:00:00.000Z',
  readBearerToken: request => request.headers.get('x-test-bearer') ?? undefined,
  requireUserBearerSession: async () =>
    sessionUserId === undefined ? undefined : { user: { userId: sessionUserId } },
  userIdFromSession: session => session.user.userId,
})

const ctx = {} as ExecutionContext

const run = (
  dependencies: PushDeviceTokenRouteDependencies<FakeEnv, FakeUser>,
  request: Request,
  env: FakeEnv,
): Promise<Response> =>
  Effect.runPromise(handlePushDeviceTokensRequest(dependencies, request, env, ctx))

describe('handlePushDeviceTokensRequest — auth gating', () => {
  test('POST without a valid mobile bearer session is 401', async () => {
    const env = makeEnv()
    const response = await run(
      makeDependencies(undefined),
      new Request(`https://openagents.com${PUSH_DEVICE_TOKENS_PATH}`, {
        body: JSON.stringify({ deviceId: 'd1', expoPushToken: 't1', platform: 'ios' }),
        method: 'POST',
      }),
      env,
    )
    expect(response.status).toBe(401)
  })

  test('DELETE without a valid mobile bearer session is 401', async () => {
    const env = makeEnv()
    const response = await run(
      makeDependencies(undefined),
      new Request(`https://openagents.com${PUSH_DEVICE_TOKENS_PATH}?deviceId=d1`, { method: 'DELETE' }),
      env,
    )
    expect(response.status).toBe(401)
  })

  test('an unsupported method is 405 with an Allow header', async () => {
    const env = makeEnv()
    const response = await run(
      makeDependencies('user-1'),
      new Request(`https://openagents.com${PUSH_DEVICE_TOKENS_PATH}`, { method: 'GET' }),
      env,
    )
    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('POST, DELETE')
  })
})

describe('handlePushDeviceTokensRequest — register (POST)', () => {
  test('registers a device token for the authenticated user', async () => {
    const env = makeEnv()
    const request = new Request(`https://openagents.com${PUSH_DEVICE_TOKENS_PATH}`, {
      body: JSON.stringify({ deviceId: 'device-1', expoPushToken: 'ExponentPushToken[abc]', platform: 'ios' }),
      headers: { 'x-test-bearer': 'access-token-1' },
      method: 'POST',
    })
    const response = await run(makeDependencies('user-1'), request, env)

    expect(response.status).toBe(200)
    const body = (await response.json()) as { ok: boolean; registration: { deviceId: string; platform: string } }
    expect(body.ok).toBe(true)
    expect(body.registration.deviceId).toBe('device-1')
    expect(body.registration.platform).toBe('ios')

    const rows = await listPushDeviceTokensForUser(env.db, 'user-1')
    expect(rows).toHaveLength(1)
    expect(rows[0]?.expoPushToken).toBe('ExponentPushToken[abc]')
  })

  test('rejects an invalid platform', async () => {
    const env = makeEnv()
    const request = new Request(`https://openagents.com${PUSH_DEVICE_TOKENS_PATH}`, {
      body: JSON.stringify({ deviceId: 'device-1', expoPushToken: 'token', platform: 'windows_phone' }),
      headers: { 'x-test-bearer': 'access-token-1' },
      method: 'POST',
    })
    const response = await run(makeDependencies('user-1'), request, env)
    expect(response.status).toBe(400)
  })

  test('rejects a missing deviceId', async () => {
    const env = makeEnv()
    const request = new Request(`https://openagents.com${PUSH_DEVICE_TOKENS_PATH}`, {
      body: JSON.stringify({ expoPushToken: 'token', platform: 'ios' }),
      headers: { 'x-test-bearer': 'access-token-1' },
      method: 'POST',
    })
    const response = await run(makeDependencies('user-1'), request, env)
    expect(response.status).toBe(400)
  })

  test('401s when the bearer session is valid but the raw access token cannot be read (defense in depth)', async () => {
    const env = makeEnv()
    const request = new Request(`https://openagents.com${PUSH_DEVICE_TOKENS_PATH}`, {
      body: JSON.stringify({ deviceId: 'device-1', expoPushToken: 'token', platform: 'ios' }),
      method: 'POST',
    })
    const response = await run(makeDependencies('user-1'), request, env)
    expect(response.status).toBe(401)
  })

  test('re-registering the same device (idempotent client retry) upserts, never duplicates', async () => {
    const env = makeEnv()
    const makeRequest = (token: string) =>
      new Request(`https://openagents.com${PUSH_DEVICE_TOKENS_PATH}`, {
        body: JSON.stringify({ deviceId: 'device-1', expoPushToken: token, platform: 'ios' }),
        headers: { 'x-test-bearer': 'access-token-1' },
        method: 'POST',
      })

    await run(makeDependencies('user-1'), makeRequest('token-a'), env)
    await run(makeDependencies('user-1'), makeRequest('token-b'), env)

    const rows = await listPushDeviceTokensForUser(env.db, 'user-1')
    expect(rows).toHaveLength(1)
    expect(rows[0]?.expoPushToken).toBe('token-b')
  })
})

describe('handlePushDeviceTokensRequest — unregister (DELETE)', () => {
  test('unregisters a previously-registered device and reports removed:true', async () => {
    const env = makeEnv()
    await run(
      makeDependencies('user-1'),
      new Request(`https://openagents.com${PUSH_DEVICE_TOKENS_PATH}`, {
        body: JSON.stringify({ deviceId: 'device-1', expoPushToken: 'token', platform: 'ios' }),
        headers: { 'x-test-bearer': 'access-token-1' },
        method: 'POST',
      }),
      env,
    )

    const response = await run(
      makeDependencies('user-1'),
      new Request(`https://openagents.com${PUSH_DEVICE_TOKENS_PATH}?deviceId=device-1`, { method: 'DELETE' }),
      env,
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { ok: boolean; removed: boolean }
    expect(body).toEqual({ ok: true, removed: true })
    expect(await listPushDeviceTokensForUser(env.db, 'user-1')).toHaveLength(0)
  })

  test('one user can never unregister a different user\'s device', async () => {
    const env = makeEnv()
    await run(
      makeDependencies('user-1'),
      new Request(`https://openagents.com${PUSH_DEVICE_TOKENS_PATH}`, {
        body: JSON.stringify({ deviceId: 'shared-device-id', expoPushToken: 'token', platform: 'ios' }),
        headers: { 'x-test-bearer': 'access-token-1' },
        method: 'POST',
      }),
      env,
    )

    // A DIFFERENT authenticated user tries to unregister the same device id.
    const response = await run(
      makeDependencies('user-2'),
      new Request(`https://openagents.com${PUSH_DEVICE_TOKENS_PATH}?deviceId=shared-device-id`, { method: 'DELETE' }),
      env,
    )
    const body = (await response.json()) as { ok: boolean; removed: boolean }
    expect(body.removed).toBe(false)
    expect(await listPushDeviceTokensForUser(env.db, 'user-1')).toHaveLength(1)
  })

  test('missing deviceId query param is 400', async () => {
    const env = makeEnv()
    const response = await run(
      makeDependencies('user-1'),
      new Request(`https://openagents.com${PUSH_DEVICE_TOKENS_PATH}`, { method: 'DELETE' }),
      env,
    )
    expect(response.status).toBe(400)
  })
})
