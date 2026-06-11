/**
 * Bounded CS336 A5 rollout-and-grading workload (issue #4682).
 *
 * Two real deterministic stages over a bounded synthetic math-task
 * environment, mirroring the A5 decomposition: rollout batches are
 * seeded sampling (so `seeded_replication` verification applies —
 * re-running the same seed reproduces the exact rollout set and its
 * digest), and reward grading is cheap deterministic CPU work
 * (`deterministic_recompute` — reference-graded GSM8K-format answer
 * extraction, exact-match rewards, and group-normalized advantages per
 * the Psionic `psion_cs336_a5_alignment_reference_v1` lane contracts
 * from `psionic#1101`).
 *
 * Provenance binds to the A1 trainer (plan step 3): the task-set seed
 * derives from the A1 tokenizer shard digest in
 * `cs336-a1-homework-workload.ts`, so every A5 rollout and grading
 * shard is downstream of the same committed corpus pipeline as the
 * #4675/#4679/#4680 runs.
 *
 * The environment is synthetic by construction: bounded seeded
 * arithmetic tasks with exact integer reference values. No hosted LLM
 * is involved anywhere — the rollout policy is a seeded stochastic
 * sampler over the task's own arithmetic, which makes the batch real
 * sampled work that is exactly replicable from the seed. Task text and
 * rollout completions stay on the contributor device; only counts,
 * digests, and refs are public. Eval scores produced here are eval
 * evidence about this bounded synthetic task set only, never model
 * capability claims, and the policy-gradient update step stays behind
 * the #4669 training boundary.
 */

import { computeCs336A1TokenizerShard } from './cs336-a1-homework-workload'

export class Cs336A5RolloutWorkloadError extends Error {
  readonly _tag = 'Cs336A5RolloutWorkloadError'
}

export const Cs336A5RolloutWorkloadRef =
  'workload.cs336_a5.seeded_rollout_and_reference_grading.v1'
export const Cs336A5TaskSetDatasetRef =
  'dataset.cs336_a5.bounded_synthetic_math_tasks.v1'

export const Cs336A5Splits = ['split_a', 'split_b'] as const
export type Cs336A5Split = (typeof Cs336A5Splits)[number]

const Cs336A5DefaultTaskCount = 32
const Cs336A5DefaultRolloutsPerTask = 4
const Cs336A5AdvantageEpsilon = 1e-6

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

/** Deterministic mulberry32 PRNG over a string-derived seed. */
const seededRandom = (seed: string): (() => number) => {
  let state = fnv1a32(seed)

  return () => {
    state = (state + 0x6d_2b_79_f5) >>> 0
    let mixed = state
    mixed = Math.imul(mixed ^ (mixed >>> 15), mixed | 1)
    mixed ^= mixed + Math.imul(mixed ^ (mixed >>> 7), mixed | 61)

    return ((mixed ^ (mixed >>> 14)) >>> 0) / 4_294_967_296
  }
}

export type Cs336A5Task = Readonly<{
  difficulty: number
  referenceValue: number
  taskRef: string
  taskText: string
}>

type Cs336A5TaskTemplate = Readonly<{
  evaluate: (a: number, b: number, c: number) => number
  render: (a: number, b: number, c: number) => string
}>

const Cs336A5TaskTemplates: ReadonlyArray<Cs336A5TaskTemplate> = [
  {
    evaluate: (a, b, c) => (a + b) * c,
    render: (a, b, c) =>
      `A crate holds ${a} parts plus ${b} spares. With ${c} crates total, how many parts are there?`,
  },
  {
    evaluate: (a, b, c) => a * b - c,
    render: (a, b, c) =>
      `Each of ${a} shelves holds ${b} units. After removing ${c} units, how many remain?`,
  },
  {
    evaluate: (a, b, c) => a + b * c,
    render: (a, b, c) =>
      `A ledger starts at ${a} entries and gains ${b} entries on each of ${c} days. How many entries result?`,
  },
  {
    evaluate: (a, b, c) => a * c + b * c,
    render: (a, b, c) =>
      `A route makes ${c} trips carrying ${a} boxes and ${b} bags each trip. How many items move in total?`,
  },
]

/**
 * Builds the bounded synthetic task set deterministically from the A1
 * tokenizer shard digest and the split ref. Each task is a two-step
 * arithmetic word problem with an exact integer reference value. Task
 * text never leaves the contributor device.
 */
