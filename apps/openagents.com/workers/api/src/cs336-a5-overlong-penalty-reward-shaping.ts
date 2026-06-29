/**
 * Bounded CS336 A5 overlong-penalty GRPO reward-shaping reference
 * workload.
 *
 * This is the length-aware reward-shaping companion to the
 * group-normalized GRPO rollout/grading workload in
 * `cs336-a5-rollout-workload.ts`. The CS336 A5 / DAPO recipe shapes the
 * base correctness reward with a "soft overlong punishment" so the
 * policy is not rewarded for running past a response budget: responses
 * inside the budget keep their reward, responses inside a soft buffer
 * before the hard cap take a linearly-ramped penalty, and responses past
 * the hard cap take the full penalty.
 *
 * The reference math here — `overlongLengthPenalty`, `shapeOverlongReward`,
 * and `gradeOverlongShapedBatch` — is exact and unit-tested. The
 * workload binding (`buildCs336A5OverlongShapedRewards`,
 * `runCs336A5OverlongPenaltyRewardShaping`) reuses the SAME seeded
 * synthetic math environment and the exact-match `parseCs336A5FinalValue`
 * base reward as the GRPO rollout workload, so every shaped reward is
 * downstream of the same committed corpus seed and an opposite-Pylon
 * re-execution reproduces the `outputDigestHex` bit-for-bit (the
 * `deterministic_recompute` property).
 *
 * IMPORTANT HONESTY BOUNDARY: no hosted LLM is involved. Response length
 * is a deterministic whitespace-token proxy over the bounded synthetic
 * completions; conformance of the real Psionic tokenizer-length parser
 * remains a Psionic ask before this shaper is pointed at real GSM8K
 * rollouts. This module is the verifiable reward-shaping function a paid
 * GRPO reward-grading dispatch would settle against; NO paid dispatch,
 * settlement, or policy-gradient update is performed here, and the update
 * step stays behind the #4669 training boundary. Records expose only
 * numeric rewards, lengths, and refs — never prompts or completions.
 */

import { computeCs336A1TokenizerShard } from './cs336-a1-homework-workload'
import {
  type Cs336A5Rollout,
  type Cs336A5Split,
  type Cs336A5Task,
  buildCs336A5Tasks,
  parseCs336A5FinalValue,
  runCs336A5RolloutBatch,
} from './cs336-a5-rollout-workload'

export class Cs336A5OverlongPenaltyError extends Error {
  readonly _tag = 'Cs336A5OverlongPenaltyError'
}

export const Cs336A5OverlongPenaltyWorkloadRef =
  'workload.cs336_a5.overlong_penalty_reward_shaping.v1'
export const Cs336A5OverlongPenaltyJobKind =
  'cs336_a5_overlong_penalty_reward_shaping'
export const Cs336A5OverlongUpdateBoundaryRef = 'issue.github.openagents.4669'

/**
 * DAPO soft-overlong defaults for the bounded synthetic completions: a
 * hard cap of 13 whitespace tokens with a 2-token soft buffer. Inside
 * the budget (length <= cap - buffer) there is no penalty; well-formed
 * synthetic completions sit at the budget while malformed ones run past
 * the cap and take the full penalty, so the shaper does real work over
 * the seeded set.
 */
export const Cs336A5OverlongMaxResponseTokens = 13
export const Cs336A5OverlongCacheTokens = 2

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

/**
 * Deterministic whitespace-token length proxy for a completion. Real
 * Psionic tokenizer-length conformance stays a Psionic ask; this proxy
 * is exact, stable, and never leaves the public boundary as text.
 */
export const overlongResponseTokenLength = (completionText: string): number => {
  const trimmed = completionText.trim()

  return trimmed.length === 0 ? 0 : trimmed.split(/\s+/).length
}

/**
 * DAPO soft overlong punishment for one response of `responseLength`
 * tokens given a hard `maxResponseLength` cap and a `cacheLength` soft
 * buffer:
 *
 *   - length <= maxLength - cacheLength : 0 (inside budget)
 *   - maxLength - cacheLength < length <= maxLength :
 *       ((maxLength - cacheLength) - length) / cacheLength
 *       (linear ramp from 0 down to -1)
 *   - length > maxLength : -1 (full penalty)
 *
 * The result is always in [-1, 0].
 */
export const overlongLengthPenalty = (
  input: Readonly<{
    cacheLength: number
    maxResponseLength: number
    responseLength: number
  }>,
): number => {
  const { cacheLength, maxResponseLength, responseLength } = input

  if (
    !Number.isInteger(maxResponseLength) ||
    maxResponseLength <= 0 ||
    !Number.isInteger(cacheLength) ||
    cacheLength <= 0 ||
    cacheLength >= maxResponseLength
  ) {
    throw new Cs336A5OverlongPenaltyError(
      'CS336 A5 overlong penalty needs 0 < cacheLength < maxResponseLength as positive integers.',
    )
  }

  if (!Number.isFinite(responseLength) || responseLength < 0) {
    throw new Cs336A5OverlongPenaltyError(
      'CS336 A5 overlong penalty needs a non-negative response length.',
    )
  }

  const budget = maxResponseLength - cacheLength

  if (responseLength <= budget) {
    return 0
  }

  if (responseLength <= maxResponseLength) {
    return (budget - responseLength) / cacheLength
  }

  return -1
}

