import {
  PublicKhalaTokensServedHistoryWindow,
  TokenUsageDemandKind,
} from '@openagentsinc/sync-schema'
import { Effect, Schema as S } from 'effect'

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

export const PublicKhalaTokensServedDemandMixGroup = S.Struct({
  kind: TokenUsageDemandKind,
  source: S.String,
  client: S.String,
  tokens: S.Int,
  reqs: S.Int,
  pct: S.Number,
})

export const PublicKhalaTokensServedDemandMixResponse = S.Struct({
  schemaVersion: S.Literal('openagents.public_khala_demand_mix.v1'),
  window: PublicKhalaTokensServedHistoryWindow,
  totalTokens: S.Int,
  groups: S.Array(PublicKhalaTokensServedDemandMixGroup),
  generatedAt: S.String,
  staleness: PublicProjectionStalenessContract,
})
export type PublicKhalaTokensServedDemandMixResponse =
  typeof PublicKhalaTokensServedDemandMixResponse.Type

type PublicKhalaTokensServedDemandMixRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  ledger?: TokenUsageLedgerShape
  nowIso?: () => string
}>

export const handlePublicKhalaTokensServedDemandMixApi = (
  request: Request,
  input: PublicKhalaTokensServedDemandMixRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const url = new URL(request.url)
  const window = url.searchParams.get('window') ?? undefined
  const nowIso = input.nowIso ?? currentIsoTimestamp
  const ledger =
    input.ledger ?? makeD1TokenUsageLedger(input.OPENAGENTS_DB as D1Database)

  return ledger.readPublicTokensServedDemandMix({ window }).pipe(
    Effect.map(mix => {
      const payload: PublicKhalaTokensServedDemandMixResponse = {
        schemaVersion: 'openagents.public_khala_demand_mix.v1',
        window: mix.window,
        totalTokens: mix.totalTokens,
        groups: mix.groups.map(group => ({
          kind: group.kind,
          source: group.source,
          client: group.client,
          tokens: group.tokens,
          reqs: group.reqs,
          pct: group.pct,
        })),
        generatedAt: nowIso(),
        staleness: liveAtReadStaleness(['token_usage_events']),
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
