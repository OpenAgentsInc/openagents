import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import {
  OmniProjectionAudience,
} from './omni-data-classification'

export const OmniSupportProjectOpsKind = S.Literals([
  'project_ops',
  'support',
])
export type OmniSupportProjectOpsKind =
  typeof OmniSupportProjectOpsKind.Type

export const OmniSupportProjectOpsState = S.Literals([
  'blocked',
  'closed',
  'decision_recorded',
  'escalation_recorded',
  'intake',
  'issue_timeline_reconstructed',
  'project_task_updated',
  'proposed_response_ready',
  'receipt_recorded',
  'risk_recorded',
  'status_report_ready',
])
export type OmniSupportProjectOpsState =
  typeof OmniSupportProjectOpsState.Type

export const OmniSupportProjectOpsAuthorityBoundary = S.Literals([
  'contract_projection_only',
])
export type OmniSupportProjectOpsAuthorityBoundary =
  typeof OmniSupportProjectOpsAuthorityBoundary.Type

export class OmniSupportProjectOpsAuthority extends S.Class<OmniSupportProjectOpsAuthority>(
  'OmniSupportProjectOpsAuthority',
)({
  authorityBoundary: OmniSupportProjectOpsAuthorityBoundary,
  noAcceptedOutcomeMutation: S.Boolean,
  noCustomerRecordMutation: S.Boolean,
  noExternalEscalation: S.Boolean,
  noProjectManagementMutation: S.Boolean,
  noSupportResponseSend: S.Boolean,
}) {}

export class OmniSupportProjectOpsTemplate extends S.Class<OmniSupportProjectOpsTemplate>(
  'OmniSupportProjectOpsTemplate',
)({
  approvalPolicyRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  closeoutRequirementRefs: S.Array(S.String),
  createdAtIso: S.String,
  evidenceRequirementRefs: S.Array(S.String),
  id: S.String,
  kind: OmniSupportProjectOpsKind,
  proofPolicyRefs: S.Array(S.String),
  requiredArtifactRefs: S.Array(S.String),
  templateRef: S.String,
  updatedAtIso: S.String,
  versionRef: S.String,
}) {}

export class OmniSupportProjectOpsTemplateProjection extends S.Class<OmniSupportProjectOpsTemplateProjection>(
  'OmniSupportProjectOpsTemplateProjection',
)({
  approvalPolicyRefs: S.Array(S.String),
  audience: OmniProjectionAudience,
  caveatRefs: S.Array(S.String),
  closeoutRequirementRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  evidenceRequirementRefs: S.Array(S.String),
  id: S.String,
  kind: OmniSupportProjectOpsKind,
  proofPolicyRefs: S.Array(S.String),
  requiredArtifactRefs: S.Array(S.String),
  templateRef: S.String,
  updatedAtDisplay: S.String,
  versionRef: S.String,
}) {}

export class OmniSupportProjectOpsWorkroomRecord extends S.Class<OmniSupportProjectOpsWorkroomRecord>(
  'OmniSupportProjectOpsWorkroomRecord',
)({
  authority: OmniSupportProjectOpsAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  closeoutRefs: S.Array(S.String),
  createdAtIso: S.String,
  customerRefs: S.Array(S.String),
  decisionRefs: S.Array(S.String),
  escalationRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  id: S.String,
  issueTimelineRefs: S.Array(S.String),
  kind: OmniSupportProjectOpsKind,
  operatorDiagnosticRefs: S.Array(S.String),
  projectTaskRefs: S.Array(S.String),
  proposedResponseRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  riskRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: OmniSupportProjectOpsState,
  statusReportRefs: S.Array(S.String),
  templateRef: S.String,
  ticketRefs: S.Array(S.String),
  updatedAtIso: S.String,
  workroomRef: S.String,
}) {}

