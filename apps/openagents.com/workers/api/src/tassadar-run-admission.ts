/**
 * Run-level admission decision for executor-trace contributors joining a
 * Tassadar run (Tassadar Launch Step 2, openagents#5007). Combines three
 * reasoned gates so every admit/exclude carries a stated measured reason:
 *
 *   1. Receipted executor capability (W4.1, #4750): the device must hold the
 *      `capability.tassadar_poc.numeric_model_executor` claim WITH a self-test
 *      receipt minted by a real digest-pinned execution. An unreceipted or
 *      missing claim is refused.
 *   2. Independent-contributor check: owner-operated nodes are excluded — they
 *      do not count as stranger contributor proof for the launch.
 *   3. Reasoned device-admission gate (#4852): host-RAM headroom floor for the
 *      bounded ALM numeric trace, evaluated with a measured reason on both the
 *      admitted and the excluded branch.
 *
 * This is the admission half of Step 2; the dispatch half is the existing
 * window-lease claim path against the run's executor-trace window, and
 * verification is the already run-aware `exact_trace_replay` challenge. No
 * payout, settlement, or serving authority is granted here.
 */
import { Schema as S } from 'effect'

import {
  TASSADAR_EXECUTOR_CAPABILITY_UNRECEIPTED_REFUSAL_REF,
  admitTassadarExecutorCapabilityClaim,
} from './tassadar-capability-admission'
import {
  type DeviceAdmissionDecisionRecord,
  type DeviceAdmissionGateDefinition,
  evaluateDeviceAdmissionGate,
  funnelReasonRefForDeviceAdmissionDecision,
} from './training-device-admission-gates'
import { TrainingPublicSafePylonRef } from './training-run-window-authority'

export const TASSADAR_RUN_ADMISSION_OWNER_OPERATED_EXCLUSION_REF =
  'device_admission.public.excluded_owner_operated_not_independent_contributor'

/**
 * Reasoned host-RAM headroom gate for executor-trace work (#4852, #5007).
 * The bounded ALM numeric window and its replay buffers live in host RAM, so a
 * small measured headroom floor keeps a device off executor work it would
 * thrash; the measured reason ships with the gate on both branches.
 */
export const TASSADAR_EXECUTOR_ADMISSION_GATE: DeviceAdmissionGateDefinition = {
  admittedReasonCode:
    'device_admission.public.admitted_tassadar_executor_host_ram_at_or_above_floor',
  excludedReasonCode:
    'device_admission.public.excluded_tassadar_executor_host_ram_below_floor',
  gateRef: 'gate.device_admission.tassadar_executor.host_ram_headroom_floor.v1',
  rationale:
    'Tassadar executor-trace replay holds the bounded ALM numeric window and its replay buffers in host RAM, so a small headroom floor keeps a device off executor work it would thrash, and that measured reason ships with the gate.',
  requirement: {
    comparison: 'at_least',
    measurementKind: 'host_ram_headroom_gb',
    threshold: 2,
    unit: 'gigabytes',
  },
  sourceRefs: [
    'docs/tassadar/README.md',
    'issue.github.openagents.4852',
    'issue.github.openagents.5007',
  ],
  workClassRef: 'work_class.tassadar_executor.alm_numeric_trace',
}

export const TrainingRunAdmissionRequest = S.Struct({
  capabilityRefs: S.Array(
    S.Trim.check(S.isNonEmpty(), S.isMaxLength(200)),
  ).check(S.isMaxLength(64)),
  deviceClassRef: S.optionalKey(TrainingPublicSafePylonRef),
  hostRamHeadroomGb: S.Number.check(
    S.isFinite(),
    S.isBetween({ minimum: 0, maximum: 1_000_000 }),
  ),
  ownerOperated: S.optionalKey(S.Boolean),
  pylonRef: TrainingPublicSafePylonRef,
})
export type TrainingRunAdmissionRequest =
  typeof TrainingRunAdmissionRequest.Type

export type TassadarRunAdmissionDecision = Readonly<{
  capabilityState: 'admitted' | 'not_claimed' | 'refused'
  decision: 'admitted' | 'excluded'
  deviceGate: DeviceAdmissionDecisionRecord
  ownerOperated: boolean
  pylonRef: string
  reasonRefs: ReadonlyArray<string>
  statedReasons: ReadonlyArray<string>
}>

const uniqueSorted = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

/**
 * Decides executor-trace run admission for one contributor. Pure: the route
 * supplies the (already-existing) training run; this composes the three gates
 * into one typed, fully-reasoned admit/exclude decision.
 */
export const decideTassadarRunAdmission = (
  request: TrainingRunAdmissionRequest,
): TassadarRunAdmissionDecision => {
  const capability = admitTassadarExecutorCapabilityClaim(request.capabilityRefs)
  const ownerOperated = request.ownerOperated ?? false
  const deviceGate = evaluateDeviceAdmissionGate({
    deviceClassRef: request.deviceClassRef ?? request.pylonRef,
    gate: TASSADAR_EXECUTOR_ADMISSION_GATE,
    measuredValue: request.hostRamHeadroomGb,
  })

  const capabilityOk = capability.state === 'admitted'
  const deviceOk = deviceGate.decision === 'admitted'
  const admitted = capabilityOk && deviceOk && !ownerOperated

  const reasonRefs: Array<string> = []
  const statedReasons: Array<string> = []

  if (!capabilityOk) {
    reasonRefs.push(
      ...(capability.refusalRefs.length > 0
        ? capability.refusalRefs
        : [TASSADAR_EXECUTOR_CAPABILITY_UNRECEIPTED_REFUSAL_REF]),
    )
    statedReasons.push(
      `executor capability claim is ${capability.state}; a receipted self-test from a real digest-pinned execution is required for executor-trace work`,
    )
  }

  if (ownerOperated) {
    reasonRefs.push(TASSADAR_RUN_ADMISSION_OWNER_OPERATED_EXCLUSION_REF)
    statedReasons.push(
      'owner-operated node does not count as independent contributor proof for the Tassadar run',
    )
  }

  reasonRefs.push(funnelReasonRefForDeviceAdmissionDecision(deviceGate))
  statedReasons.push(deviceGate.statedReason)

  return {
    capabilityState: capability.state,
    decision: admitted ? 'admitted' : 'excluded',
    deviceGate,
    ownerOperated,
    pylonRef: request.pylonRef,
    reasonRefs: uniqueSorted(reasonRefs),
    statedReasons,
  }
}
