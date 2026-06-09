import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import {
  OmniProjectionAudience,
} from './omni-data-classification'

export const OmniInvestorOpsState = S.Literals([
  'accepted_outcome_recorded',
  'blocked',
  'closed',
  'creative_work_order_ready',
  'data_room_task_ready',
  'decision_receipt_recorded',
  'follow_up_queued',
  'intake',
  'prep_packet_ready',
])
export type OmniInvestorOpsState = typeof OmniInvestorOpsState.Type

export const OmniInvestorOpsAuthorityBoundary = S.Literals([
  'contract_projection_only',
])
export type OmniInvestorOpsAuthorityBoundary =
  typeof OmniInvestorOpsAuthorityBoundary.Type

export class OmniInvestorOpsAuthority extends S.Class<OmniInvestorOpsAuthority>(
  'OmniInvestorOpsAuthority',
)({
  authorityBoundary: OmniInvestorOpsAuthorityBoundary,
  noAcceptedOutcomeMutation: S.Boolean,
  noDataRoomUpload: S.Boolean,
  noDeckOrVideoPublish: S.Boolean,
  noInvestorRecordMutation: S.Boolean,
  noOutreachSend: S.Boolean,
}) {}

export class OmniInvestorOpsTemplate extends S.Class<OmniInvestorOpsTemplate>(
  'OmniInvestorOpsTemplate',
)({
  approvalPolicyRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  closeoutRequirementRefs: S.Array(S.String),
  createdAtIso: S.String,
  dataRoomPolicyRefs: S.Array(S.String),
  evidenceRequirementRefs: S.Array(S.String),
  id: S.String,
  proofPolicyRefs: S.Array(S.String),
  requiredArtifactRefs: S.Array(S.String),
  templateRef: S.String,
  updatedAtIso: S.String,
  versionRef: S.String,
}) {}

export class OmniInvestorOpsTemplateProjection extends S.Class<OmniInvestorOpsTemplateProjection>(
  'OmniInvestorOpsTemplateProjection',
)({
  approvalPolicyRefs: S.Array(S.String),
  audience: OmniProjectionAudience,
  caveatRefs: S.Array(S.String),
  closeoutRequirementRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  dataRoomPolicyRefs: S.Array(S.String),
  evidenceRequirementRefs: S.Array(S.String),
  id: S.String,
  proofPolicyRefs: S.Array(S.String),
  requiredArtifactRefs: S.Array(S.String),
  templateRef: S.String,
  updatedAtDisplay: S.String,
  versionRef: S.String,
}) {}

export class OmniInvestorOpsWorkroomRecord extends S.Class<OmniInvestorOpsWorkroomRecord>(
  'OmniInvestorOpsWorkroomRecord',
)({
  acceptanceRefs: S.Array(S.String),
  authority: OmniInvestorOpsAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  closeoutRefs: S.Array(S.String),
  contactRefs: S.Array(S.String),
  createdAtIso: S.String,
  dataRoomTaskRefs: S.Array(S.String),
  decisionReceiptRefs: S.Array(S.String),
  deckWorkOrderRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  followUpRefs: S.Array(S.String),
  id: S.String,
  investorRefs: S.Array(S.String),
  operatorDiagnosticRefs: S.Array(S.String),
  prepPacketRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: OmniInvestorOpsState,
  templateRef: S.String,
  updatedAtIso: S.String,
  videoWorkOrderRefs: S.Array(S.String),
  workroomRef: S.String,
}) {}

export class OmniInvestorOpsWorkroomProjection extends S.Class<OmniInvestorOpsWorkroomProjection>(
  'OmniInvestorOpsWorkroomProjection',
)({
  acceptedOutcomeMutationAllowed: S.Boolean,
  acceptedOutcomeRecorded: S.Boolean,
  acceptanceRefs: S.Array(S.String),
  audience: OmniProjectionAudience,
  authority: OmniInvestorOpsAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  closeoutReady: S.Boolean,
  closeoutRefs: S.Array(S.String),
  contactRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  creativePublishAllowed: S.Boolean,
  creativeWorkOrderReady: S.Boolean,
  dataRoomMutationAllowed: S.Boolean,
  dataRoomTaskReady: S.Boolean,
  dataRoomTaskRefs: S.Array(S.String),
  decisionReceiptRecorded: S.Boolean,
  decisionReceiptRefs: S.Array(S.String),
  deckWorkOrderRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  followUpQueued: S.Boolean,
  followUpRefs: S.Array(S.String),
  id: S.String,
  investorMutationAllowed: S.Boolean,
  investorRefs: S.Array(S.String),
  operatorDiagnosticRefs: S.Array(S.String),
  outreachSendAllowed: S.Boolean,
  prepPacketReady: S.Boolean,
  prepPacketRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: OmniInvestorOpsState,
  stateLabel: S.String,
  templateRef: S.String,
  updatedAtDisplay: S.String,
  videoWorkOrderRefs: S.Array(S.String),
  workroomRef: S.String,
}) {}

