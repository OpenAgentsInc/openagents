/**
 * Reasoned device admission gates (openagents issue #4852, Pluralis
 * roadmap P1.4).
 *
 * The exclusion-with-stated-reason pattern: every device class admitted
 * to a work class carries its measured reason, and every exclusion
 * carries one too. Pluralis's canonical example is the T4/V100
 * exclusion — those cards would emulate BF16 slower than FP32, and that
 * reason ships with the gate instead of living in a maintainer's head.
 *
 * Gate definitions here are versioned, frozen, JSON-able DEFINITIONS,
 * not live admission policy. No decision in this module claims a live
 * device was measured; live measurements are hardware-gated and arrive
 * only as receipted evidence through
 * `admitCs336A2DeviceBenchmarkEvidence`. Psionic's preflight
 * qualification consumes the same exported contract
 * (`exportDeviceAdmissionGateContract`) rather than duplicating gate
 * thresholds — that cross-repo seam is documented in
 * `docs/2026-06-12-device-admission-gates.md`.
 */

import { Schema as S } from 'effect'

import { Cs336A2QualificationProbeMeasurements } from './training-device-capability'

export const DeviceAdmissionGateContractSchemaVersion =
  'openagents.training.device_admission_gates.v1'

export const DeviceAdmissionGateComparisons = ['at_least', 'at_most'] as const
export type DeviceAdmissionGateComparison =
  (typeof DeviceAdmissionGateComparisons)[number]

// Coding-host probe kinds (openagents issue #4861, tracker #4862).
// These extend the gate measurement-kind union for coding-agent work
// classes without touching the CS336 A2 qualification payload: the a2
// benchmark suite does not measure them, so they never join
// `Cs336A2QualificationProbeMeasurements` or the benchmark payload.
// Presence kinds are 0-or-1 measurements gated `at_least 1`. Live
// values are hardware-gated and arrive only as receipted evidence,
// exactly like the host-RAM and sustained-vs-burst probe kinds.
export const PylonCodingHostProbeMeasurements = [
  'node_or_bun_runtime_present',
  'workspace_write_sandbox_supported',
] as const
export type PylonCodingHostProbeMeasurement =
  (typeof PylonCodingHostProbeMeasurements)[number]

export const DeviceAdmissionGateMeasurementKinds = [
  ...Cs336A2QualificationProbeMeasurements,
  ...PylonCodingHostProbeMeasurements,
] as const
export type DeviceAdmissionGateMeasurementKind =
  (typeof DeviceAdmissionGateMeasurementKinds)[number]

export const DeviceAdmissionDecisions = ['admitted', 'excluded'] as const
export type DeviceAdmissionDecision = (typeof DeviceAdmissionDecisions)[number]

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const PublicSafeRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/),
)

// Funnel-compatible reason-code namespace, following the
// `join_lifecycle.public.*` / `dark_capacity.public.*` convention:
// platform-issued, closed-shape, projection-safe constants.
const admittedReasonCodePattern =
  /^device_admission\.public\.admitted_[a-z0-9_]{1,200}$/
const excludedReasonCodePattern =
  /^device_admission\.public\.excluded_[a-z0-9_]{1,200}$/

const AdmittedReasonCode = NonEmptyTrimmedString.check(
  S.isPattern(admittedReasonCodePattern),
)
const ExcludedReasonCode = NonEmptyTrimmedString.check(
  S.isPattern(excludedReasonCodePattern),
)

const StatedReason = NonEmptyTrimmedString.check(
  S.isMinLength(8),
  S.isMaxLength(600),
)

export const DeviceAdmissionGateRequirement = S.Struct({
  comparison: S.Literals(DeviceAdmissionGateComparisons),
  measurementKind: S.Literals(DeviceAdmissionGateMeasurementKinds),
  threshold: S.Number,
  unit: PublicSafeRef,
})
export type DeviceAdmissionGateRequirement =
  typeof DeviceAdmissionGateRequirement.Type

export const DeviceAdmissionGateDefinition = S.Struct({
  admittedReasonCode: AdmittedReasonCode,
  excludedReasonCode: ExcludedReasonCode,
  gateRef: PublicSafeRef,
  rationale: StatedReason,
  requirement: DeviceAdmissionGateRequirement,
  sourceRefs: S.Array(PublicSafeRef),
  workClassRef: PublicSafeRef,
})
export type DeviceAdmissionGateDefinition =
  typeof DeviceAdmissionGateDefinition.Type

