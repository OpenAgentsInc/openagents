import { Schema as S } from 'effect'
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import type { KhalaSyncHyperdriveBinding } from './khala-sync-push-routes'
import {
  readTokensServedProjectionCached,
  TOKENS_SERVED_PROJECTION_MAX_STALENESS_SECONDS,
  type TokensServedProjectionReadDeps,
} from './khala-sync-public-tokens-served'
import {
  liveAtReadStaleness,
  PublicProjectionStalenessContract,
  rebuiltOnTransitionStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  type TokenUsageLedgerShape,
  makeD1TokenUsageLedger,
} from './token-usage-ledger'

// The served public projection: the aggregate scalar plus the shared
// public-projection staleness contract (generatedAt + staleness). Public-safe:
// aggregate only — no per-user, per-team, demand label, provider, or secret
// material. The scalar includes all real served-token rows, including internal
// dogfood and owner-capacity work.
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
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding
  // Tests inject an in-memory ledger; production builds the D1-backed one.
  ledger?: TokenUsageLedgerShape
  /** Injectable projection-read seams (tests). */
  projectionReadDeps?: Omit<TokensServedProjectionReadDeps, 'binding'>
  nowIso?: () => string
}>

// "Khala Tokens Served" is the homepage's network-wide aggregate and the value
// shown by `khala tokens` / `/tokens`.
//
// SERVING ORDER (KS-6.3, #8304 — kill the full-table SUM on the hot path):
//   1. The `scope.public.tokens-served` projection: a single-row Postgres
//      read (khala_sync_public_counters via the KHALA_SYNC_DB Hyperdrive
//      binding) behind a small in-isolate cache. The projection is bumped
//      exact-once per ledger row by the ingest write path and reconciled to
//      the exact `token_usage_events` SUM (invariant 8), so the served value
//      is at most TOKENS_SERVED_PROJECTION_MAX_STALENESS_SECONDS behind the
//      ledger — declared honestly in the payload's staleness contract
//      (`rebuilt_on_transition`, the 2s public-stats precedent).
//   2. FAIL-OPEN FALLBACK: when the binding is absent, Postgres is
//      unreachable, or the counter is not backfilled yet, the route falls
//      back to the previous live-at-read D1 SUM — availability of the
//      public counter never regresses, and the fallback declares
//      `live_at_read` staleness exactly as before.
//
// The response remains `no-store` so browsers and CLIs never reuse a frozen
// copy. The homepage's high-frequency live updates still use the SYNC_ROOM
// PUSH path (unchanged); this route is the canonical scalar read/reconcile
// path.

export const handlePublicKhalaTokensServedApi = (
  request: Request,
  input: PublicKhalaTokensServedRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const nowIso = input.nowIso ?? currentIsoTimestamp

  const respond = (
    tokensServed: number,
    staleness: PublicProjectionStalenessContract,
  ) => {
    const payload: PublicKhalaTokensServedResponse = {
      schemaVersion: 'openagents.public_khala_tokens_served.v1',
      tokensServed,
      generatedAt: nowIso(),
      staleness,
    }
    return noStoreJsonResponse(payload)
  }

  const ledgerFallback = () => {
    const ledger =
      input.ledger ?? makeD1TokenUsageLedger(input.OPENAGENTS_DB as D1Database)
    return ledger.readPublicTokensServed().pipe(
      Effect.map(aggregate =>
        respond(
          aggregate.tokensServed,
          liveAtReadStaleness(['token_usage_events']),
        ),
      ),
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

  // Projection first (never throws; undefined ⇒ fail open to the D1 SUM).
  return Effect.promise(() =>
    readTokensServedProjectionCached({
      binding: input.KHALA_SYNC_DB,
      ...input.projectionReadDeps,
    }),
  ).pipe(
    Effect.flatMap(projection =>
      projection === undefined
        ? ledgerFallback()
        : Effect.succeed(
            respond(
              projection.tokensServed,
              rebuiltOnTransitionStaleness(
                TOKENS_SERVED_PROJECTION_MAX_STALENESS_SECONDS,
                ['token_usage_events'],
              ),
            ),
          ),
    ),
  )
}
