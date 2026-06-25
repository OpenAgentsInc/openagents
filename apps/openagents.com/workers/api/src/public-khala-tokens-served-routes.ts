import { Schema as S } from 'effect'
import { Effect } from 'effect'

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

// The served public projection: the aggregate scalar plus the shared
// public-projection staleness contract (generatedAt + staleness). The counter
// is composed LIVE from the ledger at request time (`live_at_read`), so it can
// never be older than the request; the short in-isolate cache below is a perf
// detail under that contract, not a stored snapshot. Public-safe: aggregate
// only — no per-user, per-team, provider, or secret material.
export const PublicKhalaTokensServedResponse = S.Struct({
  schemaVersion: S.Literal('openagents.public_khala_tokens_served.v1'),
  tokensServed: S.Int,
  generatedAt: S.String,
  staleness: PublicProjectionStalenessContract,
})
export type PublicKhalaTokensServedResponse =
  typeof PublicKhalaTokensServedResponse.Type

type PublicKhalaTokensServedRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  // Tests inject an in-memory ledger; production builds the D1-backed one.
  ledger?: TokenUsageLedgerShape
  nowIso?: () => string
  nowUnixMs?: () => number
}>

// "Khala Tokens Served" is the homepage's live network-wide aggregate. The
// homepage polls this every few seconds (subscriptions.ts) and several visitors
// can hit it at once; the running SUM over the full token usage ledger is a D1
// scan, so cache the computed scalar in-isolate for ~1s. That caps the D1 SUM to
// at most ~1/sec no matter how many viewers poll, while keeping the counter
// near-live (the client polls every 1s, so worst-case staleness is ~2s). The
// response stays `no-store` so each client poll gets the latest cached value,
// never a frozen browser copy. (Same shape as public-pylon-stats-routes.ts.)
const TOKENS_SERVED_CACHE_TTL_MS = 1_000
let tokensServedCache: { at: number; payload: unknown } | null = null

export const handlePublicKhalaTokensServedApi = (
  request: Request,
  input: PublicKhalaTokensServedRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const nowUnixMs = input.nowUnixMs ?? currentEpochMillis
  const nowIso = input.nowIso ?? currentIsoTimestamp

  // Only cache the real (D1-backed) production path. Tests inject their own
  // ledger and must each see their own snapshot, so they bypass the cache.
  const cacheable = input.ledger === undefined

  const cached = cacheable ? tokensServedCache : null
  if (cached !== null && nowUnixMs() - cached.at < TOKENS_SERVED_CACHE_TTL_MS) {
    return Effect.succeed(noStoreJsonResponse(cached.payload))
  }

  const ledger =
    input.ledger ?? makeD1TokenUsageLedger(input.OPENAGENTS_DB as D1Database)

  return ledger.readPublicTokensServed().pipe(
    Effect.map(aggregate => {
      const payload: PublicKhalaTokensServedResponse = {
        schemaVersion: 'openagents.public_khala_tokens_served.v1',
        tokensServed: aggregate.tokensServed,
        generatedAt: nowIso(),
        staleness: liveAtReadStaleness(['token_usage_events']),
      }
      if (cacheable) {
        tokensServedCache = { at: nowUnixMs(), payload }
      }
      return noStoreJsonResponse(payload)
    }),
    Effect.catch(() =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'internal_server_error' },
          { status: 500 },
        ),
      ),
    ),
  )
}
