import { Schema as S } from 'effect'

import {
  ProbeGepaStage0NoSpendCampaignProjection,
  projectProbeGepaStage0NoSpendCampaign,
} from './probe-gepa-stage0-no-spend-campaign'
import type { ProbeGepaStage0NoSpendCampaignInput } from './probe-gepa-stage0-no-spend-campaign'

export const ProbeGepaStage0LiveCampaignSmokeState = S.Literals([
  'blocked',
  'green',
])
export type ProbeGepaStage0LiveCampaignSmokeState =
  typeof ProbeGepaStage0LiveCampaignSmokeState.Type

export class ProbeGepaStage0PylonCandidate extends S.Class<ProbeGepaStage0PylonCandidate>(
  'ProbeGepaStage0PylonCandidate',
)({
  capabilityRefs: S.Array(S.String),
  displayName: S.String,
  latestHeartbeatDisplay: S.NullOr(S.String),
  pylonRef: S.String,
  status: S.String,
  walletReady: S.Boolean,
}) {}

export class ProbeGepaStage0LivePylonPreflight extends S.Class<ProbeGepaStage0LivePylonPreflight>(
  'ProbeGepaStage0LivePylonPreflight',
)({
  blockerRefs: S.Array(S.String),
  candidatePylonRefs: S.Array(S.String),
  requiredCapabilityRef: S.String,
  selectedPylonRefs: S.Array(S.String),
  state: ProbeGepaStage0LiveCampaignSmokeState,
}) {}

export class ProbeGepaStage0LiveCampaignSmokeProjection extends S.Class<ProbeGepaStage0LiveCampaignSmokeProjection>(
  'ProbeGepaStage0LiveCampaignSmokeProjection',
)({
  blockerRefs: S.Array(S.String),
  campaign: ProbeGepaStage0NoSpendCampaignProjection,
  preflight: S.NullOr(ProbeGepaStage0LivePylonPreflight),
  state: ProbeGepaStage0LiveCampaignSmokeState,
}) {}

export const PROBE_GEPA_STAGE0_REQUIRED_CAPABILITY_REF =
  'cap.gepa.retained.v1'

const decodeCandidate = S.decodeUnknownSync(ProbeGepaStage0PylonCandidate)
const decodePreflight = S.decodeUnknownSync(ProbeGepaStage0LivePylonPreflight)
const decodeSmokeProjection = S.decodeUnknownSync(
  ProbeGepaStage0LiveCampaignSmokeProjection,
)

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const syntheticPylonPattern =
  /(?:\.live_smoke\.|\.packaged_.*_smoke\.|smoke|fixture|loopback|demo)/i

const heartbeatLooksFresh = (label: string | null): boolean =>
  label === 'Just now' ||
  label === '1 minute ago' ||
  /^[2-5] minutes ago$/.test(label ?? '')

const isSyntheticCandidate = (
  candidate: ProbeGepaStage0PylonCandidate,
): boolean =>
  syntheticPylonPattern.test(candidate.pylonRef) ||
  syntheticPylonPattern.test(candidate.displayName)

const hasRequiredCapability = (
  candidate: ProbeGepaStage0PylonCandidate,
  requiredCapabilityRef: string,
): boolean => candidate.capabilityRefs.includes(requiredCapabilityRef)

const selectedCandidates = (
  candidates: ReadonlyArray<ProbeGepaStage0PylonCandidate>,
  selectedPylonRefs: ReadonlyArray<string>,
): ReadonlyArray<ProbeGepaStage0PylonCandidate> => {
  const selected = new Set(selectedPylonRefs)

  return selected.size === 0
    ? candidates
    : candidates.filter(candidate => selected.has(candidate.pylonRef))
}

export const projectProbeGepaStage0LivePylonPreflight = (
  input: Readonly<{
    candidates: ReadonlyArray<unknown>
    requiredCapabilityRef?: string
    selectedPylonRefs?: ReadonlyArray<string>
  }>,
): ProbeGepaStage0LivePylonPreflight => {
  const requiredCapabilityRef =
    input.requiredCapabilityRef ?? PROBE_GEPA_STAGE0_REQUIRED_CAPABILITY_REF
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
      heartbeatLooksFresh(candidate.latestHeartbeatDisplay) &&
      !isSyntheticCandidate(candidate) &&
      hasRequiredCapability(candidate, requiredCapabilityRef),
  )
  const syntheticRefs = inspected
    .filter(isSyntheticCandidate)
    .map(candidate => candidate.pylonRef)
  const missingCapabilityRefs = inspected
    .filter(candidate => !hasRequiredCapability(candidate, requiredCapabilityRef))
    .map(candidate => candidate.pylonRef)
  const staleOrNotReadyRefs = inspected
    .filter(
      candidate =>
        candidate.status !== 'active' ||
        !candidate.walletReady ||
        !heartbeatLooksFresh(candidate.latestHeartbeatDisplay),
    )
    .map(candidate => candidate.pylonRef)
  const blockerRefs = [
    ...(liveCandidates.length < 2
      ? ['blocker.probe_gepa_stage0.live_gepa_capable_pylons_missing']
      : []),
    ...(missingRefs.length > 0
      ? ['blocker.probe_gepa_stage0.selected_pylons_missing']
      : []),
    ...(syntheticRefs.length > 0
      ? ['blocker.probe_gepa_stage0.synthetic_pylons_selected']
      : []),
    ...(missingCapabilityRefs.length > 0
      ? ['blocker.probe_gepa_stage0.required_capability_missing']
      : []),
    ...(staleOrNotReadyRefs.length > 0
      ? ['blocker.probe_gepa_stage0.pylons_not_fresh_wallet_ready']
      : []),
  ]

  return decodePreflight({
    blockerRefs: uniqueRefs(blockerRefs),
    candidatePylonRefs: uniqueRefs(liveCandidates.map(candidate => candidate.pylonRef)),
    requiredCapabilityRef,
    selectedPylonRefs,
    state: blockerRefs.length === 0 ? 'green' : 'blocked',
  })
}

export const projectProbeGepaStage0LiveCampaignSmoke = (
  input: Readonly<{
    campaignInput: ProbeGepaStage0NoSpendCampaignInput
    preflight?: ProbeGepaStage0LivePylonPreflight | null
  }>,
): ProbeGepaStage0LiveCampaignSmokeProjection => {
  const campaign = projectProbeGepaStage0NoSpendCampaign(input.campaignInput)
  const preflight = input.preflight ?? null
  const blockerRefs = uniqueRefs([
    ...campaign.blockerRefs,
    ...(preflight?.blockerRefs ?? []),
  ])
  const state =
    campaign.state === 'green' &&
    (preflight === null || preflight.state === 'green')
      ? 'green'
      : 'blocked'

  return decodeSmokeProjection({
    blockerRefs,
    campaign,
    preflight,
    state,
  })
}
