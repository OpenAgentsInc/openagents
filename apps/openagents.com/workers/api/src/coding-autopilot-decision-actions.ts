import { Schema as S } from 'effect'

import type { BlueprintMissionBriefingAudience } from './blueprint/schemas/continuation-mission-briefing'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const CodingAutopilotDecisionActionKind = S.Literals([
  'approve_pr_draft',
  'continue',
  'create_followup_mission',
  'mark_unavailable',
  'provide_context',
  'request_customer_input',
  'rerun_tests',
  'retry_account',
  'steer',
  'stop',
])
export type CodingAutopilotDecisionActionKind =
  typeof CodingAutopilotDecisionActionKind.Type

export const CodingAutopilotDecisionActionStatus = S.Literals([
  'available',
  'blocked',
  'cancelled',
  'completed',
  'draft',
  'recommended',
])
export type CodingAutopilotDecisionActionStatus =
  typeof CodingAutopilotDecisionActionStatus.Type

export class CodingAutopilotDecisionActionRecord extends S.Class<CodingAutopilotDecisionActionRecord>(
  'CodingAutopilotDecisionActionRecord',
)({
  accountLeaseRefs: S.Array(S.String),
  actionKind: CodingAutopilotDecisionActionKind,
  actionRef: S.String,
  actionSubmissionRefs: S.Array(S.String),
  assignmentRefs: S.Array(S.String),
  blockedReasonRefs: S.Array(S.String),
  createdAtIso: S.String,
  customerNextActionRef: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  missionRef: S.String,
  prerequisiteRefs: S.Array(S.String),
  programRunRef: S.NullOr(S.String),
  receiptRefs: S.Array(S.String),
  routeRefs: S.Array(S.String),
  safeSummaryRef: S.String,
  sourceAuthorityRefs: S.Array(S.String),
  status: CodingAutopilotDecisionActionStatus,
  updatedAtIso: S.String,
  workroomRefs: S.Array(S.String),
}) {}

export class CodingAutopilotDecisionActionProjection extends S.Class<CodingAutopilotDecisionActionProjection>(
  'CodingAutopilotDecisionActionProjection',
)({
  accountLeaseRefs: S.Array(S.String),
  actionKind: CodingAutopilotDecisionActionKind,
  actionLabel: S.String,
  actionRef: S.String,
  actionSubmissionRefs: S.Array(S.String),
  actionSubmissionRequired: S.Boolean,
  assignmentRefs: S.Array(S.String),
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  blockedReasonRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  customerNextActionRef: S.String,
  directEffectPermitted: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  id: S.String,
  missionRef: S.String,
  prerequisiteRefs: S.Array(S.String),
  programRunRef: S.NullOr(S.String),
  receiptRefs: S.Array(S.String),
  routeRefs: S.Array(S.String),
  safeSummaryRef: S.String,
  sourceAuthorityRefs: S.Array(S.String),
  status: CodingAutopilotDecisionActionStatus,
  statusLabel: S.String,
  updatedAtDisplay: S.String,
  workroomRefs: S.Array(S.String),
}) {}

