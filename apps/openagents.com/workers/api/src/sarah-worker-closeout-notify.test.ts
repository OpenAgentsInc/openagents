import type { PushResponse } from '@openagentsinc/khala-sync'
import type { SyncSql } from '@openagentsinc/khala-sync-server'
import { describe, expect, test } from 'vitest'

import type { MobileAccessRevocationStore } from './auth/mobile-session'
import type { KhalaSyncPushSqlClient, MakeKhalaSyncPushSqlClient } from './khala-sync-push-routes'
import type { PushDeviceTokenDb } from './push/push-device-tokens'
import { makeLedgerSqliteDb } from './test/payments-ledger-sqlite'
import {
  appendSarahWorkerCloseoutNoticeToThread,
  consumeSarahWorkerDispatchMapping,
  notifySarahWorkerCloseout,
  recordSarahWorkerDispatchMapping,
  sarahWorkerCloseoutNoticeText,
  sarahWorkerCloseoutOutcomeFromStatus,
} from './sarah-worker-closeout-notify'

// ---------------------------------------------------------------------------
// Pure classifiers
// ---------------------------------------------------------------------------

describe('sarahWorkerCloseoutOutcomeFromStatus', () => {
  test('maps the real Pylon wire vocabulary onto accepted/refused/failed', () => {
    expect(sarahWorkerCloseoutOutcomeFromStatus('closeout_submitted')).toBe('accepted')
    expect(sarahWorkerCloseoutOutcomeFromStatus('accepted')).toBe('accepted')
    expect(sarahWorkerCloseoutOutcomeFromStatus('rejected')).toBe('refused')
    expect(sarahWorkerCloseoutOutcomeFromStatus('cancelled')).toBe('failed')
    expect(sarahWorkerCloseoutOutcomeFromStatus('timed-out')).toBe('failed')
    expect(sarahWorkerCloseoutOutcomeFromStatus('stale')).toBe('failed')
  })

  test('an unrecognized status is reported failed, never silently accepted', () => {
    expect(sarahWorkerCloseoutOutcomeFromStatus('some_new_wire_status')).toBe('failed')
  })
})

describe('sarahWorkerCloseoutNoticeText', () => {
  test('every outcome cites the exact assignment ref', () => {
    for (const outcome of ['accepted', 'refused', 'failed'] as const) {
      const text = sarahWorkerCloseoutNoticeText({
        assignmentRef: 'assignment.public.pylon_api.abc123',
        outcome,
        status: 'rejected',
      })
      expect(text).toContain('assignment.public.pylon_api.abc123')
    }
  })

  test('distinguishes accepted/refused/failed with different wording', () => {
    const accepted = sarahWorkerCloseoutNoticeText({
      assignmentRef: 'assignment.a',
      outcome: 'accepted',
      status: 'closeout_submitted',
    })
    const refused = sarahWorkerCloseoutNoticeText({
      assignmentRef: 'assignment.a',
      outcome: 'refused',
      status: 'rejected',
    })
    const failed = sarahWorkerCloseoutNoticeText({
      assignmentRef: 'assignment.a',
      outcome: 'failed',
      status: 'stale',
    })
    expect(accepted).not.toBe(refused)
    expect(accepted).not.toBe(failed)
    expect(refused).not.toBe(failed)
    // Never claims completion for a non-accepted outcome.
    expect(refused.toLowerCase()).not.toContain('finished its run')
    expect(failed.toLowerCase()).not.toContain('finished its run')
  })
})

// ---------------------------------------------------------------------------
// Dispatch-mapping store (fake tagged-template SQL over an in-memory table)
// ---------------------------------------------------------------------------

type FakeMappingRow = {
  assignment_ref: string
  owner_user_id: string
  thread_ref: string
  dispatched_at: string
  consumed_at: string | null
}

