import { Effect } from 'effect'
import { beforeEach, describe, expect, test } from 'vitest'

import type { KhalaSyncPushSqlClient } from './khala-sync-push-routes'
import {
  resetTokensServedProjectionCacheForTests,
  TOKENS_SERVED_PROJECTION_MAX_STALENESS_SECONDS,
} from './khala-sync-public-tokens-served'
import { handlePublicKhalaTokensServedApi } from './public-khala-tokens-served-routes'
import {
  makeD1TokenUsageLedger,
  type TokenUsageLedgerRuntime,
  type TokenUsageLedgerShape,
} from './token-usage-ledger'

const nowIso = '2026-06-24T12:00:00.000Z'

// The projection reader keeps a module-level in-isolate cache — start every
// test from a cold cache so tests stay order-independent.
beforeEach(() => {
  resetTokensServedProjectionCacheForTests()
})

const runtime: TokenUsageLedgerRuntime = {
  isoTimestampAfterIso: () => nowIso,
  nowIso: () => nowIso,
  utcStartOfDayIsoTimestamp: () => nowIso,
}

// A fake D1 that answers ONLY the tokens-served SUM query the public read runs.
// It proves the route reaches the canonical ledger SQL path, not a stub.
const fakeTokensServedDb = (
  inputTokens: number,
  outputTokens: number,
): D1Database => {
  const prepare = () => ({
    bind: () => prepare(),
    first: <T>(): Promise<T> =>
      Promise.resolve({
        tokens_served: inputTokens + outputTokens,
      } as T),
  })

  return { prepare } as unknown as D1Database
}

const fakeMutableTokensServedDb = (
  readTokensServed: () => number,
): D1Database => {
  const prepare = () => ({
    bind: () => prepare(),
    first: <T>(): Promise<T> =>
      Promise.resolve({
        tokens_served: readTokensServed(),
      } as T),
  })

  return { prepare } as unknown as D1Database
}

const routeInput = (
  inputTokens: number,
  outputTokens: number,
): Parameters<typeof handlePublicKhalaTokensServedApi>[1] => ({
  ledger: makeD1TokenUsageLedger(
    fakeTokensServedDb(inputTokens, outputTokens),
    runtime,
  ),
  nowIso: () => nowIso,
})

const getRequest = (): Request =>
  new Request('https://openagents.com/api/public/khala-tokens-served', {
    method: 'GET',
  })

describe('GET /api/public/khala-tokens-served', () => {
  test('returns the real input+output aggregate from the D1 ledger', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedApi(getRequest(), routeInput(900_000, 350_000)),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toContain('no-store')

    const body = (await response.json()) as Record<string, unknown>

    expect(body.schemaVersion).toBe('openagents.public_khala_tokens_served.v1')
    expect(body.tokensServed).toBe(1_250_000)
    expect(body.generatedAt).toBe(nowIso)
    // The shared public-projection staleness contract is declared on the payload.
    expect(body.staleness).toMatchObject({
      composition: 'live_at_read',
      maxStalenessSeconds: 0,
    })
  })

  test('counts up: a later poll reflects the larger ledger total', async () => {
    const first = await Effect.runPromise(
      handlePublicKhalaTokensServedApi(getRequest(), routeInput(10, 5)),
    )
    const later = await Effect.runPromise(
      handlePublicKhalaTokensServedApi(getRequest(), routeInput(100, 50)),
    )

    const firstBody = (await first.json()) as { tokensServed: number }
    const laterBody = (await later.json()) as { tokensServed: number }

    expect(firstBody.tokensServed).toBe(15)
    expect(laterBody.tokensServed).toBe(150)
    expect(laterBody.tokensServed).toBeGreaterThan(firstBody.tokensServed)
  })

  test('production D1 path is live-at-read instead of using an isolate cache', async () => {
    let tokensServed = 15
    const input: Parameters<typeof handlePublicKhalaTokensServedApi>[1] = {
      OPENAGENTS_DB: fakeMutableTokensServedDb(() => tokensServed),
      nowIso: () => nowIso,
    }

    const first = await Effect.runPromise(
      handlePublicKhalaTokensServedApi(getRequest(), input),
    )
    tokensServed = 150
    const later = await Effect.runPromise(
      handlePublicKhalaTokensServedApi(getRequest(), input),
    )

    const firstBody = (await first.json()) as { tokensServed: number }
    const laterBody = (await later.json()) as { tokensServed: number }

    expect(firstBody.tokensServed).toBe(15)
    expect(laterBody.tokensServed).toBe(150)
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedApi(
        new Request(
          'https://openagents.com/api/public/khala-tokens-served',
          { method: 'POST' },
        ),
        routeInput(0, 0),
      ),
    )

    expect(response.status).toBe(405)
  })

  test('is public-safe: aggregate only, no per-user or secret material', async () => {
    // An injected ledger that, if leaky, would surface private fields. The route
    // must return ONLY the aggregate + projection-contract shape regardless of
    // what the ledger holds.
    const leakyLedger: TokenUsageLedgerShape = {
      ...makeD1TokenUsageLedger(fakeTokensServedDb(42, 8), runtime),
      readPublicTokensServed: () => Effect.succeed({ tokensServed: 50 }),
    }

    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedApi(getRequest(), {
        ledger: leakyLedger,
        nowIso: () => nowIso,
      }),
    )
    const body = (await response.json()) as Record<string, unknown>

    // The public projection is EXACTLY these four fields — nothing else can
    // ride along, so no per-user/secret material can ever leak.
    expect(Object.keys(body).sort()).toEqual([
      'generatedAt',
      'schemaVersion',
      'staleness',
      'tokensServed',
    ])

    const forbiddenKeys = [
      'actor',
      'actorUserId',
      'actorTeamId',
      'accountRef',
      'userId',
      'teamId',
      'email',
      'provider',
      'model',
      'recentEvents',
      'byActor',
      'safeMetadata',
      'apiKey',
      'authorization',
      'secret',
      'mnemonic',
    ]
    for (const key of forbiddenKeys) {
      expect(Object.keys(body)).not.toContain(key)
    }
  })
})

