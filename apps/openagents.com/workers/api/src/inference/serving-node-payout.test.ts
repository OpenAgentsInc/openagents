import { describe, expect, test } from 'vitest'

import {
  hostedMdkDirectPayoutDisabledGate,
  projectMdkPayoutModeGate,
} from '../mdk-payout-mode-gate'
import { type ServingReceipt } from './openagents-network-adapter'
import {
  buildServingPayoutPayInPlan,
  computeServingPayoutSplit,
  decideServingNodePayout,
  SERVING_PAYOUT_AMOUNT_NOT_POSITIVE_REF,
  SERVING_PAYOUT_NOT_OWNER_ARMED_REF,
  SERVING_PAYOUT_PARITY_UNVERIFIED_REF,
  servingContributorCutMsat,
  servingPayoutIdempotencyKey,
  servingPayoutStageLegId,
  stageWeight,
} from './serving-node-payout'

// The owner-armed gate fully ready (used only to exercise the armed path; the
// production default is DISABLED). This does NOT dispatch a live payout — it only
// makes the gate's `livePayoutClaimAllowed` true so we can test the armed branch.
const armedGate = () =>
  projectMdkPayoutModeGate({
    hostedFundedKeyVerified: true,
    hostedProgrammaticPayoutsEnabled: true,
    requestedMode: 'hosted_mdk_direct_payout',
  })

// Full RL-3 resale ref chain so the api_inference_gateway_resale lane authorizes
// in the armed-path tests.
const fullResaleRefs = {
  assignmentReceiptRef: 'ref.assignment',
  dispatchRef: 'ref.dispatch',
  meteringReceiptRef: 'ref.metering',
  pricingPolicyRef: 'ref.pricing',
  providerGrantRef: 'ref.grant',
  routePolicyRef: 'ref.route',
  settlementReceiptRef: 'ref.settlement',
  tosBoundaryRef: 'ref.tos',
}

const wholeModel: ServingReceipt = {
  parityMode: 'exact_greedy_parity',
  parityVerified: true,
  servedModel: 'kimi-k2p6',
  sharded: false,
  servingRunRef: 'serve.run.whole',
  stages: [{ layerEnd: 32, layerStart: 0, nodeRef: 'pylon.whole', role: 'stage' }],
}

// A 3-stage shard-WAN run: 12 + 10 + 10 layers across 3 nodes, plus a coordinator.
const sharded: ServingReceipt = {
  parityMode: 'exact_greedy_parity',
  parityVerified: true,
  servedModel: 'glm-5p2',
  sharded: true,
  servingRunRef: 'serve.run.shard',
  stages: [
    { layerEnd: 12, layerStart: 0, nodeRef: 'pylon.a', role: 'stage' },
    { layerEnd: 22, layerStart: 12, nodeRef: 'pylon.b', role: 'stage' },
    { layerEnd: 32, layerStart: 22, nodeRef: 'pylon.c', role: 'stage' },
    { layerEnd: 0, layerStart: 0, nodeRef: 'pylon.coord', role: 'coordinator' },
  ],
}

describe('stageWeight (per-layer-block rule)', () => {
  test('a stage is weighted by its layer-block size', () => {
    expect(stageWeight(sharded.stages[0]!)).toBe(12)
    expect(stageWeight(sharded.stages[1]!)).toBe(10)
  })
  test('coordinator/draft roles take a flat weight', () => {
    expect(stageWeight(sharded.stages[3]!)).toBe(1)
  })
})

describe('computeServingPayoutSplit', () => {
  test('whole-model: one node earns the whole cut', () => {
    const split = computeServingPayoutSplit(wholeModel, 10_000)
    expect(split.totalMsat).toBe(10_000)
    expect(split.shares).toEqual([
      { amountMsat: 10_000, nodeRef: 'pylon.whole', weight: 32 },
    ])
  })

  test('multi-stage: split per layer-block, conserving the cut exactly', () => {
    // weights 12,10,10,1 => total 33. cut 10_000 splits by weight.
    const split = computeServingPayoutSplit(sharded, 10_000)
    expect(split.totalMsat).toBe(10_000)
    expect(split.shares.map(s => s.nodeRef)).toEqual([
      'pylon.a',
      'pylon.b',
      'pylon.c',
      'pylon.coord',
    ])
    // Exact conservation: no dust lost or minted.
    const sum = split.shares.reduce((acc, s) => acc + s.amountMsat, 0)
    expect(sum).toBe(10_000)
    // Higher-weight stage earns proportionally more.
    expect(split.shares[0]!.amountMsat).toBeGreaterThan(split.shares[3]!.amountMsat)
  })

  test('remainder is assigned deterministically to the highest-weight stage', () => {
    // cut 100, weights 12,10,10,1 (total 33): floors 36,30,30,3 = 99, remainder 1
    // -> goes to the highest-weight stage (pylon.a).
    const split = computeServingPayoutSplit(sharded, 100)
    expect(split.totalMsat).toBe(100)
    expect(split.shares[0]!.amountMsat).toBe(37)
    expect(split.shares[1]!.amountMsat).toBe(30)
    expect(split.shares[2]!.amountMsat).toBe(30)
    expect(split.shares[3]!.amountMsat).toBe(3)
  })

  test('non-positive cut or empty stages yields an empty split', () => {
    expect(computeServingPayoutSplit(wholeModel, 0).totalMsat).toBe(0)
    expect(computeServingPayoutSplit(wholeModel, -5).shares).toHaveLength(0)
    expect(
      computeServingPayoutSplit({ ...wholeModel, stages: [] }, 10_000).totalMsat,
    ).toBe(0)
  })
})

