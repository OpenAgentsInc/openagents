import { describe, expect, it } from 'vitest'

import {
  type DpoPreferencePair,
  buildCs336A5PreferencePairs,
  dpoImplicitReward,
  dpoPairLoss,
  gradeDpoPreferenceBatch,
  runCs336A5DpoPreferenceGrading,
  softplus,
} from './cs336-a5-dpo-preference-workload'
import { buildCs336A5Tasks, runCs336A5RolloutBatch } from './cs336-a5-rollout-workload'
import { computeCs336A1TokenizerShard } from './cs336-a1-homework-workload'

const LN2 = Math.log(2)

describe('CS336 A5 DPO reference math', () => {
  it('computes a numerically stable softplus matching log(1 + exp(z))', () => {
    expect(softplus(0)).toBeCloseTo(LN2, 12)
    expect(softplus(2)).toBeCloseTo(Math.log(1 + Math.exp(2)), 12)
    expect(softplus(-2)).toBeCloseTo(Math.log(1 + Math.exp(-2)), 12)
    // Stable at extremes where naive exp would overflow / underflow.
    expect(softplus(1000)).toBeCloseTo(1000, 6)
    expect(softplus(-1000)).toBeCloseTo(0, 12)
  })

  it('computes the implicit reward as beta * (logpPolicy - logpReference)', () => {
    expect(
      dpoImplicitReward({ beta: 0.1, logpPolicy: -1.5, logpReference: -2 }),
    ).toBeCloseTo(0.05, 12)
  })

  it('gives a zero-margin pair a loss of log(2) and equal rewards', () => {
    const pair: DpoPreferencePair = {
      chosenLogpPolicy: -2,
      chosenLogpReference: -2,
      pairRef: 'pair.zero',
      rejectedLogpPolicy: -2,
      rejectedLogpReference: -2,
      taskRef: 'task.zero',
    }
    const grade = dpoPairLoss({ beta: 0.1, pair })

    expect(grade.rewardMargin).toBeCloseTo(0, 12)
    expect(grade.loss).toBeCloseTo(LN2, 12)
    expect(grade.correctlyRanked).toBe(false)
  })

  it('decreases loss and ranks correctly when the policy prefers chosen', () => {
    const pair: DpoPreferencePair = {
      chosenLogpPolicy: -1,
      chosenLogpReference: -2,
      pairRef: 'pair.preferred',
      rejectedLogpPolicy: -3,
      rejectedLogpReference: -2,
      taskRef: 'task.preferred',
    }
    const grade = dpoPairLoss({ beta: 1, pair })

    // margin = (−1 − (−2)) − (−3 − (−2)) = 1 − (−1) = 2
    expect(grade.rewardMargin).toBeCloseTo(2, 12)
    expect(grade.loss).toBeCloseTo(softplus(-2), 12)
    expect(grade.loss).toBeLessThan(LN2)
    expect(grade.correctlyRanked).toBe(true)
  })

  it('rejects a non-positive beta', () => {
    const pair: DpoPreferencePair = {
      chosenLogpPolicy: -1,
      chosenLogpReference: -2,
      pairRef: 'pair.bad',
      rejectedLogpPolicy: -3,
      rejectedLogpReference: -2,
      taskRef: 'task.bad',
    }

    expect(() => dpoPairLoss({ beta: 0, pair })).toThrow()
  })

  it('aggregates a batch into mean loss and ranking accuracy', () => {
    const pairs: ReadonlyArray<DpoPreferencePair> = [
      {
        chosenLogpPolicy: -1,
        chosenLogpReference: -2,
        pairRef: 'pair.a',
        rejectedLogpPolicy: -3,
        rejectedLogpReference: -2,
        taskRef: 'task.a',
      },
      {
        chosenLogpPolicy: -2,
        chosenLogpReference: -2,
        pairRef: 'pair.b',
        rejectedLogpPolicy: -2,
        rejectedLogpReference: -2,
        taskRef: 'task.b',
      },
    ]
    const batch = gradeDpoPreferenceBatch({ beta: 1, pairs })

    expect(batch.pairCount).toBe(2)
    expect(batch.rankingAccuracy).toBeCloseTo(0.5, 12)
    expect(batch.meanLoss).toBeCloseTo((softplus(-2) + LN2) / 2, 12)
  })

  it('throws on an empty batch', () => {
    expect(() => gradeDpoPreferenceBatch({ beta: 0.1, pairs: [] })).toThrow()
  })
})

describe('CS336 A5 DPO preference workload', () => {
  it('builds public-safe preference pairs from the seeded rollout set', async () => {
    const shard = await computeCs336A1TokenizerShard()
    const tasks = buildCs336A5Tasks({
      shardDigestHex: shard.digestHex,
      splitRef: 'split_a',
    })
    const batch = await runCs336A5RolloutBatch({ splitRef: 'split_a' })
    const pairs = buildCs336A5PreferencePairs({
      rollouts: batch.rollouts,
      splitRef: 'split_a',
      tasks,
    })

    expect(pairs.length).toBeGreaterThan(0)
    const serialized = JSON.stringify(pairs)
    expect(serialized).not.toMatch(/completionText|completion|prompt|answer/i)
    for (const pair of pairs) {
      expect(Number.isFinite(pair.chosenLogpPolicy)).toBe(true)
      expect(Number.isFinite(pair.rejectedLogpReference)).toBe(true)
    }
  })

  it('recomputes the DPO grading digest deterministically across splits', async () => {
    const first = await runCs336A5DpoPreferenceGrading({ splitRef: 'split_a' })
    const recomputed = await runCs336A5DpoPreferenceGrading({
      splitRef: 'split_a',
    })
    const other = await runCs336A5DpoPreferenceGrading({ splitRef: 'split_b' })

    expect(first.outputDigestHex).toBe(recomputed.outputDigestHex)
    expect(first.outputDigestHex).not.toBe(other.outputDigestHex)
    expect(first.pairCount).toBeGreaterThan(0)
    expect(first.beta).toBe(0.1)
    // The synthetic policy is nudged toward chosen, so most pairs rank
    // correctly, but the workload never claims perfect alignment.
    expect(first.stats.rankingAccuracyBp).toBeGreaterThan(5_000)
    expect(first.stats.rankingAccuracyBp).toBeLessThanOrEqual(10_000)
    expect(first.stats.correctlyRankedCount).toBeGreaterThan(0)
  })
})
