// Monetize-any-layer RESALE RECEIPT — the inert seam that binds a per-layer
// offer + a real metered-spend event + the referral accrual outcome into ONE
// dereferenceable resale-with-referral receipt SHAPE (EPIC #5510, child #5518;
// promise marketplace.monetize_any_layer_with_referral.v1, planned).
//
// Episode 239 ("Let's Make Money", docs/transcripts/239.md): anyone (or their
// agents) can monetize / sell access to ANY layer and earn a referral cut on it.
// For that to be REAL there must be ONE receipt a reviewer (or a future armed
// run) can dereference end-to-end: the OFFER it ran against, the SPEND event it
// metered, and the REFERRAL outcome that spend produced (which ledger row, in
// which state, for how much). The two halves of the scaffold each hold one half
// of that story and never reconcile into a single artifact:
//
//   - marketplace-monetize-any-layer.ts builds the OFFER + the pure accrual PLAN;
//   - marketplace-monetize-any-layer-accrual.ts feeds an authorized plan into the
//     ONE cross-category referral ledger and returns a tagged accrual RESULT.
//
// Nothing projects those into a single dereferenceable resale receipt, so the
// blocker `monetize_any_layer_resale_receipt_missing` has no artifact behind it.
// This module is that projection: it reconciles the offer against the result's
// plan, derives the deterministic receipt + spend refs, and surfaces the ledger
// row's payout/event refs + state when (and only when) one was recorded.
//
// SCOPE / HONESTY: this is PURE and INERT. It moves no money, settles no charge,
// writes no row, and reads no balance — it only assembles + verifies the receipt
// SHAPE from an already-inert accrual result. The receipt is honestly marked
// `settled: false`, `inert: true`: it is the receipt's SHAPE over an inert
// accrual (eligibility at most), NOT a settled receipt. It therefore ADVANCES
// but does NOT clear the resale-receipt blocker, and the promise STAYS planned —
// nothing here flips it green and the no-resale invariant is never waived for
// subscription accounts. A green flip stays receipt-first and owner-signed per
// proof.claim_upgrade_receipts.v1.

import type { AccrueMonetizeLayerReferralResult } from './marketplace-monetize-any-layer-accrual'
import { monetizeLayerSpendMsatToQualifyingSats } from './marketplace-monetize-any-layer-accrual'
import {
  MARKETPLACE_MONETIZE_ANY_LAYER_PROMISE,
  type LayerMonetizationDefinition,
} from './marketplace-monetize-any-layer'
import type { SiteReferralPayoutState } from './site-referral-payout-ledger'

export const MARKETPLACE_MONETIZE_ANY_LAYER_RECEIPT_SCHEMA =
  'openagents.marketplace_monetize_any_layer_receipt.v1' as const

// The blocker this receipt SHAPE advances and does NOT clear: a reconciled,
// dereferenceable receipt shape over an INERT accrual is not a settled
// resale-with-referral receipt.
export const MONETIZE_LAYER_RESALE_RECEIPT_MISSING_REF =
  'blocker.product_promises.monetize_any_layer_resale_receipt_missing' as const

export const MONETIZE_LAYER_RECEIPT_EVENT_MISMATCH_REF =
  'blocker.monetize_any_layer.receipt_event_mismatch' as const

/**
 * The referral outcome a resale event produced, projected from the accrual
 * result. INERT in every arm: `recorded` carries an ELIGIBILITY ledger row, not
 * a settled payout. Public-safe — refs + sat amounts only, never a destination,
 * idempotency key, or payment material.
 */
export type MonetizeLayerReferralOutcome =
  // The accrual flag was off: the plan computed, the ledger was untouched.
  | Readonly<{ _tag: 'disabled' }>
  // A no-resale / boundary / self-referral guard blocked the plan: no ledger row.
  | Readonly<{ _tag: 'unauthorized'; blockerRefs: ReadonlyArray<string> }>
  // The plan was authorized but the principal had no permanent attribution.
  | Readonly<{ _tag: 'no_attribution' }>
  // The principal's attributed referrer is the principal: short-circuited.
  | Readonly<{ _tag: 'self_attribution' }>
  // 5% of the qualifying spend rounded below 1 sat: nothing to accrue.
  | Readonly<{ _tag: 'zero_referrer_share' }>
  // The RL-3 asset boundary refused the revshare.
  | Readonly<{ _tag: 'boundary_refused'; reasonRef: string }>
  // The ledger rejected the (bounded) category/event id.
  | Readonly<{ _tag: 'invalid_input'; reason: string }>
  // An eligibility row was recorded: carries the dereferenceable ledger refs +
  // state + the accrued referral cut in sats. NOT settled.
  | Readonly<{
      _tag: 'recorded'
      payoutRef: string
      qualifyingEventRef: string
      ledgerState: SiteReferralPayoutState
      referralAccrualSats: number
    }>

