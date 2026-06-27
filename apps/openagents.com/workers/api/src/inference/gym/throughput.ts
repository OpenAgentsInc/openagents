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
  baselineTtftP90Ms: ThroughputMeasuredNumber,
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

export const GymThroughputRolloutProgressStatus = S.Literals([
  'blocked',
  'awaiting_owner_arm',
  'ready_to_apply',
  'applying_owner_armed',
  'measured_lift',
])
export type GymThroughputRolloutProgressStatus =
  typeof GymThroughputRolloutProgressStatus.Type

export const GymThroughputRolloutMeasuredConfiguration = S.Struct({
  maxNumSeqs: S.Number.check(S.isBetween({ minimum: 1, maximum: 4096 })),
  prefixCachingEnabled: S.Boolean,
  chunkedPrefillEnabled: S.Boolean,
  speculativeDecodeEnabled: S.Boolean,
  lowBatchSpeculativeDecoding: GymThroughputLowBatchSpeculativeDecodingPolicy,
})
export type GymThroughputRolloutMeasuredConfiguration =
  typeof GymThroughputRolloutMeasuredConfiguration.Type

export const GymThroughputRolloutMeasuredPoint = S.Struct({
  configuration: GymThroughputRolloutMeasuredConfiguration,
  aggregateTokensPerSecond: ThroughputMeasuredNumber,
  interTokenLatencyP90Ms: ThroughputMeasuredNumber,
  ttftP90Ms: ThroughputMeasuredNumber,
})
export type GymThroughputRolloutMeasuredPoint =
  typeof GymThroughputRolloutMeasuredPoint.Type

export const GymThroughputRolloutExpectedVsActualEvidence = S.Struct({
  expectedAfter: GymThroughputRolloutMeasuredPoint,
  actualAfter: GymThroughputRolloutMeasuredPoint,
  expectedAggregateTpsLiftPercent: ThroughputMeasuredNumber,
  actualAggregateTpsLiftPercent: ThroughputMeasuredNumber,
  maxNumSeqsMatches: S.Boolean,
  prefixCachingMatches: S.Boolean,
  chunkedPrefillMatches: S.Boolean,
  speculativeDecodingMatches: S.Boolean,
})
export type GymThroughputRolloutExpectedVsActualEvidence =
  typeof GymThroughputRolloutExpectedVsActualEvidence.Type

export const GymThroughputRolloutMeasurementEvidence = S.Struct({
  schemaVersion: S.Literal(
    'openagents.gym.throughput_rollout_measurement_evidence.v1',
  ),
  evidenceRef: S.String,
  evidenceKind: S.Literal('measured_rollout'),
  publicSafe: S.Literal(true),
  liveVllmFlags: S.Array(GymThroughputRolloutVllmFlag),
  before: GymThroughputRolloutMeasuredPoint,
  after: GymThroughputRolloutMeasuredPoint,
  expectedVsActual: GymThroughputRolloutExpectedVsActualEvidence,
  publicEvidenceRefs: S.Array(S.String),
})
export type GymThroughputRolloutMeasurementEvidence =
  typeof GymThroughputRolloutMeasurementEvidence.Type

export const GymThroughputRolloutProgressEvidence = S.Struct({
  schemaVersion: S.Literal(
    'openagents.gym.throughput_rollout_progress_evidence.v1',
  ),
  rolloutRef: S.String,
  observedAt: S.String,
  status: GymThroughputRolloutProgressStatus,
  ownerArmRef: S.Union([S.String, S.Null]),
  progressPercent: S.Number.check(S.isBetween({ minimum: 0, maximum: 100 })),
  publicEvidenceRefs: S.Array(S.String),
  baselineAggregateTps: ThroughputMeasuredNumber,
  measuredAggregateTps: ThroughputMeasuredNumber,
  measuredThroughputLiftPercent: ThroughputMeasuredNumber,
  rolloutMeasurementEvidence: S.optional(
    GymThroughputRolloutMeasurementEvidence,
  ),
})
export type GymThroughputRolloutProgressEvidence =
  typeof GymThroughputRolloutProgressEvidence.Type

