// Gym throughput/concurrency environment (#6244).
//
// This is the typed, repeatable report artifact behind promoting the owner-only
// `/gym/oss` ramp into the broader Gym measurement system. It intentionally
// mirrors the `/gym/oss` measurement discipline:
//   - every metric is a real measured number or `not_measured`, never a fake 0;
//   - percentiles are computed over measured samples only;
//   - concurrency degradation is explicit (quota-limited or latency-degraded);
//   - speculative-decoding acceptance is recorded as a measured rate only when
//     the serving lane disclosed it.
//
// PURE: no Worker, no D1, no network, no clock. Same spec + samples + generatedAt
// produces the same report.
import { Schema as S } from 'effect'

import type { MeasuredNumber } from '../khala-telemetry'
import { NOT_MEASURED, isMeasured, measured } from '../khala-telemetry'
import { KhalaSpeculationMode } from '../khala-speculation'
import { BenchmarkEngine, BenchmarkLane } from '../benchmark'
import { mean, percentile } from '../benchmark/report'

export const GYM_THROUGHPUT_ENVIRONMENT_REF = 'throughput-concurrency' as const
export const GYM_THROUGHPUT_REPORT_SCHEMA =
  'openagents.gym.throughput_concurrency_report.v1' as const
export const GYM_THROUGHPUT_SAMPLE_SCHEMA =
  'openagents.gym.throughput_sample.v1' as const

const ThroughputMeasuredNumber = S.Union([S.Number, S.Literal(NOT_MEASURED)])

export const GymThroughputTarget = S.Struct({
  lane: BenchmarkLane,
  engine: BenchmarkEngine,
  modelRef: S.String,
})
export type GymThroughputTarget = typeof GymThroughputTarget.Type

export const GymThroughputEnvironmentSpec = S.Struct({
  schemaVersion: S.Literal('openagents.gym.throughput_environment.v1'),
  environmentRef: S.Literal(GYM_THROUGHPUT_ENVIRONMENT_REF),
  target: GymThroughputTarget,
  promptProfile: S.String,
  concurrencyRamp: S.Array(
    S.Number.check(S.isBetween({ minimum: 1, maximum: 64 })),
  ),
  samplesPerConcurrency: S.Number.check(
    S.isBetween({ minimum: 1, maximum: 10_000 }),
  ),
  degradationThresholdMultiplier: S.Number.check(
    S.isBetween({ minimum: 1, maximum: 100 }),
  ),
  serving: S.Struct({
    speculationMode: KhalaSpeculationMode,
  }),
})
export type GymThroughputEnvironmentSpec =
  typeof GymThroughputEnvironmentSpec.Type

export const GymThroughputSampleStatus = S.Literals([
  'ok',
  'failed',
  'quota_limited',
])
export type GymThroughputSampleStatus = typeof GymThroughputSampleStatus.Type

export const GymThroughputSample = S.Struct({
  schemaVersion: S.Literal(GYM_THROUGHPUT_SAMPLE_SCHEMA),
  lane: BenchmarkLane,
  engine: BenchmarkEngine,
  modelRef: S.String,
  concurrency: S.Number.check(S.isBetween({ minimum: 1, maximum: 64 })),
  sampleIndex: S.Number,
  status: GymThroughputSampleStatus,
  ttftMs: ThroughputMeasuredNumber,
  totalWallClockMs: ThroughputMeasuredNumber,
  perceivedTps: ThroughputMeasuredNumber,
  interTokenLatencyMs: ThroughputMeasuredNumber,
  completionTokens: ThroughputMeasuredNumber,
  speculationMode: KhalaSpeculationMode,
  speculationAcceptanceRate: ThroughputMeasuredNumber,
  errorClass: S.optional(S.String),
})
export type GymThroughputSample = typeof GymThroughputSample.Type

export const decodeGymThroughputEnvironmentSpec = S.decodeUnknownSync(
  GymThroughputEnvironmentSpec,
)
export const decodeGymThroughputSample = S.decodeUnknownSync(
  GymThroughputSample,
)

export type GymThroughputMetricSummary = Readonly<{
  p50: number | null
  p90: number | null
  p99: number | null
  mean: number | null
  sampleCount: number
}>

