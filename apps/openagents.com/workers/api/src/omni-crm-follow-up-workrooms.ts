import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import {
  OmniProjectionAudience,
} from './omni-data-classification'

export const OmniCrmFollowUpState = S.Literals([
  'approval_recorded',
  'approval_requested',
  'blocked',
  'closed',
  'draft_prepared',
  'email_receipt_recorded',
  'intake',
  'prep_packet_ready',
  'relationship_memory_recorded',
  'send_prepared',
])
export type OmniCrmFollowUpState = typeof OmniCrmFollowUpState.Type

export const OmniCrmFollowUpAuthorityBoundary = S.Literals([
  'contract_projection_only',
])
export type OmniCrmFollowUpAuthorityBoundary =
  typeof OmniCrmFollowUpAuthorityBoundary.Type

export class OmniCrmFollowUpAuthority extends S.Class<OmniCrmFollowUpAuthority>(
  'OmniCrmFollowUpAuthority',
)({
  authorityBoundary: OmniCrmFollowUpAuthorityBoundary,
  noAcceptedOutcomeSettlement: S.Boolean,
  noCrmMutation: S.Boolean,
  noEmailSend: S.Boolean,
  noExternalFollowUp: S.Boolean,
  noRelationshipMemoryMutation: S.Boolean,
}) {}

export class OmniCrmFollowUpTemplate extends S.Class<OmniCrmFollowUpTemplate>(
  'OmniCrmFollowUpTemplate',
)({
  approvalPolicyRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  closeoutRequirementRefs: S.Array(S.String),
  createdAtIso: S.String,
  evidenceRequirementRefs: S.Array(S.String),
  id: S.String,
  proofPolicyRefs: S.Array(S.String),
  requiredArtifactRefs: S.Array(S.String),
  templateRef: S.String,
  updatedAtIso: S.String,
  versionRef: S.String,
}) {}

export class OmniCrmFollowUpTemplateProjection extends S.Class<OmniCrmFollowUpTemplateProjection>(
  'OmniCrmFollowUpTemplateProjection',
)({
  approvalPolicyRefs: S.Array(S.String),
  audience: OmniProjectionAudience,
  caveatRefs: S.Array(S.String),
  closeoutRequirementRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  evidenceRequirementRefs: S.Array(S.String),
  id: S.String,
  proofPolicyRefs: S.Array(S.String),
  requiredArtifactRefs: S.Array(S.String),
  templateRef: S.String,
  updatedAtDisplay: S.String,
  versionRef: S.String,
}) {}

export class OmniCrmFollowUpWorkroomRecord extends S.Class<OmniCrmFollowUpWorkroomRecord>(
  'OmniCrmFollowUpWorkroomRecord',
)({
  approvalRefs: S.Array(S.String),
  authority: OmniCrmFollowUpAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  closeoutRefs: S.Array(S.String),
  companyRefs: S.Array(S.String),
  contactRefs: S.Array(S.String),
  createdAtIso: S.String,
  draftMessageRefs: S.Array(S.String),
  emailReceiptRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  id: S.String,
  operatorDiagnosticRefs: S.Array(S.String),
  prepPacketRefs: S.Array(S.String),
  relationshipMemoryRefs: S.Array(S.String),
  sendRequestRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: OmniCrmFollowUpState,
  templateRef: S.String,
  updatedAtIso: S.String,
  workroomRef: S.String,
}) {}

