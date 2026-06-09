import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { makeD1NexusTreasuryPayoutLedgerStore } from './nexus-treasury-payout-ledger'
import {
  type PublicPylonSettlementReceiptStore,
  type PublicPylonStatsStore,
  publicPylonStatsSnapshot,
} from './public-pylon-stats'
import { makeD1PylonApiStore } from './pylon-api'

type PublicPylonStatsRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
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
