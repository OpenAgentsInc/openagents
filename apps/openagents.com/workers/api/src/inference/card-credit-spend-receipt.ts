// Card -> credit -> inference-spend chain receipt
// (blocker.product_promises.inference_card_credit_inference_spend_receipt_missing).
//
// THE GAP this closes: the paid-credits loop already emits a receipt ref at EACH
// of its three hops, but nothing LINKS them, so there is no single
// dereferenceable artifact proving "this card dollar became this credit became
// this inference spend." The three hops and their existing refs are:
//
//   1. card -> USD credit   (Stripe checkout fulfillment, `billing.ts`):
//        USD ledger row idempotency key  `billing:stripe-checkout:<sessionId>`
//        public evidence ref             `evidence.stripe_checkout_paid.<sessionId>`
//   2. USD credit -> msat   (the USD->msat bridge, `usd-credit-bridge.ts`):
//        grant receipt ref               `receipt.inference.usd_credit_grant.<grantRef>`
//   3. msat -> inference     (the metering hook, `metering-hook.ts`):
//        charge receipt ref              `receipt.inference.charge.<requestId>`
//
// This module is the SINGLE pure linker over those three refs. It assembles one
// dereferenceable end-to-end receipt keyed by the originating Stripe session,
// asserts the value-conservation invariants that make the chain honest
// (granted USD never exceeds purchased USD; the msat grant matches the shared
// USD->msat conversion; the metered spend never exceeds the funded grant), and
// returns a public-safe receipt with NO payment material — only refs, token
// counts, and amounts the authenticated owner may already see on each leg.
//
// PURE: no D1, no clock, no network, no secrets. It does not move money; it
// describes a movement the three real ledger writes already made. When the prod
// paid loop runs end to end this produces the dereferenceable
// card->credit->inference-spend receipt the promise's verification gate needs;
// until then it is exercised by fixtures/tests so the format is review-stable.

import { usdCentsToMsatFloor } from './usd-msat-conversion'
import { usdCreditGrantReceiptRef } from './usd-credit-bridge'
import { inferenceChargeReceiptRef } from './metering-hook'

// Public-safe ledger idempotency key for the card->USD-credit purchase leg.
// Mirrors the key `applyStripeCheckoutCredit` writes (`billing.ts`) so the
// receipt references the SAME row the Stripe fulfillment created.
export const cardCreditPurchaseLedgerKey = (sessionId: string): string =>
  `billing:stripe-checkout:${sessionId}`

// Public-safe evidence ref for the card->USD-credit purchase leg. Mirrors the
// ref `fulfillCheckoutSession` records into the referral payout feed
// (`stripe-billing.ts`) so a reviewer can resolve the same paid event.
export const cardCreditPurchaseEvidenceRef = (sessionId: string): string =>
  `evidence.stripe_checkout_paid.${sessionId}`

// Public-safe, dereferenceable ref for the assembled end-to-end chain receipt,
// keyed by the originating Stripe checkout session (the chain's genesis).
export const cardCreditSpendReceiptRef = (sessionId: string): string =>
  `receipt.inference.card_credit_spend.${sessionId}`

// The card -> USD credit purchase leg (hop 1): a fulfilled Stripe checkout that
// raised the USD `billing_ledger_entries` balance.
export type CardCreditPurchaseLeg = Readonly<{
  // Stripe checkout session id (the chain genesis).
  sessionId: string
  // USD cents granted into the USD ledger by the fulfilled purchase.
  purchasedCents: number
}>

// The USD credit -> msat grant leg (hop 2): the bridge debited USD credit and
// granted the equivalent USD-origin msat into the agent balance.
export type CreditToMsatGrantLeg = Readonly<{
  // The bridge grant ref (one ref = one grant).
  grantRef: string
  // USD cents debited from the USD ledger to fund the grant.
  grantedCents: number
  // msat granted into `agent_balances` (USD-origin, inference-spendable).
  grantedMsat: number
}>

// The msat -> inference spend leg (hop 3): the metering hook decremented the
// agent balance from the real provider usage object.
export type InferenceSpendLeg = Readonly<{
  // The served request id (one id per served completion).
  requestId: string
  // msat decremented for the metered charge.
  spentMsat: number
  // The provider-native model actually served (attribution).
  servedModel: string
  // Total tokens from the real provider usage object (observability only).
  totalTokens: number
}>

export type CardCreditSpendReceiptInput = Readonly<{
  purchase: CardCreditPurchaseLeg
  grant: CreditToMsatGrantLeg
  spend: InferenceSpendLeg
}>

