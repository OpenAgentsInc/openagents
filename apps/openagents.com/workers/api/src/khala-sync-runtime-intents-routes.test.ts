import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { decodeRuntimeControlIntentRow } from '@openagentsinc/khala-sync-server'
import type {
  ReadPendingRuntimeControlIntentsInput,
  RuntimeChatMessageRow,
  RuntimeControlIntentRow,
  RuntimeTurnRow,
  SyncSql,
} from '@openagentsinc/khala-sync-server'

import {
  KHALA_SYNC_CHAT_MESSAGE_READ_PATH,
  KHALA_SYNC_CHAT_MESSAGE_READ_ROUTE_REF,
  KHALA_SYNC_RUNTIME_INTENTS_PATH,
  KHALA_SYNC_RUNTIME_INTENTS_ROUTE_REF,
  KHALA_SYNC_RUNTIME_TURN_READ_PATH,
  KHALA_SYNC_RUNTIME_TURN_READ_ROUTE_REF,
  handleKhalaSyncChatMessageRead,
  handleKhalaSyncRuntimeIntents,
  handleKhalaSyncRuntimeTurnRead,
} from './khala-sync-runtime-intents-routes'

const FAKE_CONNECTION_STRING =
  'postgresql://user:secret@hyperdrive.local:5432/khala_sync_test'

const getIntents = (query = '') =>
  new Request(
    `https://openagents.com${KHALA_SYNC_RUNTIME_INTENTS_PATH}${query}`,
  )

const getMessage = (query = '') =>
  new Request(
    `https://openagents.com${KHALA_SYNC_CHAT_MESSAGE_READ_PATH}${query}`,
  )

const getRuntimeTurn = (query = '') =>
  new Request(
    `https://openagents.com${KHALA_SYNC_RUNTIME_TURN_READ_PATH}${query}`,
  )

const controlIntent = (
  seq: number,
  overrides: Record<string, unknown> = {},
): RuntimeControlIntentRow =>
  decodeRuntimeControlIntentRow({
    createdAt: '2026-07-05T15:20:11.412Z',
    intent: {
      causalityRefs: [],
      createdAt: '2026-07-05T15:20:11.412Z',
      idempotencyKey: 'idem.intent-1',
      intentId: 'intent-1',
      kind: 'turn.start',
      origin: { lane: 'khala_sync_mobile_control', surface: 'mobile' },
      redactionClass: 'private_ref',
      schema: 'openagents.khala_runtime_control_intent.v1',
      target: { lane: 'codex_app_server' },
      threadId: 'thread-1',
      turnId: 'turn-1',
      visibility: 'private',
      bodyRef: 'chat_message.msg-1',
    },
    intentId: 'intent-1',
    kind: 'turn.start',
    ownerUserId: 'user-1',
    seq,
    status: 'accepted',
    threadId: 'thread-1',
    turnId: 'turn-1',
    updatedAt: '2026-07-05T15:20:11.412Z',
    ...overrides,
  })

const runIntents = (
  input: Readonly<{
    authorized?: boolean
    binding?: { connectionString: string } | undefined
    request?: Request
    intents?: ReadonlyArray<RuntimeControlIntentRow>
    readError?: Error
    factoryError?: Error
  }> = {},
) => {
  const reads: Array<ReadPendingRuntimeControlIntentsInput> = []
  let ended = 0
  const response = Effect.runPromise(
    handleKhalaSyncRuntimeIntents(input.request ?? getIntents(), {
      binding:
        'binding' in input
          ? input.binding
          : { connectionString: FAKE_CONNECTION_STRING },
      makeSqlClient: connectionString => {
        expect(connectionString).toBe(FAKE_CONNECTION_STRING)
        if (input.factoryError !== undefined) {
          return Promise.reject(input.factoryError)
        }
        return Promise.resolve({
          end: () => {
            ended += 1
            return Promise.resolve()
          },
          sql: {} as SyncSql,
        })
      },
      readPendingRuntimeControlIntents: (_sql, readInput) => {
        reads.push(readInput)
        if (input.readError !== undefined) {
          return Promise.reject(input.readError)
        }
        return Promise.resolve(input.intents ?? [])
      },
      requireOperator: () => Promise.resolve(input.authorized ?? true),
    }),
  )
  return { endedCount: () => ended, reads, response }
}

const runMessage = (
  input: Readonly<{
    authorized?: boolean
    binding?: { connectionString: string } | undefined
    request?: Request
    message?: RuntimeChatMessageRow | null
    readError?: Error
    factoryError?: Error
  }> = {},
) => {
  const reads: Array<{ messageId: string; threadId?: string }> = []
  let ended = 0
  const response = Effect.runPromise(
    handleKhalaSyncChatMessageRead(input.request ?? getMessage('?messageId=msg-1'), {
      binding:
        'binding' in input
          ? input.binding
          : { connectionString: FAKE_CONNECTION_STRING },
      makeSqlClient: connectionString => {
        expect(connectionString).toBe(FAKE_CONNECTION_STRING)
        if (input.factoryError !== undefined) {
          return Promise.reject(input.factoryError)
        }
        return Promise.resolve({
          end: () => {
            ended += 1
            return Promise.resolve()
          },
          sql: {} as SyncSql,
        })
      },
      readChatMessageById: (_sql, readInput) => {
        reads.push(readInput)
        if (input.readError !== undefined) {
          return Promise.reject(input.readError)
        }
        return Promise.resolve(input.message === undefined ? null : input.message)
      },
      requireOperator: () => Promise.resolve(input.authorized ?? true),
    }),
  )
  return { endedCount: () => ended, reads, response }
}

