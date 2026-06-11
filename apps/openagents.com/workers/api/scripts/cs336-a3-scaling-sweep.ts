/**
 * Contributor-side executor for bounded CS336 A3 scaling-sweep cells
 * (issue #4679). Runs real bounded training at the planner-chosen
 * (N, D) IsoFLOP grid points from `src/cs336-a3-sweep-workload.ts` on
 * this device and prints public-safe output only:
 *
 * - per-cell realized parameter/data/compute counts, the measured
 *   final validation loss, elapsed ms, and the deterministic output
 *   digest (the commitment the sampled `deterministic_recompute`
 *   re-runs check)
 *
 * No network, no secrets, no spend: dispatch, verification challenges,
 * closeout, and settlement stay on the Worker authority routes.
 *
 * Usage:
 *   bun run scripts/cs336-a3-scaling-sweep.ts \
 *     [--assignment <assignmentRef>] [--cells <i,j,k>] (1-based grid indexes)
 */
import {
  Cs336A3SweepWorkloadRef,
  planCs336A3SweepGrid,
  runCs336A3SweepCell,
} from '../src/cs336-a3-sweep-workload'

const args = process.argv.slice(2)
const flag = (name: string): string | undefined => {
  const index = args.indexOf(name)

  return index >= 0 ? args[index + 1] : undefined
}

const assignmentRef =
  flag('--assignment') ?? `assignment.cs336_a3.scaling_sweep.${Date.now()}`
const grid = planCs336A3SweepGrid()
const requested = (flag('--cells') ?? '')
  .split(',')
  .map(value => Number.parseInt(value.trim(), 10))
  .filter(value => Number.isInteger(value) && value >= 1)
const selected =
  requested.length > 0
    ? grid.filter(cell => requested.includes(cell.cellIndex))
    : grid

const run = async () => {
  const cells = []

  for (const cell of selected) {
    const result = await runCs336A3SweepCell(cell)

    cells.push({
      budgetIndex: cell.budgetIndex,
      cellIndex: cell.cellIndex,
      commitmentRef: `commitment.cs336_a3.cell_${cell.cellIndex}.sha256_${result.outputDigestHex.slice(0, 16)}`,
      computeBudgetFlops: result.computeBudgetFlops,
      elapsedMs: result.elapsedMs,
      finalLoss: result.finalLoss,
      initialLoss: result.initialLoss,
      outputDigestHex: result.outputDigestHex,
      parameterCount: result.parameterCount,
      rank: result.rank,
      trainedDataUnits: result.trainedDataUnits,
    })
  }

  console.log(
    JSON.stringify(
      {
        assignmentRef,
        cells,
        contributionRef: `contribution.cs336_a3.${assignmentRef}.sweep`,
        workloadRef: Cs336A3SweepWorkloadRef,
      },
      null,
      2,
    ),
  )
}

run()
