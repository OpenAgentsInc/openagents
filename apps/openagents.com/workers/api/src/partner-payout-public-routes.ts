import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  type PartnerPayoutAsset,
  type PartnerPayoutRole,
  type PartnerPayoutState,
} from './partner-payout-ledger'
import {
  type PartnerPayoutPublicCurrentState,
  type PartnerPayoutsPublicProjection,
  aggregatePartnerPayoutPublicProjection,
} from './partner-payout-public-projection'
import { currentIsoTimestamp } from './runtime-primitives'

/**
 * Public partner-payout projection route:
 *
 *   GET /api/public/partner-payouts
 *
 * Read-only, public-safe count projection over the partner payout ledger. It
 * selects only aggregate-safe fields from the current ledger entries and never
 * exposes partner refs, user ids, payout refs, qualifying event refs, payout
 * destinations, invoices, preimages, or provider payloads.
 */

type PartnerPayoutPublicRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  nowIso?: () => string
  readCurrentStates?: () => Promise<
    ReadonlyArray<PartnerPayoutPublicCurrentState>
  >
}>

type CurrentStateRow = Readonly<{
  amount: number
  asset: PartnerPayoutAsset
  partner_role: PartnerPayoutRole
  state: PartnerPayoutState
}>

const readCurrentPartnerPayoutStates = async (
  db: D1Database,
): Promise<ReadonlyArray<PartnerPayoutPublicCurrentState>> => {
  const result = await db
    .prepare(
      `SELECT e.state AS state,
              e.amount AS amount,
              e.asset AS asset,
              e.partner_role AS partner_role
         FROM partner_payout_ledger_entries AS e
        WHERE e.archived_at IS NULL
          AND e.id = (
            SELECT inner_e.id
              FROM partner_payout_ledger_entries AS inner_e
             WHERE inner_e.payout_ref = e.payout_ref
               AND inner_e.archived_at IS NULL
             ORDER BY inner_e.created_at DESC, inner_e.id DESC
             LIMIT 1
          )`,
    )
    .all<CurrentStateRow>()

  return (result.results ?? []).map(row => ({
    amount: Number(row.amount),
    asset: row.asset,
    partnerRole: row.partner_role,
    state: row.state,
  }))
}

const safeReadCurrentPartnerPayoutStates = async (
  db: D1Database,
): Promise<ReadonlyArray<PartnerPayoutPublicCurrentState>> => {
  try {
    return await readCurrentPartnerPayoutStates(db)
  } catch {
    return []
  }
}

export const handlePartnerPayoutsPublicApi = (
  request: Request,
  input: PartnerPayoutPublicRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const nowIso = input.nowIso?.() ?? currentIsoTimestamp()
  const readCurrentStates =
    input.readCurrentStates ??
    (() => safeReadCurrentPartnerPayoutStates(input.OPENAGENTS_DB as D1Database))

  return Effect.promise(async () => {
    const currentStates = await readCurrentStates()
    const projection = aggregatePartnerPayoutPublicProjection(currentStates)
    const body: PartnerPayoutsPublicProjection = {
      ...projection,
      generatedAt: nowIso,
    }

    return noStoreJsonResponse(body)
  })
}
