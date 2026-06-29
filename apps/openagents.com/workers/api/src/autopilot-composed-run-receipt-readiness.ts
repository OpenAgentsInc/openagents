// Autopilot composed-run REAL-BUSINESS-RECEIPT readiness report — the ONE
// reviewer-facing artifact that binds the acceptance gate's per-criterion verdict
// to the evidence manifest's per-criterion requirement, over a public-safe
// composed-run receipt context (EPIC #5510, child #5519; promises
// cloud.primitives_suite.v1, cloud.agent_cloud_one_stop_revshare.v1,
// autopilot.all_in_one_business_system.v1 — all planned).
//
// The two upstream modules each answer half the reviewer's question:
//   - the gate (autopilot-composed-run-receipt-gate.ts) answers "does this
//     evidence SATISFY each criterion?" (satisfied + detail), and
//   - the manifest (autopilot-composed-run-receipt-manifest.ts) answers "WHERE
//     does each criterion's evidence come from / what dereferenceable artifact
//     must a real armed run produce?" (governingRef + requiredArtifact).
// Neither alone is the single thing a reviewer (or a future armed run) reads to
// see, in ONE ordered list: for each criterion, whether it currently holds, why,
// and — when it does not — exactly which artifact is still owed and which seam /
// proof primitive governs it. This module is that join: one stable, ordered
// readiness report, with a public-safe receipt context and an overall verdict.
//
// SCOPE / HONESTY: this is PURE. It composes the existing gate result and the
// existing manifest; it duplicates none of their logic and INTRODUCES no new
// pass/fail rule. It moves no money, settles no charge, writes no receipt,
// records no owner sign-off, FLIPS NO promise state, and DROPS NO blocker. A
// `clearsBlocker: true` verdict here is a REPORT, not an action: recording the
// green flip stays an owner-gated step outside this module. Applied to the
// current inert receipt the report's verdict is `clearsBlocker: false` and it
// names the honest outstanding artifacts (components not billed, no owner
// sign-off, demand not external market).

import {
  composedRunReceiptProjection,
  type ComposedRunReceipt,
  type ComposedRunReceiptProjection,
} from './autopilot-composed-run-receipt'
import {
  evaluateRealBusinessReceiptGate,
  inertReceiptGateEvidence,
  type RealBusinessReceiptCriterionId,
  type RealBusinessReceiptEvidence,
  type RealBusinessReceiptGateResult,
} from './autopilot-composed-run-receipt-gate'
import {
  requirementForCriterion,
  type RealBusinessReceiptEvidenceRequirement,
} from './autopilot-composed-run-receipt-manifest'

export const AUTOPILOT_COMPOSED_RUN_RECEIPT_READINESS_SCHEMA =
  'openagents.autopilot_composed_run_receipt_readiness.v1' as const

/**
 * One line of the readiness report: the gate criterion, whether it currently
 * holds (and the gate's human-readable reason), plus the manifest's requirement
 * for it — the governing seam/proof ref, the dereferenceable artifact a real run
 * must produce, the evidence fields the gate reads, and the prose requirement.
 * Public-safe: refs + booleans + prose only; carries no amounts, idempotency
 * keys, or payment destinations.
 */
export type RealBusinessReceiptReadinessLine = Readonly<{
  criterionId: RealBusinessReceiptCriterionId
  /** From the gate: whether the supplied evidence satisfies this criterion. */
  satisfied: boolean
  /** From the gate: the human-readable reason it is (un)satisfied. */
  detail: string
  /** From the manifest: proof primitive ref OR the seam/module that emits it. */
  governingRef: string
  /** From the manifest: the dereferenceable artifact a real armed run produces. */
  requiredArtifact: string
  /** From the manifest: what must be true for this criterion to hold. */
  requirement: string
  /** From the manifest: the evidence field(s) the gate reads for this criterion. */
  evidenceFields: RealBusinessReceiptEvidenceRequirement['evidenceFields']
}>