const makeFakeMappingSql = (rows: Map<string, FakeMappingRow>): SyncSql => {
  const sql = (strings: TemplateStringsArray, ...values: ReadonlyArray<unknown>) => {
    const text = strings.join(' ')
    if (text.includes('INSERT INTO sarah_worker_dispatch_mappings')) {
      const [assignmentRef, ownerUserId, threadRef, dispatchedAt] = values as [
        string,
        string,
        string,
        string,
      ]
      if (!rows.has(assignmentRef)) {
        rows.set(assignmentRef, {
          assignment_ref: assignmentRef,
          consumed_at: null,
          dispatched_at: dispatchedAt,
          owner_user_id: ownerUserId,
          thread_ref: threadRef,
        })
      }
      return Promise.resolve([])
    }
    if (text.includes('UPDATE sarah_worker_dispatch_mappings')) {
      const [nowIso, assignmentRef] = values as [string, string]
      const row = rows.get(assignmentRef)
      if (row === undefined || row.consumed_at !== null) return Promise.resolve([])
      row.consumed_at = nowIso
      return Promise.resolve([{ owner_user_id: row.owner_user_id, thread_ref: row.thread_ref }])
    }
    throw new Error(`sarah-worker-closeout-notify.test.ts: unexpected query: ${text}`)
  }
  return sql as unknown as SyncSql
}

describe('recordSarahWorkerDispatchMapping / consumeSarahWorkerDispatchMapping', () => {
  test('a recorded mapping is consumed exactly once', async () => {
    const rows = new Map<string, FakeMappingRow>()
    const sql = makeFakeMappingSql(rows)
    await recordSarahWorkerDispatchMapping(sql, {
      assignmentRef: 'assignment.a',
      nowIso: '2026-07-19T00:00:00.000Z',
      ownerUserId: 'owner-1',
      threadRef: 'thread.sarah.a',
    })

    const first = await consumeSarahWorkerDispatchMapping(sql, {
      assignmentRef: 'assignment.a',
      nowIso: '2026-07-19T00:05:00.000Z',
    })
    expect(first).toEqual({ ownerUserId: 'owner-1', threadRef: 'thread.sarah.a' })

    const second = await consumeSarahWorkerDispatchMapping(sql, {
      assignmentRef: 'assignment.a',
      nowIso: '2026-07-19T00:06:00.000Z',
    })
    expect(second).toBeUndefined()
  })

  test('an assignment with no dispatch mapping consumes to undefined (safe no-op)', async () => {
    const sql = makeFakeMappingSql(new Map())
    const result = await consumeSarahWorkerDispatchMapping(sql, {
      assignmentRef: 'assignment.never_dispatched_by_sarah',
      nowIso: '2026-07-19T00:00:00.000Z',
    })
    expect(result).toBeUndefined()
  })

  test('a duplicate dispatch-mapping write for the same assignmentRef is a safe no-op', async () => {
    const rows = new Map<string, FakeMappingRow>()
    const sql = makeFakeMappingSql(rows)
    await recordSarahWorkerDispatchMapping(sql, {
      assignmentRef: 'assignment.a',
      nowIso: '2026-07-19T00:00:00.000Z',
      ownerUserId: 'owner-1',
      threadRef: 'thread.sarah.a',
    })
    await recordSarahWorkerDispatchMapping(sql, {
      assignmentRef: 'assignment.a',
      nowIso: '2026-07-19T01:00:00.000Z',
      ownerUserId: 'owner-DIFFERENT',
      threadRef: 'thread.sarah.DIFFERENT',
    })
    expect(rows.get('assignment.a')).toMatchObject({
      owner_user_id: 'owner-1',
      thread_ref: 'thread.sarah.a',
    })
  })
})

// ---------------------------------------------------------------------------
// Thread-notice turn synthesis (recording fake executePush, no real Postgres)
// ---------------------------------------------------------------------------

type RecordedMutation = Readonly<{ name: string; mutationId: number; args: Record<string, unknown> }>

