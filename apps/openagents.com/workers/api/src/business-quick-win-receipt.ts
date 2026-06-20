import { Schema as S } from 'effect'

/**
 * Canonical receipt for an OpenAgents Business quick win.
 *
 * The promise business.intake_quick_win_offering.v1 is yellow with two open
 * blockers:
 *   - blocker.product_promises.business_quick_win_self_serve_delivery_missing
 *   - blocker.product_promises.business_first_paid_quick_win_receipt_missing
 *
 * Its verification text states green requires "a self-serve quick-win delivery
 * loop and at least one dereferenceable first paid business quick-win receipt
 * (intake -> delivery -> accepted outcome -> receipt)". This module defines the
 * shape of that receipt and a verifier for what "first paid quick-win receipt"
 * must contain, BEFORE any real paid run exists. It flips no promise state and
 * fabricates no settlement: it is the honest contract the delivery loop must
 * later satisfy.
 *
 * Honesty rules, enforced by construction (mirrors omni-gross-margin-receipt):
 * - The full lifecycle (intake -> scoped -> delivered -> accepted -> paid ->
 *   settled) is always named with one line each; states are never collapsed.
 * - A line is only `evidenced` when a concrete reference is supplied.
 * - A later state may not be evidenced while an earlier prerequisite is
 *   unevidenced (you cannot be paid for an outcome that was never delivered).
 * - Settlement-implying states (buyer_paid, provider_settled) are flagged so a
 *   reader can never mistake an unpaid intake for a paid, settled outcome.
 */

export const BusinessQuickWinReceiptStateId = S.Literals([
  // The /business intake captured a real signup (business-signup-routes.ts).
  'intake_recorded',
  // A concrete quick win + definition-of-done was agreed with the customer.
  'quick_win_scoped',
  // The quick win was delivered with a dereferenceable evidence reference.
  'delivered_with_evidence',
  // The customer accepted the outcome (acceptance attestation reference).
  'outcome_accepted',
  // The buyer's payment was captured (settlement-implying).
  'buyer_paid',
  // The provider/operator was settled for the work (settlement-implying).
  'provider_settled',
])
export type BusinessQuickWinReceiptStateId =
  typeof BusinessQuickWinReceiptStateId.Type

export const BusinessQuickWinReceiptEvidenceState = S.Literals([
  'evidenced',
  'not_yet_evidenced',
])
export type BusinessQuickWinReceiptEvidenceState =
  typeof BusinessQuickWinReceiptEvidenceState.Type

export const BusinessQuickWinReceiptLine = S.Struct({
  stateId: BusinessQuickWinReceiptStateId,
  evidenceState: BusinessQuickWinReceiptEvidenceState,
  // A dereferenceable reference (id, URL, or receiptId) backing this state, or
  // null when the state is not yet evidenced.
  evidenceRef: S.NullOr(S.String),
  // True when claiming this state implies a money movement (buyer capture or
  // provider settlement) that this offering must never assert without proof.
  impliesSettlement: S.Boolean,
})
export type BusinessQuickWinReceiptLine =
  typeof BusinessQuickWinReceiptLine.Type

export const BusinessQuickWinReceipt = S.Struct({
  receiptKind: S.Literal('business_quick_win'),
  // The /business intake signup this quick win was scoped from.
  signupId: S.String,
  // The backing offering promiseId from the business menu, e.g.
  // 'business.coding_quick_win.v1'.
  offeringPromiseId: S.String,
  quickWinSummary: S.String,
  lines: S.Array(BusinessQuickWinReceiptLine),
  evidencedStateCount: S.Number,
  unevidencedStateIds: S.Array(BusinessQuickWinReceiptStateId),
  // Derived: true only when buyer_paid is evidenced. Lets a reader gate on a
  // real paid quick win without re-deriving the lifecycle.
  paidQuickWin: S.Boolean,
  // Honest caveat the public projection must surface.
  publicCaveatRef: S.String,
})
export type BusinessQuickWinReceipt = typeof BusinessQuickWinReceipt.Type

export class BusinessQuickWinReceiptInvariantError extends S.TaggedErrorClass<BusinessQuickWinReceiptInvariantError>()(
  'BusinessQuickWinReceiptInvariantError',
  { reason: S.String },
) {}

export type BusinessQuickWinReceiptInput = Readonly<{
  signupId: string
  offeringPromiseId: string
  quickWinSummary: string
  // Evidence references for each progressed state; omit/null when not reached.
  quickWinScopedRef?: string | null
  deliveredEvidenceRef?: string | null
  outcomeAcceptedRef?: string | null
  buyerPaidRef?: string | null
  providerSettledRef?: string | null
  publicCaveatRef?: string
}>

// Lifecycle order: a state may only be evidenced if every earlier state is too.
const LIFECYCLE_ORDER: ReadonlyArray<BusinessQuickWinReceiptStateId> = [
  'intake_recorded',
  'quick_win_scoped',
  'delivered_with_evidence',
  'outcome_accepted',
  'buyer_paid',
  'provider_settled',
]

const SETTLEMENT_IMPLYING_STATES: ReadonlySet<BusinessQuickWinReceiptStateId> =
  new Set(['buyer_paid', 'provider_settled'])

const DEFAULT_CAVEAT_REF =
  'caveat.business_quick_win.operator_assisted_not_self_serve'

const trimmedOrNull = (value: string | null | undefined): string | null => {
  if (value === undefined || value === null) {
    return null
  }
  const trimmed = value.trim()
  return trimmed === '' ? null : trimmed
}