export const GymThroughputRolloutReadoutBlocker = S.Literals([
  'rollout_run_blocked',
  'missing_progress_evidence',
  'rollout_progress_incomplete',
  'owner_arm_ref_mismatch',
  'missing_rollout_measurement_evidence',
  'rollout_measurement_incomplete',
  'rollout_measurement_selection_mismatch',
  'live_engine_flags_mismatch',
  'expected_actual_evidence_mismatch',
  'progress_measurement_mismatch',
  'measured_lift_missing',
  'measured_lift_not_positive',
  'glm_fleet_serving_not_ready',
  'glm_fleet_durability_acceptance_not_complete',
])
export type GymThroughputRolloutReadoutBlocker =
  typeof GymThroughputRolloutReadoutBlocker.Type

export const GymThroughputGlmFleetServingStatus = S.Literals([
  'degraded',
  'ready',
  'unavailable',
])
export type GymThroughputGlmFleetServingStatus =
  typeof GymThroughputGlmFleetServingStatus.Type

export const GymThroughputGlmFleetDurabilityAcceptanceStatus = S.Literals([
  'blocked',
  'complete',
  'incomplete',
])
export type GymThroughputGlmFleetDurabilityAcceptanceStatus =
  typeof GymThroughputGlmFleetDurabilityAcceptanceStatus.Type

export const GymThroughputGlmFleetDurabilityDependencyEvidence = S.Struct({
  schemaVersion: S.Literal(
    'openagents.gym.throughput_glm_fleet_durability_dependency_evidence.v1',
  ),
  issueRef: S.Literal('github.issue.OpenAgentsInc.openagents.6311'),
  publicSafe: S.Literal(true),
  servingStatus: GymThroughputGlmFleetServingStatus,
  acceptanceStatus: GymThroughputGlmFleetDurabilityAcceptanceStatus,
  servingCapacitySummary: S.String,
  readyReplicaCount: S.Number.check(S.isBetween({ minimum: 0, maximum: 4096 })),
  reclaimedReplicaCount: S.Number.check(
    S.isBetween({ minimum: 0, maximum: 4096 }),
  ),
  warmOrReadyMaxInflight: S.Number.check(
    S.isBetween({ minimum: 0, maximum: 4096 }),
  ),
  servingReadyButAcceptanceNotComplete: S.Boolean,
  blockerRefs: S.Array(S.String),
  remainingDurabilityBlockerRefs: S.Array(S.String),
  publicEvidenceRefs: S.Array(S.String),
})
export type GymThroughputGlmFleetDurabilityDependencyEvidence =
  typeof GymThroughputGlmFleetDurabilityDependencyEvidence.Type

export const GymThroughputRolloutEvidenceChecklistCheck = S.Literals([
  'owner_arm_ref',
  'live_max_num_seqs',
  'live_vllm_flags',
  'prefix_cache',
  'chunked_prefill',
  'speculative_decode',
  'before_tokens_per_second',
  'after_tokens_per_second',
  'before_inter_token_latency_p90',
  'after_inter_token_latency_p90',
  'before_ttft_p90',
  'after_ttft_p90',
  'expected_vs_actual_configuration',
  'expected_vs_actual_lift',
  'rollout_progress_status',
  'rollout_progress_percent',
  'progress_totals_match_measurement',
  'public_evidence_refs',
  'glm_fleet_serving_ready',
  'glm_fleet_durability_6311_acceptance',
])
export type GymThroughputRolloutEvidenceChecklistCheck =
  typeof GymThroughputRolloutEvidenceChecklistCheck.Type

export const GymThroughputRolloutEvidenceChecklistStatus = S.Literals([
  'satisfied',
  'missing',
  'mismatch',
])
export type GymThroughputRolloutEvidenceChecklistStatus =
  typeof GymThroughputRolloutEvidenceChecklistStatus.Type

