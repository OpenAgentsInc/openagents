/**
 * Bounded CS336 A5 DPO (Direct Preference Optimization) preference-pair
 * reference workload.
 *
 * This is the pairwise-preference analog of the GRPO rollout/grading
 * workload in `cs336-a5-rollout-workload.ts`, mirroring the A5
 * decomposition: instead of group-normalized advantages, it constructs
 * (chosen, rejected) preference pairs from the SAME bounded synthetic
 * math environment and computes the exact DPO reference loss, implicit
 * rewards, and ranking accuracy over those pairs.
 *
 * Provenance reuses the A5 rollout pipeline so every pair is downstream
 * of the same committed corpus seed: tasks and rollouts are regenerated
 * with `buildCs336A5Tasks` / `runCs336A5RolloutBatch`, then graded with
 * the same exact-match `parseCs336A5FinalValue` reward. A correct
 * rollout is the `chosen` response and an incorrect rollout is the
 * `rejected` response for the same task.
 *
 * IMPORTANT HONESTY BOUNDARY: no hosted LLM and no real policy/reference
 * model is involved. The per-response log-probs are SYNTHETIC, derived
 * deterministically from the completion text and the exact-match reward
 * (the policy is nudged toward the chosen response). The value here is
 * the DPO REFERENCE MATH itself — `dpoImplicitReward`, `dpoPairLoss`,
 * and `gradeDpoPreferenceBatch` are exact and unit-tested — plus a
 * `deterministic_recompute` grading digest that an opposite-Pylon
 * re-execution reproduces bit-for-bit. It is the verifiable grading
 * function a paid `cs336_a5_dpo_grading` dispatch would settle against;
 * NO paid dispatch has occurred, so the preference-rollout-work blocker
 * stays open. The policy-gradient / DPO update step also stays behind
 * the #4669 training boundary. Pair records expose only numeric
 * log-probs and refs — never prompts, completions, or model weights.
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

export class Cs336A5DpoPreferenceWorkloadError extends Error {
  readonly _tag = 'Cs336A5DpoPreferenceWorkloadError'
}

export const Cs336A5DpoPreferenceWorkloadRef =
  'workload.cs336_a5.dpo_preference_pair_reference_grading.v1'
export const Cs336A5DpoJobKind = 'cs336_a5_dpo_grading'
export const Cs336A5DpoUpdateBoundaryRef = 'issue.github.openagents.4669'

/** CS336 A5 DPO temperature; the assignment uses beta = 0.1. */
export const Cs336A5DpoDefaultBeta = 0.1

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

const fnv1a32 = (value: string): number => {
  let hash = 0x81_1c_9d_c5

  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 0x01_00_01_93)
  }

  return hash >>> 0
}

/** Stable [0, 1) fraction derived deterministically from a string. */
const hashFraction = (value: string): number => fnv1a32(value) / 4_294_967_296

/**
 * Numerically stable softplus: `log(1 + exp(z))` without overflow.
 * `softplus(z) = max(z, 0) + log1p(exp(-|z|))`.
 */
export const softplus = (z: number): number =>
  Math.max(z, 0) + Math.log1p(Math.exp(-Math.abs(z)))

/**
 * DPO implicit reward for one response: `beta * (logpPolicy - logpRef)`.
 * This is the reward the policy implicitly assigns to a response
 * relative to the reference model under the DPO objective.
 */
export const dpoImplicitReward = (
  input: Readonly<{ beta: number; logpPolicy: number; logpReference: number }>,
): number => input.beta * (input.logpPolicy - input.logpReference)

export type DpoPreferencePair = Readonly<{
  chosenLogpPolicy: number
  chosenLogpReference: number
  pairRef: string
  rejectedLogpPolicy: number
  rejectedLogpReference: number
  taskRef: string
}>

export type DpoPairGrade = Readonly<{
  chosenImplicitReward: number
  correctlyRanked: boolean
  loss: number
  pairRef: string
  rejectedImplicitReward: number
  rewardMargin: number
}>

/**
 * Exact per-pair DPO grade. The reward margin is the difference of
 * implicit rewards `r(chosen) - r(rejected)`; the per-pair loss is
 * `-log(sigmoid(margin)) = softplus(-margin)`. A pair is correctly
 * ranked when the margin is strictly positive (the policy prefers the
 * chosen response over the reference baseline).
 */
