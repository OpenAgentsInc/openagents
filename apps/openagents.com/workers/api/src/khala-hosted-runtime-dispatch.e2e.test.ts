// End-to-end "send a message -> get an assistant reply" guard for the
// hosted_khala chat lane (issue #8510 hardening).
//
// WHY THIS FILE EXISTS. Three server-side bugs shipped to prod that ALL broke
// the same loop — a user sends a chat message and NO assistant reply ever
// comes back — yet every existing test stayed green because nothing asserted
// the REPLY:
//   1. A shared dispatch client group threw "client group is bound to a
//      different user" for every owner after the first, silently orphaning
//      their turns (fixed: owner-scoped group; commit eb6082258c).
//   2. Inference routed through the now-401 Cloudflare AI Gateway (fixed in
//      artanis-mind.ts — see its own regression test; commit 4135071a9b).
//   3. `khala_sync_runtime_control_intents.intent_json` is stored
//      DOUBLE-ENCODED (a JSON string inside jsonb), so the bodyRef never
//      resolved and every turn failed `prompt_unresolved` (fixed:
//      readTurnStartBodyRef parses the string form; commit 82e436cb7b).
//
// This suite drives a QUEUED hosted_khala turn through the real dispatch
// (`runHostedRuntimeTurnDispatch` / `dispatchHostedRuntimeTurn`) end to end and
// asserts an ACTUAL ASSISTANT REPLY is produced: turn.started -> text.delta
// (non-empty) -> text.completed -> turn.finished with finishReason !== "error"
// and the turn answered. It is FAIL-CLOSED: an empty reply or an errored turn
// is a red test, so it would have caught all three bugs (see the case comments
// below for exactly how each maps to a red).
//
// It reuses the dispatch's injectable seams (`complete`, `executePush`, fake
// `sql`) — no Postgres, no network — and, crucially, the fake `executePush`
// MODELS the real mutation ledger's cross-user client-group binding (the real
// engine THROWS KhalaSyncClientStateMismatchError when a client group is
// reused by a different user; see packages/khala-sync-server mutation-ledger).
// That is what makes the two-owners case a real regression guard for bug #1
// rather than a cosmetic assertion on a helper.

import { describe, expect, test } from 'vitest'

import type { PushResponse } from '@openagentsinc/khala-sync'
import type { SyncSql } from '@openagentsinc/khala-sync-server'

import {
  dispatchHostedRuntimeTurn,
  runHostedRuntimeTurnDispatch,
  type HostedRuntimeCompleteFn,
  type QueuedHostedTurn,
} from './khala-hosted-runtime-dispatch'

// Binds the enforced behavior contract asserting a sent message yields an
// assistant reply (coverage checker requires this id to appear in the oracle
// source): khala_sync.hosted_chat.send_yields_assistant_reply.v1

// --------------------------------------------------------------------------
// Fake Postgres: answers the three bounded reads the dispatch makes. The
// control-intent row's `intent_json` is returned EXACTLY as configured — an
// object OR a JSON-encoded string — so the double-encoding regression (bug #3)
// is exercised against the real resolver.
// --------------------------------------------------------------------------

type QueueRow = {
  turn_id: string
  thread_id: string
  owner_user_id: string
  event_count: number
}

type FakeTables = {
  queuedTurns: ReadonlyArray<QueueRow>
  /** turnId -> intent_json, stored as a real object OR a double-encoded JSON string. */
  startIntents: Record<string, unknown>
  /** messageId -> body. */
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
                author_user_id: 'github:author',
                body,
                created_at: '2026-07-07T00:00:00.000Z',
                deleted_at: null,
                message_id: messageId,
                thread_id: 'thread.t1',
                updated_at: '2026-07-07T00:00:00.000Z',
              },
            ],
      )
    }
    throw new Error(`unexpected SQL in fake: ${text}`)
  }
  return sql as unknown as SyncSql
}

// --------------------------------------------------------------------------
// Binding-aware executePush: mirrors the real mutation ledger's rule that a
// client group is bound to ONE user for its lifetime and THROWS on cross-user
// reuse. This is the seam that makes the two-owners case fail if the dispatch
// ever regresses to a single shared client group (bug #1).
// --------------------------------------------------------------------------

type RecordedEvent = {
  ownerUserId: string
  clientGroupId: string
  clientId: string
  mutationId: number
  kind: string
  text: string | undefined
  finishReason: string | undefined
}

