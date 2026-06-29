import { Schema as S } from 'effect'

/**
 * Coding-quick-win DELIVERY EVIDENCE contract.
 *
 * Promise business.coding_quick_win.v1 is yellow with two open blockers:
 *   - blocker.product_promises.business_coding_quick_win_self_serve_missing
 *   - blocker.product_promises.business_coding_quick_win_paid_receipt_missing
 *
 * Its claim is that "a written objective is taken into a repository, the
 * customer's verification command is run, and a reviewable change is handed back
 * with verification evidence". The generic business-quick-win receipt
 * (business-quick-win-receipt.ts) treats the `delivered_with_evidence` state as
 * an opaque `deliveredEvidenceRef` string. That is fine for an operator who
 * eyeballs a PR link, but a SELF-SERVE loop cannot rely on a human to judge
 * whether a delivery is real.
 *
 * This module defines what a coding-quick-win delivery must contain to count as
 * `delivered_with_evidence`, and a verifier that decides — without an operator —
 * whether the delivery is acceptable. It flips no promise state and fabricates
 * no run: it is the honest, machine-checkable gate the self-serve delivery loop
 * must pass before a coding quick win can be handed back.
 *
 * Honesty rules, enforced by construction:
 * - A delivery is only `verification_passed` when the customer's verification
 *   command actually ran and exited 0; a non-zero/absent exit is never passed.
 * - A delivery must carry a reviewable diff reference: a coding quick win with
 *   no change is not a delivery.
 * - The verification command is recorded verbatim so a reader can re-run it; a
 *   blank command is rejected (you cannot verify against nothing).
 * - "Reviewable, not merged": this evidence asserts a diff under review, never an
 *   auto-merge or deploy.
 */

export const CodingQuickWinVerificationStatus = S.Literals([
  // The customer's verification command ran and exited 0.
  'verification_passed',
  // The command ran but exited non-zero: a real, honest failed delivery.
  'verification_failed',
  // The command was not run (e.g. environment could not execute it).
  'verification_not_run',
])
export type CodingQuickWinVerificationStatus =
  typeof CodingQuickWinVerificationStatus.Type

export const CodingQuickWinDeliveryEvidence = S.Struct({
  evidenceKind: S.Literal('coding_quick_win_delivery'),
  // The backing offering promiseId; always business.coding_quick_win.v1 here.
  offeringPromiseId: S.Literal('business.coding_quick_win.v1'),
  // The repository the objective was taken into (e.g. owner/repo or a URL).
  repo: S.String,
  // The base ref/commit the change is built on, so the diff is reproducible.
  baseRef: S.String,
  // The customer's verification command, recorded verbatim for re-run.
  verificationCommand: S.String,
  // The verification command's exit code (null only when not run).
  verificationExitCode: S.NullOr(S.Number),
  // Dereferenceable reference to the captured command output (log id/URL).
  verificationOutputRef: S.NullOr(S.String),
  // Dereferenceable reference to the reviewable diff (PR URL, patch id, etc.).
  diffRef: S.String,
  // Derived verification status (never asserted without a 0 exit code).
  verificationStatus: CodingQuickWinVerificationStatus,
  // True only when the delivery is verification_passed AND carries a diff: the
  // single boolean a self-serve loop can gate "hand back to customer" on.
  acceptableForHandback: S.Boolean,
  // Honest caveat the public projection must surface.
  reviewGateCaveatRef: S.String,
})
export type CodingQuickWinDeliveryEvidence =
  typeof CodingQuickWinDeliveryEvidence.Type

export class CodingQuickWinDeliveryInvariantError extends S.TaggedErrorClass<CodingQuickWinDeliveryInvariantError>()(
  'CodingQuickWinDeliveryInvariantError',
  { reason: S.String },
) {}

export type CodingQuickWinDeliveryInput = Readonly<{
  repo: string
  baseRef: string
  verificationCommand: string
  // Omit/null when the command could not be run at all.
  verificationExitCode?: number | null
  verificationOutputRef?: string | null
  diffRef: string
  reviewGateCaveatRef?: string
}>

const DEFAULT_REVIEW_GATE_CAVEAT_REF =
  'caveat.coding_quick_win.reviewable_not_merged'

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
    throw new CodingQuickWinDeliveryInvariantError({
      reason: `${field} is required: ${why}`,
    })
  }
  return trimmed
}

