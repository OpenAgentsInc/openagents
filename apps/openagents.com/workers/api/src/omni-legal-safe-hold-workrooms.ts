import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import {
  OmniProjectionAudience,
} from './omni-data-classification'

export const OmniLegalSafeHoldState = S.Literals([
  'blocked',
  'closed',
  'declined',
  'intake',
  'legal_review_recorded',
  'legal_review_requested',
  'released',
  'safe_hold_recorded',
  'scoping_recorded',
  'source_backed_summary_ready',
])
export type OmniLegalSafeHoldState =
  typeof OmniLegalSafeHoldState.Type

export const OmniLegalSafeHoldAuthorityBoundary = S.Literals([
  'safe_hold_contract_projection_only',
])
export type OmniLegalSafeHoldAuthorityBoundary =
  typeof OmniLegalSafeHoldAuthorityBoundary.Type

export class OmniLegalSafeHoldAuthority extends S.Class<OmniLegalSafeHoldAuthority>(
  'OmniLegalSafeHoldAuthority',
)({
  authorityBoundary: OmniLegalSafeHoldAuthorityBoundary,
  noAutomaticExecution: S.Boolean,
  noExternalSend: S.Boolean,
  noFiling: S.Boolean,
  noLegalAdviceClaims: S.Boolean,
  noPaymentSettlement: S.Boolean,
  noPublicProjectionUpgrade: S.Boolean,
}) {}

export class OmniLegalSafeHoldTemplate extends S.Class<OmniLegalSafeHoldTemplate>(
  'OmniLegalSafeHoldTemplate',
)({
  approvalPolicyRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  closeoutRequirementRefs: S.Array(S.String),
  createdAtIso: S.String,
  evidenceRequirementRefs: S.Array(S.String),
  holdPolicyRefs: S.Array(S.String),
  id: S.String,
  legalReviewRequirementRefs: S.Array(S.String),
  proofPolicyRefs: S.Array(S.String),
  releasePolicyRefs: S.Array(S.String),
  requiredArtifactRefs: S.Array(S.String),
  scopingRequirementRefs: S.Array(S.String),
  sourceRequirementRefs: S.Array(S.String),
  templateRef: S.String,
  updatedAtIso: S.String,
  versionRef: S.String,
}) {}

export class OmniLegalSafeHoldTemplateProjection extends S.Class<OmniLegalSafeHoldTemplateProjection>(
  'OmniLegalSafeHoldTemplateProjection',
)({
  approvalPolicyRefs: S.Array(S.String),
  audience: OmniProjectionAudience,
  caveatRefs: S.Array(S.String),
  closeoutRequirementRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  evidenceRequirementRefs: S.Array(S.String),
  holdPolicyRefs: S.Array(S.String),
  id: S.String,
  legalReviewRequirementRefs: S.Array(S.String),
  proofPolicyRefs: S.Array(S.String),
  releasePolicyRefs: S.Array(S.String),
  requiredArtifactRefs: S.Array(S.String),
  scopingRequirementRefs: S.Array(S.String),
  sourceRequirementRefs: S.Array(S.String),
  templateRef: S.String,
  updatedAtDisplay: S.String,
  versionRef: S.String,
}) {}

export class OmniLegalSafeHoldWorkroomRecord extends S.Class<OmniLegalSafeHoldWorkroomRecord>(
  'OmniLegalSafeHoldWorkroomRecord',
)({
  authority: OmniLegalSafeHoldAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  clientRefs: S.Array(S.String),
  closeoutRefs: S.Array(S.String),
  createdAtIso: S.String,
  declineRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  holdRefs: S.Array(S.String),
  id: S.String,
  jurisdictionRefs: S.Array(S.String),
  legalReviewRefs: S.Array(S.String),
  matterRefs: S.Array(S.String),
  operatorDiagnosticRefs: S.Array(S.String),
  releaseRefs: S.Array(S.String),
  scopingRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: OmniLegalSafeHoldState,
  templateRef: S.String,
  updatedAtIso: S.String,
  workroomRef: S.String,
}) {}

