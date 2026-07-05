// KS-6.7 (#8417): unit tests for the Worker-side tokens-served aggregates
// projection seams — debounced fail-soft refresh + cached fail-open readers.
// These drive the REAL @openagentsinc/khala-sync-server projection code
// over a scripted fake postgres.js-shaped client (pattern-matched tagged
// templates), so CI needs no database; the full Postgres integration lives
// in packages/khala-sync-server/src/tokens-served-mix-projection.test.ts.

import type { SyncSql, SyncTransactionSql } from '@openagentsinc/khala-sync-server'
import { Effect } from 'effect'
import { beforeEach, describe, expect, test } from 'vitest'

import type { KhalaSyncPushSqlClient } from './khala-sync-push-routes'
import {
  invalidateTokensServedAggregatesCache,
  readTokensServedChannelMixSnapshotCached,
  readTokensServedDemandMixSnapshotCached,
  readTokensServedHistorySnapshotCached,
  readTokensServedModelMixSnapshotCached,
  refreshTokensServedAggregatesBestEffort,
  resetTokensServedAggregatesRefreshDebounceForTests,
  TOKENS_SERVED_AGGREGATES_REFRESH_MIN_INTERVAL_MS,
} from './khala-sync-public-tokens-served-mix'

beforeEach(() => {
  resetTokensServedAggregatesRefreshDebounceForTests()
  invalidateTokensServedAggregatesCache()
})

const binding = { connectionString: 'postgres://hyperdrive-fake' }

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
      return [{ committed_at: '2026-07-05T00:00:00.000Z' }]
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

const fakeLedger = (
  overrides: Partial<{
    modelMixWindow: string
    demandMixWindow: string
    channelMixWindow: string
    historyWindow: string
  }> = {},
) => ({
  readPublicTokensServedChannelMix: () =>
    Effect.succeed({
      groups: [
        { channel: 'khala_api' as const, label: 'Khala API', pct: 100, reqs: 1, tokens: 100 },
      ],
      totalTokens: 100,
      window: (overrides.channelMixWindow ?? '30d') as never,
    }),
  readPublicTokensServedDemandMix: () =>
    Effect.succeed({
      groups: [
        {
          client: 'khala-code',
          kind: 'external' as const,
          pct: 100,
          reqs: 1,
          source: 'chat',
          tokens: 100,
        },
      ],
      totalTokens: 100,
      window: (overrides.demandMixWindow ?? '30d') as never,
    }),
  readPublicTokensServedHistory: () =>
    Effect.succeed({
      bucket: 'day' as const,
      series: [{ day: '2026-07-05', tokensServed: 100 }],
      timezone: 'America/Chicago',
      window: (overrides.historyWindow ?? '30d') as never,
    }),
  readPublicTokensServedModelMix: () =>
    Effect.succeed({
      groups: [
        { family: 'glm' as const, label: 'GLM family', pct: 100, reqs: 1, tokens: 100 },
      ],
      totalTokens: 100,
      window: (overrides.modelMixWindow ?? '30d') as never,
    }),
})

// ---------------------------------------------------------------------------
// refreshTokensServedAggregatesBestEffort
// ---------------------------------------------------------------------------

