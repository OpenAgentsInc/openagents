import {
  PublicKhalaTokensServedHistoryBucket,
  PublicKhalaTokensServedHistoryWindow,
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

type PublicKhalaTokensServedHistoryRouteInput = TokenLedgerRouteEnvSlice &
  Readonly<{
  OPENAGENTS_DB?: D1Database
  // Tests inject an in-memory ledger; production builds the D1-backed one.
  ledger?: TokenUsageLedgerShape
  nowIso?: () => string
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
  const window = url.searchParams.get('window') ?? undefined
  const bucket = url.searchParams.get('bucket') ?? undefined
  const timezone =
    url.searchParams.get('timezone') ?? url.searchParams.get('tz') ?? undefined

  const ledger =
    input.ledger ?? tokenUsageLedgerFromRouteInput(input)

  return ledger.readPublicTokensServedHistory({ bucket, timezone, window }).pipe(
    Effect.map(history => {
      const payload: PublicKhalaTokensServedHistoryResponse = {
        schemaVersion: 'openagents.public_khala_tokens_served_history.v1',
        window: history.window,
        bucket: history.bucket,
        timezone: history.timezone,
        series: history.series.map(point => ({
          day: point.day,
          tokensServed: point.tokensServed,
        })),
        generatedAt: nowIso(),
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
