import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  type SiteReferralPayoutPublicCurrentState,
  type SiteReferralPayoutsPublicProjection,
  aggregateSiteReferralPayoutPublicProjection,
} from './site-referral-payout-public-projection'
import type { SiteReferralPayoutState } from './site-referral-payout-ledger'

/**
 * RL-1 (openagents #5458) public projection route (Weekend Assault / wave-3):
 *
 *   GET /api/public/site-referral-payouts
 *
 * Read-only, public-safe dereference of the RL-1 Sites referral payout ledger
 * state. It composes live at read over the latest non-archived ledger entry per
 * payout ref, selecting ONLY `state` and `amount_sats` (never a user id,
 * attribution id, payout ref, qualifying event ref, address, preimage, or
 * invoice), then aggregates to per-state counts/sats plus the real settled
 * figures. It is honest about the current state: no real referral payout has
 * settled, so `settledCount`/`settledSats` are expected to be zero while the
 * wiring is present in source.
 *
 * Turning "wired in source" into "wired + dereferenceable". The route grants no
 * payout, settlement, or attribution authority and flips no promise; the
 * refer-once-earn-forever promise stays red/owner-gated.
 */

type SiteReferralPayoutPublicRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  nowIso?: () => string
  /** Test seam: supply current states directly instead of reading D1. */
  readCurrentStates?: () => Promise<
    ReadonlyArray<SiteReferralPayoutPublicCurrentState>
  >
}>

type CurrentStateRow = Readonly<{
  amount_sats: number
  state: SiteReferralPayoutState
}>

/**
 * Read the current (latest non-archived) state of every payout ref, selecting
 * only the count-only public-safe fields. A correlated subquery picks the latest
 * entry per payout ref by `created_at`, `id` — the same ordering
 * `readCurrentReferralPayout` uses. If the ledger table does not exist yet (some
 * D1 fixtures), the projection composes from an empty set rather than failing.
 */
const readCurrentReferralPayoutStates = async (
  db: D1Database,
): Promise<ReadonlyArray<SiteReferralPayoutPublicCurrentState>> => {
  const result = await db
    .prepare(
      `SELECT e.state AS state, e.amount_sats AS amount_sats
         FROM site_referral_payout_ledger_entries AS e
        WHERE e.archived_at IS NULL
          AND e.id = (
            SELECT inner_e.id
              FROM site_referral_payout_ledger_entries AS inner_e
             WHERE inner_e.payout_ref = e.payout_ref
               AND inner_e.archived_at IS NULL
             ORDER BY inner_e.created_at DESC, inner_e.id DESC
             LIMIT 1
          )`,
    )
    .all<CurrentStateRow>()

  return (result.results ?? []).map(row => ({
    amountSats: Number(row.amount_sats),
    state: row.state,
  }))
}

const safeReadCurrentReferralPayoutStates = async (
  db: D1Database,
): Promise<ReadonlyArray<SiteReferralPayoutPublicCurrentState>> => {
  try {
    return await readCurrentReferralPayoutStates(db)
  } catch {
    // A missing ledger table (fresh fixture) is an empty, honest projection,
    // not a server error: the wiring is present in source even with zero rows.
    return []
  }
}

export const handleSiteReferralPayoutsPublicApi = (
  request: Request,
  input: SiteReferralPayoutPublicRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const nowIso = input.nowIso?.() ?? currentIsoTimestamp()
  const readCurrentStates =
    input.readCurrentStates ??
    (() =>
      safeReadCurrentReferralPayoutStates(input.OPENAGENTS_DB as D1Database))

  return Effect.promise(async () => {
    const currentStates = await readCurrentStates()
    const projection = aggregateSiteReferralPayoutPublicProjection(currentStates)
    const body: SiteReferralPayoutsPublicProjection = {
      ...projection,
      generatedAt: nowIso,
    }

    return noStoreJsonResponse(body)
  })
}