describe('refreshTokensServedAggregatesBestEffort', () => {
  test('skips without a binding and never constructs a client', async () => {
    const outcome = await refreshTokensServedAggregatesBestEffort({
      binding: undefined,
      ledger: fakeLedger(),
      makeSqlClient: async () => {
        throw new Error('must not be constructed')
      },
    })
    expect(outcome).toEqual({ outcome: 'skipped_no_binding' })
  })

  test('refreshes all four windows for all four snapshot kinds', async () => {
    const { rows, sql } = makeFakePg()
    const outcome = await refreshTokensServedAggregatesBestEffort({
      binding,
      ledger: fakeLedger(),
      makeSqlClient: async () => clientFor(sql),
    })
    expect(outcome).toEqual({ outcome: 'refreshed' })
    // 4 windows x 4 snapshot kinds = 16 upserts.
    expect(rows).toHaveLength(16)
  })

  test('debounces a second call within the min interval (pure in-memory, no client construction)', async () => {
    let nowMs = 1_000_000
    const deps = {
      binding,
      ledger: fakeLedger(),
      makeSqlClient: async () => clientFor(makeFakePg().sql),
      nowMs: () => nowMs,
    }
    const first = await refreshTokensServedAggregatesBestEffort(deps)
    expect(first).toEqual({ outcome: 'refreshed' })

    nowMs += TOKENS_SERVED_AGGREGATES_REFRESH_MIN_INTERVAL_MS - 1
    const second = await refreshTokensServedAggregatesBestEffort({
      ...deps,
      makeSqlClient: async () => {
        throw new Error('must not be constructed while debounced')
      },
    })
    expect(second).toEqual({ outcome: 'skipped_debounced' })

    nowMs += 2
    const { sql } = makeFakePg()
    const third = await refreshTokensServedAggregatesBestEffort({
      ...deps,
      makeSqlClient: async () => clientFor(sql),
      nowMs: () => nowMs,
    })
    expect(third).toEqual({ outcome: 'refreshed' })
  })
})

// ---------------------------------------------------------------------------
// Cached readers: refresh then read back through the cache; fail-open on miss
// ---------------------------------------------------------------------------

describe('tokens-served aggregate cached readers', () => {
  test('reads back the exact refreshed model-mix snapshot for a window', async () => {
    const { sql } = makeFakePg()
    await refreshTokensServedAggregatesBestEffort({
      binding,
      ledger: fakeLedger({ modelMixWindow: '7d' }),
      makeSqlClient: async () => clientFor(sql),
    })

    const snapshot = await readTokensServedModelMixSnapshotCached(
      { binding, makeSqlClient: async () => clientFor(sql) },
      '7d',
    )
    expect(snapshot?.totalTokens).toBe(100)
    expect(snapshot?.groups[0]?.family).toBe('glm')
  })

  test('reads back the exact refreshed demand-mix / channel-mix / history snapshots', async () => {
    const { sql } = makeFakePg()
    await refreshTokensServedAggregatesBestEffort({
      binding,
      ledger: fakeLedger({
        channelMixWindow: 'all',
        demandMixWindow: 'today',
        historyWindow: '30d',
      }),
      makeSqlClient: async () => clientFor(sql),
    })

    const demand = await readTokensServedDemandMixSnapshotCached(
      { binding, makeSqlClient: async () => clientFor(sql) },
      'today',
    )
    expect(demand?.groups[0]?.source).toBe('chat')

    const channel = await readTokensServedChannelMixSnapshotCached(
      { binding, makeSqlClient: async () => clientFor(sql) },
      'all',
    )
    expect(channel?.groups[0]?.channel).toBe('khala_api')

    const history = await readTokensServedHistorySnapshotCached(
      { binding, makeSqlClient: async () => clientFor(sql) },
      '30d',
      'America/Chicago',
    )
    expect(history?.series[0]?.day).toBe('2026-07-05')
  })

  test('an unprojected window fails OPEN (undefined), never a throw', async () => {
    const { sql } = makeFakePg()
    const snapshot = await readTokensServedModelMixSnapshotCached(
      { binding, makeSqlClient: async () => clientFor(sql) },
      'today',
    )
    expect(snapshot).toBeUndefined()
  })

  test('no binding fails OPEN without constructing a client', async () => {
    const snapshot = await readTokensServedModelMixSnapshotCached(
      {
        binding: undefined,
        makeSqlClient: async () => {
          throw new Error('must not be constructed')
        },
      },
      '30d',
    )
    expect(snapshot).toBeUndefined()
  })

  test('a broken client fails OPEN, never a throw', async () => {
    const snapshot = await readTokensServedModelMixSnapshotCached(
      {
        binding,
        makeSqlClient: async () => {
          throw new Error('connection refused')
        },
      },
      '30d',
    )
    expect(snapshot).toBeUndefined()
  })
})
