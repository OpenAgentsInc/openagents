import { describe, expect, it } from 'vitest'

import { computeCs336A1TokenizerShard } from './cs336-a1-homework-workload'
import {
  buildCs336A5EvalSuiteSummary,
  buildCs336A5Tasks,
  gradeCs336A5Rollouts,
  parseCs336A5FinalValue,
  runCs336A5RewardGrading,
  runCs336A5RolloutBatch,
} from './cs336-a5-rollout-workload'

describe('CS336 A5 rollout and grading workload', () => {
  it('replays seeded rollout batches to the same digest and separates splits', async () => {
    const first = await runCs336A5RolloutBatch({ splitRef: 'split_a' })
    const replayed = await runCs336A5RolloutBatch({ splitRef: 'split_a' })
    const other = await runCs336A5RolloutBatch({ splitRef: 'split_b' })

    expect(first.outputDigestHex).toBe(replayed.outputDigestHex)
    expect(first.outputDigestHex).not.toBe(other.outputDigestHex)
    expect(first.stats.rolloutCount).toBe(128)
    expect(first.stats.taskCount).toBe(32)
    expect(first.rollouts).toHaveLength(128)
  })

  it('recomputes reward grading to the same digest with honest accuracy bounds', async () => {
    const grading = await runCs336A5RewardGrading({ splitRef: 'split_a' })
    const recomputed = await runCs336A5RewardGrading({ splitRef: 'split_a' })

    expect(grading.outputDigestHex).toBe(recomputed.outputDigestHex)
    expect(grading.accuracy).toBeGreaterThan(0)
    expect(grading.accuracy).toBeLessThan(1)
    expect(grading.stats.gradedRolloutCount).toBe(128)
    expect(grading.stats.correctCount).toBeGreaterThan(0)
    expect(grading.stats.unparseableCount).toBeGreaterThanOrEqual(0)
    expect(grading.stats.groupCount).toBe(32)
  })

  it('rejects tampered rollouts with a different grading digest', async () => {
    const shard = await computeCs336A1TokenizerShard()
    const tasks = buildCs336A5Tasks({
      shardDigestHex: shard.digestHex,
      splitRef: 'split_a',
    })
    const batch = await runCs336A5RolloutBatch({ splitRef: 'split_a' })
    const honest = await gradeCs336A5Rollouts({
      rollouts: batch.rollouts,
      splitRef: 'split_a',
      tasks,
    })
    const tampered = await gradeCs336A5Rollouts({
      rollouts: batch.rollouts.map((rollout, index) =>
        index === 0
          ? { ...rollout, completionText: '#### 999999' }
          : rollout,
      ),
      splitRef: 'split_a',
      tasks,
    })

    expect(honest.outputDigestHex).not.toBe(tampered.outputDigestHex)
  })

  it('parses bounded GSM8K-format final values and rejects malformed ones', () => {
    expect(parseCs336A5FinalValue('steps\n#### 42')).toBe(42)
    expect(parseCs336A5FinalValue('steps\n#### 1,204')).toBe(1204)
    expect(parseCs336A5FinalValue('#### 7\nmore\n#### -3')).toBe(-3)
    expect(parseCs336A5FinalValue('the final value is 42')).toBeUndefined()
    expect(parseCs336A5FinalValue('#### not_a_number')).toBeUndefined()
  })

  it('builds eval suite summaries that only count verified gradings as verified', async () => {
    const gradingA = await runCs336A5RewardGrading({ splitRef: 'split_a' })
    const gradingB = await runCs336A5RewardGrading({ splitRef: 'split_b' })
    const summary = buildCs336A5EvalSuiteSummary({
      gradings: [gradingA, gradingB],
      splitRef: 'split.cs336_a5.synthetic_math.bounded_combined.v1',
      verifiedGradingDigests: [gradingA.outputDigestHex],
    })

    expect(summary.taskSetRef).toBe('math')
    expect(summary.metric).toBe('accuracy')
    expect(summary.sampleCount).toBe(256)
    expect(summary.verifiedSampleCount).toBe(128)
    expect(summary.score).toBeGreaterThan(0)
    expect(summary.score).toBeLessThan(1)
  })
})