export class OmniInvestorOpsUnsafe extends S.TaggedErrorClass<OmniInvestorOpsUnsafe>()(
  'OmniInvestorOpsUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_INVESTOR_OPS_CONTRACT_ONLY_AUTHORITY:
  OmniInvestorOpsAuthority = {
    authorityBoundary: 'contract_projection_only',
    noAcceptedOutcomeMutation: true,
    noDataRoomUpload: true,
    noDeckOrVideoPublish: true,
    noInvestorRecordMutation: true,
    noOutreachSend: true,
  }

const stateRank: Readonly<Record<OmniInvestorOpsState, number>> = {
  accepted_outcome_recorded: 6,
  blocked: -1,
  closed: 7,
  creative_work_order_ready: 3,
  data_room_task_ready: 2,
  decision_receipt_recorded: 5,
  follow_up_queued: 4,
  intake: 0,
  prep_packet_ready: 1,
}

const stateLabelByState: Readonly<Record<OmniInvestorOpsState, string>> = {
  accepted_outcome_recorded: 'Accepted outcome recorded',
  blocked: 'Blocked',
  closed: 'Closed',
  creative_work_order_ready: 'Creative work order ready',
  data_room_task_ready: 'Data-room task ready',
  decision_receipt_recorded: 'Decision receipt recorded',
  follow_up_queued: 'Follow-up queued',
  intake: 'Intake',
  prep_packet_ready: 'Prep packet ready',
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeInvestorOpsRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|contact[_-]?(address|email|name|phone)|cookie|customer[_-]?(email|name|phone|value)|data[_-]?room\.(raw|private)|deck[_-]?(asset|raw|source)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|investor[_-]?(email|name|phone|private)|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(contact|data[_-]?room|investor|key)|provider[_-]?(account|grant|payload|token)|raw[_-]?(contact|data[_-]?room|deck|email|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source|source[_-]?archive|video|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|video[_-]?(asset|raw|source)|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(acceptance\.|contact\.|data_room|data-room|decision\.|deck\.|diagnostic\.operator|follow_up|follow-up|investor\.|prep_packet|source\.|video\.|workroom\.)/i
const customerUnsafeRefPattern =
  /(data_room\.private|deck\.private|diagnostic\.operator|investor\.private|provider\.private|source\.private|video\.private)/i
const teamUnsafeRefPattern =
  /(data_room\.private|deck\.private|diagnostic\.operator|provider\.private|source\.private|video\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

export const omniInvestorOpsStateAtLeast = (
  state: OmniInvestorOpsState,
  threshold: OmniInvestorOpsState,
): boolean => stateRank[state] >= stateRank[threshold]

export const omniInvestorOpsAuthorityIsContractOnly = (
  authority: OmniInvestorOpsAuthority,
): boolean =>
  authority.authorityBoundary === 'contract_projection_only' &&
  authority.noAcceptedOutcomeMutation &&
  authority.noDataRoomUpload &&
  authority.noDeckOrVideoPublish &&
  authority.noInvestorRecordMutation &&
  authority.noOutreachSend

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeInvestorOpsRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new OmniInvestorOpsUnsafe({
      reason: `${label} contains private investor/contact data, raw data-room material, raw deck/video assets, provider material, private repo refs, wallet/payment material, secrets, raw logs, or raw timestamps.`,
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
  template: OmniInvestorOpsTemplate,
): ReadonlyArray<string> => [
  template.id,
  template.templateRef,
  template.versionRef,
  ...template.approvalPolicyRefs,
  ...template.caveatRefs,
  ...template.closeoutRequirementRefs,
  ...template.dataRoomPolicyRefs,
  ...template.evidenceRequirementRefs,
  ...template.proofPolicyRefs,
  ...template.requiredArtifactRefs,
]

const recordRefs = (
  record: OmniInvestorOpsWorkroomRecord,
): ReadonlyArray<string> => [
  record.id,
  record.templateRef,
  record.workroomRef,
  ...record.acceptanceRefs,
  ...record.blockerRefs,
  ...record.caveatRefs,
  ...record.closeoutRefs,
  ...record.contactRefs,
  ...record.dataRoomTaskRefs,
  ...record.decisionReceiptRefs,
  ...record.deckWorkOrderRefs,
  ...record.evidenceRefs,
  ...record.followUpRefs,
  ...record.investorRefs,
  ...record.operatorDiagnosticRefs,
  ...record.prepPacketRefs,
  ...record.sourceRefs,
  ...record.videoWorkOrderRefs,
]

const assertTemplateSafe = (template: OmniInvestorOpsTemplate): void => {
  assertSafeRefs('Investor ops template refs', templateRefs(template))
}

const assertRecordSafe = (record: OmniInvestorOpsWorkroomRecord): void => {
  assertSafeRefs('Investor ops workroom refs', recordRefs(record))

  if (!omniInvestorOpsAuthorityIsContractOnly(record.authority)) {
    throw new OmniInvestorOpsUnsafe({
      reason: 'Investor ops records must remain contract/projection-only and cannot carry outreach, deck/video publish, data-room upload, investor-record mutation, or accepted-outcome mutation authority.',
    })
  }

  if (record.state === 'blocked' && record.blockerRefs.length === 0) {
    throw new OmniInvestorOpsUnsafe({
      reason: 'Blocked investor ops workrooms require blocker refs.',
    })
  }

  if (
    omniInvestorOpsStateAtLeast(record.state, 'prep_packet_ready') &&
    record.prepPacketRefs.length === 0
  ) {
    throw new OmniInvestorOpsUnsafe({
      reason: 'Investor prep state requires prep packet refs.',
    })
  }

  if (
    omniInvestorOpsStateAtLeast(record.state, 'data_room_task_ready') &&
    record.dataRoomTaskRefs.length === 0
  ) {
    throw new OmniInvestorOpsUnsafe({
      reason: 'Data-room task state requires data-room task refs.',
    })
  }

  if (
    omniInvestorOpsStateAtLeast(
      record.state,
      'creative_work_order_ready',
    ) &&
    record.deckWorkOrderRefs.length === 0 &&
    record.videoWorkOrderRefs.length === 0
  ) {
    throw new OmniInvestorOpsUnsafe({
      reason: 'Creative work-order state requires deck or video work-order refs.',
    })
  }

  if (
    omniInvestorOpsStateAtLeast(record.state, 'follow_up_queued') &&
    record.followUpRefs.length === 0
  ) {
    throw new OmniInvestorOpsUnsafe({
      reason: 'Follow-up queued state requires follow-up refs.',
    })
  }

  if (
    omniInvestorOpsStateAtLeast(
      record.state,
      'decision_receipt_recorded',
    ) &&
    record.decisionReceiptRefs.length === 0
  ) {
    throw new OmniInvestorOpsUnsafe({
      reason: 'Decision receipt state requires decision receipt refs.',
    })
  }

  if (
    omniInvestorOpsStateAtLeast(
      record.state,
      'accepted_outcome_recorded',
    ) &&
    record.acceptanceRefs.length === 0
  ) {
    throw new OmniInvestorOpsUnsafe({
      reason: 'Accepted outcome state requires acceptance refs.',
    })
  }

  if (record.state === 'closed' && record.closeoutRefs.length === 0) {
    throw new OmniInvestorOpsUnsafe({
      reason: 'Closed investor ops workrooms require closeout refs.',
    })
  }
}

export const projectOmniInvestorOpsTemplate = (
  template: OmniInvestorOpsTemplate,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OmniInvestorOpsTemplateProjection => {
  assertTemplateSafe(template)

  return {
    approvalPolicyRefs: safeRefsForAudience(
      'Investor ops approval policy refs',
      template.approvalPolicyRefs,
      audience,
    ),
    audience,
    caveatRefs: safeRefsForAudience(
      'Investor ops caveat refs',
      template.caveatRefs,
      audience,
    ),
    closeoutRequirementRefs: safeRefsForAudience(
      'Investor ops closeout requirement refs',
      template.closeoutRequirementRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      template.createdAtIso,
      nowIso,
    ),
    dataRoomPolicyRefs: safeRefsForAudience(
      'Investor ops data-room policy refs',
      template.dataRoomPolicyRefs,
      audience,
    ),
    evidenceRequirementRefs: safeRefsForAudience(
      'Investor ops evidence requirement refs',
      template.evidenceRequirementRefs,
      audience,
    ),
    id: safeRefForAudience('Investor ops template id', template.id, audience),
    proofPolicyRefs: safeRefsForAudience(
      'Investor ops proof policy refs',
      template.proofPolicyRefs,
      audience,
    ),
    requiredArtifactRefs: safeRefsForAudience(
      'Investor ops required artifact refs',
      template.requiredArtifactRefs,
      audience,
    ),
    templateRef: safeRefForAudience(
      'Investor ops template ref',
      template.templateRef,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      template.updatedAtIso,
      nowIso,
    ),
    versionRef: safeRefForAudience(
      'Investor ops template version ref',
      template.versionRef,
      audience,
    ),
  }
}

export const projectOmniInvestorOpsWorkroom = (
  record: OmniInvestorOpsWorkroomRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OmniInvestorOpsWorkroomProjection => {
  assertRecordSafe(record)

  const creativeWorkOrderReady =
    omniInvestorOpsStateAtLeast(record.state, 'creative_work_order_ready') &&
    (record.deckWorkOrderRefs.length > 0 || record.videoWorkOrderRefs.length > 0)

  return {
    acceptedOutcomeMutationAllowed: false,
    acceptedOutcomeRecorded:
      omniInvestorOpsStateAtLeast(
        record.state,
        'accepted_outcome_recorded',
      ) && record.acceptanceRefs.length > 0,
    acceptanceRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Investor ops acceptance refs',
        record.acceptanceRefs,
        audience,
      ),
    audience,
    authority: record.authority,
    blockerRefs: safeRefsForAudience(
      'Investor ops blocker refs',
      record.blockerRefs,
      audience,
    ),
    caveatRefs: safeRefsForAudience(
      'Investor ops caveat refs',
      record.caveatRefs,
      audience,
    ),
    closeoutReady: record.state === 'closed' &&
      record.closeoutRefs.length > 0 &&
      record.decisionReceiptRefs.length > 0 &&
      record.acceptanceRefs.length > 0,
    closeoutRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Investor ops closeout refs',
        record.closeoutRefs,
        audience,
      ),
    contactRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Investor ops contact refs',
        record.contactRefs,
        audience,
      ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    creativePublishAllowed: false,
    creativeWorkOrderReady,
    dataRoomMutationAllowed: false,
    dataRoomTaskReady:
      omniInvestorOpsStateAtLeast(record.state, 'data_room_task_ready') &&
      record.dataRoomTaskRefs.length > 0,
    dataRoomTaskRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Investor ops data-room task refs',
        record.dataRoomTaskRefs,
        audience,
      ),
    decisionReceiptRecorded:
      omniInvestorOpsStateAtLeast(
        record.state,
        'decision_receipt_recorded',
      ) && record.decisionReceiptRefs.length > 0,
    decisionReceiptRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Investor ops decision receipt refs',
        record.decisionReceiptRefs,
        audience,
      ),
    deckWorkOrderRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Investor ops deck work-order refs',
        record.deckWorkOrderRefs,
        audience,
      ),
    evidenceRefs: safeRefsForAudience(
      'Investor ops evidence refs',
      record.evidenceRefs,
      audience,
    ),
    followUpQueued:
      omniInvestorOpsStateAtLeast(record.state, 'follow_up_queued') &&
      record.followUpRefs.length > 0,
    followUpRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Investor ops follow-up refs',
        record.followUpRefs,
        audience,
      ),
    id: safeRefForAudience('Investor ops workroom id', record.id, audience),
    investorMutationAllowed: false,
    investorRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Investor ops investor refs',
        record.investorRefs,
        audience,
      ),
    operatorDiagnosticRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'Investor ops operator diagnostic refs',
        record.operatorDiagnosticRefs,
        audience,
      )
      : [],
    outreachSendAllowed: false,
    prepPacketReady:
      omniInvestorOpsStateAtLeast(record.state, 'prep_packet_ready') &&
      record.prepPacketRefs.length > 0,
    prepPacketRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Investor ops prep packet refs',
        record.prepPacketRefs,
        audience,
      ),
    sourceRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Investor ops source refs',
        record.sourceRefs,
        audience,
      ),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    templateRef: safeRefForAudience(
      'Investor ops template ref',
      record.templateRef,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    videoWorkOrderRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Investor ops video work-order refs',
        record.videoWorkOrderRefs,
        audience,
      ),
    workroomRef: audience === 'public' || audience === 'agent'
      ? 'redacted'
      : safeRefForAudience(
        'Investor ops workroom ref',
        record.workroomRef,
        audience,
      ),
  }
}

