import { describe, expect, it } from 'vitest'

import { computeCs336A1TokenizerShard } from './cs336-a1-homework-workload'
import {
  type OverlongShapedRow,
  buildCs336A5OverlongShapedRewards,
  gradeOverlongShapedBatch,
  overlongLengthPenalty,
  overlongResponseTokenLength,
  runCs336A5OverlongPenaltyRewardShaping,
  shapeOverlongReward,
} from './cs336-a5-overlong-penalty-reward-shaping'
import { buildCs336A5Tasks, runCs336A5RolloutBatch } from './cs336-a5-rollout-workload'

describe('CS336 A5 overlong-penalty reference math', () => {
  it('returns zero penalty inside the budget', () => {
    expect(
      overlongLengthPenalty({
        cacheLength: 4,
        maxResponseLength: 20,
        responseLength: 10,
      }),
    ).toBe(0)
    // Exactly at the budget boundary (maxLength - cacheLength) is free.
    expect(
      overlongLengthPenalty({
        cacheLength: 4,
        maxResponseLength: 20,
        responseLength: 16,
      }),
    ).toBe(0)
  })

  it('ramps the penalty linearly from 0 to -1 across the soft buffer', () => {
    // budget = 16, cap = 20, cacheLength = 4.
    expect(
      overlongLengthPenalty({
        cacheLength: 4,
        maxResponseLength: 20,
        responseLength: 18,
      }),
    ).toBeCloseTo(-0.5, 12)
    // Exactly at the hard cap is the full penalty.
    expect(
      overlongLengthPenalty({
        cacheLength: 4,
        maxResponseLength: 20,
        responseLength: 20,
      }),
    ).toBeCloseTo(-1, 12)
  })

  it('returns the full penalty past the hard cap', () => {
    expect(
      overlongLengthPenalty({
        cacheLength: 4,
        maxResponseLength: 20,
        responseLength: 21,
      }),
    ).toBe(-1)
    expect(
      overlongLengthPenalty({
        cacheLength: 4,
        maxResponseLength: 20,
        responseLength: 200,
      }),
    ).toBe(-1)
  })

  it('rejects malformed penalty parameters', () => {
    expect(() =>
      overlongLengthPenalty({
        cacheLength: 0,
        maxResponseLength: 20,
        responseLength: 5,
      }),
    ).toThrow()
    expect(() =>
      overlongLengthPenalty({
        cacheLength: 20,
        maxResponseLength: 20,
        responseLength: 5,
      }),
    ).toThrow()
    expect(() =>
      overlongLengthPenalty({
        cacheLength: 4,
        maxResponseLength: 20,
        responseLength: -1,
      }),
    ).toThrow()
  })

  it('shapes the base reward by adding the length penalty', () => {
    const withinBudget = shapeOverlongReward({
      baseReward: 1,
      cacheLength: 4,
      maxResponseLength: 20,
      responseLength: 10,
    })
    expect(withinBudget.shapedReward).toBe(1)
    expect(withinBudget.withinBudget).toBe(true)
    expect(withinBudget.inSoftZone).toBe(false)
    expect(withinBudget.overLength).toBe(false)

    const softZone = shapeOverlongReward({
      baseReward: 1,
      cacheLength: 4,
      maxResponseLength: 20,
      responseLength: 18,
    })
    expect(softZone.shapedReward).toBeCloseTo(0.5, 12)
    expect(softZone.inSoftZone).toBe(true)

    const overLength = shapeOverlongReward({
      baseReward: 1,
      cacheLength: 4,
      maxResponseLength: 20,
      responseLength: 30,
    })
    expect(overLength.shapedReward).toBe(0)
    expect(overLength.overLength).toBe(true)
  })

  it('rejects a non-exact-match base reward', () => {
    expect(() =>
      shapeOverlongReward({
        baseReward: 0.5,
        cacheLength: 4,
        maxResponseLength: 20,
        responseLength: 5,
      }),
    ).toThrow()
  })

  it('counts a whitespace-token length proxy', () => {
    expect(overlongResponseTokenLength('one two three')).toBe(3)
    expect(overlongResponseTokenLength('  padded   spacing\nhere ')).toBe(3)
    expect(overlongResponseTokenLength('   ')).toBe(0)
  })

  it('aggregates a batch into means and budget-zone counts', () => {
    const rows: ReadonlyArray<OverlongShapedRow> = [
      {
        baseReward: 1,
        penalty: 0,
        responseLength: 5,
        rolloutRef: 'rollout.a',
        shapedReward: 1,
        taskRef: 'task.a',
      },
      {
        baseReward: 1,
        penalty: -0.5,
        responseLength: 18,
        rolloutRef: 'rollout.b',
        shapedReward: 0.5,
        taskRef: 'task.b',
      },
      {
        baseReward: 0,
        penalty: -1,
        responseLength: 40,
        rolloutRef: 'rollout.c',
        shapedReward: -1,
        taskRef: 'task.c',
      },
    ]
    const grade = gradeOverlongShapedBatch({ rows })

    expect(grade.rowCount).toBe(3)
    expect(grade.withinBudgetCount).toBe(1)
    expect(grade.inSoftZoneCount).toBe(1)
    expect(grade.overLengthCount).toBe(1)
    expect(grade.penalizedCount).toBe(2)
    expect(grade.baseRewardMean).toBeCloseTo(2 / 3, 12)
    expect(grade.shapedRewardMean).toBeCloseTo(0.5 / 3, 12)
    expect(grade.penaltyMean).toBeCloseTo(-0.5, 12)
  })

  it('throws on an empty batch', () => {
    expect(() => gradeOverlongShapedBatch({ rows: [] })).toThrow()
  })
})

