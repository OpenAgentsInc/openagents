import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type SelfServeFanoutInput,
  type SelfServeFanoutWorkOrderFacts,
  SELF_SERVE_FANOUT_CLEARED_BLOCKER_REF,
  SELF_SERVE_FANOUT_PROMISE,
  SELF_SERVE_FANOUT_SCHEMA,
  SELF_SERVE_FANOUT_WORK_CLASS,
  buildSelfServeFanoutPlan,
  dispatchSelfServeFanout,
  makeInMemorySelfServeFanoutStore,
  readSelfServeFanoutPlan,
  selfServeFanoutPlanId,
} from './self-serve-fanout'
import {
  SelfServeFanoutEndpoint,
  handleSelfServeFanoutApi,
  isSelfServeFanoutEnabled,
} from './self-serve-fanout-routes'

// A work order whose owned capacity is dark (none_available) and whose privacy
// tier is public — so the lane-C gate can authorize a market fanout once the
// customer opts in with a budget cap.
const readyFacts: SelfServeFanoutWorkOrderFacts = {
  placementSource: 'none_available',
  placementAvailabilityState: 'none_available',
  privacyTier: 'public',
  settlementBridgeReady: true,
  marketInventoryReady: true,
  artifactAuthorityReady: true,
  validatorPolicyReady: true,
  missionWorkOrderUnified: true,
  providerTrustTier: 'public_rung1',
}

// A private work order whose placement still has owned capacity available — the
// gate must never authorize a market fanout for it.
const privateAvailableFacts: SelfServeFanoutWorkOrderFacts = {
  ...readyFacts,
  placementSource: 'requester_pylon',
  placementAvailabilityState: 'selected',
  privacyTier: 'private',
}

const optedInInput: SelfServeFanoutInput = {
  workOrderRef: 'wo-1',
  customerRef: 'agent:buyer',
  customerOptIn: true,
  budgetCapSats: 5000,
  title: 'Fan out repo task',
}