export class OmniCrmFollowUpWorkroomProjection extends S.Class<OmniCrmFollowUpWorkroomProjection>(
  'OmniCrmFollowUpWorkroomProjection',
)({
  approvalRecorded: S.Boolean,
  approvalRefs: S.Array(S.String),
  audience: OmniProjectionAudience,
  authority: OmniCrmFollowUpAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  closeoutReady: S.Boolean,
  closeoutRefs: S.Array(S.String),
  companyRefs: S.Array(S.String),
  contactRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  crmMutationAllowed: S.Boolean,
  draftMessageRefs: S.Array(S.String),
  draftPrepared: S.Boolean,
  emailReceiptRecorded: S.Boolean,
  emailReceiptRefs: S.Array(S.String),
  emailSendAllowed: S.Boolean,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  operatorDiagnosticRefs: S.Array(S.String),
  prepPacketReady: S.Boolean,
  prepPacketRefs: S.Array(S.String),
  relationshipMemoryMutationAllowed: S.Boolean,
  relationshipMemoryRecorded: S.Boolean,
  relationshipMemoryRefs: S.Array(S.String),
  sendPrepared: S.Boolean,
  sendRequestRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: OmniCrmFollowUpState,
  stateLabel: S.String,
  templateRef: S.String,
  updatedAtDisplay: S.String,
  workroomRef: S.String,
}) {}