// One resolved hop in the assembled chain. `receiptRef` dereferences the leg's
// own receipt; `evidenceRef` (purchase only) resolves the paid event.
export type CardCreditSpendChainStep = Readonly<{
  step: 'card_to_credit' | 'credit_to_msat' | 'msat_to_inference'
  receiptRef: string
  evidenceRef?: string
}>

// The assembled, public-safe, dereferenceable end-to-end receipt.
export type CardCreditSpendReceipt = Readonly<{
  receiptRef: string
  sessionId: string
  // The three hops, in order, each with its dereferenceable ref.
  chain: ReadonlyArray<CardCreditSpendChainStep>
  // Conservation summary (public-safe amounts the owner already sees per leg).
  conservation: Readonly<{
    purchasedCents: number
    grantedCents: number
    grantedMsat: number
    spentMsat: number
    // Granted-minus-spent msat still funded by this purchase (>= 0).
    residualMsat: number
  }>
  servedModel: string
  totalTokens: number
}>

export type CardCreditSpendReceiptResult =
  | Readonly<{ ok: true; receipt: CardCreditSpendReceipt }>
  | Readonly<{
      ok: false
      reason:
        | 'missing_ref'
        | 'nonpositive_amount'
        | 'grant_exceeds_purchase'
        | 'grant_conversion_mismatch'
        | 'spend_exceeds_grant'
      message: string
    }>

const isBlank = (value: string): boolean => value.trim() === ''

const isPositiveInt = (value: number): boolean =>
  Number.isInteger(value) && value > 0

// Assemble the dereferenceable card->credit->inference-spend receipt from the
// three real ledger legs, enforcing the conservation invariants that make the
// chain honest. Returns a typed failure (never throws) when any invariant is
// violated so a caller can refuse to publish a dishonest receipt.
export const assembleCardCreditSpendReceipt = (
  input: CardCreditSpendReceiptInput,
): CardCreditSpendReceiptResult => {
  const { grant, purchase, spend } = input

  if (
    isBlank(purchase.sessionId) ||
    isBlank(grant.grantRef) ||
    isBlank(spend.requestId)
  ) {
    return {
      message: 'Each chain leg must carry a non-empty ref.',
      ok: false,
      reason: 'missing_ref',
    }
  }

  if (
    !isPositiveInt(purchase.purchasedCents) ||
    !isPositiveInt(grant.grantedCents) ||
    !isPositiveInt(grant.grantedMsat) ||
    !isPositiveInt(spend.spentMsat)
  ) {
    return {
      message: 'Every chain amount must be a positive integer.',
      ok: false,
      reason: 'nonpositive_amount',
    }
  }

  // Hop 1 -> 2: the bridge can never grant more USD than the purchase funded.
  if (grant.grantedCents > purchase.purchasedCents) {
    return {
      message: 'Granted credit exceeds the purchased credit.',
      ok: false,
      reason: 'grant_exceeds_purchase',
    }
  }

  // Hop 2 conversion: the granted msat must match the SINGLE-source USD->msat
  // floor conversion the bridge actually used, so the receipt cannot overstate
  // how much spendable balance the dollars produced.
  if (grant.grantedMsat !== usdCentsToMsatFloor(grant.grantedCents)) {
    return {
      message:
        'Granted msat does not match the shared USD->msat conversion of the granted cents.',
      ok: false,
      reason: 'grant_conversion_mismatch',
    }
  }

  // Hop 2 -> 3: this chain claims the purchase funded the spend, so the metered
  // spend may not exceed what this grant made spendable. (Spends drawn from
  // other grants belong to a different chain receipt.)
  if (spend.spentMsat > grant.grantedMsat) {
    return {
      message: 'Metered spend exceeds the credit granted by this purchase.',
      ok: false,
      reason: 'spend_exceeds_grant',
    }
  }

  return {
    ok: true,
    receipt: {
      chain: [
        {
          evidenceRef: cardCreditPurchaseEvidenceRef(purchase.sessionId),
          receiptRef: cardCreditPurchaseLedgerKey(purchase.sessionId),
          step: 'card_to_credit',
        },
        {
          receiptRef: usdCreditGrantReceiptRef(grant.grantRef),
          step: 'credit_to_msat',
        },
        {
          receiptRef: inferenceChargeReceiptRef(spend.requestId),
          step: 'msat_to_inference',
        },
      ],
      conservation: {
        grantedCents: grant.grantedCents,
        grantedMsat: grant.grantedMsat,
        purchasedCents: purchase.purchasedCents,
        residualMsat: grant.grantedMsat - spend.spentMsat,
        spentMsat: spend.spentMsat,
      },
      receiptRef: cardCreditSpendReceiptRef(purchase.sessionId),
      servedModel: spend.servedModel,
      sessionId: purchase.sessionId,
      totalTokens: spend.totalTokens,
    },
  }
}
