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
  AUTOPILOT_COMPOSED_RUN_RECEIPT_GATE_SCHEMA,
  evaluateRealBusinessReceiptGate,
  inertReceiptGateEvidence,
  type RealBusinessReceiptEvidence,
} from './autopilot-composed-run-receipt-gate'
import type { ReferredPrincipal } from './referral-cross-category-accrual'

// A D1 stub that THROWS on any IO: the execution under test stays strictly inert.
const explodingDb = new Proxy(
  {},
  {
    get() {
      throw new Error('composed-run receipt gate must not touch the database (INERT)')
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

// Fully-armed evidence over a (still inert-shaped) receipt: every evidence flag
// positive. This proves the gate's clear path WITHOUT flipping any promise — the
// receipt object itself is untouched and still honestly inert.
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

describe('autopilot composed-run real-business-receipt gate (#5519)', () => {
  test('the current inert receipt does NOT clear the blocker', async () => {
    const receipt = await buildReceipt()
    const result = evaluateRealBusinessReceiptGate(
      inertReceiptGateEvidence(receipt),
    )
    expect(result.schema).toBe(AUTOPILOT_COMPOSED_RUN_RECEIPT_GATE_SCHEMA)
    expect(result.runId).toBe('run-1')
    expect(result.clearsBlocker).toBe(false)
    expect(result.unclearedBlockerRefs).toContain(
      COMPOSED_RUN_RECEIPT_REAL_RECEIPT_MISSING_REF,
    )
    // The honest failing criteria for the status quo.
    expect(result.unsatisfiedCriteria).toContain('components_billed')
    expect(result.unsatisfiedCriteria).toContain('owner_signoff_recorded')
    expect(result.unsatisfiedCriteria).toContain('demand_provenance_external')
    // The shape-level criteria already hold for a reconciled receipt.
    expect(result.unsatisfiedCriteria).not.toContain(
      'composes_at_least_two_primitives',
    )
    expect(result.unsatisfiedCriteria).not.toContain(
      'spend_reconciles_to_components',
    )
  })

  test('fully-armed evidence clears the blocker and leaves no open ref', async () => {
    const receipt = await buildReceipt()
    const result = evaluateRealBusinessReceiptGate(armedEvidence(receipt))
    expect(result.clearsBlocker).toBe(true)
    expect(result.unsatisfiedCriteria).toHaveLength(0)
    expect(result.unclearedBlockerRefs).toHaveLength(0)
    expect(result.criteria.every(criterion => criterion.satisfied)).toBe(true)
  })

  test('revenue that applies but has not settled fails the gate', async () => {
    const receipt = await buildReceipt()
    const result = evaluateRealBusinessReceiptGate({
      ...armedEvidence(receipt),
      revenueApplies: true,
      revshareSettled: false,
    })
    expect(result.clearsBlocker).toBe(false)
    expect(result.unsatisfiedCriteria).toEqual([
      'revenue_settled_or_not_applicable',
    ])
  })

  test('revenue that does not apply does not require settlement', async () => {
    const receipt = await buildReceipt()
    const result = evaluateRealBusinessReceiptGate({
      ...armedEvidence(receipt),
      revenueApplies: false,
      revshareSettled: false,
    })
    expect(result.clearsBlocker).toBe(true)
    expect(
      result.criteria.find(
        criterion => criterion.id === 'revenue_settled_or_not_applicable',
      )?.satisfied,
    ).toBe(true)
  })

  test('a receipt composing fewer than two primitives fails the shape criterion', () => {
    const singlePrimitiveReceipt: ComposedRunReceipt = {
      schema: 'openagents.autopilot_composed_run_receipt.v1',
      runId: 'run-solo',
      promiseIds: [
        'cloud.primitives_suite.v1',
        'cloud.agent_cloud_one_stop_revshare.v1',
        'autopilot.all_in_one_business_system.v1',
      ],
      promiseState: 'planned',
      inert: true,
      balanceRef: 'balance:agent:raynor',
      balanceAsset: 'credit',
      envelopeRef: 'receipt.autopilot.composed_run.run-solo',
      components: [
        {
          primitive: 'inference',
          componentRunId: 'req-1',
          surfaceReceiptRef: 'receipt.cloud.inference.req-1',
          settlementReceiptRef: 'receipt.cloud.inference.req-1',
          chargeMsat: 100,
        },
        {
          primitive: 'inference',
          componentRunId: 'req-2',
          surfaceReceiptRef: 'receipt.cloud.inference.req-2',
          settlementReceiptRef: 'receipt.cloud.inference.req-2',
          chargeMsat: 200,
        },
      ],
      composedSpendMsat: 300,
      referralState: 'disabled',
      billed: false,
      settled: false,
      unclearedBlockerRefs: [COMPOSED_RUN_RECEIPT_REAL_RECEIPT_MISSING_REF],
    }
    const result = evaluateRealBusinessReceiptGate({
      ...armedEvidence(singlePrimitiveReceipt),
    })
    expect(result.clearsBlocker).toBe(false)
    expect(result.unsatisfiedCriteria).toEqual([
      'composes_at_least_two_primitives',
    ])
  })
})