/**
 * The ONE dereferenceable monetize-any-layer resale receipt SHAPE: a stable
 * receipt ref, the offer it ran against, the metered spend event, the referral
 * outcome that spend produced, the no-resale guard posture, and an HONEST
 * unsettled posture. INERT: `settled` is ALWAYS false here — this is the
 * receipt's shape over an inert accrual, not a settled receipt.
 */
export type MonetizeLayerResaleReceipt = Readonly<{
  schema: typeof MARKETPLACE_MONETIZE_ANY_LAYER_RECEIPT_SCHEMA
  promiseId: typeof MARKETPLACE_MONETIZE_ANY_LAYER_PROMISE
  promiseState: 'planned'
  /** Always true — the receipt is a shape over an inert accrual. */
  inert: true
  /** Deterministic, dereferenceable receipt ref for this resale event. */
  receiptRef: string
  /** The per-layer offer the spend ran against. */
  offerId: string
  layer: string
  monetizationKind: LayerMonetizationDefinition['monetizationKind']
  sellerRef: string
  /** The metered spend event the referral cut was computed over. */
  eventId: string
  meteredSpendMsat: number
  /** The qualifying spend fed to the sat-denominated ledger (msat -> whole sats). */
  qualifyingAmountSats: number
  /** Whether the no-resale / boundary / self-referral guards permitted the plan. */
  authorized: boolean
  /** The referral outcome that spend produced. */
  referralOutcome: MonetizeLayerReferralOutcome
  /** Always false — INERT: no revshare has settled against the ledger. */
  settled: false
  /** The blocker this receipt shape advances and does NOT clear. */
  unclearedBlockerRefs: ReadonlyArray<string>
}>

export class MonetizeLayerResaleReceiptError extends Error {
  readonly _tag = 'MonetizeLayerResaleReceiptError'
  constructor(readonly reason: string) {
    super(reason)
  }
}

const isNonEmpty = (value: string): boolean => value.trim().length > 0

// Receipt + spend refs are namespaced by layer + event so a monetize-any-layer
// receipt for one event never collides with another layer's receipt for the
// same id. Bounded, deterministic shapes — the same vocabulary the accrual seam
// uses to namespace its ledger refs.
export const monetizeLayerResaleReceiptRef = (
  layer: string,
  eventId: string,
): string => `receipt.marketplace.monetize_any_layer.${layer}.${eventId}`

const referralOutcomeFromResult = (
  result: AccrueMonetizeLayerReferralResult,
): MonetizeLayerReferralOutcome => {
  switch (result._tag) {
    case 'disabled':
      return { _tag: 'disabled' }
    case 'unauthorized':
      return {
        _tag: 'unauthorized',
        blockerRefs: result.plan.blockerRefs,
      }
    case 'accrued': {
      const accrual = result.accrual
      switch (accrual._tag) {
        case 'recorded':
          return {
            _tag: 'recorded',
            payoutRef: accrual.entry.payoutRef,
            qualifyingEventRef: accrual.entry.qualifyingEventRef,
            ledgerState: accrual.entry.state,
            referralAccrualSats: accrual.entry.amountSats,
          }
        case 'invalid_input':
          return { _tag: 'invalid_input', reason: accrual.reason }
        case 'boundary_refused':
          return { _tag: 'boundary_refused', reasonRef: accrual.reasonRef }
        case 'no_attribution':
          return { _tag: 'no_attribution' }
        case 'self_attribution':
          return { _tag: 'self_attribution' }
        case 'zero_referrer_share':
          return { _tag: 'zero_referrer_share' }
      }
    }
  }
}

/**
 * Build + verify the ONE monetize-any-layer resale receipt from a per-layer
 * offer, the spend event id, and the accrual RESULT that event produced. PURE
 * and validating. Reconciles:
 *   - the eventId is a non-empty bound;
 *   - the accrual result's plan describes the SAME offer (layer + sellerRef)
 *     the receipt is being cut for (no silent cross-offer binding);
 *   - the qualifying sats reported equal the plan's metered spend converted
 *     msat -> whole sats (the ledger's denomination).
 *
 * Returns the resale receipt (inert, honestly unsettled) on success. NEVER
 * settles a charge and NEVER writes a receipt row. `authorized` mirrors the
 * plan's guard posture; the receipt is cut for blocked plans too (carrying an
 * `unauthorized`/`disabled` outcome), because a refused resale is itself an
 * auditable, dereferenceable event.
 */
