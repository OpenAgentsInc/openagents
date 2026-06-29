import { Schema as S } from 'effect'

import {
  type OmniAcceptedOutcomeArtifactKind,
  type OmniAcceptedOutcomeCloseoutRequirementKind,
  type OmniAcceptedOutcomeProofPolicy,
  type OmniAcceptedOutcomeReviewPolicy,
  type OmniAcceptedOutcomeWorkKind,
} from './omni-accepted-outcome-contracts'
import type { OmniEvidenceEntryKind } from './omni-evidence-bundles'

export const OmniWorkroomKindTemplateKind = S.Literals([
  'site',
  'coding',
  'crm',
  'investor_ops',
  'project_ops',
  'support',
  'finance_ops',
  'meeting',
  'document',
  'legal_review',
])
export type OmniWorkroomKindTemplateKind =
  typeof OmniWorkroomKindTemplateKind.Type

export const OmniWorkroomKindPublicProjectionPolicy = S.Literals([
  'none',
  'customer_safe_summary',
  'team_safe_summary',
  'public_safe_proof',
])
export type OmniWorkroomKindPublicProjectionPolicy =
  typeof OmniWorkroomKindPublicProjectionPolicy.Type

export const OmniWorkroomKindPrivacyConstraint = S.Literals([
  'public_ok',
  'customer_private',
  'team_private',
  'financial_private',
  'legal_private',
])
export type OmniWorkroomKindPrivacyConstraint =
  typeof OmniWorkroomKindPrivacyConstraint.Type

export const OmniWorkroomKindRequiredEvidence = S.Struct({
  entryKind: S.String,
  publicSafeAllowed: S.Boolean,
  required: S.Boolean,
})
export type OmniWorkroomKindRequiredEvidence = Readonly<{
  entryKind: OmniEvidenceEntryKind
  publicSafeAllowed: boolean
  required: boolean
}>

export const OmniWorkroomKindRequiredArtifact = S.Struct({
  artifactKind: S.String,
  publicSafeAllowed: S.Boolean,
  required: S.Boolean,
})
export type OmniWorkroomKindRequiredArtifact = Readonly<{
  artifactKind: OmniAcceptedOutcomeArtifactKind
  publicSafeAllowed: boolean
  required: boolean
}>

export const OmniWorkroomKindTemplate = S.Struct({
  acceptedOutcomeWorkKind: S.String,
  closeoutRequirements: S.Array(S.String),
  descriptionRef: S.String,
  kind: OmniWorkroomKindTemplateKind,
  privacyConstraint: OmniWorkroomKindPrivacyConstraint,
  proofPolicy: S.String,
  publicProjectionPolicy: OmniWorkroomKindPublicProjectionPolicy,
  requiredArtifacts: S.Array(OmniWorkroomKindRequiredArtifact),
  requiredEvidence: S.Array(OmniWorkroomKindRequiredEvidence),
  reviewPolicy: S.String,
})
export type OmniWorkroomKindTemplate = Readonly<{
  acceptedOutcomeWorkKind: OmniAcceptedOutcomeWorkKind
  closeoutRequirements: ReadonlyArray<OmniAcceptedOutcomeCloseoutRequirementKind>
  descriptionRef: string
  kind: OmniWorkroomKindTemplateKind
  privacyConstraint: OmniWorkroomKindPrivacyConstraint
  proofPolicy: OmniAcceptedOutcomeProofPolicy
  publicProjectionPolicy: OmniWorkroomKindPublicProjectionPolicy
  requiredArtifacts: ReadonlyArray<OmniWorkroomKindRequiredArtifact>
  requiredEvidence: ReadonlyArray<OmniWorkroomKindRequiredEvidence>
  reviewPolicy: OmniAcceptedOutcomeReviewPolicy
}>

export class OmniWorkroomKindTemplateValidationError extends S.TaggedErrorClass<OmniWorkroomKindTemplateValidationError>()(
  'OmniWorkroomKindTemplateValidationError',
  { reason: S.String },
) {}

const evidence = (
  entryKind: OmniEvidenceEntryKind,
  required: boolean,
  publicSafeAllowed: boolean,
): OmniWorkroomKindRequiredEvidence => ({
  entryKind,
  publicSafeAllowed,
  required,
})

const artifact = (
  artifactKind: OmniAcceptedOutcomeArtifactKind,
  required: boolean,
  publicSafeAllowed: boolean,
): OmniWorkroomKindRequiredArtifact => ({
  artifactKind,
  publicSafeAllowed,
  required,
})