export class OmniLegalSafeHoldWorkroomProjection extends S.Class<OmniLegalSafeHoldWorkroomProjection>(
  'OmniLegalSafeHoldWorkroomProjection',
)({
  audience: OmniProjectionAudience,
  authority: OmniLegalSafeHoldAuthority,
  automaticExecutionAllowed: S.Boolean,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  clientRefs: S.Array(S.String),
  closeoutReady: S.Boolean,
  closeoutRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  declineRecorded: S.Boolean,
  declineRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  externalSendAllowed: S.Boolean,
  filingAllowed: S.Boolean,
  holdRecorded: S.Boolean,
  holdRefs: S.Array(S.String),
  id: S.String,
  jurisdictionRefs: S.Array(S.String),
  legalAdviceClaimsAllowed: S.Boolean,
  legalReviewRecorded: S.Boolean,
  legalReviewRefs: S.Array(S.String),
  legalReviewRequested: S.Boolean,
  matterRefs: S.Array(S.String),
  operatorDiagnosticRefs: S.Array(S.String),
  paymentSettlementAllowed: S.Boolean,
  publicProjectionUpgradeAllowed: S.Boolean,
  releaseRecorded: S.Boolean,
  releaseRefs: S.Array(S.String),
  scopingReady: S.Boolean,
  scopingRefs: S.Array(S.String),
  sourceBackedSummaryReady: S.Boolean,
  sourceRefs: S.Array(S.String),
  state: OmniLegalSafeHoldState,
  stateLabel: S.String,
  templateRef: S.String,
  updatedAtDisplay: S.String,
  workroomRef: S.String,
}) {}

