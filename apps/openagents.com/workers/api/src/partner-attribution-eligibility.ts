/**
 * Partner-attribution -> payout-eligibility bridge (#4986 / #5524 follow-up).
 *
 * Advances `blocker.product_promises.partner_attribution_policy_missing` by
 * connecting the two halves this lane already shipped:
 *
 *   decidePartnerAttribution()        -- WHICH partner, if any, is credited
 *     (partner-attribution-policy.ts)    for a qualifying paid event (pure).
 *   createPartnerPayoutEligibility()  -- record ONE eligibility row in the
 *     (partner-payout-ledger.ts)         operator-gated payout ledger.
 *
 * Before this module the two sides could not be called together: the policy
 * returns a decision, the ledger needs a `CreatePartnerPayoutEligibilityInput`,
 * and nothing mapped one onto the other. This bridge is that mapping, and it is
 * deliberately PURE — it takes the qualifying event plus the candidate partner
 * agreements the (still-deferred) storage reader will load, runs the policy, and
 * returns EITHER a ledger-ready eligibility input or a skip reason. It never
 * reads a database and never moves money, so the product rules remain
 * independently testable while owner sign-off is pending.
 *
 * Distinct from the referral feed by construction: there is NO inferred earner.
 * A `no_active_agreement` or `self_attribution` decision records NOTHING (the
 * `skip` branch), exactly mirroring the referral feed's `no_attribution` /
 * `self_attribution` short-circuits but WITHOUT last-touch inference. The
 * remaining storage-backed reader (loading `PartnerAgreement`s from a partner
 * agreements table and calling `createPartnerPayoutEligibility` with the input
 * this returns) is the documented next step in the promise note.
 */
import {
  type PartnerAttributionEvent,
  type PartnerAgreement,
  decidePartnerAttribution,
} from './partner-attribution-policy'
import {
  type CreatePartnerPayoutEligibilityInput,
  type PartnerPayoutAsset,
} from './partner-payout-ledger'

/**
 * A qualifying paid customer event, as the deferred feed would assemble it from
 * a real payment webhook. It carries both the attribution inputs (the paying
 * customer and the event time, which decides the active agreement window) and
 * the ledger inputs (amount, asset, period, public-safe refs, idempotency key).
 *
 * `idempotencyKey` MUST be deterministic per paid event so a webhook delivered
 * twice creates the eligibility row at most once (the ledger dedupes on it).
 * `eventIso` is used both as the attribution-window check time and as the
 * ledger entry's `createdAt`/`nowIso`: eligibility accrues as of the event.
 */
export type PartnerQualifyingPaidEvent = Readonly<{
  asset: PartnerPayoutAsset
  /** The paying customer's user id (also recorded as the ledger beneficiary). */
  customerUserId: string
  /** ISO timestamp of the qualifying paid event. */
  eventIso: string
  idempotencyKey: string
  periodKey: string
  /** Asset-minor-unit amount the role percentage is applied to. */
  qualifyingAmount: number
  qualifyingEventKind: string
  qualifyingEventRef: string
}>

/**
 * The result of resolving a qualifying event against candidate agreements.
 *
 * - `eligible`            -- an explicit active agreement won; `input` is ready
 *                           to hand to `createPartnerPayoutEligibility`.
 * - `no_active_agreement` -- no covering agreement (the common case); record
 *                           nothing. There is no inferred fallback.
 * - `self_attribution`    -- the only winner is the paying customer; record
 *                           nothing (the ledger would refuse it anyway).
 */
export type PartnerPayoutEligibilityResolution =
  | Readonly<{ _tag: 'no_active_agreement' }>
  | Readonly<{ _tag: 'self_attribution'; partnerRef: string }>
  | Readonly<{
      _tag: 'eligible'
      agreementRef: string
      input: CreatePartnerPayoutEligibilityInput
      policyRef: string
    }>

/**
 * Resolve a qualifying paid event into a ledger-ready eligibility input, or a
 * skip reason. Pure: runs `decidePartnerAttribution` over the supplied
 * candidate agreements and maps an `attributed` decision onto
 * `CreatePartnerPayoutEligibilityInput`.
 *
 * The paying customer is carried through as `beneficiaryUserId`, giving the
 * ledger a defense-in-depth self-payout guard on top of the policy's own
 * self-attribution exclusion. The decision's `agreementRef`/`policyRef` are
 * surfaced on the `eligible` result so the caller can record the explicit
 * attribution basis alongside the ledger row.
 */
export const resolvePartnerPayoutEligibilityInput = (
  event: PartnerQualifyingPaidEvent,
  candidateAgreements: ReadonlyArray<PartnerAgreement>,
): PartnerPayoutEligibilityResolution => {
  const attributionEvent: PartnerAttributionEvent = {
    customerUserId: event.customerUserId,
    eventIso: event.eventIso,
  }

  const decision = decidePartnerAttribution(
    attributionEvent,
    candidateAgreements,
  )

  if (decision._tag === 'no_active_agreement') {
    return { _tag: 'no_active_agreement' }
  }

  if (decision._tag === 'self_attribution') {
    return { _tag: 'self_attribution', partnerRef: decision.partnerRef }
  }

  const input: CreatePartnerPayoutEligibilityInput = {
    asset: event.asset,
    beneficiaryUserId: event.customerUserId,
    // Persist the explicit attribution basis on the ledger row: the winning
    // agreement ref as evidence, the attribution policy ref alongside the payout
    // policy ref. This is what makes the partner rail auditable AND distinct
    // from the referral rail — every credited partner payout names the explicit
    // agreement that authorised it, never an inferred click.
    evidenceRefs: [decision.agreementRef],
    idempotencyKey: event.idempotencyKey,
    nowIso: event.eventIso,
    partnerRef: decision.partnerRef,
    partnerRole: decision.partnerRole,
    partnerUserId: decision.partnerUserId,
    periodKey: event.periodKey,
    policyRefs: [decision.policyRef],
    qualifyingAmount: event.qualifyingAmount,
    qualifyingEventKind: event.qualifyingEventKind,
    qualifyingEventRef: event.qualifyingEventRef,
  }

  return {
    _tag: 'eligible',
    agreementRef: decision.agreementRef,
    input,
    policyRef: decision.policyRef,
  }
}