const projectionText = (
  projection:
    | OmniInvestorOpsTemplateProjection
    | OmniInvestorOpsWorkroomProjection,
): string =>
  'investorRefs' in projection
    ? [
      projection.id,
      projection.templateRef,
      projection.workroomRef,
      ...projection.acceptanceRefs,
      ...projection.blockerRefs,
      ...projection.caveatRefs,
      ...projection.closeoutRefs,
      ...projection.contactRefs,
      ...projection.dataRoomTaskRefs,
      ...projection.decisionReceiptRefs,
      ...projection.deckWorkOrderRefs,
      ...projection.evidenceRefs,
      ...projection.followUpRefs,
      ...projection.investorRefs,
      ...projection.operatorDiagnosticRefs,
      ...projection.prepPacketRefs,
      ...projection.sourceRefs,
      ...projection.videoWorkOrderRefs,
    ].join(' ')
    : [
      projection.id,
      projection.templateRef,
      projection.versionRef,
      ...projection.approvalPolicyRefs,
      ...projection.caveatRefs,
      ...projection.closeoutRequirementRefs,
      ...projection.dataRoomPolicyRefs,
      ...projection.evidenceRequirementRefs,
      ...projection.proofPolicyRefs,
      ...projection.requiredArtifactRefs,
    ].join(' ')