export class OmniSupportProjectOpsWorkroomProjection extends S.Class<OmniSupportProjectOpsWorkroomProjection>(
  'OmniSupportProjectOpsWorkroomProjection',
)({
  acceptedOutcomeMutationAllowed: S.Boolean,
  audience: OmniProjectionAudience,
  authority: OmniSupportProjectOpsAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  closeoutReady: S.Boolean,
  closeoutRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  customerRecordMutationAllowed: S.Boolean,
  customerRefs: S.Array(S.String),
  decisionRecorded: S.Boolean,
  decisionRefs: S.Array(S.String),
  escalationRecorded: S.Boolean,
  escalationRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  externalEscalationAllowed: S.Boolean,
  id: S.String,
  issueTimelineReady: S.Boolean,
  issueTimelineRefs: S.Array(S.String),
  kind: OmniSupportProjectOpsKind,
  operatorDiagnosticRefs: S.Array(S.String),
  projectManagementMutationAllowed: S.Boolean,
  projectTaskRefs: S.Array(S.String),
  projectTaskUpdated: S.Boolean,
  proposedResponseReady: S.Boolean,
  proposedResponseRefs: S.Array(S.String),
  receiptRecorded: S.Boolean,
  receiptRefs: S.Array(S.String),
  riskRecorded: S.Boolean,
  riskRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: OmniSupportProjectOpsState,
  stateLabel: S.String,
  statusReportReady: S.Boolean,
  statusReportRefs: S.Array(S.String),
  supportResponseSendAllowed: S.Boolean,
  templateRef: S.String,
  ticketRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  workroomRef: S.String,
}) {}

