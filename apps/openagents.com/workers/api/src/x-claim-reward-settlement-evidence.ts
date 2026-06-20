/**
 * Pre-persistence validator for the `mark_settled` evidence refs of the live
 * single-reward X-claim dispatch smoke.
 *
 * This is the missing middle bookend of the smoke. The candidate gate
 * (`assertXClaimRewardSmokeCandidate`) inspects the `eligible` row before
 * `approve_dispatch`, and the post-settlement audit
 * (`auditXClaimRewardSmokeReceipt`) inspects the `settled` row after the fact —
 * but the evidence refs the operator passes to `mark_settled` were only checked
 * for non-emptiness before being written to the public ledger. A malformed or
 * leaky ref would therefore be PERSISTED publicly and only caught after it had
 * already landed. This pure validator runs at submit time, before persistence,
 * so payment material never reaches the row in the first place. It moves no
 * funds and reads no secrets.
 */

const NoEvidenceReasonRef =
  'reason.public.x_claim_reward_settlement_requires_evidence'
const NoPublicSettlementRefReasonRef =
  'reason.public.x_claim_reward_settlement_missing_public_ref'
const PaymentMaterialLeakedReasonRef =
  'reason.public.x_claim_reward_settlement_payment_material_leaked'

const SettlementEvidencePattern = /^settlement_evidence\.public\./

/**
 * Patterns for payment material that must never be persisted onto a public
 * reward row: lightning invoices, BOLT12 offers, lightning addresses, and
 * 64-hex secrets (preimages / payment hashes). Mirrors the candidate gate and
 * post-settlement auditor so every bookend of the smoke rejects the same
 * leakage shapes.
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

export type XClaimRewardSettlementEvidenceCheck = Readonly<{
  name: string
  ok: boolean
  reasonRef: string | null
}>

export type XClaimRewardSettlementEvidenceGate = Readonly<{
  /**
   * The trimmed, deduped, non-empty refs that are safe to persist. Empty when
   * the gate does not pass.
   */
  acceptedRefs: ReadonlyArray<string>
  blockingReasonRefs: ReadonlyArray<string>
  checks: ReadonlyArray<XClaimRewardSettlementEvidenceCheck>
  /**
   * Whether the submitted refs are safe to persist on the public reward row.
   * `true` means the route may proceed to write the `settled` transition.
   */
  ok: boolean
}>

/**
 * Validates the settlement evidence refs supplied to `mark_settled` BEFORE they
 * are appended to the public reward row. It passes only when:
 *
 * - `at_least_one_public_settlement_ref` — at least one trimmed ref matches
 *   `settlement_evidence.public.*` (the runbook's required campaign-wallet
 *   evidence shape), which also rejects the empty case.
 * - `no_payment_material_leaked` — no submitted ref contains a lightning
 *   invoice, BOLT12 offer, lightning address, preimage, or payment hash.
 */
export const assertXClaimRewardSettlementEvidenceRefs = (
  evidenceRefs: ReadonlyArray<string>,
): XClaimRewardSettlementEvidenceGate => {
  const acceptedRefs = Array.from(
    new Set(evidenceRefs.map(ref => ref.trim()).filter(ref => ref.length > 0)),
  )

  const hasPublicSettlementRef = acceptedRefs.some(ref =>
    SettlementEvidencePattern.test(ref),
  )

  const leakedMaterial = PaymentMaterialPatterns.filter(({ pattern }) =>
    acceptedRefs.some(ref => pattern.test(ref)),
  ).map(({ name }) => name)

  const checks: ReadonlyArray<XClaimRewardSettlementEvidenceCheck> = [
    {
      name: 'at_least_one_public_settlement_ref',
      ok: hasPublicSettlementRef,
      reasonRef: hasPublicSettlementRef
        ? null
        : acceptedRefs.length === 0
          ? NoEvidenceReasonRef
          : NoPublicSettlementRefReasonRef,
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
    acceptedRefs: blockingReasonRefs.length === 0 ? acceptedRefs : [],
    blockingReasonRefs,
    checks,
    ok: blockingReasonRefs.length === 0,
  }
}
