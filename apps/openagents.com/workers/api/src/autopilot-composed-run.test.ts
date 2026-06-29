import { describe, expect, test } from 'vitest'

import {
  AUTOPILOT_ALL_IN_ONE_PROMISE,
  AUTOPILOT_COMPOSED_RUN_SCHEMA,
  CLOUD_PRIMITIVES_SUITE_PROMISE,
  type ComposedRunComponentInput,
  buildComposedRunPlan,
  composedRunPrimitives,
  emptyComposedRunStore,
  listComposedRuns,
  makeInMemoryComposedRunStore,
  readComposedRun,
} from './autopilot-composed-run'
import { fineTuningJobReceiptRef } from './cloud/fine-tuning-service-routes'
import { sandboxRentalReceiptRef } from './cloud/sandbox-compute-service-routes'
import { inferenceChargeReceiptRef } from './inference/metering-hook'

const inferenceComponent: ComposedRunComponentInput = {
  primitive: 'inference',
  capabilityRef: 'promise:inference.gateway_credits_business.v1',
  componentRunId: 'req-1',
}
const sandboxComponent: ComposedRunComponentInput = {
  primitive: 'sandbox',
  capabilityRef: 'promise:cloud.sandbox_compute_service.v1',
  componentRunId: 'sbx-1',
}
const twoComponents: ReadonlyArray<ComposedRunComponentInput> = [
  inferenceComponent,
  sandboxComponent,
]

const buildOk = (
  overrides: Partial<Parameters<typeof buildComposedRunPlan>[0]> = {},
) => {
  const result = buildComposedRunPlan({
    runId: 'run-1',
    businessRef: 'agent:raynor',
    title: 'All-in-one run',
    summary: 'inference + sandbox on one balance',
    balance: { balanceRef: 'balance:agent:raynor', asset: 'credit' },
    components: twoComponents,
    createdAt: '2026-06-19T00:00:00.000Z',
    ...overrides,
  })
  if (!result.ok) {
    throw new Error(result.error.reason)
  }
  return result.plan
}