describe('buildSelfServeFanoutPlan', () => {
  test('builds a ready self-serve plan with a linked market work-request', () => {
    const result = buildSelfServeFanoutPlan(optedInInput, readyFacts, 'now')
    expect(result.ok).toBe(true)
    if (!result.ok) return
    const plan = result.plan
    expect(plan.schema).toBe(SELF_SERVE_FANOUT_SCHEMA)
    expect(plan.selfServe).toBe(true)
    expect(plan.inert).toBe(true)
    expect(plan.promiseState).toBe('yellow')
    expect(plan.promiseIds).toEqual([SELF_SERVE_FANOUT_PROMISE])
    expect(plan.workClass).toBe(SELF_SERVE_FANOUT_WORK_CLASS)
    expect(plan.planId).toBe(selfServeFanoutPlanId('wo-1'))
    expect(plan.readyForMarket).toBe(true)
    expect(plan.gate.lane).toBe('public_market')
    expect(plan.gate.state).toBe('ready')
    expect(plan.marketWorkRequest).not.toBeNull()
    expect(plan.marketWorkRequest?.workClass).toBe('code_task')
    expect(plan.marketWorkRequest?.budgetSats).toBe(5000)
    expect(plan.marketWorkRequest?.requiredCapabilityRefs).toEqual([
      'capability.pylon.local_claude_agent',
    ])
    expect(plan.marketWorkRequest?.verificationCommandRef).toBe(
      'command.public.pylon.labor.bun_test',
    )
    expect(plan.clearedBlockerRefs).toEqual([
      SELF_SERVE_FANOUT_CLEARED_BLOCKER_REF,
    ])
    expect(plan.unclearedBlockerRefs).toEqual([])
  })

  test('builds a ready non-code plugin work-class plan with per-class verification', () => {
    const result = buildSelfServeFanoutPlan(
      { ...optedInInput, workClass: 'data_labeling' },
      readyFacts,
      'now',
    )
    expect(result.ok).toBe(true)
    if (!result.ok) return
    const plan = result.plan
    expect(plan.workClass).toBe('data_labeling')
    expect(plan.planId).toBe(selfServeFanoutPlanId('wo-1', 'data_labeling'))
    expect(plan.planId).not.toBe(selfServeFanoutPlanId('wo-1'))
    expect(plan.readyForMarket).toBe(true)
    expect(plan.marketWorkRequest).toMatchObject({
      requiredCapabilityRefs: ['capability.market.data_labeling'],
      verificationCommandRef: 'command.public.market.data_labeling.audit',
      workClass: 'data_labeling',
    })
    expect(plan.unclearedBlockerRefs).toEqual([])
  })

  test('rejects an inert plugin work class before market authorization', () => {
    const result = buildSelfServeFanoutPlan(
      { ...optedInInput, workClass: 'research_brief' },
      readyFacts,
      'now',
    )
    expect(result.ok).toBe(false)
    if (result.ok) return
    expect(result.error.reason).toContain('live marketplace work class')
  })

  test('opt-out yields a blocked gate and no market work-request', () => {
    const result = buildSelfServeFanoutPlan(
      { ...optedInInput, customerOptIn: false },
      readyFacts,
      'now',
    )
    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.plan.readyForMarket).toBe(false)
    expect(result.plan.marketWorkRequest).toBeNull()
    expect(result.plan.gate.reasonRefs).toContain('lane_c.customer_opt_in_missing')
  })

  test('a private order with owned capacity never fans out', () => {
    const result = buildSelfServeFanoutPlan(
      optedInInput,
      privateAvailableFacts,
      'now',
    )
    expect(result.ok).toBe(true)
    if (!result.ok) return
    // Owned capacity available => stays on owned lane, not market.
    expect(result.plan.readyForMarket).toBe(false)
    expect(result.plan.marketWorkRequest).toBeNull()
    expect(result.plan.gate.lane).toBe('owned_capacity')
  })

  test('a budget cap exceeded blocks the gate', () => {
    const result = buildSelfServeFanoutPlan(
      { ...optedInInput, budgetCapSats: 0 },
      readyFacts,
      'now',
    )
    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.plan.readyForMarket).toBe(false)
  })

  test('rejects empty work order ref', () => {
    const result = buildSelfServeFanoutPlan(
      { ...optedInInput, workOrderRef: '  ' },
      readyFacts,
    )
    expect(result.ok).toBe(false)
  })

  test('rejects a non-whole budget cap', () => {
    const result = buildSelfServeFanoutPlan(
      { ...optedInInput, budgetCapSats: 1.5 },
      readyFacts,
    )
    expect(result.ok).toBe(false)
  })
})

describe('dispatchSelfServeFanout (FLAG-GATED INERT)', () => {
  const readyPlan = () => {
    const r = buildSelfServeFanoutPlan(optedInInput, readyFacts, 'now')
    if (!r.ok) throw new Error(r.error.reason)
    return r.plan
  }
  const blockedPlan = () => {
    const r = buildSelfServeFanoutPlan(
      { ...optedInInput, customerOptIn: false },
      readyFacts,
      'now',
    )
    if (!r.ok) throw new Error(r.error.reason)
    return r.plan
  }

  test('default (flag off) is disabled — lists nothing', async () => {
    const out = await Effect.runPromise(
      dispatchSelfServeFanout({ enabled: false }, { plan: readyPlan() }),
    )
    expect(out._tag).toBe('disabled')
  })

  test('armed + gate-ready returns the authorized market work-request', async () => {
    const out = await Effect.runPromise(
      dispatchSelfServeFanout({ enabled: true }, { plan: readyPlan() }),
    )
    expect(out._tag).toBe('authorized')
    if (out._tag !== 'authorized') return
    expect(out.marketWorkRequest.workClass).toBe('code_task')
  })

  test('armed + gate-ready returns the authorized non-code market work-request', async () => {
    const r = buildSelfServeFanoutPlan(
      { ...optedInInput, workClass: 'data_labeling' },
      readyFacts,
      'now',
    )
    if (!r.ok) throw new Error(r.error.reason)
    const out = await Effect.runPromise(
      dispatchSelfServeFanout({ enabled: true }, { plan: r.plan }),
    )
    expect(out._tag).toBe('authorized')
    if (out._tag !== 'authorized') return
    expect(out.marketWorkRequest.workClass).toBe('data_labeling')
    expect(out.marketWorkRequest.verificationCommandRef).toBe(
      'command.public.market.data_labeling.audit',
    )
  })

  test('armed but gate-blocked returns blocked with reason refs', async () => {
    const out = await Effect.runPromise(
      dispatchSelfServeFanout({ enabled: true }, { plan: blockedPlan() }),
    )
    expect(out._tag).toBe('blocked')
    if (out._tag !== 'blocked') return
    expect(out.reasonRefs.length).toBeGreaterThan(0)
  })
})

