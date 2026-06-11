/**
 * Contributor-side executor for bounded CS336 A5 rollout and grading
 * shards (issue #4682). Runs the real deterministic workload from
 * `src/cs336-a5-rollout-workload.ts` on this device and prints
 * public-safe output only:
 *
 * - per-stage public counts (rollout counts, graded counts, correct
 *   counts, unparseable counts, group counts), elapsed ms, and the
 *   deterministic output digest (the commitment the
 *   `seeded_replication` and `deterministic_recompute` re-runs check)
 *
 * No network, no secrets, no spend, no hosted LLM, and no raw task or
 * completion text in the output: the environment is a bounded seeded
 * synthetic math-task set and the policy is a seeded sampler, so the
 * whole batch is exactly replicable from the committed seed. Dispatch,
 * verification challenges, closeout, and settlement stay on the Worker
 * authority routes (see the A5 lane doc and the live evidence doc).
 *
 * Usage:
 *   bun run scripts/cs336-a5-alignment-run.ts \
 *     [--assignment <assignmentRef>] [--splits split_a,split_b] \
 *     [--stages rollout_batch,reward_grading]
 */
import {
  Cs336A5RolloutWorkloadRef,
  Cs336A5Splits,
  runCs336A5RewardGrading,
  runCs336A5RolloutBatch,
} from '../src/cs336-a5-rollout-workload'

const args = process.argv.slice(2)
const flag = (name: string): string | undefined => {
  const index = args.indexOf(name)

  return index >= 0 ? args[index + 1] : undefined
}

const assignmentRef =
  flag('--assignment') ?? `assignment.cs336_a5.alignment.${Date.now()}`
const allStages = ['rollout_batch', 'reward_grading'] as const
const requestedStages = (flag('--stages') ?? '')
  .split(',')
  .map(value => value.trim())
  .filter(value => value.length > 0)
const selectedStages =
  requestedStages.length > 0
    ? allStages.filter(stage => requestedStages.includes(stage))
    : allStages
const requestedSplits = (flag('--splits') ?? '')
  .split(',')
  .map(value => value.trim())
  .filter(value => value.length > 0)
const selectedSplits =
  requestedSplits.length > 0
    ? Cs336A5Splits.filter(split => requestedSplits.includes(split))
    : Cs336A5Splits

const run = async () => {
  const shards = []

  for (const splitRef of selectedSplits) {
    for (const stage of selectedStages) {
      const result =
        stage === 'rollout_batch'
          ? await runCs336A5RolloutBatch({ splitRef })
          : await runCs336A5RewardGrading({ splitRef })

      shards.push({
        commitmentRef: `commitment.cs336_a5.${stage}.${splitRef}.sha256_${result.outputDigestHex.slice(0, 16)}`,
        elapsedMs: result.elapsedMs,
        outputDigestHex: result.outputDigestHex,
        splitRef,
        stage,
        stats: result.stats,
      })
    }
  }

  console.log(
    JSON.stringify(
      {
        assignmentRef,
        contributionRef: `contribution.cs336_a5.${assignmentRef}.alignment`,
        shards,
        workloadRef: Cs336A5RolloutWorkloadRef,
      },
      null,
      2,
    ),
  )
}

run()