export const buildCs336A5Tasks = (
  input: Readonly<{
    shardDigestHex: string
    splitRef: string
    taskCount?: number
  }>,
): ReadonlyArray<Cs336A5Task> => {
  const taskCount = input.taskCount ?? Cs336A5DefaultTaskCount

  if (!Number.isInteger(taskCount) || taskCount < 4) {
    throw new Cs336A5RolloutWorkloadError(
      'CS336 A5 task set needs at least 4 tasks.',
    )
  }

  const random = seededRandom(
    `cs336_a5.tasks.${input.shardDigestHex}.${input.splitRef}`,
  )
  const tasks: Cs336A5Task[] = []

  for (let taskIndex = 0; taskIndex < taskCount; taskIndex += 1) {
    const template =
      Cs336A5TaskTemplates[
        Math.floor(random() * Cs336A5TaskTemplates.length)
      ]!
    const a = 2 + Math.floor(random() * 18)
    const b = 2 + Math.floor(random() * 18)
    const c = 2 + Math.floor(random() * 8)

    tasks.push({
      difficulty: random(),
      referenceValue: template.evaluate(a, b, c),
      taskRef: `task.cs336_a5.${input.splitRef}.${taskIndex}`,
      taskText: template.render(a, b, c),
    })
  }

  return tasks
}

export type Cs336A5Rollout = Readonly<{
  completionText: string
  rolloutIndex: number
  taskRef: string
}>

export type Cs336A5RolloutBatchResult = Readonly<{
  elapsedMs: number
  outputDigestHex: string
  rollouts: ReadonlyArray<Cs336A5Rollout>
  splitRef: string
  stats: Readonly<Record<string, number>>
  taskSetDatasetRef: typeof Cs336A5TaskSetDatasetRef
  workloadRef: typeof Cs336A5RolloutWorkloadRef
}>

/**
 * Seeded stochastic rollout policy. For each task the policy samples a
 * group of completions: with task-dependent propensity it emits the
 * exact reference value in GSM8K answer format (`#### <value>`),
 * otherwise it emits a seeded arithmetic error, and a small seeded
 * fraction of completions are malformed (missing the `####` line) so
 * the grader's bounded format parsing does real work. Everything is a
 * pure function of the seed: replaying the same seed reproduces the
 * exact rollout set, which is the `seeded_replication` property.
 */
const sampleRollout = (
  input: Readonly<{
    rolloutIndex: number
    seedScope: string
    task: Cs336A5Task
  }>,
): Cs336A5Rollout => {
  const random = seededRandom(
    `${input.seedScope}.${input.task.taskRef}.${input.rolloutIndex}`,
  )
  const correctnessPropensity = 0.35 + 0.5 * input.task.difficulty
  const isCorrect = random() < correctnessPropensity
  const isMalformed = random() < 0.05
  const errorOffset = (1 + Math.floor(random() * 3)) * (random() < 0.5 ? -1 : 1)
  const finalValue = isCorrect
    ? input.task.referenceValue
    : input.task.referenceValue + errorOffset
  const reasoningLine = `Working through the quantities step by step gives ${finalValue}.`
  const completionText = isMalformed
    ? `${reasoningLine}\nThe final value is ${finalValue}`
    : `${reasoningLine}\n#### ${finalValue}`

  return {
    completionText,
    rolloutIndex: input.rolloutIndex,
    taskRef: input.task.taskRef,
  }
}

export const runCs336A5RolloutBatch = async (
  input: Readonly<{
    rolloutsPerTask?: number
    splitRef: Cs336A5Split
    taskCount?: number
  }>,
): Promise<Cs336A5RolloutBatchResult> => {
  const startedAt = performance.now()
  const rolloutsPerTask = input.rolloutsPerTask ?? Cs336A5DefaultRolloutsPerTask

  if (!Number.isInteger(rolloutsPerTask) || rolloutsPerTask < 2) {
    throw new Cs336A5RolloutWorkloadError(
      'CS336 A5 rollout batches need at least 2 rollouts per task.',
    )
  }

  const shard = await computeCs336A1TokenizerShard()
  const tasks = buildCs336A5Tasks({
    shardDigestHex: shard.digestHex,
    splitRef: input.splitRef,
    ...(input.taskCount === undefined ? {} : { taskCount: input.taskCount }),
  })
  const seedScope = `cs336_a5.rollout.${shard.digestHex}.${input.splitRef}`
  const rollouts: Cs336A5Rollout[] = []

  for (const task of tasks) {
    for (
      let rolloutIndex = 0;
      rolloutIndex < rolloutsPerTask;
      rolloutIndex += 1
    ) {
      rollouts.push(sampleRollout({ rolloutIndex, seedScope, task }))
    }
  }

  const meanCompletionChars =
    rollouts.reduce(
      (total, rollout) => total + rollout.completionText.length,
      0,
    ) / rollouts.length
  const outputDigestHex = await sha256Hex(
    JSON.stringify({
      rollouts,
      shardDigestHex: shard.digestHex,
      splitRef: input.splitRef,
      taskSetDatasetRef: Cs336A5TaskSetDatasetRef,
      workloadRef: Cs336A5RolloutWorkloadRef,
    }),
  )

  return {
    elapsedMs: performance.now() - startedAt,
    outputDigestHex,
    rollouts,
    splitRef: input.splitRef,
    stats: {
      meanCompletionChars: Math.round(meanCompletionChars),
      rolloutCount: rollouts.length,
      rolloutsPerTask,
      taskCount: tasks.length,
    },
    taskSetDatasetRef: Cs336A5TaskSetDatasetRef,
    workloadRef: Cs336A5RolloutWorkloadRef,
  }
}

