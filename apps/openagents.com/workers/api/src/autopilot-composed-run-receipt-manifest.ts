// Autopilot composed-run REAL-BUSINESS-RECEIPT evidence manifest — the typed,
// machine-checkable specification of EXACTLY which dereferenceable artifact a
// real armed composed run must produce to satisfy each acceptance-gate criterion
// (EPIC #5510, child #5519; promises cloud.primitives_suite.v1,
// cloud.agent_cloud_one_stop_revshare.v1, autopilot.all_in_one_business_system.v1
// — all planned).
//
// The acceptance gate (autopilot-composed-run-receipt-gate.ts) DECIDES whether
// supplied evidence clears
// `blocker.product_promises.autopilot_business_system_real_business_receipt_missing`.
// But the gate alone does not say WHERE each piece of evidence comes from or
// which governing primitive/seam produces the dereferenceable artifact behind it.
// A reviewer (or a future armed run) staring at `unsatisfiedCriteria` still has
// to map each criterion back to the concrete artifact it must go produce. This
// module is that map: per gate criterion, the evidence field(s) the gate reads,
// the governing ref (a proof primitive or the real seam/module that emits the
// artifact), the dereferenceable artifact a real run must produce, and a
// human-readable requirement.
//
// The manifest is keyed by the gate's own `RealBusinessReceiptCriterionId` union,
// so TypeScript enforces it stays 1:1 with the gate: you cannot add a gate
// criterion without adding its evidence requirement here, and vice versa.
//
// SCOPE / HONESTY: this is PURE and INERT data + helpers. It moves no money,
// settles no charge, writes no receipt, records no owner sign-off, FLIPS NO
// promise, and DROPS NO blocker. It only DESCRIBES the evidence bar. The
// governing refs that are not existing proof primitives point at the real
// in-repo seam that would emit the artifact — they assert no live product.

import type {
  RealBusinessReceiptCriterionId,
  RealBusinessReceiptEvidence,
  RealBusinessReceiptGateResult,
} from './autopilot-composed-run-receipt-gate'

export const AUTOPILOT_COMPOSED_RUN_RECEIPT_MANIFEST_SCHEMA =
  'openagents.autopilot_composed_run_receipt_manifest.v1' as const

/**
 * One criterion's evidence requirement: the gate criterion it backs, the
 * evidence field(s) the gate reads to evaluate it, the governing ref (an existing
 * proof primitive, or the real in-repo seam/module that emits the dereferenceable
 * artifact), the artifact a REAL armed run must produce, and the human-readable
 * requirement. Public-safe: refs and prose only — no amounts, keys, or
 * destinations.
 */
export type RealBusinessReceiptEvidenceRequirement = Readonly<{
  criterionId: RealBusinessReceiptCriterionId
  /** The evidence field(s) the gate reads to evaluate this criterion. */
  evidenceFields: ReadonlyArray<keyof RealBusinessReceiptEvidence>
  /** Proof primitive ref OR the real seam/module that emits the artifact. */
  governingRef: string
  /** The dereferenceable artifact a real armed run must produce. */
  requiredArtifact: string
  /** What must be true for this criterion to hold. No secrets. */
  requirement: string
}>

/**
 * The evidence manifest, keyed by gate criterion. The `Record<...>` over the
 * gate's criterion-id union makes TypeScript enforce that every gate criterion
 * has exactly one evidence requirement here (and no stray ones).
 */
export const REAL_BUSINESS_RECEIPT_EVIDENCE_MANIFEST: Readonly<
  Record<RealBusinessReceiptCriterionId, RealBusinessReceiptEvidenceRequirement>
