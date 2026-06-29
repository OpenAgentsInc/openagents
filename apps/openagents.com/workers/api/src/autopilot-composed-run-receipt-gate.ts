// Autopilot composed-run REAL-BUSINESS-RECEIPT acceptance gate — the pure,
// machine-checkable definition of what a composed-run receipt must satisfy
// before the real-business-receipt blocker on the all-in-one capstone is
// genuinely cleared (EPIC #5510; promises cloud.primitives_suite.v1,
// cloud.agent_cloud_one_stop_revshare.v1, autopilot.all_in_one_business_system.v1
// — all planned).
//
// Episode 239 ("Let's Make Money", docs/transcripts/239.md): a real business
// runs on Autopilot, composed of OpenAgents Cloud primitives bought from ONE
// balance, and the run produces ONE receipt that shows the composed usage
// actually billed (and, where revenue applies, settled). Today the reconciliation
// module (autopilot-composed-run-receipt.ts) builds the receipt SHAPE over an
// INERT execution with `billed`/`settled` hardcoded false. The criteria that
// distinguish that inert shape from a REAL-business receipt — the criteria a
// reviewer must check before flipping the capstone — lived only in prose (module
// comments + the registry verification string). This module turns that prose
// into a TYPED, TESTABLE gate: given evidence about a composed-run receipt, it
// reports, per criterion, whether the evidence satisfies it, and whether the
// whole set would clear the real-business-receipt blocker.
//
// SCOPE / HONESTY: this gate is PURE and DECIDES NOTHING IRREVERSIBLE. It moves
// no money, settles no charge, writes no receipt row, records no owner sign-off,
// and — critically — FLIPS NO PROMISE STATE and DROPS NO BLOCKER. It only
// evaluates whether supplied evidence WOULD meet the bar; acting on a `true`
// result (recording the transition receipt, dropping the blocker, flipping the
// capstone) remains an owner-gated step outside this module. Applied to the
// current inert receipt the gate returns `clearsBlocker: false` and names the
// unmet criteria (components not billed, no owner sign-off, demand provenance not
// external market) — the honest status quo.

import {
  COMPOSED_RUN_RECEIPT_REAL_RECEIPT_MISSING_REF,
  type ComposedRunReceipt,
} from './autopilot-composed-run-receipt'

export const AUTOPILOT_COMPOSED_RUN_RECEIPT_GATE_SCHEMA =
  'openagents.autopilot_composed_run_receipt_gate.v1' as const

/**
 * Where the demand for the composed run came from. Per proof.demand_provenance.v1,
 * internal first-party use is PLUMBING proof, not MARKET proof: only
 * `external_market` demand can back a green capstone claim.
 */
export type RealBusinessReceiptDemandProvenance =
  | 'external_market'
  | 'internal_first_party'
  | 'unknown'

/**
 * The evidence a reviewer (or a future armed run) supplies about a composed-run
 * receipt so the gate can decide whether it is a REAL-business receipt:
 *   - the reconciled receipt SHAPE (from autopilot-composed-run-receipt.ts);
 *   - whether every component charge actually SETTLED against the ledger (a
 *     dereferenceable billed charge, not an inert plan);
 *   - whether revenue applies to this run at all, and if so whether the revshare
 *     receipt has settled;
 *   - the owner sign-off transition-receipt ref per proof.claim_upgrade_receipts.v1
 *     (null when none has been recorded);
 *   - the demand provenance per proof.demand_provenance.v1.
 * Public-safe: refs and booleans only, never amounts beyond the receipt's own
 * fields, idempotency keys, or payment destinations.
 */
export type RealBusinessReceiptEvidence = Readonly<{
  receipt: ComposedRunReceipt
  /** Every component charge settled against the ledger (dereferenceable). */
  componentsBilled: boolean
  /** Whether this run carries revenue that a revshare would settle. */
  revenueApplies: boolean
  /** The revshare receipt has settled (only meaningful when revenueApplies). */
  revshareSettled: boolean
  /** Owner sign-off transition receipt ref per proof.claim_upgrade_receipts.v1. */
  ownerSignoffReceiptRef: string | null
  /** Demand provenance per proof.demand_provenance.v1. */
  demandProvenance: RealBusinessReceiptDemandProvenance
}>

export type RealBusinessReceiptCriterionId =
  | 'composes_at_least_two_primitives'
  | 'one_shared_balance'
  | 'spend_reconciles_to_components'
  | 'components_billed'
  | 'revenue_settled_or_not_applicable'
  | 'owner_signoff_recorded'
  | 'demand_provenance_external'

export type RealBusinessReceiptCriterion = Readonly<{
  id: RealBusinessReceiptCriterionId
  satisfied: boolean
  /** Human-readable reason the criterion is (un)satisfied. No secrets. */
  detail: string
}>

/**
 * The gate decision over one composed-run receipt's evidence. `clearsBlocker` is
 * true ONLY when every criterion is satisfied; even then the gate flips no
 * promise and drops no blocker — `unclearedBlockerRefs` reports what remains
 * open from the gate's perspective, and recording the green flip stays an
 * owner-gated step outside this module.
 */
export type RealBusinessReceiptGateResult = Readonly<{
  schema: typeof AUTOPILOT_COMPOSED_RUN_RECEIPT_GATE_SCHEMA
  runId: string
  /** The single blocker this gate decides on. */
  blockerRef: typeof COMPOSED_RUN_RECEIPT_REAL_RECEIPT_MISSING_REF
  criteria: ReadonlyArray<RealBusinessReceiptCriterion>
  /** True ONLY when every criterion is satisfied. Flips NO promise. */
  clearsBlocker: boolean
  /** The criterion ids not yet satisfied (empty when clearsBlocker). */
  unsatisfiedCriteria: ReadonlyArray<RealBusinessReceiptCriterionId>
  /** Blocker refs still open after this evidence (the gate never drops one). */
  unclearedBlockerRefs: ReadonlyArray<string>
}>

