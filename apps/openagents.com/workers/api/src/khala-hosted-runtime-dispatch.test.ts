import { describe, expect, test } from 'vitest'

import type { PushResponse } from '@openagentsinc/khala-sync'
import type { SyncSql } from '@openagentsinc/khala-sync-server'

import {
  DEFAULT_HOSTED_RUNTIME_SYSTEM_PROMPT,
  HOSTED_RUNTIME_DISPATCH_CLIENT_GROUP_ID,
  dispatchHostedRuntimeTurn,
  hostedRuntimeDispatchClientGroupIdForOwner,
  readQueuedHostedTurns,
  resolveHostedTurnPrompt,
  runHostedRuntimeTurnDispatch,
  type HostedRuntimeCompleteFn,
  type QueuedHostedTurn,
} from './khala-hosted-runtime-dispatch'

// Regression for the "client group is bound to a different user" bug that
// silently orphaned every hosted chat turn after the first owner (same class
// as the #8477 writeback recorder). The dispatch client group MUST be
// owner-scoped so distinct owners never collide on one mutation-ledger group.
describe('hostedRuntimeDispatchClientGroupIdForOwner', () => {
  test('scopes the client group per owner and stays under the base prefix', () => {
    const a = hostedRuntimeDispatchClientGroupIdForOwner('github:300914913')
    const b = hostedRuntimeDispatchClientGroupIdForOwner('user_b02c2298')
    expect(a).toBe(`${HOSTED_RUNTIME_DISPATCH_CLIENT_GROUP_ID}.github:300914913`)
    expect(a).not.toBe(b)
    expect(a.startsWith(HOSTED_RUNTIME_DISPATCH_CLIENT_GROUP_ID)).toBe(true)
    expect(hostedRuntimeDispatchClientGroupIdForOwner('github:300914913')).toBe(a)
  })
})

// --------------------------------------------------------------------------
// Fakes: a tagged-template SQL that answers the three bounded reads by
// keyword, and a recording executePush that never touches a database.
// --------------------------------------------------------------------------

type QueueRow = {
  turn_id: string
  thread_id: string
  owner_user_id: string
  event_count: number
}

type FakeTables = {
  queuedTurns: ReadonlyArray<QueueRow>
  /** turnId -> intent_json (or absent). */
  startIntents: Record<string, unknown>
  /** messageId -> body (or absent). */
  chatMessages: Record<string, string>
}

const makeFakeSql = (tables: FakeTables): SyncSql => {
  const sql = (strings: TemplateStringsArray, ...values: Array<unknown>) => {
    const text = strings.join(' ')
    if (text.includes('FROM khala_sync_runtime_turns') && text.includes('status')) {
      return Promise.resolve([...tables.queuedTurns])
    }
    if (text.includes('FROM khala_sync_runtime_control_intents')) {
      const turnId = values[0] as string
      const intentJson = tables.startIntents[turnId]
      return Promise.resolve(
        intentJson === undefined ? [] : [{ intent_json: intentJson }],
      )
    }
    if (text.includes('FROM khala_sync_chat_messages')) {
      const messageId = values[0] as string
      const body = tables.chatMessages[messageId]
      return Promise.resolve(
        body === undefined
          ? []
          : [
              {
                author_user_id: 'github:1',
                body,
                created_at: '2026-07-06T00:00:00.000Z',
                deleted_at: null,
                message_id: messageId,
                thread_id: 'thread.t1',
                updated_at: '2026-07-06T00:00:00.000Z',
              },
            ],
      )
    }
    throw new Error(`unexpected SQL in fake: ${text}`)
  }
  return sql as unknown as SyncSql
}

type RecordedEvent = {
  userId: string
  mutationId: number
  kind: string
  text: string | undefined
  finishReason: string | undefined
  clientId: string
}

const makeRecordingExecutePush = (
  behavior: (event: RecordedEvent) => 'applied' | 'rejected' = () => 'applied',
) => {
  const recorded: Array<RecordedEvent> = []
  const executePush = (input: {
    readonly userId: string
    readonly request: {
      readonly clientId: string
      readonly mutations: ReadonlyArray<{ mutationId: number; argsJson: string }>
    }
  }): Promise<PushResponse> => {
    const envelope = input.request.mutations[0]!
    const event = JSON.parse(envelope.argsJson) as {
      kind: string
      text?: string
      finishReason?: string
    }
    const rec: RecordedEvent = {
      clientId: input.request.clientId,
      finishReason: event.finishReason,
      kind: event.kind,
      mutationId: envelope.mutationId,
      text: event.text,
      userId: input.userId,
    }
    recorded.push(rec)
    const status = behavior(rec)
    return Promise.resolve({
      lastMutationId: envelope.mutationId,
      protocolVersion: 1,
      results: [{ mutationId: envelope.mutationId, status }],
    } as unknown as PushResponse)
  }
  return { executePush: executePush as never, recorded }
}