export class OmniSupportProjectOpsUnsafe extends S.TaggedErrorClass<OmniSupportProjectOpsUnsafe>()(
  'OmniSupportProjectOpsUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_SUPPORT_PROJECT_OPS_CONTRACT_ONLY_AUTHORITY:
  OmniSupportProjectOpsAuthority = {
    authorityBoundary: 'contract_projection_only',
    noAcceptedOutcomeMutation: true,
    noCustomerRecordMutation: true,
    noExternalEscalation: true,
    noProjectManagementMutation: true,
    noSupportResponseSend: true,
  }

const stateRank: Readonly<Record<OmniSupportProjectOpsState, number>> = {
  blocked: -1,
  closed: 9,
  decision_recorded: 5,
  escalation_recorded: 3,
  intake: 0,
  issue_timeline_reconstructed: 1,
  project_task_updated: 4,
  proposed_response_ready: 2,
  receipt_recorded: 8,
  risk_recorded: 6,
  status_report_ready: 7,
}

const stateLabelByState:
  Readonly<Record<OmniSupportProjectOpsState, string>> = {
    blocked: 'Blocked',
    closed: 'Closed',
    decision_recorded: 'Decision recorded',
    escalation_recorded: 'Escalation recorded',
    intake: 'Intake',
    issue_timeline_reconstructed: 'Issue timeline reconstructed',
    project_task_updated: 'Project task updated',
    proposed_response_ready: 'Proposed response ready',
    receipt_recorded: 'Receipt recorded',
    risk_recorded: 'Risk recorded',
    status_report_ready: 'Status report ready',
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeSupportProjectOpsRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|contact[_-]?(address|email|name|phone)|cookie|customer[_-]?(email|name|phone|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(customer|key|ticket)|provider[_-]?(account|grant|payload|token)|raw[_-]?(customer|email|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source|source[_-]?archive|support|ticket|transcript|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|support[_-]?transcript|ticket[_-]?private|token|transcript[_-]?(full|raw)|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(customer\.|decision\.|escalation\.|issue_timeline|project_task|proposed_response|receipt\.|risk\.|source\.|status_report|ticket\.|workroom\.)/i
const customerUnsafeRefPattern =
  /(diagnostic\.operator|provider\.private|source\.private|ticket\.private)/i
const teamUnsafeRefPattern =
  /(diagnostic\.operator|provider\.private|source\.private|ticket\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

export const omniSupportProjectOpsStateAtLeast = (
  state: OmniSupportProjectOpsState,
  threshold: OmniSupportProjectOpsState,
): boolean => stateRank[state] >= stateRank[threshold]

export const omniSupportProjectOpsAuthorityIsContractOnly = (
  authority: OmniSupportProjectOpsAuthority,
): boolean =>
  authority.authorityBoundary === 'contract_projection_only' &&
  authority.noAcceptedOutcomeMutation &&
  authority.noCustomerRecordMutation &&
  authority.noExternalEscalation &&
  authority.noProjectManagementMutation &&
  authority.noSupportResponseSend

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeSupportProjectOpsRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new OmniSupportProjectOpsUnsafe({
      reason: `${label} contains customer private data, raw support transcripts, private ticket refs, provider material, private repo refs, wallet/payment material, secrets, raw logs, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: typeof OmniProjectionAudience.Type,
): RegExp | null => {
  if (audience === 'public' || audience === 'agent') {
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
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const safeRefForAudience = (
  label: string,
  ref: string,
  audience: typeof OmniProjectionAudience.Type,
): string =>
  safeRefsForAudience(label, [ref], audience)[0] ??
  `${label.replaceAll(' ', '_')}.redacted`

const templateRefs = (
  template: OmniSupportProjectOpsTemplate,
): ReadonlyArray<string> => [
  template.id,
  template.templateRef,
  template.versionRef,
  ...template.approvalPolicyRefs,
  ...template.caveatRefs,
  ...template.closeoutRequirementRefs,
  ...template.evidenceRequirementRefs,
  ...template.proofPolicyRefs,
  ...template.requiredArtifactRefs,
]

const recordRefs = (
  record: OmniSupportProjectOpsWorkroomRecord,
): ReadonlyArray<string> => [
  record.id,
  record.templateRef,
  record.workroomRef,
  ...record.blockerRefs,
  ...record.caveatRefs,
  ...record.closeoutRefs,
  ...record.customerRefs,
  ...record.decisionRefs,
  ...record.escalationRefs,
  ...record.evidenceRefs,
  ...record.issueTimelineRefs,
  ...record.operatorDiagnosticRefs,
  ...record.projectTaskRefs,
  ...record.proposedResponseRefs,
  ...record.receiptRefs,
  ...record.riskRefs,
  ...record.sourceRefs,
  ...record.statusReportRefs,
  ...record.ticketRefs,
]

const assertTemplateSafe = (
  template: OmniSupportProjectOpsTemplate,
): void => {
  assertSafeRefs('Support/project ops template refs', templateRefs(template))
}

const assertRecordSafe = (
  record: OmniSupportProjectOpsWorkroomRecord,
): void => {
  assertSafeRefs('Support/project ops workroom refs', recordRefs(record))

  if (!omniSupportProjectOpsAuthorityIsContractOnly(record.authority)) {
    throw new OmniSupportProjectOpsUnsafe({
      reason: 'Support/project ops records must remain contract/projection-only and cannot carry support-send, project-management mutation, customer-record mutation, external escalation, or accepted-outcome mutation authority.',
    })
  }

  if (record.state === 'blocked' && record.blockerRefs.length === 0) {
    throw new OmniSupportProjectOpsUnsafe({
      reason: 'Blocked support/project ops workrooms require blocker refs.',
    })
  }

  if (
    omniSupportProjectOpsStateAtLeast(
      record.state,
      'issue_timeline_reconstructed',
    ) &&
    record.issueTimelineRefs.length === 0
  ) {
    throw new OmniSupportProjectOpsUnsafe({
      reason: 'Timeline reconstruction state requires issue timeline refs.',
    })
  }

  if (
    omniSupportProjectOpsStateAtLeast(
      record.state,
      'proposed_response_ready',
    ) &&
    record.proposedResponseRefs.length === 0
  ) {
    throw new OmniSupportProjectOpsUnsafe({
      reason: 'Proposed response state requires proposed response refs.',
    })
  }

  if (
    omniSupportProjectOpsStateAtLeast(record.state, 'escalation_recorded') &&
    record.escalationRefs.length === 0
  ) {
    throw new OmniSupportProjectOpsUnsafe({
      reason: 'Escalation state requires escalation refs.',
    })
  }

  if (
    omniSupportProjectOpsStateAtLeast(record.state, 'project_task_updated') &&
    record.projectTaskRefs.length === 0
  ) {
    throw new OmniSupportProjectOpsUnsafe({
      reason: 'Project task state requires project task refs.',
    })
  }

  if (
    omniSupportProjectOpsStateAtLeast(record.state, 'decision_recorded') &&
    record.decisionRefs.length === 0
  ) {
    throw new OmniSupportProjectOpsUnsafe({
      reason: 'Decision state requires decision refs.',
    })
  }

  if (
    omniSupportProjectOpsStateAtLeast(record.state, 'risk_recorded') &&
    record.riskRefs.length === 0
  ) {
    throw new OmniSupportProjectOpsUnsafe({
      reason: 'Risk state requires risk refs.',
    })
  }

  if (
    omniSupportProjectOpsStateAtLeast(record.state, 'status_report_ready') &&
    record.statusReportRefs.length === 0
  ) {
    throw new OmniSupportProjectOpsUnsafe({
      reason: 'Status report state requires status report refs.',
    })
  }

  if (
    omniSupportProjectOpsStateAtLeast(record.state, 'receipt_recorded') &&
    record.receiptRefs.length === 0
  ) {
    throw new OmniSupportProjectOpsUnsafe({
      reason: 'Receipt state requires receipt refs.',
    })
  }

  if (record.state === 'closed' && record.closeoutRefs.length === 0) {
    throw new OmniSupportProjectOpsUnsafe({
      reason: 'Closed support/project ops workrooms require closeout refs.',
    })
  }
}

export const projectOmniSupportProjectOpsTemplate = (
  template: OmniSupportProjectOpsTemplate,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OmniSupportProjectOpsTemplateProjection => {
  assertTemplateSafe(template)

  return {
    approvalPolicyRefs: safeRefsForAudience(
      'Support/project ops approval policy refs',
      template.approvalPolicyRefs,
      audience,
    ),
    audience,
    caveatRefs: safeRefsForAudience(
      'Support/project ops caveat refs',
      template.caveatRefs,
      audience,
    ),
    closeoutRequirementRefs: safeRefsForAudience(
      'Support/project ops closeout requirement refs',
      template.closeoutRequirementRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      template.createdAtIso,
      nowIso,
    ),
    evidenceRequirementRefs: safeRefsForAudience(
      'Support/project ops evidence requirement refs',
      template.evidenceRequirementRefs,
      audience,
    ),
    id: safeRefForAudience(
      'Support/project ops template id',
      template.id,
      audience,
    ),
    kind: template.kind,
    proofPolicyRefs: safeRefsForAudience(
      'Support/project ops proof policy refs',
      template.proofPolicyRefs,
      audience,
    ),
    requiredArtifactRefs: safeRefsForAudience(
      'Support/project ops required artifact refs',
      template.requiredArtifactRefs,
      audience,
    ),
    templateRef: safeRefForAudience(
      'Support/project ops template ref',
      template.templateRef,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      template.updatedAtIso,
      nowIso,
    ),
    versionRef: safeRefForAudience(
      'Support/project ops template version ref',
      template.versionRef,
      audience,
    ),
  }
}

export const projectOmniSupportProjectOpsWorkroom = (
  record: OmniSupportProjectOpsWorkroomRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OmniSupportProjectOpsWorkroomProjection => {
  assertRecordSafe(record)

  return {
    acceptedOutcomeMutationAllowed: false,
    audience,
    authority: record.authority,
    blockerRefs: safeRefsForAudience(
      'Support/project ops blocker refs',
      record.blockerRefs,
      audience,
    ),
    caveatRefs: safeRefsForAudience(
      'Support/project ops caveat refs',
      record.caveatRefs,
      audience,
    ),
    closeoutReady: record.state === 'closed' &&
      record.closeoutRefs.length > 0 &&
      record.receiptRefs.length > 0,
    closeoutRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Support/project ops closeout refs',
        record.closeoutRefs,
        audience,
      ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    customerRecordMutationAllowed: false,
    customerRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Support/project ops customer refs',
        record.customerRefs,
        audience,
      ),
    decisionRecorded:
      omniSupportProjectOpsStateAtLeast(record.state, 'decision_recorded') &&
      record.decisionRefs.length > 0,
    decisionRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Support/project ops decision refs',
        record.decisionRefs,
        audience,
      ),
    escalationRecorded:
      omniSupportProjectOpsStateAtLeast(record.state, 'escalation_recorded') &&
      record.escalationRefs.length > 0,
    escalationRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'Support/project ops escalation refs',
        record.escalationRefs,
        audience,
      )
      : [],
    evidenceRefs: safeRefsForAudience(
      'Support/project ops evidence refs',
      record.evidenceRefs,
      audience,
    ),
    externalEscalationAllowed: false,
    id: safeRefForAudience(
      'Support/project ops workroom id',
      record.id,
      audience,
    ),
    issueTimelineReady:
      omniSupportProjectOpsStateAtLeast(
        record.state,
        'issue_timeline_reconstructed',
      ) && record.issueTimelineRefs.length > 0,
    issueTimelineRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Support/project ops issue timeline refs',
        record.issueTimelineRefs,
        audience,
      ),
    kind: record.kind,
    operatorDiagnosticRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'Support/project ops operator diagnostic refs',
        record.operatorDiagnosticRefs,
        audience,
      )
      : [],
    projectManagementMutationAllowed: false,
    projectTaskRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Support/project ops project task refs',
        record.projectTaskRefs,
        audience,
      ),
    projectTaskUpdated:
      omniSupportProjectOpsStateAtLeast(
        record.state,
        'project_task_updated',
      ) && record.projectTaskRefs.length > 0,
    proposedResponseReady:
      omniSupportProjectOpsStateAtLeast(
        record.state,
        'proposed_response_ready',
      ) && record.proposedResponseRefs.length > 0,
    proposedResponseRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Support/project ops proposed response refs',
        record.proposedResponseRefs,
        audience,
      ),
    receiptRecorded:
      omniSupportProjectOpsStateAtLeast(record.state, 'receipt_recorded') &&
      record.receiptRefs.length > 0,
    receiptRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'Support/project ops receipt refs',
        record.receiptRefs,
        audience,
      )
      : [],
    riskRecorded:
      omniSupportProjectOpsStateAtLeast(record.state, 'risk_recorded') &&
      record.riskRefs.length > 0,
    riskRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Support/project ops risk refs',
        record.riskRefs,
        audience,
      ),
    sourceRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Support/project ops source refs',
        record.sourceRefs,
        audience,
      ),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    statusReportReady:
      omniSupportProjectOpsStateAtLeast(record.state, 'status_report_ready') &&
      record.statusReportRefs.length > 0,
    statusReportRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Support/project ops status report refs',
        record.statusReportRefs,
        audience,
      ),
    supportResponseSendAllowed: false,
    templateRef: safeRefForAudience(
      'Support/project ops template ref',
      record.templateRef,
      audience,
    ),
    ticketRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Support/project ops ticket refs',
        record.ticketRefs,
        audience,
      ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workroomRef: audience === 'public' || audience === 'agent'
      ? 'redacted'
      : safeRefForAudience(
        'Support/project ops workroom ref',
        record.workroomRef,
        audience,
      ),
  }
}

const projectionText = (
  projection:
    | OmniSupportProjectOpsTemplateProjection
    | OmniSupportProjectOpsWorkroomProjection,
): string =>
  'customerRefs' in projection
    ? [
      projection.id,
      projection.templateRef,
      projection.workroomRef,
      ...projection.blockerRefs,
      ...projection.caveatRefs,
      ...projection.closeoutRefs,
      ...projection.customerRefs,
      ...projection.decisionRefs,
      ...projection.escalationRefs,
      ...projection.evidenceRefs,
      ...projection.issueTimelineRefs,
      ...projection.operatorDiagnosticRefs,
      ...projection.projectTaskRefs,
      ...projection.proposedResponseRefs,
      ...projection.receiptRefs,
      ...projection.riskRefs,
      ...projection.sourceRefs,
      ...projection.statusReportRefs,
      ...projection.ticketRefs,
    ].join(' ')
    : [
      projection.id,
      projection.templateRef,
      projection.versionRef,
      ...projection.approvalPolicyRefs,
      ...projection.caveatRefs,
      ...projection.closeoutRequirementRefs,
      ...projection.evidenceRequirementRefs,
      ...projection.proofPolicyRefs,
      ...projection.requiredArtifactRefs,
    ].join(' ')

export const omniSupportProjectOpsProjectionHasPrivateMaterial = (
  projection:
    | OmniSupportProjectOpsTemplateProjection
    | OmniSupportProjectOpsWorkroomProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeSupportProjectOpsRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const OMNI_SUPPORT_OPS_TEMPLATE_FIXTURE:
  OmniSupportProjectOpsTemplate = {
    approvalPolicyRefs: ['approval_policy.support.operator_review'],
    caveatRefs: ['caveat.support.no_send_authority'],
    closeoutRequirementRefs: ['closeout.support.receipt_required'],
    createdAtIso: '2026-06-07T06:10:00.000Z',
    evidenceRequirementRefs: ['evidence_requirement.support.status_receipt'],
    id: 'support_ops_template.default',
    kind: 'support',
    proofPolicyRefs: ['proof_policy.support.customer_safe_summary'],
    requiredArtifactRefs: ['artifact_requirement.support.response_summary'],
    templateRef: 'template.support.default',
    updatedAtIso: '2026-06-07T06:15:00.000Z',
    versionRef: 'version.support.v1',
  }

export const OMNI_PROJECT_OPS_TEMPLATE_FIXTURE:
  OmniSupportProjectOpsTemplate = {
    approvalPolicyRefs: ['approval_policy.project_ops.operator_review'],
    caveatRefs: ['caveat.project_ops.no_external_mutation'],
    closeoutRequirementRefs: ['closeout.project_ops.receipt_required'],
    createdAtIso: '2026-06-07T06:10:00.000Z',
    evidenceRequirementRefs: ['evidence_requirement.project_ops.status_report'],
    id: 'project_ops_template.default',
    kind: 'project_ops',
    proofPolicyRefs: ['proof_policy.project_ops.team_safe_summary'],
    requiredArtifactRefs: ['artifact_requirement.project_ops.status_report'],
    templateRef: 'template.project_ops.default',
    updatedAtIso: '2026-06-07T06:15:00.000Z',
    versionRef: 'version.project_ops.v1',
  }

export const OMNI_SUPPORT_OPS_WORKROOM_FIXTURE:
  OmniSupportProjectOpsWorkroomRecord = {
    authority: OMNI_SUPPORT_PROJECT_OPS_CONTRACT_ONLY_AUTHORITY,
    blockerRefs: [],
    caveatRefs: ['caveat.support.contract_only'],
    closeoutRefs: ['closeout.support.issue_done'],
    createdAtIso: '2026-06-07T06:20:00.000Z',
    customerRefs: ['customer.support.public_ref'],
    decisionRefs: ['decision.support.escalation_not_needed'],
    escalationRefs: ['escalation.support.operator_reviewed'],
    evidenceRefs: ['evidence.support.summary'],
    id: 'support_ops_workroom.issue_1',
    issueTimelineRefs: ['issue_timeline.support.reconstructed'],
    kind: 'support',
    operatorDiagnosticRefs: ['diagnostic.operator.support_route'],
    projectTaskRefs: ['project_task.support.follow_up'],
    proposedResponseRefs: ['proposed_response.support.customer_safe'],
    receiptRefs: ['receipt.support.response_reviewed'],
    riskRefs: ['risk.support.low'],
    sourceRefs: ['source.support.ticket_summary'],
    state: 'closed',
    statusReportRefs: ['status_report.support.issue'],
    templateRef: 'template.support.default',
    ticketRefs: ['ticket.support.public_ref'],
    updatedAtIso: '2026-06-07T06:35:00.000Z',
    workroomRef: 'workroom.support.issue_1',
  }

export const OMNI_PROJECT_OPS_WORKROOM_FIXTURE:
  OmniSupportProjectOpsWorkroomRecord = {
    ...OMNI_SUPPORT_OPS_WORKROOM_FIXTURE,
    caveatRefs: ['caveat.project_ops.contract_only'],
    customerRefs: ['customer.project_ops.public_ref'],
    id: 'project_ops_workroom.project_1',
    kind: 'project_ops',
    projectTaskRefs: ['project_task.project_ops.roadmap_update'],
    sourceRefs: ['source.project_ops.status_summary'],
    statusReportRefs: ['status_report.project_ops.weekly'],
    templateRef: 'template.project_ops.default',
    ticketRefs: ['ticket.project_ops.public_ref'],
    workroomRef: 'workroom.project_ops.project_1',
  }
