// KS-6.7b (#8421): unit tests for the Worker-side activity-timeline
// projection seams — debounced fail-soft cron refresh + cached fail-open
// reader. These drive the REAL @openagentsinc/khala-sync-server projection
// code over a scripted fake postgres.js-shaped client (pattern-matched
// tagged templates, same fixture style as
// khala-sync-public-tokens-served-mix.test.ts), so CI needs no database;
// the full Postgres integration lives in
// packages/khala-sync-server/src/activity-timeline-projection.test.ts.

import type { SyncSql, SyncTransactionSql } from '@openagentsinc/khala-sync-server'
import { beforeEach, describe, expect, test } from 'vitest'

import type { KhalaSyncPushSqlClient } from './khala-sync-push-routes'
import {
  ACTIVITY_TIMELINE_SNAPSHOT_HARD_STALE_SECONDS,
  ACTIVITY_TIMELINE_SNAPSHOT_REFRESH_MIN_INTERVAL_MS,
  invalidateActivityTimelineSnapshotCacheForTests,
  readActivityTimelineSnapshotCached,
  refreshActivityTimelineSnapshotBestEffort,
  resetActivityTimelineRefreshDebounceForTests,
} from './khala-sync-public-activity-timeline'
import type { PublicActivityTimelineForumStore } from './public-activity-timeline'

beforeEach(() => {
  resetActivityTimelineRefreshDebounceForTests()
  invalidateActivityTimelineSnapshotCacheForTests()
})

const binding = { connectionString: 'postgres://hyperdrive-fake' }
const nowIso = '2026-07-05T00:00:00.000Z'

// ---------------------------------------------------------------------------
// Scripted fake Postgres: a minimal `khala_sync_changelog`/`khala_sync_scopes`
// simulator supporting exactly the statements the projector/reader issue.
// ---------------------------------------------------------------------------

type ChangelogRow = {
  scope: string
  entityType: string
  entityId: string
  version: number
  postImageJson: string | null
}

const makeFakePg = (): { sql: SyncSql; rows: Array<ChangelogRow> } => {
  const rows: Array<ChangelogRow> = []
  let lastVersion = 0

  const run = async (
    strings: TemplateStringsArray,
    ...values: ReadonlyArray<unknown>
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  ): Promise<any> => {
    const text = strings.join('?')
    if (text.includes('INSERT INTO khala_sync_scopes')) {
      lastVersion += 1
      return [{ last_version: lastVersion }]
    }
    if (text.includes('INSERT INTO khala_sync_changelog')) {
      // outbox-writer's exact bind order:
      // (scope, version, entityType, entityId, op, postImageJson, mutationRef).
      rows.push({
        entityId: String(values[3]),
        entityType: String(values[2]),
        postImageJson: values[5] === null ? null : String(values[5]),
        scope: String(values[0]),
        version: Number(values[1]),
      })
      return [{ committed_at: nowIso }]
    }
    if (text.includes('SELECT post_image_json')) {
      const entityType = String(values[1])
      const entityId = String(values[2])
      const matches = rows
        .filter(r => r.entityType === entityType && r.entityId === entityId)
        .sort((a, b) => b.version - a.version)
      const top = matches[0]
      return top === undefined
        ? []
        : [{ post_image_json: top.postImageJson }]
    }
    throw new Error(`fake pg: unscripted statement: ${text.slice(0, 120)}`)
  }

  const sql = run as unknown as SyncSql & {
    begin: <A>(fn: (tx: SyncTransactionSql) => Promise<A>) => Promise<A>
  }
  ;(sql as { begin: unknown }).begin = async <A>(
    fn: (tx: SyncTransactionSql) => Promise<A>,
  ): Promise<A> => fn(run as unknown as SyncTransactionSql)

  return { rows, sql: sql as SyncSql }
}

const clientFor = (sql: SyncSql): KhalaSyncPushSqlClient => ({
  end: async () => undefined,
  sql,
})

const fakeForumStore = (): PublicActivityTimelineForumStore => ({
  listRecentActivity: async () => [
    {
      actorRef: 'agent.public.forum.author',
      createdAt: '2026-07-04T18:00:16.000Z',
      eventRef: 'forum.topic.public.timeline.1',
      kind: 'topic',
      postRef: null,
      sourceRefs: ['forum.topic.public.timeline.1', 'route:/api/forum'],
      state: 'open',
      title: 'Timeline topic',
      topicRef: 'forum.topic.public.timeline.1',
    },
  ],
})

/** Only forumStore is real; the other six domains fail open into projection_gap events — deterministic and cheap for these seam tests. */
const fakeSources = () => ({
  forumStore: fakeForumStore(),
  nowIso: () => nowIso,
})

// ---------------------------------------------------------------------------
// refreshActivityTimelineSnapshotBestEffort
// ---------------------------------------------------------------------------

