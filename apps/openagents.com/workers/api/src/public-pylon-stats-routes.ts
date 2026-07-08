import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { makeD1NexusTreasuryPayoutLedgerStore } from './nexus-treasury-payout-ledger'
import {
  makeD1Nip90MarketReceiptStore,
  type Nip90MarketReceiptStore,
} from './nip90-market-receipts'
import {
  type PublicPylonSettlementReceiptStore,
  type PublicPylonStatsStore,
  type PublicTrainingContributorStatsStore,
  type PublicTreasuryPayoutStatsStore,
  publicPylonStatsSnapshot,
} from './public-pylon-stats'
import {
  makePylonApiStoreForEnv,
  type MakePylonApiStoreForEnvOptions,
  type PylonDispatchStoreEnv,
} from './pylon-dispatch-store'
import { makeD1TrainingAuthorityStore } from './training-run-window-authority'
import { makeD1TreasuryTransactionStore } from './treasury-page-routes'
import { currentEpochMillis } from './runtime-primitives'

type PublicPylonStatsRouteInput = Partial<PylonDispatchStoreEnv> &
  Readonly<{
    marketReceiptStore?: Nip90MarketReceiptStore
    nowUnixMs?: () => number
    pylonStoreOptions?: MakePylonApiStoreForEnvOptions
    receiptStore?: PublicPylonSettlementReceiptStore
    store?: PublicPylonStatsStore
    trainingStore?: PublicTrainingContributorStatsStore
    treasuryPayoutStore?: PublicTreasuryPayoutStatsStore
  }>

const makePublicPylonStatsStoreForEnv = (
  input: PublicPylonStatsRouteInput,
): PublicPylonStatsStore =>
  makePylonApiStoreForEnv(
    input as PylonDispatchStoreEnv,
    input.pylonStoreOptions,
  )

// #5050 perf: the homepage polls this every few seconds and several visitors hit
// it at once; recomputing the full snapshot per request (D1 scans + receipt
// aggregation) made it ~5s. Cache the computed snapshot in-isolate for a few
// seconds so reads are instant and at most a few seconds stale (the "online now"
// window is minutes and the sats window is 24h, well within budget). The response
// stays `no-store` so each client poll gets the latest cached value, not a frozen
// browser copy.
const STATS_CACHE_TTL_MS = 4_000
let statsCache: { at: number; stats: unknown } | null = null

export const handlePublicPylonStatsApi = (
  request: Request,
  input: PublicPylonStatsRouteInput,
) => {
  if (request.method !== 'GET') return Effect.succeed(methodNotAllowed(['GET']))

  // Only cache the real production path. Tests inject in-memory stores or route
  // factory options and must each see their own snapshot, so they bypass it.
  const cacheable =
    input.store === undefined &&
    input.pylonStoreOptions === undefined &&
    input.receiptStore === undefined &&
    input.marketReceiptStore === undefined &&
    input.treasuryPayoutStore === undefined &&
    input.trainingStore === undefined

  const cached = cacheable ? statsCache : null
  if (
    cached !== null &&
    currentEpochMillis() - cached.at < STATS_CACHE_TTL_MS
  ) {
    return Effect.succeed(noStoreJsonResponse(cached.stats))
  }

  return Effect.map(
    publicPylonStatsSnapshot({
      marketReceiptStore:
        input.marketReceiptStore ??
        (input.OPENAGENTS_DB === undefined
          ? undefined
          : makeD1Nip90MarketReceiptStore(input.OPENAGENTS_DB)),
      nowUnixMs: input.nowUnixMs,
      receiptStore:
        input.receiptStore ??
        (input.OPENAGENTS_DB === undefined
          ? undefined
          : makeD1NexusTreasuryPayoutLedgerStore(input.OPENAGENTS_DB)),
      store: input.store ?? makePublicPylonStatsStoreForEnv(input),
      trainingStore:
        input.trainingStore ??
        (input.OPENAGENTS_DB === undefined
          ? undefined
          : makeD1TrainingAuthorityStore(input.OPENAGENTS_DB)),
      treasuryPayoutStore:
        input.treasuryPayoutStore ??
        (input.OPENAGENTS_DB === undefined
          ? undefined
          : makeD1TreasuryTransactionStore(input.OPENAGENTS_DB)),
    }),
    stats => {
      if (cacheable) statsCache = { at: currentEpochMillis(), stats }
      return noStoreJsonResponse(stats)
    },
  )
}