const GymThroughputRolloutEvidenceChecklistValue = S.Union([
  S.String,
  S.Number,
  S.Boolean,
  S.Null,
])
export type GymThroughputRolloutEvidenceChecklistValue =
  typeof GymThroughputRolloutEvidenceChecklistValue.Type

export const GymThroughputRolloutEvidenceChecklistItem = S.Struct({
  check: GymThroughputRolloutEvidenceChecklistCheck,
  status: GymThroughputRolloutEvidenceChecklistStatus,
  expected: GymThroughputRolloutEvidenceChecklistValue,
  actual: GymThroughputRolloutEvidenceChecklistValue,
  publicEvidenceRefs: S.Array(S.String),
})
export type GymThroughputRolloutEvidenceChecklistItem =
  typeof GymThroughputRolloutEvidenceChecklistItem.Type

export const GymThroughputRolloutOperatorAcceptanceStatus = S.Literals([
  'accepted_for_stress_and_benchmark',
  'blocked_before_stress_and_benchmark',
])
export type GymThroughputRolloutOperatorAcceptanceStatus =
  typeof GymThroughputRolloutOperatorAcceptanceStatus.Type

export const GymThroughputRolloutOperatorAcceptanceRequirement = S.Struct({
  check: GymThroughputRolloutEvidenceChecklistCheck,
  status: GymThroughputRolloutEvidenceChecklistStatus,
  blocksIssueRefs: S.Array(S.String),
  expected: GymThroughputRolloutEvidenceChecklistValue,
  actual: GymThroughputRolloutEvidenceChecklistValue,
  publicEvidenceRefs: S.Array(S.String),
})
export type GymThroughputRolloutOperatorAcceptanceRequirement =
  typeof GymThroughputRolloutOperatorAcceptanceRequirement.Type

export const GymThroughputRolloutOperatorAcceptance = S.Struct({
  schemaVersion: S.Literal(
    'openagents.gym.throughput_rollout_operator_acceptance.v1',
  ),
  status: GymThroughputRolloutOperatorAcceptanceStatus,
  canStartIssue6317Stress: S.Boolean,
  canStartIssue6312Benchmark: S.Boolean,
  remainingChecks: S.Array(GymThroughputRolloutEvidenceChecklistCheck),
  requirements: S.Array(GymThroughputRolloutOperatorAcceptanceRequirement),
  publicEvidenceRefs: S.Array(S.String),
})
export type GymThroughputRolloutOperatorAcceptance =
  typeof GymThroughputRolloutOperatorAcceptance.Type

export const GymThroughputRolloutReadout = S.Struct({
  schemaVersion: S.Literal('openagents.gym.throughput_rollout_readout.v1'),
  generatedAt: S.String,
  environmentRef: S.Literal(GYM_THROUGHPUT_ENVIRONMENT_REF),
  publicSafe: S.Literal(true),
  metadataOnly: S.Literal(true),
  canMutateInfrastructure: S.Literal(false),
  status: S.Literals(['blocked', 'ready', 'measured']),
  rolloutRun: GymThroughputOwnerArmedRolloutRunArtifact,
  progressEvidence: S.Union([GymThroughputRolloutProgressEvidence, S.Null]),
  rolloutMeasurementEvidence: S.Union([
    GymThroughputRolloutMeasurementEvidence,
    S.Null,
  ]),
  glmFleetDurabilityDependencyEvidence: S.Union([
    GymThroughputGlmFleetDurabilityDependencyEvidence,
    S.Null,
  ]),
  evidenceChecklist: S.Array(GymThroughputRolloutEvidenceChecklistItem),
  operatorAcceptance: GymThroughputRolloutOperatorAcceptance,
  measuredLiftPercent: ThroughputMeasuredNumber,
  blockers: S.Array(GymThroughputRolloutReadoutBlocker),
})
export type GymThroughputRolloutReadout =
  typeof GymThroughputRolloutReadout.Type

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
    baselineTtftP90Ms:
      input.baseline.ttftMs.p90 === null
        ? NOT_MEASURED
        : input.baseline.ttftMs.p90,
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

