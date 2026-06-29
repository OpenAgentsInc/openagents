import { Schema as S } from 'effect'

import {
  QwenRemotePylonFineTuneGateInput,
  QwenRemotePylonFineTuneGateProjection,
  projectQwenRemotePylonFineTuneGate,
} from './qwen-remote-pylon-finetune-gate'
import type { QwenRemotePylonFineTuneGateInput as QwenRemotePylonFineTuneGateInputType } from './qwen-remote-pylon-finetune-gate'

export const QwenRemotePylonLiveTrainingSmokeState = S.Literals([
  'blocked',
  'green',
])
export type QwenRemotePylonLiveTrainingSmokeState =
  typeof QwenRemotePylonLiveTrainingSmokeState.Type

export class QwenRemotePylonLiveTrainingCandidate extends S.Class<QwenRemotePylonLiveTrainingCandidate>(
  'QwenRemotePylonLiveTrainingCandidate',
)({
  capabilityRefs: S.Array(S.String),
  displayName: S.String,
  latestHeartbeatDisplay: S.NullOr(S.String),
  latestHeartbeatStatus: S.NullOr(S.String),
  pylonRef: S.String,
  status: S.String,
  walletReady: S.Boolean,
}) {}

export class QwenRemotePylonLiveTrainingPreflight extends S.Class<QwenRemotePylonLiveTrainingPreflight>(
  'QwenRemotePylonLiveTrainingPreflight',
)({
  assignmentCapabilityRef: S.String,
  blockerRefs: S.Array(S.String),
  candidatePylonRefs: S.Array(S.String),
  requiredCapabilityRef: S.String,
  selectedPylonRefs: S.Array(S.String),
  state: QwenRemotePylonLiveTrainingSmokeState,
}) {}

export class QwenRemotePylonLiveTrainingSmokeProjection extends S.Class<QwenRemotePylonLiveTrainingSmokeProjection>(
  'QwenRemotePylonLiveTrainingSmokeProjection',
)({
  blockerRefs: S.Array(S.String),
  gate: QwenRemotePylonFineTuneGateProjection,
  preflight: S.NullOr(QwenRemotePylonLiveTrainingPreflight),
  state: QwenRemotePylonLiveTrainingSmokeState,
}) {}

export const QWEN_REMOTE_PYLON_TRAINING_REQUIRED_CAPABILITY_REF =
  'capability.public.pylon.fine_tuning_training'

export const QWEN_REMOTE_PYLON_ASSIGNMENT_READY_CAPABILITY_REF =
  'capability.pylon.assignment_ready'

const decodeCandidate = S.decodeUnknownSync(QwenRemotePylonLiveTrainingCandidate)
const decodePreflight = S.decodeUnknownSync(QwenRemotePylonLiveTrainingPreflight)
const decodeSmokeProjection = S.decodeUnknownSync(
  QwenRemotePylonLiveTrainingSmokeProjection,
)

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const syntheticPylonPattern =
  /(?:\.live_smoke\.|\.packaged_.*_smoke\.|smoke|fixture|loopback|demo|canary)/i

const heartbeatLooksFresh = (candidate: QwenRemotePylonLiveTrainingCandidate): boolean =>
  candidate.latestHeartbeatStatus === 'online' &&
  (candidate.latestHeartbeatDisplay === 'Just now' ||
    candidate.latestHeartbeatDisplay === '1 minute ago' ||
    /^[2-5] minutes ago$/.test(candidate.latestHeartbeatDisplay ?? ''))

const isSyntheticCandidate = (
  candidate: QwenRemotePylonLiveTrainingCandidate,
): boolean =>
  syntheticPylonPattern.test(candidate.pylonRef) ||
  syntheticPylonPattern.test(candidate.displayName)

const hasCapability = (
  candidate: QwenRemotePylonLiveTrainingCandidate,
  capabilityRef: string,
): boolean => candidate.capabilityRefs.includes(capabilityRef)

const selectedCandidates = (
  candidates: ReadonlyArray<QwenRemotePylonLiveTrainingCandidate>,
  selectedPylonRefs: ReadonlyArray<string>,
): ReadonlyArray<QwenRemotePylonLiveTrainingCandidate> => {
  const selected = new Set(selectedPylonRefs)

  return selected.size === 0
    ? candidates
    : candidates.filter(candidate => selected.has(candidate.pylonRef))
}

