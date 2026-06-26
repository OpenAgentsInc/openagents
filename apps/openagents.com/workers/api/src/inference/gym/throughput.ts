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

import { BenchmarkEngine, BenchmarkLane } from '../benchmark'
import { mean, percentile } from '../benchmark/report'
import { KhalaSpeculationMode } from '../khala-speculation'
import type { MeasuredNumber } from '../khala-telemetry'
import { NOT_MEASURED, isMeasured, measured } from '../khala-telemetry'

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

export const GymThroughputQuantizationMode = S.Literals([
  'none',
  'nvfp4',
  'fp8',
  'int8',
  'nf4',
  'awq',
  'gptq',
])
export type GymThroughputQuantizationMode =
  typeof GymThroughputQuantizationMode.Type

export const GymThroughputLowBatchSpeculativeDecodingPolicy = S.Struct({
  policy: S.Literals(['disabled', 'enabled_below_batch']),
  maxBatchSize: S.Number.check(S.isBetween({ minimum: 1, maximum: 64 })),
  mode: KhalaSpeculationMode,
})
export type GymThroughputLowBatchSpeculativeDecodingPolicy =
  typeof GymThroughputLowBatchSpeculativeDecodingPolicy.Type

export const GymThroughputKvHeadroomGate = S.Struct({
  minFreeKvCachePercent: S.Number.check(
    S.isBetween({ minimum: 0, maximum: 100 }),
  ),
  action: S.Literals(['skip', 'degrade_max_num_seqs', 'fail']),
})
export type GymThroughputKvHeadroomGate =
  typeof GymThroughputKvHeadroomGate.Type

export const GymThroughputLeverExpectation = S.Struct({
  label: S.String,
  maxNumSeqs: S.Number.check(S.isBetween({ minimum: 1, maximum: 4096 })),
  enablePrefixCaching: S.Boolean,
  enableChunkedPrefill: S.Boolean,
  lowBatchSpeculativeDecoding: GymThroughputLowBatchSpeculativeDecodingPolicy,
  quantization: S.Struct({
    mode: GymThroughputQuantizationMode,
    qualityGateRef: S.String,
  }),
  kvHeadroomGate: GymThroughputKvHeadroomGate,
})
export type GymThroughputLeverExpectation =
  typeof GymThroughputLeverExpectation.Type

export const GymThroughputLeverActual = S.Struct({
  label: S.String,
  maxNumSeqs: S.Number.check(S.isBetween({ minimum: 1, maximum: 4096 })),
  enablePrefixCaching: S.Boolean,
  enableChunkedPrefill: S.Boolean,
  lowBatchSpeculativeDecoding: GymThroughputLowBatchSpeculativeDecodingPolicy,
  quantization: S.Struct({
    mode: GymThroughputQuantizationMode,
    qualityGateRef: S.String,
    gateStatus: S.Literals(['passed', 'failed', 'not_checked']),
  }),
  kvHeadroomGate: S.Struct({
    minFreeKvCachePercent: S.Number.check(
      S.isBetween({ minimum: 0, maximum: 100 }),
    ),
    action: S.Literals(['skip', 'degrade_max_num_seqs', 'fail']),
    observedFreeKvCachePercent: ThroughputMeasuredNumber,
    gateStatus: S.Literals(['passed', 'failed', 'not_checked']),
  }),
})
export type GymThroughputLeverActual = typeof GymThroughputLeverActual.Type

export const GymThroughputOptimizationSweep = S.Struct({
  sweepRef: S.String,
  publicSafeSummary: S.String,
  maxNumSeqsValues: S.Array(
    S.Number.check(S.isBetween({ minimum: 1, maximum: 4096 })),
  ),
  expectedLevers: S.Array(GymThroughputLeverExpectation),
})
export type GymThroughputOptimizationSweep =
  typeof GymThroughputOptimizationSweep.Type

const glmVllmLever = (maxNumSeqs: number): GymThroughputLeverExpectation => ({
  label: `vllm.max_num_seqs.${maxNumSeqs}.prefix_cache.chunked_prefill.nvfp4`,
  maxNumSeqs,
  enablePrefixCaching: true,
  enableChunkedPrefill: true,
  lowBatchSpeculativeDecoding: {
    policy: 'enabled_below_batch',
    maxBatchSize: 4,
    mode: 'n_gram',
  },
  quantization: {
    mode: 'nvfp4',
    qualityGateRef: 'gate.gym.glm_52.reap_504b.nvfp4.accepted_outcome.v1',
  },
  kvHeadroomGate: {
    minFreeKvCachePercent: 15,
    action: 'skip',
  },
})

