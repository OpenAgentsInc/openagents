import {
  PublicKhalaTokensServedHistoryWindow,
  PublicKhalaTokensServedModelFamily,
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

export const PublicKhalaTokensServedModelMixFamily = S.Struct({
  family: PublicKhalaTokensServedModelFamily,
  tokensServed: S.Int,
  usageEvents: S.Int,
  share: S.Number,
})

export const PublicKhalaTokensServedModelMixResponse = S.Struct({
  schemaVersion: S.Literal('openagents.public_khala_tokens_served_model_mix.v1'),
  window: PublicKhalaTokensServedHistoryWindow,
  totalTokensServed: S.Int,
  families: S.Array(PublicKhalaTokensServedModelMixFamily),
  generatedAt: S.String,
  staleness: PublicProjectionStalenessContract,
})
export type PublicKhalaTokensServedModelMixResponse =
  typeof PublicKhalaTokensServedModelMixResponse.Type

type PublicKhalaTokensServedModelMixRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  ledger?: TokenUsageLedgerShape
  nowIso?: () => string
}>

export const handlePublicKhalaTokensServedModelMixApi = (
  request: Request,
  input: PublicKhalaTokensServedModelMixRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const url = new URL(request.url)
  const window = url.searchParams.get('window') ?? undefined
  const nowIso = input.nowIso ?? currentIsoTimestamp
  const ledger =
    input.ledger ?? makeD1TokenUsageLedger(input.OPENAGENTS_DB as D1Database)

  return ledger.readPublicTokensServedModelMix({ window }).pipe(
    Effect.map(mix => {
      const payload: PublicKhalaTokensServedModelMixResponse = {
        schemaVersion: 'openagents.public_khala_tokens_served_model_mix.v1',
        window: mix.window,
        totalTokensServed: mix.totalTokensServed,
        families: mix.families.map(family => ({
          family: family.family,
          tokensServed: family.tokensServed,
          usageEvents: family.usageEvents,
          share: family.share,
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