/**
 * The consolidated readiness report for the real-business-receipt blocker over
 * one composed-run receipt's evidence: a public-safe receipt context, one ordered
 * line per gate criterion (verdict + governing artifact), the satisfied/total
 * tally, the artifacts still owed, and the overall verdict. `clearsBlocker` is a
 * REPORT mirroring the gate; it flips no promise and drops no blocker.
 */
export type RealBusinessReceiptReadinessReport = Readonly<{
  schema: typeof AUTOPILOT_COMPOSED_RUN_RECEIPT_READINESS_SCHEMA
  runId: string
  /** The single blocker this report decides on. */
  blockerRef: RealBusinessReceiptGateResult['blockerRef']
  /** Public-safe receipt context (no amounts, keys, or destinations). */
  receipt: ComposedRunReceiptProjection
  /** One line per gate criterion, in the gate's stable order. */
  lines: ReadonlyArray<RealBusinessReceiptReadinessLine>
  /** Count of satisfied criteria. */
  satisfiedCount: number
  /** Total criteria evaluated. */
  totalCount: number
  /** The artifacts still owed (the requirement for each unsatisfied criterion). */
  outstandingArtifacts: ReadonlyArray<RealBusinessReceiptEvidenceRequirement>
  /** True ONLY when every criterion is satisfied. A report, not an action. */
  clearsBlocker: boolean
  /** Blocker refs still open after this evidence (the report never drops one). */
  unclearedBlockerRefs: ReadonlyArray<string>
}>

/**
 * Build the consolidated readiness report from a composed-run receipt's evidence.
 * PURE: runs the existing gate, joins each criterion to its manifest requirement,
 * and projects a public-safe receipt context. Introduces NO new pass/fail rule —
 * `clearsBlocker` mirrors the gate exactly. Decides nothing irreversible: flips no
 * promise, drops no blocker, moves no money.
 */
export const buildRealBusinessReceiptReadinessReport = (
  evidence: RealBusinessReceiptEvidence,
): RealBusinessReceiptReadinessReport => {
  const result = evaluateRealBusinessReceiptGate(evidence)

  const lines: ReadonlyArray<RealBusinessReceiptReadinessLine> =
    result.criteria.map(criterion => {
      const requirement = requirementForCriterion(criterion.id)
      return {
        criterionId: criterion.id,
        satisfied: criterion.satisfied,
        detail: criterion.detail,
        governingRef: requirement.governingRef,
        requiredArtifact: requirement.requiredArtifact,
        requirement: requirement.requirement,
        evidenceFields: requirement.evidenceFields,
      }
    })

  const satisfiedCount = lines.filter(line => line.satisfied).length

  const outstandingArtifacts = result.unsatisfiedCriteria.map(criterionId =>
    requirementForCriterion(criterionId),
  )

  return {
    schema: AUTOPILOT_COMPOSED_RUN_RECEIPT_READINESS_SCHEMA,
    runId: result.runId,
    blockerRef: result.blockerRef,
    receipt: composedRunReceiptProjection(evidence.receipt),
    lines,
    satisfiedCount,
    totalCount: lines.length,
    outstandingArtifacts,
    clearsBlocker: result.clearsBlocker,
    unclearedBlockerRefs: result.unclearedBlockerRefs,
  }
}

/**
 * Build the HONEST status-quo readiness report for a reconciled receipt SHAPE:
 * nothing billed, no revenue, no owner sign-off, internal first-party demand
 * (inertReceiptGateEvidence). Its verdict is `clearsBlocker: false` and it names
 * the outstanding artifacts — the status quo, rendered as one reviewer-facing
 * report. PURE.
 */
export const inertReadinessReport = (
  receipt: ComposedRunReceipt,
): RealBusinessReceiptReadinessReport =>
  buildRealBusinessReceiptReadinessReport(inertReceiptGateEvidence(receipt))
