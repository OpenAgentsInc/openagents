import {
  PublicKhalaTokensServedHistoryWindow,
  PublicKhalaTokensServedModelFamily,
} from '@openagentsinc/sync-schema'
import { Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  PublicProjectionStalenessContract,
  rebuiltOnTransitionStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  type TokenUsageLedgerShape,
  makeD1TokenUsageLedger,
} from './token-usage-ledger'

export const PublicKhalaTokensServedModelMixGroup = S.Struct({
  family: PublicKhalaTokensServedModelFamily,
  label: S.String,
  tokens: S.Int,
  reqs: S.Int,
  pct: S.Number,
})

export const PublicKhalaTokensServedModelMixResponse = S.Struct({
  schemaVersion: S.Literal('openagents.public_khala_model_mix.v1'),
  window: PublicKhalaTokensServedHistoryWindow,
  liveAt: S.String,
  totalTokens: S.Int,
  groups: S.Array(PublicKhalaTokensServedModelMixGroup),
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
      const readAt = nowIso()
      const payload: PublicKhalaTokensServedModelMixResponse = {
        schemaVersion: 'openagents.public_khala_model_mix.v1',
        window: mix.window,
        liveAt: readAt,
        totalTokens: mix.totalTokens,
        groups: mix.groups.map(group => ({
          family: group.family,
          label: group.label,
          tokens: group.tokens,
          reqs: group.reqs,
          pct: group.pct,
        })),
        generatedAt: readAt,
        staleness: rebuiltOnTransitionStaleness(0, [
          'token_usage_events_insert',
        ]),
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
