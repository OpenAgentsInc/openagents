import { Schema as S } from 'effect'

import type { BlueprintMissionBriefingAudience } from './blueprint/schemas/continuation-mission-briefing'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const CodingAutopilotMissionWorkKind = S.Literals([
  'coding',
  'site',
])
export type CodingAutopilotMissionWorkKind =
  typeof CodingAutopilotMissionWorkKind.Type

export const CodingAutopilotMissionStatus = S.Literals([
  'accepted',
  'archived',
  'blocked',
  'cancelled',
  'delivered',
  'proposed',
  'queued',
  'running',
  'waiting_for_input',
  'waiting_for_review',
])
export type CodingAutopilotMissionStatus =
  typeof CodingAutopilotMissionStatus.Type

export class CodingAutopilotMissionRecord extends S.Class<CodingAutopilotMissionRecord>(
  'CodingAutopilotMissionRecord',
)({
  accountLeaseRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  assignmentRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  budgetRefs: S.Array(S.String),
  createdAtIso: S.String,
  customerRefs: S.Array(S.String),
  id: S.String,
  latestBriefingRef: S.NullOr(S.String),
  missionRef: S.String,
  nextOrderRefs: S.Array(S.String),
  objectiveStackRefs: S.Array(S.String),
  ownerRefs: S.Array(S.String),
  routeScorecardRefs: S.Array(S.String),
  status: CodingAutopilotMissionStatus,
  teamRefs: S.Array(S.String),
  updatedAtIso: S.String,
  workKind: CodingAutopilotMissionWorkKind,
  workroomRefs: S.Array(S.String),
}) {}

export class CodingAutopilotMissionProjection extends S.Class<CodingAutopilotMissionProjection>(
  'CodingAutopilotMissionProjection',
)({
  accountLeaseRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  assignmentRefs: S.Array(S.String),
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  blockerRefs: S.Array(S.String),
  budgetRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  customerRefs: S.Array(S.String),
  id: S.String,
  latestBriefingRef: S.NullOr(S.String),
  missionRef: S.String,
  nextOrderRefs: S.Array(S.String),
  objectiveStackRefs: S.Array(S.String),
  ownerRefs: S.Array(S.String),
  routeScorecardRefs: S.Array(S.String),
  status: CodingAutopilotMissionStatus,
  statusLabel: S.String,
  teamRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  workKind: CodingAutopilotMissionWorkKind,
  workroomRefs: S.Array(S.String),
}) {}