export const GLM_VLLM_THROUGHPUT_OPTIMIZATION_SWEEP: GymThroughputOptimizationSweep =
  {
    sweepRef: 'sweep.gym.glm_52.vllm_throughput_levers.v1',
    publicSafeSummary:
      'GLM vLLM declarative throughput sweep over max-num-seqs, prefix caching, chunked prefill, low-batch speculative decoding, quantization, and KV headroom gates.',
    maxNumSeqsValues: [2, 4, 8, 16],
    expectedLevers: [2, 4, 8, 16].map(glmVllmLever),
  }

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
    optimizationSweep: S.optional(GymThroughputOptimizationSweep),
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
  actualThroughputLevers: S.optional(GymThroughputLeverActual),
  errorClass: S.optional(S.String),
})
export type GymThroughputSample = typeof GymThroughputSample.Type

export const decodeGymThroughputEnvironmentSpec = S.decodeUnknownSync(
  GymThroughputEnvironmentSpec,
)
export const decodeGymThroughputSample =
  S.decodeUnknownSync(GymThroughputSample)

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
  actualThroughputLevers: ReadonlyArray<GymThroughputLeverActual>
}>

export type GymThroughputDegradationReason =
  | 'latency_degraded'
  | 'quota_limited'
  | 'not_detected'

export type GymThroughputDegradationPoint = Readonly<{
  concurrency: number | null
  reason: GymThroughputDegradationReason
}>

export const GymThroughputRolloutBlocker = S.Literals([
  'missing_optimization_sweep',
  'missing_baseline_max_num_seqs_2_measurement',
  'missing_candidate_measurement',
  'missing_measured_itl_p90',
  'missing_measured_aggregate_tps',
  'interactive_itl_slo_exceeded',
  'no_measured_throughput_lift',
])
export type GymThroughputRolloutBlocker =
  typeof GymThroughputRolloutBlocker.Type

export const GymThroughputRolloutVllmFlag = S.Struct({
  name: S.String,
  value: S.optional(S.String),
})
export type GymThroughputRolloutVllmFlag =
  typeof GymThroughputRolloutVllmFlag.Type

export const GymThroughputRolloutSelection = S.Struct({
  label: S.String,
  maxNumSeqs: S.Number.check(S.isBetween({ minimum: 1, maximum: 4096 })),
  concurrency: S.Number.check(S.isBetween({ minimum: 1, maximum: 64 })),
  aggregateTps: S.Number,
  baselineAggregateTps: S.Number,
  aggregateTpsLiftPercent: S.Number,
  interTokenLatencyP90Ms: S.Number,
  baselineInterTokenLatencyP90Ms: S.Number,
  ttftP90Ms: ThroughputMeasuredNumber,
  prefixCachingEnabled: S.Boolean,
  chunkedPrefillEnabled: S.Boolean,
  lowBatchSpeculativeDecoding: GymThroughputLowBatchSpeculativeDecodingPolicy,
  vllmFlags: S.Array(GymThroughputRolloutVllmFlag),
})
export type GymThroughputRolloutSelection =
  typeof GymThroughputRolloutSelection.Type

export const GymThroughputRolloutRecommendation = S.Struct({
  schemaVersion: S.Literal(
    'openagents.gym.throughput_rollout_recommendation.v1',
  ),
  environmentRef: S.Literal(GYM_THROUGHPUT_ENVIRONMENT_REF),
  sweepRef: S.String,
  target: GymThroughputTarget,
  publicSafe: S.Literal(true),
  decisionGrade: S.Boolean,
  selection: S.Union([GymThroughputRolloutSelection, S.Null]),
  blockers: S.Array(GymThroughputRolloutBlocker),
})
export type GymThroughputRolloutRecommendation =
  typeof GymThroughputRolloutRecommendation.Type

export const GymThroughputOwnerArmedRolloutRunBlocker = S.Literals([
  'recommendation_not_decision_grade',
  'recommendation_has_blockers',
  'missing_selection',
  'missing_owner_arm_ref',
])
export type GymThroughputOwnerArmedRolloutRunBlocker =
  typeof GymThroughputOwnerArmedRolloutRunBlocker.Type