describe('handleKhalaSyncRuntimeIntents', () => {
  test('rejects non-GET methods', async () => {
    const { response } = runIntents({
      request: new Request(
        `https://openagents.com${KHALA_SYNC_RUNTIME_INTENTS_PATH}`,
        { method: 'POST' },
      ),
    })
    expect((await response).status).toBe(405)
  })

  test('requires the admin bearer', async () => {
    const { reads, response } = runIntents({ authorized: false })
    const result = await response
    expect(result.status).toBe(401)
    expect(reads).toHaveLength(0)
  })

  test('binding absent: honest ok:false enablement gap, no read attempted', async () => {
    const { reads, response } = runIntents({ binding: undefined })
    const result = await response
    expect(result.status).toBe(200)
    const body = (await result.json()) as { ok: boolean; reason: string }
    expect(body.ok).toBe(false)
    expect(body.reason).toContain('KHALA_SYNC_DB')
    expect(reads).toHaveLength(0)
  })

  test('invalid ownerUserId / after / limit are typed 400s before any read', async () => {
    for (const query of [
      '?ownerUserId=not valid',
      '?after=-1',
      '?after=abc',
      '?limit=0',
      '?limit=nope',
    ]) {
      const { reads, response } = runIntents({ request: getIntents(query) })
      const result = await response
      expect(result.status).toBe(400)
      expect(reads).toHaveLength(0)
    }
  })

  test('returns intents oldest-first with a nextAfter watermark', async () => {
    const rows = [
      controlIntent(11),
      controlIntent(12, { intentId: 'intent-2', kind: 'turn.interrupt' }),
      controlIntent(13, { intentId: 'intent-3', kind: 'message.append' }),
    ]
    const { endedCount, reads, response } = runIntents({
      intents: rows,
      request: getIntents('?ownerUserId=user-1&after=10&limit=3'),
    })
    const result = await response
    expect(result.status).toBe(200)
    const body = (await result.json()) as {
      ok: boolean
      intents: Array<{ seq: number; kind: string }>
      nextAfter: number
      upToDate: boolean
      routeRef: string
    }
    expect(body.ok).toBe(true)
    expect(body.routeRef).toBe(KHALA_SYNC_RUNTIME_INTENTS_ROUTE_REF)
    expect(body.intents.map(i => i.seq)).toEqual([11, 12, 13])
    expect(body.intents[2]!.kind).toBe('message.append')
    expect(body.nextAfter).toBe(13)
    expect(body.upToDate).toBe(false)
    expect(reads).toEqual([{ afterSeq: 10, limit: 3, ownerUserId: 'user-1' }])
    expect(endedCount()).toBe(1)
  })

  test('empty page: nextAfter stays at the requested watermark, upToDate true', async () => {
    const { response } = runIntents({ intents: [], request: getIntents('?after=42') })
    const result = await response
    const body = (await result.json()) as { nextAfter: number; upToDate: boolean; ok: boolean }
    expect(body.ok).toBe(true)
    expect(body.nextAfter).toBe(42)
    expect(body.upToDate).toBe(true)
  })

  test('storage failure: 503 without echoing driver detail; client still torn down', async () => {
    const { endedCount, response } = runIntents({
      readError: new Error(`connect ECONNREFUSED at ${FAKE_CONNECTION_STRING}`),
    })
    const result = await response
    expect(result.status).toBe(503)
    const text = await result.text()
    expect(text).not.toContain('secret')
    expect(text).not.toContain('hyperdrive.local')
    expect(endedCount()).toBe(1)
  })

  test('client factory failure: 503 without echoing driver detail', async () => {
    const { response } = runIntents({
      factoryError: new Error(`auth failed for ${FAKE_CONNECTION_STRING}`),
    })
    const result = await response
    expect(result.status).toBe(503)
    expect(await result.text()).not.toContain('secret')
  })
})

const runRuntimeTurn = (
  input: Readonly<{
    authorized?: boolean
    binding?: { connectionString: string } | undefined
    request?: Request
    turn?: RuntimeTurnRow | null
    readError?: Error
    factoryError?: Error
  }> = {},
) => {
  const reads: Array<{ turnId: string }> = []
  let ended = 0
  const response = Effect.runPromise(
    handleKhalaSyncRuntimeTurnRead(input.request ?? getRuntimeTurn('?turnId=turn-1'), {
      binding:
        'binding' in input
          ? input.binding
          : { connectionString: FAKE_CONNECTION_STRING },
      makeSqlClient: connectionString => {
        expect(connectionString).toBe(FAKE_CONNECTION_STRING)
        if (input.factoryError !== undefined) {
          return Promise.reject(input.factoryError)
        }
        return Promise.resolve({
          end: () => {
            ended += 1
            return Promise.resolve()
          },
          sql: {} as SyncSql,
        })
      },
      readRuntimeTurnById: (_sql, readInput) => {
        reads.push(readInput)
        if (input.readError !== undefined) {
          return Promise.reject(input.readError)
        }
        return Promise.resolve(input.turn === undefined ? null : input.turn)
      },
      requireOperator: () => Promise.resolve(input.authorized ?? true),
    }),
  )
  return { endedCount: () => ended, reads, response }
}

