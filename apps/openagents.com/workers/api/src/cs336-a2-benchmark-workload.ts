/**
 * Bounded CS336 A2 device-capability benchmark workload (issue #4681).
 *
 * This module executes the four measurement kinds of
 * `benchmark_suite.cs336_a2.pylon_runtime_device_capability.v1` on a
 * contributor device:
 *
 * - `attention_throughput`: bounded softmax(QK^T)V attention block over
 *   seeded matrices, reported as measured megaflops.
 * - `memory_bandwidth`: a STREAM-triad style pass over typed arrays,
 *   reported as measured gigabytes per second.
 * - `tokens_per_second`: a bounded greedy reference-row decode loop
 *   (vocab-projection matmul per step), reported as measured rows/sec.
 * - `step_time_ms`: one bounded forward/backward/SGD training step,
 *   reported as measured wall milliseconds.
 *
 * Every kernel produces a deterministic output digest (the commitment the
 * receipted assignment binds) alongside its nondeterministic timing, so
 * `statistical_cross_check` verification compares timing distributions
 * across same-class devices while output digests stay exactly checkable.
 *
 * Payloads are public-safe by construction: seeded numeric work, digests,
 * refs, and timings only. No device identifiers, wallet, payment, or
 * private material is ever part of the workload.
 */

import {
  Cs336A2BenchmarkMeasurements,
  type Cs336A2BenchmarkMeasurement,
  Cs336A2DeviceBenchmarkSuiteRef,
  type Cs336A2MeasurementEvidence,
  Cs336A2ThermalThrottleMetric,
} from './training-device-capability'

const AttentionSequenceLength = 64
const AttentionHeadDimension = 32
const BandwidthElementCount = 1 << 21
const BandwidthPassCount = 8
const DecodeHiddenDimension = 48
const DecodeVocabularySize = 96
const DecodeStepCount = 48
const TrainingStepRows = 32
const TrainingStepInner = 64
const TrainingStepColumns = 32

export type Cs336A2BenchmarkSample = Readonly<{
  elapsedMs: number
  metric: Cs336A2BenchmarkMeasurement
  outputDigestHex: string
  unit: string
  value: number
}>

export type Cs336A2BenchmarkSuiteResult = Readonly<{
  benchmarkSuiteRef: typeof Cs336A2DeviceBenchmarkSuiteRef
  repetitions: number
  samples: ReadonlyArray<Cs336A2BenchmarkSample>
  suiteElapsedMs: number
}>

export type Cs336A2MeasurementAggregate = Readonly<{
  max: number
  metric: Cs336A2BenchmarkMeasurement
  min: number
  p50: number
  p90: number
  sampleCount: number
  unit: string
}>

export type Cs336A2ThermalThrottleWindowSample = Readonly<{
  phase: 'burst' | 'sustained'
  throughput: number
}>

export type Cs336A2ThermalThrottleEvidenceInput = Readonly<{
  deviceClassRef: string
  digestCommitmentRefs: ReadonlyArray<string>
  receiptRefs: ReadonlyArray<string>
  samples: ReadonlyArray<Cs336A2ThermalThrottleWindowSample>
  sourceRefs?: ReadonlyArray<string>
  verificationRefs: ReadonlyArray<string>
  workClass: string
}>

export class Cs336A2ThermalThrottleEvidenceError extends TypeError {
  readonly _tag = 'Cs336A2ThermalThrottleEvidenceError'
}

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

/** Deterministic 32-bit PRNG so every device runs identical numeric work. */
const seededGenerator = (seed: number): (() => number) => {
  let state = seed >>> 0

  return () => {
    state = (state + 0x6d2b79f5) >>> 0
    let mixed = state
    mixed = Math.imul(mixed ^ (mixed >>> 15), mixed | 1)
    mixed ^= mixed + Math.imul(mixed ^ (mixed >>> 7), mixed | 61)

    return ((mixed ^ (mixed >>> 14)) >>> 0) / 4294967296
  }
}

const seededArray = (seed: number, length: number): Float64Array => {
  const next = seededGenerator(seed)
  const values = new Float64Array(length)

  for (let index = 0; index < length; index += 1) {
    values[index] = next() - 0.5
  }

  return values
}

