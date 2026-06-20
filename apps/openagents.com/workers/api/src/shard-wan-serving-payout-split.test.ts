import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ShardWanLargeModelServingBlocker,
  ShardWanServingPayoutDecision,
  ShardWanShardedRunReceiptSchemaVersion,
  evaluateShardWanServingPayout,
} from './shard-wan-serving-payout-split'

const validReceipt = (overrides: Record<string, unknown> = {}) => ({
  modelArtifactDigest: 'sha256:deadbeef',
  parityMode: 'verified',
  schemaVersion: ShardWanShardedRunReceiptSchemaVersion,
  stages: [
    { gpuResident: true, layerEnd: 12, layerStart: 0, stageIndex: 0 },
    { gpuResident: true, layerEnd: 30, layerStart: 12, stageIndex: 1 },
  ],
  totalLayerCount: 30,
  ...overrides,
})

describe('evaluateShardWanServingPayout', () => {
  test('decodes as the public projection schema and stays owner-armed', () => {
    const decision = evaluateShardWanServingPayout({
      contributorCutSats: 1000,
      receipt: validReceipt(),
    })
    expect(() =>
      S.decodeUnknownSync(ShardWanServingPayoutDecision)(decision),
    ).not.toThrow()
    expect(decision.ownerArmedRequired).toBe(true)
    expect(decision.blockerRef).toBe(ShardWanLargeModelServingBlocker)
  })

  test('splits the contributor cut per layer-block and sums to the cut exactly', () => {
    const decision = evaluateShardWanServingPayout({
      contributorCutSats: 1000,
      receipt: validReceipt(),
    })
    expect(decision.payable).toBe(true)
    expect(decision.payoutGate).toBe('pay_against_verified_parity')
    expect(decision.split).not.toBeNull()
    // stage 0 holds 12/30 layers, stage 1 holds 18/30.
    expect(decision.split?.map((s) => s.payoutSats)).toEqual([400, 600])
    expect(decision.totalSplitSats).toBe(1000)
  })

  test('largest-remainder distribution loses no sats on indivisible cuts', () => {
    const decision = evaluateShardWanServingPayout({
      contributorCutSats: 1001,
      receipt: validReceipt(),
    })
    expect(decision.totalSplitSats).toBe(1001)
    // 1001 * 12/30 = 400.4 -> floor 400; 1001 * 18/30 = 600.6 -> floor 600;
    // 1 leftover sat goes to the larger remainder (stage 1).
    expect(decision.split?.map((s) => s.payoutSats)).toEqual([400, 601])
  })

  test('zero contributor cut yields a zero-sat split, still payable', () => {
    const decision = evaluateShardWanServingPayout({
      contributorCutSats: 0,
      receipt: validReceipt(),
    })
    expect(decision.payable).toBe(true)
    expect(decision.totalSplitSats).toBe(0)
    expect(decision.split?.map((s) => s.payoutSats)).toEqual([0, 0])
  })

  test('parity mismatch is rejected, never payable', () => {
    const decision = evaluateShardWanServingPayout({
      contributorCutSats: 1000,
      receipt: validReceipt({ parityMode: 'mismatch' }),
    })
    expect(decision.receiptValid).toBe(true)
    expect(decision.payable).toBe(false)
    expect(decision.payoutGate).toBe('rejected_parity_mismatch')
    expect(decision.split).toBeNull()
  })

  test('no feasible reference defaults to hold, not pay-against-self-report', () => {
    const decision = evaluateShardWanServingPayout({
      contributorCutSats: 1000,
      receipt: validReceipt({ parityMode: 'no_reference' }),
    })
    expect(decision.payable).toBe(false)
    expect(decision.payoutGate).toBe('flagged_no_reference_default_hold')
    expect(decision.split).toBeNull()
  })

  test('a single stage covering the whole model is not a valid split', () => {
    const decision = evaluateShardWanServingPayout({
      contributorCutSats: 1000,
      receipt: validReceipt({
        stages: [
          { gpuResident: true, layerEnd: 30, layerStart: 0, stageIndex: 0 },
        ],
      }),
    })
    expect(decision.receiptValid).toBe(false)
    expect(decision.payoutGate).toBe('rejected_invalid_receipt')
    expect(decision.validationErrors.length).toBeGreaterThan(0)
  })

  test('a gap in layer coverage is rejected', () => {
    const decision = evaluateShardWanServingPayout({
      contributorCutSats: 1000,
      receipt: validReceipt({
        stages: [
          { gpuResident: true, layerEnd: 10, layerStart: 0, stageIndex: 0 },
          { gpuResident: true, layerEnd: 30, layerStart: 12, stageIndex: 1 },
        ],
      }),
    })
    expect(decision.receiptValid).toBe(false)
    expect(
      decision.validationErrors.some((e) => e.includes('gap in layer coverage')),
    ).toBe(true)
  })

  test('an overlap in layer coverage is rejected', () => {
    const decision = evaluateShardWanServingPayout({
      contributorCutSats: 1000,
      receipt: validReceipt({
        stages: [
          { gpuResident: true, layerEnd: 15, layerStart: 0, stageIndex: 0 },
          { gpuResident: true, layerEnd: 30, layerStart: 12, stageIndex: 1 },
        ],
      }),
    })
    expect(decision.receiptValid).toBe(false)
    expect(
      decision.validationErrors.some((e) => e.includes('overlap')),
    ).toBe(true)
  })

  test('a non-GPU-resident stage (whole-model fallback faking) is rejected', () => {
    const decision = evaluateShardWanServingPayout({
      contributorCutSats: 1000,
      receipt: validReceipt({
        stages: [
          { gpuResident: false, layerEnd: 12, layerStart: 0, stageIndex: 0 },
          { gpuResident: true, layerEnd: 30, layerStart: 12, stageIndex: 1 },
        ],
      }),
    })
    expect(decision.receiptValid).toBe(false)
    expect(
      decision.validationErrors.some((e) => e.includes('GPU-resident')),
    ).toBe(true)
  })

  test('a malformed receipt is rejected without throwing', () => {
    const decision = evaluateShardWanServingPayout({
      contributorCutSats: 1000,
      receipt: { not: 'a receipt' },
    })
    expect(decision.receiptValid).toBe(false)
    expect(decision.payoutGate).toBe('rejected_invalid_receipt')
  })

  test('a negative contributor cut is rejected', () => {
    const decision = evaluateShardWanServingPayout({
      contributorCutSats: -5,
      receipt: validReceipt(),
    })
    expect(decision.receiptValid).toBe(false)
    expect(decision.validationErrors[0]).toContain('non-negative')
  })
})

