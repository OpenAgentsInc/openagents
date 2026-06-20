import type { AutopilotWorkReviewAction } from './autopilot-work-routes'
import {
  autopilotDecisionCloseoutRef,
  type AutopilotDecisionCloseoutOutcome,
} from './autopilot-decision-closeout'
import type { AutopilotDecisionCloseoutLedger } from './autopilot-decision-closeout-ledger'

// #5004 receipt-backed command closeout — completeness verification.
//
// Earlier runs built the closeout RECEIPT (`buildAutopilotDecisionCloseoutReceipt`)
// and the accumulation LEDGER (`createAutopilotDecisionCloseoutLedger`). Together
// they let a closeout be produced and stored. But "receipt-backed command
// closeout" makes a stronger claim than "closeouts can accumulate": it asserts
// that EVERY resolved decision has a dereferenceable closeout — no silent gaps.
// Nothing yet verified that. A decision could be recorded as resolved in the work
// store while its closeout was never appended (a crash between record + append, a
// missed call site, a future non-review act path) and no audit would notice.
//
// This module is that verification. Given the set of decisions the work store
// reports as RESOLVED and a closeout ledger, it reconciles the two and reports:
//   - `covered`  : resolved decisions whose closeout is present (the happy path)
//   - `missing`  : resolved decisions with NO closeout — an audit GAP, the
//                  receipt-backed invariant is violated for that decision
//   - `orphans`  : closeoutRefs in the ledger with no matching resolved decision
//                  in the reconciled set (stale/foreign closeout)
//   - `complete` : true iff there are no gaps (every resolved decision is covered)
//
// Pure: no I/O, no time, no store. The caller supplies the resolved set and the
// ledger; this module derives each decision's exactly-once closeoutRef from the
// same single-source helper the builder stamps, so the keys always line up.

// The minimal facts the reconciler needs about a resolved decision. These are
// exactly the fields `actOnDecision` already has when it records a review
// decision, so the route can hand them straight in.
export type ResolvedAutopilotDecision = Readonly<{
  decisionRef: string
  workOrderRef: string
  action: AutopilotWorkReviewAction
}>

export type AutopilotDecisionCoverageEntry = Readonly<{
  decisionRef: string
  workOrderRef: string
  action: AutopilotWorkReviewAction
  closeoutRef: string
  // Present only when the closeout exists (covered entries).
  outcome?: AutopilotDecisionCloseoutOutcome
}>

export type AutopilotDecisionCloseoutCoverage = Readonly<{
  // True iff every resolved decision has a closeout (no gaps).
  complete: boolean
  // Resolved decisions whose closeout is present in the ledger.
  covered: ReadonlyArray<AutopilotDecisionCoverageEntry>
  // Resolved decisions with NO closeout — the receipt-backed invariant is broken.
  missing: ReadonlyArray<AutopilotDecisionCoverageEntry>
  // closeoutRefs in the ledger not matched by any resolved decision here.
  orphans: ReadonlyArray<string>
}>

const byCloseoutRef = (
  left: AutopilotDecisionCoverageEntry,
  right: AutopilotDecisionCoverageEntry,
): number => left.closeoutRef.localeCompare(right.closeoutRef)

// Reconcile a set of resolved decisions against a closeout ledger. Pure.
//
// Multiple resolved entries that map to the same closeoutRef (the idempotent
// replay case — same work order + action) collapse to one expected key, so a
// replay never shows as a duplicate gap. The ledger is read through its public
// `get` / `list` contract, so a persistent (D1/KV) ledger that satisfies the
// same interface reconciles identically.
export const reconcileAutopilotDecisionCloseoutCoverage = (
  input: Readonly<{
    resolved: ReadonlyArray<ResolvedAutopilotDecision>
    ledger: Pick<AutopilotDecisionCloseoutLedger, 'get' | 'list'>
  }>,
): AutopilotDecisionCloseoutCoverage => {
  // Collapse resolved decisions to their exactly-once closeoutRef. Insertion
  // order preserved; later duplicates for the same key are ignored.
  const expected = new Map<string, ResolvedAutopilotDecision>()
  for (const decision of input.resolved) {
    const closeoutRef = autopilotDecisionCloseoutRef(
      decision.action,
      decision.workOrderRef,
    )
    if (!expected.has(closeoutRef)) {
      expected.set(closeoutRef, decision)
    }
  }

  const covered: AutopilotDecisionCoverageEntry[] = []
  const missing: AutopilotDecisionCoverageEntry[] = []

  for (const [closeoutRef, decision] of expected) {
    const receipt = input.ledger.get(closeoutRef)
    const base = {
      action: decision.action,
      closeoutRef,
      decisionRef: decision.decisionRef,
      workOrderRef: decision.workOrderRef,
    }
    if (receipt === undefined) {
      missing.push(base)
    } else {
      covered.push({ ...base, outcome: receipt.outcome })
    }
  }

  const orphans = input.ledger
    .list()
    .map(receipt => receipt.closeoutRef)
    .filter(closeoutRef => !expected.has(closeoutRef))
    .sort((left, right) => left.localeCompare(right))

  return {
    complete: missing.length === 0,
    covered: [...covered].sort(byCloseoutRef),
    missing: [...missing].sort(byCloseoutRef),
    orphans: [...new Set(orphans)],
  }
}