export type OverlongShapedReward = Readonly<{
  baseReward: number
  inSoftZone: boolean
  overLength: boolean
  penalty: number
  responseLength: number
  shapedReward: number
  withinBudget: boolean
}>

/**
 * Shapes a base correctness reward with the soft overlong punishment:
 * `shapedReward = baseReward + penalty`. The base reward must be the
 * exact-match {0, 1} signal from the grader.
 */
export const shapeOverlongReward = (
  input: Readonly<{
    baseReward: number
    cacheLength: number
    maxResponseLength: number
    responseLength: number
  }>,
): OverlongShapedReward => {
  if (input.baseReward !== 0 && input.baseReward !== 1) {
    throw new Cs336A5OverlongPenaltyError(
      'CS336 A5 overlong shaping expects an exact-match base reward of 0 or 1.',
    )
  }

  const penalty = overlongLengthPenalty({
    cacheLength: input.cacheLength,
    maxResponseLength: input.maxResponseLength,
    responseLength: input.responseLength,
  })
  const budget = input.maxResponseLength - input.cacheLength

  return {
    baseReward: input.baseReward,
    inSoftZone:
      input.responseLength > budget &&
      input.responseLength <= input.maxResponseLength,
    overLength: input.responseLength > input.maxResponseLength,
    penalty,
    responseLength: input.responseLength,
    shapedReward: input.baseReward + penalty,
    withinBudget: input.responseLength <= budget,
  }
}

export type OverlongShapedRow = Readonly<{
  baseReward: number
  penalty: number
  responseLength: number
  rolloutRef: string
  shapedReward: number
  taskRef: string
}>

export type OverlongShapedBatchGrade = Readonly<{
  baseRewardMean: number
  inSoftZoneCount: number
  overLengthCount: number
  penalizedCount: number
  penaltyMean: number
  rowCount: number
  shapedRewardMean: number
  withinBudgetCount: number
}>

/**
 * Aggregates a set of shaped rows into mean base/shaped rewards, mean
 * penalty, and counts of within-budget / soft-zone / over-length rows.
 * Pure function of its inputs.
 */
export const gradeOverlongShapedBatch = (
  input: Readonly<{ rows: ReadonlyArray<OverlongShapedRow> }>,
): OverlongShapedBatchGrade => {
  if (input.rows.length === 0) {
    throw new Cs336A5OverlongPenaltyError(
      'CS336 A5 overlong shaping grade needs at least one row.',
    )
  }

  const budgetCap = 0
  const sums = input.rows.reduce(
    (totals, row) => ({
      baseReward: totals.baseReward + row.baseReward,
      inSoftZone:
        totals.inSoftZone + (row.penalty < budgetCap && row.penalty > -1 ? 1 : 0),
      overLength: totals.overLength + (row.penalty <= -1 ? 1 : 0),
      penalized: totals.penalized + (row.penalty < budgetCap ? 1 : 0),
      penalty: totals.penalty + row.penalty,
      shapedReward: totals.shapedReward + row.shapedReward,
      withinBudget: totals.withinBudget + (row.penalty === budgetCap ? 1 : 0),
    }),
    {
      baseReward: 0,
      inSoftZone: 0,
      overLength: 0,
      penalized: 0,
      penalty: 0,
      shapedReward: 0,
      withinBudget: 0,
    },
  )
  const rowCount = input.rows.length

  return {
    baseRewardMean: sums.baseReward / rowCount,
    inSoftZoneCount: sums.inSoftZone,
    overLengthCount: sums.overLength,
    penalizedCount: sums.penalized,
    penaltyMean: sums.penalty / rowCount,
    rowCount,
    shapedRewardMean: sums.shapedReward / rowCount,
    withinBudgetCount: sums.withinBudget,
  }
}

/**
 * Builds bounded shaped-reward rows from a graded rollout set: each
 * rollout's exact-match base reward is shaped by its whitespace-token
 * length. Rows are sorted by (taskRef, rolloutIndex) for a stable digest
 * and expose only numeric rewards, lengths, and refs.
 */