export const projectQwenRemotePylonLiveTrainingPreflight = (
  input: Readonly<{
    assignmentCapabilityRef?: string
    candidates: ReadonlyArray<unknown>
    requiredCapabilityRef?: string
    selectedPylonRefs?: ReadonlyArray<string>
  }>,
): QwenRemotePylonLiveTrainingPreflight => {
  const assignmentCapabilityRef =
    input.assignmentCapabilityRef ?? QWEN_REMOTE_PYLON_ASSIGNMENT_READY_CAPABILITY_REF
  const requiredCapabilityRef =
    input.requiredCapabilityRef ?? QWEN_REMOTE_PYLON_TRAINING_REQUIRED_CAPABILITY_REF
  const selectedPylonRefs = uniqueRefs(input.selectedPylonRefs ?? [])
  const candidates = input.candidates.map(candidate => decodeCandidate(candidate))
  const inspected = selectedCandidates(candidates, selectedPylonRefs)
  const missingRefs = selectedPylonRefs.filter(
    pylonRef => !candidates.some(candidate => candidate.pylonRef === pylonRef),
  )
  const liveCandidates = inspected.filter(
    candidate =>
      candidate.status === 'active' &&
      candidate.walletReady &&
      heartbeatLooksFresh(candidate) &&
      !isSyntheticCandidate(candidate) &&
      hasCapability(candidate, assignmentCapabilityRef) &&
      hasCapability(candidate, requiredCapabilityRef),
  )
  const syntheticRefs = inspected
    .filter(isSyntheticCandidate)
    .map(candidate => candidate.pylonRef)
  const missingAssignmentCapabilityRefs = inspected
    .filter(candidate => !hasCapability(candidate, assignmentCapabilityRef))
    .map(candidate => candidate.pylonRef)
  const missingTrainingCapabilityRefs = inspected
    .filter(candidate => !hasCapability(candidate, requiredCapabilityRef))
    .map(candidate => candidate.pylonRef)
  const staleOrNotReadyRefs = inspected
    .filter(
      candidate =>
        candidate.status !== 'active' ||
        !candidate.walletReady ||
        !heartbeatLooksFresh(candidate),
    )
    .map(candidate => candidate.pylonRef)
  const blockerRefs = [
    ...(liveCandidates.length < 2
      ? ['blocker.public.qwen_remote_training.live_training_pylons_missing']
      : []),
    ...(missingRefs.length > 0
      ? ['blocker.public.qwen_remote_training.selected_pylons_missing']
      : []),
    ...(syntheticRefs.length > 0
      ? ['blocker.public.qwen_remote_training.synthetic_pylons_selected']
      : []),
    ...(missingAssignmentCapabilityRefs.length > 0
      ? ['blocker.public.qwen_remote_training.assignment_capability_missing']
      : []),
    ...(missingTrainingCapabilityRefs.length > 0
      ? ['blocker.public.qwen_remote_training.required_capability_missing']
      : []),
    ...(staleOrNotReadyRefs.length > 0
      ? ['blocker.public.qwen_remote_training.pylons_not_fresh_wallet_ready']
      : []),
  ]

  return decodePreflight({
    assignmentCapabilityRef,
    blockerRefs: uniqueRefs(blockerRefs),
    candidatePylonRefs: uniqueRefs(liveCandidates.map(candidate => candidate.pylonRef)),
    requiredCapabilityRef,
    selectedPylonRefs,
    state: blockerRefs.length === 0 ? 'green' : 'blocked',
  })
}

export const projectQwenRemotePylonLiveTrainingSmoke = (
  input: Readonly<{
    gateInput: QwenRemotePylonFineTuneGateInputType
    preflight?: QwenRemotePylonLiveTrainingPreflight | null
  }>,
): QwenRemotePylonLiveTrainingSmokeProjection => {
  const gate = projectQwenRemotePylonFineTuneGate(
    S.decodeUnknownSync(QwenRemotePylonFineTuneGateInput)(input.gateInput),
  )
  const preflight = input.preflight ?? null
  const preflightCandidateRefs = new Set(preflight?.candidatePylonRefs ?? [])
  const missingLiveWorkerRefs =
    preflight === null
      ? []
      : gate.remoteWorkerRefs.filter(workerRef => !preflightCandidateRefs.has(workerRef))
  const blockerRefs = uniqueRefs([
    ...gate.blockerRefs,
    ...(preflight?.blockerRefs ?? []),
    ...(missingLiveWorkerRefs.length > 0
      ? ['blocker.public.qwen_remote_training.worker_preflight_missing']
      : []),
  ])
  const state =
    gate.decision === 'ready' &&
    (preflight === null || preflight.state === 'green') &&
    missingLiveWorkerRefs.length === 0
      ? 'green'
      : 'blocked'

  return decodeSmokeProjection({
    blockerRefs,
    gate,
    preflight,
    state,
  })
}
