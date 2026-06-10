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
  publicPylonStatsSnapshot,
} from './public-pylon-stats'
import { makeD1PylonApiStore } from './pylon-api'

type PublicPylonStatsRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  marketReceiptStore?: Nip90MarketReceiptStore
  nowUnixMs?: () => number
  receiptStore?: PublicPylonSettlementReceiptStore
  store?: PublicPylonStatsStore
}>

export const handlePublicPylonStatsApi = (
  request: Request,
  input: PublicPylonStatsRouteInput,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.map(
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
          store:
            input.store ??
            makeD1PylonApiStore(input.OPENAGENTS_DB as D1Database),
        }),
        stats => noStoreJsonResponse(stats),
      )
