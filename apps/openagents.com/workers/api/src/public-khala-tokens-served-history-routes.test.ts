import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { handlePublicKhalaTokensServedHistoryApi } from './public-khala-tokens-served-history-routes'
import { dayKeyInTimezone } from './runtime-primitives'
import {
  makeD1TokenUsageLedger,
  type TokenUsageLedgerRuntime,
  type TokenUsageLedgerShape,
} from './token-usage-ledger'

const nowIso = '2026-06-26T12:00:00.000Z'

const runtime: TokenUsageLedgerRuntime = {
  // 30d window → since = now - 30 days. The fake DB ignores the bound and
  // returns the canned rows, so the value is irrelevant to the assertions.
  isoTimestampAfterIso: () => '2026-05-25T12:00:00.000Z',
  nowIso: () => nowIso,
  utcStartOfDayIsoTimestamp: () => '2026-06-24T00:00:00.000Z',
}

type HistoryRow = { day: string; tokens: number }
type RawTokenRow = {
  observed_at: string
  input_tokens: number
  output_tokens: number
}

// A fake D1 that answers ONLY the per-day history GROUP BY query the public
// read runs. It proves the route reaches the canonical ledger SQL path, not a
// stub, and returns the rows already ordered ascending by day (as the SQL does).
const fakeHistoryDb = (
  rows: ReadonlyArray<HistoryRow | RawTokenRow>,
): D1Database => {
  const prepare = (sql = '') => {
    let values: ReadonlyArray<unknown> = []

    const statement = {
      bind: (...nextValues: ReadonlyArray<unknown>) => {
        values = nextValues

        return statement
      },
      all: <T>(): Promise<{ results: ReadonlyArray<T> }> =>
        Promise.resolve({
          results: sql.includes(
            'FROM public_khala_tokens_served_daily_rollups',
          )
            ? (groupRowsByRollupDays(rows, values) as ReadonlyArray<T>)
            : sql.includes('WITH bounded_token_usage_events AS')
              ? (groupRawRowsByBoundedDays(rows, values) as ReadonlyArray<T>)
              : sql.includes('date(observed_at)')
                ? (groupRawRowsByUtcDay(rows) as ReadonlyArray<T>)
                : (rows as ReadonlyArray<T>),
        }),
      first: <T>(): Promise<T | null> =>
        Promise.resolve(
          sql.includes('MIN(observed_at) AS first_observed_at')
            ? ({
                first_observed_at:
                  rows
                    .filter((row): row is RawTokenRow => !('day' in row))
                    .map(row => row.observed_at)
                    .sort((left, right) => left.localeCompare(right))[0] ??
                  null,
              } as T)
            : sql.includes(' AS day') &&
                sql.includes(' AS tokens') &&
                sql.includes('COUNT(*) AS usage_events')
              ? (partialDayRow(rows, sql, values) as T)
            : null,
        ),
    }

    return statement
  }

  return { prepare } as unknown as D1Database
}

const groupRowsByRollupDays = (
  rows: ReadonlyArray<HistoryRow | RawTokenRow>,
  values: ReadonlyArray<unknown>,
): ReadonlyArray<{ day: string; tokens_served: number }> => {
  const timezone = String(values[0])
  const startDay = String(values[1])
  const endDay = String(values[2])
  const historyRows = rows.filter((row): row is HistoryRow => 'day' in row)
  if (historyRows.length > 0) {
    return historyRows
      .filter(row => row.day >= startDay && row.day <= endDay)
      .map(row => ({ day: row.day, tokens_served: row.tokens }))
  }

  const grouped = rows
    .filter((row): row is RawTokenRow => !('day' in row))
    .reduce((days, row) => {
      const day = dayKeyInTimezone(row.observed_at, timezone)
      if (day === undefined || day < startDay || day > endDay) {
        return days
      }

      days.set(
        day,
        (days.get(day) ?? 0) + row.input_tokens + row.output_tokens,
      )
      return days
    }, new Map<string, number>())

  return [...grouped.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([day, tokens]) => ({ day, tokens_served: tokens }))
}

const partialDayRow = (
  rows: ReadonlyArray<HistoryRow | RawTokenRow>,
  sql: string,
  values: ReadonlyArray<unknown>,
): { day: string | null; tokens: number; usage_events: number } => {
  const day = sql.match(/SELECT\s+'([^']+)'\s+AS day/)?.[1] ?? null
  const startIso = String(values[0])
  const endIso = String(values[1])
  const matchingRows = rows.filter(
    (row): row is RawTokenRow =>
      !('day' in row) &&
      row.observed_at >= startIso &&
      row.observed_at < endIso,
  )

  return {
    day,
    tokens: matchingRows.reduce(
      (sum, row) => sum + row.input_tokens + row.output_tokens,
      0,
    ),
    usage_events: matchingRows.length,
  }
}