describe('refreshActivityTimelineSnapshotBestEffort', () => {
  test('skips without a binding and never constructs a client', async () => {
    const outcome = await refreshActivityTimelineSnapshotBestEffort({
      binding: undefined,
      makeSqlClient: async () => {
        throw new Error('must not be constructed')
      },
      sources: fakeSources(),
    })
    expect(outcome).toEqual({ outcome: 'skipped_no_binding' })
  })

  test('refreshes the whole snapshot in ONE upsert', async () => {
    const { rows, sql } = makeFakePg()
    const outcome = await refreshActivityTimelineSnapshotBestEffort({
      binding,
      makeSqlClient: async () => clientFor(sql),
      sources: fakeSources(),
    })
    expect(outcome.outcome).toBe('refreshed')
    if (outcome.outcome === 'refreshed') {
      // forum topic + 6 projection_gap events (pylon/training/receipt/
      // inference-receipt/artanis/capacity stores all undefined).
      expect(outcome.eventCount).toBe(7)
    }
    expect(rows).toHaveLength(1)
  })

  test('debounces a second call within the min interval (pure in-memory, no client construction)', async () => {
    let nowMs = 1_000_000
    const deps = {
      binding,
      makeSqlClient: async () => clientFor(makeFakePg().sql),
      nowMs: () => nowMs,
      sources: fakeSources(),
    }
    const first = await refreshActivityTimelineSnapshotBestEffort(deps)
    expect(first.outcome).toBe('refreshed')

    nowMs += ACTIVITY_TIMELINE_SNAPSHOT_REFRESH_MIN_INTERVAL_MS - 1
    const second = await refreshActivityTimelineSnapshotBestEffort({
      ...deps,
      makeSqlClient: async () => {
        throw new Error('must not be constructed while debounced')
      },
      nowMs: () => nowMs,
    })
    expect(second).toEqual({ outcome: 'skipped_debounced' })

    nowMs += 2
    const { sql } = makeFakePg()
    const third = await refreshActivityTimelineSnapshotBestEffort({
      ...deps,
      makeSqlClient: async () => clientFor(sql),
      nowMs: () => nowMs,
    })
    expect(third.outcome).toBe('refreshed')
  })
})

// ---------------------------------------------------------------------------
// readActivityTimelineSnapshotCached
// ---------------------------------------------------------------------------

describe('readActivityTimelineSnapshotCached', () => {
  test('reads back the exact refreshed snapshot', async () => {
    const { sql } = makeFakePg()
    await refreshActivityTimelineSnapshotBestEffort({
      binding,
      makeSqlClient: async () => clientFor(sql),
      sources: fakeSources(),
    })

    const snapshot = await readActivityTimelineSnapshotCached({
      binding,
      makeSqlClient: async () => clientFor(sql),
      nowIso: () => nowIso,
    })
    expect(snapshot).toBeDefined()
    expect(snapshot?.generatedAt).toBe(nowIso)
    expect(
      snapshot?.events.some(
        event => event.eventRef === 'event.public.forum_topic.forum.topic.public.timeline.1',
      ),
    ).toBe(true)
  })

  test('an unprojected scope fails OPEN (undefined), never a throw', async () => {
    const { sql } = makeFakePg()
    const snapshot = await readActivityTimelineSnapshotCached({
      binding,
      makeSqlClient: async () => clientFor(sql),
    })
    expect(snapshot).toBeUndefined()
  })

  test('a broken Postgres client fails OPEN (undefined), never a throw', async () => {
    const snapshot = await readActivityTimelineSnapshotCached({
      binding,
      makeSqlClient: async () => {
        throw new Error('connection refused')
      },
    })
    expect(snapshot).toBeUndefined()
  })

  test('a snapshot older than the hard-stale ceiling fails OPEN (undefined)', async () => {
    const { sql } = makeFakePg()
    await refreshActivityTimelineSnapshotBestEffort({
      binding,
      makeSqlClient: async () => clientFor(sql),
      sources: fakeSources(),
    })

    const farFutureIso = new Date(
      Date.parse(nowIso) +
        (ACTIVITY_TIMELINE_SNAPSHOT_HARD_STALE_SECONDS + 60) * 1000,
    ).toISOString()

    const snapshot = await readActivityTimelineSnapshotCached({
      binding,
      makeSqlClient: async () => clientFor(sql),
      nowIso: () => farFutureIso,
    })
    expect(snapshot).toBeUndefined()
  })

  test('caches within the TTL: a second read never constructs a new client', async () => {
    const { sql } = makeFakePg()
    await refreshActivityTimelineSnapshotBestEffort({
      binding,
      makeSqlClient: async () => clientFor(sql),
      sources: fakeSources(),
    })

    let nowMs = 5_000_000
    const first = await readActivityTimelineSnapshotCached({
      binding,
      makeSqlClient: async () => clientFor(sql),
      nowIso: () => nowIso,
      nowMs: () => nowMs,
    })
    expect(first).toBeDefined()

    nowMs += 1
    const second = await readActivityTimelineSnapshotCached({
      binding,
      makeSqlClient: async () => {
        throw new Error('must not be constructed while cached')
      },
      nowIso: () => nowIso,
      nowMs: () => nowMs,
    })
    expect(second).toBeDefined()
  })
})