export class CodingAutopilotMissionUnsafe extends S.TaggedErrorClass<CodingAutopilotMissionUnsafe>()(
  'CodingAutopilotMissionUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const universallyUnsafeRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|preimage)|preimage|private[_-]?key|private[_-]?repo|provider[_-]?(account|grant|payload|token)|raw[_-]?(email|invoice|payment|payload|patch|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|token|wallet|webhook[_-]?secret|workroom[_-]?private)/i
const publicUnsafeRefPattern =
  /(account[_-]?lease|budget[_-]?private|github\.com\/[^:/]+\/private|private[_-]?repo|provider[_-]?account|route[_-]?scorecard)/i
const customerUnsafeRefPattern =
  /(account[_-]?lease|github\.com\/[^:/]+\/private|private[_-]?repo|provider[_-]?account|route[_-]?scorecard)/i
const teamUnsafeRefPattern =
  /(account[_-]?lease|provider[_-]?account)/i

const statusLabelByStatus: Record<CodingAutopilotMissionStatus, string> = {
  accepted: 'Accepted',
  archived: 'Archived',
  blocked: 'Blocked',
  cancelled: 'Cancelled',
  delivered: 'Delivered',
  proposed: 'Proposed',
  queued: 'Queued',
  running: 'Running',
  waiting_for_input: 'Waiting for input',
  waiting_for_review: 'Waiting for review',
}

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertNoUniversalPrivateMaterial = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    universallyUnsafeRefPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new CodingAutopilotMissionUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, customer, private repo, or raw artifact material.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: BlueprintMissionBriefingAudience,
): RegExp | null => {
  if (audience === 'public') {
    return publicUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const safeRefsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: BlueprintMissionBriefingAudience,
): ReadonlyArray<string> => {
  assertNoUniversalPrivateMaterial(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const safeNullableRefForAudience = (
  label: string,
  ref: string | null,
  audience: BlueprintMissionBriefingAudience,
): string | null => {
  if (ref === null) {
    return null
  }

  return safeRefsForAudience(label, [ref], audience)[0] ?? null
}

const assertRecordSafe = (record: CodingAutopilotMissionRecord): void => {
  assertNoUniversalPrivateMaterial('mission identity refs', [
    record.id,
    record.missionRef,
    record.latestBriefingRef ?? '',
  ])
  assertNoUniversalPrivateMaterial('mission objective refs', record.objectiveStackRefs)
  assertNoUniversalPrivateMaterial('mission owner refs', record.ownerRefs)
  assertNoUniversalPrivateMaterial('mission customer refs', record.customerRefs)
  assertNoUniversalPrivateMaterial('mission team refs', record.teamRefs)
  assertNoUniversalPrivateMaterial('mission workroom refs', record.workroomRefs)
  assertNoUniversalPrivateMaterial('mission assignment refs', record.assignmentRefs)
  assertNoUniversalPrivateMaterial('mission route refs', record.routeScorecardRefs)
  assertNoUniversalPrivateMaterial('mission account lease refs', record.accountLeaseRefs)
  assertNoUniversalPrivateMaterial('mission budget refs', record.budgetRefs)
  assertNoUniversalPrivateMaterial('mission blocker refs', record.blockerRefs)
  assertNoUniversalPrivateMaterial('mission next-order refs', record.nextOrderRefs)
  assertNoUniversalPrivateMaterial('mission artifact refs', record.artifactRefs)
}

export const codingAutopilotMissionProjectionHasPrivateMaterial = (
  projection: CodingAutopilotMissionProjection,
): boolean =>
  universallyUnsafeRefPattern.test(JSON.stringify(projection)) ||
  /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/.test(JSON.stringify(projection))

export const projectCodingAutopilotMissionRecord = (
  record: CodingAutopilotMissionRecord,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): CodingAutopilotMissionProjection => {
  assertRecordSafe(record)

  const projection: CodingAutopilotMissionProjection = {
    accountLeaseRefs: audience === 'operator'
      ? safeRefsForAudience('mission account lease refs', record.accountLeaseRefs, audience)
      : [],
    artifactRefs: safeRefsForAudience('mission artifact refs', record.artifactRefs, audience),
    assignmentRefs: audience === 'public'
      ? []
      : safeRefsForAudience('mission assignment refs', record.assignmentRefs, audience),
    audience,
    blockerRefs: safeRefsForAudience('mission blocker refs', record.blockerRefs, audience),
    budgetRefs: audience === 'operator'
      ? safeRefsForAudience('mission budget refs', record.budgetRefs, audience)
      : [],
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    customerRefs: audience === 'customer' ||
      audience === 'team' ||
      audience === 'operator'
      ? safeRefsForAudience('mission customer refs', record.customerRefs, audience)
      : [],
    id: record.id,
    latestBriefingRef: safeNullableRefForAudience(
      'mission latest briefing ref',
      record.latestBriefingRef,
      audience,
    ),
    missionRef: record.missionRef,
    nextOrderRefs: safeRefsForAudience('mission next-order refs', record.nextOrderRefs, audience),
    objectiveStackRefs: safeRefsForAudience(
      'mission objective refs',
      record.objectiveStackRefs,
      audience,
    ),
    ownerRefs: audience === 'operator'
      ? safeRefsForAudience('mission owner refs', record.ownerRefs, audience)
      : [],
    routeScorecardRefs: audience === 'team' || audience === 'operator'
      ? safeRefsForAudience('mission route refs', record.routeScorecardRefs, audience)
      : [],
    status: record.status,
    statusLabel: statusLabelByStatus[record.status],
    teamRefs: audience === 'team' || audience === 'operator'
      ? safeRefsForAudience('mission team refs', record.teamRefs, audience)
      : [],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workKind: record.workKind,
    workroomRefs: audience === 'public'
      ? []
      : safeRefsForAudience('mission workroom refs', record.workroomRefs, audience),
  }

  if (codingAutopilotMissionProjectionHasPrivateMaterial(projection)) {
    throw new CodingAutopilotMissionUnsafe({
      reason: 'Mission projection contains private material or raw timestamps.',
    })
  }

  return projection
}

export const exampleCodingAutopilotMissionRecord =
  (): CodingAutopilotMissionRecord => ({
    accountLeaseRefs: ['account_lease.codex_3.run_otec_revision_4'],
    artifactRefs: [
      'artifact.diff_summary.otec_revision_4',
      'artifact.test_run.otec_revision_4',
      'artifact.pr_draft.otec_revision_4',
    ],
    assignmentRefs: ['assignment.site_generation.otec_revision_4'],
    blockerRefs: ['blocker.customer_feedback_needed.otec_revision_4'],
    budgetRefs: ['budget.internal_free_beta.otec_revision_4'],
    createdAtIso: '2026-06-06T20:15:00.000Z',
    customerRefs: ['customer_ref.order_otec'],
    id: 'mission_otec_revision_4',
    latestBriefingRef: 'briefing.continuation.otec_revision_4.latest',
    missionRef: 'mission.otec_revision_4',
    nextOrderRefs: ['next_order.review_saved_revision.otec_revision_4'],
    objectiveStackRefs: [
      'objective.build_customer_site_revision',
      'objective.keep_latest_revision_visible',
    ],
    ownerRefs: ['owner_ref.openagents_operator'],
    routeScorecardRefs: ['route_scorecard.codex_container_to_site_build'],
    status: 'waiting_for_review',
    teamRefs: ['team_ref.sites_fulfillment'],
    updatedAtIso: '2026-06-06T21:00:00.000Z',
    workKind: 'site',
    workroomRefs: ['workroom.otec_site_revision_4'],
  })