const groupRawRowsByBoundedDays = (
  rows: ReadonlyArray<HistoryRow | RawTokenRow>,
  values: ReadonlyArray<unknown>,
): ReadonlyArray<HistoryRow> => {
  const dayWindows: Array<{
    day: string
    endIso: string
    startIso: string
  }> = []
  for (let index = 2; index + 2 < values.length; index += 3) {
    dayWindows.push({
      day: String(values[index + 2]),
      endIso: String(values[index + 1]),
      startIso: String(values[index]),
    })
  }

  const grouped = rows
    .filter((row): row is RawTokenRow => !('day' in row))
    .reduce((days, row) => {
      const dayWindow = dayWindows.find(
        window =>
          row.observed_at >= window.startIso && row.observed_at < window.endIso,
      )
      if (dayWindow === undefined) {
        return days
      }

      days.set(
        dayWindow.day,
        (days.get(dayWindow.day) ?? 0) + row.input_tokens + row.output_tokens,
      )
      return days
    }, new Map<string, number>())

  return [...grouped.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([day, tokens]) => ({ day, tokens }))
}

const groupRawRowsByUtcDay = (
  rows: ReadonlyArray<HistoryRow | RawTokenRow>,
): ReadonlyArray<HistoryRow> => {
  const historyRows = rows.filter((row): row is HistoryRow => 'day' in row)
  const grouped = rows
    .filter((row): row is RawTokenRow => !('day' in row))
    .reduce((days, row) => {
      const day = row.observed_at.slice(0, 10)
      days.set(
        day,
        (days.get(day) ?? 0) + row.input_tokens + row.output_tokens,
      )
      return days
    }, new Map<string, number>())

  return historyRows.length > 0
    ? historyRows
    : [...grouped.entries()]
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([day, tokens]) => ({ day, tokens }))
}

const routeInput = (
  rows: ReadonlyArray<HistoryRow | RawTokenRow>,
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
          {
            observed_at: '2026-06-22T12:00:00.000Z',
            input_tokens: 700,
            output_tokens: 300,
          },
          {
            observed_at: '2026-06-23T12:00:00.000Z',
            input_tokens: 2_000,
            output_tokens: 500,
          },
          {
            observed_at: '2026-06-24T12:00:00.000Z',
            input_tokens: 4_000,
            output_tokens: 200,
          },
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
      composition: 'rebuilt_on_transition',
      maxStalenessSeconds: 0,
      rebuildsOn: ['token_usage_events_insert'],
    })
    expect(body.series).toEqual([
      { day: '2026-06-22', tokensServed: 1_000 },
      { day: '2026-06-23', tokensServed: 2_500 },
      { day: '2026-06-24', tokensServed: 4_200 },
    ])
  })

  test('defaults to the 30d window, day bucket, and America/Chicago timezone', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedHistoryApi(
        getRequest(),
        routeInput([
          {
            observed_at: '2026-06-25T04:30:00.000Z',
            input_tokens: 6,
            output_tokens: 4,
          },
        ]),
      ),
    )
    const body = (await response.json()) as Record<string, unknown>

    expect(body.window).toBe('30d')
    expect(body.bucket).toBe('day')
    expect(body.timezone).toBe('America/Chicago')
    expect(body.series).toEqual([{ day: '2026-06-24', tokensServed: 10 }])
  })

  test('uses America/Chicago by default and supports explicit UTC at the day boundary', async () => {
    const rows: ReadonlyArray<RawTokenRow> = [
      {
        observed_at: '2026-06-25T04:30:00.000Z',
        input_tokens: 6,
        output_tokens: 4,
      },
      {
        observed_at: '2026-06-25T05:30:00.000Z',
        input_tokens: 20,
        output_tokens: 5,
      },
    ]

    const utcResponse = await Effect.runPromise(
      handlePublicKhalaTokensServedHistoryApi(
        getRequest('?window=7d&bucket=day&tz=UTC'),
        routeInput(rows),
      ),
    )
    const chicagoResponse = await Effect.runPromise(
      handlePublicKhalaTokensServedHistoryApi(
        getRequest('?window=7d&bucket=day'),
        routeInput(rows),
      ),
    )

    const utcBody = (await utcResponse.json()) as Record<string, unknown>
    const chicagoBody =
      (await chicagoResponse.json()) as Record<string, unknown>

    expect(utcBody.timezone).toBe('UTC')
    expect(utcBody.series).toEqual([{ day: '2026-06-25', tokensServed: 35 }])
    expect(chicagoBody.timezone).toBe('America/Chicago')
    expect(chicagoBody.series).toEqual([
      { day: '2026-06-24', tokensServed: 10 },
      { day: '2026-06-25', tokensServed: 25 },
    ])
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
          timezone: 'UTC',
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
      'timezone',
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