export class CodingAutopilotDecisionActionUnsafe extends S.TaggedErrorClass<CodingAutopilotDecisionActionUnsafe>()(
  'CodingAutopilotDecisionActionUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const universallyUnsafeRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|preimage)|preimage|private[_-]?key|private[_-]?repo|provider[_-]?(account|grant|payload|token)|raw[_-]?(email|invoice|payment|payload|patch|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|token|wallet|webhook[_-]?secret|workroom[_-]?private)/i
const publicUnsafeRefPattern =
  /(account[_-]?lease|assignment\.private|provider[_-]?account|route[_-]?scorecard|source[_-]?authority|workroom\.private)/i
const customerUnsafeRefPattern =
  /(account[_-]?lease|provider[_-]?account|route[_-]?scorecard|source[_-]?authority|workroom\.private)/i
const teamUnsafeRefPattern =
  /(account[_-]?lease|provider[_-]?account|source[_-]?authority)/i

const actionLabelByKind: Record<CodingAutopilotDecisionActionKind, string> = {
  approve_pr_draft: 'Approve PR draft',
  continue: 'Continue',
  create_followup_mission: 'Create follow-up mission',
  mark_unavailable: 'Mark unavailable',
  provide_context: 'Provide context',
  request_customer_input: 'Request customer input',
  rerun_tests: 'Rerun tests',
  retry_account: 'Retry account',
  steer: 'Steer',
  stop: 'Stop',
}

const statusLabelByStatus: Record<CodingAutopilotDecisionActionStatus, string> =
  {
    available: 'Available',
    blocked: 'Blocked',
    cancelled: 'Cancelled',
    completed: 'Completed',
    draft: 'Draft',
    recommended: 'Recommended',
  }

const actionSubmissionRequiredKinds =
  new Set<CodingAutopilotDecisionActionKind>([
    'approve_pr_draft',
    'continue',
    'create_followup_mission',
    'mark_unavailable',
    'provide_context',
    'request_customer_input',
    'rerun_tests',
    'retry_account',
    'steer',
    'stop',
  ])

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
    throw new CodingAutopilotDecisionActionUnsafe({
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

const assertRecordSafe = (
  record: CodingAutopilotDecisionActionRecord,
): void => {
  assertNoUniversalPrivateMaterial('decision action identity refs', [
    record.id,
    record.actionRef,
    record.missionRef,
    record.programRunRef ?? '',
    record.customerNextActionRef,
    record.safeSummaryRef,
  ])
  assertNoUniversalPrivateMaterial('decision action workroom refs', record.workroomRefs)
  assertNoUniversalPrivateMaterial('decision action assignment refs', record.assignmentRefs)
  assertNoUniversalPrivateMaterial('decision action route refs', record.routeRefs)
  assertNoUniversalPrivateMaterial('decision action account refs', record.accountLeaseRefs)
  assertNoUniversalPrivateMaterial('decision action evidence refs', record.evidenceRefs)
  assertNoUniversalPrivateMaterial('decision action source authority refs', record.sourceAuthorityRefs)
  assertNoUniversalPrivateMaterial('decision action submission refs', record.actionSubmissionRefs)
  assertNoUniversalPrivateMaterial('decision action prerequisite refs', record.prerequisiteRefs)
  assertNoUniversalPrivateMaterial('decision action blocked reason refs', record.blockedReasonRefs)
  assertNoUniversalPrivateMaterial('decision action receipt refs', record.receiptRefs)
}

export const codingAutopilotDecisionActionProjectionHasPrivateMaterial = (
  projection: CodingAutopilotDecisionActionProjection,
): boolean =>
  universallyUnsafeRefPattern.test(JSON.stringify(projection)) ||
  /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/.test(JSON.stringify(projection))

export const projectCodingAutopilotDecisionActionRecord = (
  record: CodingAutopilotDecisionActionRecord,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): CodingAutopilotDecisionActionProjection => {
  assertRecordSafe(record)

  const projection: CodingAutopilotDecisionActionProjection = {
    accountLeaseRefs: audience === 'operator'
      ? safeRefsForAudience('decision action account refs', record.accountLeaseRefs, audience)
      : [],
    actionKind: record.actionKind,
    actionLabel: actionLabelByKind[record.actionKind],
    actionRef: record.actionRef,
    actionSubmissionRefs: audience === 'operator'
      ? safeRefsForAudience('decision action submission refs', record.actionSubmissionRefs, audience)
      : [],
    actionSubmissionRequired: actionSubmissionRequiredKinds.has(record.actionKind),
    assignmentRefs: audience === 'public'
      ? []
      : safeRefsForAudience('decision action assignment refs', record.assignmentRefs, audience),
    audience,
    blockedReasonRefs: safeRefsForAudience(
      'decision action blocked reason refs',
      record.blockedReasonRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    customerNextActionRef: safeRefsForAudience(
      'decision action customer next action ref',
      [record.customerNextActionRef],
      audience,
    )[0] ?? 'next_action.redacted',
    directEffectPermitted: false,
    evidenceRefs: safeRefsForAudience('decision action evidence refs', record.evidenceRefs, audience),
    id: record.id,
    missionRef: record.missionRef,
    prerequisiteRefs: safeRefsForAudience(
      'decision action prerequisite refs',
      record.prerequisiteRefs,
      audience,
    ),
    programRunRef: safeNullableRefForAudience(
      'decision action Program Run ref',
      record.programRunRef,
      audience,
    ),
    receiptRefs: safeRefsForAudience('decision action receipt refs', record.receiptRefs, audience),
    routeRefs: audience === 'team' || audience === 'operator'
      ? safeRefsForAudience('decision action route refs', record.routeRefs, audience)
      : [],
    safeSummaryRef: safeRefsForAudience(
      'decision action summary ref',
      [record.safeSummaryRef],
      audience,
    )[0] ?? 'summary.redacted',
    sourceAuthorityRefs: audience === 'operator'
      ? safeRefsForAudience('decision action source authority refs', record.sourceAuthorityRefs, audience)
      : [],
    status: record.status,
    statusLabel: statusLabelByStatus[record.status],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workroomRefs: audience === 'public'
      ? []
      : safeRefsForAudience('decision action workroom refs', record.workroomRefs, audience),
  }

  if (codingAutopilotDecisionActionProjectionHasPrivateMaterial(projection)) {
    throw new CodingAutopilotDecisionActionUnsafe({
      reason:
        'Decision action projection contains private material or raw timestamps.',
    })
  }

  return projection
}

export const exampleCodingAutopilotDecisionActions =
  (): ReadonlyArray<CodingAutopilotDecisionActionRecord> => [
    {
      accountLeaseRefs: [],
      actionKind: 'continue',
      actionRef: 'decision_action.otec_revision_4.continue',
      actionSubmissionRefs: ['action_submission.otec_revision_4.continue'],
      assignmentRefs: ['assignment.site_generation.otec_revision_4'],
      blockedReasonRefs: [],
      createdAtIso: '2026-06-06T20:30:00.000Z',
      customerNextActionRef: 'next_action.autopilot_continue',
      evidenceRefs: ['evidence.briefing.otec_revision_4.ready'],
      id: 'decision_action_otec_revision_4_continue',
      missionRef: 'mission.otec_revision_4',
      prerequisiteRefs: ['prerequisite.operator_confirms_scope'],
      programRunRef: 'program_run.continuation.otec_revision_4',
      receiptRefs: [],
      routeRefs: ['route_scorecard.codex_container_to_site_build'],
      safeSummaryRef: 'summary.autopilot_can_continue',
      sourceAuthorityRefs: ['source_authority.operator_confirmed_scope'],
      status: 'recommended',
      updatedAtIso: '2026-06-06T21:00:00.000Z',
      workroomRefs: ['workroom.otec_site_revision_4'],
    },
    {
      accountLeaseRefs: ['account_lease.codex_3.run_otec_revision_4'],
      actionKind: 'retry_account',
      actionRef: 'decision_action.otec_revision_4.retry_account',
      actionSubmissionRefs: ['action_submission.otec_revision_4.retry_account'],
      assignmentRefs: ['assignment.site_generation.otec_revision_4'],
      blockedReasonRefs: ['blocked.provider_rate_limit'],
      createdAtIso: '2026-06-06T20:45:00.000Z',
      customerNextActionRef: 'next_action.autopilot_retrying',
      evidenceRefs: ['evidence.account_failover_needed'],
      id: 'decision_action_otec_revision_4_retry_account',
      missionRef: 'mission.otec_revision_4',
      prerequisiteRefs: ['prerequisite.alternate_account_available'],
      programRunRef: 'program_run.continuation.otec_revision_4',
      receiptRefs: ['receipt.account_failover.otec_revision_4'],
      routeRefs: ['route_scorecard.codex_account_fleet'],
      safeSummaryRef: 'summary.autopilot_retry_account',
      sourceAuthorityRefs: ['source_authority.account_fleet_health'],
      status: 'available',
      updatedAtIso: '2026-06-06T21:00:00.000Z',
      workroomRefs: ['workroom.otec_site_revision_4'],
    },
  ]