export class OmniLegalSafeHoldUnsafe extends S.TaggedErrorClass<OmniLegalSafeHoldUnsafe>()(
  'OmniLegalSafeHoldUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_LEGAL_SAFE_HOLD_CONTRACT_ONLY_AUTHORITY:
  OmniLegalSafeHoldAuthority = {
    authorityBoundary: 'safe_hold_contract_projection_only',
    noAutomaticExecution: true,
    noExternalSend: true,
    noFiling: true,
    noLegalAdviceClaims: true,
    noPaymentSettlement: true,
    noPublicProjectionUpgrade: true,
  }

const stateRank: Readonly<Record<OmniLegalSafeHoldState, number>> = {
  blocked: -1,
  closed: 8,
  declined: 7,
  intake: 0,
  legal_review_recorded: 5,
  legal_review_requested: 4,
  released: 7,
  safe_hold_recorded: 1,
  scoping_recorded: 2,
  source_backed_summary_ready: 3,
}

const stateLabelByState: Readonly<Record<OmniLegalSafeHoldState, string>> = {
  blocked: 'Blocked',
  closed: 'Closed',
  declined: 'Declined',
  intake: 'Intake',
  legal_review_recorded: 'Legal review recorded',
  legal_review_requested: 'Legal review requested',
  released: 'Released',
  safe_hold_recorded: 'Safe hold recorded',
  scoping_recorded: 'Scoping recorded',
  source_backed_summary_ready: 'Source-backed summary ready',
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeLegalSafeHoldRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|attorney[_-]?client|auth\.json|bearer|callback[_-]?token|client[_-]?(address|email|identity|name|phone)|confidential|cookie|customer[_-]?(email|identity|name|phone|value)|draft[_-]?filing[_-]?raw|email[_-]?(address|body|html|raw|text)|filing[_-]?(claim|raw|submission)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|legal[_-]?advice[_-]?claim|lnbc|lntb|lnbcrt|lno1|matter[_-]?(confidential|identity|private|raw)|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(client|customer|key|matter|source)|privileged|provider[_-]?(account|grant|payload|token)|raw[_-]?(client|document|doc|email|filing|invoice|legal|matter|payment|payload|prompt|provider|runner|run[_-]?log|source|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(client\.|closeout\.|decline\.|hold\.|jurisdiction\.|legal_review\.|matter\.|release\.|scoping\.|source\.|workroom\.)/i
const customerUnsafeRefPattern =
  /(diagnostic\.operator|matter\.private|provider\.private|source\.private)/i
const teamUnsafeRefPattern =
  /(diagnostic\.operator|matter\.private|provider\.private|source\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

export const omniLegalSafeHoldStateAtLeast = (
  state: OmniLegalSafeHoldState,
  threshold: OmniLegalSafeHoldState,
): boolean => stateRank[state] >= stateRank[threshold]

export const omniLegalSafeHoldAuthorityIsContractOnly = (
  authority: OmniLegalSafeHoldAuthority,
): boolean =>
  authority.authorityBoundary === 'safe_hold_contract_projection_only' &&
  authority.noAutomaticExecution &&
  authority.noExternalSend &&
  authority.noFiling &&
  authority.noLegalAdviceClaims &&
  authority.noPaymentSettlement &&
  authority.noPublicProjectionUpgrade

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeLegalSafeHoldRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new OmniLegalSafeHoldUnsafe({
      reason: `${label} contains legal-sensitive data, client identity, matter data, privileged/confidential refs, raw docs, provider material, private repo refs, wallet/payment material, secrets, raw logs, or raw timestamps.`,
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
  template: OmniLegalSafeHoldTemplate,
): ReadonlyArray<string> => [
  template.id,
  template.templateRef,
  template.versionRef,
  ...template.approvalPolicyRefs,
  ...template.caveatRefs,
  ...template.closeoutRequirementRefs,
  ...template.evidenceRequirementRefs,
  ...template.holdPolicyRefs,
  ...template.legalReviewRequirementRefs,
  ...template.proofPolicyRefs,
  ...template.releasePolicyRefs,
  ...template.requiredArtifactRefs,
  ...template.scopingRequirementRefs,
  ...template.sourceRequirementRefs,
]

const recordRefs = (
  record: OmniLegalSafeHoldWorkroomRecord,
): ReadonlyArray<string> => [
  record.id,
  record.templateRef,
  record.workroomRef,
  ...record.blockerRefs,
  ...record.caveatRefs,
  ...record.clientRefs,
  ...record.closeoutRefs,
  ...record.declineRefs,
  ...record.evidenceRefs,
  ...record.holdRefs,
  ...record.jurisdictionRefs,
  ...record.legalReviewRefs,
  ...record.matterRefs,
  ...record.operatorDiagnosticRefs,
  ...record.releaseRefs,
  ...record.scopingRefs,
  ...record.sourceRefs,
]

const assertTemplateSafe = (
  template: OmniLegalSafeHoldTemplate,
): void => {
  assertSafeRefs('Legal safe-hold template refs', templateRefs(template))
}

const assertRecordSafe = (
  record: OmniLegalSafeHoldWorkroomRecord,
): void => {
  assertSafeRefs('Legal safe-hold workroom refs', recordRefs(record))

  if (!omniLegalSafeHoldAuthorityIsContractOnly(record.authority)) {
    throw new OmniLegalSafeHoldUnsafe({
      reason: 'Legal safe-hold records must remain contract/projection-only and cannot carry automatic execution, external send, filing, legal advice claim, payment settlement, or public projection upgrade authority.',
    })
  }

  if (record.state === 'blocked' && record.blockerRefs.length === 0) {
    throw new OmniLegalSafeHoldUnsafe({
      reason: 'Blocked legal safe-hold workrooms require blocker refs.',
    })
  }

  if (
    omniLegalSafeHoldStateAtLeast(record.state, 'safe_hold_recorded') &&
    record.holdRefs.length === 0
  ) {
    throw new OmniLegalSafeHoldUnsafe({
      reason: 'Safe-hold state requires hold refs.',
    })
  }

  if (
    omniLegalSafeHoldStateAtLeast(record.state, 'scoping_recorded') &&
    record.scopingRefs.length === 0
  ) {
    throw new OmniLegalSafeHoldUnsafe({
      reason: 'Scoping state requires scoping refs.',
    })
  }

  if (
    omniLegalSafeHoldStateAtLeast(
      record.state,
      'source_backed_summary_ready',
    ) &&
    record.sourceRefs.length === 0
  ) {
    throw new OmniLegalSafeHoldUnsafe({
      reason: 'Source-backed summary state requires source refs.',
    })
  }

  if (
    omniLegalSafeHoldStateAtLeast(record.state, 'legal_review_requested') &&
    record.legalReviewRefs.length === 0
  ) {
    throw new OmniLegalSafeHoldUnsafe({
      reason: 'Legal review state requires legal review refs.',
    })
  }

  if (record.state === 'released' && record.releaseRefs.length === 0) {
    throw new OmniLegalSafeHoldUnsafe({
      reason: 'Released legal safe-hold workrooms require release refs.',
    })
  }

  if (record.state === 'declined' && record.declineRefs.length === 0) {
    throw new OmniLegalSafeHoldUnsafe({
      reason: 'Declined legal safe-hold workrooms require decline refs.',
    })
  }

  if (
    record.state === 'closed' &&
    record.releaseRefs.length === 0 &&
    record.declineRefs.length === 0
  ) {
    throw new OmniLegalSafeHoldUnsafe({
      reason: 'Closed legal safe-hold workrooms require release or decline refs.',
    })
  }

  if (record.state === 'closed' && record.closeoutRefs.length === 0) {
    throw new OmniLegalSafeHoldUnsafe({
      reason: 'Closed legal safe-hold workrooms require closeout refs.',
    })
  }
}

export const projectOmniLegalSafeHoldTemplate = (
  template: OmniLegalSafeHoldTemplate,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OmniLegalSafeHoldTemplateProjection => {
  assertTemplateSafe(template)

  return {
    approvalPolicyRefs: safeRefsForAudience(
      'Legal safe-hold approval policy refs',
      template.approvalPolicyRefs,
      audience,
    ),
    audience,
    caveatRefs: safeRefsForAudience(
      'Legal safe-hold caveat refs',
      template.caveatRefs,
      audience,
    ),
    closeoutRequirementRefs: safeRefsForAudience(
      'Legal safe-hold closeout requirement refs',
      template.closeoutRequirementRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      template.createdAtIso,
      nowIso,
    ),
    evidenceRequirementRefs: safeRefsForAudience(
      'Legal safe-hold evidence requirement refs',
      template.evidenceRequirementRefs,
      audience,
    ),
    holdPolicyRefs: safeRefsForAudience(
      'Legal safe-hold policy refs',
      template.holdPolicyRefs,
      audience,
    ),
    id: safeRefForAudience('Legal safe-hold template id', template.id, audience),
    legalReviewRequirementRefs: safeRefsForAudience(
      'Legal safe-hold review requirement refs',
      template.legalReviewRequirementRefs,
      audience,
    ),
    proofPolicyRefs: safeRefsForAudience(
      'Legal safe-hold proof policy refs',
      template.proofPolicyRefs,
      audience,
    ),
    releasePolicyRefs: safeRefsForAudience(
      'Legal safe-hold release policy refs',
      template.releasePolicyRefs,
      audience,
    ),
    requiredArtifactRefs: safeRefsForAudience(
      'Legal safe-hold artifact refs',
      template.requiredArtifactRefs,
      audience,
    ),
    scopingRequirementRefs: safeRefsForAudience(
      'Legal safe-hold scoping requirement refs',
      template.scopingRequirementRefs,
      audience,
    ),
    sourceRequirementRefs: safeRefsForAudience(
      'Legal safe-hold source requirement refs',
      template.sourceRequirementRefs,
      audience,
    ),
    templateRef: safeRefForAudience(
      'Legal safe-hold template ref',
      template.templateRef,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      template.updatedAtIso,
      nowIso,
    ),
    versionRef: safeRefForAudience(
      'Legal safe-hold template version ref',
      template.versionRef,
      audience,
    ),
  }
}

export const projectOmniLegalSafeHoldWorkroom = (
  record: OmniLegalSafeHoldWorkroomRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OmniLegalSafeHoldWorkroomProjection => {
  assertRecordSafe(record)

  return {
    audience,
    authority: record.authority,
    automaticExecutionAllowed: false,
    blockerRefs: safeRefsForAudience(
      'Legal safe-hold blocker refs',
      record.blockerRefs,
      audience,
    ),
    caveatRefs: safeRefsForAudience(
      'Legal safe-hold caveat refs',
      record.caveatRefs,
      audience,
    ),
    clientRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Legal safe-hold client refs',
        record.clientRefs,
        audience,
      ),
    closeoutReady: record.state === 'closed' &&
      record.closeoutRefs.length > 0 &&
      (record.releaseRefs.length > 0 || record.declineRefs.length > 0),
    closeoutRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'Legal safe-hold closeout refs',
        record.closeoutRefs,
        audience,
      )
      : [],
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    declineRecorded:
      record.state === 'declined' ||
      (record.state === 'closed' && record.declineRefs.length > 0),
    declineRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'Legal safe-hold decline refs',
        record.declineRefs,
        audience,
      )
      : [],
    evidenceRefs: safeRefsForAudience(
      'Legal safe-hold evidence refs',
      record.evidenceRefs,
      audience,
    ),
    externalSendAllowed: false,
    filingAllowed: false,
    holdRecorded:
      omniLegalSafeHoldStateAtLeast(record.state, 'safe_hold_recorded') &&
      record.holdRefs.length > 0,
    holdRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience('Legal safe-hold hold refs', record.holdRefs, audience)
      : [],
    id: safeRefForAudience('Legal safe-hold workroom id', record.id, audience),
    jurisdictionRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Legal safe-hold jurisdiction refs',
        record.jurisdictionRefs,
        audience,
      ),
    legalAdviceClaimsAllowed: false,
    legalReviewRecorded:
      omniLegalSafeHoldStateAtLeast(record.state, 'legal_review_recorded') &&
      record.legalReviewRefs.length > 0,
    legalReviewRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'Legal safe-hold legal review refs',
        record.legalReviewRefs,
        audience,
      )
      : [],
    legalReviewRequested:
      omniLegalSafeHoldStateAtLeast(record.state, 'legal_review_requested') &&
      record.legalReviewRefs.length > 0,
    matterRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Legal safe-hold matter refs',
        record.matterRefs,
        audience,
      ),
    operatorDiagnosticRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'Legal safe-hold operator diagnostic refs',
        record.operatorDiagnosticRefs,
        audience,
      )
      : [],
    paymentSettlementAllowed: false,
    publicProjectionUpgradeAllowed: false,
    releaseRecorded:
      record.state === 'released' ||
      (record.state === 'closed' && record.releaseRefs.length > 0),
    releaseRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'Legal safe-hold release refs',
        record.releaseRefs,
        audience,
      )
      : [],
    scopingReady:
      omniLegalSafeHoldStateAtLeast(record.state, 'scoping_recorded') &&
      record.scopingRefs.length > 0,
    scopingRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Legal safe-hold scoping refs',
        record.scopingRefs,
        audience,
      ),
    sourceBackedSummaryReady:
      omniLegalSafeHoldStateAtLeast(
        record.state,
        'source_backed_summary_ready',
      ) && record.sourceRefs.length > 0,
    sourceRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Legal safe-hold source refs',
        record.sourceRefs,
        audience,
      ),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    templateRef: safeRefForAudience(
      'Legal safe-hold template ref',
      record.templateRef,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workroomRef: audience === 'public' || audience === 'agent'
      ? 'redacted'
      : safeRefForAudience(
        'Legal safe-hold workroom ref',
        record.workroomRef,
        audience,
      ),
  }
}

