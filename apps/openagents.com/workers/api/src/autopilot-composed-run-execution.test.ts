import { describe, expect, test } from 'vitest'

import {
  buildComposedRunPlan,
  type ComposedRunComponentInput,
  type ComposedRunPlan,
} from './autopilot-composed-run'
import {
  AUTOPILOT_COMPOSED_RUN_EXECUTION_SCHEMA,
  COMPOSED_RUN_EXECUTION_PROMISE_IDS,
  composeRunExecution,
  composedComponentCharge,
  composedRunExecutionProjection,
} from './autopilot-composed-run-execution'
import {
  cloudChargeIdempotencyKey,
  cloudChargeReceiptRef,
} from './cloud/cloud-metering'
import { FINE_TUNING_PRIMITIVE } from './cloud/fine-tuning-service-routes'
import { SANDBOX_COMPUTE_PRIMITIVE } from './cloud/sandbox-compute-service-routes'
import {
  inferenceChargeIdempotencyKey,
  inferenceChargeReceiptRef,
} from './inference/metering-hook'
import type { ReferredPrincipal } from './referral-cross-category-accrual'

// A D1 stub that THROWS on any IO. The execution seam runs the referral bridge
// with `enabled: false`, so the bridge must NEVER touch the ledger; if it does,
// these tests fail loudly. (Proves the inert guarantee, not just asserts it.)
const explodingDb = new Proxy(
  {},
  {
    get() {
      throw new Error('execution composition must not touch the database (INERT)')
    },
  },
) as unknown as D1Database

const principal: ReferredPrincipal = { kind: 'agent', userId: 'agent-payer' }

const buildPlan = (
  components: ReadonlyArray<ComposedRunComponentInput>,
): ComposedRunPlan => {
  const result = buildComposedRunPlan({
    runId: 'run-1',
    businessRef: 'agent:raynor',
    title: 'All-in-one run',
    summary: 'inference + sandbox on one balance',
    balance: { balanceRef: 'balance:agent:raynor', asset: 'credit' },
    components,
    createdAt: '2026-06-19T00:00:00.000Z',
  })
  if (!result.ok) {
    throw new Error(result.error.reason)
  }
  return result.plan
}

const inferenceAndSandboxPlan = buildPlan([
  {
    primitive: 'inference',
    capabilityRef: 'promise:inference.gateway_credits_business.v1',
    componentRunId: 'req-1',
  },
  {
    primitive: 'sandbox',
    capabilityRef: 'promise:cloud.sandbox_compute_service.v1',
    componentRunId: 'sbx-1',
  },
])

const baseReferral = {
  eventId: 'composed-run-1',
  sellerRef: 'agent:raynor',
  referrerRef: 'agent:referrer',
  referralBps: 500,
  principal,
} as const

describe('composedComponentCharge: derives each charge from its own primitive helper', () => {
  test('fine-tuning builds a CloudPrimitiveCharge through the merged metering seam', () => {
    const charge = composedComponentCharge({
      primitive: 'fine_tuning',
      componentRunId: 'ft-1',
      accountRef: 'agent:payer',
      chargeMsat: 3000,
      adapterId: 'autopilot.composed_run',
    })
    expect(charge.receiptRef).toBe(
      cloudChargeReceiptRef(FINE_TUNING_PRIMITIVE, 'ft-1'),
    )
    expect(charge.idempotencyKey).toBe(
      cloudChargeIdempotencyKey(FINE_TUNING_PRIMITIVE, 'ft-1'),
    )
    expect(charge.cloudCharge).not.toBeNull()
    expect(charge.cloudCharge?.primitive).toBe(FINE_TUNING_PRIMITIVE)
    expect(charge.cloudCharge?.chargeMsat).toBe(3000)
  })

  test('sandbox builds a CloudPrimitiveCharge through the merged metering seam', () => {
    const charge = composedComponentCharge({
      primitive: 'sandbox',
      componentRunId: 'sbx-1',
      accountRef: 'agent:payer',
      chargeMsat: 1500,
      adapterId: 'autopilot.composed_run',
    })
    expect(charge.receiptRef).toBe(
      cloudChargeReceiptRef(SANDBOX_COMPUTE_PRIMITIVE, 'sbx-1'),
    )
    expect(charge.idempotencyKey).toBe(
      cloudChargeIdempotencyKey(SANDBOX_COMPUTE_PRIMITIVE, 'sbx-1'),
    )
    expect(charge.cloudCharge?.primitive).toBe(SANDBOX_COMPUTE_PRIMITIVE)
  })

  test('inference uses the inference metering hook (no cloud charge plan)', () => {
    const charge = composedComponentCharge({
      primitive: 'inference',
      componentRunId: 'req-1',
      accountRef: 'agent:payer',
      chargeMsat: 800,
      adapterId: 'autopilot.composed_run',
    })
    expect(charge.receiptRef).toBe(inferenceChargeReceiptRef('req-1'))
    expect(charge.idempotencyKey).toBe(inferenceChargeIdempotencyKey('req-1'))
    expect(charge.cloudCharge).toBeNull()
  })
})