const okComplete = (text: string): HostedRuntimeCompleteFn => () =>
  Promise.resolve({ ok: true, text })

let uuidCounter = 0
const deterministicUuid = () => `uuid-${(uuidCounter += 1)}`

const baseDeps = (
  tables: FakeTables,
  push: ReturnType<typeof makeRecordingExecutePush>,
  complete: HostedRuntimeCompleteFn,
) => ({
  complete,
  executePush: push.executePush,
  now: () => '2026-07-06T00:00:00.000Z',
  sql: makeFakeSql(tables),
  uuid: deterministicUuid,
})

const oneQueuedTurn: FakeTables = {
  chatMessages: { 'msg.1': 'Explain this codebase' },
  queuedTurns: [
    {
      event_count: 0,
      owner_user_id: 'github:14167547',
      thread_id: 'thread.t1',
      turn_id: 'turn.t1',
    },
  ],
  startIntents: { 'turn.t1': { bodyRef: 'chat_message.msg.1' } },
}

describe('readQueuedHostedTurns', () => {
  test('maps queued hosted turn rows with numeric event_count', async () => {
    const turns = await readQueuedHostedTurns(makeFakeSql(oneQueuedTurn), 8)
    expect(turns).toEqual([
      {
        eventCount: 0,
        ownerUserId: 'github:14167547',
        threadId: 'thread.t1',
        turnId: 'turn.t1',
      },
    ])
  })
})

describe('resolveHostedTurnPrompt', () => {
  const turn: QueuedHostedTurn = {
    eventCount: 0,
    ownerUserId: 'github:14167547',
    threadId: 'thread.t1',
    turnId: 'turn.t1',
  }

  test('resolves the prompt via bodyRef -> chat_message body', async () => {
    const prompt = await resolveHostedTurnPrompt(makeFakeSql(oneQueuedTurn), turn)
    expect(prompt).toBe('Explain this codebase')
  })

  test('null when the start intent has no bodyRef', async () => {
    const prompt = await resolveHostedTurnPrompt(
      makeFakeSql({ ...oneQueuedTurn, startIntents: { 'turn.t1': {} } }),
      turn,
    )
    expect(prompt).toBeNull()
  })

  test('null when the referenced chat_message is missing', async () => {
    const prompt = await resolveHostedTurnPrompt(
      makeFakeSql({ ...oneQueuedTurn, chatMessages: {} }),
      turn,
    )
    expect(prompt).toBeNull()
  })
})