const checksum = (values: Float64Array): string => {
  let sum = 0

  for (let index = 0; index < values.length; index += 1) {
    sum += values[index]! * ((index % 17) + 1)
  }

  return sum.toPrecision(12)
}

type KernelOutcome = Readonly<{
  checksumText: string
  workUnits: number
}>

const attentionKernel = (): KernelOutcome => {
  const seq = AttentionSequenceLength
  const dim = AttentionHeadDimension
  const queries = seededArray(101, seq * dim)
  const keys = seededArray(102, seq * dim)
  const values = seededArray(103, seq * dim)
  const scores = new Float64Array(seq * seq)
  const output = new Float64Array(seq * dim)
  const scale = 1 / Math.sqrt(dim)

  for (let row = 0; row < seq; row += 1) {
    let maxScore = Number.NEGATIVE_INFINITY

    for (let column = 0; column < seq; column += 1) {
      let dot = 0

      for (let inner = 0; inner < dim; inner += 1) {
        dot += queries[row * dim + inner]! * keys[column * dim + inner]!
      }

      const scaled = dot * scale

      scores[row * seq + column] = scaled
      maxScore = Math.max(maxScore, scaled)
    }

    let denominator = 0

    for (let column = 0; column < seq; column += 1) {
      const weight = Math.exp(scores[row * seq + column]! - maxScore)

      scores[row * seq + column] = weight
      denominator += weight
    }

    for (let inner = 0; inner < dim; inner += 1) {
      let mixed = 0

      for (let column = 0; column < seq; column += 1) {
        mixed += scores[row * seq + column]! * values[column * dim + inner]!
      }

      output[row * dim + inner] = mixed / denominator
    }
  }

  return {
    checksumText: checksum(output),
    workUnits: 4 * seq * seq * dim + 5 * seq * seq,
  }
}

const memoryBandwidthKernel = (): KernelOutcome => {
  const count = BandwidthElementCount
  const source = seededArray(201, count)
  const blend = seededArray(202, count)
  const target = new Float64Array(count)

  for (let pass = 0; pass < BandwidthPassCount; pass += 1) {
    for (let index = 0; index < count; index += 1) {
      target[index] = source[index]! + 0.5 * blend[index]!
      source[index] = target[index]! * 0.999
    }
  }

  return {
    checksumText: checksum(target),
    workUnits: BandwidthPassCount * count * 8 * 4,
  }
}

const referenceRowDecodeKernel = (): KernelOutcome => {
  const hidden = DecodeHiddenDimension
  const vocabulary = DecodeVocabularySize
  const projection = seededArray(301, vocabulary * hidden)
  const embeddings = seededArray(302, vocabulary * hidden)
  const state = seededArray(303, hidden)
  const decoded = new Float64Array(DecodeStepCount)

  for (let step = 0; step < DecodeStepCount; step += 1) {
    let bestRow = 0
    let bestLogit = Number.NEGATIVE_INFINITY

    for (let row = 0; row < vocabulary; row += 1) {
      let logit = 0

      for (let inner = 0; inner < hidden; inner += 1) {
        logit += projection[row * hidden + inner]! * state[inner]!
      }

      if (logit > bestLogit) {
        bestLogit = logit
        bestRow = row
      }
    }

    decoded[step] = bestRow

    for (let inner = 0; inner < hidden; inner += 1) {
      state[inner] =
        state[inner]! * 0.75 + embeddings[bestRow * hidden + inner]! * 0.25
    }
  }

  return {
    checksumText: checksum(decoded),
    workUnits: DecodeStepCount,
  }
}

