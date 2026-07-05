// KS-6.4 (#8414): unit tests for the Worker-side public settled-feed
// projection seams — fail-soft batch producer and cached reader. Drives
// the REAL @openagentsinc/khala-sync-server projection code over a
// scripted fake postgres.js-shaped client (pattern-matched tagged
// templates), so CI needs no database; the full Postgres integration
// lives in packages/khala-sync-server/src/settled-feed-projection.test.ts.

import type { SyncSql, SyncTransactionSql } from '@openagentsinc/khala-sync-server'
import { beforeEach, describe, expect, test } from 'vitest'

import type { KhalaSyncPushSqlClient } from './khala-sync-push-routes'
import {
  invalidateSettledFeedProjectionCacheForTests,
  projectSettledFeedBatchBestEffort,
  readSettledFeedProjectionCached,
  SETTLED_FEED_PROJECTION_CACHE_TTL_MS,
  type SettledFeedProjectionLog,
} from './khala-sync-public-settled-feed'
import type { PublicSettledFeedEvent } from './tassadar-settled-feed-sync'

const observedAt = '2026-07-05T12:00:00.000Z'

beforeEach(() => {
  invalidateSettledFeedProjectionCacheForTests()
})

// ---------------------------------------------------------------------------
// Scripted fake Postgres (pattern-matched tagged-template SQL)
// ---------------------------------------------------------------------------

type ChangelogRow = {
  entityType: string
  entityId: string
  version: number
  postImageJson: string | null
}

type FakePgState = {
  lastVersion: number
  changelog: Array<ChangelogRow>
}

const makeFakePg = (): { state: FakePgState; sql: SyncSql } => {
  const state: FakePgState = { changelog: [], lastVersion: 0 }

  const latestPerEntity = (entityType: string): Array<ChangelogRow> => {
    const byEntity = new Map<string, ChangelogRow>()
    for (const row of state.changelog) {
      if (row.entityType !== entityType) continue
      const existing = byEntity.get(row.entityId)
      if (existing === undefined || row.version > existing.version) {
        byEntity.set(row.entityId, row)
      }
    }
    return [...byEntity.values()].sort((a, b) => b.version - a.version)
  }

  const run = async (
    strings: TemplateStringsArray,
    ...values: ReadonlyArray<unknown>
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  ): Promise<any> => {
    const text = strings.join('?')
    if (text.includes('INSERT INTO khala_sync_scopes')) {
      state.lastVersion += 1
      return [{ last_version: state.lastVersion }]
    }
    if (text.includes('INSERT INTO khala_sync_changelog')) {
      const [, version, entityType, entityId, , postImageJson] = values
      state.changelog.push({
        entityId: String(entityId),
        entityType: String(entityType),
        postImageJson: postImageJson === null ? null : String(postImageJson),
        version: Number(version),
      })
      return [{ committed_at: observedAt }]
    }
    if (text.includes('SELECT DISTINCT ON (entity_id)')) {
      const entityType = String(values[1])
      const limit = Number(values[2])
      return latestPerEntity(entityType)
        .slice(0, limit)
        .map(row => ({ post_image_json: row.postImageJson }))
    }
    if (
      text.includes('FROM khala_sync_changelog') &&
      text.includes('entity_id = ')
    ) {
      const entityType = String(values[1])
      const rows = latestPerEntity(entityType)
      return rows.length === 0 ? [] : [{ post_image_json: rows[0]?.postImageJson }]
    }
    throw new Error(`fake pg: unscripted statement: ${text.slice(0, 120)}`)
  }

  const sql = run as unknown as SyncSql & {
    begin: <A>(fn: (tx: SyncTransactionSql) => Promise<A>) => Promise<A>
  }
  ;(sql as { begin: unknown }).begin = async <A>(
    fn: (tx: SyncTransactionSql) => Promise<A>,
  ): Promise<A> => {
    const snapshot = {
      changelog: [...state.changelog],
      lastVersion: state.lastVersion,
    }
    try {
      return await fn(run as unknown as SyncTransactionSql)
    } catch (error) {
      state.changelog = snapshot.changelog
      state.lastVersion = snapshot.lastVersion
      throw error
    }
  }
  return { sql: sql as SyncSql, state }
}

const clientFor = (sql: SyncSql): KhalaSyncPushSqlClient => ({
  end: async () => undefined,
  sql,
})

const binding = { connectionString: 'postgres://hyperdrive-fake' }

type LogCall = { event: string; fields: Record<string, string | number> }
const makeLog = (): { calls: Array<LogCall>; log: SettledFeedProjectionLog } => {
  const calls: Array<LogCall> = []
  return {
    calls,
    log: (event, fields) => {
      calls.push({ event, fields: { ...fields } })
    },
  }
}

const settledEvent = (
  overrides: Partial<PublicSettledFeedEvent> = {},
): PublicSettledFeedEvent => ({
  amountSats: 5,
  challengeRef: 'challenge.tassadar.window.0001',
  contributorRef: 'pylon.worker.orrery',
  eventRef: 'settled.window0001.worker.0',
  party: 'worker',
  runRef: 'run.tassadar.poc',
  settledAt: observedAt,
  totalSettledCount: 1,
  totalSettledSats: 5,
  windowRef: 'window.tassadar.0001',
  ...overrides,
})

const summaryFor = (event: PublicSettledFeedEvent) => ({
  latestEventRef: event.eventRef,
  latestSettledAt: event.settledAt,
  totalSettledCount: event.totalSettledCount,
  totalSettledSats: event.totalSettledSats,
  updatedAt: event.settledAt,
})

