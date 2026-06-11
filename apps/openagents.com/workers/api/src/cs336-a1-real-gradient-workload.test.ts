import { describe, expect, it } from 'vitest'

import {
  applyAggregatedSgdStep,
  computeCs336A1ShardGradient,
  computeCs336A1ValidationLoss,
  cs336A1RealGradientRunConfig,
  detExp,
  detLn,
  initialParameters,
  numericShardLossGradient,
  parameterCount,
  parameterStateDigest,
  runCs336A1RealGradientTraining,
  type Cs336A1RealGradientConfig,
} from './cs336-a1-real-gradient-workload'

const tinyConfig: Cs336A1RealGradientConfig = {
  dFf: 6,
  dModel: 4,
  learningRate: 1,
  sequenceLength: 5,
  shardCount: 2,
  stepCount: 2,
  structureBias: 0.85,
  trainSequencesPerShard: 2,
  validationSequenceCount: 2,
  vocabularySize: 7,
}

const seedDigestHex = 'cs336-a1-real-gradient-test-seed'

describe('deterministic transcendentals', () => {
  it('computes exp and ln to f64 accuracy without Math.exp/Math.log', () => {
    for (const x of [-20, -3.7, -1, -0.1, 0, 0.1, 1, 2.5, 10, 50]) {
      expect(Math.abs(detExp(x) - Math.exp(x))).toBeLessThanOrEqual(
        Math.exp(x) * 1e-12,
      )
    }

    for (const x of [1e-6, 0.3, 1, 1.5, 2, 32, 12345.678]) {
      expect(Math.abs(detLn(x) - Math.log(x))).toBeLessThanOrEqual(
        Math.max(1e-12, Math.abs(Math.log(x))) * 1e-12,
      )
    }
  })
})

describe('cs336 a1 real-gradient workload', () => {
  it('matches central finite differences for every parameter tensor (Psionic lane bar)', async () => {
    const params = await initialParameters(tinyConfig, seedDigestHex)
    const analytic = await computeCs336A1ShardGradient({
      config: tinyConfig,
      params,
      seedDigestHex,
      shardIndex: 0,
      stepIndex: 0,
    })
    const numeric = await numericShardLossGradient({
      config: tinyConfig,
      epsilon: 1e-5,
      params,
      seedDigestHex,
      shardIndex: 0,
    })

    expect(params.length).toBe(parameterCount(tinyConfig))

    let worstRelativeError = 0

    for (let index = 0; index < params.length; index += 1) {
      const a = analytic.gradient[index]!
      const n = numeric[index]!
      const relativeError =
        Math.abs(a - n) / Math.max(1e-7, Math.abs(a) + Math.abs(n))

      if (relativeError > worstRelativeError) {
        worstRelativeError = relativeError
      }
    }

    expect(worstRelativeError).toBeLessThan(1e-5)
  })

  it('descends validation loss over aggregated multi-shard SGD steps under the uniform baseline budget', async () => {
    const trajectory = await runCs336A1RealGradientTraining(
      cs336A1RealGradientRunConfig,
      seedDigestHex,
    )
    const losses = [
      trajectory.initialValidationLoss,
      ...trajectory.steps.map(step => step.validationLoss),
    ]

    for (let index = 1; index < losses.length; index += 1) {
      expect(losses[index]!).toBeLessThan(losses[index - 1]!)
    }

    const uniformBaseline = detLn(
      cs336A1RealGradientRunConfig.vocabularySize,
    )

    expect(losses[losses.length - 1]!).toBeLessThan(uniformBaseline)
    expect(trajectory.steps).toHaveLength(
      cs336A1RealGradientRunConfig.stepCount,
    )

    for (const step of trajectory.steps) {
      expect(step.shardResults).toHaveLength(
        cs336A1RealGradientRunConfig.shardCount,
      )

      for (const shard of step.shardResults) {
        expect(shard.gradientL2Norm).toBeGreaterThan(0)
        expect(Number.isFinite(shard.shardLoss)).toBe(true)
      }
    }
  })

  it('verifies deterministic recompute on matching shard gradients and rejects tampered state', async () => {
    const params = await initialParameters(tinyConfig, seedDigestHex)
    const first = await computeCs336A1ShardGradient({
      config: tinyConfig,
      params,
      seedDigestHex,
      shardIndex: 1,
      stepIndex: 0,
    })
    const recompute = await computeCs336A1ShardGradient({
      config: tinyConfig,
      params,
      seedDigestHex,
      shardIndex: 1,
      stepIndex: 0,
    })

    expect(recompute.digestHex).toBe(first.digestHex)
    expect(recompute.shardLoss).toBe(first.shardLoss)

    const tampered = Float64Array.from(params)

    tampered[0] = tampered[0]! + 1e-9

    const tamperedRecompute = await computeCs336A1ShardGradient({
      config: tinyConfig,
      params: tampered,
      seedDigestHex,
      shardIndex: 1,
      stepIndex: 0,
    })

    expect(tamperedRecompute.digestHex).not.toBe(first.digestHex)
  })

  it('pins the aggregated state digest to the exact shard gradient average', async () => {
    const params = await initialParameters(tinyConfig, seedDigestHex)
    const shard0 = await computeCs336A1ShardGradient({
      config: tinyConfig,
      params,
      seedDigestHex,
      shardIndex: 0,
      stepIndex: 0,
    })
    const shard1 = await computeCs336A1ShardGradient({
      config: tinyConfig,
      params,
      seedDigestHex,
      shardIndex: 1,
      stepIndex: 0,
    })
    const next = applyAggregatedSgdStep(tinyConfig, params, [
      shard0.gradient,
      shard1.gradient,
    ])
    const digest = await parameterStateDigest(seedDigestHex, 1, next)
    const trajectory = await runCs336A1RealGradientTraining(
      tinyConfig,
      seedDigestHex,
    )

    expect(trajectory.steps[0]!.aggregatedStateDigestHex).toBe(digest)

    const validation = await computeCs336A1ValidationLoss({
      config: tinyConfig,
      params: next,
      seedDigestHex,
      stepIndex: 1,
    })

    expect(validation.validationLoss).toBe(
      trajectory.steps[0]!.validationLoss,
    )
  })
})
