import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { handlePublicKhalaTokensServedApi } from './public-khala-tokens-served-routes'
import {
  makeD1TokenUsageLedger,
  type TokenUsageLedgerRuntime,
  type TokenUsageLedgerShape,
} from './token-usage-ledger'

const nowIso = '2026-06-24T12:00:00.000Z'

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
