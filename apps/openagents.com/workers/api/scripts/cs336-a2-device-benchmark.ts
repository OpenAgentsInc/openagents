/**
 * Contributor-side executor for the bounded CS336 A2 device-capability
 * benchmark (issue #4681). Runs the real benchmark suite from
 * `src/cs336-a2-benchmark-workload.ts` on this device with wall-clock
 * timing and prints public-safe output only:
 *
 * - per-sample metric values, units, elapsed ms, and output digests
 *   (the deterministic commitments the receipted assignment binds)
 * - per-metric nearest-rank aggregates (sampleCount, p50/p90/min/max)
 * - the device class ref (platform/arch class only, never an identifier)
 *
 * No network, no secrets, no spend: dispatch, verification challenges,
 * closeout, and settlement stay on the Worker authority routes.
 *
 * Usage:
 *   bun run scripts/cs336-a2-device-benchmark.ts \
 *     [--assignment <assignmentRef>] [--repetitions <n>]
 */
import {
  aggregateCs336A2Samples,
  runCs336A2BenchmarkSuite,
} from '../src/cs336-a2-benchmark-workload'

const args = process.argv.slice(2)
const flag = (name: string): string | undefined => {
  const index = args.indexOf(name)

  return index >= 0 ? args[index + 1] : undefined
}

const assignmentRef =
  flag('--assignment') ?? `assignment.cs336_a2.device_benchmark.${Date.now()}`
const repetitions = Math.max(1, Number(flag('--repetitions') ?? 3))
const deviceClassRef = `device_class.${
  process.platform === 'darwin' && process.arch === 'arm64'
    ? 'apple_silicon_macos.arm64'
    : `${process.platform}.${process.arch}`
}`

const run = async () => {
  const suite = await runCs336A2BenchmarkSuite({ repetitions })
  const aggregates = aggregateCs336A2Samples(suite.samples)
  const contributionRef = `contribution.cs336_a2.${assignmentRef}.benchmark`

  console.log(
    JSON.stringify(
      {
        aggregates,
        assignmentRef,
        benchmarkSuiteRef: suite.benchmarkSuiteRef,
        contributionRef,
        deviceClassRef,
        digestRefs: [
          ...new Set(
            suite.samples.map(
              sample =>
                `commitment.cs336_a2.${sample.metric}.sha256_${sample.outputDigestHex.slice(0, 16)}`,
            ),
          ),
        ],
        repetitions: suite.repetitions,
        samples: suite.samples,
        suiteElapsedMs: suite.suiteElapsedMs,
      },
      null,
      2,
    ),
  )
}

run()
