import { Schema as S } from 'effect'

import {
  isRecord,
  optionalString,
  parseJsonRecord,
  stringArrayFromUnknown,
} from './json-boundary'
import type {
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
  TrainingWindowRecord,
} from './training-run-window-authority'
import type { TrainingVerificationChallengeRecord } from './training-verification'

export const Cs336A2DeviceBenchmarkJobKind = 'cs336_a2_device_benchmark'
export const Cs336A2DeviceBenchmarkSuiteRef =
  'benchmark_suite.cs336_a2.pylon_runtime_device_capability.v1'
export const Cs336A2RequestSchemaRef =
  'openagents.cs336_a2_device_benchmark_request.v1'
export const Cs336A2OutputSchemaRef =
  'openagents.cs336_a2_device_benchmark_output.v1'

export const Cs336A2BenchmarkMeasurements = [
  'attention_throughput',
  'memory_bandwidth',
  'tokens_per_second',
  'step_time_ms',
] as const
export type Cs336A2BenchmarkMeasurement =
  (typeof Cs336A2BenchmarkMeasurements)[number]

// Host-side qualification probe kinds (openagents issue #4852, Pluralis
// roadmap P1.4). These join the qualification schema and evidence
// admission alongside the kernel-executable benchmark kinds, but they
// are hardware-gated: the bounded workload module cannot synthesize a
// host-RAM headroom or sustained-vs-burst thermal measurement, so live
// values arrive only as receipted device evidence. Pluralis's
// contributor shape (24 GB GPU + 80 GB system RAM, Adam moments
// offloaded to host RAM) is the reason host RAM is a binding constraint
// the kernel suite alone does not measure.
export const Cs336A2HostProbeMeasurements = [
  'host_ram_headroom_gb',
  'sustained_vs_burst_throughput_ratio',
] as const
export type Cs336A2HostProbeMeasurement =
  (typeof Cs336A2HostProbeMeasurements)[number]

export const Cs336A2QualificationProbeMeasurements = [
  ...Cs336A2BenchmarkMeasurements,
  ...Cs336A2HostProbeMeasurements,
] as const
export type Cs336A2QualificationProbeMeasurement =
  (typeof Cs336A2QualificationProbeMeasurements)[number]

export type Cs336A2DeviceBenchmarkPayload = Readonly<{
  assignmentRef: string
  benchmarkSuiteRef: typeof Cs336A2DeviceBenchmarkSuiteRef
  jobKind: typeof Cs336A2DeviceBenchmarkJobKind
  measurementKinds: ReadonlyArray<Cs336A2QualificationProbeMeasurement>
  outputSchemaRef: typeof Cs336A2OutputSchemaRef
  privacyPolicyRefs: ReadonlyArray<string>
  requestSchemaRef: typeof Cs336A2RequestSchemaRef
  verificationClass: 'statistical_cross_check'
}>

export type DeviceCapabilityEarningEstimate = Readonly<{
  basisLabel: 'modeled_from_measured_benchmark_distribution'
  estimateRef: string
  policyRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  workClass: string
  p50SatsPerHour: number | null
  p90SatsPerHour: number | null
}>

export type DeviceCapabilityDistribution = Readonly<{
  crossCheckState:
    | 'cross_checked'
    | 'insufficient_same_class_samples'
    | 'needs_replication'
  deviceClassRef: string
  earningEstimate: DeviceCapabilityEarningEstimate | null
  max: number
  measurementRef: string
  metric: Cs336A2QualificationProbeMeasurement
  min: number
  p50: number
  p90: number
  receiptRefs: ReadonlyArray<string>
  sampleCount: number
  sourceRefs: ReadonlyArray<string>
  unit: string
  verificationRefs: ReadonlyArray<string>
  verified: boolean
  workClass: string
}>

export type DeviceCapabilityDatasetProjection = Readonly<{
  benchmarkSuiteRef: typeof Cs336A2DeviceBenchmarkSuiteRef
  blockerRefs: ReadonlyArray<string>
  classDistributions: ReadonlyArray<DeviceCapabilityDistribution>
  jobKind: typeof Cs336A2DeviceBenchmarkJobKind
  observedDeviceClassCount: number
  observedMeasurementCount: number
  privacyBoundaryRefs: ReadonlyArray<string>
  requiredSameClassSampleCount: number
  schemaVersion: 'openagents.training.device_capability_dataset.v1'
  scopeBoundaryRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
}>

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const PublicSafeRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/),
)
const PublicSafeRefs = S.optionalKey(S.Array(PublicSafeRef))

