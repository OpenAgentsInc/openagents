import type { SyncSql } from '@openagentsinc/khala-sync-server'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { handlePublicKhalaTokensServedChannelMixApi } from './public-khala-tokens-served-channel-mix-routes'
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

type ChannelMixRow = {
  demand_channel: string | null
  tokens: number
  usage_events: number
}

const fakeChannelMixDb = (rows: ReadonlyArray<ChannelMixRow>): D1Database => {
  const prepare = (query = '') => ({
    bind: () => prepare(query),
    all: <T>(): Promise<{ results: ReadonlyArray<T> }> =>
      Promise.resolve({
        results: query.includes(
          'public_khala_tokens_served_channel_daily_rollups',
        )
          ? (rows as ReadonlyArray<T>)
          : [],
      }),
  })

  return { prepare } as unknown as D1Database
}

const routeInput = (
  rows: ReadonlyArray<ChannelMixRow>,
): Parameters<typeof handlePublicKhalaTokensServedChannelMixApi>[1] => ({
  ledger: makeD1TokenUsageLedger(fakeChannelMixDb(rows), runtime),
  nowIso: () => nowIso,
})

const getRequest = (search = ''): Request =>
  new Request(
    `https://openagents.com/api/public/khala-tokens-served/channel-mix${search}`,
    { method: 'GET' },
  )

describe('GET /api/public/khala-tokens-served/channel-mix', () => {
  test('returns public aggregate rows by product demand channel', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedChannelMixApi(
        getRequest('?window=30d'),
        routeInput([
          {
            demand_channel: 'khala_api',
            tokens: 1_500,
            usage_events: 3,
          },
          {
            demand_channel: 'direct_local',
            tokens: 500,
            usage_events: 1,
          },
        ]),
      ),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toContain('no-store')

    const body = (await response.json()) as Record<string, unknown>

    expect(body.schemaVersion).toBe('openagents.public_khala_channel_mix.v1')
    expect(body.window).toBe('30d')
    expect(body.totalTokens).toBe(2_000)
    expect(body.generatedAt).toBe(nowIso)
    expect(body.staleness).toMatchObject({
      composition: 'live_at_read',
      maxStalenessSeconds: 0,
      rebuildsOn: ['token_usage_events'],
    })
    expect(body.groups).toEqual([
      {
        channel: 'khala_api',
        label: 'Khala API',
        pct: 75,
        reqs: 3,
        tokens: 1_500,
      },
      {
        channel: 'direct_local',
        label: 'Direct local',
        pct: 25,
        reqs: 1,
        tokens: 500,
      },
    ])
  })

  test('defaults unknown legacy rows to khala_api', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedChannelMixApi(
        getRequest(),
        routeInput([
          {
            demand_channel: null,
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
        channel: 'khala_api',
        label: 'Khala API',
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
        { channel: 'khala_api', label: 'Khala API', pct: 100, reqs: 4, tokens: 400 },
      ],
      totalTokens: 400,
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
      handlePublicKhalaTokensServedChannelMixApi(getRequest('?window=30d'), {
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
    expect(body.totalTokens).toBe(400)
    expect(body.generatedAt).toBe(postImage.generatedAt)
    expect(body.staleness).toMatchObject({
      composition: 'stored_snapshot',
      maxStalenessSeconds: 2,
      rebuildsOn: ['scope.public.tokens-served-aggregates'],
    })
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedChannelMixApi(
        new Request(
          'https://openagents.com/api/public/khala-tokens-served/channel-mix',
          { method: 'POST' },
        ),
        routeInput([]),
      ),
    )

    expect(response.status).toBe(405)
  })

  test('is public-safe: aggregate channel rows only', async () => {
    const leakyLedger: TokenUsageLedgerShape = {
      ...makeD1TokenUsageLedger(fakeChannelMixDb([]), runtime),
      readPublicTokensServedChannelMix: () =>
        Effect.succeed({
          groups: [
            {
              channel: 'direct_local',
              label: 'Direct local',
              accountRef: 'account.private.secret',
              rawPrompt: 'do not expose this prompt',
              pct: 100,
              reqs: 1,
              tokens: 50,
            } as unknown as {
              channel: 'direct_local'
              label: string
              pct: number
              reqs: number
              tokens: number
            },
          ],
          totalTokens: 50,
          window: '30d',
        }),
    }

    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedChannelMixApi(getRequest(), {
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
      'channel',
      'label',
      'pct',
      'reqs',
      'tokens',
    ])
    expect(JSON.stringify(body)).not.toContain('account.private.secret')
    expect(JSON.stringify(body)).not.toContain('do not expose this prompt')
  })
})