const makeBindingAwareExecutePush = () => {
  const recorded: Array<RecordedEvent> = []
  // clientGroupId -> the first userId it was bound to. Real ledger throws
  // KhalaSyncClientStateMismatchError when a second user reuses the group.
  const groupOwner = new Map<string, string>()
  // (turnId|sequence) dedupe key so a duplicate claim is rejected in-band,
  // exactly like the real (turn_id, sequence) unique constraint.
  const recordedTurnSeq = new Set<string>()

  const executePush = (input: {
    readonly userId: string
    readonly request: {
      readonly clientGroupId: string
      readonly clientId: string
      readonly mutations: ReadonlyArray<{ mutationId: number; argsJson: string }>
    }
  }): Promise<PushResponse> => {
    const { clientGroupId, clientId } = input.request
    const boundTo = groupOwner.get(clientGroupId)
    if (boundTo !== undefined && boundTo !== input.userId) {
      // Same class as the real KhalaSyncClientStateMismatchError. A shared
      // (non-owner-scoped) client group hits this for the SECOND owner.
      return Promise.reject(
        new Error(
          'client group is bound to a different user; ' +
            'client groups never migrate between users',
        ),
      )
    }
    groupOwner.set(clientGroupId, input.userId)

    const envelope = input.request.mutations[0]!
    const event = JSON.parse(envelope.argsJson) as {
      kind: string
      text?: string
      finishReason?: string
      turnId?: string
      sequence?: number
    }
    const dedupeKey = `${event.turnId ?? '?'}|${event.sequence ?? -1}`
    const isDuplicate = recordedTurnSeq.has(dedupeKey)
    if (!isDuplicate) recordedTurnSeq.add(dedupeKey)

    recorded.push({
      clientGroupId,
      clientId,
      finishReason: event.finishReason,
      kind: event.kind,
      mutationId: envelope.mutationId,
      ownerUserId: input.userId,
      text: event.text,
    })
    return Promise.resolve({
      lastMutationId: envelope.mutationId,
      protocolVersion: 1,
      results: [
        {
          mutationId: envelope.mutationId,
          status: isDuplicate ? 'rejected' : 'applied',
        },
      ],
    } as unknown as PushResponse)
  }
  return { executePush: executePush as never, groupOwner, recorded }
}

let uuidCounter = 0
const deterministicUuid = () => `uuid-${(uuidCounter += 1)}`

const okComplete = (text: string): HostedRuntimeCompleteFn => () =>
  Promise.resolve({ ok: true, text })

const baseDeps = (
  tables: FakeTables,
  push: ReturnType<typeof makeBindingAwareExecutePush>,
  complete: HostedRuntimeCompleteFn,
) => ({
  complete,
  executePush: push.executePush,
  now: () => '2026-07-07T00:00:00.000Z',
  sql: makeFakeSql(tables),
  uuid: deterministicUuid,
})

/**
 * The core oracle: prove the recorded events for `turnId` constitute a REAL
 * assistant reply. Fail-closed — empty text or an errored/absent finish is a
 * red test. This is the assertion all three prod bugs would have tripped.
 */
const assertAssistantReplied = (
  recorded: ReadonlyArray<RecordedEvent>,
  turnId: string,
  expectedReply: string,
) => {
  const kinds = recorded.map(r => r.kind)
  expect(kinds).toContain('turn.started')
  const delta = recorded.find(r => r.kind === 'text.delta')
  // A reply actually exists and carries non-empty assistant text.
  expect(delta).toBeDefined()
  expect(delta?.text ?? '').not.toBe('')
  expect(delta?.text).toBe(expectedReply)
  expect(kinds).toContain('text.completed')
  const finished = recorded.find(r => r.kind === 'turn.finished')
  expect(finished).toBeDefined()
  // The turn settled successfully — NOT the finishReason:"error" orphan the
  // three bugs produced.
  expect(finished?.finishReason).toBe('stop')
  expect(finished?.finishReason).not.toBe('error')
}

const ownerA = 'github:14167547'
const ownerB = 'user_b02c2298'

const singleTurnTables = (intentJson: unknown): FakeTables => ({
  chatMessages: { 'msg.1': 'What is the capital of France? One word.' },
  queuedTurns: [
    { event_count: 0, owner_user_id: ownerA, thread_id: 'thread.t1', turn_id: 'turn.t1' },
  ],
  startIntents: { 'turn.t1': intentJson },
})

