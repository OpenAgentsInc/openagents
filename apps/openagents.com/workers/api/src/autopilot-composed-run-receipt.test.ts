import { describe, expect, test } from 'vitest'

import {
  buildComposedRunPlan,
  type ComposedRunComponentInput,
  type ComposedRunPlan,
} from './autopilot-composed-run'
import {
  composeRunExecution,
  type ComposedRunExecution,
  type ComposedRunExecutionComponentInput,
  type ComposedRunReferralInput,
} from './autopilot-composed-run-execution'
import {
  AUTOPILOT_COMPOSED_RUN_RECEIPT_SCHEMA,
  COMPOSED_RUN_RECEIPT_PROMISE_IDS,
  COMPOSED_RUN_RECEIPT_REAL_RECEIPT_MISSING_REF,
  buildComposedRunReceipt,
  composedRunReceiptProjection,
} from './autopilot-composed-run-receipt'
import { cloudChargeReceiptRef } from './cloud/cloud-metering'
import { FINE_TUNING_PRIMITIVE } from './cloud/fine-tuning-service-routes'
import { fineTuningJobReceiptRef } from './cloud/fine-tuning-service-routes'
import { inferenceChargeReceiptRef } from './inference/metering-hook'
import type { ReferredPrincipal } from './referral-cross-category-accrual'

// A D1 stub that THROWS on any IO. composeRunExecution runs the referral bridge
// with `enabled: false`, so it must NEVER touch the ledger; this proves the
// receipt is built over a strictly inert execution.
const explodingDb = new Proxy(
  {},
  {
    get() {
      throw new Error('composed-run receipt must not touch the database (INERT)')
    },
  },
) as unknown as D1Database

const principal: ReferredPrincipal = { kind: 'agent', userId: 'agent-payer' }

const inferenceComponent: ComposedRunComponentInput = {
  primitive: 'inference',
  capabilityRef: 'promise:inference.gateway_credits_business.v1',
  componentRunId: 'req-1',
}
const fineTuningComponent: ComposedRunComponentInput = {
  primitive: 'fine_tuning',
  capabilityRef: 'promise:cloud.fine_tuning_service.v1',
  componentRunId: 'ft-1',
}

const buildPlan = (
  components: ReadonlyArray<ComposedRunComponentInput> = [
    inferenceComponent,
    fineTuningComponent,
  ],
): ComposedRunPlan => {
  const result = buildComposedRunPlan({
    runId: 'run-1',
    businessRef: 'agent:raynor',
    title: 'All-in-one run',
    summary: 'inference + fine-tuning on one balance',
    balance: { balanceRef: 'balance:agent:raynor', asset: 'credit' },
    components,
    createdAt: '2026-06-19T00:00:00.000Z',
  })
  if (!result.ok) {
    throw new Error(result.error.reason)
  }
  return result.plan
}

const referral: ComposedRunReferralInput = {
  eventId: 'evt-1',
  sellerRef: 'agent:raynor',
  referrerRef: 'agent:kerrigan',
  referralBps: 500,
  principal,
}

const buildExecution = async (
  plan: ComposedRunPlan,
  charges: ReadonlyArray<ComposedRunExecutionComponentInput> = [
    { primitive: 'inference', componentRunId: 'req-1', chargeMsat: 1200 },
    { primitive: 'fine_tuning', componentRunId: 'ft-1', chargeMsat: 3400 },
  ],
): Promise<ComposedRunExecution> => {
  const result = await composeRunExecution(explodingDb, {
    plan,
    accountRef: 'account:raynor',
    components: charges,
    referral,
  })
  if (!result.ok) {
    throw new Error(result.error.reason)
  }
  return result.execution
}