/**
 * Build coding-quick-win delivery evidence from a runtime run. Deterministic and
 * pure: identical input yields identical evidence.
 *
 * verificationStatus is derived, never trusted from the caller:
 * - exit code 0            -> verification_passed
 * - exit code non-zero     -> verification_failed
 * - exit code null/missing -> verification_not_run
 *
 * acceptableForHandback is true ONLY for verification_passed with a diff, so a
 * self-serve loop can gate handback on a single honest boolean.
 */
export const buildCodingQuickWinDeliveryEvidence = (
  input: CodingQuickWinDeliveryInput,
): CodingQuickWinDeliveryEvidence => {
  const repo = requireField(
    input.repo,
    'repo',
    'a coding quick win must name the repository the objective was taken into.',
  )
  const baseRef = requireField(
    input.baseRef,
    'baseRef',
    'the change must declare the base ref it is built on so the diff is reproducible.',
  )
  const verificationCommand = requireField(
    input.verificationCommand,
    'verificationCommand',
    "the customer's verification command must be recorded verbatim; you cannot verify against nothing.",
  )
  const diffRef = requireField(
    input.diffRef,
    'diffRef',
    'a coding quick win must hand back a reviewable diff reference; no diff is not a delivery.',
  )

  const exitCode =
    input.verificationExitCode === undefined
      ? null
      : input.verificationExitCode

  const verificationStatus: CodingQuickWinVerificationStatus =
    exitCode === null
      ? 'verification_not_run'
      : exitCode === 0
        ? 'verification_passed'
        : 'verification_failed'

  // A passed verification must carry an output reference: a "pass" with no
  // captured output is unfalsifiable and not acceptable as self-serve evidence.
  const verificationOutputRef = trimmedOrNull(input.verificationOutputRef)
  if (verificationStatus === 'verification_passed' && verificationOutputRef === null) {
    throw new CodingQuickWinDeliveryInvariantError({
      reason:
        'a verification_passed delivery must carry verificationOutputRef so the passing run can be re-checked.',
    })
  }

  const acceptableForHandback = verificationStatus === 'verification_passed'

  return {
    evidenceKind: 'coding_quick_win_delivery',
    offeringPromiseId: 'business.coding_quick_win.v1',
    repo,
    baseRef,
    verificationCommand,
    verificationExitCode: exitCode,
    verificationOutputRef,
    diffRef,
    verificationStatus,
    acceptableForHandback,
    reviewGateCaveatRef:
      trimmedOrNull(input.reviewGateCaveatRef) ?? DEFAULT_REVIEW_GATE_CAVEAT_REF,
  }
}

/**
 * Gate for a self-serve coding-quick-win handback: throws unless the delivery
 * ran the customer's verification command, that command exited 0, and a
 * reviewable diff is present. This is the check the self-serve loop must pass
 * before marking the receipt's `delivered_with_evidence` state.
 */
export const assertCodingQuickWinDeliverable = (
  evidence: CodingQuickWinDeliveryEvidence,
): void => {
  if (evidence.verificationStatus !== 'verification_passed') {
    throw new CodingQuickWinDeliveryInvariantError({
      reason: `delivery is not handback-ready: verification status is ${evidence.verificationStatus}, not verification_passed.`,
    })
  }
  if (!evidence.acceptableForHandback) {
    throw new CodingQuickWinDeliveryInvariantError({
      reason: 'delivery is not acceptable for handback.',
    })
  }
}

/**
 * Produce the stable `deliveredEvidenceRef` string to feed
 * buildBusinessQuickWinReceipt's `deliveredEvidenceRef`. Returns the diff
 * reference ONLY for a handback-ready delivery; throws otherwise, so a receipt
 * can never claim `delivered_with_evidence` for a failed or empty delivery.
 */
export const codingQuickWinDeliveredEvidenceRef = (
  evidence: CodingQuickWinDeliveryEvidence,
): string => {
  assertCodingQuickWinDeliverable(evidence)
  return evidence.diffRef
}

/**
 * Public projection: keeps the verification status, command, and review-gate
 * caveat visible but drops internal output/diff references.
 */
export const publicCodingQuickWinDeliveryProjection = (
  evidence: CodingQuickWinDeliveryEvidence,
) => ({
  evidenceKind: evidence.evidenceKind,
  offeringPromiseId: evidence.offeringPromiseId,
  repo: evidence.repo,
  verificationCommand: evidence.verificationCommand,
  verificationStatus: evidence.verificationStatus,
  acceptableForHandback: evidence.acceptableForHandback,
  reviewGateCaveatRef: evidence.reviewGateCaveatRef,
})
