import { Schema as S } from 'effect'

/**
 * Coding-quick-win ACCEPTANCE EVIDENCE contract.
 *
 * Promise business.coding_quick_win.v1 is yellow with two open blockers:
 *   - blocker.product_promises.business_coding_quick_win_self_serve_missing
 *   - blocker.product_promises.business_coding_quick_win_paid_receipt_missing
 *
 * The generic business-quick-win receipt (business-quick-win-receipt.ts)
 * treats the `outcome_accepted` state as an opaque string. This module defines
 * what an accepted coding-quick-win outcome must contain so a self-serve
 * loop can advance the receipt automatically without an operator eyeballing a
 * "LGTM" email or PR merge.
 *
 * Honesty rules:
 * - A delivery is only accepted if the customer explicitly approved or merged it.
 * - An explicit rejection is recorded honestly but throws if an upstream attempts
 *   to use it as acceptance evidence.
 */

export const CodingQuickWinAcceptanceAction = S.Literals([
  // The customer explicitly approved the diff/PR.
  'diff_approved',
  // The customer explicitly merged the diff/PR.
  'diff_merged',
  // The customer explicitly rejected the diff/PR (failed acceptance).
  'diff_rejected',
])
export type CodingQuickWinAcceptanceAction =
  typeof CodingQuickWinAcceptanceAction.Type

export const CodingQuickWinAcceptanceEvidence = S.Struct({
  evidenceKind: S.Literal('coding_quick_win_acceptance'),
  offeringPromiseId: S.Literal('business.coding_quick_win.v1'),
  // The diff reference that was accepted (must match the delivery's diffRef).
  diffRef: S.String,
  // The identity of the customer who performed the acceptance action.
  acceptedByUserId: S.String,
  // The recorded action that constitutes acceptance.
  acceptanceAction: CodingQuickWinAcceptanceAction,
  // Dereferenceable proof of the action (e.g., GitHub PR review URL, merge SHA).
  attestationRef: S.String,
  // True only if the action was an approval or merge.
  isAccepted: S.Boolean,
})
export type CodingQuickWinAcceptanceEvidence =
  typeof CodingQuickWinAcceptanceEvidence.Type

export class CodingQuickWinAcceptanceInvariantError extends S.TaggedErrorClass<CodingQuickWinAcceptanceInvariantError>()(
  'CodingQuickWinAcceptanceInvariantError',
  { reason: S.String },
) {
  override get message() {
    return this.reason
  }
}

export type CodingQuickWinAcceptanceInput = Readonly<{
  diffRef: string
  acceptedByUserId: string
  acceptanceAction: CodingQuickWinAcceptanceAction
  attestationRef: string
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
    throw new CodingQuickWinAcceptanceInvariantError({
      reason: `${field} is required: ${why}`,
    })
  }
  return trimmed
}

/**
 * Build coding-quick-win acceptance evidence. Deterministic and pure.
 */
export const buildCodingQuickWinAcceptanceEvidence = (
  input: CodingQuickWinAcceptanceInput,
): CodingQuickWinAcceptanceEvidence => {
  const diffRef = requireField(
    input.diffRef,
    'diffRef',
    'acceptance must declare exactly which diff was accepted.',
  )
  const acceptedByUserId = requireField(
    input.acceptedByUserId,
    'acceptedByUserId',
    'acceptance must record the user who accepted it.',
  )
  const attestationRef = requireField(
    input.attestationRef,
    'attestationRef',
    'acceptance must carry a dereferenceable proof (e.g., PR review URL).',
  )

  const isAccepted =
    input.acceptanceAction === 'diff_approved' ||
    input.acceptanceAction === 'diff_merged'

  return {
    evidenceKind: 'coding_quick_win_acceptance',
    offeringPromiseId: 'business.coding_quick_win.v1',
    diffRef,
    acceptedByUserId,
    acceptanceAction: input.acceptanceAction,
    attestationRef,
    isAccepted,
  }
}

/**
 * Produce the stable `outcomeAcceptedRef` string to feed
 * buildBusinessQuickWinReceipt's `outcomeAcceptedRef`. Returns the attestation
 * reference ONLY for an accepted action; throws otherwise, so a receipt can
 * never claim `outcome_accepted` for a rejection.
 */
export const codingQuickWinAcceptedEvidenceRef = (
  evidence: CodingQuickWinAcceptanceEvidence,
): string => {
  if (!evidence.isAccepted) {
    throw new CodingQuickWinAcceptanceInvariantError({
      reason: `evidence is not accepted: action is ${evidence.acceptanceAction}.`,
    })
  }
  return evidence.attestationRef
}

/**
 * Public projection: drops internal user ID and attestation references.
 */
export const publicCodingQuickWinAcceptanceProjection = (
  evidence: CodingQuickWinAcceptanceEvidence,
) => ({
  evidenceKind: evidence.evidenceKind,
  offeringPromiseId: evidence.offeringPromiseId,
  acceptanceAction: evidence.acceptanceAction,
  isAccepted: evidence.isAccepted,
})
