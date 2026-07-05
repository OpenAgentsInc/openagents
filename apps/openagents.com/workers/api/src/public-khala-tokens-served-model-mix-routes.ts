import {
  PublicKhalaTokensServedHistoryWindow,
  PublicKhalaTokensServedModelFamily,
} from '@openagentsinc/sync-schema'
import { Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import type { KhalaSyncHyperdriveBinding } from './khala-sync-push-routes'
import {
  readTokensServedModelMixSnapshotCached,
  TOKENS_SERVED_AGGREGATES_MAX_STALENESS_SECONDS,
  type TokensServedAggregatesReadDeps,
} from './khala-sync-public-tokens-served-mix'
import {
  liveAtReadStaleness,
  PublicProjectionStalenessContract,
  storedSnapshotStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  normalizeLeaderboardWindow,
  type TokenUsageLedgerShape,
} from './token-usage-ledger'
import {
  tokenUsageLedgerFromRouteInput,
  type TokenLedgerRouteEnvSlice,
} from './token-ledger-store'

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

// KS-6.7 (#8417): scope.public.tokens-served-aggregates rebuildsOn label —
// the projection is refreshed from the token-usage ledger's own ingest
// observer (see khala-sync-public-tokens-served-mix.ts's module doc).
const TOKENS_SERVED_AGGREGATES_REBUILDS_ON = [
  'scope.public.tokens-served-aggregates',
]

type PublicKhalaTokensServedModelMixRouteInput = TokenLedgerRouteEnvSlice &
  Readonly<{
  OPENAGENTS_DB?: D1Database
  ledger?: TokenUsageLedgerShape
  nowIso?: () => string
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding
  /** Injectable projection-read seams (tests). */
  projectionReadDeps?: Omit<TokensServedAggregatesReadDeps, 'binding' | 'ledger'>
}>

export const handlePublicKhalaTokensServedModelMixApi = (
  request: Request,
  input: PublicKhalaTokensServedModelMixRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const url = new URL(request.url)
  const rawWindow = url.searchParams.get('window') ?? undefined
  const nowIso = input.nowIso ?? currentIsoTimestamp
  const ledger =
    input.ledger ?? tokenUsageLedgerFromRouteInput(input)

  // KS-6.7 (#8417): projection FIRST, fail-open fallback to the existing
  // live-at-read ledger call on any miss (binding absent, Postgres
  // unreachable, or the window not projected yet) — same discipline as
  // KS-6.3's tokens-served counter and KS-6.4's settled-feed route.
  return normalizeLeaderboardWindow(rawWindow ?? '30d').pipe(
    Effect.flatMap(window =>
      Effect.promise(() =>
        readTokensServedModelMixSnapshotCached(
          { binding: input.KHALA_SYNC_DB, ...input.projectionReadDeps },
          window,
        ),
      ).pipe(
        Effect.flatMap(snapshot => {
          if (snapshot !== undefined) {
            const payload: PublicKhalaTokensServedModelMixResponse = {
              schemaVersion: 'openagents.public_khala_model_mix.v1',
              window: snapshot.window,
              liveAt: nowIso(),
              totalTokens: snapshot.totalTokens,
              groups: snapshot.groups,
              generatedAt: snapshot.generatedAt,
              staleness: storedSnapshotStaleness(
                TOKENS_SERVED_AGGREGATES_MAX_STALENESS_SECONDS,
                TOKENS_SERVED_AGGREGATES_REBUILDS_ON,
              ),
            }
            return Effect.succeed(noStoreJsonResponse(payload))
          }

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
                staleness: liveAtReadStaleness(['token_usage_events']),
              }

              return noStoreJsonResponse(payload)
            }),
          )
        }),
      ),
    ),
    Effect.catch(() =>
      Effect.succeed(
        noStoreJsonResponse({ error: 'internal_server_error' }, { status: 500 }),
      ),
    ),
  )
}