const projectionText = (
  projection:
    | OmniLegalSafeHoldTemplateProjection
    | OmniLegalSafeHoldWorkroomProjection,
): string =>
  'clientRefs' in projection
    ? [
      projection.id,
      projection.templateRef,
      projection.workroomRef,
      ...projection.blockerRefs,
      ...projection.caveatRefs,
      ...projection.clientRefs,
      ...projection.closeoutRefs,
      ...projection.declineRefs,
      ...projection.evidenceRefs,
      ...projection.holdRefs,
      ...projection.jurisdictionRefs,
      ...projection.legalReviewRefs,
      ...projection.matterRefs,
      ...projection.operatorDiagnosticRefs,
      ...projection.releaseRefs,
      ...projection.scopingRefs,
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
      ...projection.holdPolicyRefs,
      ...projection.legalReviewRequirementRefs,
      ...projection.proofPolicyRefs,
      ...projection.releasePolicyRefs,
      ...projection.requiredArtifactRefs,
      ...projection.scopingRequirementRefs,
      ...projection.sourceRequirementRefs,
    ].join(' ')

export const omniLegalSafeHoldProjectionHasPrivateMaterial = (
  projection:
    | OmniLegalSafeHoldTemplateProjection
    | OmniLegalSafeHoldWorkroomProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeLegalSafeHoldRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const OMNI_LEGAL_SAFE_HOLD_TEMPLATE_FIXTURE:
  OmniLegalSafeHoldTemplate = {
    approvalPolicyRefs: ['approval_policy.legal_safe_hold.operator_review'],
    caveatRefs: ['caveat.legal_safe_hold.no_legal_advice'],
    closeoutRequirementRefs: ['closeout.legal_safe_hold.release_or_decline'],
    createdAtIso: '2026-06-07T07:00:00.000Z',
    evidenceRequirementRefs: ['evidence_requirement.legal.source_backed'],
    holdPolicyRefs: ['hold_policy.legal_sensitive.operator_hold'],
    id: 'legal_safe_hold_template.default',
    legalReviewRequirementRefs: ['legal_review_requirement.human_required'],
    proofPolicyRefs: ['proof_policy.legal.private_only'],
    releasePolicyRefs: ['release_policy.legal.operator_release_required'],
    requiredArtifactRefs: ['artifact_requirement.legal.safe_summary'],
    scopingRequirementRefs: ['scoping_requirement.legal.limited_scope'],
    sourceRequirementRefs: ['source_requirement.legal.source_backed_summary'],
    templateRef: 'template.legal_safe_hold.default',
    updatedAtIso: '2026-06-07T07:05:00.000Z',
    versionRef: 'version.legal_safe_hold.v1',
  }

export const OMNI_LEGAL_SAFE_HOLD_WORKROOM_FIXTURE:
  OmniLegalSafeHoldWorkroomRecord = {
    authority: OMNI_LEGAL_SAFE_HOLD_CONTRACT_ONLY_AUTHORITY,
    blockerRefs: [],
    caveatRefs: ['caveat.legal_safe_hold.attorney_review_required'],
    clientRefs: ['client.legal.public_ref'],
    closeoutRefs: ['closeout.legal.release_recorded'],
    createdAtIso: '2026-06-07T07:10:00.000Z',
    declineRefs: [],
    evidenceRefs: ['evidence.legal.source_summary'],
    holdRefs: ['hold.legal.operator_safe_hold'],
    id: 'legal_safe_hold_workroom.matter_1',
    jurisdictionRefs: ['jurisdiction.us.mn'],
    legalReviewRefs: ['legal_review.human.operator_reviewed'],
    matterRefs: ['matter.legal.public_ref'],
    operatorDiagnosticRefs: ['diagnostic.operator.legal_hold_route'],
    releaseRefs: ['release.legal.operator_released'],
    scopingRefs: ['scoping.legal.limited_request'],
    sourceRefs: ['source.legal.public_statute_summary'],
    state: 'closed',
    templateRef: 'template.legal_safe_hold.default',
    updatedAtIso: '2026-06-07T07:30:00.000Z',
    workroomRef: 'workroom.legal_safe_hold.matter_1',
  }