describe('composeRunExecution: ONE balance, real charge shapes, referral on composed spend', () => {
  test('composes inference + sandbox into one inert execution', async () => {
    const result = await composeRunExecution(explodingDb, {
      plan: inferenceAndSandboxPlan,
      accountRef: 'agent:payer',
      components: [
        { primitive: 'inference', componentRunId: 'req-1', chargeMsat: 800 },
        { primitive: 'sandbox', componentRunId: 'sbx-1', chargeMsat: 1500 },
      ],
      referral: baseReferral,
    })
    expect(result.ok).toBe(true)
    if (!result.ok) {
      return
    }
    const { execution } = result
    expect(execution.schema).toBe(AUTOPILOT_COMPOSED_RUN_EXECUTION_SCHEMA)
    expect(execution.inert).toBe(true)
    expect(execution.promiseState).toBe('planned')
    expect(execution.promiseIds).toEqual(COMPOSED_RUN_EXECUTION_PROMISE_IDS)
    // ONE shared-balance debit total = sum of the per-component charges.
    expect(execution.composedSpendMsat).toBe(2300)
    expect(execution.balance.balanceRef).toBe('balance:agent:raynor')
    // Each component's receipt ref came from its OWN primitive helper.
    expect(execution.componentCharges.map(c => c.receiptRef)).toEqual([
      inferenceChargeReceiptRef('req-1'),
      cloudChargeReceiptRef(SANDBOX_COMPUTE_PRIMITIVE, 'sbx-1'),
    ])
  })

  test('the referral bridge runs INERT (disabled) on the composed spend', async () => {
    const result = await composeRunExecution(explodingDb, {
      plan: inferenceAndSandboxPlan,
      accountRef: 'agent:payer',
      components: [
        { primitive: 'inference', componentRunId: 'req-1', chargeMsat: 1000 },
        { primitive: 'sandbox', componentRunId: 'sbx-1', chargeMsat: 9000 },
      ],
      referral: baseReferral,
    })
    expect(result.ok).toBe(true)
    if (!result.ok) {
      return
    }
    // Bridge returns its `disabled` tag: plan computed, NO ledger row.
    expect(result.execution.referral._tag).toBe('disabled')
    if (result.execution.referral._tag === 'disabled') {
      // The plan carried the composed spend and the referrer's would-be cut.
      expect(result.execution.referral.plan.meteredSpendMsat).toBe(10000)
      // 5% of 10000 msat = 500 msat (the cut a settled event WOULD accrue).
      expect(result.execution.referral.plan.referralAccrualMsat).toBe(500)
    }
  })

  test('records the uncleared blockers (an inert composition clears nothing)', async () => {
    const result = await composeRunExecution(explodingDb, {
      plan: inferenceAndSandboxPlan,
      accountRef: 'agent:payer',
      components: [
        { primitive: 'inference', componentRunId: 'req-1', chargeMsat: 1 },
        { primitive: 'sandbox', componentRunId: 'sbx-1', chargeMsat: 1 },
      ],
      referral: baseReferral,
    })
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.execution.unclearedBlockerRefs.length).toBe(3)
    }
  })

  test('composes inference + fine-tuning (the other billable mix)', async () => {
    const plan = buildPlan([
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
    ])
    const result = await composeRunExecution(explodingDb, {
      plan,
      accountRef: 'agent:payer',
      components: [
        { primitive: 'inference', componentRunId: 'req-1', chargeMsat: 500 },
        { primitive: 'fine_tuning', componentRunId: 'ft-1', chargeMsat: 4500 },
      ],
      referral: baseReferral,
    })
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.execution.composedSpendMsat).toBe(5000)
    }
  })
})