describe('autopilot composed-run receipt reconciliation (#5519)', () => {
  test('binds surface <-> settlement refs and is INERT, planned, unbilled', async () => {
    const plan = buildPlan()
    const execution = await buildExecution(plan)
    const result = buildComposedRunReceipt({ plan, execution })
    expect(result.ok).toBe(true)
    if (!result.ok) {
      return
    }
    const receipt = result.receipt
    expect(receipt.schema).toBe(AUTOPILOT_COMPOSED_RUN_RECEIPT_SCHEMA)
    expect(receipt.promiseIds).toEqual(COMPOSED_RUN_RECEIPT_PROMISE_IDS)
    expect(receipt.promiseState).toBe('planned')
    expect(receipt.inert).toBe(true)
    expect(receipt.billed).toBe(false)
    expect(receipt.settled).toBe(false)
    expect(receipt.envelopeRef).toBe('receipt.autopilot.composed_run.run-1')
    expect(receipt.unclearedBlockerRefs).toContain(
      COMPOSED_RUN_RECEIPT_REAL_RECEIPT_MISSING_REF,
    )

    // The receipt binds the SURFACE ref (what the plan advertises) and the
    // SETTLEMENT ref (what the execution settles under) for each component. The
    // cloud primitive surface ref is now ALIGNED to the ledger ref, so the two
    // coincide for fine-tuning (the run is dereferenceable at a single ledger
    // ref) — the binding still asserts BOTH explicitly.
    const fineTuning = receipt.components.find(c => c.primitive === 'fine_tuning')
    expect(fineTuning).toBeDefined()
    expect(fineTuning?.surfaceReceiptRef).toBe(fineTuningJobReceiptRef('ft-1'))
    expect(fineTuning?.settlementReceiptRef).toBe(
      cloudChargeReceiptRef(FINE_TUNING_PRIMITIVE, 'ft-1'),
    )
    expect(fineTuning?.surfaceReceiptRef).toBe(
      fineTuning?.settlementReceiptRef,
    )

    // Inference's surface and settlement refs coincide (one ref shape).
    const inference = receipt.components.find(c => c.primitive === 'inference')
    expect(inference?.surfaceReceiptRef).toBe(inferenceChargeReceiptRef('req-1'))
    expect(inference?.settlementReceiptRef).toBe(
      inferenceChargeReceiptRef('req-1'),
    )
  })

  test('reconciles the one shared-balance debit to the sum of components', async () => {
    const plan = buildPlan()
    const execution = await buildExecution(plan)
    const result = buildComposedRunReceipt({ plan, execution })
    expect(result.ok).toBe(true)
    if (!result.ok) {
      return
    }
    expect(result.receipt.composedSpendMsat).toBe(1200 + 3400)
    expect(result.receipt.balanceRef).toBe('balance:agent:raynor')
    expect(result.receipt.balanceAsset).toBe('credit')
  })

  test('fails when the plan and execution run ids disagree', async () => {
    const plan = buildPlan()
    const execution = await buildExecution(plan)
    const mismatched: ComposedRunExecution = { ...execution, runId: 'run-2' }
    const result = buildComposedRunReceipt({ plan, execution: mismatched })
    expect(result.ok).toBe(false)
    if (result.ok) {
      return
    }
    expect(result.error.reason).toContain('does not match execution runId')
  })

  test('fails when the spend total does not reconcile to its components', async () => {
    const plan = buildPlan()
    const execution = await buildExecution(plan)
    const tampered: ComposedRunExecution = {
      ...execution,
      composedSpendMsat: execution.composedSpendMsat + 1,
    }
    const result = buildComposedRunReceipt({ plan, execution: tampered })
    expect(result.ok).toBe(false)
    if (result.ok) {
      return
    }
    expect(result.error.reason).toContain('does not equal the sum of component')
  })

  test('projection drops amounts/keys and keeps both ref layers + honest posture', async () => {
    const plan = buildPlan()
    const execution = await buildExecution(plan)
    const result = buildComposedRunReceipt({ plan, execution })
    if (!result.ok) {
      throw new Error(result.error.reason)
    }
    const projection = composedRunReceiptProjection(result.receipt)
    const serialized = JSON.stringify(projection)
    expect(serialized).not.toContain('chargeMsat')
    expect(serialized).not.toContain('idempotencyKey')
    expect(projection.billed).toBe(false)
    expect(projection.settled).toBe(false)
    expect(projection.componentReceiptRefs).toHaveLength(2)
    for (const ref of projection.componentReceiptRefs) {
      expect(ref.surfaceReceiptRef.length).toBeGreaterThan(0)
      expect(ref.settlementReceiptRef.length).toBeGreaterThan(0)
    }
  })
})