const Cs336A2EarningEstimateEvidence = S.Struct({
  estimateRef: S.optionalKey(PublicSafeRef),
  p50SatsPerHour: S.optionalKey(S.Number),
  p90SatsPerHour: S.optionalKey(S.Number),
  policyRefs: PublicSafeRefs,
  sourceRefs: PublicSafeRefs,
  workClass: PublicSafeRef,
})

export const Cs336A2MeasurementEvidence = S.Struct({
  deviceClassRef: PublicSafeRef,
  earningEstimate: S.optionalKey(Cs336A2EarningEstimateEvidence),
  max: S.Number,
  measurementRef: S.optionalKey(PublicSafeRef),
  metric: S.Literals(Cs336A2QualificationProbeMeasurements),
  min: S.Number,
  p50: S.Number,
  p90: S.Number,
  receiptRefs: S.Array(PublicSafeRef),
  sampleCount: S.Number,
  sourceRefs: PublicSafeRefs,
  unit: PublicSafeRef,
  verificationRefs: PublicSafeRefs,
  workClass: PublicSafeRef,
})
export type Cs336A2MeasurementEvidence =
  typeof Cs336A2MeasurementEvidence.Type

export const Cs336A2DeviceBenchmarkEvidenceRequest = S.Struct({
  measurements: S.Array(Cs336A2MeasurementEvidence),
  receiptRefs: PublicSafeRefs,
  sourceRefs: PublicSafeRefs,
})
export type Cs336A2DeviceBenchmarkEvidenceRequest =
  typeof Cs336A2DeviceBenchmarkEvidenceRequest.Type

export class DeviceCapabilityUnsafeProjectionError extends Error {
  readonly _tag = 'DeviceCapabilityUnsafeProjectionError'
}

export class DeviceCapabilityEvidenceValidationError extends Error {
  readonly _tag = 'DeviceCapabilityEvidenceValidationError'
}

const unsafePublicMaterialPattern =
  /(\"?(deviceId|deviceRef|nodeId|nodeRef|ownerId|ownerRef|pylonId|pylonRef|wallet[A-Za-z0-9_-]*|mnemonic|payment[A-Za-z0-9_-]*|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const optionalNumber = (value: unknown): number | undefined => {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return value
  }

  if (typeof value !== 'string' || value.trim() === '') {
    return undefined
  }

  const parsed = Number(value)

  return Number.isFinite(parsed) ? parsed : undefined
}

const metricFromUnknown = (
  value: unknown,
): Cs336A2QualificationProbeMeasurement | undefined => {
  const text = optionalString(value)

  return Cs336A2QualificationProbeMeasurements.find(metric => metric === text)
}

const publicSafeJson = (value: unknown): string => {
  const json = JSON.stringify(value)

  if (unsafePublicMaterialPattern.test(json)) {
    throw new DeviceCapabilityUnsafeProjectionError(
      'CS336 A2 device-capability projection contains device-identifying or private material.',
    )
  }

  return json
}

const benchmarkEvidenceRecord = (
  run: TrainingRunRecord,
): Record<string, unknown> | undefined => {
  const projection = parseJsonRecord(run.publicProjectionJson)
  const nested = projection?.a2DeviceBenchmark

  return isRecord(nested) ? nested : undefined
}

const estimateFromEvidence = (
  measurementRef: string,
  measurement: Record<string, unknown>,
): DeviceCapabilityEarningEstimate | null => {
  const estimate = measurement.earningEstimate

  if (!isRecord(estimate)) {
    return null
  }

  const workClass = optionalString(estimate.workClass)

  if (workClass === undefined) {
    return null
  }

  const projected: DeviceCapabilityEarningEstimate = {
    basisLabel: 'modeled_from_measured_benchmark_distribution',
    estimateRef:
      optionalString(estimate.estimateRef) ??
      `estimate.cs336_a2.${measurementRef}`,
    p50SatsPerHour: optionalNumber(estimate.p50SatsPerHour) ?? null,
    p90SatsPerHour: optionalNumber(estimate.p90SatsPerHour) ?? null,
    policyRefs: uniqueRefs([
      'policy.public.device_capability.earning_estimates_are_modeled',
      ...stringArrayFromUnknown(estimate.policyRefs),
    ]),
    sourceRefs: uniqueRefs(stringArrayFromUnknown(estimate.sourceRefs)),
    workClass,
  }

  publicSafeJson(projected)

  return projected
}

const crossCheckState = (
  sampleCount: number,
  verificationRefs: ReadonlyArray<string>,
): DeviceCapabilityDistribution['crossCheckState'] => {
  if (sampleCount >= 3 && verificationRefs.length > 0) {
    return 'cross_checked'
  }

  if (verificationRefs.length > 0) {
    return 'needs_replication'
  }

  return 'insufficient_same_class_samples'
}