export const omniInvestorOpsProjectionHasPrivateMaterial = (
  projection:
    | OmniInvestorOpsTemplateProjection
    | OmniInvestorOpsWorkroomProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeInvestorOpsRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const OMNI_INVESTOR_OPS_TEMPLATE_FIXTURE:
  OmniInvestorOpsTemplate = {
    approvalPolicyRefs: ['approval_policy.investor_ops.operator_review'],
    caveatRefs: ['caveat.investor_ops.no_outreach_authority'],
    closeoutRequirementRefs: ['closeout.investor_ops.receipt_and_acceptance'],
    createdAtIso: '2026-06-07T05:30:00.000Z',
    dataRoomPolicyRefs: ['policy.data_room.no_private_upload'],
    evidenceRequirementRefs: ['evidence_requirement.investor_ops.receipt'],
    id: 'investor_ops_template.default',
    proofPolicyRefs: ['proof_policy.investor_ops.team_safe_summary'],
    requiredArtifactRefs: ['artifact_requirement.investor_ops.packet'],
    templateRef: 'template.investor_ops.default',
    updatedAtIso: '2026-06-07T05:35:00.000Z',
    versionRef: 'version.investor_ops.v1',
  }

export const OMNI_INVESTOR_OPS_WORKROOM_FIXTURE:
  OmniInvestorOpsWorkroomRecord = {
    acceptanceRefs: ['acceptance.investor_ops.partner_interest'],
    authority: OMNI_INVESTOR_OPS_CONTRACT_ONLY_AUTHORITY,
    blockerRefs: [],
    caveatRefs: ['caveat.investor_ops.contract_only'],
    closeoutRefs: ['closeout.investor_ops.partner_packet_done'],
    contactRefs: ['contact.investor_ops.partner_public_ref'],
    createdAtIso: '2026-06-07T05:40:00.000Z',
    dataRoomTaskRefs: ['data_room_task.investor_ops.index_safe'],
    decisionReceiptRefs: ['decision.investor_ops.partner_follow_up'],
    deckWorkOrderRefs: ['deck.work_order.investor_ops.update'],
    evidenceRefs: ['evidence.investor_ops.summary'],
    followUpRefs: ['follow_up.investor_ops.queue_item'],
    id: 'investor_ops_workroom.partner_1',
    investorRefs: ['investor.investor_ops.partner_public_ref'],
    operatorDiagnosticRefs: ['diagnostic.operator.investor_ops_route'],
    prepPacketRefs: ['prep_packet.investor_ops.partner'],
    sourceRefs: ['source.investor_ops.public_summary'],
    state: 'closed',
    templateRef: 'template.investor_ops.default',
    updatedAtIso: '2026-06-07T06:00:00.000Z',
    videoWorkOrderRefs: ['video.work_order.investor_ops.short_clip'],
    workroomRef: 'workroom.investor_ops.partner',
  }