const sameNullableRef = (left: string | null, right: string | null): boolean =>
  left === right

const measuredConfigurationFromSelection = (
  selection: GymThroughputRolloutSelection,
): GymThroughputRolloutMeasuredConfiguration => ({
  maxNumSeqs: selection.maxNumSeqs,
  prefixCachingEnabled: selection.prefixCachingEnabled,
  chunkedPrefillEnabled: selection.chunkedPrefillEnabled,
  speculativeDecodeEnabled:
    selection.lowBatchSpeculativeDecoding.policy === 'enabled_below_batch',
  lowBatchSpeculativeDecoding: selection.lowBatchSpeculativeDecoding,
})

const sameLowBatchSpeculativeDecoding = (
  left: GymThroughputLowBatchSpeculativeDecodingPolicy,
  right: GymThroughputLowBatchSpeculativeDecodingPolicy,
): boolean =>
  left.policy === right.policy &&
  left.maxBatchSize === right.maxBatchSize &&
  left.mode === right.mode

const sameMeasuredConfiguration = (
  left: GymThroughputRolloutMeasuredConfiguration,
  right: GymThroughputRolloutMeasuredConfiguration,
): boolean =>
  left.maxNumSeqs === right.maxNumSeqs &&
  left.prefixCachingEnabled === right.prefixCachingEnabled &&
  left.chunkedPrefillEnabled === right.chunkedPrefillEnabled &&
  left.speculativeDecodeEnabled === right.speculativeDecodeEnabled &&
  sameLowBatchSpeculativeDecoding(
    left.lowBatchSpeculativeDecoding,
    right.lowBatchSpeculativeDecoding,
  )

const sameMeasuredNumber = (
  left: MeasuredNumber,
  right: MeasuredNumber,
): boolean => {
  if (left === NOT_MEASURED || right === NOT_MEASURED) {
    return left === right
  }
  return Math.abs(left - right) < 0.000001
}

const rolloutMeasurementHasBeforeAfter = (
  evidence: GymThroughputRolloutMeasurementEvidence,
): boolean =>
  isMeasured(evidence.before.aggregateTokensPerSecond) &&
  isMeasured(evidence.before.interTokenLatencyP90Ms) &&
  isMeasured(evidence.before.ttftP90Ms) &&
  isMeasured(evidence.after.aggregateTokensPerSecond) &&
  isMeasured(evidence.after.interTokenLatencyP90Ms) &&
  isMeasured(evidence.after.ttftP90Ms)

const rolloutMeasurementMatchesSelection = (
  evidence: GymThroughputRolloutMeasurementEvidence,
  selection: GymThroughputRolloutSelection,
): boolean => {
  const selectedConfiguration = measuredConfigurationFromSelection(selection)
  return (
    sameMeasuredConfiguration(
      evidence.expectedVsActual.expectedAfter.configuration,
      selectedConfiguration,
    ) &&
    sameMeasuredConfiguration(
      evidence.after.configuration,
      selectedConfiguration,
    )
  )
}

const rolloutExpectedActualMatches = (
  evidence: GymThroughputRolloutMeasurementEvidence,
): boolean =>
  evidence.expectedVsActual.maxNumSeqsMatches &&
  evidence.expectedVsActual.prefixCachingMatches &&
  evidence.expectedVsActual.chunkedPrefillMatches &&
  evidence.expectedVsActual.speculativeDecodingMatches &&
  sameMeasuredConfiguration(
    evidence.expectedVsActual.actualAfter.configuration,
    evidence.after.configuration,
  )

const sameVllmFlags = (
  left: ReadonlyArray<GymThroughputRolloutVllmFlag>,
  right: ReadonlyArray<GymThroughputRolloutVllmFlag>,
): boolean => {
  if (left.length !== right.length) {
    return false
  }
  const rightKeys = new Set(
    right.map(flag => `${flag.name}\u0000${flag.value ?? ''}`),
  )
  return left.every(flag =>
    rightKeys.has(`${flag.name}\u0000${flag.value ?? ''}`),
  )
}

