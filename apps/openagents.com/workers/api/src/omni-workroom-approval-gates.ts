import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { OmniAcceptedOutcomeWorkKind as OmniAcceptedOutcomeWorkKindSchema } from './omni-accepted-outcome-contracts'
import {
  VerticalPackComplianceProfile,
  VerticalPackOutboundComplianceCheckInput,
  VerticalPackOutboundComplianceDecision,
  decideVerticalPackOutboundCompliance,
} from './blueprint/vertical-pack'

export const OmniWorkroomApprovalLadderLevel = S.Literals([
  'draft',
  'suggest',
  'execute_with_approval',
  'trusted',
])
export type OmniWorkroomApprovalLadderLevel =
  typeof OmniWorkroomApprovalLadderLevel.Type

export const OmniWorkroomOutboundActionKind = S.Literals([
  'send',
  'publish',
  'file',
  'spend',
])
export type OmniWorkroomOutboundActionKind =
  typeof OmniWorkroomOutboundActionKind.Type

export const OmniWorkroomProfessionalReviewerRole = S.Literals([
  'licensed_practitioner',
  'professional_reviewer',
])
export type OmniWorkroomProfessionalReviewerRole =
  typeof OmniWorkroomProfessionalReviewerRole.Type

export class OmniWorkroomApprovalGatePolicy extends S.Class<OmniWorkroomApprovalGatePolicy>(
  'OmniWorkroomApprovalGatePolicy',
)({
  approvalLevel: OmniWorkroomApprovalLadderLevel,
  professionalReviewRequired: S.Boolean,
  professionalReviewerRole: S.NullOr(OmniWorkroomProfessionalReviewerRole),
  sourceRefs: S.Array(S.String),
  workspaceRef: S.String,
}) {}

export class OmniWorkroomOutboundDeliverableReviewInput extends S.Class<OmniWorkroomOutboundDeliverableReviewInput>(
  'OmniWorkroomOutboundDeliverableReviewInput',
)({
  approvalDecisionReceiptRefs: S.Array(S.String),
  complianceCheck: S.NullOr(VerticalPackOutboundComplianceCheckInput),
  complianceProfile: S.NullOr(VerticalPackComplianceProfile),
  deliverableRef: S.String,
  evidenceRefs: S.Array(S.String),
  outboundActionKind: OmniWorkroomOutboundActionKind,
  policy: OmniWorkroomApprovalGatePolicy,
  professionalReviewReceiptRefs: S.Array(S.String),
  reviewerRoleRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workroomRef: S.String,
}) {}

export class OmniWorkroomOutboundDeliverableReviewDecision extends S.Class<OmniWorkroomOutboundDeliverableReviewDecision>(
  'OmniWorkroomOutboundDeliverableReviewDecision',
)({
  approvalDecisionRecorded: S.Boolean,
  approvalLevel: OmniWorkroomApprovalLadderLevel,
  blockedExternalAction: S.Boolean,
  blockerRefs: S.Array(S.String),
  complianceDecision: S.NullOr(VerticalPackOutboundComplianceDecision),
  deliverableRef: S.String,
  evidenceRefs: S.Array(S.String),
  externalActionAllowed: S.Boolean,
  outboundActionKind: OmniWorkroomOutboundActionKind,
  professionalReviewRecorded: S.Boolean,
  professionalReviewRequired: S.Boolean,
  professionalReviewerRole: S.NullOr(OmniWorkroomProfessionalReviewerRole),
  receiptRefs: S.Array(S.String),
  reviewerRoleRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workroomRef: S.String,
  workspaceRef: S.String,
}) {}

export class OmniWorkroomApprovalGateUnsafe extends S.TaggedErrorClass<OmniWorkroomApprovalGateUnsafe>()(
  'OmniWorkroomApprovalGateUnsafe',
  { reason: S.String },
) {}

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?email|email[_ -]?body|contact[_ -]?email|customer[_ -]?email|customer[_ -]?name|run[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|settlement|payout|paid[_ -]?out|payment[_ -]?settled|eligible[_ -]?for[_ -]?payout|gho_[a-z0-9_]+|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|xprv|mnemonic)\b|@/i

const levelAllowsExternalAction: Readonly<
  Record<OmniWorkroomApprovalLadderLevel, boolean>
> = {
  draft: false,
  execute_with_approval: true,
  suggest: false,
  trusted: true,
}

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const assertSafeRef = (field: string, value: string): void => {
  if (!SAFE_REF_PATTERN.test(value) || !textIsSafe(value)) {
    throw new OmniWorkroomApprovalGateUnsafe({
      reason: `${field} must be a public-safe ref without raw provider, run log, email, payment, settlement, payout, wallet, or private customer material.`,
    })
  }
}

const assertSafeRefs = (
  field: string,
  refs: ReadonlyArray<string>,
): void => {
  refs.forEach(ref => {
    assertSafeRef(field, ref)
  })
}

const uniqueSorted = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].sort()

const professionalReviewRequiredFor = (
  input: OmniWorkroomOutboundDeliverableReviewInput,
): boolean =>
  input.policy.professionalReviewRequired || input.workKind === 'legal_sensitive'

const requiredProfessionalReviewerRoleFor = (
  input: OmniWorkroomOutboundDeliverableReviewInput,
): OmniWorkroomProfessionalReviewerRole | null =>
  professionalReviewRequiredFor(input)
    ? input.policy.professionalReviewerRole ?? 'licensed_practitioner'
    : null

