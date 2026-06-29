import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import {
  PARTNER_PAYOUT_POLICY_REF,
  PARTNER_PAYOUT_ROLE_POLICY,
  type PartnerPayoutAsset,
  type PartnerPayoutRole,
  type PartnerPayoutState,
} from './partner-payout-ledger'

/**
 * Public partner-payout ledger projection.
 *
 * This is intentionally narrower than the operator ledger projection. It proves
 * the partner payout ledger has a dereferenceable public-safe API without
 * exposing partner refs, user ids, payout refs, qualifying event refs, payout
 * destinations, invoices, preimages, or provider payloads.
 */

export const PARTNER_PAYOUT_PUBLIC_PROJECTION_SCHEMA_VERSION =
  'openagents.partner_payouts.v1'

export const PARTNER_PAYOUT_PUBLIC_PROJECTION_AUTHORITY_BOUNDARY =
  'A public count of partner payout ledger state grants no partner attribution, ' +
  'eligibility, payout, settlement, withdrawal, revenue, or spend authority. ' +
  'Ledger state is not spendable value; settlement requires separate operator ' +
  'dispatch authority and public-safe settlement evidence refs.'

const PARTNER_PAYOUT_PUBLIC_CAVEAT_REFS = [
  'caveat.public.partner_payouts.counts_only_no_partner_or_user_identifiers',
  'caveat.public.partner_payouts.ledger_state_not_spendable_value',
  'caveat.public.partner_payouts.settlement_evidence_required',
  'caveat.public.partner_payouts.partner_policy_not_owner_signed',
] as const

const PARTNER_PAYOUT_PUBLIC_BLOCKER_REFS = [
  'blocker.product_promises.partner_first_real_payout_pending',
] as const

const ALL_STATES: ReadonlyArray<PartnerPayoutState> = [
  'eligible',
  'approved',
  'dispatched',
  'settled',
  'failed',
  'refused',
  'reversed',
]

const ALL_ROLES: ReadonlyArray<PartnerPayoutRole> = [
  'design_partner',
  'referral',
  'affiliate',
]

const ALL_ASSETS: ReadonlyArray<PartnerPayoutAsset> = [
  'usd',
  'credits',
  'sats',
]

export type PartnerPayoutPublicCurrentState = Readonly<{
  amount: number
  asset: PartnerPayoutAsset
  partnerRole: PartnerPayoutRole
  state: PartnerPayoutState
}>

export type PartnerPayoutPublicStateTotal = Readonly<{
  count: number
  state: PartnerPayoutState
  totalAmount: number
}>

export type PartnerPayoutPublicRoleTotal = Readonly<{
  count: number
  partnerRole: PartnerPayoutRole
  totalAmount: number
}>

export type PartnerPayoutPublicAssetTotal = Readonly<{
  asset: PartnerPayoutAsset
  count: number
  settledAmount: number
  totalAmount: number
}>

export type PartnerPayoutPublicProjection = Readonly<{
  authorityBoundary: string
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  kind: 'partner_payouts_public'
  ledgerWiredInSource: boolean
  operatorRoutesWiredInSource: boolean
  partnerProjectionApiWiredInSource: boolean
  policy: Readonly<{
    policyRef: string
    rolePolicies: ReadonlyArray<
      Readonly<{
        maxEventAmount: number
        maxPartnerPeriodAmount: number
        maxPartnerPeriodCount: number
        partnerRole: PartnerPayoutRole
        percentBps: number
      }>
    >
  }>
  publicSafe: boolean
  schemaVersion: string
  assetTotals: ReadonlyArray<PartnerPayoutPublicAssetTotal>
  roleTotals: ReadonlyArray<PartnerPayoutPublicRoleTotal>
  settledCount: number
  settledSats: number
  stateTotals: ReadonlyArray<PartnerPayoutPublicStateTotal>
  staleness: PublicProjectionStalenessContract
  totalCurrentPayouts: number
}>

export type PartnerPayoutsPublicProjection = PartnerPayoutPublicProjection &
  Readonly<{
    generatedAt: string
    staleness: PublicProjectionStalenessContract
  }>

