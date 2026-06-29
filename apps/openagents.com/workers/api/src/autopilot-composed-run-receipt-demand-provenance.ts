// Autopilot composed-run REAL-BUSINESS-RECEIPT demand-provenance binding — the
// pure derivation that resolves the acceptance gate's `demand_provenance_external`
// criterion from the REAL proof.demand_provenance.v1 surface, instead of a
// reviewer hand-asserting the value (EPIC #5510, child #5519; promises
// cloud.primitives_suite.v1, cloud.agent_cloud_one_stop_revshare.v1,
// autopilot.all_in_one_business_system.v1 — all planned).
//
// The acceptance gate (autopilot-composed-run-receipt-gate.ts) carries a
// `demandProvenance` evidence field typed as a free union
// (external_market | internal_first_party | unknown), and the evidence manifest
// (autopilot-composed-run-receipt-manifest.ts) already names
// `proof.demand_provenance.v1` as the governing ref for that criterion. But
// nothing BOUND the two: a reviewer (or a future armed run) could type
// `external_market` by hand with no link to the actual provenance projection. Per
// proof.demand_provenance.v1's own rule (`no_external_dollar_no_demand_claim`),
// internal first-party use is PLUMBING proof, not MARKET proof — exactly the
// distinction the capstone's real-business-receipt blocker turns on. This module
// closes that gap: it DERIVES the gate-facing demand provenance from the real
// DemandProvenanceProjection so the criterion reads from the governing surface.
//
// SCOPE / HONESTY: this is PURE. It reads an already-built provenance projection
// (totals + the projection's own externalDemandClaimAllowed gate, which encodes
// the no-external-dollar rule) and maps it to the gate's union. It introduces no
// new demand rule — it honors the projection's. It moves no money, settles no
// charge, writes no receipt, records no owner sign-off, FLIPS NO promise state,
// and DROPS NO blocker. With the current internal-only provenance projection the
// derivation returns `internal_first_party` (or `unknown` when no accepted
// outcome is labeled), so the gate's external-demand criterion stays UNSATISFIED
// — the honest status quo, now read from the governing surface rather than
// asserted by hand.

import type { DemandProvenanceProjection } from './demand-provenance'
import type {
  RealBusinessReceiptDemandProvenance,
  RealBusinessReceiptEvidence,
} from './autopilot-composed-run-receipt-gate'

export const AUTOPILOT_COMPOSED_RUN_RECEIPT_DEMAND_PROVENANCE_SCHEMA =
  'openagents.autopilot_composed_run_receipt_demand_provenance.v1' as const

// The single proof primitive this derivation reads — the same governing ref the
// evidence manifest names for the `demand_provenance_external` criterion.
export const COMPOSED_RUN_DEMAND_PROVENANCE_GOVERNING_REF =
  'proof.demand_provenance.v1' as const

/**
 * The narrow, public-safe slice of the proof.demand_provenance.v1 projection this
 * derivation reads: the projection's own rule gate (externalDemandClaimAllowed,
 * encoding `no_external_dollar_no_demand_claim`) and the accepted-outcome totals.
 * Carries no amounts, idempotency keys, or payment destinations. Use
 * `demandProvenanceSignalFromProjection` to lift a full projection into it.
 */
export type DemandProvenanceSignal = Readonly<{
  /** The projection's rule gate: external-demand claim permitted. */
  externalDemandClaimAllowed: boolean
  externalAcceptedOutcomeCount: number
  internalAcceptedOutcomeCount: number
  unlabeledAcceptedOutcomeCount: number
}>

/**
 * Lift a full proof.demand_provenance.v1 projection into the narrow signal the
 * derivation reads. PURE: copies only the public counts + rule gate.
 */
export const demandProvenanceSignalFromProjection = (
  projection: DemandProvenanceProjection,
): DemandProvenanceSignal => ({
  externalDemandClaimAllowed: projection.externalDemandClaimAllowed,
  externalAcceptedOutcomeCount: projection.totals.externalAcceptedOutcomeCount,
  internalAcceptedOutcomeCount: projection.totals.internalAcceptedOutcomeCount,
  unlabeledAcceptedOutcomeCount: projection.totals.unlabeledAcceptedOutcomeCount,
})

