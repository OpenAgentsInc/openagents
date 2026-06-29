import { describe, expect, test } from 'vitest'

import {
  buildComposedRunPlan,
  type ComposedRunComponentInput,
  type ComposedRunPlan,
} from './autopilot-composed-run'
import {
  composeRunExecution,
  type ComposedRunExecution,
  type ComposedRunReferralInput,
} from './autopilot-composed-run-execution'
import {
  buildComposedRunReceipt,
  type ComposedRunReceipt,
} from './autopilot-composed-run-receipt'
import {
  evaluateRealBusinessReceiptGate,
  inertReceiptGateEvidence,
  type RealBusinessReceiptEvidence,
} from './autopilot-composed-run-receipt-gate'
import {
  REAL_BUSINESS_RECEIPT_EVIDENCE_MANIFEST,
  realBusinessReceiptEvidenceRequirements,
  reconcileManifestWithGate,
  requirementForCriterion,
  unmetEvidenceRequirements,
} from './autopilot-composed-run-receipt-manifest'
import type { ReferredPrincipal } from './referral-cross-category-accrual'

// A D1 stub that THROWS on any IO: the execution under test stays strictly inert.
const explodingDb = new Proxy(
  {},
  {
    get() {
      throw new Error('composed-run receipt manifest must not touch the database (INERT)')
    },
  },
) as unknown as D1Database

const principal: ReferredPrincipal = { kind: 'agent', userId: 'agent-payer' }

const components: ReadonlyArray<ComposedRunComponentInput> = [
  {
    primitive: 'inference',
    capabilityRef: 'promise:inference.gateway_credits_business.v1',
    componentRunId: 'req-1',
  },
  {
    primitive: 'fine_tuning',
    capabilityRef: 'promise:cloud.fine_tuning_service.v1',
    componentRunId: 'ft-1',
  },
]

const referral: ComposedRunReferralInput = {
  eventId: 'evt-1',
  sellerRef: 'agent:raynor',
  referrerRef: 'agent:kerrigan',
  referralBps: 500,
  principal,
}

const buildReceipt = async (): Promise<ComposedRunReceipt> => {
  const planResult = buildComposedRunPlan({
    runId: 'run-1',
    businessRef: 'agent:raynor',
    title: 'All-in-one run',
    summary: 'inference + fine-tuning on one balance',
    balance: { balanceRef: 'balance:agent:raynor', asset: 'credit' },
    components,
    createdAt: '2026-06-20T00:00:00.000Z',
  })
  if (!planResult.ok) {
    throw new Error(planResult.error.reason)
  }
  const plan: ComposedRunPlan = planResult.plan

  const execResult = await composeRunExecution(explodingDb, {
    plan,
    accountRef: 'account:raynor',
    components: [
      { primitive: 'inference', componentRunId: 'req-1', chargeMsat: 1200 },
      { primitive: 'fine_tuning', componentRunId: 'ft-1', chargeMsat: 3400 },
    ],
    referral,
  })
  if (!execResult.ok) {
    throw new Error(execResult.error.reason)
  }
  const execution: ComposedRunExecution = execResult.execution

  const receiptResult = buildComposedRunReceipt({ plan, execution })
  if (!receiptResult.ok) {
    throw new Error(receiptResult.error.reason)
  }
  return receiptResult.receipt
}

const armedEvidence = (
  receipt: ComposedRunReceipt,
): RealBusinessReceiptEvidence => ({
  receipt,
  componentsBilled: true,
  revenueApplies: true,
  revshareSettled: true,
  ownerSignoffReceiptRef: 'receipt.promise_transition.autopilot.run-1',
  demandProvenance: 'external_market',
})

describe('autopilot composed-run real-business-receipt evidence manifest (#5519)', () => {
  test('every manifest entry is keyed by its own criterionId with non-empty refs', () => {
    for (const [key, requirement] of Object.entries(
      REAL_BUSINESS_RECEIPT_EVIDENCE_MANIFEST,
    )) {
      expect(requirement.criterionId).toBe(key)
      expect(requirement.evidenceFields.length).toBeGreaterThan(0)
      expect(requirement.governingRef.trim().length).toBeGreaterThan(0)
      expect(requirement.requiredArtifact.trim().length).toBeGreaterThan(0)
      expect(requirement.requirement.trim().length).toBeGreaterThan(0)
    }
  })

  test('the manifest is 1:1 with the gate criteria', async () => {
    const receipt = await buildReceipt()
    const result = evaluateRealBusinessReceiptGate(armedEvidence(receipt))
    const reconciliation = reconcileManifestWithGate(result)
    expect(reconciliation.aligned).toBe(true)
    expect(reconciliation.missingFromManifest).toHaveLength(0)
    expect(reconciliation.unknownInManifest).toHaveLength(0)
    // Same count both ways.
    expect(realBusinessReceiptEvidenceRequirements()).toHaveLength(
      result.criteria.length,
    )
  })

  test('the inert receipt names exactly the honest unmet artifacts', async () => {
    const receipt = await buildReceipt()
    const result = evaluateRealBusinessReceiptGate(
      inertReceiptGateEvidence(receipt),
    )
    const unmet = unmetEvidenceRequirements(result)
    const unmetIds = unmet.map(requirement => requirement.criterionId)
    expect(unmetIds).toContain('components_billed')
    expect(unmetIds).toContain('owner_signoff_recorded')
    expect(unmetIds).toContain('demand_provenance_external')
    // The shape-level criteria are already satisfied, so no artifact is owed.
    expect(unmetIds).not.toContain('composes_at_least_two_primitives')
    expect(unmetIds).not.toContain('spend_reconciles_to_components')

    // The honest unmet artifacts carry their governing proof primitives.
    expect(requirementForCriterion('owner_signoff_recorded').governingRef).toBe(
      'proof.claim_upgrade_receipts.v1',
    )
    expect(
      requirementForCriterion('demand_provenance_external').governingRef,
    ).toBe('proof.demand_provenance.v1')
  })

  test('fully-armed evidence owes no outstanding artifact', async () => {
    const receipt = await buildReceipt()
    const result = evaluateRealBusinessReceiptGate(armedEvidence(receipt))
    expect(result.clearsBlocker).toBe(true)
    expect(unmetEvidenceRequirements(result)).toHaveLength(0)
  })
})