export const partnerPayoutPublicStaleness =
  (): PublicProjectionStalenessContract =>
    liveAtReadStaleness([
      'partner_payout_eligibility_recorded',
      'partner_payout_state_transition_recorded',
    ])

const incrementTotals = <Key extends string>(
  totals: Map<Key, Readonly<{ count: number; totalAmount: number }>>,
  key: Key,
  amount: number,
) => {
  const existing = totals.get(key) ?? { count: 0, totalAmount: 0 }
  totals.set(key, {
    count: existing.count + 1,
    totalAmount: existing.totalAmount + amount,
  })
}

export const aggregatePartnerPayoutPublicProjection = (
  currentStates: ReadonlyArray<PartnerPayoutPublicCurrentState>,
): PartnerPayoutPublicProjection => {
  const byState = new Map<
    PartnerPayoutState,
    Readonly<{ count: number; totalAmount: number }>
  >()
  const byRole = new Map<
    PartnerPayoutRole,
    Readonly<{ count: number; totalAmount: number }>
  >()
  const byAsset = new Map<
    PartnerPayoutAsset,
    Readonly<{ count: number; settledAmount: number; totalAmount: number }>
  >()

  for (const state of ALL_STATES) {
    byState.set(state, { count: 0, totalAmount: 0 })
  }

  for (const role of ALL_ROLES) {
    byRole.set(role, { count: 0, totalAmount: 0 })
  }

  for (const asset of ALL_ASSETS) {
    byAsset.set(asset, { count: 0, settledAmount: 0, totalAmount: 0 })
  }

  for (const current of currentStates) {
    incrementTotals(byState, current.state, current.amount)
    incrementTotals(byRole, current.partnerRole, current.amount)

    const existing = byAsset.get(current.asset) ?? {
      count: 0,
      settledAmount: 0,
      totalAmount: 0,
    }
    byAsset.set(current.asset, {
      count: existing.count + 1,
      settledAmount:
        existing.settledAmount +
        (current.state === 'settled' ? current.amount : 0),
      totalAmount: existing.totalAmount + current.amount,
    })
  }

  const stateTotals = ALL_STATES.map(state => {
    const totals = byState.get(state) ?? { count: 0, totalAmount: 0 }

    return { count: totals.count, state, totalAmount: totals.totalAmount }
  })
  const roleTotals = ALL_ROLES.map(partnerRole => {
    const totals = byRole.get(partnerRole) ?? { count: 0, totalAmount: 0 }

    return {
      count: totals.count,
      partnerRole,
      totalAmount: totals.totalAmount,
    }
  })
  const assetTotals = ALL_ASSETS.map(asset => {
    const totals = byAsset.get(asset) ?? {
      count: 0,
      settledAmount: 0,
      totalAmount: 0,
    }

    return {
      asset,
      count: totals.count,
      settledAmount: totals.settledAmount,
      totalAmount: totals.totalAmount,
    }
  })
  const settled = byState.get('settled') ?? { count: 0, totalAmount: 0 }
  const settledSats =
    assetTotals.find(total => total.asset === 'sats')?.settledAmount ?? 0

  return {
    assetTotals,
    authorityBoundary: PARTNER_PAYOUT_PUBLIC_PROJECTION_AUTHORITY_BOUNDARY,
    blockerRefs: PARTNER_PAYOUT_PUBLIC_BLOCKER_REFS,
    caveatRefs: PARTNER_PAYOUT_PUBLIC_CAVEAT_REFS,
    kind: 'partner_payouts_public',
    ledgerWiredInSource: true,
    operatorRoutesWiredInSource: true,
    partnerProjectionApiWiredInSource: true,
    policy: {
      policyRef: PARTNER_PAYOUT_POLICY_REF,
      rolePolicies: ALL_ROLES.map(partnerRole => ({
        partnerRole,
        ...PARTNER_PAYOUT_ROLE_POLICY[partnerRole],
      })),
    },
    publicSafe: true,
    roleTotals,
    schemaVersion: PARTNER_PAYOUT_PUBLIC_PROJECTION_SCHEMA_VERSION,
    settledCount: settled.count,
    settledSats,
    stateTotals,
    staleness: partnerPayoutPublicStaleness(),
    totalCurrentPayouts: currentStates.length,
  }
}