export const GymThroughputOwnerArmedRolloutFlagApplicationPlan = S.Struct({
  target: GymThroughputTarget,
  sweepRef: S.String,
  ownerArmRef: S.String,
  applyMode: S.Literal('owner_armed_manual'),
  selection: GymThroughputRolloutSelection,
  vllmFlags: S.Array(GymThroughputRolloutVllmFlag),
})
export type GymThroughputOwnerArmedRolloutFlagApplicationPlan =
  typeof GymThroughputOwnerArmedRolloutFlagApplicationPlan.Type

export const GymThroughputOwnerArmedRolloutRunArtifact = S.Struct({
  schemaVersion: S.Literal(
    'openagents.gym.throughput_owner_armed_rollout_run.v1',
  ),
  generatedAt: S.String,
  environmentRef: S.Literal(GYM_THROUGHPUT_ENVIRONMENT_REF),
  publicSafe: S.Literal(true),
  status: S.Literals(['blocked', 'ready_to_apply']),
  recommendation: GymThroughputRolloutRecommendation,
  ownerArmRef: S.Union([S.String, S.Null]),
  canApplyLiveFlags: S.Boolean,
  applicationPlan: S.Union([
    GymThroughputOwnerArmedRolloutFlagApplicationPlan,
    S.Null,
  ]),
  blockers: S.Array(GymThroughputOwnerArmedRolloutRunBlocker),
})
export type GymThroughputOwnerArmedRolloutRunArtifact =
  typeof GymThroughputOwnerArmedRolloutRunArtifact.Type

export type GymThroughputLaneReport = Readonly<{
  lane: BenchmarkLane
  engine: BenchmarkEngine
  modelRef: string
  promptProfile: string
  concurrencyRamp: ReadonlyArray<number>
  samplesPerConcurrency: number
  degradationThresholdMultiplier: number
  speculationMode: KhalaSpeculationMode
  optimizationSweep: GymThroughputOptimizationSweep | null
  expectedThroughputLevers: ReadonlyArray<GymThroughputLeverExpectation>
  throughputLeverLabels: ReadonlyArray<string>
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
    actualThroughputLevers: normalized.flatMap(sample =>
      sample.actualThroughputLevers === undefined
        ? []
        : [sample.actualThroughputLevers],
    ),
  }
}

const firstMeasuredLatencyPoint = (
  points: ReadonlyArray<GymThroughputConcurrencyPoint>,
): GymThroughputConcurrencyPoint | null =>
  points.find(point => point.totalWallClockMs.p90 !== null) ?? null

const actualLeverForPoint = (
  point: GymThroughputConcurrencyPoint,
): GymThroughputLeverActual | null =>
  point.actualThroughputLevers.find(
    lever =>
      lever.quantization.gateStatus === 'passed' &&
      lever.kvHeadroomGate.gateStatus === 'passed',
  ) ?? null

const vllmFlagsForLever = (
  lever: GymThroughputLeverActual,
): ReadonlyArray<GymThroughputRolloutVllmFlag> => {
  const baseFlags: Array<GymThroughputRolloutVllmFlag> = [
    { name: '--max-num-seqs', value: String(lever.maxNumSeqs) },
  ]
  if (lever.enablePrefixCaching) {
    baseFlags.push({ name: '--enable-prefix-caching' })
  }
  if (lever.enableChunkedPrefill) {
    baseFlags.push({ name: '--enable-chunked-prefill' })
  }
  if (lever.lowBatchSpeculativeDecoding.policy === 'enabled_below_batch') {
    baseFlags.push({
      name: '--speculative-config',
      value: JSON.stringify({
        method: lever.lowBatchSpeculativeDecoding.mode,
        disable_at_batch_size:
          lever.lowBatchSpeculativeDecoding.maxBatchSize + 1,
      }),
    })
  }
  return baseFlags
}

const pointWithMeasuredBaseline = (
  points: ReadonlyArray<GymThroughputConcurrencyPoint>,
): GymThroughputConcurrencyPoint | null =>
  points.find(point => {
    const lever = actualLeverForPoint(point)
    return (
      lever?.maxNumSeqs === 2 &&
      point.aggregateTps !== null &&
      point.interTokenLatencyMs.p90 !== null
    )
  }) ?? null