/**
 * The gate-facing demand provenance derived from the real
 * proof.demand_provenance.v1 projection, plus the rationale a reviewer can audit.
 * Public-safe: counts + booleans + prose only — the provenance projection itself
 * carries no amounts, idempotency keys, or payment destinations, so neither does
 * this. `satisfiesExternalCriterion` mirrors the gate's `demand_provenance_external`
 * rule (only `external_market` clears it); it introduces no new pass/fail rule.
 */
export type DerivedComposedRunDemandProvenance = Readonly<{
  schema: typeof AUTOPILOT_COMPOSED_RUN_RECEIPT_DEMAND_PROVENANCE_SCHEMA
  /** The governing proof primitive this derivation reads. */
  governingRef: typeof COMPOSED_RUN_DEMAND_PROVENANCE_GOVERNING_REF
  /** The gate-facing demand provenance derived from the projection. */
  provenance: RealBusinessReceiptDemandProvenance
  /** True ONLY when the derived provenance is external_market. */
  satisfiesExternalCriterion: boolean
  /** Accepted-outcome counts the derivation read, for auditability. */
  externalAcceptedOutcomeCount: number
  internalAcceptedOutcomeCount: number
  unlabeledAcceptedOutcomeCount: number
  /** Human-readable reason citing the governing surface. No secrets. */
  detail: string
}>

/**
 * Derive the gate-facing demand provenance from a proof.demand_provenance.v1
 * signal. PURE. Honors the projection's OWN rule
 * (`no_external_dollar_no_demand_claim`, surfaced as externalDemandClaimAllowed):
 *   - external_market: the projection permits the external-demand claim
 *     (externalDemandClaimAllowed === true — i.e. real external accepted-outcome
 *     demand exists under the rule);
 *   - internal_first_party: no external demand is permitted but internal
 *     first-party accepted outcomes exist (plumbing proof, not market proof);
 *   - unknown: no accepted outcome is labeled either way (only unlabeled / none).
 * Introduces no new demand rule and decides nothing irreversible: flips no
 * promise, drops no blocker, moves no money.
 */
export const deriveComposedRunDemandProvenance = (
  signal: DemandProvenanceSignal,
): DerivedComposedRunDemandProvenance => {
  const {
    externalAcceptedOutcomeCount,
    internalAcceptedOutcomeCount,
    unlabeledAcceptedOutcomeCount,
  } = signal

  let provenance: RealBusinessReceiptDemandProvenance
  let detail: string
  if (signal.externalDemandClaimAllowed) {
    provenance = 'external_market'
    detail = `proof.demand_provenance.v1 permits the external-demand claim (${externalAcceptedOutcomeCount} external accepted outcome(s))`
  } else if (internalAcceptedOutcomeCount > 0) {
    provenance = 'internal_first_party'
    detail = `proof.demand_provenance.v1 shows ${internalAcceptedOutcomeCount} internal first-party accepted outcome(s) and no permitted external demand — plumbing proof, not market proof`
  } else {
    provenance = 'unknown'
    detail = `proof.demand_provenance.v1 labels no accepted outcome as external or internal (${unlabeledAcceptedOutcomeCount} unlabeled)`
  }

  return {
    schema: AUTOPILOT_COMPOSED_RUN_RECEIPT_DEMAND_PROVENANCE_SCHEMA,
    governingRef: COMPOSED_RUN_DEMAND_PROVENANCE_GOVERNING_REF,
    provenance,
    satisfiesExternalCriterion: provenance === 'external_market',
    externalAcceptedOutcomeCount,
    internalAcceptedOutcomeCount,
    unlabeledAcceptedOutcomeCount,
    detail,
  }
}

/**
 * Bind a composed-run receipt's gate evidence to the REAL
 * proof.demand_provenance.v1 surface: return the evidence with its
 * `demandProvenance` field set from the surface signal (via
 * deriveComposedRunDemandProvenance) instead of hand-asserted. Lift a live
 * projection with `demandProvenanceSignalFromProjection(projectDemandProvenance())`.
 * PURE: returns a new evidence object and touches nothing else. Decides nothing
 * irreversible: flips no promise, drops no blocker, moves no money. With the
 * current internal-only surface this sets `demandProvenance` to
 * `internal_first_party` (or `unknown`), so the gate's external-demand criterion
 * stays unsatisfied.
 */
export const withDerivedDemandProvenance = (
  evidence: RealBusinessReceiptEvidence,
  signal: DemandProvenanceSignal,
): RealBusinessReceiptEvidence => ({
  ...evidence,
  demandProvenance: deriveComposedRunDemandProvenance(signal).provenance,
})