const vllmFlagsFingerprint = (
  flags: ReadonlyArray<GymThroughputRolloutVllmFlag>,
): string =>
  flags
    .map(flag =>
      flag.value === undefined ? flag.name : `${flag.name}=${flag.value}`,
    )
    .join(' ')

const rolloutProgressMatchesMeasurement = (
  progress: GymThroughputRolloutProgressEvidence,
  evidence: GymThroughputRolloutMeasurementEvidence,
): boolean =>
  sameMeasuredNumber(
    progress.baselineAggregateTps,
    evidence.before.aggregateTokensPerSecond,
  ) &&
  sameMeasuredNumber(
    progress.measuredAggregateTps,
    evidence.after.aggregateTokensPerSecond,
  ) &&
  sameMeasuredNumber(
    progress.measuredThroughputLiftPercent,
    evidence.expectedVsActual.actualAggregateTpsLiftPercent,
  )

const checklistItem = (
  input: Omit<
    GymThroughputRolloutEvidenceChecklistItem,
    'publicEvidenceRefs'
  > & {
    publicEvidenceRefs?: ReadonlyArray<string>
  },
): GymThroughputRolloutEvidenceChecklistItem => ({
  ...input,
  publicEvidenceRefs: [...(input.publicEvidenceRefs ?? [])],
})

const measuredChecklistStatus = (
  value: MeasuredNumber | null,
): GymThroughputRolloutEvidenceChecklistStatus =>
  value !== null && isMeasured(value) ? 'satisfied' : 'missing'

const matchedChecklistStatus = (
  available: boolean,
  matched: boolean,
): GymThroughputRolloutEvidenceChecklistStatus =>
  available ? (matched ? 'satisfied' : 'mismatch') : 'missing'