const rolloutCandidateSelection = (input: {
  baseline: GymThroughputConcurrencyPoint
  point: GymThroughputConcurrencyPoint
  lever: GymThroughputLeverActual
}): GymThroughputRolloutSelection | null => {
  if (
    input.point.aggregateTps === null ||
    input.baseline.aggregateTps === null ||
    input.point.interTokenLatencyMs.p90 === null ||
    input.baseline.interTokenLatencyMs.p90 === null
  ) {
    return null
  }
  return {
    label: input.lever.label,
    maxNumSeqs: input.lever.maxNumSeqs,
    concurrency: input.point.concurrency,
    aggregateTps: input.point.aggregateTps,
    baselineAggregateTps: input.baseline.aggregateTps,
    aggregateTpsLiftPercent:
      ((input.point.aggregateTps - input.baseline.aggregateTps) /
        input.baseline.aggregateTps) *
      100,
    interTokenLatencyP90Ms: input.point.interTokenLatencyMs.p90,
    baselineInterTokenLatencyP90Ms: input.baseline.interTokenLatencyMs.p90,
    ttftP90Ms:
      input.point.ttftMs.p90 === null ? NOT_MEASURED : input.point.ttftMs.p90,
    prefixCachingEnabled: input.lever.enablePrefixCaching,
    chunkedPrefillEnabled: input.lever.enableChunkedPrefill,
    lowBatchSpeculativeDecoding: input.lever.lowBatchSpeculativeDecoding,
    vllmFlags: [...vllmFlagsForLever(input.lever)],
  }
}

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

export const recommendGymThroughputRollout = (input: {
  report: GymThroughputReport
  lane: BenchmarkLane
  maxInteractiveItlP90Multiplier: number
}): GymThroughputRolloutRecommendation => {
  const lane = input.report.lanes.find(row => row.lane === input.lane)
  const sweepRef =
    lane?.optimizationSweep?.sweepRef ?? 'sweep.gym.throughput.missing'
  const target =
    lane === undefined
      ? {
          lane: input.lane,
          engine: 'vllm' as const,
          modelRef: 'missing',
        }
      : {
          lane: lane.lane,
          engine: lane.engine,
          modelRef: lane.modelRef,
        }
  const missingSweepBlockers: ReadonlyArray<GymThroughputRolloutBlocker> =
    lane?.optimizationSweep === undefined || lane.optimizationSweep === null
      ? ['missing_optimization_sweep']
      : []
  const baseline =
    lane === undefined
      ? null
      : pointWithMeasuredBaseline(lane.concurrencyPoints)
  const baselineBlockers: ReadonlyArray<GymThroughputRolloutBlocker> =
    baseline === null ? ['missing_baseline_max_num_seqs_2_measurement'] : []

  if (lane === undefined || baseline === null) {
    return {
      schemaVersion: 'openagents.gym.throughput_rollout_recommendation.v1',
      environmentRef: GYM_THROUGHPUT_ENVIRONMENT_REF,
      sweepRef,
      target,
      publicSafe: true,
      decisionGrade: false,
      selection: null,
      blockers: [...missingSweepBlockers, ...baselineBlockers],
    }
  }

  const candidates = lane.concurrencyPoints.flatMap(point => {
    const lever = actualLeverForPoint(point)
    if (lever === null || lever.maxNumSeqs <= 2) {
      return []
    }
    const selection = rolloutCandidateSelection({ baseline, point, lever })
    if (selection === null) {
      return []
    }
    return [selection]
  })
  const measuredCandidates = candidates.filter(
    candidate =>
      candidate.interTokenLatencyP90Ms <=
      candidate.baselineInterTokenLatencyP90Ms *
        input.maxInteractiveItlP90Multiplier,
  )
  const best =
    [...measuredCandidates].sort(
      (
        left: GymThroughputRolloutSelection,
        right: GymThroughputRolloutSelection,
      ) => right.aggregateTps - left.aggregateTps,
    )[0] ?? null
  const blockers: Array<GymThroughputRolloutBlocker> = [...missingSweepBlockers]
  if (candidates.length === 0) {
    blockers.push('missing_candidate_measurement')
  }
  if (
    candidates.some(
      candidate =>
        candidate.interTokenLatencyP90Ms >
        candidate.baselineInterTokenLatencyP90Ms *
          input.maxInteractiveItlP90Multiplier,
    ) &&
    best === null
  ) {
    blockers.push('interactive_itl_slo_exceeded')
  }
  if (best !== null && best.aggregateTpsLiftPercent <= 0) {
    blockers.push('no_measured_throughput_lift')
  }
  if (
    lane.concurrencyPoints.some(
      point =>
        actualLeverForPoint(point) !== null &&
        point.interTokenLatencyMs.p90 === null,
    )
  ) {
    blockers.push('missing_measured_itl_p90')
  }
  if (
    lane.concurrencyPoints.some(
      point =>
        actualLeverForPoint(point) !== null && point.aggregateTps === null,
    )
  ) {
    blockers.push('missing_measured_aggregate_tps')
  }

  const selection =
    best === null || best.aggregateTpsLiftPercent <= 0 ? null : best

  return {
    schemaVersion: 'openagents.gym.throughput_rollout_recommendation.v1',
    environmentRef: GYM_THROUGHPUT_ENVIRONMENT_REF,
    sweepRef,
    target,
    publicSafe: true,
    decisionGrade: selection !== null && blockers.length === 0,
    selection,
    blockers,
  }
}