export type GymThroughputConcurrencyPoint = Readonly<{
  concurrency: number
  totalSamples: number
  okSamples: number
  failedSamples: number
  quotaLimitedSamples: number
  ttftMs: GymThroughputMetricSummary
  totalWallClockMs: GymThroughputMetricSummary
  perceivedTps: GymThroughputMetricSummary
  interTokenLatencyMs: GymThroughputMetricSummary
  completionTokens: GymThroughputMetricSummary
  aggregateTps: number | null
  speculationAcceptanceRate: GymThroughputMetricSummary
}>

export type GymThroughputDegradationReason =
  | 'latency_degraded'
  | 'quota_limited'
  | 'not_detected'

export type GymThroughputDegradationPoint = Readonly<{
  concurrency: number | null
  reason: GymThroughputDegradationReason
}>

export type GymThroughputLaneReport = Readonly<{
  lane: BenchmarkLane
  engine: BenchmarkEngine
  modelRef: string
  promptProfile: string
  concurrencyRamp: ReadonlyArray<number>
  samplesPerConcurrency: number
  degradationThresholdMultiplier: number
  speculationMode: KhalaSpeculationMode
  concurrencyPoints: ReadonlyArray<GymThroughputConcurrencyPoint>
  degradation: GymThroughputDegradationPoint
}>

export type GymThroughputReport = Readonly<{
  schemaVersion: typeof GYM_THROUGHPUT_REPORT_SCHEMA
  generatedAt: string
  environmentRef: typeof GYM_THROUGHPUT_ENVIRONMENT_REF
  lanes: ReadonlyArray<GymThroughputLaneReport>
}>

const measuredValues = (
  samples: ReadonlyArray<GymThroughputSample>,
  pick: (sample: GymThroughputSample) => MeasuredNumber,
): ReadonlyArray<number> => {
  const out: Array<number> = []
  for (const sample of samples) {
    if (sample.status !== 'ok') {
      continue
    }
    const value = pick(sample)
    if (isMeasured(value)) {
      out.push(value)
    }
  }
  return out
}

export const summarizeThroughputMetric = (
  values: ReadonlyArray<number>,
): GymThroughputMetricSummary => ({
  p50: percentile(values, 50),
  p90: percentile(values, 90),
  p99: percentile(values, 99),
  mean: mean(values),
  sampleCount: values.length,
})

const normalizeMeasuredNumber = (value: MeasuredNumber): MeasuredNumber =>
  value === NOT_MEASURED ? NOT_MEASURED : measured(value)

const aggregateConcurrencyPoint = (
  concurrency: number,
  samples: ReadonlyArray<GymThroughputSample>,
): GymThroughputConcurrencyPoint => {
  const normalized = samples.map(sample => ({
    ...sample,
    ttftMs: normalizeMeasuredNumber(sample.ttftMs),
    totalWallClockMs: normalizeMeasuredNumber(sample.totalWallClockMs),
    perceivedTps: normalizeMeasuredNumber(sample.perceivedTps),
    interTokenLatencyMs: normalizeMeasuredNumber(sample.interTokenLatencyMs),
    completionTokens: normalizeMeasuredNumber(sample.completionTokens),
    speculationAcceptanceRate: normalizeMeasuredNumber(
      sample.speculationAcceptanceRate,
    ),
  }))
  const okSamples = normalized.filter(sample => sample.status === 'ok')
  const failedSamples = normalized.filter(sample => sample.status === 'failed')
  const quotaLimitedSamples = normalized.filter(
    sample => sample.status === 'quota_limited',
  )
  const tpsValues = measuredValues(normalized, sample => sample.perceivedTps)

  return {
    concurrency,
    totalSamples: normalized.length,
    okSamples: okSamples.length,
    failedSamples: failedSamples.length,
    quotaLimitedSamples: quotaLimitedSamples.length,
    ttftMs: summarizeThroughputMetric(
      measuredValues(normalized, sample => sample.ttftMs),
    ),
    totalWallClockMs: summarizeThroughputMetric(
      measuredValues(normalized, sample => sample.totalWallClockMs),
    ),
    perceivedTps: summarizeThroughputMetric(tpsValues),
    interTokenLatencyMs: summarizeThroughputMetric(
      measuredValues(normalized, sample => sample.interTokenLatencyMs),
    ),
    completionTokens: summarizeThroughputMetric(
      measuredValues(normalized, sample => sample.completionTokens),
    ),
    aggregateTps:
      tpsValues.length === 0
        ? null
        : tpsValues.reduce((sum, value) => sum + value, 0),
    speculationAcceptanceRate: summarizeThroughputMetric(
      measuredValues(normalized, sample => sample.speculationAcceptanceRate),
    ),
  }
}