describe('handleKhalaSyncRuntimeTurnRead', () => {
  test('rejects non-GET methods', async () => {
    const { response } = runRuntimeTurn({
      request: new Request(`https://openagents.com${KHALA_SYNC_RUNTIME_TURN_READ_PATH}`, {
        method: 'POST',
      }),
    })
    expect((await response).status).toBe(405)
  })

  test('requires the admin bearer', async () => {
    const { reads, response } = runRuntimeTurn({ authorized: false })
    expect((await response).status).toBe(401)
    expect(reads).toHaveLength(0)
  })

  test('requires a bounded turnId', async () => {
    const { reads, response } = runRuntimeTurn({ request: getRuntimeTurn('') })
    expect((await response).status).toBe(400)
    expect(reads).toHaveLength(0)
  })

  test('returns the turn when found, including its current event count', async () => {
    const turn: RuntimeTurnRow = {
      eventCount: 7,
      lane: 'codex_app_server',
      ownerUserId: 'user-1',
      status: 'failed',
      threadId: 'thread-1',
      turnId: 'turn-1',
    }
    const { reads, response } = runRuntimeTurn({ turn })
    const result = await response
    expect(result.status).toBe(200)
    const body = (await result.json()) as { ok: boolean; turn: RuntimeTurnRow; routeRef: string }
    expect(body.ok).toBe(true)
    expect(body.turn).toEqual(turn)
    expect(body.routeRef).toBe(KHALA_SYNC_RUNTIME_TURN_READ_ROUTE_REF)
    expect(reads).toEqual([{ turnId: 'turn-1' }])
  })

  test('returns ok:true, turn:null when the turn does not exist', async () => {
    const { response } = runRuntimeTurn({ turn: null })
    const result = await response
    const body = (await result.json()) as { ok: boolean; turn: null }
    expect(body.ok).toBe(true)
    expect(body.turn).toBeNull()
  })

  test('storage failure: 503 without echoing driver detail', async () => {
    const { response } = runRuntimeTurn({
      readError: new Error(`connect ECONNREFUSED at ${FAKE_CONNECTION_STRING}`),
    })
    const result = await response
    expect(result.status).toBe(503)
    expect(await result.text()).not.toContain('secret')
  })
})

describe('handleKhalaSyncChatMessageRead', () => {
  test('rejects non-GET methods', async () => {
    const { response } = runMessage({
      request: new Request(`https://openagents.com${KHALA_SYNC_CHAT_MESSAGE_READ_PATH}`, {
        method: 'POST',
      }),
    })
    expect((await response).status).toBe(405)
  })

  test('requires the admin bearer', async () => {
    const { reads, response } = runMessage({ authorized: false })
    expect((await response).status).toBe(401)
    expect(reads).toHaveLength(0)
  })

  test('requires a bounded messageId', async () => {
    const { reads, response } = runMessage({ request: getMessage('') })
    expect((await response).status).toBe(400)
    expect(reads).toHaveLength(0)
  })

  test('returns the message when found', async () => {
    const message: RuntimeChatMessageRow = {
      authorUserId: 'user-1',
      body: 'real prompt text',
      createdAt: '2026-07-05T15:20:11.412Z',
      deletedAt: null,
      messageId: 'msg-1',
      threadId: 'thread-1',
      updatedAt: '2026-07-05T15:20:11.412Z',
    }
    const { reads, response } = runMessage({
      message,
      request: getMessage('?messageId=msg-1&threadId=thread-1'),
    })
    const result = await response
    expect(result.status).toBe(200)
    const body = (await result.json()) as { ok: boolean; message: RuntimeChatMessageRow; routeRef: string }
    expect(body.ok).toBe(true)
    expect(body.message).toEqual(message)
    expect(body.routeRef).toBe(KHALA_SYNC_CHAT_MESSAGE_READ_ROUTE_REF)
    expect(reads).toEqual([{ messageId: 'msg-1', threadId: 'thread-1' }])
  })

  test('returns ok:true, message:null when the message does not exist', async () => {
    const { response } = runMessage({ message: null })
    const result = await response
    const body = (await result.json()) as { ok: boolean; message: null }
    expect(body.ok).toBe(true)
    expect(body.message).toBeNull()
  })

  test('storage failure: 503 without echoing driver detail', async () => {
    const { response } = runMessage({
      readError: new Error(`connect ECONNREFUSED at ${FAKE_CONNECTION_STRING}`),
    })
    const result = await response
    expect(result.status).toBe(503)
    expect(await result.text()).not.toContain('secret')
  })
})