export class OmniCrmFollowUpUnsafe extends S.TaggedErrorClass<OmniCrmFollowUpUnsafe>()(
  'OmniCrmFollowUpUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_CRM_FOLLOW_UP_CONTRACT_ONLY_AUTHORITY:
  OmniCrmFollowUpAuthority = {
    authorityBoundary: 'contract_projection_only',
    noAcceptedOutcomeSettlement: true,
    noCrmMutation: true,
    noEmailSend: true,
    noExternalFollowUp: true,
    noRelationshipMemoryMutation: true,
  }

const stateRank: Readonly<Record<OmniCrmFollowUpState, number>> = {
  approval_recorded: 4,
  approval_requested: 3,
  blocked: -1,
  closed: 8,
  draft_prepared: 2,
  email_receipt_recorded: 6,
  intake: 0,
  prep_packet_ready: 1,
  relationship_memory_recorded: 7,
  send_prepared: 5,
}

const stateLabelByState: Readonly<Record<OmniCrmFollowUpState, string>> = {
  approval_recorded: 'Approval recorded',
  approval_requested: 'Approval requested',
  blocked: 'Blocked',
  closed: 'Closed',
  draft_prepared: 'Draft prepared',
  email_receipt_recorded: 'Email receipt recorded',
  intake: 'Intake',
  prep_packet_ready: 'Prep packet ready',
  relationship_memory_recorded: 'Relationship memory recorded',
  send_prepared: 'Send prepared',
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeCrmFollowUpRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|contact[_-]?(address|email|name|phone)|cookie|customer[_-]?(email|name|phone|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(contact|key)|provider[_-]?(account|grant|payload|token)|raw[_-]?(contact|email|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(approval\.|company\.|contact\.|diagnostic\.operator|draft\.|email\.|email[_-]?receipt|relationship\.|relationship[_-]?memory|send\.|send[_-]?request|source\.)/i
const customerUnsafeRefPattern =
  /(diagnostic\.operator|email\.private|provider\.private|source\.private)/i
const teamUnsafeRefPattern =
  /(diagnostic\.operator|provider\.private|source\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

export const omniCrmFollowUpStateAtLeast = (
  state: OmniCrmFollowUpState,
  threshold: OmniCrmFollowUpState,
): boolean => stateRank[state] >= stateRank[threshold]

export const omniCrmFollowUpAuthorityIsContractOnly = (
  authority: OmniCrmFollowUpAuthority,
): boolean =>
  authority.authorityBoundary === 'contract_projection_only' &&
  authority.noAcceptedOutcomeSettlement &&
  authority.noCrmMutation &&
  authority.noEmailSend &&
  authority.noExternalFollowUp &&
  authority.noRelationshipMemoryMutation

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeCrmFollowUpRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new OmniCrmFollowUpUnsafe({
      reason: `${label} contains raw email, private contact, customer, provider, source payload, secret, wallet/payment, private repo, or raw timestamp material.`,
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
  template: OmniCrmFollowUpTemplate,
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
  record: OmniCrmFollowUpWorkroomRecord,
): ReadonlyArray<string> => [
  record.id,
  record.templateRef,
  record.workroomRef,
  ...record.approvalRefs,
  ...record.blockerRefs,
  ...record.caveatRefs,
  ...record.closeoutRefs,
  ...record.companyRefs,
  ...record.contactRefs,
  ...record.draftMessageRefs,
  ...record.emailReceiptRefs,
  ...record.evidenceRefs,
  ...record.operatorDiagnosticRefs,
  ...record.prepPacketRefs,
  ...record.relationshipMemoryRefs,
  ...record.sendRequestRefs,
  ...record.sourceRefs,
]

const assertTemplateSafe = (template: OmniCrmFollowUpTemplate): void => {
  assertSafeRefs('CRM follow-up template refs', templateRefs(template))
}

const assertRecordSafe = (record: OmniCrmFollowUpWorkroomRecord): void => {
  assertSafeRefs('CRM follow-up workroom refs', recordRefs(record))

  if (!omniCrmFollowUpAuthorityIsContractOnly(record.authority)) {
    throw new OmniCrmFollowUpUnsafe({
      reason: 'CRM follow-up records must remain contract/projection-only and cannot carry email-send, CRM mutation, relationship-memory mutation, external follow-up, or settlement authority.',
    })
  }

  if (record.state === 'blocked' && record.blockerRefs.length === 0) {
    throw new OmniCrmFollowUpUnsafe({
      reason: 'Blocked CRM follow-up workrooms require blocker refs.',
    })
  }

  if (
    omniCrmFollowUpStateAtLeast(record.state, 'prep_packet_ready') &&
    record.prepPacketRefs.length === 0
  ) {
    throw new OmniCrmFollowUpUnsafe({
      reason: 'CRM prep-packet state requires prep packet refs.',
    })
  }

  if (
    omniCrmFollowUpStateAtLeast(record.state, 'draft_prepared') &&
    record.draftMessageRefs.length === 0
  ) {
    throw new OmniCrmFollowUpUnsafe({
      reason: 'CRM draft-prepared state requires draft message refs.',
    })
  }

  if (
    omniCrmFollowUpStateAtLeast(record.state, 'approval_requested') &&
    record.approvalRefs.length === 0
  ) {
    throw new OmniCrmFollowUpUnsafe({
      reason: 'CRM approval state requires approval refs.',
    })
  }

  if (
    omniCrmFollowUpStateAtLeast(record.state, 'send_prepared') &&
    record.sendRequestRefs.length === 0
  ) {
    throw new OmniCrmFollowUpUnsafe({
      reason: 'CRM send-prepared state requires send request refs.',
    })
  }

  if (
    omniCrmFollowUpStateAtLeast(record.state, 'email_receipt_recorded') &&
    record.emailReceiptRefs.length === 0
  ) {
    throw new OmniCrmFollowUpUnsafe({
      reason: 'CRM email receipt state requires email receipt refs.',
    })
  }

  if (
    omniCrmFollowUpStateAtLeast(
      record.state,
      'relationship_memory_recorded',
    ) &&
    record.relationshipMemoryRefs.length === 0
  ) {
    throw new OmniCrmFollowUpUnsafe({
      reason: 'CRM relationship-memory state requires relationship memory refs.',
    })
  }

  if (record.state === 'closed' && record.closeoutRefs.length === 0) {
    throw new OmniCrmFollowUpUnsafe({
      reason: 'Closed CRM follow-up workrooms require closeout refs.',
    })
  }
}

export const projectOmniCrmFollowUpTemplate = (
  template: OmniCrmFollowUpTemplate,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OmniCrmFollowUpTemplateProjection => {
  assertTemplateSafe(template)

  return {
    approvalPolicyRefs: safeRefsForAudience(
      'CRM template approval policy refs',
      template.approvalPolicyRefs,
      audience,
    ),
    audience,
    caveatRefs: safeRefsForAudience(
      'CRM template caveat refs',
      template.caveatRefs,
      audience,
    ),
    closeoutRequirementRefs: safeRefsForAudience(
      'CRM template closeout requirement refs',
      template.closeoutRequirementRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      template.createdAtIso,
      nowIso,
    ),
    evidenceRequirementRefs: safeRefsForAudience(
      'CRM template evidence requirement refs',
      template.evidenceRequirementRefs,
      audience,
    ),
    id: safeRefForAudience('CRM template id', template.id, audience),
    proofPolicyRefs: safeRefsForAudience(
      'CRM template proof policy refs',
      template.proofPolicyRefs,
      audience,
    ),
    requiredArtifactRefs: safeRefsForAudience(
      'CRM template required artifact refs',
      template.requiredArtifactRefs,
      audience,
    ),
    templateRef: safeRefForAudience(
      'CRM template ref',
      template.templateRef,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      template.updatedAtIso,
      nowIso,
    ),
    versionRef: safeRefForAudience(
      'CRM template version ref',
      template.versionRef,
      audience,
    ),
  }
}

export const projectOmniCrmFollowUpWorkroom = (
  record: OmniCrmFollowUpWorkroomRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OmniCrmFollowUpWorkroomProjection => {
  assertRecordSafe(record)

  return {
    approvalRecorded:
      omniCrmFollowUpStateAtLeast(record.state, 'approval_recorded') &&
      record.approvalRefs.length > 0,
    approvalRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience('CRM approval refs', record.approvalRefs, audience),
    audience,
    authority: record.authority,
    blockerRefs: safeRefsForAudience(
      'CRM blocker refs',
      record.blockerRefs,
      audience,
    ),
    caveatRefs: safeRefsForAudience(
      'CRM caveat refs',
      record.caveatRefs,
      audience,
    ),
    closeoutReady: record.state === 'closed' &&
      record.closeoutRefs.length > 0 &&
      record.emailReceiptRefs.length > 0,
    closeoutRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience('CRM closeout refs', record.closeoutRefs, audience),
    companyRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience('CRM company refs', record.companyRefs, audience),
    contactRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience('CRM contact refs', record.contactRefs, audience),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    crmMutationAllowed: false,
    draftMessageRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'CRM draft message refs',
        record.draftMessageRefs,
        audience,
      ),
    draftPrepared:
      omniCrmFollowUpStateAtLeast(record.state, 'draft_prepared') &&
      record.draftMessageRefs.length > 0,
    emailReceiptRecorded:
      omniCrmFollowUpStateAtLeast(record.state, 'email_receipt_recorded') &&
      record.emailReceiptRefs.length > 0,
    emailReceiptRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'CRM email receipt refs',
        record.emailReceiptRefs,
        audience,
      )
      : [],
    emailSendAllowed: false,
    evidenceRefs: safeRefsForAudience(
      'CRM evidence refs',
      record.evidenceRefs,
      audience,
    ),
    id: safeRefForAudience('CRM workroom id', record.id, audience),
    operatorDiagnosticRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'CRM operator diagnostic refs',
        record.operatorDiagnosticRefs,
        audience,
      )
      : [],
    prepPacketReady:
      omniCrmFollowUpStateAtLeast(record.state, 'prep_packet_ready') &&
      record.prepPacketRefs.length > 0,
    prepPacketRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'CRM prep packet refs',
        record.prepPacketRefs,
        audience,
      ),
    relationshipMemoryMutationAllowed: false,
    relationshipMemoryRecorded:
      omniCrmFollowUpStateAtLeast(
        record.state,
        'relationship_memory_recorded',
      ) && record.relationshipMemoryRefs.length > 0,
    relationshipMemoryRefs:
      audience === 'operator' || audience === 'private'
        ? safeRefsForAudience(
          'CRM relationship memory refs',
          record.relationshipMemoryRefs,
          audience,
        )
        : [],
    sendPrepared:
      omniCrmFollowUpStateAtLeast(record.state, 'send_prepared') &&
      record.sendRequestRefs.length > 0,
    sendRequestRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'CRM send request refs',
        record.sendRequestRefs,
        audience,
      )
      : [],
    sourceRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience('CRM source refs', record.sourceRefs, audience),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    templateRef: safeRefForAudience(
      'CRM template ref',
      record.templateRef,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workroomRef: audience === 'public' || audience === 'agent'
      ? 'redacted'
      : safeRefForAudience('CRM workroom ref', record.workroomRef, audience),
  }
}

const projectionText = (
  projection:
    | OmniCrmFollowUpTemplateProjection
    | OmniCrmFollowUpWorkroomProjection,
): string =>
  'approvalRefs' in projection
    ? [
      projection.id,
      projection.templateRef,
      projection.workroomRef,
      ...projection.approvalRefs,
      ...projection.blockerRefs,
      ...projection.caveatRefs,
      ...projection.closeoutRefs,
      ...projection.companyRefs,
      ...projection.contactRefs,
      ...projection.draftMessageRefs,
      ...projection.emailReceiptRefs,
      ...projection.evidenceRefs,
      ...projection.operatorDiagnosticRefs,
      ...projection.prepPacketRefs,
      ...projection.relationshipMemoryRefs,
      ...projection.sendRequestRefs,
      ...projection.sourceRefs,
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

export const omniCrmFollowUpProjectionHasPrivateMaterial = (
  projection:
    | OmniCrmFollowUpTemplateProjection
    | OmniCrmFollowUpWorkroomProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeCrmFollowUpRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const OMNI_CRM_FOLLOW_UP_TEMPLATE_FIXTURE:
  OmniCrmFollowUpTemplate = {
    approvalPolicyRefs: ['approval_policy.crm.operator_review'],
    caveatRefs: ['caveat.crm.no_send_authority'],
    closeoutRequirementRefs: ['closeout.crm.receipt_and_memory'],
    createdAtIso: '2026-06-07T05:00:00.000Z',
    evidenceRequirementRefs: ['evidence_requirement.crm.receipt'],
    id: 'crm_follow_up_template.default',
    proofPolicyRefs: ['proof_policy.crm.private_receipt'],
    requiredArtifactRefs: ['artifact_requirement.crm.draft_and_receipt'],
    templateRef: 'template.crm.follow_up',
    updatedAtIso: '2026-06-07T05:05:00.000Z',
    versionRef: 'version.crm.follow_up.v1',
  }

export const OMNI_CRM_FOLLOW_UP_WORKROOM_FIXTURE:
  OmniCrmFollowUpWorkroomRecord = {
    approvalRefs: ['approval.crm.operator_approved'],
    authority: OMNI_CRM_FOLLOW_UP_CONTRACT_ONLY_AUTHORITY,
    blockerRefs: [],
    caveatRefs: ['caveat.crm.contract_only'],
    closeoutRefs: ['closeout.crm.follow_up_done'],
    companyRefs: ['company.crm.acme'],
    contactRefs: ['contact.crm.primary_public_ref'],
    createdAtIso: '2026-06-07T05:10:00.000Z',
    draftMessageRefs: ['draft.crm.follow_up_1'],
    emailReceiptRefs: ['email_receipt.crm.follow_up_1'],
    evidenceRefs: ['evidence.crm.follow_up_summary'],
    id: 'crm_follow_up_workroom.acme_1',
    operatorDiagnosticRefs: ['diagnostic.operator.crm_follow_up_route'],
    prepPacketRefs: ['prep_packet.crm.acme_follow_up'],
    relationshipMemoryRefs: ['relationship_memory.crm.acme_follow_up'],
    sendRequestRefs: ['send_request.crm.approved_follow_up'],
    sourceRefs: ['source.crm.meeting_notes_summary'],
    state: 'closed',
    templateRef: 'template.crm.follow_up',
    updatedAtIso: '2026-06-07T05:25:00.000Z',
    workroomRef: 'workroom.crm.acme_follow_up',
  }
