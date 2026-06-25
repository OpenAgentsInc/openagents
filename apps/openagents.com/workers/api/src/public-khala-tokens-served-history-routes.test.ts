import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { handlePublicKhalaTokensServedHistoryApi } from './public-khala-tokens-served-history-routes'
import {
  makeD1TokenUsageLedger,
  type TokenUsageLedgerRuntime,
  type TokenUsageLedgerShape,
} from './token-usage-ledger'

const nowIso = '2026-06-24T12:00:00.000Z'

const runtime: TokenUsageLedgerRuntime = {
  // 30d window → since = now - 30 days. The fake DB ignores the bound and
  // returns the canned rows, so the value is irrelevant to the assertions.
  isoTimestampAfterIso: () => '2026-05-25T12:00:00.000Z',
  nowIso: () => nowIso,
  utcStartOfDayIsoTimestamp: () => '2026-06-24T00:00:00.000Z',
}

type HistoryRow = { day: string; tokens: number }

// A fake D1 that answers ONLY the per-day history GROUP BY query the public
// read runs. It proves the route reaches the canonical ledger SQL path, not a
// stub, and returns the rows already ordered ascending by day (as the SQL does).
const fakeHistoryDb = (rows: ReadonlyArray<HistoryRow>): D1Database => {
  const prepare = () => ({
    bind: () => prepare(),
    all: <T>(): Promise<{ results: ReadonlyArray<T> }> =>
      Promise.resolve({ results: rows as ReadonlyArray<T> }),
  })

  return { prepare } as unknown as D1Database
}

const routeInput = (
  rows: ReadonlyArray<HistoryRow>,
): Parameters<typeof handlePublicKhalaTokensServedHistoryApi>[1] => ({
  ledger: makeD1TokenUsageLedger(fakeHistoryDb(rows), runtime),
  nowIso: () => nowIso,
})

const getRequest = (search = ''): Request =>
  new Request(
    `https://openagents.com/api/public/khala-tokens-served/history${search}`,
    { method: 'GET' },
  )

describe('GET /api/public/khala-tokens-served/history', () => {
  test('returns the per-day series from the D1 ledger', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedHistoryApi(
        getRequest('?window=7d&bucket=day'),
        routeInput([
          { day: '2026-06-22', tokens: 1_000 },
          { day: '2026-06-23', tokens: 2_500 },
          { day: '2026-06-24', tokens: 4_200 },
        ]),
      ),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toContain('no-store')

    const body = (await response.json()) as Record<string, unknown>

    expect(body.schemaVersion).toBe(
      'openagents.public_khala_tokens_served_history.v1',
    )
    expect(body.window).toBe('7d')
    expect(body.bucket).toBe('day')
    expect(body.generatedAt).toBe(nowIso)
    expect(body.staleness).toMatchObject({
      composition: 'live_at_read',
      maxStalenessSeconds: 0,
    })
    expect(body.series).toEqual([
      { day: '2026-06-22', tokensServed: 1_000 },
      { day: '2026-06-23', tokensServed: 2_500 },
      { day: '2026-06-24', tokensServed: 4_200 },
    ])
  })

  test('defaults to the 30d window and day bucket', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedHistoryApi(
        getRequest(),
        routeInput([{ day: '2026-06-24', tokens: 7 }]),
      ),
    )
    const body = (await response.json()) as Record<string, unknown>

    expect(body.window).toBe('30d')
    expect(body.bucket).toBe('day')
  })

  test('empty window → empty series', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedHistoryApi(getRequest(), routeInput([])),
    )
    const body = (await response.json()) as Record<string, unknown>

    expect(response.status).toBe(200)
    expect(body.series).toEqual([])
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedHistoryApi(
        new Request(
          'https://openagents.com/api/public/khala-tokens-served/history',
          { method: 'POST' },
        ),
        routeInput([]),
      ),
    )

    expect(response.status).toBe(405)
  })

  test('is public-safe: bare day + sum, no per-user or secret material', async () => {
    // An injected ledger that, if leaky, would surface private fields on each
    // point. The route must project ONLY { day, tokensServed } per point and the
    // fixed envelope fields regardless of what the ledger holds.
    const leakyLedger: TokenUsageLedgerShape = {
      ...makeD1TokenUsageLedger(fakeHistoryDb([]), runtime),
      readPublicTokensServedHistory: () =>
        Effect.succeed({
          bucket: 'day',
          window: '30d',
          series: [
            {
              day: '2026-06-24',
              tokensServed: 50,
              // These extra fields must never ride along into the response.
              actorUserId: 'user_secret',
              provider: 'anthropic',
            } as unknown as { day: string; tokensServed: number },
          ],
        }),
    }

    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedHistoryApi(getRequest(), {
        ledger: leakyLedger,
        nowIso: () => nowIso,
      }),
    )
    const body = (await response.json()) as Record<string, unknown>

    // The envelope is EXACTLY these fields — nothing else can ride along.
    expect(Object.keys(body).sort()).toEqual([
      'bucket',
      'generatedAt',
      'schemaVersion',
      'series',
      'staleness',
      'window',
    ])

    const series = body.series as ReadonlyArray<Record<string, unknown>>
    expect(series).toHaveLength(1)
    // Each point is bare day + sum: no actor/provider/secret material leaks.
    expect(Object.keys(series[0]!).sort()).toEqual(['day', 'tokensServed'])

    const forbiddenKeys = [
      'actorUserId',
      'actorTeamId',
      'accountRef',
      'userId',
      'teamId',
      'email',
      'provider',
      'model',
      'apiKey',
      'authorization',
      'secret',
      'mnemonic',
    ]
    for (const key of forbiddenKeys) {
      expect(Object.keys(series[0]!)).not.toContain(key)
    }
  })
})