// The decision record makes a reasonless gate unrepresentable at the
// schema level: `statedReason` and `reasonCode` are required on both
// the admitted and the excluded branch.
export const DeviceAdmissionDecisionRecord = S.Struct({
  comparison: S.Literals(DeviceAdmissionGateComparisons),
  decision: S.Literals(DeviceAdmissionDecisions),
  deviceClassRef: PublicSafeRef,
  gateRef: PublicSafeRef,
  measuredValue: S.Number,
  measurementKind: S.Literals(DeviceAdmissionGateMeasurementKinds),
  reasonCode: NonEmptyTrimmedString.check(
    S.isPattern(/^device_admission\.public\.(admitted|excluded)_[a-z0-9_]{1,200}$/),
  ),
  statedReason: StatedReason,
  threshold: S.Number,
  unit: PublicSafeRef,
  workClassRef: PublicSafeRef,
})
export type DeviceAdmissionDecisionRecord =
  typeof DeviceAdmissionDecisionRecord.Type

export type DeviceAdmissionGateContract = Readonly<{
  definitionsOnly: true
  gates: ReadonlyArray<DeviceAdmissionGateDefinition>
  liveAdmissionClaim: false
  policyRefs: ReadonlyArray<string>
  schemaVersion: typeof DeviceAdmissionGateContractSchemaVersion
  sourceRefs: ReadonlyArray<string>
}>

export class DeviceAdmissionGateValidationError extends Error {
  readonly _tag = 'DeviceAdmissionGateValidationError'
}

export class DeviceAdmissionGateUnsafeError extends Error {
  readonly _tag = 'DeviceAdmissionGateUnsafeError'
}

// Same posture as the device-capability projection guard: stated
// reasons and rationales are prose, so they get a substring scan for
// private host, wallet, payment, payout, secret, or raw timestamp
// material before any record carrying them is considered projectable.
const unsafeProsePattern =
  /(@|access[_-]?token|api[_-]?key|bearer|cookie|hostname|invoice|lnbc|lntb|lnbcrt|lno1|mac[_-]?address|mnemonic|oauth|payment[_-]?(hash|id|preimage)|payout[_-]?(address|destination)|preimage|private[_-]?key|secret|seed[_-]?phrase|serial[_-]?number|sk-[a-z0-9]|wallet|\/Users\/|\/home\/)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

// Platform-issued reason-code constants are a closed shape; strip them
// before the substring scan so the taxonomy cannot trip its own
// scanner (the dark_capacity.public.wallet_not_ready lesson from the
// live funnel 500 of 2026-06-11).
const platformIssuedReasonPattern =
  /device_admission\.public\.[a-z0-9_]+/g

const assertSafeProse = (label: string, text: string): void => {
  const scrubbed = text.replaceAll(
    platformIssuedReasonPattern,
    'device_admission.public.reason',
  )

  if (unsafeProsePattern.test(scrubbed) || isoTimestampPattern.test(scrubbed)) {
    throw new DeviceAdmissionGateUnsafeError(
      `${label} contains private host, wallet, payment, payout target, secret, or raw timestamp material.`,
    )
  }
}

const assertFiniteNumber = (label: string, value: number): void => {
  if (!Number.isFinite(value)) {
    throw new DeviceAdmissionGateValidationError(
      `${label} requires a finite number.`,
    )
  }
}

export const assertAdmissibleDeviceAdmissionGate = (
  gate: DeviceAdmissionGateDefinition,
): void => {
  assertFiniteNumber(
    `Device admission gate ${gate.gateRef} threshold`,
    gate.requirement.threshold,
  )
  assertSafeProse(
    `Device admission gate ${gate.gateRef} rationale`,
    gate.rationale,
  )
}

const requirementSatisfied = (
  requirement: DeviceAdmissionGateRequirement,
  measuredValue: number,
): boolean =>
  requirement.comparison === 'at_least'
    ? measuredValue >= requirement.threshold
    : measuredValue <= requirement.threshold

const comparisonPhrase: Record<
  DeviceAdmissionGateComparison,
  Readonly<{ failing: string; passing: string }>
