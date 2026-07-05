import type { SyncSql } from '@openagentsinc/khala-sync-server'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { handlePublicKhalaTokensServedDemandMixApi } from './public-khala-tokens-served-demand-mix-routes'
import {
  makeD1TokenUsageLedger,
  type TokenUsageLedgerRuntime,
  type TokenUsageLedgerShape,
} from './token-usage-ledger'

const nowIso = '2026-06-24T12:00:00.000Z'

const runtime: TokenUsageLedgerRuntime = {
  isoTimestampAfterIso: () => '2026-05-25T12:00:00.000Z',
  nowIso: () => nowIso,
  utcStartOfDayIsoTimestamp: () => '2026-06-24T00:00:00.000Z',
}

type DemandMixRow = {
  demand_client: string | null
  demand_kind: string | null
  demand_source: string | null
  tokens: number
  usage_events: number
}

const fakeDemandMixDb = (rows: ReadonlyArray<DemandMixRow>): D1Database => {
  const prepare = () => ({
    bind: () => prepare(),
    all: <T>(): Promise<{ results: ReadonlyArray<T> }> =>
      Promise.resolve({ results: rows as ReadonlyArray<T> }),
  })

  return { prepare } as unknown as D1Database
}

const routeInput = (
  rows: ReadonlyArray<DemandMixRow>,
): Parameters<typeof handlePublicKhalaTokensServedDemandMixApi>[1] => ({
  ledger: makeD1TokenUsageLedger(fakeDemandMixDb(rows), runtime),
  nowIso: () => nowIso,
})

const getRequest = (search = ''): Request =>
  new Request(
    `https://openagents.com/api/public/khala-tokens-served/demand-mix${search}`,
    { method: 'GET' },
  )

describe('GET /api/public/khala-tokens-served/demand-mix', () => {
  test('returns public demand-source aggregate rows from the D1 ledger', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedDemandMixApi(
        getRequest('?window=30d'),
        routeInput([
          {
            demand_client: 'khala-cli',
            demand_kind: 'external',
            demand_source: 'ecosystem',
            tokens: 1_000,
            usage_events: 2,
          },
          {
            demand_client: 'pylon',
            demand_kind: 'own_capacity',
            demand_source: 'khala_coding_delegation',
            tokens: 500,
            usage_events: 1,
          },
          {
            demand_client: null,
            demand_kind: 'not-a-kind',
            demand_source: '',
            tokens: 250,
            usage_events: 1,
          },
        ]),
      ),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toContain('no-store')

    const body = (await response.json()) as Record<string, unknown>

    expect(body.schemaVersion).toBe('openagents.public_khala_demand_mix.v1')
    expect(body.window).toBe('30d')
    expect(body.totalTokens).toBe(1_750)
    expect(body.generatedAt).toBe(nowIso)
    expect(body.staleness).toMatchObject({
      composition: 'live_at_read',
      maxStalenessSeconds: 0,
    })
    expect(body.groups).toEqual([
      {
        kind: 'external',
        source: 'ecosystem',
        client: 'khala-cli',
        pct: 57.142857,
        reqs: 2,
        tokens: 1_000,
      },
      {
        kind: 'own_capacity',
        source: 'khala_coding_delegation',
        client: 'pylon',
        pct: 28.571429,
        reqs: 1,
        tokens: 500,
      },
      {
        kind: 'unlabeled',
        source: 'unknown',
        client: 'unknown',
        pct: 14.285714,
        reqs: 1,
        tokens: 250,
      },
    ])
  })

  test('defaults to the 30d window', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedDemandMixApi(
        getRequest(),
        routeInput([
          {
            demand_client: 'qa-runner',
            demand_kind: 'internal',
            demand_source: 'qa-dogfood',
            tokens: 42,
            usage_events: 1,
          },
        ]),
      ),
    )
    const body = (await response.json()) as Record<string, unknown>

    expect(body.window).toBe('30d')
    expect(body.groups).toEqual([
      {
        kind: 'internal',
        source: 'qa-dogfood',
        client: 'qa-runner',
        pct: 100,
        reqs: 1,
        tokens: 42,
      },
    ])
  })

  test('KS-6.7 (#8417): a projected snapshot is served FIRST — the ledger is never called', async () => {
    const postImage = {
      generatedAt: '2026-07-05T00:00:00.000Z',
      groups: [
        {
          client: 'khala-code',
          kind: 'external',
          pct: 100,
          reqs: 5,
          source: 'chat',
          tokens: 500,
        },
      ],
      totalTokens: 500,
      window: '30d',
    }
    const fakeSql = (async (strings: TemplateStringsArray) => {
      const text = strings.join('?')
      if (text.includes('SELECT post_image_json')) {
        return [{ post_image_json: JSON.stringify(postImage) }]
      }
      throw new Error(`unscripted: ${text.slice(0, 80)}`)
    }) as unknown as SyncSql

    const throwingLedger = new Proxy(
      {},
      {
        get: () => () => {
          throw new Error('ledger must not be called when the projection hits')
        },
      },
    ) as TokenUsageLedgerShape

    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedDemandMixApi(getRequest('?window=30d'), {
        KHALA_SYNC_DB: { connectionString: 'postgres://hyperdrive-fake' },
        ledger: throwingLedger,
        nowIso: () => nowIso,
        projectionReadDeps: {
          makeSqlClient: async () => ({ end: async () => undefined, sql: fakeSql }),
        },
      }),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.window).toBe('30d')
    expect(body.totalTokens).toBe(500)
    expect(body.generatedAt).toBe(postImage.generatedAt)
    expect(body.staleness).toMatchObject({
      composition: 'stored_snapshot',
      maxStalenessSeconds: 2,
      rebuildsOn: ['scope.public.tokens-served-aggregates'],
    })
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedDemandMixApi(
        new Request(
          'https://openagents.com/api/public/khala-tokens-served/demand-mix',
          { method: 'POST' },
        ),
        routeInput([]),
      ),
    )

    expect(response.status).toBe(405)
  })

  test('is public-safe: aggregate demand rows only', async () => {
    const leakyLedger: TokenUsageLedgerShape = {
      ...makeD1TokenUsageLedger(fakeDemandMixDb([]), runtime),
      readPublicTokensServedDemandMix: () =>
        Effect.succeed({
          groups: [
            {
              kind: 'external',
              source: 'ecosystem',
              client: 'khala-cli',
              accountRef: 'account.private.secret',
              actorUserId: 'user_private',
              rawPrompt: 'do not expose this prompt',
              pct: 100,
              reqs: 1,
              tokens: 50,
            } as unknown as {
              client: string
              kind: 'external'
              pct: number
              reqs: number
              source: string
              tokens: number
            },
          ],
          totalTokens: 50,
          window: '30d',
        }),
    }

    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedDemandMixApi(getRequest(), {
        ledger: leakyLedger,
        nowIso: () => nowIso,
      }),
    )
    const body = (await response.json()) as Record<string, unknown>

    expect(Object.keys(body).sort()).toEqual([
      'generatedAt',
      'groups',
      'schemaVersion',
      'staleness',
      'totalTokens',
      'window',
    ])

    const groups = body.groups as ReadonlyArray<Record<string, unknown>>
    expect(groups).toHaveLength(1)
    expect(Object.keys(groups[0]!).sort()).toEqual([
      'client',
      'kind',
      'pct',
      'reqs',
      'source',
      'tokens',
    ])
    expect(JSON.stringify(body)).not.toContain('account.private.secret')
    expect(JSON.stringify(body)).not.toContain('do not expose this prompt')
  })
})