// ---------------------------------------------------------------------------
// Producer
// ---------------------------------------------------------------------------

describe('projectSettledFeedBatchBestEffort', () => {
  test('skips with no binding', async () => {
    const outcome = await projectSettledFeedBatchBestEffort(
      { binding: undefined },
      { events: [settledEvent()], summary: summaryFor(settledEvent()) },
    )
    expect(outcome.outcome).toBe('skipped_no_binding')
  })

  test('skips with no events (never touches storage)', async () => {
    const { sql } = makeFakePg()
    const outcome = await projectSettledFeedBatchBestEffort(
      { binding, makeSqlClient: async () => clientFor(sql) },
      { events: [], summary: summaryFor(settledEvent()) },
    )
    expect(outcome.outcome).toBe('skipped_no_events')
  })

  test('projects events + summary into the fake changelog', async () => {
    const { sql, state } = makeFakePg()
    const event = settledEvent()
    const outcome = await projectSettledFeedBatchBestEffort(
      { binding, makeSqlClient: async () => clientFor(sql) },
      { events: [event], summary: summaryFor(event) },
    )
    expect(outcome.outcome).toBe('projected')
    expect(state.changelog).toHaveLength(2)
    expect(new Set(state.changelog.map(r => r.entityType))).toEqual(
      new Set(['settled_feed_event', 'settled_feed_summary']),
    )
    // Both rows land at the SAME version (one changelog batch).
    expect(new Set(state.changelog.map(r => r.version)).size).toBe(1)
  })

  test('a broken client is fail-soft and logs a public-safe diagnostic', async () => {
    const { calls, log } = makeLog()
    const outcome = await projectSettledFeedBatchBestEffort(
      {
        binding,
        log,
        makeSqlClient: async () => {
          throw new Error('connect failed: postgres://user:secret@10.0.0.1')
        },
      },
      { events: [settledEvent()], summary: summaryFor(settledEvent()) },
    )
    expect(outcome.outcome).toBe('failed')
    expect(calls).toHaveLength(1)
    expect(calls[0]?.event).toBe('khala_sync_settled_feed_projection_failed')
    expect(JSON.stringify(calls[0])).not.toContain('secret')
    expect(JSON.stringify(calls[0])).not.toContain('10.0.0.1')
  })

  test('invalid input yields invalid_input and writes nothing', async () => {
    const { sql, state } = makeFakePg()
    const outcome = await projectSettledFeedBatchBestEffort(
      { binding, makeSqlClient: async () => clientFor(sql) },
      {
        events: [settledEvent({ amountSats: -1 })],
        summary: summaryFor(settledEvent()),
      },
    )
    expect(outcome.outcome).toBe('failed')
    if (outcome.outcome === 'failed') {
      expect(outcome.diagnostic.reason).toBe('invalid_input')
    }
    expect(state.changelog).toHaveLength(0)
  })
})

// ---------------------------------------------------------------------------
// Cached reader
// ---------------------------------------------------------------------------

describe('readSettledFeedProjectionCached', () => {
  test('fails open (undefined) with no binding', async () => {
    const snapshot = await readSettledFeedProjectionCached({ binding: undefined })
    expect(snapshot).toBeUndefined()
  })

  test('reads the projected events + summary after a write', async () => {
    const { sql } = makeFakePg()
    const event = settledEvent()
    await projectSettledFeedBatchBestEffort(
      { binding, makeSqlClient: async () => clientFor(sql) },
      { events: [event], summary: summaryFor(event) },
    )
    const snapshot = await readSettledFeedProjectionCached({
      binding,
      makeSqlClient: async () => clientFor(sql),
    })
    expect(snapshot?.events).toHaveLength(1)
    expect(snapshot?.events[0]?.eventRef).toBe(event.eventRef)
    expect(snapshot?.summary?.totalSettledCount).toBe(1)
  })

  test('caches within the TTL and refreshes after it expires', async () => {
    const { sql } = makeFakePg()
    const first = settledEvent()
    await projectSettledFeedBatchBestEffort(
      { binding, makeSqlClient: async () => clientFor(sql) },
      { events: [first], summary: summaryFor(first) },
    )
    let nowMs = 1_000
    const readDeps = {
      binding,
      makeSqlClient: async () => clientFor(sql),
      nowMs: () => nowMs,
    }
    const initial = await readSettledFeedProjectionCached(readDeps)
    expect(initial?.events).toHaveLength(1)

    // A second event lands, but the cache is still fresh.
    const second = settledEvent({
      eventRef: 'settled.window0001.worker.1',
      totalSettledCount: 2,
      totalSettledSats: 10,
    })
    await projectSettledFeedBatchBestEffort(
      { binding, makeSqlClient: async () => clientFor(sql) },
      { events: [second], summary: summaryFor(second) },
    )
    const stillCached = await readSettledFeedProjectionCached(readDeps)
    expect(stillCached?.events).toHaveLength(1)

    // Advance past the TTL: the cache refreshes to the current projection.
    nowMs += SETTLED_FEED_PROJECTION_CACHE_TTL_MS + 1
    const refreshed = await readSettledFeedProjectionCached(readDeps)
    expect(refreshed?.events).toHaveLength(2)
    expect(refreshed?.summary?.totalSettledCount).toBe(2)
  })

  test('fails open (undefined) when the client throws', async () => {
    const snapshot = await readSettledFeedProjectionCached({
      binding,
      makeSqlClient: async () => {
        throw new Error('connect failed')
      },
    })
    expect(snapshot).toBeUndefined()
  })
})