const validateInputRefs = (
  input: OmniWorkroomOutboundDeliverableReviewInput,
): void => {
  assertSafeRef('deliverableRef', input.deliverableRef)
  assertSafeRef('workroomRef', input.workroomRef)
  assertSafeRef('workspaceRef', input.policy.workspaceRef)
  assertSafeRefs(
    'approvalDecisionReceiptRefs',
    input.approvalDecisionReceiptRefs,
  )
  assertSafeRefs('evidenceRefs', input.evidenceRefs)
  assertSafeRefs(
    'professionalReviewReceiptRefs',
    input.professionalReviewReceiptRefs,
  )
  assertSafeRefs('reviewerRoleRefs', input.reviewerRoleRefs)
  assertSafeRefs('sourceRefs', input.sourceRefs)
  assertSafeRefs('policy.sourceRefs', input.policy.sourceRefs)
  if (input.complianceCheck !== null) {
    assertSafeRef('complianceCheck.actionRef', input.complianceCheck.actionRef)
    assertSafeRef(
      'complianceCheck.verticalPackId',
      input.complianceCheck.verticalPackId,
    )
    assertSafeRefs(
      'complianceCheck.advertisingRuleConstraintRefs',
      input.complianceCheck.advertisingRuleConstraintRefs,
    )
    assertSafeRefs(
      'complianceCheck.consentChannelRefs',
      input.complianceCheck.consentChannelRefs,
    )
    assertSafeRefs(
      'complianceCheck.proposedActionRefs',
      input.complianceCheck.proposedActionRefs,
    )
    assertSafeRefs(
      'complianceCheck.provenanceReceiptRefs',
      input.complianceCheck.provenanceReceiptRefs,
    )
    assertSafeRefs(
      'complianceCheck.regulatedDataHandlingRefs',
      input.complianceCheck.regulatedDataHandlingRefs,
    )
    assertSafeRefs('complianceCheck.sourceRefs', input.complianceCheck.sourceRefs)
  }
}

const complianceDecisionFor = (
  input: OmniWorkroomOutboundDeliverableReviewInput,
): VerticalPackOutboundComplianceDecision | null => {
  if (input.complianceProfile === null || input.complianceCheck === null) {
    return null
  }

  return decideVerticalPackOutboundCompliance(
    input.complianceProfile,
    input.complianceCheck,
  )
}

const blockerRefsFor = (
  input: OmniWorkroomOutboundDeliverableReviewInput,
  complianceDecision: VerticalPackOutboundComplianceDecision | null,
): ReadonlyArray<string> => {
  const approvalLevelAllowsExternalAction =
    levelAllowsExternalAction[input.policy.approvalLevel]
  const approvalDecisionRecorded = input.approvalDecisionReceiptRefs.length > 0
  const professionalReviewRequired = professionalReviewRequiredFor(input)
  const professionalReviewRecorded =
    !professionalReviewRequired ||
    input.professionalReviewReceiptRefs.length > 0
  const requiredProfessionalReviewerRole =
    requiredProfessionalReviewerRoleFor(input)
  const requiredReviewerRoleRecorded =
    requiredProfessionalReviewerRole === null ||
    input.reviewerRoleRefs.some(ref =>
      ref.includes(requiredProfessionalReviewerRole),
    )

  return [
    ...(!approvalLevelAllowsExternalAction
      ? [
          `blocker.workroom_approval_ladder.${input.policy.approvalLevel}.external_action_not_allowed`,
        ]
      : []),
    ...(!approvalDecisionRecorded
      ? ['blocker.workroom_approval_ladder.decision_receipt_missing']
      : []),
    ...(!professionalReviewRecorded
      ? ['blocker.workroom_approval_ladder.professional_review_receipt_missing']
      : []),
    ...(!requiredReviewerRoleRecorded
      ? ['blocker.workroom_approval_ladder.professional_reviewer_role_missing']
      : []),
    ...(complianceDecision?.blockerRefs ?? []),
  ].sort()
}

export const decideOmniWorkroomOutboundDeliverableReview = (
  input: OmniWorkroomOutboundDeliverableReviewInput,
): OmniWorkroomOutboundDeliverableReviewDecision => {
  validateInputRefs(input)

  const complianceDecision = complianceDecisionFor(input)
  const blockerRefs = blockerRefsFor(input, complianceDecision)
  const professionalReviewRequired = professionalReviewRequiredFor(input)
  const professionalReviewerRole = requiredProfessionalReviewerRoleFor(input)
  const professionalReviewRecorded =
    !professionalReviewRequired ||
    input.professionalReviewReceiptRefs.length > 0
  const approvalDecisionRecorded = input.approvalDecisionReceiptRefs.length > 0
  const externalActionAllowed = blockerRefs.length === 0

  return new OmniWorkroomOutboundDeliverableReviewDecision({
    approvalDecisionRecorded,
    approvalLevel: input.policy.approvalLevel,
    blockedExternalAction: !externalActionAllowed,
    blockerRefs,
    complianceDecision,
    deliverableRef: input.deliverableRef,
    evidenceRefs: uniqueSorted(input.evidenceRefs),
    externalActionAllowed,
    outboundActionKind: input.outboundActionKind,
    professionalReviewRecorded,
    professionalReviewRequired,
    professionalReviewerRole,
    receiptRefs: uniqueSorted([
      ...input.approvalDecisionReceiptRefs,
      ...input.professionalReviewReceiptRefs,
    ]),
    reviewerRoleRefs: uniqueSorted(input.reviewerRoleRefs),
    sourceRefs: uniqueSorted([...input.sourceRefs, ...input.policy.sourceRefs]),
    workKind: input.workKind,
    workroomRef: input.workroomRef,
    workspaceRef: input.policy.workspaceRef,
  })
}