describe('autopilot composed-run model (#5519)', () => {
  test('builds a valid plan that is INERT, planned, and pins both promises', () => {
    const plan = buildOk()
    expect(plan.schema).toBe(AUTOPILOT_COMPOSED_RUN_SCHEMA)
    expect(plan.inert).toBe(true)
    expect(plan.promiseState).toBe('planned')
    expect(plan.promiseIds).toEqual([
      AUTOPILOT_ALL_IN_ONE_PROMISE,
      CLOUD_PRIMITIVES_SUITE_PROMISE,
    ])
  })

  test('derives component receipt refs from the owning primitive scaffolds', () => {
    const plan = buildOk()
    const refs = plan.components.map(c => c.componentReceiptRef)
    expect(refs).toContain(inferenceChargeReceiptRef('req-1'))
    expect(refs).toContain(sandboxRentalReceiptRef('sbx-1'))
  })

  test('composes ONE balance and ONE receipt envelope over the components', () => {
    const plan = buildOk()
    // One shared balance.
    expect(plan.balance.balanceRef).toBe('balance:agent:raynor')
    // One envelope referencing every component receipt ref.
    expect(plan.receiptEnvelope.envelopeRef).toBe(
      'receipt.autopilot.composed_run.run-1',
    )
    expect(plan.receiptEnvelope.componentReceiptRefs).toEqual(
      plan.components.map(c => c.componentReceiptRef),
    )
  })

  test('uses a fine-tuning receipt ref when fine_tuning is composed', () => {
    const plan = buildOk({
      components: [
        inferenceComponent,
        {
          primitive: 'fine_tuning',
          capabilityRef: 'promise:cloud.fine_tuning_service.v1',
          componentRunId: 'ft-1',
        },
      ],
    })
    const refs = plan.components.map(c => c.componentReceiptRef)
    expect(refs).toContain(fineTuningJobReceiptRef('ft-1'))
  })

  test('falls back to a neutral namespaced ref for primitives without a helper', () => {
    const plan = buildOk({
      components: [
        inferenceComponent,
        {
          primitive: 'market_labor',
          capabilityRef: 'promise:markets.open_protocol_markets.v1',
          componentRunId: 'job-1',
        },
      ],
    })
    const refs = plan.components.map(c => c.componentReceiptRef)
    expect(refs).toContain(
      'receipt.autopilot.composed_run.component.market_labor.job-1',
    )
  })

  test('records the uncleared blockers (a typed shape clears nothing)', () => {
    const plan = buildOk()
    expect(plan.unclearedBlockerRefs).toEqual([
      'blocker.product_promises.autopilot_business_system_composition_unbuilt',
      'blocker.product_promises.autopilot_business_system_unified_billing_unbuilt',
      'blocker.product_promises.autopilot_business_system_real_business_receipt_missing',
    ])
  })

  test('reports the distinct composed primitives', () => {
    const plan = buildOk()
    expect([...composedRunPrimitives(plan)].sort()).toEqual([
      'inference',
      'sandbox',
    ])
  })

  describe('composition invariant: >= 2 primitives on one balance', () => {
    test('rejects a single-primitive run', () => {
      const result = buildComposedRunPlan({
        runId: 'run-1',
        businessRef: 'agent:raynor',
        title: 'one',
        summary: 'one',
        balance: { balanceRef: 'b', asset: 'credit' },
        components: [inferenceComponent],
      })
      expect(result.ok).toBe(false)
      if (!result.ok) {
        expect(result.error.reason).toContain('at least two primitives')
      }
    })

    test('rejects empty run id / business ref / title / balance ref', () => {
      for (const overrides of [
        { runId: '' },
        { businessRef: '' },
        { title: '' },
        { balance: { balanceRef: '', asset: 'credit' as const } },
      ]) {
        const result = buildComposedRunPlan({
          runId: 'run-1',
          businessRef: 'agent:raynor',
          title: 'ok',
          summary: 'ok',
          balance: { balanceRef: 'b', asset: 'credit' },
          components: twoComponents,
          ...overrides,
        })
        expect(result.ok).toBe(false)
      }
    })

    test('rejects a component with an empty capability ref or run id', () => {
      const emptyCap = buildComposedRunPlan({
        runId: 'run-1',
        businessRef: 'agent:raynor',
        title: 'ok',
        summary: 'ok',
        balance: { balanceRef: 'b', asset: 'credit' },
        components: [
          inferenceComponent,
          { primitive: 'sandbox', capabilityRef: '', componentRunId: 'sbx-1' },
        ],
      })
      expect(emptyCap.ok).toBe(false)

      const emptyRunId = buildComposedRunPlan({
        runId: 'run-1',
        businessRef: 'agent:raynor',
        title: 'ok',
        summary: 'ok',
        balance: { balanceRef: 'b', asset: 'credit' },
        components: [
          inferenceComponent,
          { primitive: 'sandbox', capabilityRef: 'cap', componentRunId: '' },
        ],
      })
      expect(emptyRunId.ok).toBe(false)
    })
  })
})

describe('autopilot composed-run store projection (#5519)', () => {
  test('empty store lists no runs but still reports inert/planned/blockers', () => {
    const projection = listComposedRuns(emptyComposedRunStore)
    expect(projection.runs).toHaveLength(0)
    expect(projection.inert).toBe(true)
    expect(projection.promiseState).toBe('planned')
    expect(projection.unclearedBlockerRefs.length).toBe(3)
    expect(projection.maxStalenessSeconds).toBe(0)
  })

  test('lists runs from a populated store, still inert/planned', () => {
    const store = makeInMemoryComposedRunStore([buildOk()])
    const projection = listComposedRuns(store)
    expect(projection.runs.map(r => r.runId)).toEqual(['run-1'])
    expect(projection.inert).toBe(true)
    expect(projection.promiseState).toBe('planned')
  })

  test('reads one run by id, null when absent', () => {
    const store = makeInMemoryComposedRunStore([buildOk()])
    expect(readComposedRun(store, 'run-1')?.runId).toBe('run-1')
    expect(readComposedRun(store, 'missing')).toBeNull()
  })
})
