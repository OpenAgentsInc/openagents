import type { SyncSql } from '@openagentsinc/khala-sync-server'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { handlePublicKhalaTokensServedModelMixApi } from './public-khala-tokens-served-model-mix-routes'
import {
  makeD1TokenUsageLedger,
  publicModelFamilyFromProviderAndModel,
  type TokenUsageLedgerRuntime,
  type TokenUsageLedgerShape,
} from './token-usage-ledger'

const nowIso = '2026-06-24T12:00:00.000Z'

const runtime: TokenUsageLedgerRuntime = {
  isoTimestampAfterIso: () => '2026-05-25T12:00:00.000Z',
  nowIso: () => nowIso,
  utcStartOfDayIsoTimestamp: () => '2026-06-24T00:00:00.000Z',
}

type ModelMixRow = {
  model: string | null
  provider: string | null
  tokens: number
  usage_events: number
}

const fakeModelMixDb = (rows: ReadonlyArray<ModelMixRow>): D1Database => {
  const prepare = (query = '') => ({
    bind: () => prepare(query),
    all: <T>(): Promise<{ results: ReadonlyArray<T> }> =>
      Promise.resolve({
        results: query.includes(
          'public_khala_tokens_served_model_daily_rollups',
        )
          ? (rows as ReadonlyArray<T>)
          : [],
      }),
  })

  return { prepare } as unknown as D1Database
}

const routeInput = (
  rows: ReadonlyArray<ModelMixRow>,
): Parameters<typeof handlePublicKhalaTokensServedModelMixApi>[1] => ({
  ledger: makeD1TokenUsageLedger(fakeModelMixDb(rows), runtime),
  nowIso: () => nowIso,
})

const getRequest = (search = ''): Request =>
  new Request(
    `https://openagents.com/api/public/khala-tokens-served/model-mix${search}`,
    { method: 'GET' },
  )

