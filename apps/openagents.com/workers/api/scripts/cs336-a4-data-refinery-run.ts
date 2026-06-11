/**
 * Contributor-side executor for bounded CS336 A4 data-refinery shards
 * (issue #4680). Runs the real deterministic refinery stages from
 * `src/cs336-a4-refinery-workload.ts` over the bounded public-safe
 * synthetic corpus on this device and prints public-safe output only:
 *
 * - per-stage public counts (masked PII per class, deduped lines,
 *   kept/removed documents, near-duplicate clusters), the input
 *   document count, elapsed ms, and the deterministic output digest
 *   (the commitment the sampled `deterministic_recompute` re-runs check)
 *
 * No network, no secrets, no spend, no real Common Crawl payload: the
 * corpus is synthetic by construction. Dispatch, verification
 * challenges, closeout, and settlement stay on the Worker authority
 * routes (see the A4 payment-policy doc and the live evidence doc).
 *
 * Usage:
 *   bun run scripts/cs336-a4-data-refinery-run.ts \
 *     [--assignment <assignmentRef>] [--stages pii_masking,minhash_dedup]
 */
import { Cs336A4HomeworkStages } from '../src/cs336-a4-data-refinery'
import {
  Cs336A4RefineryWorkloadRef,
  runCs336A4RefineryStage,
} from '../src/cs336-a4-refinery-workload'

const args = process.argv.slice(2)
const flag = (name: string): string | undefined => {
  const index = args.indexOf(name)

  return index >= 0 ? args[index + 1] : undefined
}

const assignmentRef =
  flag('--assignment') ?? `assignment.cs336_a4.data_refinery.${Date.now()}`
const requested = (flag('--stages') ?? '')
  .split(',')
  .map(value => value.trim())
  .filter(value => value.length > 0)
const selected =
  requested.length > 0
    ? Cs336A4HomeworkStages.filter(stage => requested.includes(stage))
    : Cs336A4HomeworkStages

const run = async () => {
  const shards = []

  for (const stage of selected) {
    const result = await runCs336A4RefineryStage({ stage })

    shards.push({
      commitmentRef: `commitment.cs336_a4.${stage}.sha256_${result.outputDigestHex.slice(0, 16)}`,
      elapsedMs: result.elapsedMs,
      inputDocumentCount: result.inputDocumentCount,
      outputDigestHex: result.outputDigestHex,
      stage,
      stats: result.stats,
    })
  }

  console.log(
    JSON.stringify(
      {
        assignmentRef,
        contributionRef: `contribution.cs336_a4.${assignmentRef}.data_refinery`,
        shards,
        workloadRef: Cs336A4RefineryWorkloadRef,
      },
      null,
      2,
    ),
  )
}

run()