> = {
  at_least: { failing: 'below the floor of', passing: 'at or above the floor of' },
  at_most: { failing: 'above the ceiling of', passing: 'at or below the ceiling of' },
}

/**
 * Validates that a decision record is honest and fully reasoned: the
 * stated reason is non-blank, the reason code matches the decision
 * branch and the gate taxonomy shape, and the recorded decision agrees
 * with its own measured value, comparison, and threshold. A reasonless
 * or self-contradicting decision is rejected, never stored.
 */
export const assertAdmissibleDeviceAdmissionDecision = (
  decision: DeviceAdmissionDecisionRecord,
): void => {
  assertFiniteNumber('Device admission decision measuredValue', decision.measuredValue)
  assertFiniteNumber('Device admission decision threshold', decision.threshold)

  if (decision.statedReason.trim().length < 8) {
    throw new DeviceAdmissionGateValidationError(
      'Device admission decisions require a stated, measured reason on both branches; reasonless gates are not admissible.',
    )
  }

  const branchPattern =
    decision.decision === 'admitted'
      ? admittedReasonCodePattern
      : excludedReasonCodePattern

  if (!branchPattern.test(decision.reasonCode)) {
    throw new DeviceAdmissionGateValidationError(
      `Device admission decision ${decision.decision} requires a device_admission.public.${decision.decision}_* reason code.`,
    )
  }

  const satisfied = requirementSatisfied(
    {
      comparison: decision.comparison,
      measurementKind: decision.measurementKind,
      threshold: decision.threshold,
      unit: decision.unit,
    },
    decision.measuredValue,
  )
  const consistent =
    (decision.decision === 'admitted') === satisfied

  if (!consistent) {
    throw new DeviceAdmissionGateValidationError(
      'Device admission decision contradicts its own measured value, comparison, and threshold.',
    )
  }

  assertSafeProse('Device admission decision stated reason', decision.statedReason)
}

/**
 * Evaluates one gate against one measured value and returns the typed
 * decision record. The stated reason is composed from the measured
 * value, the gate requirement, and the gate's rationale, so both the
 * admitted and the excluded branch always carry a measured reason.
 */
export const evaluateDeviceAdmissionGate = (
  input: Readonly<{
    deviceClassRef: string
    gate: DeviceAdmissionGateDefinition
    measuredValue: number
  }>,
): DeviceAdmissionDecisionRecord => {
  assertAdmissibleDeviceAdmissionGate(input.gate)
  assertFiniteNumber('Device admission measured value', input.measuredValue)

  const satisfied = requirementSatisfied(
    input.gate.requirement,
    input.measuredValue,
  )
  const phrase = satisfied
    ? comparisonPhrase[input.gate.requirement.comparison].passing
    : comparisonPhrase[input.gate.requirement.comparison].failing
  const decision: DeviceAdmissionDecisionRecord = {
    comparison: input.gate.requirement.comparison,
    decision: satisfied ? 'admitted' : 'excluded',
    deviceClassRef: input.deviceClassRef,
    gateRef: input.gate.gateRef,
    measuredValue: input.measuredValue,
    measurementKind: input.gate.requirement.measurementKind,
    reasonCode: satisfied
      ? input.gate.admittedReasonCode
      : input.gate.excludedReasonCode,
    statedReason: `measured ${input.gate.requirement.measurementKind} ${input.measuredValue} ${input.gate.requirement.unit} is ${phrase} ${input.gate.requirement.threshold} ${input.gate.requirement.unit} for ${input.gate.workClassRef}: ${input.gate.rationale}`,
    threshold: input.gate.requirement.threshold,
    unit: input.gate.requirement.unit,
    workClassRef: input.gate.workClassRef,
  }

  assertAdmissibleDeviceAdmissionDecision(decision)

  return decision
}

/**
 * The funnel-compatible surfacing of a decision: its reason code, ready
 * to join `darkCapacityReasonRefs` (exclusions) or evidence refs
 * (admissions) on a `PylonCapacityFunnelRecord` without further
 * translation.
 */
export const funnelReasonRefForDeviceAdmissionDecision = (
  decision: DeviceAdmissionDecisionRecord,
): string => decision.reasonCode

const deepFreeze = <T>(value: T): T => {
  if (typeof value === 'object' && value !== null && !Object.isFrozen(value)) {
    Object.freeze(value)
    Object.values(value).forEach(deepFreeze)
  }

  return value
}

