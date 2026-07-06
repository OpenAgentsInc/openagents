import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import { registerPushDeviceToken, listPushDeviceTokensForUser } from './push-device-tokens'
import {
  buildExpoPushMessage,
  chunkExpoPushMessages,
  EXPO_PUSH_MAX_MESSAGES_PER_REQUEST,
  sendExpoPushMessages,
  type ExpoPushMessage,
} from './push-sender'
import type { PushNotificationPayload } from './push-notify-events'

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

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(schema)
  return new SqliteD1(raw) as unknown as D1Database
}

const payload: PushNotificationPayload = {
  body: 'Your task finished.',
  data: { deepLink: 'khala://thread/t1', kind: 'turn_completed', threadId: 't1' },
  title: 'Task finished',
}

describe('buildExpoPushMessage', () => {
  test('carries the payload straight through, addressed to the given token', () => {
    expect(buildExpoPushMessage('ExponentPushToken[abc]', payload)).toEqual({
      body: payload.body,
      data: payload.data,
      title: payload.title,
      to: 'ExponentPushToken[abc]',
    })
  })
})

describe('chunkExpoPushMessages', () => {
  test('splits into batches no larger than the given size', () => {
    const messages: Array<ExpoPushMessage> = Array.from({ length: 250 }, (_, i) =>
      buildExpoPushMessage(`token-${i}`, payload),
    )
    const chunks = chunkExpoPushMessages(messages, 100)
    expect(chunks.map(chunk => chunk.length)).toEqual([100, 100, 50])
  })

  test('defaults to Expo\'s documented per-request cap', () => {
    const messages: Array<ExpoPushMessage> = Array.from({ length: 150 }, (_, i) =>
      buildExpoPushMessage(`token-${i}`, payload),
    )
    const chunks = chunkExpoPushMessages(messages)
    expect(chunks[0]).toHaveLength(EXPO_PUSH_MAX_MESSAGES_PER_REQUEST)
  })
})

describe('sendExpoPushMessages', () => {
  test('sends a single batch and returns ok tickets in order', async () => {
    const db = makeDb()
    const messages = [buildExpoPushMessage('token-1', payload), buildExpoPushMessage('token-2', payload)]
    const calls: Array<unknown> = []
    const result = await sendExpoPushMessages(db, messages, async (url, init) => {
      calls.push({ body: JSON.parse(String(init.body)), url })
      return new Response(
        JSON.stringify({ data: [{ id: 'ticket-1', status: 'ok' }, { id: 'ticket-2', status: 'ok' }] }),
        { status: 200 },
      )
    })

    expect(result.tickets).toEqual([
      { id: 'ticket-1', status: 'ok' },
      { id: 'ticket-2', status: 'ok' },
    ])
    expect(result.invalidatedTokens).toEqual([])
    expect(calls).toHaveLength(1)
  })

  test('splits across TWO requests when messages exceed the per-request cap', async () => {
    const db = makeDb()
    const messages: Array<ExpoPushMessage> = Array.from({ length: 120 }, (_, i) =>
      buildExpoPushMessage(`token-${i}`, payload),
    )
    let requestCount = 0
    await sendExpoPushMessages(db, messages, async (_url, init) => {
      requestCount += 1
      const sent = JSON.parse(String(init.body)) as Array<unknown>
      return new Response(JSON.stringify({ data: sent.map(() => ({ id: 'x', status: 'ok' })) }), {
        status: 200,
      })
    })
    expect(requestCount).toBe(2)
  })

  test('prunes a device token whose ticket reports DeviceNotRegistered', async () => {
    const db = makeDb()
    await registerPushDeviceToken(db, {
      accessToken: 'a1',
      deviceId: 'device-1',
      expoPushToken: 'stale-token',
      nowIso: '2026-07-06T00:00:00.000Z',
      platform: 'ios',
      userId: 'user-1',
    })

    const messages = [buildExpoPushMessage('stale-token', payload)]
    const result = await sendExpoPushMessages(db, messages, async () =>
      new Response(
        JSON.stringify({
          data: [
            {
              details: { error: 'DeviceNotRegistered' },
              message: '"stale-token" is not a registered push notification recipient',
              status: 'error',
            },
          ],
        }),
        { status: 200 },
      ),
    )

    expect(result.invalidatedTokens).toEqual(['stale-token'])
    expect(await listPushDeviceTokensForUser(db, 'user-1')).toHaveLength(0)
  })

  test('a non-DeviceNotRegistered error ticket does NOT prune the token', async () => {
    const db = makeDb()
    await registerPushDeviceToken(db, {
      accessToken: 'a1',
      deviceId: 'device-1',
      expoPushToken: 'token-1',
      nowIso: '2026-07-06T00:00:00.000Z',
      platform: 'ios',
      userId: 'user-1',
    })

    const messages = [buildExpoPushMessage('token-1', payload)]
    const result = await sendExpoPushMessages(db, messages, async () =>
      new Response(
        JSON.stringify({ data: [{ details: { error: 'MessageRateExceeded' }, message: 'rate', status: 'error' }] }),
        { status: 200 },
      ),
    )

    expect(result.invalidatedTokens).toEqual([])
    expect(await listPushDeviceTokensForUser(db, 'user-1')).toHaveLength(1)
  })

  test('a whole-batch HTTP failure returns error tickets for every message without throwing', async () => {
    const db = makeDb()
    const messages = [buildExpoPushMessage('token-1', payload), buildExpoPushMessage('token-2', payload)]
    const result = await sendExpoPushMessages(db, messages, async () => new Response('', { status: 500 }))

    expect(result.tickets).toHaveLength(2)
    expect(result.tickets.every(ticket => ticket.status === 'error')).toBe(true)
    expect(result.invalidatedTokens).toEqual([])
  })

  test('an empty message list is a no-op (never calls fetch)', async () => {
    const db = makeDb()
    let called = false
    const result = await sendExpoPushMessages(db, [], async () => {
      called = true
      return new Response('{}', { status: 200 })
    })
    expect(called).toBe(false)
    expect(result).toEqual({ invalidatedTokens: [], tickets: [] })
  })
})
