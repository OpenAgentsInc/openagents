import { Schema as S } from 'effect'

/**
 * Business-quick-win PAYMENT EVIDENCE contract.
 *
 * The promise business.intake_quick_win_offering.v1 and its backing offerings
 * (like business.coding_quick_win.v1) require a machine-checkable payment gate
 * to close their self-serve blockers.
 *
 * The generic business-quick-win receipt (business-quick-win-receipt.ts)
 * treats the `buyer_paid` state as an opaque string. This module defines
 * what a paid business quick win must contain so a self-serve loop can advance
 * the receipt automatically without an operator eyeballing a Stripe dashboard
 * or lightning invoice.
 *
 * Honesty rules:
 * - A payment is only accepted if its status is 'settled'.
 * - An unpaid or pending payment is recorded honestly but throws if an upstream
 *   attempts to use it as payment evidence.
 */

export const BusinessQuickWinPaymentStatus = S.Literals([
  // The payment successfully settled (money captured).
  'settled',
  // The payment is pending or incomplete.
  'pending',
  // The payment failed or was rejected.
  'failed',
])
export type BusinessQuickWinPaymentStatus =
  typeof BusinessQuickWinPaymentStatus.Type

export const BusinessQuickWinPaymentEvidence = S.Struct({
  evidenceKind: S.Literal('business_quick_win_payment'),
  // The /business intake signup this payment is for.
  signupId: S.String,
  // The amount paid (in the smallest currency unit, e.g., cents or sats).
  amount: S.Number,
  // The currency code (e.g., 'usd', 'sat').
  currency: S.String,
  // The payment status.
  paymentStatus: BusinessQuickWinPaymentStatus,
  // Dereferenceable proof of the payment (e.g., Stripe charge ID, L402 receipt).
  paymentRef: S.String,
  // True only if the payment successfully settled.
  isPaid: S.Boolean,
})
export type BusinessQuickWinPaymentEvidence =
  typeof BusinessQuickWinPaymentEvidence.Type

export class BusinessQuickWinPaymentInvariantError extends S.TaggedErrorClass<BusinessQuickWinPaymentInvariantError>()(
  'BusinessQuickWinPaymentInvariantError',
  { reason: S.String },
) {
  override get message() {
    return this.reason
  }
}

export type BusinessQuickWinPaymentInput = Readonly<{
  signupId: string
  amount: number
  currency: string
  paymentStatus: BusinessQuickWinPaymentStatus
  paymentRef: string
}>

const trimmedOrNull = (value: string | null | undefined): string | null => {
  if (value === undefined || value === null) {
    return null
  }
  const trimmed = value.trim()
  return trimmed === '' ? null : trimmed
}

const requireField = (
  value: string | null | undefined,
  field: string,
  why: string,
): string => {
  const trimmed = trimmedOrNull(value)
  if (trimmed === null) {
    throw new BusinessQuickWinPaymentInvariantError({
      reason: `${field} is required: ${why}`,
    })
  }
  return trimmed
}

/**
 * Build business-quick-win payment evidence. Deterministic and pure.
 */
export const buildBusinessQuickWinPaymentEvidence = (
  input: BusinessQuickWinPaymentInput,
): BusinessQuickWinPaymentEvidence => {
  const signupId = requireField(
    input.signupId,
    'signupId',
    'payment evidence must link to the signup it settles.',
  )
  const currency = requireField(
    input.currency,
    'currency',
    'payment evidence must specify the currency code.',
  )
  const paymentRef = requireField(
    input.paymentRef,
    'paymentRef',
    'payment evidence must carry a dereferenceable proof (e.g., Stripe charge ID).',
  )

  if (input.amount <= 0) {
    throw new BusinessQuickWinPaymentInvariantError({
      reason: 'payment amount must be greater than zero.',
    })
  }

  const isPaid = input.paymentStatus === 'settled'

  return {
    evidenceKind: 'business_quick_win_payment',
    signupId,
    amount: input.amount,
    currency,
    paymentStatus: input.paymentStatus,
    paymentRef,
    isPaid,
  }
}

/**
 * Produce the stable `buyerPaidRef` string to feed
 * buildBusinessQuickWinReceipt's `buyerPaidRef`. Returns the payment reference
 * ONLY for a settled payment; throws otherwise, so a receipt can never claim
 * `buyer_paid` for a failed or pending payment.
 */
export const businessQuickWinPaidEvidenceRef = (
  evidence: BusinessQuickWinPaymentEvidence,
): string => {
  if (!evidence.isPaid) {
    throw new BusinessQuickWinPaymentInvariantError({
      reason: `payment is not settled: status is ${evidence.paymentStatus}.`,
    })
  }
  return evidence.paymentRef
}

/**
 * Public projection: drops the internal paymentRef.
 */
export const publicBusinessQuickWinPaymentProjection = (
  evidence: BusinessQuickWinPaymentEvidence,
) => ({
  evidenceKind: evidence.evidenceKind,
  amount: evidence.amount,
  currency: evidence.currency,
  paymentStatus: evidence.paymentStatus,
  isPaid: evidence.isPaid,
})