const firstMeasuredLatencyPoint = (
  points: ReadonlyArray<GymThroughputConcurrencyPoint>,
): GymThroughputConcurrencyPoint | null =>
  points.find(point => point.totalWallClockMs.p90 !== null) ?? null

export const detectThroughputDegradation = (
  points: ReadonlyArray<GymThroughputConcurrencyPoint>,
  thresholdMultiplier: number,
): GymThroughputDegradationPoint => {
  for (const point of points) {
    if (point.quotaLimitedSamples > 0) {
      return { concurrency: point.concurrency, reason: 'quota_limited' }
    }
  }

  const baseline = firstMeasuredLatencyPoint(points)
  if (baseline === null || baseline.totalWallClockMs.p90 === null) {
    return { concurrency: null, reason: 'not_detected' }
  }
  const baselineP90 = baseline.totalWallClockMs.p90

  for (const point of points) {
    if (point.concurrency <= baseline.concurrency) {
      continue
    }
    const p90 = point.totalWallClockMs.p90
    if (p90 !== null && p90 >= baselineP90 * thresholdMultiplier) {
      return { concurrency: point.concurrency, reason: 'latency_degraded' }
    }
  }

  return { concurrency: null, reason: 'not_detected' }
}

const specKey = (spec: GymThroughputEnvironmentSpec): string =>
  `${spec.target.lane}::${spec.target.engine}::${spec.target.modelRef}`

const sampleKey = (sample: GymThroughputSample): string =>
  `${sample.lane}::${sample.engine}::${sample.modelRef}`

export const buildGymThroughputReport = (input: {
  generatedAt: string
  specs: ReadonlyArray<GymThroughputEnvironmentSpec>
  samples: ReadonlyArray<GymThroughputSample>
}): GymThroughputReport => {
  const samplesByTarget = new Map<string, Array<GymThroughputSample>>()
  for (const sample of input.samples) {
    const key = sampleKey(sample)
    const existing = samplesByTarget.get(key)
    if (existing === undefined) {
      samplesByTarget.set(key, [decodeGymThroughputSample(sample)])
    } else {
      existing.push(decodeGymThroughputSample(sample))
    }
  }

  const lanes = input.specs.map(specInput => {
    const spec = decodeGymThroughputEnvironmentSpec(specInput)
    const samplesForSpec = samplesByTarget.get(specKey(spec)) ?? []
    const concurrencyPoints = spec.concurrencyRamp.map(concurrency =>
      aggregateConcurrencyPoint(
        concurrency,
        samplesForSpec.filter(sample => sample.concurrency === concurrency),
      ),
    )

    return {
      lane: spec.target.lane,
      engine: spec.target.engine,
      modelRef: spec.target.modelRef,
      promptProfile: spec.promptProfile,
      concurrencyRamp: [...spec.concurrencyRamp],
      samplesPerConcurrency: spec.samplesPerConcurrency,
      degradationThresholdMultiplier: spec.degradationThresholdMultiplier,
      speculationMode: spec.serving.speculationMode,
      concurrencyPoints,
      degradation: detectThroughputDegradation(
        concurrencyPoints,
        spec.degradationThresholdMultiplier,
      ),
    }
  })

  return {
    schemaVersion: GYM_THROUGHPUT_REPORT_SCHEMA,
    generatedAt: input.generatedAt,
    environmentRef: GYM_THROUGHPUT_ENVIRONMENT_REF,
    lanes,
  }
}