> = {
  composes_at_least_two_primitives: {
    criterionId: 'composes_at_least_two_primitives',
    evidenceFields: ['receipt'],
    governingRef: 'apps/openagents.com/workers/api/src/autopilot-composed-run.ts',
    requiredArtifact:
      'a composed-run receipt whose components span >= 2 distinct primitives',
    requirement:
      'the all-in-one invariant: the run composes at least two distinct OpenAgents Cloud primitives, not one primitive standing in for the whole system',
  },
  one_shared_balance: {
    criterionId: 'one_shared_balance',
    evidenceFields: ['receipt'],
    governingRef: 'apps/openagents.com/workers/api/src/autopilot-composed-run.ts',
    requiredArtifact:
      'a non-empty shared balance ref every component charge debits',
    requirement:
      'every composed component debits ONE shared balance (one balance ref), not a separate per-primitive balance',
  },
  spend_reconciles_to_components: {
    criterionId: 'spend_reconciles_to_components',
    evidenceFields: ['receipt'],
    governingRef:
      'apps/openagents.com/workers/api/src/autopilot-composed-run-receipt.ts',
    requiredArtifact:
      'composedSpendMsat equal to the sum of the per-component charge msat',
    requirement:
      'the one shared-balance debit reconciles to the sum of the component charges it composes (no unexplained spend)',
  },
  components_billed: {
    criterionId: 'components_billed',
    evidenceFields: ['componentsBilled'],
    governingRef:
      'apps/openagents.com/workers/api/src/cloud/cloud-metering.ts',
    requiredArtifact:
      'a settled ledger row per component (settleCloudPrimitiveCharge) each component receipt ref dereferences',
    requirement:
      'every component charge actually settled against the credit ledger — a dereferenceable billed charge, not an inert plan',
  },
  revenue_settled_or_not_applicable: {
    criterionId: 'revenue_settled_or_not_applicable',
    evidenceFields: ['revenueApplies', 'revshareSettled'],
    governingRef:
      'apps/openagents.com/workers/api/src/marketplace-monetize-any-layer-accrual.ts',
    requiredArtifact:
      'where revenue applies: a settled revshare receipt under the RL-1 cross-category ledger (else: an explicit no-revenue-applies marker)',
    requirement:
      'where revenue applies the revshare receipt has settled; where no revenue applies, settlement is not required',
  },
  owner_signoff_recorded: {
    criterionId: 'owner_signoff_recorded',
    evidenceFields: ['ownerSignoffReceiptRef'],
    governingRef: 'proof.claim_upgrade_receipts.v1',
    requiredArtifact:
      'an owner sign-off promise-transition receipt ref for the capstone upgrade',
    requirement:
      'an owner sign-off transition receipt is recorded per proof.claim_upgrade_receipts.v1 before any green flip',
  },
  demand_provenance_external: {
    criterionId: 'demand_provenance_external',
    evidenceFields: ['demandProvenance'],
    governingRef: 'proof.demand_provenance.v1',
    requiredArtifact:
      'a demand-provenance record showing external-market demand (not internal first-party plumbing)',
    requirement:
      'demand provenance is external market per proof.demand_provenance.v1 — internal first-party use is plumbing proof, not market proof',
  },
}

/**
 * The evidence requirements as a stable-ordered array (manifest insertion order).
 * PURE.
 */
export const realBusinessReceiptEvidenceRequirements =
  (): ReadonlyArray<RealBusinessReceiptEvidenceRequirement> =>
    Object.values(REAL_BUSINESS_RECEIPT_EVIDENCE_MANIFEST)

/**
 * The evidence requirement for one gate criterion. PURE.
 */
export const requirementForCriterion = (
  criterionId: RealBusinessReceiptCriterionId,
): RealBusinessReceiptEvidenceRequirement =>
  REAL_BUSINESS_RECEIPT_EVIDENCE_MANIFEST[criterionId]

/**
 * Map a gate result's UNSATISFIED criteria to their evidence requirements — the
 * concrete, dereferenceable artifacts a real armed run still has to produce
 * before the blocker clears. Empty when the gate already clears the blocker.
 * PURE: reads the gate result, decides nothing irreversible, flips no promise.
 */
export const unmetEvidenceRequirements = (
  result: RealBusinessReceiptGateResult,
): ReadonlyArray<RealBusinessReceiptEvidenceRequirement> =>
  result.unsatisfiedCriteria.map(criterionId =>
    requirementForCriterion(criterionId),
  )

/**
 * Verify the manifest stays 1:1 with the gate result's criteria: every gate
 * criterion has an evidence requirement and the manifest carries no requirement
 * for an unknown criterion. Returns the discrepancies (empty when aligned). PURE.
 */
export const reconcileManifestWithGate = (
  result: RealBusinessReceiptGateResult,
): Readonly<{
  aligned: boolean
  /** Gate criteria with no manifest entry. */
  missingFromManifest: ReadonlyArray<string>
  /** Manifest entries with no matching gate criterion. */
  unknownInManifest: ReadonlyArray<string>
}> => {
  const gateCriterionIds = new Set<string>(
    result.criteria.map(criterion => criterion.id),
  )
  const manifestCriterionIds = new Set<string>(
    Object.keys(REAL_BUSINESS_RECEIPT_EVIDENCE_MANIFEST),
  )

  const missingFromManifest = [...gateCriterionIds].filter(
    id => !manifestCriterionIds.has(id),
  )
  const unknownInManifest = [...manifestCriterionIds].filter(
    id => !gateCriterionIds.has(id),
  )

  return {
    aligned:
      missingFromManifest.length === 0 && unknownInManifest.length === 0,
    missingFromManifest,
    unknownInManifest,
  }
}
