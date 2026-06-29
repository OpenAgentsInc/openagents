import type { XClaimRewardRecord } from './agent-owner-claim-routes'
import { X_CLAIM_REWARD_AMOUNT_SATS } from './agent-owner-claim-routes'

const X_CLAIM_REWARD_PROMISE_ID = 'agents.x_claim_reward.v1'
const SmokeNotReadyReasonRef =
  'reason.public.x_claim_reward_smoke_transition_not_ready'

const SettledExpectedStateReasonRef =
  'reason.public.x_claim_reward_smoke_unexpected_state'
const AmountMismatchReasonRef =
  'reason.public.x_claim_reward_smoke_amount_mismatch'
const ReceiptRefMalformedReasonRef =
  'reason.public.x_claim_reward_smoke_receipt_ref_malformed'
const SettlementEvidenceMissingReasonRef =
  'reason.public.x_claim_reward_smoke_settlement_evidence_missing'
const SettledReceiptMissingReasonRef =
  'reason.public.x_claim_reward_smoke_settled_receipt_missing'
const PaymentMaterialLeakedReasonRef =
  'reason.public.x_claim_reward_smoke_payment_material_leaked'

const ReceiptRefPattern = /^x_claim_reward_receipt_/
const SettledReceiptPattern = /^receipt\.public\.x_claim_reward\.settled\./
const SettlementEvidencePattern = /^settlement_evidence\.public\./

/**
 * Patterns for payment material that must never appear in a public-safe
 * settled-reward record: lightning invoices, BOLT12 offers, lightning
 * addresses, and 64-hex secrets (preimages / payment hashes).
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

export type XClaimRewardSmokeReceiptCheck = Readonly<{
  name: string
  ok: boolean
  reasonRef: string | null
}>

export type XClaimRewardSmokeReceiptAudit = Readonly<{
  checks: ReadonlyArray<XClaimRewardSmokeReceiptCheck>
  ok: boolean
  /**
   * Public-safe summary the operator can paste into the issue #4626 transition
   * receipt. It carries only campaign-state fields and public evidence refs —
   * never the treasury payment id, destination, invoice, or preimage.
   */
  transitionReceiptSummary: Readonly<{
    amountSats: number
    receiptRef: string
    rewardId: string
    settledReceiptRefs: ReadonlyArray<string>
    settlementEvidenceRefs: ReadonlyArray<string>
    state: string
  }>
  violationReasonRefs: ReadonlyArray<string>
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
 * Audits a settled X-claim reward record after the first live single-reward
 * dispatch smoke. This is a pure, public-safe gate the operator runs before
 * recording the smoke's transition receipt on issue #4626: it confirms the row
 * landed in `settled` for the bounded 1000-sat amount with a well-formed public
 * receipt ref and at least one public settlement evidence ref, and that no
 * payment material (invoice, BOLT12 offer, lightning address, preimage, or
 * payment hash) leaked into any public-facing field. It moves no funds.
 */