describe('composedRunExecutionProjection: public-safe, no payment material', () => {
  test('exposes receipt refs but no amounts/keys', async () => {
    const result = await composeRunExecution(explodingDb, {
      plan: inferenceAndSandboxPlan,
      accountRef: 'agent:payer',
      components: [
        { primitive: 'inference', componentRunId: 'req-1', chargeMsat: 800 },
        { primitive: 'sandbox', componentRunId: 'sbx-1', chargeMsat: 1500 },
      ],
      referral: baseReferral,
    })
    expect(result.ok).toBe(true)
    if (!result.ok) {
      return
    }
    const projection = composedRunExecutionProjection(result.execution)
    expect(projection.schema).toBe(AUTOPILOT_COMPOSED_RUN_EXECUTION_SCHEMA)
    expect(projection.inert).toBe(true)
    expect(projection.referralState).toBe('disabled')
    expect(projection.balanceRef).toBe('balance:agent:raynor')
    expect(projection.componentReceiptRefs).toEqual([
      inferenceChargeReceiptRef('req-1'),
      cloudChargeReceiptRef(SANDBOX_COMPUTE_PRIMITIVE, 'sbx-1'),
    ])
    // No amounts / idempotency keys / composedSpendMsat leak into the projection.
    const serialized = JSON.stringify(projection)
    expect(serialized).not.toContain('chargeMsat')
    expect(serialized).not.toContain('idempotencyKey')
    expect(serialized).not.toContain('composedSpendMsat')
  })
})

describe('composeRunExecution: composition + receipt-first invariants', () => {
  test('rejects fewer than two components', async () => {
    const result = await composeRunExecution(explodingDb, {
      plan: inferenceAndSandboxPlan,
      accountRef: 'agent:payer',
      components: [{ primitive: 'inference', componentRunId: 'req-1', chargeMsat: 1 }],
      referral: baseReferral,
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.reason).toContain('at least two primitives')
    }
  })

  test('rejects a charge whose component is not in the plan', async () => {
    const result = await composeRunExecution(explodingDb, {
      plan: inferenceAndSandboxPlan,
      accountRef: 'agent:payer',
      components: [
        { primitive: 'inference', componentRunId: 'req-1', chargeMsat: 1 },
        { primitive: 'sandbox', componentRunId: 'not-in-plan', chargeMsat: 1 },
      ],
      referral: baseReferral,
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.reason).toContain('not in the composed-run plan')
    }
  })

  test('rejects a non-integer / negative charge (receipt-first, never an estimate)', async () => {
    for (const chargeMsat of [-1, 1.5, Number.NaN]) {
      const result = await composeRunExecution(explodingDb, {
        plan: inferenceAndSandboxPlan,
        accountRef: 'agent:payer',
        components: [
          { primitive: 'inference', componentRunId: 'req-1', chargeMsat: 1 },
          { primitive: 'sandbox', componentRunId: 'sbx-1', chargeMsat },
        ],
        referral: baseReferral,
      })
      expect(result.ok).toBe(false)
    }
  })

  test('rejects a run without the inference primitive', async () => {
    const plan = buildPlan([
      {
        primitive: 'fine_tuning',
        capabilityRef: 'promise:cloud.fine_tuning_service.v1',
        componentRunId: 'ft-1',
      },
      {
        primitive: 'sandbox',
        capabilityRef: 'promise:cloud.sandbox_compute_service.v1',
        componentRunId: 'sbx-1',
      },
    ])
    const result = await composeRunExecution(explodingDb, {
      plan,
      accountRef: 'agent:payer',
      components: [
        { primitive: 'fine_tuning', componentRunId: 'ft-1', chargeMsat: 1 },
        { primitive: 'sandbox', componentRunId: 'sbx-1', chargeMsat: 1 },
      ],
      referral: baseReferral,
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.reason).toContain('inference primitive')
    }
  })

  test('rejects inference-only mix without fine-tuning/sandbox', async () => {
    const plan = buildPlan([
      {
        primitive: 'inference',
        capabilityRef: 'promise:inference.gateway_credits_business.v1',
        componentRunId: 'req-1',
      },
      {
        primitive: 'inference',
        capabilityRef: 'promise:inference.gateway_credits_business.v1',
        componentRunId: 'req-2',
      },
    ])
    const result = await composeRunExecution(explodingDb, {
      plan,
      accountRef: 'agent:payer',
      components: [
        { primitive: 'inference', componentRunId: 'req-1', chargeMsat: 1 },
        { primitive: 'inference', componentRunId: 'req-2', chargeMsat: 1 },
      ],
      referral: baseReferral,
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.reason).toContain('fine-tuning or sandbox')
    }
  })

  test('rejects self-referral (referrer must differ from seller)', async () => {
    const result = await composeRunExecution(explodingDb, {
      plan: inferenceAndSandboxPlan,
      accountRef: 'agent:payer',
      components: [
        { primitive: 'inference', componentRunId: 'req-1', chargeMsat: 1 },
        { primitive: 'sandbox', componentRunId: 'sbx-1', chargeMsat: 1 },
      ],
      referral: { ...baseReferral, referrerRef: baseReferral.sellerRef },
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.reason).toContain('self-referral')
    }
  })
})