export const dpoPairLoss = (
  input: Readonly<{ beta: number; pair: DpoPreferencePair }>,
): DpoPairGrade => {
  if (!Number.isFinite(input.beta) || input.beta <= 0) {
    throw new Cs336A5DpoPreferenceWorkloadError(
      'CS336 A5 DPO beta must be a positive finite number.',
    )
  }

  const chosenImplicitReward = dpoImplicitReward({
    beta: input.beta,
    logpPolicy: input.pair.chosenLogpPolicy,
    logpReference: input.pair.chosenLogpReference,
  })
  const rejectedImplicitReward = dpoImplicitReward({
    beta: input.beta,
    logpPolicy: input.pair.rejectedLogpPolicy,
    logpReference: input.pair.rejectedLogpReference,
  })
  const rewardMargin = chosenImplicitReward - rejectedImplicitReward

  return {
    chosenImplicitReward,
    correctlyRanked: rewardMargin > 0,
    loss: softplus(-rewardMargin),
    pairRef: input.pair.pairRef,
    rejectedImplicitReward,
    rewardMargin,
  }
}

export type DpoBatchGrade = Readonly<{
  chosenRewardMean: number
  grades: ReadonlyArray<DpoPairGrade>
  meanLoss: number
  pairCount: number
  rankingAccuracy: number
  rejectedRewardMean: number
  rewardMarginMean: number
}>

/**
 * Reference-grades a batch of preference pairs under the DPO objective:
 * mean loss, ranking accuracy (fraction of pairs with a positive reward
 * margin), and mean implicit rewards. Pure function of its inputs.
 */
export const gradeDpoPreferenceBatch = (
  input: Readonly<{ beta: number; pairs: ReadonlyArray<DpoPreferencePair> }>,
): DpoBatchGrade => {
  if (input.pairs.length === 0) {
    throw new Cs336A5DpoPreferenceWorkloadError(
      'CS336 A5 DPO grading needs at least one preference pair.',
    )
  }

  const grades = input.pairs.map(pair =>
    dpoPairLoss({ beta: input.beta, pair }),
  )
  const pairCount = grades.length
  const sum = grades.reduce(
    (totals, grade) => ({
      chosenReward: totals.chosenReward + grade.chosenImplicitReward,
      correctlyRanked: totals.correctlyRanked + (grade.correctlyRanked ? 1 : 0),
      loss: totals.loss + grade.loss,
      margin: totals.margin + grade.rewardMargin,
      rejectedReward: totals.rejectedReward + grade.rejectedImplicitReward,
    }),
    {
      chosenReward: 0,
      correctlyRanked: 0,
      loss: 0,
      margin: 0,
      rejectedReward: 0,
    },
  )

  return {
    chosenRewardMean: sum.chosenReward / pairCount,
    grades,
    meanLoss: sum.loss / pairCount,
    pairCount,
    rankingAccuracy: sum.correctlyRanked / pairCount,
    rejectedRewardMean: sum.rejectedReward / pairCount,
    rewardMarginMean: sum.margin / pairCount,
  }
}

/**
 * Synthetic, deterministic per-response log-probs. The reference
 * (SFT-baseline) log-prob is a stable function of the completion text;
 * the policy log-prob adds a deterministic correctness-aligned shaping
 * term so a correct (chosen) response is nudged above the reference and
 * an incorrect (rejected) response below it. No real model is queried.
 */
const syntheticLogProbs = (
  input: Readonly<{ completionText: string; reward: number; taskRef: string }>,
): Readonly<{ logpPolicy: number; logpReference: number }> => {
  const seed = `${input.taskRef}:${input.completionText}`
  const logpReference = -2 - 2 * hashFraction(`ref.${seed}`)
  const shaping = (input.reward === 1 ? 0.6 : -0.6) +
    0.2 * (hashFraction(`pol.${seed}`) - 0.5)

  return {
    logpPolicy: logpReference + shaping,
    logpReference,
  }
}

/**
 * Builds bounded preference pairs from a graded rollout set: for each
 * task with both a correct and an incorrect rollout, the lowest-index
 * correct rollout is `chosen` and the lowest-index incorrect rollout is
 * `rejected`. One pair per eligible task keeps the batch bounded and
 * fully reconstructable from the seed. Log-probs are six-decimal fixed
 * so downstream digests are stable across platforms.
 */