const trainingStepKernel = (): KernelOutcome => {
  const rows = TrainingStepRows
  const inner = TrainingStepInner
  const columns = TrainingStepColumns
  const activations = seededArray(401, rows * inner)
  const weights = seededArray(402, inner * columns)
  const targets = seededArray(403, rows * columns)
  const outputs = new Float64Array(rows * columns)
  const errors = new Float64Array(rows * columns)

  for (let row = 0; row < rows; row += 1) {
    for (let column = 0; column < columns; column += 1) {
      let sum = 0

      for (let k = 0; k < inner; k += 1) {
        sum += activations[row * inner + k]! * weights[k * columns + column]!
      }

      outputs[row * columns + column] = sum
      errors[row * columns + column] = sum - targets[row * columns + column]!
    }
  }

  for (let k = 0; k < inner; k += 1) {
    for (let column = 0; column < columns; column += 1) {
      let gradient = 0

      for (let row = 0; row < rows; row += 1) {
        gradient +=
          activations[row * inner + k]! * errors[row * columns + column]!
      }

      weights[k * columns + column] =
        weights[k * columns + column]! - (0.001 * gradient) / rows
    }
  }

  return {
    checksumText: checksum(weights),
    workUnits: 1,
  }
}

const kernelByMetric: Readonly<
  Record<Cs336A2BenchmarkMeasurement, () => KernelOutcome>
> = {
  attention_throughput: attentionKernel,
  memory_bandwidth: memoryBandwidthKernel,
  step_time_ms: trainingStepKernel,
  tokens_per_second: referenceRowDecodeKernel,
}

const unitByMetric: Readonly<Record<Cs336A2BenchmarkMeasurement, string>> = {
  attention_throughput: 'megaflops',
  memory_bandwidth: 'gigabytes_per_second',
  step_time_ms: 'milliseconds',
  tokens_per_second: 'tokens_per_second',
}

const sampleValue = (
  metric: Cs336A2BenchmarkMeasurement,
  workUnits: number,
  elapsedMs: number,
): number => {
  const elapsedSeconds = Math.max(elapsedMs, 0.000_001) / 1000

  if (metric === 'attention_throughput') {
    return workUnits / elapsedSeconds / 1_000_000
  }

  if (metric === 'memory_bandwidth') {
    return workUnits / elapsedSeconds / 1_000_000_000
  }

  if (metric === 'tokens_per_second') {
    return workUnits / elapsedSeconds
  }

  return Math.max(elapsedMs, 0.000_001)
}

/**
 * Runs every benchmark kernel `repetitions` times and returns timed,
 * digest-committed samples. The clock is injectable so tests stay
 * deterministic inside the workers test runtime, where wall time does
 * not advance during synchronous execution.
 */
export const runCs336A2BenchmarkSuite = async (
  input?: Readonly<{ now?: () => number; repetitions?: number }>,
): Promise<Cs336A2BenchmarkSuiteResult> => {
  const repetitions = Math.max(1, Math.trunc(input?.repetitions ?? 3))
  const now = input?.now ?? (() => performance.now())
  const samples: Cs336A2BenchmarkSample[] = []
  const suiteStartedAt = now()

  for (let repetition = 0; repetition < repetitions; repetition += 1) {
    for (const metric of Cs336A2BenchmarkMeasurements) {
      const startedAt = now()
      const outcome = kernelByMetric[metric]()
      const elapsedMs = Math.max(now() - startedAt, 0)

      samples.push({
        elapsedMs,
        metric,
        outputDigestHex: await sha256Hex(
          `${Cs336A2DeviceBenchmarkSuiteRef}:${metric}:${outcome.checksumText}`,
        ),
        unit: unitByMetric[metric],
        value: sampleValue(metric, outcome.workUnits, elapsedMs),
      })
    }
  }

  return {
    benchmarkSuiteRef: Cs336A2DeviceBenchmarkSuiteRef,
    repetitions,
    samples,
    suiteElapsedMs: Math.max(now() - suiteStartedAt, 0),
  }
}

const nearestRankPercentile = (
  sorted: ReadonlyArray<number>,
  percentile: number,
): number =>
  sorted[
    Math.min(
      sorted.length - 1,
      Math.max(0, Math.ceil(percentile * sorted.length) - 1),
    )
  ]!

const aggregateValues = (
  values: ReadonlyArray<number>,
): Readonly<{
  max: number
  min: number
  p50: number
  p90: number
  sampleCount: number
}> | null => {
  const sorted = values
    .filter(value => Number.isFinite(value) && value > 0)
    .sort((left, right) => left - right)

  if (sorted.length === 0) {
    return null
  }

  return {
    max: sorted[sorted.length - 1]!,
    min: sorted[0]!,
    p50: nearestRankPercentile(sorted, 0.5),
    p90: nearestRankPercentile(sorted, 0.9),
    sampleCount: sorted.length,
  }
}

