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

export type Cs336A2DeviceBenchmarkPayload = Readonly<{
  assignmentRef: string
  benchmarkSuiteRef: typeof Cs336A2DeviceBenchmarkSuiteRef
  jobKind: typeof Cs336A2DeviceBenchmarkJobKind
  measurementKinds: ReadonlyArray<Cs336A2BenchmarkMeasurement>
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
  metric: Cs336A2BenchmarkMeasurement
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

export class DeviceCapabilityUnsafeProjectionError extends Error {
  readonly _tag = 'DeviceCapabilityUnsafeProjectionError'
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
): Cs336A2BenchmarkMeasurement | undefined => {
  const text = optionalString(value)

  return Cs336A2BenchmarkMeasurements.find(metric => metric === text)
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
    measurementKinds: Cs336A2BenchmarkMeasurements,
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