describe('hosted chat send -> assistant reply E2E guard (khala_sync.hosted_chat.send_yields_assistant_reply.v1)', () => {
  // (a) HAPPY PATH — a sent message yields an assistant reply.
  test('(a) a queued turn is driven end to end and produces an assistant reply', async () => {
    const push = makeBindingAwareExecutePush()
    const summary = await runHostedRuntimeTurnDispatch(
      baseDeps(singleTurnTables({ bodyRef: 'chat_message.msg.1' }), push, okComplete('Paris')),
    )
    expect(summary).toEqual({ answered: 1, claimed: 1, failed: 0, scanned: 1, skipped: 0 })
    assertAssistantReplied(push.recorded, 'turn.t1', 'Paris')
  })

  // (b) BUG #3 REGRESSION — double-encoded intent_json (a JSON string inside
  // jsonb) must STILL resolve the prompt and answer. Runs BOTH encodings: the
  // object form (what the intent_json writer fix produces for new rows) AND the
  // string form (legacy/double-encoded rows). Both MUST answer identically, so
  // this guard survives the concurrent writer fix either way. If the resolver
  // ever stops parsing the string form, the string case goes prompt_unresolved
  // -> finished(error) and assertAssistantReplied fails.
  for (const encoding of ['object', 'double-encoded string'] as const) {
    test(`(b) intent_json as ${encoding} still resolves the prompt and replies`, async () => {
      const intentJson =
        encoding === 'object'
          ? { bodyRef: 'chat_message.msg.1' }
          : JSON.stringify({ bodyRef: 'chat_message.msg.1' })
      const push = makeBindingAwareExecutePush()
      const summary = await runHostedRuntimeTurnDispatch(
        baseDeps(singleTurnTables(intentJson), push, okComplete('Paris')),
      )
      expect(summary.answered).toBe(1)
      expect(summary.failed).toBe(0)
      assertAssistantReplied(push.recorded, 'turn.t1', 'Paris')
    })
  }

  // (c) BUG #1 REGRESSION — two DIFFERENT owners both get answered. The
  // binding-aware fake throws on cross-user client-group reuse exactly like the
  // real ledger; if the dispatch regressed to one shared client group, ownerB's
  // turn.started claim would throw and NEVER answer, so `answered` would be 1
  // (not 2) and ownerB's reply would be absent — a red test.
  test('(c) two different owners are BOTH answered without a client-group collision', async () => {
    const tables: FakeTables = {
      chatMessages: { 'msg.a': 'Prompt from owner A', 'msg.b': 'Prompt from owner B' },
      queuedTurns: [
        { event_count: 0, owner_user_id: ownerA, thread_id: 'thread.a', turn_id: 'turn.a' },
        { event_count: 0, owner_user_id: ownerB, thread_id: 'thread.b', turn_id: 'turn.b' },
      ],
      startIntents: {
        'turn.a': { bodyRef: 'chat_message.msg.a' },
        // deliberately the double-encoded form on one of them too.
        'turn.b': JSON.stringify({ bodyRef: 'chat_message.msg.b' }),
      },
    }
    const push = makeBindingAwareExecutePush()
    const complete: HostedRuntimeCompleteFn = input =>
      Promise.resolve({ ok: true, text: `reply to: ${input.prompt}` })

    const summary = await runHostedRuntimeTurnDispatch(baseDeps(tables, push, complete))

    // BOTH owners answered — the crux of the #1 regression.
    expect(summary).toEqual({ answered: 2, claimed: 2, failed: 0, scanned: 2, skipped: 0 })
    assertAssistantReplied(
      push.recorded.filter(r => r.ownerUserId === ownerA),
      'turn.a',
      'reply to: Prompt from owner A',
    )
    assertAssistantReplied(
      push.recorded.filter(r => r.ownerUserId === ownerB),
      'turn.b',
      'reply to: Prompt from owner B',
    )
    // Each owner rode their OWN client group (no shared group), and no group
    // was ever bound to two users.
    const groupA = new Set(
      push.recorded.filter(r => r.ownerUserId === ownerA).map(r => r.clientGroupId),
    )
    const groupB = new Set(
      push.recorded.filter(r => r.ownerUserId === ownerB).map(r => r.clientGroupId),
    )
    expect(groupA.size).toBe(1)
    expect(groupB.size).toBe(1)
    expect([...groupA][0]).not.toBe([...groupB][0])
    for (const [group, owner] of push.groupOwner.entries()) {
      expect(group).toContain(owner)
    }
  })

  // (d) INFERENCE-ERROR PATH — when inference fails (the shape bug #2 produced
  // when the 401 gateway made the mind unavailable), the turn MUST still settle
  // as a terminal turn.finished(error) — never a silent orphan the client spins
  // on forever. No assistant reply is claimed.
  test('(d) an inference failure records a terminal turn.finished(error), never a silent orphan', async () => {
    const push = makeBindingAwareExecutePush()
    const outcome = await dispatchHostedRuntimeTurn(
      baseDeps(singleTurnTables({ bodyRef: 'chat_message.msg.1' }), push, () =>
        Promise.resolve({ detail: 'artanis_mind_unavailable', ok: false }),
      ),
      { eventCount: 0, ownerUserId: ownerA, threadId: 'thread.t1', turnId: 'turn.t1' } satisfies QueuedHostedTurn,
    )
    expect(outcome).toBe('failed')
    const kinds = push.recorded.map(r => r.kind)
    expect(kinds).toEqual(['turn.started', 'turn.finished'])
    // Terminal settlement exists (client stops spinning) but it is honestly an
    // error — NOT a fabricated reply.
    const finished = push.recorded.find(r => r.kind === 'turn.finished')
    expect(finished?.finishReason).toBe('error')
    expect(push.recorded.some(r => r.kind === 'text.delta')).toBe(false)
  })

  // Guard-of-the-guard: prove the oracle is FAIL-CLOSED. An empty assistant
  // reply (the symptom common to all three bugs) must make assertAssistantReplied
  // throw — otherwise a broken loop could pass silently, which is exactly how
  // the prod bugs escaped.
  test('the reply oracle is fail-closed: an empty reply is a red test', async () => {
    const push = makeBindingAwareExecutePush()
    await runHostedRuntimeTurnDispatch(
      baseDeps(singleTurnTables({ bodyRef: 'chat_message.msg.1' }), push, okComplete('')),
    )
    // Empty completion emits only started + finished(stop) — no text.delta.
    expect(() => assertAssistantReplied(push.recorded, 'turn.t1', 'Paris')).toThrow()
  })
})