describe('GET /api/public/khala-tokens-served/model-mix', () => {
  test('returns canonical family aggregate mix from the D1 ledger', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedModelMixApi(
        getRequest('?window=30d'),
        routeInput([
          {
            model: 'gpt-4.1',
            provider: 'openai',
            tokens: 1_000,
            usage_events: 2,
          },
          {
            model: 'gemini-2.5-pro',
            provider: 'google_vertex',
            tokens: 500,
            usage_events: 1,
          },
          {
            model: 'deepseek-chat',
            provider: 'deepseek',
            tokens: 500,
            usage_events: 1,
          },
        ]),
      ),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toContain('no-store')

    const body = (await response.json()) as Record<string, unknown>

    expect(body.schemaVersion).toBe('openagents.public_khala_model_mix.v1')
    expect(body.window).toBe('30d')
    expect(body.totalTokens).toBe(2_000)
    expect(body.generatedAt).toBe(nowIso)
    expect(body.liveAt).toBe(nowIso)
    expect(body.staleness).toMatchObject({
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
      rebuildsOn: ['token_usage_events'],
    })
    expect(body.groups).toEqual([
      {
        family: 'other',
        label: 'Other',
        pct: 50,
        reqs: 2,
        tokens: 1_000,
      },
      {
        family: 'fireworks_deepseek',
        label: 'Fireworks DeepSeek',
        pct: 25,
        reqs: 1,
        tokens: 500,
      },
      {
        family: 'gemini',
        label: 'Gemini',
        pct: 25,
        reqs: 1,
        tokens: 500,
      },
    ])
  })

  test('defaults to the 30d window', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedModelMixApi(
        getRequest(),
        routeInput([
          {
            model: 'openagents/pylon-codex',
            provider: 'pylon-codex-own-capacity',
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
        family: 'pylon_codex',
        label: 'Pylon-Codex',
        pct: 100,
        reqs: 1,
        tokens: 42,
      },
    ])
  })

  test('shows direct local Codex as a separate public family', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedModelMixApi(
        getRequest(),
        routeInput([
          {
            model: 'openagents/codex-direct-local',
            provider: 'pylon-codex-direct-local',
            tokens: 64,
            usage_events: 2,
          },
        ]),
      ),
    )
    const body = (await response.json()) as Record<string, unknown>

    expect(body.groups).toEqual([
      {
        family: 'codex_direct',
        label: 'Codex (direct)',
        pct: 100,
        reqs: 2,
        tokens: 64,
      },
    ])
  })

  test('KS-6.7 (#8417): a projected snapshot is served FIRST — the ledger is never called', async () => {
    const postImage = {
      generatedAt: '2026-07-05T00:00:00.000Z',
      groups: [
        { family: 'glm', label: 'GLM family', pct: 100, reqs: 3, tokens: 300 },
      ],
      totalTokens: 300,
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
      handlePublicKhalaTokensServedModelMixApi(getRequest('?window=30d'), {
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
    expect(body.totalTokens).toBe(300)
    expect(body.generatedAt).toBe(postImage.generatedAt)
    expect(body.staleness).toMatchObject({
      composition: 'stored_snapshot',
      maxStalenessSeconds: 2,
      rebuildsOn: ['scope.public.tokens-served-aggregates'],
    })
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedModelMixApi(
        new Request(
          'https://openagents.com/api/public/khala-tokens-served/model-mix',
          { method: 'POST' },
        ),
        routeInput([]),
      ),
    )

    expect(response.status).toBe(405)
  })

  test('is public-safe: aggregate family rows only, no raw provider/model material', async () => {
    const leakyLedger: TokenUsageLedgerShape = {
      ...makeD1TokenUsageLedger(fakeModelMixDb([]), runtime),
      readPublicTokensServedModelMix: () =>
        Effect.succeed({
          groups: [
            {
              family: 'other',
              label: 'Other',
              model: 'gpt-4.1-secret-experiment',
              pct: 100,
              provider: 'openai-private-lane',
              reqs: 1,
              tokens: 50,
            } as unknown as {
              family: 'other'
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
      handlePublicKhalaTokensServedModelMixApi(getRequest(), {
        ledger: leakyLedger,
        nowIso: () => nowIso,
      }),
    )
    const body = (await response.json()) as Record<string, unknown>

    expect(Object.keys(body).sort()).toEqual([
      'generatedAt',
      'groups',
      'liveAt',
      'schemaVersion',
      'staleness',
      'totalTokens',
      'window',
    ])

    const groups = body.groups as ReadonlyArray<Record<string, unknown>>
    expect(groups).toHaveLength(1)
    expect(Object.keys(groups[0]!).sort()).toEqual([
      'family',
      'label',
      'pct',
      'reqs',
      'tokens',
    ])
    expect(JSON.stringify(body)).not.toContain('gpt-4.1-secret-experiment')
    expect(JSON.stringify(body)).not.toContain('openai-private-lane')
  })

  test('normalizes provider and model variants into stable public families', () => {
    expect(publicModelFamilyFromProviderAndModel('reap', 'glm-4.5')).toBe('glm')
    expect(publicModelFamilyFromProviderAndModel('Z.AI', 'zhipu-chat')).toBe(
      'glm',
    )
    expect(
      publicModelFamilyFromProviderAndModel('fireworks-ai', 'deepseek-v3'),
    ).toBe('fireworks_deepseek')
    expect(
      publicModelFamilyFromProviderAndModel(
        'pylon-codex-own-capacity',
        'openagents/pylon-codex',
      ),
    ).toBe('pylon_codex')
    expect(
      publicModelFamilyFromProviderAndModel(
        'pylon-codex-direct-local',
        'openagents/codex-direct-local',
      ),
    ).toBe('codex_direct')
    expect(
      publicModelFamilyFromProviderAndModel(
        'pylon_claude_own_capacity',
        'openagents/pylon-claude',
      ),
    ).toBe('pylon_claude')
    expect(publicModelFamilyFromProviderAndModel('openrouter', 'gpt_oss_120b')).toBe(
      'gpt_oss',
    )
    expect(publicModelFamilyFromProviderAndModel('google_vertex', 'gemini-pro')).toBe(
      'gemini',
    )
    expect(publicModelFamilyFromProviderAndModel('private-provider', 'x')).toBe(
      'other',
    )
  })
})