export const OMNI_WORKROOM_KIND_TEMPLATES: Readonly<Record<
  OmniWorkroomKindTemplateKind,
  OmniWorkroomKindTemplate
>> = {
  coding: {
    acceptedOutcomeWorkKind: 'coding',
    closeoutRequirements: ['operator_review', 'tests_passed', 'source_exported'],
    descriptionRef: 'omni_template_coding_pr_or_patch',
    kind: 'coding',
    privacyConstraint: 'customer_private',
    proofPolicy: 'customer_safe_summary',
    publicProjectionPolicy: 'customer_safe_summary',
    requiredArtifacts: [
      artifact('pull_request', true, false),
      artifact('diff', true, false),
      artifact('source_commit', true, false),
      artifact('test_report', true, false),
    ],
    requiredEvidence: [
      evidence('diff', true, false),
      evidence('source_commit', true, false),
      evidence('test_report', true, false),
    ],
    reviewPolicy: 'customer_review',
  },
  crm: {
    acceptedOutcomeWorkKind: 'business',
    closeoutRequirements: ['operator_review', 'proof_bundle_ready'],
    descriptionRef: 'omni_template_crm_customer_ops',
    kind: 'crm',
    privacyConstraint: 'customer_private',
    proofPolicy: 'private_receipt',
    publicProjectionPolicy: 'none',
    requiredArtifacts: [artifact('operator_receipt', true, false)],
    requiredEvidence: [
      evidence('receipt', true, false),
      evidence('redaction_report', true, false),
    ],
    reviewPolicy: 'operator_review',
  },
  document: {
    acceptedOutcomeWorkKind: 'business',
    closeoutRequirements: ['customer_review', 'proof_bundle_ready'],
    descriptionRef: 'omni_template_document_work',
    kind: 'document',
    privacyConstraint: 'customer_private',
    proofPolicy: 'customer_safe_summary',
    publicProjectionPolicy: 'customer_safe_summary',
    requiredArtifacts: [
      artifact('research_brief', true, false),
      artifact('operator_receipt', true, false),
    ],
    requiredEvidence: [
      evidence('research_brief', true, false),
      evidence('receipt', true, false),
    ],
    reviewPolicy: 'customer_review',
  },
  finance_ops: {
    acceptedOutcomeWorkKind: 'business',
    closeoutRequirements: ['operator_review', 'proof_bundle_ready'],
    descriptionRef: 'omni_template_finance_ops',
    kind: 'finance_ops',
    privacyConstraint: 'financial_private',
    proofPolicy: 'private_receipt',
    publicProjectionPolicy: 'none',
    requiredArtifacts: [artifact('operator_receipt', true, false)],
    requiredEvidence: [
      evidence('receipt', true, false),
      evidence('redaction_report', true, false),
    ],
    reviewPolicy: 'owner_review',
  },
  investor_ops: {
    acceptedOutcomeWorkKind: 'business',
    closeoutRequirements: ['operator_review', 'proof_bundle_ready'],
    descriptionRef: 'omni_template_investor_ops',
    kind: 'investor_ops',
    privacyConstraint: 'team_private',
    proofPolicy: 'customer_safe_summary',
    publicProjectionPolicy: 'team_safe_summary',
    requiredArtifacts: [
      artifact('research_brief', true, false),
      artifact('operator_receipt', true, false),
    ],
    requiredEvidence: [
      evidence('research_brief', true, false),
      evidence('receipt', true, false),
    ],
    reviewPolicy: 'operator_review',
  },
  legal_review: {
    acceptedOutcomeWorkKind: 'legal_sensitive',
    closeoutRequirements: [
      'legal_review',
      'redaction_passed',
      'operator_review',
      'proof_bundle_ready',
    ],
    descriptionRef: 'omni_template_legal_review_private',
    kind: 'legal_review',
    privacyConstraint: 'legal_private',
    proofPolicy: 'legal_sensitive_private',
    publicProjectionPolicy: 'none',
    requiredArtifacts: [
      artifact('research_brief', true, false),
      artifact('redaction_report', true, false),
      artifact('operator_receipt', true, false),
    ],
    requiredEvidence: [
      evidence('research_brief', true, false),
      evidence('redaction_report', true, false),
      evidence('receipt', true, false),
    ],
    reviewPolicy: 'dual_review',
  },
  meeting: {
    acceptedOutcomeWorkKind: 'business',
    closeoutRequirements: ['customer_review', 'proof_bundle_ready'],
    descriptionRef: 'omni_template_meeting_summary',
    kind: 'meeting',
    privacyConstraint: 'customer_private',
    proofPolicy: 'customer_safe_summary',
    publicProjectionPolicy: 'customer_safe_summary',
    requiredArtifacts: [
      artifact('research_brief', true, false),
      artifact('operator_receipt', true, false),
    ],
    requiredEvidence: [
      evidence('research_brief', true, false),
      evidence('receipt', true, false),
    ],
    reviewPolicy: 'customer_review',
  },
  project_ops: {
    acceptedOutcomeWorkKind: 'business',
    closeoutRequirements: ['operator_review', 'proof_bundle_ready'],
    descriptionRef: 'omni_template_project_ops',
    kind: 'project_ops',
    privacyConstraint: 'team_private',
    proofPolicy: 'customer_safe_summary',
    publicProjectionPolicy: 'team_safe_summary',
    requiredArtifacts: [artifact('operator_receipt', true, false)],
    requiredEvidence: [
      evidence('receipt', true, false),
      evidence('test_report', false, false),
    ],
    reviewPolicy: 'operator_review',
  },
  site: {
    acceptedOutcomeWorkKind: 'site',
    closeoutRequirements: [
      'build_passed',
      'tests_passed',
      'deployment_live',
      'customer_review',
      'email_sent',
      'proof_bundle_ready',
    ],
    descriptionRef: 'omni_template_site_revision_or_launch',
    kind: 'site',
    privacyConstraint: 'public_ok',
    proofPolicy: 'public_safe_proof',
    publicProjectionPolicy: 'public_safe_proof',
    requiredArtifacts: [
      artifact('site_url', true, true),
      artifact('site_version', true, true),
      artifact('screenshot', true, true),
      artifact('test_report', true, true),
      artifact('email_receipt', true, true),
    ],
    requiredEvidence: [
      evidence('deployment_url', true, true),
      evidence('screenshot', true, true),
      evidence('test_report', true, true),
      evidence('email_receipt', true, true),
    ],
    reviewPolicy: 'customer_review',
  },
  support: {
    acceptedOutcomeWorkKind: 'business',
    closeoutRequirements: ['operator_review', 'email_sent'],
    descriptionRef: 'omni_template_support_case',
    kind: 'support',
    privacyConstraint: 'customer_private',
    proofPolicy: 'private_receipt',
    publicProjectionPolicy: 'none',
    requiredArtifacts: [
      artifact('email_receipt', true, false),
      artifact('operator_receipt', true, false),
    ],
    requiredEvidence: [
      evidence('email_receipt', true, false),
      evidence('receipt', true, false),
    ],
    reviewPolicy: 'operator_review',
  },
}

