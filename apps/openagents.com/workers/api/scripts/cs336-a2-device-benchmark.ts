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

// Device class is platform/arch only — never an identifier. The canonical
// class refs the public dataset admits today:
// - device_class.apple_silicon_macos.arm64 (the first, settled class)
// - device_class.x86_64_linux.intel (a genuinely distinct second class:
//   x86_64 Linux, e.g. an Intel Core i7-14700K contributor on Node 25)
const deviceClassRef =
  process.platform === 'darwin' && process.arch === 'arm64'
    ? 'device_class.apple_silicon_macos.arm64'
    : process.platform === 'linux' && process.arch === 'x64'
      ? 'device_class.x86_64_linux.intel'
      : `device_class.${process.platform}.${process.arch}`

// Genuinely measured but not paid/cross-checked unless an operator
// dispatches a paid assignment and a validator finalizes a
// statistical_cross_check verdict for this class. The contributor runner
// therefore emits the honest `measured_unsettled` provenance by default;
// the Worker admission path enforces that such rows carry no settlement
// receipt and no earning estimate.
const measurementProvenance = 'measured_unsettled'

const run = async () => {
  const suite = await runCs336A2BenchmarkSuite({ repetitions })
  const aggregates = aggregateCs336A2Samples(suite.samples)
  const contributionRef = `contribution.cs336_a2.${assignmentRef}.benchmark`
  const digestRefs = [
    ...new Set(
      suite.samples.map(
        sample =>
          `commitment.cs336_a2.${sample.metric}.sha256_${sample.outputDigestHex.slice(0, 16)}`,
      ),
    ),
  ]

  console.log(
    JSON.stringify(
      {
        aggregates,
        assignmentRef,
        benchmarkSuiteRef: suite.benchmarkSuiteRef,
        contributionRef,
        deviceClassRef,
        digestRefs,
        // Admission-ready measured_unsettled rows for the public dataset.
        measurementEvidence: aggregates.map(aggregate => ({
          deviceClassRef,
          digestCommitmentRefs: digestRefs,
          max: aggregate.max,
          measurementProvenance,
          measurementRef: `measurement.cs336_a2.${deviceClassRef.replace(/[.:]/g, '_')}.${aggregate.metric}`,
          metric: aggregate.metric,
          min: aggregate.min,
          p50: aggregate.p50,
          p90: aggregate.p90,
          receiptRefs: [],
          unit: aggregate.unit,
          verificationRefs: [],
          workClass: 'cs336_a2_device_benchmark',
          sampleCount: aggregate.sampleCount,
        })),
        measurementProvenance,
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
