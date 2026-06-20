import type { XClaimRewardRecord } from './agent-owner-claim-routes'
import { X_CLAIM_REWARD_AMOUNT_SATS } from './agent-owner-claim-routes'

const StateNotEligibleReasonRef =
  'reason.public.x_claim_reward_smoke_candidate_not_eligible'
const AmountMismatchReasonRef =
  'reason.public.x_claim_reward_smoke_candidate_amount_mismatch'
const ReceiptRefMalformedReasonRef =
  'reason.public.x_claim_reward_smoke_candidate_receipt_ref_malformed'
const PaymentAlreadyAttachedReasonRef =
  'reason.public.x_claim_reward_smoke_candidate_payment_already_attached'
const PaymentMaterialLeakedReasonRef =
  'reason.public.x_claim_reward_smoke_candidate_payment_material_leaked'

const ReceiptRefPattern = /^x_claim_reward_receipt_/

/**
 * Patterns for payment material that must never already be attached to a reward
 * row offered up as the live single-reward smoke candidate: lightning invoices,
 * BOLT12 offers, lightning addresses, and 64-hex secrets (preimages / payment
 * hashes). Mirrors the post-settlement receipt auditor so both bookends of the
 * smoke reject the same leakage shapes.
 */
const PaymentMaterialPatterns: ReadonlyArray<Readonly<{
  name: string
  pattern: RegExp
}>> = [
  { name: 'lightning_invoice', pattern: /ln(bc|tb|bcrt|sb)[0-9][a-z0-9]+/i },
  { name: 'bolt12_offer', pattern: /lno1[a-z0-9]{20,}/i },
  {
    name: 'lightning_address',
    pattern: /[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}/i,
  },
  { name: 'hex_secret', pattern: /\b[0-9a-f]{64}\b/i },
]

export type XClaimRewardSmokeCandidateCheck = Readonly<{
  name: string
  ok: boolean
  reasonRef: string | null
}>

export type XClaimRewardSmokeCandidateGate = Readonly<{
  blockingReasonRefs: ReadonlyArray<string>
  checks: ReadonlyArray<XClaimRewardSmokeCandidateCheck>
  /**
   * Whether this specific reward row is a clean candidate for the first live
   * single-reward dispatch smoke. `true` means the operator may proceed to the
   * `approve_dispatch` step in the runbook for this row.
   */
  ready: boolean
  /**
   * Public-safe echo of the candidate. It carries only campaign-state fields —
   * never a destination, receive code, invoice, or preimage.
   */
  candidateSummary: Readonly<{
    amountSats: number
    receiptRef: string
    rewardId: string
    state: string
  }>
}>

const findPaymentMaterial = (
  reward: XClaimRewardRecord,
): ReadonlyArray<string> => {
  const scannedFields: ReadonlyArray<string> = [
    reward.receiptRef,
    reward.stateReasonRef ?? '',
    ...reward.evidenceRefs,
  ]

  return PaymentMaterialPatterns.filter(({ pattern }) =>
    scannedFields.some(field => pattern.test(field)),
  ).map(({ name }) => name)
}

/**
 * Pre-dispatch candidate gate for the live single-reward X-claim dispatch smoke.
 *
 * This is the front bookend of the smoke: a pure, public-safe per-row check the
 * operator runs on the chosen reward BEFORE calling `approve_dispatch`. It moves
 * no funds and complements the aggregate `evaluateXClaimRewardSmokePreflight`
 * (which inspects ledger-wide stats) by confirming the exact row picked for the
 * smoke is a clean starting point: it sits in `eligible`, carries the bounded
 * 1000-sat amount with a well-formed public receipt ref, has no treasury payment
 * id attached yet, and leaks no payment material in any public-facing field.
 */
export const assertXClaimRewardSmokeCandidate = (
  reward: XClaimRewardRecord,
): XClaimRewardSmokeCandidateGate => {
  const leakedMaterial = findPaymentMaterial(reward)

  const checks: ReadonlyArray<XClaimRewardSmokeCandidateCheck> = [
    {
      name: 'state_is_eligible',
      ok: reward.state === 'eligible',
      reasonRef: reward.state === 'eligible' ? null : StateNotEligibleReasonRef,
    },
    {
      name: 'amount_is_campaign_reward',
      ok: reward.amountSats === X_CLAIM_REWARD_AMOUNT_SATS,
      reasonRef:
        reward.amountSats === X_CLAIM_REWARD_AMOUNT_SATS
          ? null
          : AmountMismatchReasonRef,
    },
    {
      name: 'receipt_ref_well_formed',
      ok: ReceiptRefPattern.test(reward.receiptRef),
      reasonRef: ReceiptRefPattern.test(reward.receiptRef)
        ? null
        : ReceiptRefMalformedReasonRef,
    },
    {
      name: 'no_treasury_payment_attached',
      ok: reward.treasuryPaymentId === null,
      reasonRef:
        reward.treasuryPaymentId === null
          ? null
          : PaymentAlreadyAttachedReasonRef,
    },
    {
      name: 'no_payment_material_leaked',
      ok: leakedMaterial.length === 0,
      reasonRef:
        leakedMaterial.length === 0 ? null : PaymentMaterialLeakedReasonRef,
    },
  ]

  const blockingReasonRefs = Array.from(
    new Set(
      checks
        .filter(check => !check.ok && check.reasonRef !== null)
        .map(check => check.reasonRef as string),
    ),
  )

  return {
    blockingReasonRefs,
    checks,
    ready: blockingReasonRefs.length === 0,
    candidateSummary: {
      amountSats: reward.amountSats,
      receiptRef: reward.receiptRef,
      rewardId: reward.id,
      state: reward.state,
    },
  }
}
