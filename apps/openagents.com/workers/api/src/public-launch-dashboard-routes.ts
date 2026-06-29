import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { makeD1NexusTreasuryPayoutLedgerStore } from './nexus-treasury-payout-ledger'
import {
  PublicLaunchDashboardUnsafe,
  projectPublicLaunchDashboard,
} from './public-launch-dashboard'
import {
  type PublicPylonSettlementReceiptStore,
  type PublicPylonStatsStore,
  publicPylonStatsSnapshot,
} from './public-pylon-stats'
import { makeD1PylonApiStore } from './pylon-api'
import {
  currentEpochMillis,
  epochMillisToIsoTimestamp,
} from './runtime-primitives'

type PublicLaunchDashboardRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  nowUnixMs?: () => number
  receiptStore?: PublicPylonSettlementReceiptStore
  store?: PublicPylonStatsStore
}>

const routeErrorResponse = (error: PublicLaunchDashboardUnsafe) =>
  noStoreJsonResponse(
    {
      error: 'public_launch_dashboard_unsafe',
      reason: error.reason,
    },
    { status: 500 },
  )

export const handlePublicLaunchDashboardApi = (
  request: Request,
  input: PublicLaunchDashboardRouteInput,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : publicPylonStatsSnapshot({
        nowUnixMs: input.nowUnixMs,
        receiptStore:
          input.receiptStore ??
          (input.OPENAGENTS_DB === undefined
            ? undefined
            : makeD1NexusTreasuryPayoutLedgerStore(input.OPENAGENTS_DB)),
        store:
          input.store ?? makeD1PylonApiStore(input.OPENAGENTS_DB as D1Database),
      }).pipe(
        Effect.flatMap(pylonStats =>
          Effect.try({
            try: () => {
              const nowUnixMs = input.nowUnixMs?.() ?? currentEpochMillis()
              return noStoreJsonResponse(
                projectPublicLaunchDashboard({
                  generatedAt: epochMillisToIsoTimestamp(nowUnixMs),
                  nowUnixMs,
                  pylonStats,
                }),
              )
            },
            catch: error =>
              error instanceof PublicLaunchDashboardUnsafe
                ? error
                : new PublicLaunchDashboardUnsafe({
                    reason: 'Public launch dashboard projection failed.',
                  }),
          }),
        ),
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