describe('isSelfServeFanoutEnabled', () => {
  test('default off', () => {
    expect(isSelfServeFanoutEnabled(undefined)).toBe(false)
    expect(isSelfServeFanoutEnabled('')).toBe(false)
    expect(isSelfServeFanoutEnabled('off')).toBe(false)
  })
  test('explicit truthy tokens arm it', () => {
    for (const v of ['1', 'true', 'yes', 'on', 'TRUE']) {
      expect(isSelfServeFanoutEnabled(v)).toBe(true)
    }
  })
})

describe('handleSelfServeFanoutApi (read-only)', () => {
  const request = (suffix = '') =>
    new Request(`https://openagents.com${SelfServeFanoutEndpoint}${suffix}`)

  test('inert by default (empty listing, yellow)', async () => {
    const res = await Effect.runPromise(
      handleSelfServeFanoutApi(request(), { enabled: false }),
    )
    const body = (await res.json()) as Record<string, unknown>
    expect(body.inert).toBe(true)
    expect(body.promiseState).toBe('yellow')
    expect(body.selfServe).toBe(true)
    expect(body.workClass).toBe('code_task')
    expect(body.plans).toEqual([])
    expect(body.unclearedBlockerRefs).toEqual([])
  })

  test('armed surface lists injected plans', async () => {
    const r = buildSelfServeFanoutPlan(optedInInput, readyFacts, 'now')
    if (!r.ok) throw new Error(r.error.reason)
    const store = makeInMemorySelfServeFanoutStore([r.plan])
    const res = await Effect.runPromise(
      handleSelfServeFanoutApi(request(), { enabled: true, store }),
    )
    const body = (await res.json()) as Record<string, unknown>
    expect((body.plans as ReadonlyArray<unknown>).length).toBe(1)
    expect(readSelfServeFanoutPlan(store, r.plan.planId)?.planId).toBe(
      r.plan.planId,
    )
  })

  test('?planId= reads a single plan', async () => {
    const r = buildSelfServeFanoutPlan(optedInInput, readyFacts, 'now')
    if (!r.ok) throw new Error(r.error.reason)
    const store = makeInMemorySelfServeFanoutStore([r.plan])
    const res = await Effect.runPromise(
      handleSelfServeFanoutApi(request(`?planId=${r.plan.planId}`), {
        enabled: true,
        store,
      }),
    )
    const body = (await res.json()) as Record<string, unknown>
    expect((body.plan as Record<string, unknown>).planId).toBe(r.plan.planId)
  })

  test('rejects non-GET', async () => {
    const res = await Effect.runPromise(
      handleSelfServeFanoutApi(
        new Request(`https://openagents.com${SelfServeFanoutEndpoint}`, {
          method: 'POST',
        }),
        { enabled: false },
      ),
    )
    expect(res.status).toBe(405)
  })
})