// Seeded example gate set. These are DEFINITIONS demonstrating the
// reasoned-admission pattern, not live admission policy: no device has
// been measured against them, and no funnel row may cite them as a
// live admission claim until receipted device evidence exists.
export const EXAMPLE_DEVICE_ADMISSION_GATE_SET: ReadonlyArray<DeviceAdmissionGateDefinition> =
  deepFreeze([
    {
      admittedReasonCode:
        'device_admission.public.admitted_bf16_attention_throughput_at_or_above_floor',
      excludedReasonCode:
        'device_admission.public.excluded_bf16_attention_throughput_below_floor',
      gateRef: 'gate.device_admission.example.bf16_attention_throughput_floor.v1',
      rationale:
        'bf16-class work requires native bf16 attention throughput; Pluralis excluded T4/V100 because those cards would emulate BF16 slower than FP32, and that measured reason ships with the gate.',
      requirement: {
        comparison: 'at_least',
        measurementKind: 'attention_throughput',
        threshold: 2000,
        unit: 'megaflops',
      },
      sourceRefs: [
        'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md#p1.4',
        'issue.github.openagents.4852',
      ],
      workClassRef: 'work_class.example.bf16_pipeline_training',
    },
    {
      admittedReasonCode:
        'device_admission.public.admitted_host_ram_headroom_at_or_above_floor',
      excludedReasonCode:
        'device_admission.public.excluded_host_ram_headroom_below_floor',
      gateRef: 'gate.device_admission.example.host_ram_headroom_floor.v1',
      rationale:
        'optimizer-offload work classes keep Adam moments in host RAM; the Pluralis contributor shape is 24 GB GPU plus 80 GB system RAM, so host-RAM headroom is a binding constraint and must be measured, not assumed.',
      requirement: {
        comparison: 'at_least',
        measurementKind: 'host_ram_headroom_gb',
        threshold: 80,
        unit: 'gigabytes',
      },
      sourceRefs: [
        'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md#p1.4',
        'issue.github.openagents.4852',
      ],
      workClassRef: 'work_class.example.optimizer_offload_training',
    },
    {
      admittedReasonCode:
        'device_admission.public.admitted_sustained_throughput_ratio_at_or_above_floor',
      excludedReasonCode:
        'device_admission.public.excluded_sustained_throughput_ratio_below_floor',
      gateRef:
        'gate.device_admission.example.sustained_throughput_ratio_floor.v1',
      rationale:
        'collective training runs at the pace of its slowest member; one thermally throttling GPU collapsed a 14-node Pluralis collective, so burst benchmarks alone overstate sustained capability.',
      requirement: {
        comparison: 'at_least',
        measurementKind: 'sustained_vs_burst_throughput_ratio',
        threshold: 0.8,
        unit: 'ratio',
      },
      sourceRefs: [
        'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md#p1.4',
        'issue.github.openagents.4852',
      ],
      workClassRef: 'work_class.example.sustained_collective_training',
    },
  ])

/**
 * The versioned, frozen, JSON-able gate-definition contract. Psionic's
 * preflight qualification mirrors this exact structure instead of
 * duplicating thresholds; the schema version string is the cross-repo
 * compatibility key. `definitionsOnly`/`liveAdmissionClaim` make the
 * non-claim explicit in the exported payload itself.
 */
export const exportDeviceAdmissionGateContract = (
  gates: ReadonlyArray<DeviceAdmissionGateDefinition> = EXAMPLE_DEVICE_ADMISSION_GATE_SET,
): DeviceAdmissionGateContract => {
  for (const gate of gates) {
    assertAdmissibleDeviceAdmissionGate(gate)
  }

  return deepFreeze({
    definitionsOnly: true,
    gates,
    liveAdmissionClaim: false,
    policyRefs: [
      'policy.public.device_admission.every_decision_carries_stated_measured_reason',
      'policy.public.device_admission.gate_definitions_are_not_live_admission_claims',
      'policy.public.device_admission.psionic_preflight_consumes_this_contract',
    ],
    schemaVersion: DeviceAdmissionGateContractSchemaVersion,
    sourceRefs: [
      'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md#p1.4',
      'issue.github.openagents.4852',
    ],
  })
}