/**
 * Bounded GSM8K-format final-value extraction: the last `#### <value>`
 * line wins, commas and surrounding whitespace are tolerated, and
 * anything else is an unparseable completion (reward 0, counted).
 * Stanford-fixture conformance for the full parsers remains a Psionic
 * ask before this grader is pointed at real GSM8K data.
 */
export const parseCs336A5FinalValue = (
  completionText: string,
): number | undefined => {
  const matches = [...completionText.matchAll(/^####\s*(-?[\d,]+)\s*$/gm)]
  const last = matches[matches.length - 1]

  if (last === undefined) {
    return undefined
  }

  const parsed = Number(last[1]!.replaceAll(',', ''))

  return Number.isSafeInteger(parsed) ? parsed : undefined
}

export type Cs336A5GradingResult = Readonly<{
  accuracy: number
  elapsedMs: number
  outputDigestHex: string
  splitRef: string
  stats: Readonly<Record<string, number>>
  workloadRef: typeof Cs336A5RolloutWorkloadRef
}>

type Cs336A5GradedGroup = Readonly<{
  advantages: ReadonlyArray<string>
  rewards: ReadonlyArray<number>
  taskRef: string
}>

/**
 * Reference-grades a rollout set against the task reference values:
 * exact-match rewards, then group-normalized advantages per task group
 * (reward minus group mean over group standard deviation with epsilon,
 * zero for zero-variance groups), mirroring the group-normalized
 * rewards in the Psionic A5 lane. Advantages are committed at fixed
 * six-decimal precision so the digest is a stable deterministic
 * commitment for `deterministic_recompute` re-runs.
 */
export const gradeCs336A5Rollouts = async (
  input: Readonly<{
    rollouts: ReadonlyArray<Cs336A5Rollout>
    splitRef: string
    tasks: ReadonlyArray<Cs336A5Task>
  }>,
): Promise<Cs336A5GradingResult> => {
  const startedAt = performance.now()

  if (input.rollouts.length === 0) {
    throw new Cs336A5RolloutWorkloadError(
      'CS336 A5 grading needs at least one rollout.',
    )
  }

  const referenceByTaskRef = new Map(
    input.tasks.map(task => [task.taskRef, task.referenceValue]),
  )
  const rewardsByTaskRef = new Map<string, number[]>()
  let correctCount = 0
  let unparseableCount = 0

  for (const rollout of input.rollouts) {
    const reference = referenceByTaskRef.get(rollout.taskRef)

    if (reference === undefined) {
      throw new Cs336A5RolloutWorkloadError(
        'CS336 A5 grading saw a rollout for an unknown task ref.',
      )
    }

    const parsed = parseCs336A5FinalValue(rollout.completionText)

    if (parsed === undefined) {
      unparseableCount += 1
    }

    const reward = parsed !== undefined && parsed === reference ? 1 : 0

    correctCount += reward
    const group = rewardsByTaskRef.get(rollout.taskRef) ?? []

    group.push(reward)
    rewardsByTaskRef.set(rollout.taskRef, group)
  }

  let zeroVarianceGroupCount = 0
  const gradedGroups: Cs336A5GradedGroup[] = [...rewardsByTaskRef.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([taskRef, rewards]) => {
      const mean = rewards.reduce((total, value) => total + value, 0) /
        rewards.length
      const variance =
        rewards.reduce((total, value) => total + (value - mean) ** 2, 0) /
        rewards.length
      const std = Math.sqrt(variance)

      if (std === 0) {
        zeroVarianceGroupCount += 1
      }

      return {
        advantages: rewards.map(reward =>
          std === 0
            ? (0).toFixed(6)
            : ((reward - mean) / (std + Cs336A5AdvantageEpsilon)).toFixed(6),
        ),
        rewards,
        taskRef,
      }
    })

  const accuracy = correctCount / input.rollouts.length
  // The grading digest binds the exact rollout input set, not just the
  // grading output: tampering any completion changes the commitment
  // even when the reward vector happens to stay identical.
  const inputRolloutDigestHex = await sha256Hex(JSON.stringify(input.rollouts))
  const outputDigestHex = await sha256Hex(
    JSON.stringify({
      accuracyBp: Math.round(accuracy * 10_000),
      correctCount,
      gradedGroups,
      gradedRolloutCount: input.rollouts.length,
      inputRolloutDigestHex,
      splitRef: input.splitRef,
      unparseableCount,
      workloadRef: Cs336A5RolloutWorkloadRef,
    }),
  )

  return {
    accuracy,
    elapsedMs: performance.now() - startedAt,
    outputDigestHex,
    splitRef: input.splitRef,
    stats: {
      accuracyBp: Math.round(accuracy * 10_000),
      correctCount,
      gradedRolloutCount: input.rollouts.length,
      groupCount: gradedGroups.length,
      unparseableCount,
      zeroVarianceGroupCount,
    },
    workloadRef: Cs336A5RolloutWorkloadRef,
  }
}

/**
 * Runs the grading stage end to end: deterministically regenerates the
 * seeded task set and rollout batch for the split, then
 * reference-grades it. Because both the environment and the policy are
 * pure functions of the committed seed, an opposite-Pylon re-execution
 * reproduces the grading digest exactly — the
 * `deterministic_recompute` property.
 */
export const runCs336A5RewardGrading = async (
  input: Readonly<{
    rolloutsPerTask?: number
    splitRef: Cs336A5Split
    taskCount?: number
  }>,
): Promise<Cs336A5GradingResult> => {
  const shard = await computeCs336A1TokenizerShard()
  const tasks = buildCs336A5Tasks({
    shardDigestHex: shard.digestHex,
    splitRef: input.splitRef,
    ...(input.taskCount === undefined ? {} : { taskCount: input.taskCount }),
  })
  const batch = await runCs336A5RolloutBatch(input)

  return gradeCs336A5Rollouts({
    rollouts: batch.rollouts,
    splitRef: input.splitRef,
    tasks,
  })
}

export type Cs336A5EvalSuiteSummary = Readonly<{
  metric: 'accuracy'
  sampleCount: number
  score: number
  splitRef: string
  taskSetRef: 'math'
  verifiedSampleCount: number
}>

/**
 * Combines grading results into one public eval-suite summary row. The
 * task set is honestly labeled `math` (a bounded synthetic arithmetic
 * task set), never `gsm8k` or `mmlu`: no real Stanford eval data was
 * graded, and the Psionic parser fixture-conformance ask stands before
 * real GSM8K/MMLU suites are paid. Verified sample counts must be
 * claimed only for gradings whose digests carry Verified challenges.
 */
export const buildCs336A5EvalSuiteSummary = (
  input: Readonly<{
    gradings: ReadonlyArray<Cs336A5GradingResult>
    splitRef: string
    verifiedGradingDigests: ReadonlyArray<string>
  }>,
): Cs336A5EvalSuiteSummary => {
  if (input.gradings.length === 0) {
    throw new Cs336A5RolloutWorkloadError(
      'CS336 A5 eval suite summary needs at least one grading result.',
    )
  }

  const sampleCount = input.gradings.reduce(
    (total, grading) => total + (grading.stats.gradedRolloutCount ?? 0),
    0,
  )
  const correctCount = input.gradings.reduce(
    (total, grading) => total + (grading.stats.correctCount ?? 0),
    0,
  )
  const verifiedSampleCount = input.gradings.reduce(
    (total, grading) =>
      total +
      (input.verifiedGradingDigests.includes(grading.outputDigestHex)
        ? (grading.stats.gradedRolloutCount ?? 0)
        : 0),
    0,
  )

  return {
    metric: 'accuracy',
    sampleCount,
    score: sampleCount === 0 ? 0 : correctCount / sampleCount,
    splitRef: input.splitRef,
    taskSetRef: 'math',
    verifiedSampleCount,
  }
}
