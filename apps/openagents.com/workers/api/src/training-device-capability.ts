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

export const Cs336A2ThermalThrottleRatioFloor = 0.8
export const Cs336A2ThermalThrottleMetric =
  'sustained_vs_burst_throughput_ratio'

export const DeviceCapabilityThermalThrottleStates = [
  'thermal_probe_needs_verification',
  'thermal_throttle_not_observed',
  'thermal_throttle_observed',
] as const
export type DeviceCapabilityThermalThrottleState =
  (typeof DeviceCapabilityThermalThrottleStates)[number]

export const DeviceCapabilityThermalThrottleDetectionStatuses = [
  'missing',
  'needs_verified_thermal_probe',
  'thermal_throttle_not_observed',
  'thermal_throttle_observed',
] as const
export type DeviceCapabilityThermalThrottleDetectionStatus =
  (typeof DeviceCapabilityThermalThrottleDetectionStatuses)[number]

export const DeviceCapabilitySameClassReplicationScopes = [
  'cross_machine_same_class',
  'cross_process_same_host',
  'single_observation',
  'unknown',
] as const
export type DeviceCapabilitySameClassReplicationScope =
  (typeof DeviceCapabilitySameClassReplicationScopes)[number]

export const DeviceCapabilitySameClassReplicationStatuses = [
  'missing',
  'cross_machine_replicated',
  'same_host_only',
  'single_observation',
  'unknown_scope',
] as const
export type DeviceCapabilitySameClassReplicationStatus =
  (typeof DeviceCapabilitySameClassReplicationStatuses)[number]

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

/**
 * Provenance label for a device-capability measurement row.
 *
 * - `settled_cross_checked`: the original, fully-receipted basis — a paid
 *   benchmark closeout settled over Lightning and verified by
 *   `statistical_cross_check`. Rows of this provenance may carry a
 *   settlement receipt and a modeled-from-measured earning estimate, and
 *   become `verified` once they reach the same-class sample/verdict bar.
 * - `measured_unsettled`: a genuinely measured benchmark distribution that
 *   is NOT paid and NOT cross-check verified yet. This is the honest path
 *   for a freshly-characterized device class whose work has not been
 *   dispatched as a paid assignment and whose timings have not been
 *   replicated on a second same-class device. Such a row never carries a
 *   settlement receipt-derived earning estimate, never reports `verified`,
 *   and is held at `crossCheckState: 'measured_unverified'`. Its honesty
 *   anchor is the deterministic output-digest commitment, which must match
 *   the suite's commitment across heterogeneous hardware.
 */
export const DeviceCapabilityMeasurementProvenances = [
  'settled_cross_checked',
  'measured_unsettled',
] as const
export type DeviceCapabilityMeasurementProvenance =
  (typeof DeviceCapabilityMeasurementProvenances)[number]

export type DeviceCapabilityDistribution = Readonly<{
  crossCheckState:
    | 'cross_checked'
    | 'insufficient_same_class_samples'
    | 'measured_unverified'
    | 'needs_replication'
  deviceClassRef: string
  digestCommitmentRefs: ReadonlyArray<string>
  earningEstimate: DeviceCapabilityEarningEstimate | null
  max: number
  measurementProvenance: DeviceCapabilityMeasurementProvenance
  measurementRef: string
  metric: Cs336A2QualificationProbeMeasurement
  min: number
  ownerAcceptedProductionReceiptRefs: ReadonlyArray<string>
  p50: number
  p90: number
  receiptRefs: ReadonlyArray<string>
  sampleCount: number
  sourceRefs: ReadonlyArray<string>
  unit: string
  verificationRefs: ReadonlyArray<string>
  sameClassReplicationBlockerRefs: ReadonlyArray<string>
  sameClassReplicationScope: DeviceCapabilitySameClassReplicationScope
  sameClassReplicationStatus: Exclude<
    DeviceCapabilitySameClassReplicationStatus,
    'missing'
  >
  verified: boolean
  workClass: string
}>