// ---------------------------------------------------------------------------
// KS-6.3 (#8304): projection-first serving with fail-open D1 fallback
// ---------------------------------------------------------------------------

const fakeSqlClient = (): KhalaSyncPushSqlClient => ({
  end: async () => undefined,
  sql: (() => {
    throw new Error('unused by injected readProjection')
  }) as unknown as KhalaSyncPushSqlClient['sql'],
})

const explodingDb = (): D1Database =>
  ({
    prepare: () => {
      throw new Error('D1 must not be touched when the projection serves')
    },
  }) as unknown as D1Database

describe('GET /api/public/khala-tokens-served — projection path (KS-6.3 #8304)', () => {
  test('serves the projection (no D1 SUM on the hot path) with the honest 2s staleness contract', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedApi(getRequest(), {
        KHALA_SYNC_DB: { connectionString: 'postgres://hyperdrive' },
        // If the route ran the D1 SUM, this database would throw.
        OPENAGENTS_DB: explodingDb(),
        nowIso: () => nowIso,
        projectionReadDeps: {
          makeSqlClient: async () => fakeSqlClient(),
          readProjection: async () => ({
            lastEventAt: '2026-06-24T11:59:59.000Z',
            total: 8_555_123,
          }),
        },
      }),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toContain('no-store')
    const body = (await response.json()) as Record<string, unknown>
    expect(body.tokensServed).toBe(8_555_123)
    expect(body.staleness).toMatchObject({
      composition: 'rebuilt_on_transition',
      maxStalenessSeconds: TOKENS_SERVED_PROJECTION_MAX_STALENESS_SECONDS,
      rebuildsOn: ['token_usage_events'],
    })
    // Same public shape as the fallback path — nothing extra rides along.
    expect(Object.keys(body).sort()).toEqual([
      'generatedAt',
      'schemaVersion',
      'staleness',
      'tokensServed',
    ])
  })

  test('falls back (fail-open) to the live D1 SUM when the projection read fails', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedApi(getRequest(), {
        KHALA_SYNC_DB: { connectionString: 'postgres://hyperdrive' },
        ledger: makeD1TokenUsageLedger(fakeTokensServedDb(900, 100), runtime),
        nowIso: () => nowIso,
        projectionReadDeps: {
          makeSqlClient: async () => fakeSqlClient(),
          readProjection: async () => {
            throw new Error('postgres unreachable')
          },
        },
      }),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.tokensServed).toBe(1_000)
    expect(body.staleness).toMatchObject({
      composition: 'live_at_read',
      maxStalenessSeconds: 0,
    })
  })

  test('falls back to the live D1 SUM before the counter is backfilled (null row)', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedApi(getRequest(), {
        KHALA_SYNC_DB: { connectionString: 'postgres://hyperdrive' },
        ledger: makeD1TokenUsageLedger(fakeTokensServedDb(10, 5), runtime),
        nowIso: () => nowIso,
        projectionReadDeps: {
          makeSqlClient: async () => fakeSqlClient(),
          readProjection: async () => null,
        },
      }),
    )

    const body = (await response.json()) as Record<string, unknown>
    expect(body.tokensServed).toBe(15)
    expect(body.staleness).toMatchObject({ composition: 'live_at_read' })
  })

  test('falls back to the live D1 SUM when the KHALA_SYNC_DB binding is absent', async () => {
    const response = await Effect.runPromise(
      handlePublicKhalaTokensServedApi(getRequest(), {
        ledger: makeD1TokenUsageLedger(fakeTokensServedDb(1, 2), runtime),
        nowIso: () => nowIso,
      }),
    )

    const body = (await response.json()) as Record<string, unknown>
    expect(body.tokensServed).toBe(3)
    expect(body.staleness).toMatchObject({ composition: 'live_at_read' })
  })

  test('caches the projection read for the declared window, then re-reads', async () => {
    let projectionTotal = 1_000
    let reads = 0
    let currentMs = 0
    const input: Parameters<typeof handlePublicKhalaTokensServedApi>[1] = {
      KHALA_SYNC_DB: { connectionString: 'postgres://hyperdrive' },
      OPENAGENTS_DB: explodingDb(),
      nowIso: () => nowIso,
      projectionReadDeps: {
        makeSqlClient: async () => fakeSqlClient(),
        nowMs: () => currentMs,
        readProjection: async () => {
          reads += 1
          return { lastEventAt: null, total: projectionTotal }
        },
      },
    }

    const first = await Effect.runPromise(
      handlePublicKhalaTokensServedApi(getRequest(), input),
    )
    projectionTotal = 2_000
    currentMs = 1_000 // inside the 2s window — served from the cache
    const cached = await Effect.runPromise(
      handlePublicKhalaTokensServedApi(getRequest(), input),
    )
    currentMs = 2_500 // past the window — a fresh read sees the new total
    const fresh = await Effect.runPromise(
      handlePublicKhalaTokensServedApi(getRequest(), input),
    )

    expect(((await first.json()) as { tokensServed: number }).tokensServed).toBe(
      1_000,
    )
    expect(
      ((await cached.json()) as { tokensServed: number }).tokensServed,
    ).toBe(1_000)
    expect(((await fresh.json()) as { tokensServed: number }).tokensServed).toBe(
      2_000,
    )
    expect(reads).toBe(2)
  })
})