/**
 * Build a quick-win receipt from an intake signup plus whatever delivery
 * evidence exists so far. Deterministic and pure: identical input yields an
 * identical receipt.
 *
 * intake_recorded is evidenced from the (required, non-empty) signupId, since a
 * receipt always derives from a recorded intake. Every later state is evidenced
 * only when a concrete reference is supplied, and the lifecycle-order invariant
 * rejects an evidenced state that skips an unevidenced prerequisite.
 */
export const buildBusinessQuickWinReceipt = (
  input: BusinessQuickWinReceiptInput,
): BusinessQuickWinReceipt => {
  const signupId = trimmedOrNull(input.signupId)
  if (signupId === null) {
    throw new BusinessQuickWinReceiptInvariantError({
      reason: 'signupId is required: a quick-win receipt must derive from a recorded /business intake.',
    })
  }

  const offeringPromiseId = trimmedOrNull(input.offeringPromiseId)
  if (offeringPromiseId === null) {
    throw new BusinessQuickWinReceiptInvariantError({
      reason: 'offeringPromiseId is required and must name a backing offering promiseId from the business menu.',
    })
  }

  const quickWinSummary = trimmedOrNull(input.quickWinSummary)
  if (quickWinSummary === null) {
    throw new BusinessQuickWinReceiptInvariantError({
      reason: 'quickWinSummary is required: a receipt must state the quick win it covers.',
    })
  }

  const refByState: Record<BusinessQuickWinReceiptStateId, string | null> = {
    intake_recorded: signupId,
    quick_win_scoped: trimmedOrNull(input.quickWinScopedRef),
    delivered_with_evidence: trimmedOrNull(input.deliveredEvidenceRef),
    outcome_accepted: trimmedOrNull(input.outcomeAcceptedRef),
    buyer_paid: trimmedOrNull(input.buyerPaidRef),
    provider_settled: trimmedOrNull(input.providerSettledRef),
  }

  const lines: ReadonlyArray<BusinessQuickWinReceiptLine> = LIFECYCLE_ORDER.map(
    stateId => {
      const evidenceRef = refByState[stateId]
      return {
        stateId,
        evidenceState:
          evidenceRef === null ? 'not_yet_evidenced' : 'evidenced',
        evidenceRef,
        impliesSettlement: SETTLEMENT_IMPLYING_STATES.has(stateId),
      }
    },
  )

  // Invariant: no evidenced state may follow an unevidenced prerequisite.
  let sawUnevidenced = false
  for (const line of lines) {
    if (line.evidenceState === 'not_yet_evidenced') {
      sawUnevidenced = true
      continue
    }
    if (sawUnevidenced) {
      throw new BusinessQuickWinReceiptInvariantError({
        reason: `state ${line.stateId} cannot be evidenced while an earlier lifecycle state is not yet evidenced.`,
      })
    }
  }

  const unevidencedStateIds = lines
    .filter(line => line.evidenceState === 'not_yet_evidenced')
    .map(line => line.stateId)

  const paidQuickWin =
    lines.find(line => line.stateId === 'buyer_paid')?.evidenceState ===
    'evidenced'

  return {
    receiptKind: 'business_quick_win',
    signupId,
    offeringPromiseId,
    quickWinSummary,
    lines,
    evidencedStateCount: lines.length - unevidencedStateIds.length,
    unevidencedStateIds,
    paidQuickWin,
    publicCaveatRef: trimmedOrNull(input.publicCaveatRef) ?? DEFAULT_CAVEAT_REF,
  }
}

/**
 * Gate for the registry's "dereferenceable first paid business quick-win
 * receipt": throws unless intake -> scoped -> delivered -> accepted -> paid are
 * all evidenced with concrete references. provider_settled is NOT required (a
 * buyer-paid, accepted quick win is a valid paid receipt even before payout
 * settles), but if absent the receipt's settlement state stays honest.
 *
 * A future green flip of business.intake_quick_win_offering.v1 should pass this
 * verifier against a real receipt; until then it documents the exact bar.
 */
export const REQUIRED_PAID_QUICK_WIN_STATES: ReadonlyArray<BusinessQuickWinReceiptStateId> =
  ['intake_recorded', 'quick_win_scoped', 'delivered_with_evidence', 'outcome_accepted', 'buyer_paid']

export const assertFirstPaidQuickWinReceipt = (
  receipt: BusinessQuickWinReceipt,
): void => {
  for (const stateId of REQUIRED_PAID_QUICK_WIN_STATES) {
    const line = receipt.lines.find(candidate => candidate.stateId === stateId)
    if (line === undefined) {
      throw new BusinessQuickWinReceiptInvariantError({
        reason: `receipt is missing lifecycle state ${stateId}.`,
      })
    }
    if (line.evidenceState !== 'evidenced' || line.evidenceRef === null) {
      throw new BusinessQuickWinReceiptInvariantError({
        reason: `a first paid quick-win receipt requires state ${stateId} to be evidenced with a dereferenceable reference.`,
      })
    }
  }
}

/**
 * Public projection: keeps the full lifecycle and evidence labels visible (so a
 * reader sees exactly which states are unevidenced) but drops internal
 * evidence references.
 */
export const publicBusinessQuickWinReceiptProjection = (
  receipt: BusinessQuickWinReceipt,
) => ({
  receiptKind: receipt.receiptKind,
  offeringPromiseId: receipt.offeringPromiseId,
  lines: receipt.lines.map(line => ({
    stateId: line.stateId,
    evidenceState: line.evidenceState,
    impliesSettlement: line.impliesSettlement,
  })),
  evidencedStateCount: receipt.evidencedStateCount,
  unevidencedStateIds: receipt.unevidencedStateIds,
  paidQuickWin: receipt.paidQuickWin,
  publicCaveatRef: receipt.publicCaveatRef,
})