describe('buildShardWanServingPayoutPayInPlan', () => {
  const { buildShardWanServingPayoutPayInPlan } = require('./shard-wan-serving-payout-split')

  test('builds a PayInPlan when payable and owner armed', () => {
    const decision = evaluateShardWanServingPayout({
      contributorCutSats: 1000,
      receipt: validReceipt(),
    })
    const stageNodeRefs = new Map<number, string>([
      [0, 'node:abc'],
      [1, 'node:def'],
    ])

    const plan = buildShardWanServingPayoutPayInPlan({
      decision,
      houseMarginAccountRef: 'account:margin',
      ownerArmed: true,
      servingRunRef: 'run:123',
      stageNodeRefs,
    })

    expect(plan).toBeDefined()
    expect(plan?.costMsat).toBe(1000000)
    expect(plan?.legs.length).toBe(3) // 1 in, 2 out
    expect(plan?.legs[0]?.direction).toBe('in')
    expect(plan?.legs[0]?.partyRef).toBe('account:margin')
    expect(plan?.legs[0]?.amountMsat).toBe(1000000)

    expect(plan?.legs[1]?.direction).toBe('out')
    expect(plan?.legs[1]?.partyRef).toBe('node:abc')
    expect(plan?.legs[1]?.amountMsat).toBe(400000)

    expect(plan?.legs[2]?.direction).toBe('out')
    expect(plan?.legs[2]?.partyRef).toBe('node:def')
    expect(plan?.legs[2]?.amountMsat).toBe(600000)
  })

  test('returns undefined when not owner armed', () => {
    const decision = evaluateShardWanServingPayout({
      contributorCutSats: 1000,
      receipt: validReceipt(),
    })
    const plan = buildShardWanServingPayoutPayInPlan({
      decision,
      houseMarginAccountRef: 'account:margin',
      ownerArmed: false,
      servingRunRef: 'run:123',
      stageNodeRefs: new Map(),
    })
    expect(plan).toBeUndefined()
  })

  test('returns undefined when not payable', () => {
    const decision = evaluateShardWanServingPayout({
      contributorCutSats: 1000,
      receipt: validReceipt({ parityMode: 'mismatch' }),
    })
    const plan = buildShardWanServingPayoutPayInPlan({
      decision,
      houseMarginAccountRef: 'account:margin',
      ownerArmed: true,
      servingRunRef: 'run:123',
      stageNodeRefs: new Map(),
    })
    expect(plan).toBeUndefined()
  })
})
