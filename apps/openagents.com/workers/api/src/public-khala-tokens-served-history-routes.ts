import {
  PublicKhalaTokensServedHistoryBucket,
  PublicKhalaTokensServedHistoryWindow,
} from '@openagentsinc/sync-schema'
import { Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentEpochMillis, currentIsoTimestamp } from './runtime-primitives'
import {
  type TokenUsageLedgerShape,
  makeD1TokenUsageLedger,
} from './token-usage-ledger'

// The served history projection: the requested window + bucket, the per-day
// series, plus the shared public-projection staleness contract
// (generatedAt + staleness). Like the scalar counter, the series is composed
// LIVE from the ledger at request time (`live_at_read`); the short in-isolate
// cache below is a perf detail under that contract, not a stored snapshot.
// Public-safe: each point is a bare day + sum — no per-user, per-team,
// provider, or secret material.
export const PublicKhalaTokensServedHistoryPoint = S.Struct({
  day: S.String,
  tokensServed: S.Int,
})

export const PublicKhalaTokensServedHistoryResponse = S.Struct({
  schemaVersion: S.Literal('openagents.public_khala_tokens_served_history.v1'),
  window: PublicKhalaTokensServedHistoryWindow,
  bucket: PublicKhalaTokensServedHistoryBucket,
  series: S.Array(PublicKhalaTokensServedHistoryPoint),
  generatedAt: S.String,
  staleness: PublicProjectionStalenessContract,
})
export type PublicKhalaTokensServedHistoryResponse =
  typeof PublicKhalaTokensServedHistoryResponse.Type

type PublicKhalaTokensServedHistoryRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  // Tests inject an in-memory ledger; production builds the D1-backed one.
  ledger?: TokenUsageLedgerShape
  nowIso?: () => string
  nowUnixMs?: () => number
}>

// The /stats history graph polls this on the same short interval as the
// scalar counter (subscriptions.ts), and the per-day GROUP BY scan over the
// ledger is heavier than the single SUM. Cache the computed series in-isolate
// for a few seconds keyed on (window, bucket); reads are then instant and at
// most a few seconds stale. The response stays `no-store` so each poll gets the
// latest cached value, never a frozen browser copy. (Same shape as
// public-khala-tokens-served-routes.ts.)
const HISTORY_CACHE_TTL_MS = 4_000
const historyCache = new Map<string, { at: number; payload: unknown }>()

export const handlePublicKhalaTokensServedHistoryApi = (
  request: Request,
  input: PublicKhalaTokensServedHistoryRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const nowUnixMs = input.nowUnixMs ?? currentEpochMillis
  const nowIso = input.nowIso ?? currentIsoTimestamp

  const url = new URL(request.url)
  const window = url.searchParams.get('window') ?? undefined
  const bucket = url.searchParams.get('bucket') ?? undefined

  // Only cache the real (D1-backed) production path. Tests inject their own
  // ledger and must each see their own snapshot, so they bypass the cache.
  const cacheable = input.ledger === undefined
  const cacheKey = `${window ?? '30d'}|${bucket ?? 'day'}`

  const cached = cacheable ? historyCache.get(cacheKey) : undefined
  if (
    cached !== undefined &&
    nowUnixMs() - cached.at < HISTORY_CACHE_TTL_MS
  ) {
    return Effect.succeed(noStoreJsonResponse(cached.payload))
  }

  const ledger =
    input.ledger ?? makeD1TokenUsageLedger(input.OPENAGENTS_DB as D1Database)

  return ledger.readPublicTokensServedHistory({ bucket, window }).pipe(
    Effect.map(history => {
      const payload: PublicKhalaTokensServedHistoryResponse = {
        schemaVersion: 'openagents.public_khala_tokens_served_history.v1',
        window: history.window,
        bucket: history.bucket,
        series: history.series.map(point => ({
          day: point.day,
          tokensServed: point.tokensServed,
        })),
        generatedAt: nowIso(),
        staleness: liveAtReadStaleness(['token_usage_events']),
      }
      if (cacheable) {
        historyCache.set(cacheKey, { at: nowUnixMs(), payload })
      }
      return noStoreJsonResponse(payload)
    }),
    Effect.catch(() =>
      Effect.succeed(
        noStoreJsonResponse({ error: 'internal_server_error' }, { status: 500 }),
      ),
    ),
  )
}