export const auditXClaimRewardSmokeReceipt = (
  reward: XClaimRewardRecord,
): XClaimRewardSmokeReceiptAudit => {
  const settledReceiptRefs = reward.evidenceRefs.filter(ref =>
    SettledReceiptPattern.test(ref),
  )
  const settlementEvidenceRefs = reward.evidenceRefs.filter(ref =>
    SettlementEvidencePattern.test(ref),
  )
  const leakedMaterial = findPaymentMaterial(reward)

  const checks: ReadonlyArray<XClaimRewardSmokeReceiptCheck> = [
    {
      name: 'state_is_settled',
      ok: reward.state === 'settled',
      reasonRef:
        reward.state === 'settled' ? null : SettledExpectedStateReasonRef,
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
      name: 'settlement_evidence_present',
      ok: settlementEvidenceRefs.length >= 1,
      reasonRef:
        settlementEvidenceRefs.length >= 1
          ? null
          : SettlementEvidenceMissingReasonRef,
    },
    {
      name: 'settled_receipt_present',
      ok: settledReceiptRefs.length >= 1,
      reasonRef:
        settledReceiptRefs.length >= 1
          ? null
          : SettledReceiptMissingReasonRef,
    },
    {
      name: 'no_payment_material_leaked',
      ok: leakedMaterial.length === 0,
      reasonRef:
        leakedMaterial.length === 0 ? null : PaymentMaterialLeakedReasonRef,
    },
  ]

  const violationReasonRefs = Array.from(
    new Set(
      checks
        .filter(check => !check.ok && check.reasonRef !== null)
        .map(check => check.reasonRef as string),
    ),
  )

  return {
    checks,
    ok: violationReasonRefs.length === 0,
    transitionReceiptSummary: {
      amountSats: reward.amountSats,
      receiptRef: reward.receiptRef,
      rewardId: reward.id,
      settledReceiptRefs,
      settlementEvidenceRefs,
      state: reward.state,
    },
    violationReasonRefs,
  }
}

export type XClaimRewardSmokeTransitionRequest = Readonly<{
  evidenceRefs: ReadonlyArray<string>
  promiseId: typeof X_CLAIM_REWARD_PROMISE_ID
  toState: 'green'
}>

export type XClaimRewardSmokeTransitionProposal = Readonly<{
  audit: XClaimRewardSmokeReceiptAudit
  blockingReasonRefs: ReadonlyArray<string>
  ready: boolean
  /**
   * The public-safe `POST /api/operator/product-promises/transitions` body the
   * operator submits to PROPOSE the green flip. It is `null` until the
   * post-settlement audit passes. Building it changes no promise state and moves
   * no funds: the registry route still re-evaluates blockers, and the green flip
   * additionally requires owner sign-off.
   */
  transitionRequest: XClaimRewardSmokeTransitionRequest | null
}>

/**
 * Assembles the public-safe transition-receipt PROPOSAL that bridges a passing
 * post-settlement audit to `POST /api/operator/product-promises/transitions`.
 *
 * It is the final pure, fund-free step of the live single-reward smoke: it runs
 * the post-settlement audit, and only when that audit passes does it emit a
 * deduped evidence-ref bundle (public receipt ref + public settlement evidence
 * refs) re-scanned for leaked payment material. It never flips a promise state —
 * the registry route re-checks blockers and the green flip still needs operator
 * submission plus owner sign-off.
 */
export const buildXClaimRewardSmokeTransitionRequest = (
  reward: XClaimRewardRecord,
): XClaimRewardSmokeTransitionProposal => {
  const audit = auditXClaimRewardSmokeReceipt(reward)

  if (!audit.ok) {
    return {
      audit,
      blockingReasonRefs: audit.violationReasonRefs,
      ready: false,
      transitionRequest: null,
    }
  }

  const evidenceRefs = Array.from(
    new Set([
      audit.transitionReceiptSummary.receiptRef,
      ...audit.transitionReceiptSummary.settledReceiptRefs,
      ...audit.transitionReceiptSummary.settlementEvidenceRefs,
    ]),
  )

  // Defensive re-scan: never let payment material reach the registry payload,
  // even if a future evidence ref slips past the upstream audit.
  const leakedInProposal = PaymentMaterialPatterns.filter(({ pattern }) =>
    evidenceRefs.some(ref => pattern.test(ref)),
  )

  if (leakedInProposal.length > 0 || evidenceRefs.length === 0) {
    return {
      audit,
      blockingReasonRefs:
        leakedInProposal.length > 0
          ? [PaymentMaterialLeakedReasonRef]
          : [SmokeNotReadyReasonRef],
      ready: false,
      transitionRequest: null,
    }
  }

  return {
    audit,
    blockingReasonRefs: [],
    ready: true,
    transitionRequest: {
      evidenceRefs,
      promiseId: X_CLAIM_REWARD_PROMISE_ID,
      toState: 'green',
    },
  }
}