const rolloutEvidenceChecklist = (input: {
  rolloutRun: GymThroughputOwnerArmedRolloutRunArtifact
  progressEvidence: GymThroughputRolloutProgressEvidence | null
  rolloutMeasurementEvidence: GymThroughputRolloutMeasurementEvidence | null
  glmFleetDurabilityDependencyEvidence:
    | GymThroughputGlmFleetDurabilityDependencyEvidence
    | null
}): ReadonlyArray<GymThroughputRolloutEvidenceChecklistItem> => {
  const selection = input.rolloutRun.recommendation.selection
  const evidence = input.rolloutMeasurementEvidence
  const progress = input.progressEvidence
  const dependencyEvidence = input.glmFleetDurabilityDependencyEvidence
  const publicEvidenceRefs = [
    ...new Set([
      ...(progress?.publicEvidenceRefs ?? []),
      ...(evidence?.publicEvidenceRefs ?? []),
    ]),
  ]
  const dependencyChecks =
    dependencyEvidence === null
      ? []
      : [
          checklistItem({
            check: 'glm_fleet_serving_ready',
            status:
              dependencyEvidence.servingStatus === 'ready'
                ? 'satisfied'
                : 'mismatch',
            expected: 'ready',
            actual: dependencyEvidence.servingStatus,
            publicEvidenceRefs: dependencyEvidence.publicEvidenceRefs,
          }),
          checklistItem({
            check: 'glm_fleet_durability_6311_acceptance',
            status:
              dependencyEvidence.acceptanceStatus === 'complete'
                ? 'satisfied'
                : 'mismatch',
            expected: 'complete',
            actual: dependencyEvidence.acceptanceStatus,
            publicEvidenceRefs: dependencyEvidence.publicEvidenceRefs,
          }),
        ]
  const expectedConfiguration =
    selection === null ? null : measuredConfigurationFromSelection(selection)
  const actualConfiguration = evidence?.expectedVsActual.actualAfter.configuration
  const expectedVllmFlags = selection?.vllmFlags ?? []
  const actualVllmFlags = evidence?.liveVllmFlags ?? []
  const ownerArmRefStatus =
    input.rolloutRun.ownerArmRef === null
      ? 'missing'
      : progress !== null &&
          !sameNullableRef(progress.ownerArmRef, input.rolloutRun.ownerArmRef)
        ? 'mismatch'
        : 'satisfied'
  const expectedLift =
    evidence?.expectedVsActual.expectedAggregateTpsLiftPercent ?? null
  const actualLift =
    evidence?.expectedVsActual.actualAggregateTpsLiftPercent ?? null

  return [
    checklistItem({
      check: 'owner_arm_ref',
      status: ownerArmRefStatus,
      expected: input.rolloutRun.ownerArmRef,
      actual: progress?.ownerArmRef ?? null,
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'live_max_num_seqs',
      status: matchedChecklistStatus(
        evidence !== null,
        evidence?.expectedVsActual.maxNumSeqsMatches ?? false,
      ),
      expected: expectedConfiguration?.maxNumSeqs ?? null,
      actual: actualConfiguration?.maxNumSeqs ?? null,
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'live_vllm_flags',
      status: matchedChecklistStatus(
        evidence !== null,
        evidence === null
          ? false
          : sameVllmFlags(expectedVllmFlags, actualVllmFlags),
      ),
      expected:
        expectedVllmFlags.length === 0
          ? null
          : vllmFlagsFingerprint(expectedVllmFlags),
      actual:
        actualVllmFlags.length === 0
          ? null
          : vllmFlagsFingerprint(actualVllmFlags),
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'prefix_cache',
      status: matchedChecklistStatus(
        evidence !== null,
        evidence?.expectedVsActual.prefixCachingMatches ?? false,
      ),
      expected: expectedConfiguration?.prefixCachingEnabled ?? null,
      actual: actualConfiguration?.prefixCachingEnabled ?? null,
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'chunked_prefill',
      status: matchedChecklistStatus(
        evidence !== null,
        evidence?.expectedVsActual.chunkedPrefillMatches ?? false,
      ),
      expected: expectedConfiguration?.chunkedPrefillEnabled ?? null,
      actual: actualConfiguration?.chunkedPrefillEnabled ?? null,
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'speculative_decode',
      status: matchedChecklistStatus(
        evidence !== null,
        evidence?.expectedVsActual.speculativeDecodingMatches ?? false,
      ),
      expected: expectedConfiguration?.speculativeDecodeEnabled ?? null,
      actual: actualConfiguration?.speculativeDecodeEnabled ?? null,
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'before_tokens_per_second',
      status: measuredChecklistStatus(
        evidence?.before.aggregateTokensPerSecond ?? null,
      ),
      expected: selection?.baselineAggregateTps ?? null,
      actual: evidence?.before.aggregateTokensPerSecond ?? null,
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'after_tokens_per_second',
      status: measuredChecklistStatus(
        evidence?.after.aggregateTokensPerSecond ?? null,
      ),
      expected: selection?.aggregateTps ?? null,
      actual: evidence?.after.aggregateTokensPerSecond ?? null,
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'before_inter_token_latency_p90',
      status: measuredChecklistStatus(
        evidence?.before.interTokenLatencyP90Ms ?? null,
      ),
      expected: selection?.baselineInterTokenLatencyP90Ms ?? null,
      actual: evidence?.before.interTokenLatencyP90Ms ?? null,
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'after_inter_token_latency_p90',
      status: measuredChecklistStatus(
        evidence?.after.interTokenLatencyP90Ms ?? null,
      ),
      expected: selection?.interTokenLatencyP90Ms ?? null,
      actual: evidence?.after.interTokenLatencyP90Ms ?? null,
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'before_ttft_p90',
      status: measuredChecklistStatus(evidence?.before.ttftP90Ms ?? null),
      expected: selection?.baselineTtftP90Ms ?? null,
      actual: evidence?.before.ttftP90Ms ?? null,
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'after_ttft_p90',
      status: measuredChecklistStatus(evidence?.after.ttftP90Ms ?? null),
      expected: selection?.ttftP90Ms ?? null,
      actual: evidence?.after.ttftP90Ms ?? null,
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'expected_vs_actual_configuration',
      status: matchedChecklistStatus(
        evidence !== null,
        evidence === null ? false : rolloutExpectedActualMatches(evidence),
      ),
      expected: expectedConfiguration === null ? null : 'selected_configuration',
      actual: evidence === null ? null : 'measured_configuration',
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'expected_vs_actual_lift',
      status:
        expectedLift === null || actualLift === null
          ? 'missing'
          : sameMeasuredNumber(expectedLift, actualLift)
            ? 'satisfied'
            : 'mismatch',
      expected: expectedLift,
      actual: actualLift,
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'rollout_progress_status',
      status:
        progress === null
          ? 'missing'
          : progress.status === 'measured_lift'
            ? 'satisfied'
            : 'mismatch',
      expected: 'measured_lift',
      actual: progress?.status ?? null,
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'rollout_progress_percent',
      status:
        progress === null
          ? 'missing'
          : progress.progressPercent >= 100
            ? 'satisfied'
            : 'mismatch',
      expected: 100,
      actual: progress?.progressPercent ?? null,
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'progress_totals_match_measurement',
      status:
        progress === null || evidence === null
          ? 'missing'
          : rolloutProgressMatchesMeasurement(progress, evidence)
            ? 'satisfied'
            : 'mismatch',
      expected: evidence === null ? null : 'measurement_totals',
      actual: progress === null ? null : 'progress_totals',
      publicEvidenceRefs,
    }),
    checklistItem({
      check: 'public_evidence_refs',
      status: publicEvidenceRefs.length > 0 ? 'satisfied' : 'missing',
      expected: 'public_refs',
      actual: publicEvidenceRefs.length,
      publicEvidenceRefs,
    }),
    ...dependencyChecks,
  ]
}