const distributionsFromEvidence = (
  input: Readonly<{
    challenges: ReadonlyArray<TrainingVerificationChallengeRecord>
    evidence: Record<string, unknown> | undefined
    leases: ReadonlyArray<TrainingWindowLeaseRecord>
    run: TrainingRunRecord
    windows: ReadonlyArray<TrainingWindowRecord>
  }>,
): ReadonlyArray<DeviceCapabilityDistribution> => {
  const measurements = input.evidence?.measurements

  if (!Array.isArray(measurements)) {
    return []
  }

  publicSafeJson(measurements)

  const verifiedChallengeRefs = input.challenges
    .filter(challenge => challenge.state === 'Verified')
    .map(challenge => challenge.challengeRef)

  return measurements.flatMap(
    (measurement, index): ReadonlyArray<DeviceCapabilityDistribution> => {
      if (!isRecord(measurement)) {
        return []
      }

      const metric = metricFromUnknown(measurement.metric)
      const deviceClassRef = optionalString(measurement.deviceClassRef)
      const workClass = optionalString(measurement.workClass)
      const unit = optionalString(measurement.unit)
      const sampleCount = optionalNumber(measurement.sampleCount)
      const p50 = optionalNumber(measurement.p50)
      const p90 = optionalNumber(measurement.p90)
      const min = optionalNumber(measurement.min)
      const max = optionalNumber(measurement.max)

      if (
        metric === undefined ||
        deviceClassRef === undefined ||
        workClass === undefined ||
        unit === undefined ||
        sampleCount === undefined ||
        p50 === undefined ||
        p90 === undefined ||
        min === undefined ||
        max === undefined
      ) {
        return []
      }

      const measurementRef =
        optionalString(measurement.measurementRef) ??
        `measurement.cs336_a2.${input.run.trainingRunRef}.${index + 1}`
      const verificationRefs = uniqueRefs([
        ...stringArrayFromUnknown(measurement.verificationRefs),
        ...verifiedChallengeRefs,
      ])
      const state = crossCheckState(sampleCount, verificationRefs)
      const projected: DeviceCapabilityDistribution = {
        crossCheckState: state,
        deviceClassRef,
        earningEstimate: estimateFromEvidence(measurementRef, measurement),
        max,
        measurementRef,
        metric,
        min,
        p50,
        p90,
        receiptRefs: uniqueRefs(stringArrayFromUnknown(measurement.receiptRefs)),
        sampleCount,
        sourceRefs: uniqueRefs([
          ...stringArrayFromUnknown(measurement.sourceRefs),
          ...input.windows.flatMap(window => window.sourceRefs),
          ...input.leases.map(lease => lease.leaseRef),
        ]),
        unit,
        verificationRefs,
        verified: state === 'cross_checked',
        workClass,
      }

      publicSafeJson(projected)

      return [projected]
    },
  )
}

export const buildCs336A2DeviceBenchmarkPayload = (
  input: Readonly<{ assignmentRef: string }>,
): Cs336A2DeviceBenchmarkPayload => {
  const payload: Cs336A2DeviceBenchmarkPayload = {
    assignmentRef: input.assignmentRef,
    benchmarkSuiteRef: Cs336A2DeviceBenchmarkSuiteRef,
    jobKind: Cs336A2DeviceBenchmarkJobKind,
    measurementKinds: Cs336A2QualificationProbeMeasurements,
    outputSchemaRef: Cs336A2OutputSchemaRef,
    privacyPolicyRefs: [
      'policy.public.device_capability.no_device_identifiers',
      'policy.public.device_capability.class_level_counts_only',
      'policy.public.device_capability.no_wallet_or_payment_material',
    ],
    requestSchemaRef: Cs336A2RequestSchemaRef,
    verificationClass: 'statistical_cross_check',
  }

  publicSafeJson(payload)

  return payload
}

