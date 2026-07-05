import {
  PublicKhalaTokensServedHistoryBucket,
  PublicKhalaTokensServedHistoryWindow,
} from '@openagentsinc/sync-schema'
import { Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import type { KhalaSyncHyperdriveBinding } from './khala-sync-push-routes'
import {
  readTokensServedHistorySnapshotCached,
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
  normalizeHistoryBucket,
  normalizeHistoryTimezone,
  normalizeLeaderboardWindow,
  type TokenUsageLedgerShape,
} from './token-usage-ledger'
import {
  tokenUsageLedgerFromRouteInput,
  type TokenLedgerRouteEnvSlice,
} from './token-ledger-store'

// The served history projection: the requested window + bucket, the per-day
// series, plus the shared public-projection staleness contract
// (generatedAt + staleness). The series is maintained as a daily aggregate
// projection on each successful token ledger insert, with the rolling-window
// boundary day read from the raw ledger when needed.
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
  timezone: S.String,
  series: S.Array(PublicKhalaTokensServedHistoryPoint),
  generatedAt: S.String,
  staleness: PublicProjectionStalenessContract,
})
export type PublicKhalaTokensServedHistoryResponse =
  typeof PublicKhalaTokensServedHistoryResponse.Type

// KS-6.7 (#8417): scope.public.tokens-served-aggregates rebuildsOn label —
// the projection is refreshed from the token-usage ledger's own ingest
// observer (see khala-sync-public-tokens-served-mix.ts's module doc).
const TOKENS_SERVED_AGGREGATES_REBUILDS_ON = [
  'scope.public.tokens-served-aggregates',
]

type PublicKhalaTokensServedHistoryRouteInput = TokenLedgerRouteEnvSlice &
  Readonly<{
  OPENAGENTS_DB?: D1Database
  // Tests inject an in-memory ledger; production builds the D1-backed one.
  ledger?: TokenUsageLedgerShape
  nowIso?: () => string
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding
  /** Injectable projection-read seams (tests). */
  projectionReadDeps?: Omit<TokensServedAggregatesReadDeps, 'binding' | 'ledger'>
}>

export const handlePublicKhalaTokensServedHistoryApi = (
  request: Request,
  input: PublicKhalaTokensServedHistoryRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const nowIso = input.nowIso ?? currentIsoTimestamp

  const url = new URL(request.url)
  const rawWindow = url.searchParams.get('window') ?? undefined
  const rawBucket = url.searchParams.get('bucket') ?? undefined
  const rawTimezone =
    url.searchParams.get('timezone') ?? url.searchParams.get('tz') ?? undefined

  const ledger =
    input.ledger ?? tokenUsageLedgerFromRouteInput(input)

  // KS-6.7 (#8417): projection FIRST (only the refreshed default window +
  // timezone are ever stored today — see
  // TOKENS_SERVED_AGGREGATES_HISTORY_TIMEZONE), fail-open fallback to the
  // existing live-at-read ledger call on any miss (binding absent, Postgres
  // unreachable, non-default timezone, or the window not projected yet).
  return Effect.all([
    normalizeLeaderboardWindow(rawWindow ?? '30d'),
    normalizeHistoryBucket(rawBucket),
    normalizeHistoryTimezone(rawTimezone),
  ]).pipe(
    Effect.flatMap(([window, bucket, timezone]) =>
      Effect.promise(() =>
        readTokensServedHistorySnapshotCached(
          { binding: input.KHALA_SYNC_DB, ...input.projectionReadDeps },
          window,
          timezone,
        ),
      ).pipe(
        Effect.flatMap(snapshot => {
          if (snapshot !== undefined) {
            const payload: PublicKhalaTokensServedHistoryResponse = {
              schemaVersion:
                'openagents.public_khala_tokens_served_history.v1',
              window: snapshot.window,
              bucket: snapshot.bucket,
              timezone: snapshot.timezone,
              series: snapshot.series,
              generatedAt: snapshot.generatedAt,
              staleness: storedSnapshotStaleness(
                TOKENS_SERVED_AGGREGATES_MAX_STALENESS_SECONDS,
                TOKENS_SERVED_AGGREGATES_REBUILDS_ON,
              ),
            }
            return Effect.succeed(noStoreJsonResponse(payload))
          }

          return ledger
            .readPublicTokensServedHistory({ bucket, timezone, window })
            .pipe(
              Effect.map(history => {
                const payload: PublicKhalaTokensServedHistoryResponse = {
                  schemaVersion:
                    'openagents.public_khala_tokens_served_history.v1',
                  window: history.window,
                  bucket: history.bucket,
                  timezone: history.timezone,
                  series: history.series.map(point => ({
                    day: point.day,
                    tokensServed: point.tokensServed,
                  })),
                  generatedAt: nowIso(),
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