export const buildCs336A5OverlongShapedRewards = (
  input: Readonly<{
    cacheLength?: number
    maxResponseLength?: number
    rollouts: ReadonlyArray<Cs336A5Rollout>
    splitRef: string
    tasks: ReadonlyArray<Cs336A5Task>
  }>,
): ReadonlyArray<OverlongShapedRow> => {
  const cacheLength = input.cacheLength ?? Cs336A5OverlongCacheTokens
  const maxResponseLength =
    input.maxResponseLength ?? Cs336A5OverlongMaxResponseTokens
  const referenceByTaskRef = new Map(
    input.tasks.map(task => [task.taskRef, task.referenceValue]),
  )
  const round6 = (value: number): number => Number(value.toFixed(6))

  return [...input.rollouts]
    .sort((left, right) =>
      left.taskRef === right.taskRef
        ? left.rolloutIndex - right.rolloutIndex
        : left.taskRef.localeCompare(right.taskRef),
    )
    .map((rollout): OverlongShapedRow => {
      const reference = referenceByTaskRef.get(rollout.taskRef)

      if (reference === undefined) {
        throw new Cs336A5OverlongPenaltyError(
          'CS336 A5 overlong shaping saw a rollout for an unknown task ref.',
        )
      }

      const parsed = parseCs336A5FinalValue(rollout.completionText)
      const baseReward = parsed !== undefined && parsed === reference ? 1 : 0
      const responseLength = overlongResponseTokenLength(rollout.completionText)
      const shaped = shapeOverlongReward({
        baseReward,
        cacheLength,
        maxResponseLength,
        responseLength,
      })

      return {
        baseReward,
        penalty: round6(shaped.penalty),
        responseLength,
        rolloutRef: `rollout.cs336_a5.${input.splitRef}.${rollout.taskRef}.${rollout.rolloutIndex}`,
        shapedReward: round6(shaped.shapedReward),
        taskRef: rollout.taskRef,
      }
    })
}

export type Cs336A5OverlongShapingResult = Readonly<{
  cacheLength: number
  elapsedMs: number
  maxResponseLength: number
  outputDigestHex: string
  rowCount: number
  splitRef: string
  stats: Readonly<Record<string, number>>
  workloadRef: typeof Cs336A5OverlongPenaltyWorkloadRef
}>

/**
 * Runs the overlong-penalty reward-shaping stage end to end:
 * deterministically regenerates the seeded task set and rollout batch for
 * the split, shapes each base reward by response length, and commits the
 * batch grade. Both the environment and the length proxy are pure
 * functions of the committed seed, so an opposite-Pylon re-execution
 * reproduces the output digest exactly — the `deterministic_recompute`
 * property.
 */
export const runCs336A5OverlongPenaltyRewardShaping = async (
  input: Readonly<{
    cacheLength?: number
    maxResponseLength?: number
    rolloutsPerTask?: number
    splitRef: Cs336A5Split
    taskCount?: number
  }>,
): Promise<Cs336A5OverlongShapingResult> => {
  const startedAt = performance.now()
  const cacheLength = input.cacheLength ?? Cs336A5OverlongCacheTokens
  const maxResponseLength =
    input.maxResponseLength ?? Cs336A5OverlongMaxResponseTokens
  const shard = await computeCs336A1TokenizerShard()
  const tasks = buildCs336A5Tasks({
    shardDigestHex: shard.digestHex,
    splitRef: input.splitRef,
    ...(input.taskCount === undefined ? {} : { taskCount: input.taskCount }),
  })
  const batch = await runCs336A5RolloutBatch(input)
  const rows = buildCs336A5OverlongShapedRewards({
    cacheLength,
    maxResponseLength,
    rollouts: batch.rollouts,
    splitRef: input.splitRef,
    tasks,
  })
  const graded = gradeOverlongShapedBatch({ rows })

  const outputDigestHex = await sha256Hex(
    JSON.stringify({
      baseRewardMeanMicro: Math.round(graded.baseRewardMean * 1_000_000),
      cacheLength,
      maxResponseLength,
      overLengthCount: graded.overLengthCount,
      penalizedCount: graded.penalizedCount,
      penaltyMeanMicro: Math.round(graded.penaltyMean * 1_000_000),
      rowCount: graded.rowCount,
      rows,
      shapedRewardMeanMicro: Math.round(graded.shapedRewardMean * 1_000_000),
      splitRef: input.splitRef,
      withinBudgetCount: graded.withinBudgetCount,
      workloadRef: Cs336A5OverlongPenaltyWorkloadRef,
    }),
  )

  return {
    cacheLength,
    elapsedMs: performance.now() - startedAt,
    maxResponseLength,
    outputDigestHex,
    rowCount: graded.rowCount,
    splitRef: input.splitRef,
    stats: {
      baseRewardMeanMicro: Math.round(graded.baseRewardMean * 1_000_000),
      inSoftZoneCount: graded.inSoftZoneCount,
      overLengthCount: graded.overLengthCount,
      penalizedCount: graded.penalizedCount,
      penaltyMeanMicro: Math.round(graded.penaltyMean * 1_000_000),
      rowCount: graded.rowCount,
      shapedRewardMeanMicro: Math.round(graded.shapedRewardMean * 1_000_000),
      withinBudgetCount: graded.withinBudgetCount,
    },
    workloadRef: Cs336A5OverlongPenaltyWorkloadRef,
  }
}