const assertAdmissibleMeasurement = (
  measurement: Cs336A2MeasurementEvidence,
): void => {
  const numbers = [
    measurement.min,
    measurement.p50,
    measurement.p90,
    measurement.max,
    measurement.sampleCount,
  ]

  if (!numbers.every(value => Number.isFinite(value))) {
    throw new DeviceCapabilityEvidenceValidationError(
      'CS336 A2 measurement evidence requires finite numeric statistics.',
    )
  }

  if (
    !(
      measurement.min <= measurement.p50 &&
      measurement.p50 <= measurement.p90 &&
      measurement.p90 <= measurement.max
    )
  ) {
    throw new DeviceCapabilityEvidenceValidationError(
      'CS336 A2 measurement evidence requires min <= p50 <= p90 <= max.',
    )
  }

  if (
    !Number.isInteger(measurement.sampleCount) ||
    measurement.sampleCount < 1
  ) {
    throw new DeviceCapabilityEvidenceValidationError(
      'CS336 A2 measurement evidence requires an integer sampleCount >= 1.',
    )
  }

  if (measurement.receiptRefs.length === 0) {
    throw new DeviceCapabilityEvidenceValidationError(
      'CS336 A2 measurement evidence requires at least one receipt ref; unreceipted benchmark rows are not admissible.',
    )
  }
}

/**
 * Admits receipted CS336 A2 benchmark measurements into a training run's
 * public projection. The privacy guard runs at admission time on the
 * exact evidence that will be projected, so device-identifying or
 * wallet/payment material is rejected before it can reach D1.
 */
export const admitCs336A2DeviceBenchmarkEvidence = (
  input: Readonly<{
    nowIso: string
    request: Cs336A2DeviceBenchmarkEvidenceRequest
    run: TrainingRunRecord
  }>,
): TrainingRunRecord => {
  if (input.request.measurements.length === 0) {
    throw new DeviceCapabilityEvidenceValidationError(
      'CS336 A2 benchmark evidence requires at least one measurement.',
    )
  }

  for (const measurement of input.request.measurements) {
    assertAdmissibleMeasurement(measurement)
  }

  const evidence = {
    benchmarkSuiteRef: Cs336A2DeviceBenchmarkSuiteRef,
    jobKind: Cs336A2DeviceBenchmarkJobKind,
    measurements: input.request.measurements,
    receiptRefs: uniqueRefs([...(input.request.receiptRefs ?? [])]),
    sourceRefs: uniqueRefs([...(input.request.sourceRefs ?? [])]),
  }

  publicSafeJson(evidence)

  const projection = parseJsonRecord(input.run.publicProjectionJson) ?? {}

  return {
    ...input.run,
    publicProjectionJson: JSON.stringify({
      ...projection,
      a2DeviceBenchmark: evidence,
    }),
    updatedAt: input.nowIso,
  }
}

export const publicDeviceCapabilityProjection = (
  input: Readonly<{
    challenges: ReadonlyArray<TrainingVerificationChallengeRecord>
    leases: ReadonlyArray<TrainingWindowLeaseRecord>
    run: TrainingRunRecord
    windows: ReadonlyArray<TrainingWindowRecord>
  }>,
): DeviceCapabilityDatasetProjection => {
  const evidence = benchmarkEvidenceRecord(input.run)
  const classDistributions = distributionsFromEvidence({ ...input, evidence })
  const observedDeviceClassCount = new Set(
    classDistributions.map(distribution => distribution.deviceClassRef),
  ).size
  const observedMeasurementCount = classDistributions.length
  const verifiedCount = classDistributions.filter(
    distribution => distribution.verified,
  ).length

  return {
    benchmarkSuiteRef: Cs336A2DeviceBenchmarkSuiteRef,
    blockerRefs:
      observedMeasurementCount > 0 && verifiedCount === observedMeasurementCount
        ? []
        : [
            'blocker.cs336_a2.requires_receipted_benchmark_results',
            'blocker.cs336_a2.requires_statistical_cross_check',
            'blocker.cs336_a2.requires_replication_across_same_class_devices',
          ],
    classDistributions,
    jobKind: Cs336A2DeviceBenchmarkJobKind,
    observedDeviceClassCount,
    observedMeasurementCount,
    privacyBoundaryRefs: [
      'privacy.cs336_a2.class_level_dataset_only',
      'privacy.cs336_a2.no_device_identifiers',
      'privacy.cs336_a2.no_wallet_or_payment_material',
    ],
    requiredSameClassSampleCount: 3,
    schemaVersion: 'openagents.training.device_capability_dataset.v1',
    scopeBoundaryRefs: [
      'scope.cs336_a2.benchmark_measurement_not_assignment_settlement',
      'scope.cs336_a2.earning_estimates_modeled_from_measured',
      'scope.cs336_a2.psionic_kernel_and_transport_parity_external',
    ],
    sourceRefs: uniqueRefs([
      'route:/api/training/device-capabilities/a2',
      `route:/api/training/runs/${input.run.trainingRunRef}`,
      ...input.run.sourceRefs,
      ...input.windows.flatMap(window => window.sourceRefs),
      ...input.leases.map(lease => lease.leaseRef),
    ]),
  }
}