export type DeviceCapabilityThermalThrottleSignal = Readonly<{
  blockerRefs: ReadonlyArray<string>
  crossCheckState: DeviceCapabilityDistribution['crossCheckState']
  deviceClassRef: string
  maxRatio: number
  measurementProvenance: DeviceCapabilityMeasurementProvenance
  measurementRef: string
  metric: typeof Cs336A2ThermalThrottleMetric
  minRatio: number
  ownerAcceptedProductionReceiptRefs: ReadonlyArray<string>
  p50Ratio: number
  p90Ratio: number
  ratioFloor: typeof Cs336A2ThermalThrottleRatioFloor
  reasonCode:
    | 'device_capability.public.thermal_probe_needs_statistical_cross_check'
    | 'device_capability.public.thermal_throttle_not_observed_sustained_ratio_at_or_above_floor'
    | 'device_capability.public.thermal_throttle_observed_sustained_ratio_below_floor'
  receiptRefs: ReadonlyArray<string>
  sampleCount: number
  sourceRefs: ReadonlyArray<string>
  state: DeviceCapabilityThermalThrottleState
  unit: 'ratio'
  verified: boolean
  workClass: string
}>

export type DeviceCapabilitySameClassReplicationSignal = Readonly<{
  blockerRefs: ReadonlyArray<string>
  crossCheckState: DeviceCapabilityDistribution['crossCheckState']
  deviceClassRef: string
  measurementProvenance: DeviceCapabilityMeasurementProvenance
  measurementRef: string
  metric: Cs336A2QualificationProbeMeasurement
  reasonCode:
    | 'device_capability.public.same_class_replication_cross_machine'
    | 'device_capability.public.same_class_replication_same_host_only'
    | 'device_capability.public.same_class_replication_single_observation'
    | 'device_capability.public.same_class_replication_scope_unknown'
  sampleCount: number
  scope: DeviceCapabilitySameClassReplicationScope
  sourceRefs: ReadonlyArray<string>
  state: Exclude<DeviceCapabilitySameClassReplicationStatus, 'missing'>
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
  observedSettledDeviceClassCount: number
  ownerAcceptedProductionThermalReceiptRefs: ReadonlyArray<string>
  privacyBoundaryRefs: ReadonlyArray<string>
  requiredSameClassSampleCount: number
  schemaVersion: 'openagents.training.device_capability_dataset.v1'
  sameClassReplicationBlockerRefs: ReadonlyArray<string>
  sameClassReplicationSignals: ReadonlyArray<DeviceCapabilitySameClassReplicationSignal>
  sameClassReplicationStatus: DeviceCapabilitySameClassReplicationStatus
  scopeBoundaryRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  thermalThrottleBlockerRefs: ReadonlyArray<string>
  thermalThrottleDetectionStatus: DeviceCapabilityThermalThrottleDetectionStatus
  thermalThrottleFunnelReasonCodes: ReadonlyArray<string>
  thermalThrottleReceiptRefs: ReadonlyArray<string>
  thermalThrottleSignals: ReadonlyArray<DeviceCapabilityThermalThrottleSignal>
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
  digestCommitmentRefs: PublicSafeRefs,
  earningEstimate: S.optionalKey(Cs336A2EarningEstimateEvidence),
  max: S.Number,
  measurementProvenance: S.optionalKey(
    S.Literals(DeviceCapabilityMeasurementProvenances),
  ),
  measurementRef: S.optionalKey(PublicSafeRef),
  metric: S.Literals(Cs336A2QualificationProbeMeasurements),
  min: S.Number,
  ownerAcceptedProductionReceiptRefs: PublicSafeRefs,
  p50: S.Number,
  p90: S.Number,
  receiptRefs: S.Array(PublicSafeRef),
  sameClassReplicationScope: S.optionalKey(
    S.Literals(DeviceCapabilitySameClassReplicationScopes),
  ),
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

const provenanceFromUnknown = (
  value: unknown,
): DeviceCapabilityMeasurementProvenance => {
  const text = optionalString(value)
  const match = DeviceCapabilityMeasurementProvenances.find(
    provenance => provenance === text,
  )

  // Default to the original settled/cross-checked basis so existing
  // receipted rows keep their exact prior semantics.
  return match ?? 'settled_cross_checked'
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
  provenance: DeviceCapabilityMeasurementProvenance,
): DeviceCapabilityEarningEstimate | null => {
  // Earning estimates are modeled FROM a receipted, settled closeout. A
  // genuinely measured but unsettled row has no paid basis, so it never
  // carries an earning estimate even if one is present in the evidence.
  if (provenance === 'measured_unsettled') {
    return null
  }

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
  provenance: DeviceCapabilityMeasurementProvenance,
): DeviceCapabilityDistribution['crossCheckState'] => {
  // A genuinely measured but unsettled row is never treated as verified,
  // regardless of sample count, because no validator verdict and no
  // second same-class device have replicated it.
  if (provenance === 'measured_unsettled') {
    return 'measured_unverified'
  }

  if (sampleCount >= 3 && verificationRefs.length > 0) {
    return 'cross_checked'
  }

  if (verificationRefs.length > 0) {
    return 'needs_replication'
  }

  return 'insufficient_same_class_samples'
}

const sameClassReplicationScopeFromUnknown = (
  value: unknown,
  provenance: DeviceCapabilityMeasurementProvenance,
): DeviceCapabilitySameClassReplicationScope => {
  const text = optionalString(value)
  const match = DeviceCapabilitySameClassReplicationScopes.find(
    scope => scope === text,
  )

  if (match !== undefined) {
    return match
  }

  // Fail closed for legacy rows. The first receipted A2 rows were two Pylons
  // on the same physical host, so an omitted scope may not become a
  // cross-machine replication claim. Measured-only rows default even lower:
  // one public observation until a second same-class machine is admitted.
  return provenance === 'measured_unsettled'
    ? 'single_observation'
    : 'cross_process_same_host'
}

const sameClassReplicationStatusFromScope = (
  scope: DeviceCapabilitySameClassReplicationScope,
): Exclude<DeviceCapabilitySameClassReplicationStatus, 'missing'> => {
  switch (scope) {
    case 'cross_machine_same_class':
      return 'cross_machine_replicated'
    case 'cross_process_same_host':
      return 'same_host_only'
    case 'single_observation':
      return 'single_observation'
    case 'unknown':
      return 'unknown_scope'
  }
}

const sameClassReplicationBlockersForStatus = (
  status: Exclude<DeviceCapabilitySameClassReplicationStatus, 'missing'>,
): ReadonlyArray<string> => {
  switch (status) {
    case 'cross_machine_replicated':
      return []
    case 'same_host_only':
    case 'single_observation':
    case 'unknown_scope':
      return ['blocker.cs336_a2.requires_cross_machine_same_class_replication']
  }
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

      const provenance = provenanceFromUnknown(measurement.measurementProvenance)
      const measurementRef =
        optionalString(measurement.measurementRef) ??
        `measurement.cs336_a2.${input.run.trainingRunRef}.${index + 1}`
      // Run-level verified challenges attach only to settled rows. A
      // genuinely measured but unsettled row never borrows another row's
      // verdict, so its verification set stays exactly what the evidence
      // declared (in practice, empty).
      const verificationRefs = uniqueRefs([
        ...stringArrayFromUnknown(measurement.verificationRefs),
        ...(provenance === 'measured_unsettled' ? [] : verifiedChallengeRefs),
      ])
      const state = crossCheckState(sampleCount, verificationRefs, provenance)
      const sameClassReplicationScope = sameClassReplicationScopeFromUnknown(
        measurement.sameClassReplicationScope,
        provenance,
      )
      const sameClassReplicationStatus =
        sameClassReplicationStatusFromScope(sameClassReplicationScope)
      const projected: DeviceCapabilityDistribution = {
        crossCheckState: state,
        deviceClassRef,
        digestCommitmentRefs: uniqueRefs(
          stringArrayFromUnknown(measurement.digestCommitmentRefs),
        ),
        earningEstimate: estimateFromEvidence(
          measurementRef,
          measurement,
          provenance,
        ),
        max,
        measurementProvenance: provenance,
        measurementRef,
        metric,
        min,
        ownerAcceptedProductionReceiptRefs: uniqueRefs(
          stringArrayFromUnknown(measurement.ownerAcceptedProductionReceiptRefs),
        ),
        p50,
        p90,
        receiptRefs: uniqueRefs(stringArrayFromUnknown(measurement.receiptRefs)),
        sameClassReplicationBlockerRefs:
          sameClassReplicationBlockersForStatus(sameClassReplicationStatus),
        sameClassReplicationScope,
        sameClassReplicationStatus,
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

export const buildDeviceCapabilitySameClassReplicationSignals = (
  distributions: ReadonlyArray<DeviceCapabilityDistribution>,
): ReadonlyArray<DeviceCapabilitySameClassReplicationSignal> =>
  distributions.map(distribution => {
    const state = distribution.sameClassReplicationStatus
    const reasonCode =
      state === 'cross_machine_replicated'
        ? 'device_capability.public.same_class_replication_cross_machine'
        : state === 'same_host_only'
          ? 'device_capability.public.same_class_replication_same_host_only'
          : state === 'single_observation'
            ? 'device_capability.public.same_class_replication_single_observation'
            : 'device_capability.public.same_class_replication_scope_unknown'
    const signal: DeviceCapabilitySameClassReplicationSignal = {
      blockerRefs: distribution.sameClassReplicationBlockerRefs,
      crossCheckState: distribution.crossCheckState,
      deviceClassRef: distribution.deviceClassRef,
      measurementProvenance: distribution.measurementProvenance,
      measurementRef: distribution.measurementRef,
      metric: distribution.metric,
      reasonCode,
      sampleCount: distribution.sampleCount,
      scope: distribution.sameClassReplicationScope,
      sourceRefs: distribution.sourceRefs,
      state,
      verified: distribution.verified,
      workClass: distribution.workClass,
    }

    publicSafeJson(signal)

    return signal
  })

export const sameClassReplicationStatus = (
  signals: ReadonlyArray<DeviceCapabilitySameClassReplicationSignal>,
): DeviceCapabilitySameClassReplicationStatus => {
  if (signals.length === 0) {
    return 'missing'
  }

  if (signals.some(signal => signal.state === 'cross_machine_replicated')) {
    return 'cross_machine_replicated'
  }

  if (signals.some(signal => signal.state === 'same_host_only')) {
    return 'same_host_only'
  }

  if (signals.some(signal => signal.state === 'single_observation')) {
    return 'single_observation'
  }

  return 'unknown_scope'
}

export const sameClassReplicationBlockerRefs = (
  signals: ReadonlyArray<DeviceCapabilitySameClassReplicationSignal>,
): ReadonlyArray<string> => {
  if (signals.length === 0) {
    return ['blocker.cs336_a2.requires_replication_across_same_class_devices']
  }

  return uniqueRefs(signals.flatMap(signal => signal.blockerRefs))
}

export const buildDeviceCapabilityThermalThrottleSignals = (
  distributions: ReadonlyArray<DeviceCapabilityDistribution>,
): ReadonlyArray<DeviceCapabilityThermalThrottleSignal> =>
  distributions.flatMap(
    (distribution): ReadonlyArray<DeviceCapabilityThermalThrottleSignal> => {
      if (distribution.metric !== Cs336A2ThermalThrottleMetric) {
        return []
      }

      const needsVerification = !distribution.verified
      const throttled =
        distribution.verified &&
        distribution.p50 < Cs336A2ThermalThrottleRatioFloor
      const state: DeviceCapabilityThermalThrottleState = needsVerification
        ? 'thermal_probe_needs_verification'
        : throttled
          ? 'thermal_throttle_observed'
          : 'thermal_throttle_not_observed'
      const blockerRefs =
        state === 'thermal_probe_needs_verification'
          ? ['blocker.cs336_a2.requires_verified_sustained_vs_burst_thermal_probe']
          : []
      const reasonCode =
        state === 'thermal_probe_needs_verification'
          ? 'device_capability.public.thermal_probe_needs_statistical_cross_check'
          : state === 'thermal_throttle_observed'
            ? 'device_capability.public.thermal_throttle_observed_sustained_ratio_below_floor'
            : 'device_capability.public.thermal_throttle_not_observed_sustained_ratio_at_or_above_floor'
      const signal: DeviceCapabilityThermalThrottleSignal = {
        blockerRefs,
        crossCheckState: distribution.crossCheckState,
        deviceClassRef: distribution.deviceClassRef,
        maxRatio: distribution.max,
        measurementProvenance: distribution.measurementProvenance,
        measurementRef: distribution.measurementRef,
        metric: Cs336A2ThermalThrottleMetric,
        minRatio: distribution.min,
        ownerAcceptedProductionReceiptRefs:
          distribution.ownerAcceptedProductionReceiptRefs,
        p50Ratio: distribution.p50,
        p90Ratio: distribution.p90,
        ratioFloor: Cs336A2ThermalThrottleRatioFloor,
        reasonCode,
        receiptRefs: distribution.receiptRefs,
        sampleCount: distribution.sampleCount,
        sourceRefs: distribution.sourceRefs,
        state,
        unit: 'ratio',
        verified: distribution.verified,
        workClass: distribution.workClass,
      }

      publicSafeJson(signal)

      return [signal]
    },
  )

export const thermalThrottleDetectionStatus = (
  signals: ReadonlyArray<DeviceCapabilityThermalThrottleSignal>,
): DeviceCapabilityThermalThrottleDetectionStatus => {
  if (signals.length === 0) {
    return 'missing'
  }

  if (signals.some(signal => signal.state === 'thermal_throttle_observed')) {
    return 'thermal_throttle_observed'
  }

  if (
    signals.some(signal => signal.state === 'thermal_probe_needs_verification')
  ) {
    return 'needs_verified_thermal_probe'
  }

  return 'thermal_throttle_not_observed'
}

export const thermalThrottleBlockerRefs = (
  signals: ReadonlyArray<DeviceCapabilityThermalThrottleSignal>,
): ReadonlyArray<string> => {
  if (signals.length === 0) {
    return ['blocker.cs336_a2.requires_sustained_vs_burst_thermal_probe']
  }

  const ownerAcceptedProductionReceiptRefs =
    ownerAcceptedProductionThermalReceiptRefs(signals)

  return uniqueRefs([
    ...signals.flatMap(signal => signal.blockerRefs),
    ...(ownerAcceptedProductionReceiptRefs.length === 0
      ? ['blocker.cs336_a2.requires_owner_accepted_production_thermal_receipt']
      : []),
  ])
}

export const thermalThrottleFunnelReasonCodes = (
  signals: ReadonlyArray<DeviceCapabilityThermalThrottleSignal>,
): ReadonlyArray<string> =>
  uniqueRefs(signals.map(signal => signal.reasonCode))

export const thermalThrottleReceiptRefs = (
  signals: ReadonlyArray<DeviceCapabilityThermalThrottleSignal>,
): ReadonlyArray<string> =>
  uniqueRefs(
    signals
      .filter(signal => signal.verified)
      .flatMap(signal => signal.receiptRefs),
  )

export const ownerAcceptedProductionThermalReceiptRefs = (
  signals: ReadonlyArray<DeviceCapabilityThermalThrottleSignal>,
): ReadonlyArray<string> =>
  uniqueRefs(
    signals
      .filter(signal => signal.verified)
      .flatMap(signal => signal.ownerAcceptedProductionReceiptRefs),
  )

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

  if (
    measurement.metric === Cs336A2ThermalThrottleMetric &&
    measurement.unit !== 'ratio'
  ) {
    throw new DeviceCapabilityEvidenceValidationError(
      'CS336 A2 sustained-vs-burst thermal evidence requires unit ratio.',
    )
  }

  const provenance = measurement.measurementProvenance ?? 'settled_cross_checked'
  const ownerAcceptedProductionReceiptRefs =
    measurement.ownerAcceptedProductionReceiptRefs ?? []

  if (
    ownerAcceptedProductionReceiptRefs.length > 0 &&
    measurement.metric !== Cs336A2ThermalThrottleMetric
  ) {
    throw new DeviceCapabilityEvidenceValidationError(
      'CS336 A2 owner-accepted production receipt refs are only admissible on sustained-vs-burst thermal evidence.',
    )
  }

  if (provenance === 'measured_unsettled') {
    // Genuinely measured but unsettled rows are NOT paid, so they carry no
    // settlement receipt and no earning estimate. Their honesty anchor is
    // the deterministic output-digest commitment that must match the
    // suite's commitment across heterogeneous hardware, so at least one
    // digest-commitment ref is required in lieu of a settlement receipt.
    if (measurement.digestCommitmentRefs === undefined ||
      measurement.digestCommitmentRefs.length === 0) {
      throw new DeviceCapabilityEvidenceValidationError(
        'CS336 A2 measured_unsettled evidence requires at least one digest-commitment ref (the deterministic cross-device output commitment).',
      )
    }

    if (measurement.earningEstimate !== undefined) {
      throw new DeviceCapabilityEvidenceValidationError(
        'CS336 A2 measured_unsettled evidence must not carry an earning estimate; earning estimates are modeled only from a receipted, settled closeout.',
      )
    }

    if (measurement.receiptRefs.length > 0) {
      throw new DeviceCapabilityEvidenceValidationError(
        'CS336 A2 measured_unsettled evidence must not carry a settlement receipt ref; unsettled rows are explicitly not paid.',
      )
    }

    if (ownerAcceptedProductionReceiptRefs.length > 0) {
      throw new DeviceCapabilityEvidenceValidationError(
        'CS336 A2 measured_unsettled evidence must not carry owner-accepted production receipt refs.',
      )
    }

    return
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
  const observedSettledDeviceClassCount = new Set(
    classDistributions
      .filter(distribution => distribution.verified)
      .map(distribution => distribution.deviceClassRef),
  ).size
  const sameClassReplicationSignals =
    buildDeviceCapabilitySameClassReplicationSignals(classDistributions)
  const replicationBlockers = sameClassReplicationBlockerRefs(
    sameClassReplicationSignals,
  )
  const measurementBlockers =
    observedMeasurementCount > 0 && verifiedCount === observedMeasurementCount
      ? []
      : [
          'blocker.cs336_a2.requires_receipted_benchmark_results',
          'blocker.cs336_a2.requires_statistical_cross_check',
          'blocker.cs336_a2.requires_replication_across_same_class_devices',
        ]
  const thermalThrottleSignals =
    buildDeviceCapabilityThermalThrottleSignals(classDistributions)

  return {
    benchmarkSuiteRef: Cs336A2DeviceBenchmarkSuiteRef,
    blockerRefs: uniqueRefs([...measurementBlockers, ...replicationBlockers]),
    classDistributions,
    jobKind: Cs336A2DeviceBenchmarkJobKind,
    observedDeviceClassCount,
    observedMeasurementCount,
    observedSettledDeviceClassCount,
    ownerAcceptedProductionThermalReceiptRefs:
      ownerAcceptedProductionThermalReceiptRefs(thermalThrottleSignals),
    privacyBoundaryRefs: [
      'privacy.cs336_a2.class_level_dataset_only',
      'privacy.cs336_a2.no_device_identifiers',
      'privacy.cs336_a2.no_wallet_or_payment_material',
    ],
    requiredSameClassSampleCount: 3,
    schemaVersion: 'openagents.training.device_capability_dataset.v1',
    sameClassReplicationBlockerRefs: replicationBlockers,
    sameClassReplicationSignals,
    sameClassReplicationStatus: sameClassReplicationStatus(
      sameClassReplicationSignals,
    ),
    scopeBoundaryRefs: [
      'scope.cs336_a2.benchmark_measurement_not_assignment_settlement',
      'scope.cs336_a2.earning_estimates_modeled_from_measured',
      'scope.cs336_a2.psionic_kernel_and_transport_parity_external',
      'scope.cs336_a2.same_class_replication_requires_cross_machine_scope',
      'scope.cs336_a2.thermal_probe_classifier_not_continuous_fleet_monitoring',
    ],
    sourceRefs: uniqueRefs([
      'route:/api/training/device-capabilities/a2',
      `route:/api/training/runs/${input.run.trainingRunRef}`,
      ...input.run.sourceRefs,
      ...input.windows.flatMap(window => window.sourceRefs),
      ...input.leases.map(lease => lease.leaseRef),
    ]),
    thermalThrottleBlockerRefs:
      thermalThrottleBlockerRefs(thermalThrottleSignals),
    thermalThrottleDetectionStatus:
      thermalThrottleDetectionStatus(thermalThrottleSignals),
    thermalThrottleFunnelReasonCodes:
      thermalThrottleFunnelReasonCodes(thermalThrottleSignals),
    thermalThrottleReceiptRefs:
      thermalThrottleReceiptRefs(thermalThrottleSignals),
    thermalThrottleSignals,
  }
}