export const buildMonetizeLayerResaleReceipt = (input: {
  definition: LayerMonetizationDefinition
  eventId: string
  result: AccrueMonetizeLayerReferralResult
}):
  | { ok: true; receipt: MonetizeLayerResaleReceipt }
  | { ok: false; error: MonetizeLayerResaleReceiptError } => {
  const { definition, eventId, result } = input
  const fail = (
    reason: string,
  ): { ok: false; error: MonetizeLayerResaleReceiptError } => ({
    ok: false,
    error: new MonetizeLayerResaleReceiptError(reason),
  })

  if (!isNonEmpty(eventId)) {
    return fail('eventId must be non-empty')
  }

  const plan = result.plan
  if (plan.layer !== definition.layer) {
    return fail(
      `accrual plan layer ${plan.layer} does not match offer layer ${definition.layer}`,
    )
  }
  if (plan.sellerRef !== definition.sellerRef) {
    return fail(
      `accrual plan sellerRef ${plan.sellerRef} does not match offer sellerRef ${definition.sellerRef}`,
    )
  }

  const qualifyingAmountSats = monetizeLayerSpendMsatToQualifyingSats(
    plan.meteredSpendMsat,
  )

  return {
    ok: true,
    receipt: {
      schema: MARKETPLACE_MONETIZE_ANY_LAYER_RECEIPT_SCHEMA,
      promiseId: MARKETPLACE_MONETIZE_ANY_LAYER_PROMISE,
      promiseState: 'planned',
      inert: true,
      receiptRef: monetizeLayerResaleReceiptRef(definition.layer, eventId),
      offerId: definition.offerId,
      layer: definition.layer,
      monetizationKind: definition.monetizationKind,
      sellerRef: definition.sellerRef,
      eventId,
      meteredSpendMsat: plan.meteredSpendMsat,
      qualifyingAmountSats,
      authorized: plan.authorized,
      referralOutcome: referralOutcomeFromResult(result),
      settled: false,
      unclearedBlockerRefs: [MONETIZE_LAYER_RESALE_RECEIPT_MISSING_REF],
    },
  }
}

/**
 * A public-safe projection of a resale receipt: the receipt ref, the offer
 * (offer/layer/kind/seller — no price), the spend event (refs + sats, no msat
 * destination), the referral outcome tag, and the inert/unsettled posture.
 * Suitable for a read-only surface. Honest: a reconciled receipt SHAPE over an
 * inert accrual is not a settled resale receipt.
 */
export type MonetizeLayerResaleReceiptProjection = Readonly<{
  schema: typeof MARKETPLACE_MONETIZE_ANY_LAYER_RECEIPT_SCHEMA
  promiseId: typeof MARKETPLACE_MONETIZE_ANY_LAYER_PROMISE
  promiseState: 'planned'
  inert: true
  receiptRef: string
  offerId: string
  layer: string
  monetizationKind: LayerMonetizationDefinition['monetizationKind']
  sellerRef: string
  eventId: string
  qualifyingAmountSats: number
  authorized: boolean
  referralOutcomeTag: MonetizeLayerReferralOutcome['_tag']
  settled: false
  unclearedBlockerRefs: ReadonlyArray<string>
}>

export const monetizeLayerResaleReceiptProjection = (
  receipt: MonetizeLayerResaleReceipt,
): MonetizeLayerResaleReceiptProjection => ({
  schema: receipt.schema,
  promiseId: receipt.promiseId,
  promiseState: receipt.promiseState,
  inert: receipt.inert,
  receiptRef: receipt.receiptRef,
  offerId: receipt.offerId,
  layer: receipt.layer,
  monetizationKind: receipt.monetizationKind,
  sellerRef: receipt.sellerRef,
  eventId: receipt.eventId,
  qualifyingAmountSats: receipt.qualifyingAmountSats,
  authorized: receipt.authorized,
  referralOutcomeTag: receipt.referralOutcome._tag,
  settled: receipt.settled,
  unclearedBlockerRefs: receipt.unclearedBlockerRefs,
})