/**
 * Aggregates raw samples into the class-level distribution shape the
 * public dataset admits: nearest-rank percentiles plus min/max and the
 * honest sample count. Sample values only — no device identifiers.
 */
export const aggregateCs336A2Samples = (
  samples: ReadonlyArray<Cs336A2BenchmarkSample>,
): ReadonlyArray<Cs336A2MeasurementAggregate> =>
  Cs336A2BenchmarkMeasurements.flatMap(metric => {
    const values = aggregateValues(
      samples
        .filter(sample => sample.metric === metric)
        .map(sample => sample.value),
    )

    if (values === null) {
      return []
    }

    const aggregate: Cs336A2MeasurementAggregate = {
      max: values.max,
      metric,
      min: values.min,
      p50: values.p50,
      p90: values.p90,
      sampleCount: values.sampleCount,
      unit: samples.find(sample => sample.metric === metric)!.unit,
    }

    return [aggregate]
  })

export const buildCs336A2ThermalThrottleMeasurementEvidence = (
  input: Cs336A2ThermalThrottleEvidenceInput,
): Cs336A2MeasurementEvidence => {
  const burst = aggregateValues(
    input.samples
      .filter(sample => sample.phase === 'burst')
      .map(sample => sample.throughput),
  )
  const sustained = aggregateValues(
    input.samples
      .filter(sample => sample.phase === 'sustained')
      .map(sample => sample.throughput),
  )

  if (burst === null || sustained === null) {
    throw new Cs336A2ThermalThrottleEvidenceError(
      'CS336 A2 thermal-throttle evidence requires positive burst and sustained samples.',
    )
  }

  const ratios = input.samples
    .filter(sample => sample.phase === 'sustained')
    .map(sample => sample.throughput / burst.p50)
  const ratio = aggregateValues(ratios)

  if (ratio === null) {
    throw new Cs336A2ThermalThrottleEvidenceError(
      'CS336 A2 thermal-throttle evidence requires finite sustained-vs-burst ratios.',
    )
  }

  return {
    deviceClassRef: input.deviceClassRef,
    digestCommitmentRefs: input.digestCommitmentRefs,
    max: ratio.max,
    measurementRef: `measurement.cs336_a2.thermal.${input.deviceClassRef}`,
    metric: Cs336A2ThermalThrottleMetric,
    min: ratio.min,
    p50: ratio.p50,
    p90: ratio.p90,
    receiptRefs: input.receiptRefs,
    sampleCount: Math.min(burst.sampleCount, sustained.sampleCount),
    ...(input.sourceRefs === undefined ? {} : { sourceRefs: input.sourceRefs }),
    unit: 'ratio',
    verificationRefs: input.verificationRefs,
    workClass: input.workClass,
  }
}

/**
 * Cross-device agreement score for `statistical_cross_check`: the ratio
 * of the smallest to the largest same-class device median, in (0, 1].
 * Identical devices score 1; a device that disagrees by more than the
 * class threshold drags the score below `minimumScore` and the challenge
 * rejects with `StatisticalThresholdFailed`.
 */
export const cs336A2CrossDeviceAgreementScore = (
  deviceMedians: ReadonlyArray<number>,
): number => {
  const medians = deviceMedians.filter(
    value => Number.isFinite(value) && value > 0,
  )

  if (medians.length < 2 || medians.length !== deviceMedians.length) {
    return 0
  }

  return Math.min(...medians) / Math.max(...medians)
}

/**
 * Models sats/hour from a measured, receipted closeout: paid sats over
 * the measured suite wall time. This is the only earning basis the
 * public dataset accepts (`modeled_from_measured_benchmark_distribution`).
 */
export const cs336A2ModeledSatsPerHour = (
  input: Readonly<{ paidSats: number; suiteElapsedMs: number }>,
): number | null =>
  Number.isFinite(input.paidSats) &&
  input.paidSats > 0 &&
  Number.isFinite(input.suiteElapsedMs) &&
  input.suiteElapsedMs > 0
    ? (input.paidSats * 3_600_000) / input.suiteElapsedMs
    : null
