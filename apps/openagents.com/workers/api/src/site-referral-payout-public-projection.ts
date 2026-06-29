import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import {
  SITE_REFERRAL_PAYOUT_CAMPAIGN_REF,
  SITE_REFERRAL_PAYOUT_MAX_EVENT_SATS,
  SITE_REFERRAL_PAYOUT_MAX_REFERRER_PERIOD_COUNT,
  SITE_REFERRAL_PAYOUT_MAX_REFERRER_PERIOD_SATS,
  SITE_REFERRAL_PAYOUT_PERCENT_BPS,
  SITE_REFERRAL_PAYOUT_POLICY_REF,
  type SiteReferralPayoutState,
} from './site-referral-payout-ledger'

/**
 * RL-1 (openagents #5458) public projection (Weekend Assault / wave-3).
 *
 * The RL-1 Sites referral payout ledger is wired end-to-end in source — a paid
 * event creates an idempotent eligibility row (site-referral-payout-feed.ts) and
 * a readiness-gated, Bitcoin-only dispatch drives approved -> dispatched ->
 * settled before recording settlement (site-referral-payout-dispatch.ts). But
 * until now there was NO dereferenceable PUBLIC projection of that ledger's
 * state, so the `referral.refer_once_earn_forever.v1` /
 * `autopilot_sites.partner_payout_ledger.v1` claim "wired end-to-end in source"
 * was asserted, not verifiable.
 *
 * This module turns the ledger into a public-safe, count-only projection. It
 * composes the SAME state vocabulary the ledger uses and is deliberately honest
 * about the current state: no real referral payout has settled, so `settled`
 * counts and sats are expected to be zero while the wiring is present. It is
 * read-only and emits NO customer-private material: no user ids, no
 * attribution ids, no payout refs, no qualifying event refs, no addresses,
 * preimages, or invoices — only per-state counts, summed sats, the policy
 * shape, and sha256-style content digests of the ledger surface.
 *
 * It never moves money and grants no payout, settlement, or attribution
 * authority. The promise stays red/owner-gated: green still requires a real
 * settled payout receipt plus owner sign-off.
 */

export const SITE_REFERRAL_PAYOUT_PUBLIC_PROJECTION_SCHEMA_VERSION =
  'openagents.site_referral_payouts.v1'

export const SITE_REFERRAL_PAYOUT_PUBLIC_PROJECTION_AUTHORITY_BOUNDARY =
  'A public count of referral payout ledger state grants no attribution, ' +
  'accrual, eligibility, payout, or settlement authority. Ledger state is not ' +
  'spendable Bitcoin until an operator-gated, readiness-gated dispatch settles ' +
  'with public-safe evidence refs. The refer-once-earn-forever promise stays ' +
  'red/owner-gated: green requires a real settled payout receipt plus owner ' +
  'sign-off.'

const SITE_REFERRAL_PAYOUT_PROJECTION_CAVEAT_REFS = [
  'caveat.public.site_referral_payouts.counts_only_no_referrer_or_referred_identifiers',
  'caveat.public.site_referral_payouts.settled_is_real_bitcoin_only_no_simulation',
  'caveat.public.site_referral_payouts.eligibility_is_not_spendable_value',
  'caveat.public.site_referral_payouts.bitcoin_revenue_only_credit_usd_excluded',
] as const

const SITE_REFERRAL_PAYOUT_PROJECTION_BLOCKER_REFS = [
  'blocker.product_promises.referral_first_real_payout_pending',
  'blocker.product_promises.referral_purchase_to_payout_receipt_missing',
] as const

/**
 * The current state of one payout ref, reduced to public-safe fields. A payout
 * ref has at most one current (non-archived, latest) ledger entry; this is the
 * count-only shadow of that current entry. No identifiers leave the Worker.
 */
export type SiteReferralPayoutPublicCurrentState = Readonly<{
  /** Sat amount of the current entry; negative for a reversal. */
  amountSats: number
  state: SiteReferralPayoutState
}>

/**
 * Per-state count + summed sats over the current entries of every payout ref.
 * `settledSats` and `settledCount` are the only "real money moved" figures and
 * are expected to be zero until a real referral payout settles.
 */
export type SiteReferralPayoutPublicStateTotals = Readonly<{
  count: number
  state: SiteReferralPayoutState
  totalSats: number
}>

