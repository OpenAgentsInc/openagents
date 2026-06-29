import type { AutopilotMissionBriefingProjection } from './autopilot-mission-briefing'
import type { CodingAutopilotMissionRecord } from './coding-autopilot-missions'

/**
 * Mission Briefing citation: the verifiable link between a live coding
 * Autopilot mission (CodingAutopilotMissionRecord) and the Mission Briefing
 * projection it cites at GET /api/autopilot/work/{workOrderRef}/briefing.
 *
 * This closes the structural gap for the
 * `mission_briefing_live_mission_citation_missing` blocker: it lets a reviewer
 * confirm that a given mission actually points its `latestBriefingRef` at the
 * briefing JSON produced for its work order, derive whether the mission is
 * waiting on a caller decision, and surface only public-safe proof/verification
 * refs. It does NOT manufacture a live mission and grants no authority.
 */

export type AutopilotMissionBriefingCitationNextActionState =
  AutopilotMissionBriefingProjection['decisionsWaiting']['nextActionState']

/**
 * Next-action states that require a caller decision before the mission can
 * progress (review delivered work, fund payment, supply input, revise, or
 * resolve a blocker). `accepted`, `ready`, `rejected`, and `retry_later` are
 * either terminal or automated and do not wait on the caller.
 */
const DECISION_NEEDED_STATES: ReadonlySet<AutopilotMissionBriefingCitationNextActionState> =
  new Set<AutopilotMissionBriefingCitationNextActionState>([
    'blocked',
    'delivered',
    'needs_input',
    'payment_required',
    'revision_required',
  ])

export type AutopilotMissionBriefingCitation = Readonly<{
  briefingCitedByMission: boolean
  briefingRef: string
  citationRef: string
  citedBriefingRef: string | null
  decisionNeeded: boolean
  decisionReasonRefs: ReadonlyArray<string>
  generatedAt: string
  kind: 'autopilot_mission_briefing_citation'
  missionRef: string
  missionStatus: CodingAutopilotMissionRecord['status']
  nextActionState: AutopilotMissionBriefingCitationNextActionState
  proofRefs: ReadonlyArray<string>
  publicSafe: true
  riskLevel: AutopilotMissionBriefingProjection['risk']['level']
  state: AutopilotMissionBriefingProjection['state']
  verificationRefs: ReadonlyArray<string>
  workOrderRef: string
}>

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const unsafeRefPattern =
  /(@|auth\.json|bearer|cookie|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|invoice|lnbc|lntb|mnemonic|oauth|preimage|private[_-]?key|secret|sk-[a-z0-9]|token|wallet|webhook)/i

export class AutopilotMissionBriefingCitationUnsafe extends Error {
  readonly _tag = 'AutopilotMissionBriefingCitationUnsafe'
  constructor(reason: string) {
    super(reason)
    this.name = 'AutopilotMissionBriefingCitationUnsafe'
  }
}

const assertPublicSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = refs.find(
    ref => !safeRefPattern.test(ref) || unsafeRefPattern.test(ref),
  )
  if (unsafe !== undefined) {
    throw new AutopilotMissionBriefingCitationUnsafe(
      `${label} contains a private, secret, payment, wallet, or malformed ref.`,
    )
  }
}

/**
 * Returns true when the mission's recorded `latestBriefingRef` matches the
 * `briefingRef` of the supplied briefing projection — i.e. the live mission is
 * actually citing this briefing JSON.
 */
export const missionCitesBriefing = (
  mission: CodingAutopilotMissionRecord,
  briefing: AutopilotMissionBriefingProjection,
): boolean => mission.latestBriefingRef === briefing.briefingRef

/**
 * Builds a public-safe citation linking a live mission to the Mission Briefing
 * projection it cites. Throws AutopilotMissionBriefingCitationUnsafe if any
 * surfaced ref is private/secret. The caller must still hold owner-granted
 * read authority for the underlying work order; this projection grants none.
 */
export const missionBriefingCitation = (
  input: Readonly<{
    briefing: AutopilotMissionBriefingProjection
    mission: CodingAutopilotMissionRecord
    nowIso: string
  }>,
): AutopilotMissionBriefingCitation => {
  const { briefing, mission, nowIso } = input

  const citedBriefingRef = mission.latestBriefingRef
  const proofRefs = briefing.receipts.proofRefs
  const verificationRefs = briefing.receipts.verificationRefs
  const decisionReasonRefs = briefing.decisionsWaiting.reasonRefs

  assertPublicSafeRefs('mission identity refs', [
    mission.missionRef,
    briefing.workOrderRef,
    briefing.briefingRef,
    ...(citedBriefingRef === null ? [] : [citedBriefingRef]),
  ])
  assertPublicSafeRefs('citation proof refs', proofRefs)
  assertPublicSafeRefs('citation verification refs', verificationRefs)
  assertPublicSafeRefs('citation decision reason refs', decisionReasonRefs)

  return {
    briefingCitedByMission: missionCitesBriefing(mission, briefing),
    briefingRef: briefing.briefingRef,
    citationRef: `citation.${mission.missionRef}.${briefing.briefingRef}`,
    citedBriefingRef,
    decisionNeeded: DECISION_NEEDED_STATES.has(
      briefing.decisionsWaiting.nextActionState,
    ),
    decisionReasonRefs,
    generatedAt: nowIso,
    kind: 'autopilot_mission_briefing_citation',
    missionRef: mission.missionRef,
    missionStatus: mission.status,
    nextActionState: briefing.decisionsWaiting.nextActionState,
    proofRefs,
    publicSafe: true,
    riskLevel: briefing.risk.level,
    state: briefing.state,
    verificationRefs,
    workOrderRef: briefing.workOrderRef,
  }
}
