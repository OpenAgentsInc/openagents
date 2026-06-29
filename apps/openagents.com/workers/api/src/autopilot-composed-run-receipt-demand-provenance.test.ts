import { describe, expect, test } from 'vitest'

import { projectDemandProvenance } from './demand-provenance'
import {
  evaluateRealBusinessReceiptGate,
  type RealBusinessReceiptEvidence,
} from './autopilot-composed-run-receipt-gate'
import {
  AUTOPILOT_COMPOSED_RUN_RECEIPT_DEMAND_PROVENANCE_SCHEMA,
  COMPOSED_RUN_DEMAND_PROVENANCE_GOVERNING_REF,
  deriveComposedRunDemandProvenance,
  demandProvenanceSignalFromProjection,
  withDerivedDemandProvenance,
  type DemandProvenanceSignal,
} from './autopilot-composed-run-receipt-demand-provenance'
import {
  buildComposedRunReceipt,
  type ComposedRunReceipt,
} from './autopilot-composed-run-receipt'
import {
  buildComposedRunPlan,
  type ComposedRunComponentInput,
} from './autopilot-composed-run'
import {
  composeRunExecution,
  type ComposedRunReferralInput,
} from './autopilot-composed-run-execution'
import type { ReferredPrincipal } from './referral-cross-category-accrual'

// A D1 stub that THROWS on any IO: every helper under test stays strictly inert.
const explodingDb = new Proxy(
  {},
  {
    get() {
      throw new Error('demand-provenance binding must not touch the database (INERT)')
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
  eventId: 'evt-demand-prov',
  sellerRef: 'agent:raynor',
  referrerRef: 'agent:kerrigan',
  referralBps: 500,
  principal,
}

// Build a reconciled inert composed-run receipt to attach evidence to.
const buildInertReceipt = async (): Promise<ComposedRunReceipt> => {
  const planResult = buildComposedRunPlan({
    runId: 'composed-run-demand-prov',
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

  const execResult = await composeRunExecution(explodingDb, {
    plan: planResult.plan,
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

  const built = buildComposedRunReceipt({
    plan: planResult.plan,
    execution: execResult.execution,
  })
  if (!built.ok) {
    throw new Error(`fixture receipt failed to build: ${built.error.reason}`)
  }
  return built.receipt
}

// A narrow demand-provenance signal with explicit accepted-outcome totals, so each
// branch of the derivation is exercised without depending on live counts. Mirrors
// the projection's own rule: externalDemandClaimAllowed iff an external outcome.
const signalWith = (totals: {
  externalAcceptedOutcomeCount: number
  internalAcceptedOutcomeCount: number
  unlabeledAcceptedOutcomeCount: number
}): DemandProvenanceSignal => ({
  externalDemandClaimAllowed: totals.externalAcceptedOutcomeCount > 0,
  ...totals,
})

describe('composed-run real-business-receipt demand-provenance binding', () => {
  test('the live internal-only surface derives non-external — honest status quo', () => {
    const signal = demandProvenanceSignalFromProjection(projectDemandProvenance())
    const derived = deriveComposedRunDemandProvenance(signal)

    expect(derived.schema).toBe(
      AUTOPILOT_COMPOSED_RUN_RECEIPT_DEMAND_PROVENANCE_SCHEMA,
    )
    expect(derived.governingRef).toBe(
      COMPOSED_RUN_DEMAND_PROVENANCE_GOVERNING_REF,
    )
    expect(derived.governingRef).toBe('proof.demand_provenance.v1')
    // Honest status quo: no external dollar => no external-market provenance.
    expect(derived.provenance).not.toBe('external_market')
    expect(derived.satisfiesExternalCriterion).toBe(false)
  })

  test('external accepted-outcome demand derives external_market and satisfies the criterion', () => {
    const derived = deriveComposedRunDemandProvenance(
      signalWith({
        externalAcceptedOutcomeCount: 3,
        internalAcceptedOutcomeCount: 1,
        unlabeledAcceptedOutcomeCount: 0,
      }),
    )

    expect(derived.provenance).toBe('external_market')
    expect(derived.satisfiesExternalCriterion).toBe(true)
    expect(derived.externalAcceptedOutcomeCount).toBe(3)
  })

  test('internal-only demand derives internal_first_party (plumbing, not market)', () => {
    const derived = deriveComposedRunDemandProvenance(
      signalWith({
        externalAcceptedOutcomeCount: 0,
        internalAcceptedOutcomeCount: 4,
        unlabeledAcceptedOutcomeCount: 0,
      }),
    )

    expect(derived.provenance).toBe('internal_first_party')
    expect(derived.satisfiesExternalCriterion).toBe(false)
    expect(derived.detail).toContain('plumbing proof, not market proof')
  })

  test('no labeled demand derives unknown', () => {
    const derived = deriveComposedRunDemandProvenance(
      signalWith({
        externalAcceptedOutcomeCount: 0,
        internalAcceptedOutcomeCount: 0,
        unlabeledAcceptedOutcomeCount: 2,
      }),
    )

    expect(derived.provenance).toBe('unknown')
    expect(derived.satisfiesExternalCriterion).toBe(false)
  })

  test('withDerivedDemandProvenance binds the gate criterion to the real surface', async () => {
    const receipt = await buildInertReceipt()
    const baseEvidence: RealBusinessReceiptEvidence = {
      receipt,
      // Everything else armed, so only demand provenance decides the criterion.
      componentsBilled: true,
      revenueApplies: false,
      revshareSettled: false,
      ownerSignoffReceiptRef: 'receipt.proof.claim_upgrade.composed-run-demand-prov',
      // A hand-asserted lie we are about to overwrite from the real surface.
      demandProvenance: 'external_market',
    }

    // Bound to the live internal-only surface: the lie is corrected.
    const internalBound = withDerivedDemandProvenance(
      baseEvidence,
      demandProvenanceSignalFromProjection(projectDemandProvenance()),
    )
    expect(internalBound.demandProvenance).not.toBe('external_market')
    const internalResult = evaluateRealBusinessReceiptGate(internalBound)
    expect(internalResult.unsatisfiedCriteria).toContain(
      'demand_provenance_external',
    )
    expect(internalResult.clearsBlocker).toBe(false)

    // Bound to a surface with real external demand: the criterion holds.
    const externalBound = withDerivedDemandProvenance(
      baseEvidence,
      signalWith({
        externalAcceptedOutcomeCount: 5,
        internalAcceptedOutcomeCount: 0,
        unlabeledAcceptedOutcomeCount: 0,
      }),
    )
    expect(externalBound.demandProvenance).toBe('external_market')
    const externalResult = evaluateRealBusinessReceiptGate(externalBound)
    expect(externalResult.unsatisfiedCriteria).not.toContain(
      'demand_provenance_external',
    )
  })
})