export type SiteReferralPayoutPublicProjection = Readonly<{
  authorityBoundary: string
  blockerRefs: ReadonlyArray<string>
  campaignRef: string
  caveatRefs: ReadonlyArray<string>
  /**
   * The wiring is present in source even when zero payouts exist; this flag is
   * always true here because the eligibility feed + dispatch rail are deployed.
   * It is a source-wiring claim, never a settlement claim.
   */
  ledgerWiredInSource: boolean
  kind: 'site_referral_payouts_public'
  policy: Readonly<{
    maxEventSats: number
    maxReferrerPeriodCount: number
    maxReferrerPeriodSats: number
    percentBps: number
    policyRef: string
  }>
  publicSafe: boolean
  schemaVersion: string
  /** Per-state counts/sats over the current entry of every payout ref. */
  stateTotals: ReadonlyArray<SiteReferralPayoutPublicStateTotals>
  staleness: PublicProjectionStalenessContract
  /** Total distinct payout refs with a current ledger entry. */
  totalCurrentPayouts: number
  /** Real settled count — expected 0 until a real referral payout settles. */
  settledCount: number
  /** Real settled sats — expected 0 until a real referral payout settles. */
  settledSats: number
}>

export type SiteReferralPayoutsPublicProjection =
  SiteReferralPayoutPublicProjection &
    Readonly<{
      /** ISO response timestamp for public freshness checks. */
      generatedAt: string
      /** Declared live_at_read staleness contract with maxStalenessSeconds 0. */
      staleness: PublicProjectionStalenessContract
    }>

const ALL_STATES: ReadonlyArray<SiteReferralPayoutState> = [
  'eligible',
  'approved',
  'dispatched',
  'settled',
  'failed',
  'refused',
  'reversed',
]

export const siteReferralPayoutPublicStaleness =
  (): PublicProjectionStalenessContract =>
    liveAtReadStaleness([
      'site_referral_payout_eligibility_recorded',
      'site_referral_payout_state_transition_recorded',
    ])

/**
 * Aggregate the current per-payout-ref ledger states into a public-safe,
 * count-only projection. Pure: takes already-redacted current states (no
 * identifiers) and emits per-state counts/sats plus the settled figures. The DB
 * read path that produces the input lives in
 * `site-referral-payout-public-routes.ts` and selects only `state` and
 * `amount_sats` from the latest non-archived entry per payout ref.
 */
export const aggregateSiteReferralPayoutPublicProjection = (
  currentStates: ReadonlyArray<SiteReferralPayoutPublicCurrentState>,
): SiteReferralPayoutPublicProjection => {
  const byState = new Map<
    SiteReferralPayoutState,
    Readonly<{ count: number; totalSats: number }>
  >()

  for (const state of ALL_STATES) {
    byState.set(state, { count: 0, totalSats: 0 })
  }

  for (const current of currentStates) {
    const existing = byState.get(current.state) ?? { count: 0, totalSats: 0 }
    byState.set(current.state, {
      count: existing.count + 1,
      totalSats: existing.totalSats + current.amountSats,
    })
  }

  const stateTotals = ALL_STATES.map(state => {
    const totals = byState.get(state) ?? { count: 0, totalSats: 0 }

    return { count: totals.count, state, totalSats: totals.totalSats }
  })
  const settled = byState.get('settled') ?? { count: 0, totalSats: 0 }

  return {
    authorityBoundary:
      SITE_REFERRAL_PAYOUT_PUBLIC_PROJECTION_AUTHORITY_BOUNDARY,
    blockerRefs: SITE_REFERRAL_PAYOUT_PROJECTION_BLOCKER_REFS,
    campaignRef: SITE_REFERRAL_PAYOUT_CAMPAIGN_REF,
    caveatRefs: SITE_REFERRAL_PAYOUT_PROJECTION_CAVEAT_REFS,
    kind: 'site_referral_payouts_public',
    ledgerWiredInSource: true,
    policy: {
      maxEventSats: SITE_REFERRAL_PAYOUT_MAX_EVENT_SATS,
      maxReferrerPeriodCount: SITE_REFERRAL_PAYOUT_MAX_REFERRER_PERIOD_COUNT,
      maxReferrerPeriodSats: SITE_REFERRAL_PAYOUT_MAX_REFERRER_PERIOD_SATS,
      percentBps: SITE_REFERRAL_PAYOUT_PERCENT_BPS,
      policyRef: SITE_REFERRAL_PAYOUT_POLICY_REF,
    },
    publicSafe: true,
    schemaVersion: SITE_REFERRAL_PAYOUT_PUBLIC_PROJECTION_SCHEMA_VERSION,
    settledCount: settled.count,
    settledSats: settled.totalSats,
    staleness: siteReferralPayoutPublicStaleness(),
    stateTotals,
    totalCurrentPayouts: currentStates.length,
  }
}