export const getOmniWorkroomKindTemplate = (
  kind: OmniWorkroomKindTemplateKind,
): OmniWorkroomKindTemplate => OMNI_WORKROOM_KIND_TEMPLATES[kind]

const publicProjectionAllowed = (
  template: OmniWorkroomKindTemplate,
): boolean =>
  template.publicProjectionPolicy === 'public_safe_proof'

export const validateOmniWorkroomKindTemplate = (
  template: OmniWorkroomKindTemplate,
): OmniWorkroomKindTemplate => {
  if (
    template.publicProjectionPolicy === 'public_safe_proof' &&
    template.proofPolicy !== 'public_safe_proof'
  ) {
    throw new OmniWorkroomKindTemplateValidationError({
      reason:
        'public_safe_proof projections require public_safe_proof proof policy.',
    })
  }

  if (
    template.privacyConstraint === 'legal_private' &&
    template.publicProjectionPolicy !== 'none'
  ) {
    throw new OmniWorkroomKindTemplateValidationError({
      reason: 'legal-private templates may not allow public projection.',
    })
  }

  if (
    publicProjectionAllowed(template) &&
    template.requiredEvidence.some(
      item => item.required && item.publicSafeAllowed === false,
    )
  ) {
    throw new OmniWorkroomKindTemplateValidationError({
      reason:
        'customer/public projection templates may not require private-only evidence.',
    })
  }

  return template
}

export const validateOmniWorkroomKindEvidence = (
  kind: OmniWorkroomKindTemplateKind,
  entries: ReadonlyArray<OmniEvidenceEntryKind>,
): void => {
  const template = getOmniWorkroomKindTemplate(kind)
  const entrySet = new Set(entries)
  const missing = template.requiredEvidence.filter(
    item => item.required && !entrySet.has(item.entryKind),
  )

  if (missing.length > 0) {
    throw new OmniWorkroomKindTemplateValidationError({
      reason: `missing required evidence: ${missing
        .map(item => item.entryKind)
        .join(', ')}`,
    })
  }
}