describe('CS336 A5 overlong-penalty reward-shaping workload', () => {
  it('builds public-safe shaped rows from the seeded rollout set', async () => {
    const shard = await computeCs336A1TokenizerShard()
    const tasks = buildCs336A5Tasks({
      shardDigestHex: shard.digestHex,
      splitRef: 'split_a',
    })
    const batch = await runCs336A5RolloutBatch({ splitRef: 'split_a' })
    const rows = buildCs336A5OverlongShapedRewards({
      rollouts: batch.rollouts,
      splitRef: 'split_a',
      tasks,
    })

    expect(rows.length).toBe(batch.rollouts.length)
    const serialized = JSON.stringify(rows)
    expect(serialized).not.toMatch(/completionText|completion|prompt|answer/i)
    for (const row of rows) {
      expect(row.penalty).toBeLessThanOrEqual(0)
      expect(row.penalty).toBeGreaterThanOrEqual(-1)
      expect(row.shapedReward).toBe(Number((row.baseReward + row.penalty).toFixed(6)))
    }
  })

  it('recomputes the shaping digest deterministically across splits', async () => {
    const first = await runCs336A5OverlongPenaltyRewardShaping({
      splitRef: 'split_a',
    })
    const recomputed = await runCs336A5OverlongPenaltyRewardShaping({
      splitRef: 'split_a',
    })
    const other = await runCs336A5OverlongPenaltyRewardShaping({
      splitRef: 'split_b',
    })

    expect(first.outputDigestHex).toBe(recomputed.outputDigestHex)
    expect(first.outputDigestHex).not.toBe(other.outputDigestHex)
    expect(first.rowCount).toBeGreaterThan(0)
    expect(first.maxResponseLength).toBe(13)
    expect(first.cacheLength).toBe(2)
    // The malformed (overlong) completions take the full penalty, so the
    // shaper does real work over the seeded set.
    expect(first.stats.penalizedCount).toBeGreaterThan(0)
    expect(first.stats.overLengthCount).toBeGreaterThan(0)
    expect(first.stats.withinBudgetCount).toBeGreaterThan(0)
    expect(first.stats.penaltyMeanMicro).toBeLessThan(0)
  })
})