describe('servingContributorCutMsat', () => {
  test('applies the published contributor share, rounding down', () => {
    expect(servingContributorCutMsat(10_000, 0.5)).toBe(5_000)
    expect(servingContributorCutMsat(10_001, 0.5)).toBe(5_000)
  })
  test('non-positive margin or share yields zero', () => {
    expect(servingContributorCutMsat(0)).toBe(0)
    expect(servingContributorCutMsat(10_000, 0)).toBe(0)
  })
})

describe('idempotency keys', () => {
  test('whole-run key is stable per serving run', () => {
    expect(servingPayoutIdempotencyKey('serve.run.x')).toBe(
      'serving:payout:serve.run.x',
    )
  })
  test('per-stage leg id is stable per run+node', () => {
    expect(servingPayoutStageLegId('serve.run.x', 'pylon.a')).toBe(
      'serve.run.x:stage:pylon.a',
    )
  })
})

describe('decideServingNodePayout — gates (fail closed)', () => {
  test('default DISABLED owner gate => not armed (no live payout)', () => {
    const decision = decideServingNodePayout({
      contributorCutMsat: 5_000,
      payoutGate: hostedMdkDirectPayoutDisabledGate(),
      receipt: wholeModel,
      revenueAsset: 'bitcoin',
      resaleRefs: fullResaleRefs,
    })
    expect(decision.armed).toBe(false)
    expect(decision.blockerRefs).toContain(SERVING_PAYOUT_NOT_OWNER_ARMED_REF)
    // The split is still computed for inspection.
    expect(decision.split.totalMsat).toBe(5_000)
  })

  test('unverified parity => not armed (pay only against a checkable outcome)', () => {
    const decision = decideServingNodePayout({
      contributorCutMsat: 5_000,
      payoutGate: armedGate(),
      receipt: { ...wholeModel, parityVerified: false },
      revenueAsset: 'bitcoin',
      resaleRefs: fullResaleRefs,
    })
    expect(decision.armed).toBe(false)
    expect(decision.blockerRefs).toContain(SERVING_PAYOUT_PARITY_UNVERIFIED_REF)
  })

  test('RL-3 asset boundary: credit revenue cannot fund a Bitcoin serving share', () => {
    const decision = decideServingNodePayout({
      contributorCutMsat: 5_000,
      payoutGate: armedGate(),
      receipt: wholeModel,
      revenueAsset: 'credit',
      resaleRefs: fullResaleRefs,
    })
    expect(decision.armed).toBe(false)
    expect(
      decision.blockerRefs.some(ref => ref.includes('asset_boundary')),
    ).toBe(true)
  })

  test('zero contributor cut => not armed (amount not positive)', () => {
    const decision = decideServingNodePayout({
      contributorCutMsat: 0,
      payoutGate: armedGate(),
      receipt: wholeModel,
      revenueAsset: 'bitcoin',
      resaleRefs: fullResaleRefs,
    })
    expect(decision.armed).toBe(false)
    expect(decision.blockerRefs).toContain(SERVING_PAYOUT_AMOUNT_NOT_POSITIVE_REF)
  })

  test('all gates pass + owner armed => ARMED, with a clean per-stage split', () => {
    const decision = decideServingNodePayout({
      contributorCutMsat: 10_000,
      payoutGate: armedGate(),
      receipt: sharded,
      revenueAsset: 'bitcoin',
      resaleRefs: fullResaleRefs,
    })
    expect(decision.armed).toBe(true)
    expect(decision.blockerRefs).toHaveLength(0)
    expect(decision.split.totalMsat).toBe(10_000)
    expect(decision.idempotencyKey).toBe('serving:payout:serve.run.shard')
    expect(decision.receiptRef).toBe('receipt.serving.payout.serve.run.shard')
  })
})

describe('buildServingPayoutPayInPlan', () => {
  test('returns undefined for an UNARMED decision (no live payout row)', () => {
    const decision = decideServingNodePayout({
      contributorCutMsat: 5_000,
      payoutGate: hostedMdkDirectPayoutDisabledGate(),
      receipt: wholeModel,
      revenueAsset: 'bitcoin',
      resaleRefs: fullResaleRefs,
    })
    expect(buildServingPayoutPayInPlan(decision, 'house:margin')).toBeUndefined()
  })

  test('armed decision builds a reward pay-in: 1 funding in-leg + 1 out-leg per stage', () => {
    const decision = decideServingNodePayout({
      contributorCutMsat: 10_000,
      payoutGate: armedGate(),
      receipt: sharded,
      revenueAsset: 'bitcoin',
      resaleRefs: fullResaleRefs,
    })
    const plan = buildServingPayoutPayInPlan(decision, 'house:margin')
    expect(plan).toBeDefined()
    if (plan === undefined) return
    expect(plan.payInType).toBe('reward')
    expect(plan.costMsat).toBe(10_000)
    expect(plan.idempotencyKey).toBe('serving:payout:serve.run.shard')
    // One funding 'in' leg covering the cost exactly + one 'out' leg per stage.
    const inLegs = plan.legs.filter(l => l.direction === 'in')
    const outLegs = plan.legs.filter(l => l.direction === 'out')
    expect(inLegs).toHaveLength(1)
    expect(inLegs[0]!.amountMsat).toBe(10_000)
    expect(outLegs).toHaveLength(4)
    // The funding leg covers the cost exactly (PayIn invariant) and out legs sum
    // to the cost (whole margin cut distributed).
    expect(outLegs.reduce((acc, l) => acc + l.amountMsat, 0)).toBe(10_000)
    // Per-stage out legs carry stable, per-node leg ids.
    expect(outLegs.map(l => l.legId)).toContain('serve.run.shard:stage:pylon.a')
  })
})