describe('dispatchHostedRuntimeTurn', () => {
  const turn: QueuedHostedTurn = {
    eventCount: 0,
    ownerUserId: 'github:14167547',
    threadId: 'thread.t1',
    turnId: 'turn.t1',
  }

  test('happy path: started -> text.delta -> text.completed -> finished(stop)', async () => {
    const push = makeRecordingExecutePush()
    const outcome = await dispatchHostedRuntimeTurn(
      baseDeps(oneQueuedTurn, push, okComplete('Here is the answer.')),
      turn,
    )
    expect(outcome).toBe('answered')
    expect(push.recorded.map(r => r.kind)).toEqual([
      'turn.started',
      'text.delta',
      'text.completed',
      'turn.finished',
    ])
    // mutationIds are dense per attempt (1..4).
    expect(push.recorded.map(r => r.mutationId)).toEqual([1, 2, 3, 4])
    // every event recorded AS THE TURN OWNER.
    expect(push.recorded.every(r => r.userId === 'github:14167547')).toBe(true)
    // one stable clientId per dispatch attempt, in the server group.
    const clientIds = new Set(push.recorded.map(r => r.clientId))
    expect(clientIds.size).toBe(1)
    expect([...clientIds][0]).toContain(HOSTED_RUNTIME_DISPATCH_CLIENT_GROUP_ID)
    // the assistant text is delivered in the text.delta.
    const delta = push.recorded.find(r => r.kind === 'text.delta')
    expect(delta?.text).toBe('Here is the answer.')
    const finished = push.recorded.find(r => r.kind === 'turn.finished')
    expect(finished?.finishReason).toBe('stop')
  })

  test('inference failure still settles the turn as finished(error)', async () => {
    const push = makeRecordingExecutePush()
    const outcome = await dispatchHostedRuntimeTurn(
      baseDeps(oneQueuedTurn, push, () =>
        Promise.resolve({ detail: 'mind_unavailable', ok: false }),
      ),
      turn,
    )
    expect(outcome).toBe('failed')
    expect(push.recorded.map(r => r.kind)).toEqual(['turn.started', 'turn.finished'])
    expect(push.recorded[1]?.finishReason).toBe('error')
  })

  test('unresolved prompt settles as finished(error) without calling inference', async () => {
    const push = makeRecordingExecutePush()
    let completeCalls = 0
    const complete: HostedRuntimeCompleteFn = () => {
      completeCalls += 1
      return Promise.resolve({ ok: true, text: 'x' })
    }
    const outcome = await dispatchHostedRuntimeTurn(
      baseDeps({ ...oneQueuedTurn, chatMessages: {} }, push, complete),
      turn,
    )
    expect(outcome).toBe('failed')
    expect(completeCalls).toBe(0)
    expect(push.recorded.map(r => r.kind)).toEqual(['turn.started', 'turn.finished'])
    expect(push.recorded[1]?.finishReason).toBe('error')
  })

  test('a rejected claim (lost race) skips inference and further writes', async () => {
    let completeCalls = 0
    const complete: HostedRuntimeCompleteFn = () => {
      completeCalls += 1
      return Promise.resolve({ ok: true, text: 'x' })
    }
    // First recordEvent (the turn.started claim) is rejected.
    const push = makeRecordingExecutePush(event =>
      event.kind === 'turn.started' ? 'rejected' : 'applied',
    )
    const outcome = await dispatchHostedRuntimeTurn(
      baseDeps(oneQueuedTurn, push, complete),
      turn,
    )
    expect(outcome).toBe('skipped')
    expect(completeCalls).toBe(0)
    expect(push.recorded.map(r => r.kind)).toEqual(['turn.started'])
  })

  test('empty answer text yields only started + finished(stop)', async () => {
    const push = makeRecordingExecutePush()
    const outcome = await dispatchHostedRuntimeTurn(
      baseDeps(oneQueuedTurn, push, okComplete('')),
      turn,
    )
    expect(outcome).toBe('answered')
    expect(push.recorded.map(r => r.kind)).toEqual(['turn.started', 'turn.finished'])
    expect(push.recorded[1]?.finishReason).toBe('stop')
  })
})

describe('runHostedRuntimeTurnDispatch', () => {
  test('tallies a batch and is failure-isolated across turns', async () => {
    const tables: FakeTables = {
      chatMessages: { 'msg.a': 'Prompt A', 'msg.b': 'Prompt B' },
      queuedTurns: [
        {
          event_count: 0,
          owner_user_id: 'github:1',
          thread_id: 'thread.a',
          turn_id: 'turn.a',
        },
        {
          event_count: 0,
          owner_user_id: 'github:1',
          thread_id: 'thread.b',
          turn_id: 'turn.b',
        },
      ],
      startIntents: {
        'turn.a': { bodyRef: 'chat_message.msg.a' },
        'turn.b': { bodyRef: 'chat_message.msg.b' },
      },
    }
    const push = makeRecordingExecutePush()
    // turn.b's inference fails; turn.a succeeds.
    const complete: HostedRuntimeCompleteFn = input =>
      input.prompt === 'Prompt B'
        ? Promise.resolve({ detail: 'boom', ok: false })
        : Promise.resolve({ ok: true, text: 'answer A' })

    const summary = await runHostedRuntimeTurnDispatch(
      baseDeps(tables, push, complete),
    )
    expect(summary).toEqual({
      answered: 1,
      claimed: 2,
      failed: 1,
      scanned: 2,
      skipped: 0,
    })
  })

  test('empty queue is a clean no-op', async () => {
    const push = makeRecordingExecutePush()
    const summary = await runHostedRuntimeTurnDispatch(
      baseDeps(
        { chatMessages: {}, queuedTurns: [], startIntents: {} },
        push,
        okComplete('x'),
      ),
    )
    expect(summary).toEqual({
      answered: 0,
      claimed: 0,
      failed: 0,
      scanned: 0,
      skipped: 0,
    })
    expect(push.recorded).toHaveLength(0)
  })

  test('exports a default system prompt', () => {
    expect(DEFAULT_HOSTED_RUNTIME_SYSTEM_PROMPT.length).toBeGreaterThan(0)
  })
})