const distinctPrimitiveCount = (receipt: ComposedRunReceipt): number =>
  new Set(receipt.components.map(component => component.primitive)).size

const isNonEmptyRef = (value: string | null): boolean =>
  value !== null && value.trim().length > 0

/**
 * Evaluate whether the supplied evidence makes a composed-run receipt a REAL-
 * business receipt that would clear
 * `blocker.product_promises.autopilot_business_system_real_business_receipt_missing`.
 * PURE. Decides nothing irreversible: flips no promise, drops no blocker, moves
 * no money. Criteria (ALL must hold to clear the blocker):
 *   - composes >= 2 distinct primitives (the all-in-one invariant);
 *   - one shared balance (a single non-empty balance ref);
 *   - the composed spend reconciles to the sum of the component charges;
 *   - every component charge is actually billed (settled against the ledger);
 *   - revenue is settled where it applies (or no revenue applies);
 *   - an owner sign-off transition receipt is recorded
 *     (proof.claim_upgrade_receipts.v1);
 *   - demand provenance is external market, not internal first-party plumbing
 *     (proof.demand_provenance.v1).
 */
export const evaluateRealBusinessReceiptGate = (
  evidence: RealBusinessReceiptEvidence,
): RealBusinessReceiptGateResult => {
  const { receipt } = evidence

  const distinct = distinctPrimitiveCount(receipt)
  const summedChargesMsat = receipt.components.reduce(
    (sum, component) => sum + component.chargeMsat,
    0,
  )

  const criteria: ReadonlyArray<RealBusinessReceiptCriterion> = [
    {
      id: 'composes_at_least_two_primitives',
      satisfied: distinct >= 2,
      detail:
        distinct >= 2
          ? `composes ${distinct} distinct primitives`
          : `composes ${distinct} distinct primitive(s); >= 2 required`,
    },
    {
      id: 'one_shared_balance',
      satisfied: receipt.balanceRef.trim().length > 0,
      detail:
        receipt.balanceRef.trim().length > 0
          ? 'one shared balance ref present'
          : 'no shared balance ref',
    },
    {
      id: 'spend_reconciles_to_components',
      satisfied: summedChargesMsat === receipt.composedSpendMsat,
      detail:
        summedChargesMsat === receipt.composedSpendMsat
          ? 'composed spend reconciles to component charges'
          : `composed spend ${receipt.composedSpendMsat} != sum of components ${summedChargesMsat}`,
    },
    {
      id: 'components_billed',
      satisfied: evidence.componentsBilled,
      detail: evidence.componentsBilled
        ? 'every component charge settled against the ledger'
        : 'no component charge has settled (inert plan only)',
    },
    {
      id: 'revenue_settled_or_not_applicable',
      satisfied: !evidence.revenueApplies || evidence.revshareSettled,
      detail: !evidence.revenueApplies
        ? 'no revenue applies; settlement not required'
        : evidence.revshareSettled
          ? 'revshare receipt settled'
          : 'revenue applies but revshare receipt has not settled',
    },
    {
      id: 'owner_signoff_recorded',
      satisfied: isNonEmptyRef(evidence.ownerSignoffReceiptRef),
      detail: isNonEmptyRef(evidence.ownerSignoffReceiptRef)
        ? 'owner sign-off transition receipt recorded'
        : 'no owner sign-off transition receipt recorded',
    },
    {
      id: 'demand_provenance_external',
      satisfied: evidence.demandProvenance === 'external_market',
      detail:
        evidence.demandProvenance === 'external_market'
          ? 'external market demand provenance'
          : `demand provenance is ${evidence.demandProvenance}; external market required (internal first-party is plumbing proof, not market proof)`,
    },
  ]

  const unsatisfiedCriteria = criteria
    .filter(criterion => !criterion.satisfied)
    .map(criterion => criterion.id)
  const clearsBlocker = unsatisfiedCriteria.length === 0

  return {
    schema: AUTOPILOT_COMPOSED_RUN_RECEIPT_GATE_SCHEMA,
    runId: receipt.runId,
    blockerRef: COMPOSED_RUN_RECEIPT_REAL_RECEIPT_MISSING_REF,
    criteria,
    clearsBlocker,
    unsatisfiedCriteria,
    // The gate never drops a blocker; it only reports whether one is open.
    unclearedBlockerRefs: clearsBlocker
      ? []
      : [COMPOSED_RUN_RECEIPT_REAL_RECEIPT_MISSING_REF],
  }
}

/**
 * Derive the HONEST inert-state evidence for a reconciled receipt SHAPE: nothing
 * is billed, no revenue applies (the inert receipt's referral is disabled and its
 * `settled` is false), no owner sign-off is recorded, and the demand is internal
 * first-party plumbing. Passing this to evaluateRealBusinessReceiptGate yields
 * `clearsBlocker: false` — the status quo, stated machine-checkably.
 */
export const inertReceiptGateEvidence = (
  receipt: ComposedRunReceipt,
): RealBusinessReceiptEvidence => ({
  receipt,
  // The inert receipt's own honest posture: billed/settled are always false.
  componentsBilled: receipt.billed,
  revenueApplies: false,
  revshareSettled: receipt.settled,
  ownerSignoffReceiptRef: null,
  demandProvenance: 'internal_first_party',
})