const rolloutOperatorAcceptance = (input: {
  status: GymThroughputRolloutReadout['status']
  evidenceChecklist: ReadonlyArray<GymThroughputRolloutEvidenceChecklistItem>
}): GymThroughputRolloutOperatorAcceptance => {
  const remainingChecks = input.evidenceChecklist
    .filter(item => item.status !== 'satisfied')
    .map(item => item.check)
  const accepted =
    input.status === 'measured' && remainingChecks.length === 0
  const publicEvidenceRefs = [
    ...new Set(
      input.evidenceChecklist.flatMap(item => item.publicEvidenceRefs),
    ),
  ]

  return {
    schemaVersion:
      'openagents.gym.throughput_rollout_operator_acceptance.v1',
    status: accepted
      ? 'accepted_for_stress_and_benchmark'
      : 'blocked_before_stress_and_benchmark',
    canStartIssue6317Stress: accepted,
    canStartIssue6312Benchmark: accepted,
    remainingChecks,
    requirements: input.evidenceChecklist.map(item => ({
      check: item.check,
      status: item.status,
      blocksIssueRefs: ['#6317', '#6312'],
      expected: item.expected,
      actual: item.actual,
      publicEvidenceRefs: [...item.publicEvidenceRefs],
    })),
    publicEvidenceRefs,
  }
}

