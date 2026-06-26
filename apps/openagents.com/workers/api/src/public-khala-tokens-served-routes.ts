import { Schema as S } from 'effect'
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  type TokenUsageLedgerShape,
  makeD1TokenUsageLedger,
} from './token-usage-ledger'

// The served public projection: the aggregate scalar plus the shared
// public-projection staleness contract (generatedAt + staleness). The counter
// is composed LIVE from the ledger at request time (`live_at_read`), so it can
// never be older than the request. Public-safe: aggregate only — no per-user,
// per-team, provider, or secret material. The scalar excludes
// `demand_kind=internal` dogfood/ops probes while keeping other
// Khala-orchestrated demand classes public-countable.
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
}>

// "Khala Tokens Served" is the homepage's network-wide aggregate and the value
// shown by `khala tokens` / `/tokens`. Keep this scalar endpoint truly
// live-at-read: a just-served Khala turn must be visible on the next poll rather
// than hidden behind an in-isolate cache. The response remains `no-store` so
// browsers and CLIs never reuse a frozen copy. The homepage's high-frequency
// live updates still use the PUSH path; this route is the canonical scalar
// read/reconcile path.

export const handlePublicKhalaTokensServedApi = (
  request: Request,
  input: PublicKhalaTokensServedRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const nowIso = input.nowIso ?? currentIsoTimestamp
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
