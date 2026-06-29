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
  COMPOSED_RUN_RECEIPT_REAL_RECEIPT_MISSING_REF,
  type ComposedRunReceipt,
} from './autopilot-composed-run-receipt'
import {
  evaluateRealBusinessReceiptGate,
  type RealBusinessReceiptEvidence,
} from './autopilot-composed-run-receipt-gate'
import { REAL_BUSINESS_RECEIPT_EVIDENCE_MANIFEST } from './autopilot-composed-run-receipt-manifest'
import {
  AUTOPILOT_COMPOSED_RUN_RECEIPT_READINESS_SCHEMA,
  buildRealBusinessReceiptReadinessReport,
  inertReadinessReport,
} from './autopilot-composed-run-receipt-readiness'
import type { ReferredPrincipal } from './referral-cross-category-accrual'

// A D1 stub that THROWS on any IO: the execution under test stays strictly inert.
const explodingDb = new Proxy(
  {},
  {
    get() {
      throw new Error(
        'composed-run receipt readiness must not touch the database (INERT)',
      )
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

describe('autopilot composed-run real-business-receipt readiness report (#5519)', () => {
  test('has one line per gate criterion, in gate order, each joined to its manifest requirement', async () => {
    const receipt = await buildReceipt()
    const evidence = armedEvidence(receipt)
    const report = buildRealBusinessReceiptReadinessReport(evidence)
    const gate = evaluateRealBusinessReceiptGate(evidence)

    expect(report.schema).toBe(AUTOPILOT_COMPOSED_RUN_RECEIPT_READINESS_SCHEMA)
    expect(report.blockerRef).toBe(COMPOSED_RUN_RECEIPT_REAL_RECEIPT_MISSING_REF)
    expect(report.totalCount).toBe(gate.criteria.length)
    expect(report.lines.map(line => line.criterionId)).toEqual(
      gate.criteria.map(criterion => criterion.id),
    )

    for (const line of report.lines) {
      const manifest = REAL_BUSINESS_RECEIPT_EVIDENCE_MANIFEST[line.criterionId]
      const gateCriterion = gate.criteria.find(
        criterion => criterion.id === line.criterionId,
      )
      expect(gateCriterion).toBeDefined()
      // Gate half.
      expect(line.satisfied).toBe(gateCriterion?.satisfied)
      expect(line.detail).toBe(gateCriterion?.detail)
      // Manifest half.
      expect(line.governingRef).toBe(manifest.governingRef)
      expect(line.requiredArtifact).toBe(manifest.requiredArtifact)
      expect(line.requirement).toBe(manifest.requirement)
      expect(line.evidenceFields).toEqual(manifest.evidenceFields)
    }
  })

  test('the receipt context is public-safe — refs only, no amounts or keys', async () => {
    const receipt = await buildReceipt()
    const report = buildRealBusinessReceiptReadinessReport(armedEvidence(receipt))

    expect(report.receipt.runId).toBe('run-1')
    expect(report.receipt.balanceRef).toBe('balance:agent:raynor')
    expect(report.receipt.inert).toBe(true)
    // No per-component amounts leak into the report's public-safe context.
    const serialized = JSON.stringify(report.receipt)
    expect(serialized).not.toContain('1200')
    expect(serialized).not.toContain('3400')
    expect(serialized).not.toContain('chargeMsat')
  })

  test('fully-armed evidence reports clearsBlocker with no outstanding artifact', async () => {
    const receipt = await buildReceipt()
    const report = buildRealBusinessReceiptReadinessReport(armedEvidence(receipt))

    expect(report.clearsBlocker).toBe(true)
    expect(report.satisfiedCount).toBe(report.totalCount)
    expect(report.outstandingArtifacts).toHaveLength(0)
    expect(report.unclearedBlockerRefs).toHaveLength(0)
  })

  test('the inert report is honest: fails and names the outstanding artifacts', async () => {
    const receipt = await buildReceipt()
    const report = inertReadinessReport(receipt)

    expect(report.clearsBlocker).toBe(false)
    expect(report.satisfiedCount).toBeLessThan(report.totalCount)
    expect(report.unclearedBlockerRefs).toContain(
      COMPOSED_RUN_RECEIPT_REAL_RECEIPT_MISSING_REF,
    )

    const outstandingIds = report.outstandingArtifacts.map(
      artifact => artifact.criterionId,
    )
    // The honest status quo: not billed, no sign-off, demand not external market.
    expect(outstandingIds).toContain('components_billed')
    expect(outstandingIds).toContain('owner_signoff_recorded')
    expect(outstandingIds).toContain('demand_provenance_external')
    // The shape-level criteria already hold, so they owe no artifact.
    expect(outstandingIds).not.toContain('composes_at_least_two_primitives')
    expect(outstandingIds).not.toContain('spend_reconciles_to_components')

    // Outstanding artifacts are exactly the unsatisfied lines.
    const unsatisfiedLineIds = report.lines
      .filter(line => !line.satisfied)
      .map(line => line.criterionId)
    expect(outstandingIds).toEqual(unsatisfiedLineIds)
  })

  test('outstanding artifacts carry their governing proof primitives', async () => {
    const receipt = await buildReceipt()
    const report = inertReadinessReport(receipt)
    const byId = new Map(
      report.outstandingArtifacts.map(artifact => [
        artifact.criterionId,
        artifact,
      ]),
    )

    expect(byId.get('owner_signoff_recorded')?.governingRef).toBe(
      'proof.claim_upgrade_receipts.v1',
    )
    expect(byId.get('demand_provenance_external')?.governingRef).toBe(
      'proof.demand_provenance.v1',
    )
  })
})