export const buildGymThroughputRolloutReadout = (input: {
  generatedAt: string
  rolloutRun: GymThroughputOwnerArmedRolloutRunArtifact
  progressEvidence?: GymThroughputRolloutProgressEvidence | null
  glmFleetDurabilityDependencyEvidence?:
    | GymThroughputGlmFleetDurabilityDependencyEvidence
    | null
}): GymThroughputRolloutReadout => {
  const progressEvidence = input.progressEvidence ?? null
  const glmFleetDurabilityDependencyEvidence =
    input.glmFleetDurabilityDependencyEvidence ?? null
  const rolloutMeasurementEvidence =
    progressEvidence?.rolloutMeasurementEvidence ?? null
  const blockers: Array<GymThroughputRolloutReadoutBlocker> = []

  if (input.rolloutRun.status !== 'ready_to_apply') {
    blockers.push('rollout_run_blocked')
  }
  if (progressEvidence === null) {
    blockers.push('missing_progress_evidence')
  }
  if (
    progressEvidence !== null &&
    !sameNullableRef(progressEvidence.ownerArmRef, input.rolloutRun.ownerArmRef)
  ) {
    blockers.push('owner_arm_ref_mismatch')
  }
  if (
    progressEvidence !== null &&
    (progressEvidence.status !== 'measured_lift' ||
      progressEvidence.progressPercent < 100)
  ) {
    blockers.push('rollout_progress_incomplete')
  }
  if (progressEvidence?.status === 'measured_lift') {
    if (!isMeasured(progressEvidence.measuredThroughputLiftPercent)) {
      blockers.push('measured_lift_missing')
    } else if (progressEvidence.measuredThroughputLiftPercent <= 0) {
      blockers.push('measured_lift_not_positive')
    }
    if (rolloutMeasurementEvidence === null) {
      blockers.push('missing_rollout_measurement_evidence')
    } else {
      if (!rolloutMeasurementHasBeforeAfter(rolloutMeasurementEvidence)) {
        blockers.push('rollout_measurement_incomplete')
      }
      if (
        input.rolloutRun.recommendation.selection === null ||
        !rolloutMeasurementMatchesSelection(
          rolloutMeasurementEvidence,
          input.rolloutRun.recommendation.selection,
        )
      ) {
        blockers.push('rollout_measurement_selection_mismatch')
      }
      if (
        input.rolloutRun.recommendation.selection !== null &&
        !sameVllmFlags(
          input.rolloutRun.recommendation.selection.vllmFlags,
          rolloutMeasurementEvidence.liveVllmFlags,
        )
      ) {
        blockers.push('live_engine_flags_mismatch')
      }
      if (!rolloutExpectedActualMatches(rolloutMeasurementEvidence)) {
        blockers.push('expected_actual_evidence_mismatch')
      }
      if (
        progressEvidence !== null &&
        !rolloutProgressMatchesMeasurement(
          progressEvidence,
          rolloutMeasurementEvidence,
        )
      ) {
        blockers.push('progress_measurement_mismatch')
      }
    }
  }
  if (
    glmFleetDurabilityDependencyEvidence !== null &&
    glmFleetDurabilityDependencyEvidence.servingStatus !== 'ready'
  ) {
    blockers.push('glm_fleet_serving_not_ready')
  }
  if (
    glmFleetDurabilityDependencyEvidence !== null &&
    glmFleetDurabilityDependencyEvidence.acceptanceStatus !== 'complete'
  ) {
    blockers.push('glm_fleet_durability_acceptance_not_complete')
  }

  const measuredLiftPercent =
    progressEvidence === null
      ? NOT_MEASURED
      : normalizeMeasuredNumber(progressEvidence.measuredThroughputLiftPercent)
  const status =
    blockers.length > 0
      ? 'blocked'
      : progressEvidence?.status === 'measured_lift'
        ? 'measured'
        : 'ready'
  const evidenceChecklist = rolloutEvidenceChecklist({
    rolloutRun: input.rolloutRun,
    progressEvidence,
    rolloutMeasurementEvidence,
    glmFleetDurabilityDependencyEvidence,
  })
  const operatorAcceptance = rolloutOperatorAcceptance({
    status,
    evidenceChecklist,
  })

  return {
    schemaVersion: 'openagents.gym.throughput_rollout_readout.v1',
    generatedAt: input.generatedAt,
    environmentRef: GYM_THROUGHPUT_ENVIRONMENT_REF,
    publicSafe: true,
    metadataOnly: true,
    canMutateInfrastructure: false,
    status,
    rolloutRun: input.rolloutRun,
    progressEvidence,
    rolloutMeasurementEvidence,
    glmFleetDurabilityDependencyEvidence,
    evidenceChecklist,
    operatorAcceptance,
    measuredLiftPercent,
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