export const buildCs336A5PreferencePairs = (
  input: Readonly<{
    rollouts: ReadonlyArray<Cs336A5Rollout>
    splitRef: string
    tasks: ReadonlyArray<Cs336A5Task>
  }>,
): ReadonlyArray<DpoPreferencePair> => {
  const referenceByTaskRef = new Map(
    input.tasks.map(task => [task.taskRef, task.referenceValue]),
  )
  const chosenByTaskRef = new Map<string, Cs336A5Rollout>()
  const rejectedByTaskRef = new Map<string, Cs336A5Rollout>()

  const sortedRollouts = [...input.rollouts].sort((left, right) =>
    left.taskRef === right.taskRef
      ? left.rolloutIndex - right.rolloutIndex
      : left.taskRef.localeCompare(right.taskRef),
  )

  for (const rollout of sortedRollouts) {
    const reference = referenceByTaskRef.get(rollout.taskRef)

    if (reference === undefined) {
      throw new Cs336A5DpoPreferenceWorkloadError(
        'CS336 A5 DPO pair build saw a rollout for an unknown task ref.',
      )
    }

    const parsed = parseCs336A5FinalValue(rollout.completionText)
    const isCorrect = parsed !== undefined && parsed === reference

    if (isCorrect && !chosenByTaskRef.has(rollout.taskRef)) {
      chosenByTaskRef.set(rollout.taskRef, rollout)
    }

    if (!isCorrect && !rejectedByTaskRef.has(rollout.taskRef)) {
      rejectedByTaskRef.set(rollout.taskRef, rollout)
    }
  }

  const round6 = (value: number): number => Number(value.toFixed(6))

  return [...chosenByTaskRef.keys()]
    .filter(taskRef => rejectedByTaskRef.has(taskRef))
    .sort((left, right) => left.localeCompare(right))
    .map((taskRef): DpoPreferencePair => {
      const chosen = chosenByTaskRef.get(taskRef)!
      const rejected = rejectedByTaskRef.get(taskRef)!
      const chosenLogProbs = syntheticLogProbs({
        completionText: chosen.completionText,
        reward: 1,
        taskRef,
      })
      const rejectedLogProbs = syntheticLogProbs({
        completionText: rejected.completionText,
        reward: 0,
        taskRef,
      })

      return {
        chosenLogpPolicy: round6(chosenLogProbs.logpPolicy),
        chosenLogpReference: round6(chosenLogProbs.logpReference),
        pairRef: `pair.cs336_a5_dpo.${input.splitRef}.${taskRef}`,
        rejectedLogpPolicy: round6(rejectedLogProbs.logpPolicy),
        rejectedLogpReference: round6(rejectedLogProbs.logpReference),
        taskRef,
      }
    })
}

export type Cs336A5DpoGradingResult = Readonly<{
  beta: number
  elapsedMs: number
  outputDigestHex: string
  pairCount: number
  splitRef: string
  stats: Readonly<Record<string, number>>
  workloadRef: typeof Cs336A5DpoPreferenceWorkloadRef
}>

/**
 * Runs the DPO preference grading stage end to end: deterministically
 * regenerates the seeded task set and rollout batch for the split,
 * builds preference pairs, and computes the DPO batch grade. Both the
 * environment and the synthetic log-probs are pure functions of the
 * committed seed, so an opposite-Pylon re-execution reproduces the
 * output digest exactly — the `deterministic_recompute` property.
 */
export const runCs336A5DpoPreferenceGrading = async (
  input: Readonly<{
    beta?: number
    rolloutsPerTask?: number
    splitRef: Cs336A5Split
    taskCount?: number
  }>,
): Promise<Cs336A5DpoGradingResult> => {
  const startedAt = performance.now()
  const beta = input.beta ?? Cs336A5DpoDefaultBeta
  const shard = await computeCs336A1TokenizerShard()
  const tasks = buildCs336A5Tasks({
    shardDigestHex: shard.digestHex,
    splitRef: input.splitRef,
    ...(input.taskCount === undefined ? {} : { taskCount: input.taskCount }),
  })
  const batch = await runCs336A5RolloutBatch(input)
  const pairs = buildCs336A5PreferencePairs({
    rollouts: batch.rollouts,
    splitRef: input.splitRef,
    tasks,
  })
  const graded = gradeDpoPreferenceBatch({ beta, pairs })

  const outputDigestHex = await sha256Hex(
    JSON.stringify({
      beta,
      meanLossMicro: Math.round(graded.meanLoss * 1_000_000),
      pairCount: graded.pairCount,
      pairs,
      rankingAccuracyBp: Math.round(graded.rankingAccuracy * 10_000),
      rewardMarginMeanMicro: Math.round(graded.rewardMarginMean * 1_000_000),
      splitRef: input.splitRef,
      workloadRef: Cs336A5DpoPreferenceWorkloadRef,
    }),
  )

  return {
    beta,
    elapsedMs: performance.now() - startedAt,
    outputDigestHex,
    pairCount: graded.pairCount,
    splitRef: input.splitRef,
    stats: {
      chosenRewardMeanMicro: Math.round(graded.chosenRewardMean * 1_000_000),
      correctlyRankedCount: graded.grades.filter(
        grade => grade.correctlyRanked,
      ).length,
      meanLossMicro: Math.round(graded.meanLoss * 1_000_000),
      pairCount: graded.pairCount,
      rankingAccuracyBp: Math.round(graded.rankingAccuracy * 10_000),
      rejectedRewardMeanMicro: Math.round(
        graded.rejectedRewardMean * 1_000_000,
      ),
      rewardMarginMeanMicro: Math.round(graded.rewardMarginMean * 1_000_000),
    },
    workloadRef: Cs336A5DpoPreferenceWorkloadRef,
  }
}