const makeRecordingExecutePush = (statuses?: ReadonlyArray<'applied' | 'rejected'>) => {
  const recorded: Array<RecordedMutation> = []
  const requests: Array<Readonly<{ clientGroupId: string; clientId: string; userId: string }>> = []
  const executePush = (input: {
    registry: unknown
    request: { clientGroupId: string; clientId: string; mutations: ReadonlyArray<{ argsJson: string; mutationId: number; name: string }> }
    sql: unknown
    userId: string
  }): Promise<PushResponse> => {
    requests.push({
      clientGroupId: input.request.clientGroupId,
      clientId: input.request.clientId,
      userId: input.userId,
    })
    for (const envelope of input.request.mutations) {
      recorded.push({
        args: JSON.parse(envelope.argsJson) as Record<string, unknown>,
        mutationId: envelope.mutationId,
        name: envelope.name,
      })
    }
    return Promise.resolve({
      lastMutationId: input.request.mutations.at(-1)?.mutationId ?? 0,
      protocolVersion: 1,
      results: input.request.mutations.map((envelope, index) => ({
        mutationId: envelope.mutationId,
        status: statuses?.[index] ?? 'applied',
      })),
    } as unknown as PushResponse)
  }
  return { executePush: executePush as never, recorded, requests }
}

describe('appendSarahWorkerCloseoutNoticeToThread', () => {
  test('synthesizes turn.start + turn.started/text.delta/text.completed/turn.finished with the fixed notice text', async () => {
    const push = makeRecordingExecutePush()
    const applied = await appendSarahWorkerCloseoutNoticeToThread(
      {
        executePush: push.executePush,
        nowIso: () => '2026-07-19T00:10:00.000Z',
        sql: (() => Promise.resolve([])) as never,
        uuid: () => 'uuid-fixed',
      } as never,
      {
        assignmentRef: 'assignment.public.pylon_api.abc123',
        ownerUserId: 'owner-1',
        text: 'A Codex worker I dispatched has finished its run and submitted its closeout for review. Assignment: assignment.public.pylon_api.abc123.',
        threadRef: 'thread.sarah.deadbeefdeadbeefdeadbeef',
      },
    )

    expect(applied).toBe(true)
    expect(push.recorded.map(entry => entry.name)).toEqual([
      'runtime.startTurn',
      'runtime.recordEvent',
      'runtime.recordEvent',
      'runtime.recordEvent',
      'runtime.recordEvent',
    ])
    const [start, started, delta, completed, finished] = push.recorded
    expect(start!.args).toMatchObject({
      kind: 'turn.start',
      threadId: 'thread.sarah.deadbeefdeadbeefdeadbeef',
    })
    expect(started!.args).toMatchObject({ kind: 'turn.started', sequence: 0 })
    expect(delta!.args).toMatchObject({
      kind: 'text.delta',
      sequence: 1,
      text: expect.stringContaining('assignment.public.pylon_api.abc123'),
    })
    expect(completed!.args).toMatchObject({ kind: 'text.completed', sequence: 2 })
    expect(finished!.args).toMatchObject({
      finishReason: 'stop',
      kind: 'turn.finished',
      sequence: 3,
    })
    // Every event and the request itself are scoped to the SAME owner —
    // this is what proves the notice lands in the owner's own thread rather
    // than an inferred/foreign scope.
    expect(push.requests).toEqual([
      expect.objectContaining({ userId: 'owner-1' }),
    ])
  })

  test('returns false when the push engine rejects any mutation in the turn', async () => {
    const push = makeRecordingExecutePush(['applied', 'rejected'])
    const applied = await appendSarahWorkerCloseoutNoticeToThread(
      {
        executePush: push.executePush,
        sql: (() => Promise.resolve([])) as never,
      } as never,
      {
        assignmentRef: 'assignment.a',
        ownerUserId: 'owner-1',
        text: 'notice text',
        threadRef: 'thread.sarah.deadbeefdeadbeefdeadbeef',
      },
    )
    expect(applied).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// notifySarahWorkerCloseout — the closeout-side entry point
// ---------------------------------------------------------------------------

const fakeAuthStorage = (): MobileAccessRevocationStore =>
  ({ get: async () => null, put: async () => {} }) as unknown as MobileAccessRevocationStore

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

const makePushDb = (): PushDeviceTokenDb => makeLedgerSqliteDb(PUSH_TABLES_SQLITE_SCHEMA)

describe('notifySarahWorkerCloseout', () => {
  const okBinding = { connectionString: 'postgres://fixture' }

  const makeSqlClientFor = (
    rows: Map<string, FakeMappingRow>,
  ): MakeKhalaSyncPushSqlClient =>
    (async () =>
      ({
        end: async () => {},
        sql: makeFakeMappingSql(rows),
      }) satisfies KhalaSyncPushSqlClient) as MakeKhalaSyncPushSqlClient

  test('no binding is a safe no-op', async () => {
    const outcome = await notifySarahWorkerCloseout(
      {
        authStorage: fakeAuthStorage(),
        binding: undefined,
        makeSqlClient: makeSqlClientFor(new Map()),
        pushDb: makePushDb(),
      },
      { assignmentRef: 'assignment.a', eventStatus: 'closeout_submitted', nowIso: '2026-07-19T00:00:00.000Z' },
    )
    expect(outcome).toEqual({ outcome: 'no_binding' })
  })

  test('an assignment with no Sarah dispatch mapping is a safe no-op — no thread write, no push', async () => {
    const push = makeRecordingExecutePush()
    const outcome = await notifySarahWorkerCloseout(
      {
        authStorage: fakeAuthStorage(),
        binding: okBinding,
        executePush: push.executePush,
        makeSqlClient: makeSqlClientFor(new Map()),
        pushDb: makePushDb(),
      },
      {
        assignmentRef: 'assignment.not_sarah_dispatched',
        eventStatus: 'closeout_submitted',
        nowIso: '2026-07-19T00:00:00.000Z',
      },
    )
    expect(outcome).toEqual({ outcome: 'no_mapping' })
    expect(push.recorded).toHaveLength(0)
  })

  test('a matching mapping appends the thread notice and fires the paired push exactly once', async () => {
    const rows = new Map<string, FakeMappingRow>([
      [
        'assignment.public.pylon_api.abc123',
        {
          assignment_ref: 'assignment.public.pylon_api.abc123',
          consumed_at: null,
          dispatched_at: '2026-07-18T23:00:00.000Z',
          owner_user_id: 'owner-1',
          thread_ref: 'thread.sarah.deadbeefdeadbeefdeadbeef',
        },
      ],
    ])
    const push = makeRecordingExecutePush()
    const outcome = await notifySarahWorkerCloseout(
      {
        authStorage: fakeAuthStorage(),
        binding: okBinding,
        executePush: push.executePush,
        makeSqlClient: makeSqlClientFor(rows),
        pushDb: makePushDb(),
      },
      {
        assignmentRef: 'assignment.public.pylon_api.abc123',
        eventStatus: 'closeout_submitted',
        nowIso: '2026-07-19T00:00:00.000Z',
      },
    )
    expect(outcome).toMatchObject({ outcome: 'notified', threadNoticeApplied: true })
    if (outcome.outcome !== 'notified') throw new Error('unreachable')
    expect(outcome.push).toMatchObject({ ok: true })
    expect(push.recorded.filter(entry => entry.name === 'runtime.startTurn')).toHaveLength(1)
    expect(rows.get('assignment.public.pylon_api.abc123')?.consumed_at).toBe('2026-07-19T00:00:00.000Z')
  })

  test('a duplicate/retried worker_closeout event for the same assignment posts exactly one notice, never two', async () => {
    const rows = new Map<string, FakeMappingRow>([
      [
        'assignment.a',
        {
          assignment_ref: 'assignment.a',
          consumed_at: null,
          dispatched_at: '2026-07-18T23:00:00.000Z',
          owner_user_id: 'owner-1',
          thread_ref: 'thread.sarah.deadbeefdeadbeefdeadbeef',
        },
      ],
    ])
    const push = makeRecordingExecutePush()
    const deps = {
      authStorage: fakeAuthStorage(),
      binding: okBinding,
      executePush: push.executePush,
      makeSqlClient: makeSqlClientFor(rows),
      pushDb: makePushDb(),
    }
    const input = { assignmentRef: 'assignment.a', eventStatus: 'closeout_submitted', nowIso: '2026-07-19T00:00:00.000Z' }

    const first = await notifySarahWorkerCloseout(deps, input)
    const second = await notifySarahWorkerCloseout(deps, {
      ...input,
      nowIso: '2026-07-19T00:01:00.000Z',
    })

    expect(first.outcome).toBe('notified')
    expect(second.outcome).toBe('no_mapping')
    expect(push.recorded.filter(entry => entry.name === 'runtime.startTurn')).toHaveLength(1)
  })

  test('a refused closeout status produces an honest, non-completion notice', async () => {
    const rows = new Map<string, FakeMappingRow>([
      [
        'assignment.a',
        {
          assignment_ref: 'assignment.a',
          consumed_at: null,
          dispatched_at: '2026-07-18T23:00:00.000Z',
          owner_user_id: 'owner-1',
          thread_ref: 'thread.sarah.deadbeefdeadbeefdeadbeef',
        },
      ],
    ])
    const push = makeRecordingExecutePush()
    await notifySarahWorkerCloseout(
      {
        authStorage: fakeAuthStorage(),
        binding: okBinding,
        executePush: push.executePush,
        makeSqlClient: makeSqlClientFor(rows),
        pushDb: makePushDb(),
      },
      { assignmentRef: 'assignment.a', eventStatus: 'rejected', nowIso: '2026-07-19T00:00:00.000Z' },
    )
    const delta = push.recorded.find(entry => entry.name === 'runtime.recordEvent' && entry.args.kind === 'text.delta')
    expect(delta?.args.text).toContain('declined the assignment')
    expect((delta?.args.text as string).toLowerCase()).not.toContain('finished its run')
  })

  test('a thread-write failure is fail-soft: push still fires and the outcome is reported honestly', async () => {
    const rows = new Map<string, FakeMappingRow>([
      [
        'assignment.a',
        {
          assignment_ref: 'assignment.a',
          consumed_at: null,
          dispatched_at: '2026-07-18T23:00:00.000Z',
          owner_user_id: 'owner-1',
          thread_ref: 'thread.sarah.deadbeefdeadbeefdeadbeef',
        },
      ],
    ])
    const throwingExecutePush = (() => Promise.reject(new Error('push engine down'))) as never
    const outcome = await notifySarahWorkerCloseout(
      {
        authStorage: fakeAuthStorage(),
        binding: okBinding,
        executePush: throwingExecutePush,
        makeSqlClient: makeSqlClientFor(rows),
        pushDb: makePushDb(),
      },
      { assignmentRef: 'assignment.a', eventStatus: 'closeout_submitted', nowIso: '2026-07-19T00:00:00.000Z' },
    )
    expect(outcome).toMatchObject({ outcome: 'notified', threadNoticeApplied: false })
  })

  test('a makeSqlClient failure never throws — reported as a failed outcome', async () => {
    const outcome = await notifySarahWorkerCloseout(
      {
        authStorage: fakeAuthStorage(),
        binding: okBinding,
        makeSqlClient: (async () => {
          throw new Error('cannot reach khala sync postgres')
        }) as MakeKhalaSyncPushSqlClient,
        pushDb: makePushDb(),
      },
      { assignmentRef: 'assignment.a', eventStatus: 'closeout_submitted', nowIso: '2026-07-19T00:00:00.000Z' },
    )
    expect(outcome.outcome).toBe('failed')
  })
})