const normalizeOwnerArmRef = (ownerArmRef: string | null | undefined) => {
  const trimmed = ownerArmRef?.trim()
  return trimmed === undefined || trimmed.length === 0 ? null : trimmed
}

export const buildGymThroughputOwnerArmedRolloutRunArtifact = (input: {
  generatedAt: string
  recommendation: GymThroughputRolloutRecommendation
  ownerArmRef?: string | null
}): GymThroughputOwnerArmedRolloutRunArtifact => {
  const ownerArmRef = normalizeOwnerArmRef(input.ownerArmRef)
  const blockers: Array<GymThroughputOwnerArmedRolloutRunBlocker> = []
  if (input.recommendation.decisionGrade !== true) {
    blockers.push('recommendation_not_decision_grade')
  }
  if (input.recommendation.blockers.length > 0) {
    blockers.push('recommendation_has_blockers')
  }
  if (input.recommendation.selection === null) {
    blockers.push('missing_selection')
  }
  if (ownerArmRef === null) {
    blockers.push('missing_owner_arm_ref')
  }

  if (
    blockers.length > 0 ||
    input.recommendation.selection === null ||
    ownerArmRef === null
  ) {
    return {
      schemaVersion: 'openagents.gym.throughput_owner_armed_rollout_run.v1',
      generatedAt: input.generatedAt,
      environmentRef: GYM_THROUGHPUT_ENVIRONMENT_REF,
      publicSafe: true,
      status: 'blocked',
      recommendation: input.recommendation,
      ownerArmRef,
      canApplyLiveFlags: false,
      applicationPlan: null,
      blockers,
    }
  }

  return {
    schemaVersion: 'openagents.gym.throughput_owner_armed_rollout_run.v1',
    generatedAt: input.generatedAt,
    environmentRef: GYM_THROUGHPUT_ENVIRONMENT_REF,
    publicSafe: true,
    status: 'ready_to_apply',
    recommendation: input.recommendation,
    ownerArmRef,
    canApplyLiveFlags: true,
    applicationPlan: {
      target: input.recommendation.target,
      sweepRef: input.recommendation.sweepRef,
      ownerArmRef,
      applyMode: 'owner_armed_manual',
      selection: input.recommendation.selection,
      vllmFlags: [...input.recommendation.selection.vllmFlags],
    },
    blockers,
  }
}

const specKey = (spec: GymThroughputEnvironmentSpec): string =>
  `${spec.target.lane}::${spec.target.engine}::${spec.target.modelRef}`

const sampleKey = (sample: GymThroughputSample): string =>
  `${sample.lane}::${sample.engine}::${sample.modelRef}`

export const expandGymThroughputOptimizationSweep = (
  spec: GymThroughputEnvironmentSpec,
): ReadonlyArray<GymThroughputLeverExpectation> => {
  const decoded = decodeGymThroughputEnvironmentSpec(spec)
  return decoded.serving.optimizationSweep?.expectedLevers ?? []
}

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
      optimizationSweep: spec.serving.optimizationSweep ?? null,
      expectedThroughputLevers: expandGymThroughputOptimizationSweep(spec),
      throughputLeverLabels: expandGymThroughputOptimizationSweep(spec).map(
        lever => lever.label,
      ),
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
