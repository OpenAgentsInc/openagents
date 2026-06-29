import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import {
  OmniProjectionAudience,
} from './omni-data-classification'

export const WorkroomTemplatePackageState = S.Literals([
  'blocked',
  'draft',
  'org_private_enabled',
  'public_projection_ready',
  'review_recorded',
  'runtime_promotion_requested',
  'validation_recorded',
])
export type WorkroomTemplatePackageState =
  typeof WorkroomTemplatePackageState.Type

export const WorkroomTemplatePackageAuthorityBoundary = S.Literals([
  'package_review_projection_only',
])
export type WorkroomTemplatePackageAuthorityBoundary =
  typeof WorkroomTemplatePackageAuthorityBoundary.Type

export class WorkroomTemplatePackageAuthority extends S.Class<WorkroomTemplatePackageAuthority>(
  'WorkroomTemplatePackageAuthority',
)({
  authorityBoundary: WorkroomTemplatePackageAuthorityBoundary,
  noDeployment: S.Boolean,
  noExternalRunnerLaunch: S.Boolean,
  noMarketplaceListing: S.Boolean,
  noPaymentMutation: S.Boolean,
  noRuntimePromotion: S.Boolean,
}) {}

export class WorkroomTemplatePackageVersion extends S.Class<WorkroomTemplatePackageVersion>(
  'WorkroomTemplatePackageVersion',
)({
  approvalPolicyRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  evidenceRequirementRefs: S.Array(S.String),
  id: S.String,
  outcomeTemplateRefs: S.Array(S.String),
  proofRuleRefs: S.Array(S.String),
  requiredArtifactRefs: S.Array(S.String),
  runnerNeedRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  templateVersionRef: S.String,
  uiBindingRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class WorkroomTemplatePackageRecord extends S.Class<WorkroomTemplatePackageRecord>(
  'WorkroomTemplatePackageRecord',
)({
  approvalPolicyRefs: S.Array(S.String),
  authority: WorkroomTemplatePackageAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  displayName: S.String,
  evidenceRequirementRefs: S.Array(S.String),
  id: S.String,
  operatorDiagnosticRefs: S.Array(S.String),
  orgPrivateEnablementRefs: S.Array(S.String),
  outcomeTemplateRefs: S.Array(S.String),
  packageRef: S.String,
  proofRuleRefs: S.Array(S.String),
  promotionRefs: S.Array(S.String),
  publicProjectionRefs: S.Array(S.String),
  requiredArtifactRefs: S.Array(S.String),
  reviewRefs: S.Array(S.String),
  runnerNeedRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: WorkroomTemplatePackageState,
  templateVersionRefs: S.Array(S.String),
  uiBindingRefs: S.Array(S.String),
  updatedAtIso: S.String,
  validationRefs: S.Array(S.String),
  versionRef: S.String,
}) {}

export class WorkroomTemplatePackageVersionProjection extends S.Class<WorkroomTemplatePackageVersionProjection>(
  'WorkroomTemplatePackageVersionProjection',
)({
  approvalPolicyRefs: S.Array(S.String),
  audience: OmniProjectionAudience,
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  evidenceRequirementRefs: S.Array(S.String),
  id: S.String,
  outcomeTemplateRefs: S.Array(S.String),
  proofRuleRefs: S.Array(S.String),
  requiredArtifactRefs: S.Array(S.String),
  runnerNeedRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  templateVersionRef: S.String,
  uiBindingRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export class WorkroomTemplatePackageProjection extends S.Class<WorkroomTemplatePackageProjection>(
  'WorkroomTemplatePackageProjection',
)({
  approvalPolicyRefs: S.Array(S.String),
  audience: OmniProjectionAudience,
  authority: WorkroomTemplatePackageAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  deploymentAllowed: S.Boolean,
  displayName: S.String,
  evidenceRequirementRefs: S.Array(S.String),
  externalRunnerLaunchAllowed: S.Boolean,
  id: S.String,
  marketplaceListingAllowed: S.Boolean,
  operatorDiagnosticRefs: S.Array(S.String),
  orgPrivateEnablementRecorded: S.Boolean,
  orgPrivateEnablementRefs: S.Array(S.String),
  outcomeTemplateRefs: S.Array(S.String),
  packageRef: S.String,
  paymentMutationAllowed: S.Boolean,
  proofRuleRefs: S.Array(S.String),
  promotionRefs: S.Array(S.String),
  publicProjectionReady: S.Boolean,
  publicProjectionRefs: S.Array(S.String),
  requiredArtifactRefs: S.Array(S.String),
  reviewRecorded: S.Boolean,
  reviewRefs: S.Array(S.String),
  runnerNeedRefs: S.Array(S.String),
  runtimePromotionAllowed: S.Boolean,
  runtimePromotionRequested: S.Boolean,
  sourceRefs: S.Array(S.String),
  state: WorkroomTemplatePackageState,
  stateLabel: S.String,
  templateVersionRefs: S.Array(S.String),
  uiBindingRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  validationRecorded: S.Boolean,
  validationRefs: S.Array(S.String),
  versionRef: S.String,
}) {}

export class WorkroomTemplatePackageUnsafe extends S.TaggedErrorClass<WorkroomTemplatePackageUnsafe>()(
  'WorkroomTemplatePackageUnsafe',
  {
    reason: S.String,
  },
) {}

export const WORKROOM_TEMPLATE_PACKAGE_REVIEW_ONLY_AUTHORITY:
  WorkroomTemplatePackageAuthority = {
    authorityBoundary: 'package_review_projection_only',
    noDeployment: true,
    noExternalRunnerLaunch: true,
    noMarketplaceListing: true,
    noPaymentMutation: true,
    noRuntimePromotion: true,
  }

const stateRank: Readonly<Record<WorkroomTemplatePackageState, number>> = {
  blocked: -1,
  draft: 0,
  org_private_enabled: 3,
  public_projection_ready: 4,
  review_recorded: 2,
  runtime_promotion_requested: 5,
  validation_recorded: 1,
}

const stateLabelByState:
  Readonly<Record<WorkroomTemplatePackageState, string>> = {
    blocked: 'Blocked',
    draft: 'Draft',
    org_private_enabled: 'Org-private enabled',
    public_projection_ready: 'Public projection ready',
    review_recorded: 'Review recorded',
    runtime_promotion_requested: 'Runtime promotion requested',
    validation_recorded: 'Validation recorded',
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeWorkroomTemplatePackageRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|package[_-]?source[_-]?private|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(key|package|repo|source)|provider[_-]?(account|grant|payload|token)|raw[_-]?(document|email|fixture|invoice|package|payment|payload|prompt|provider|runner|run[_-]?log|schema|source|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(operator\.|package_source\.|provider\.|promotion\.runtime|raw\.|source\.private|workroom\.private)/i
const customerUnsafeRefPattern =
  /(operator\.|package_source\.private|provider\.private|source\.private)/i
const teamUnsafeRefPattern =
  /(operator\.|package_source\.private|provider\.private|source\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

export const workroomTemplatePackageStateAtLeast = (
  state: WorkroomTemplatePackageState,
  threshold: WorkroomTemplatePackageState,
): boolean => stateRank[state] >= stateRank[threshold]

export const workroomTemplatePackageAuthorityIsReviewOnly = (
  authority: WorkroomTemplatePackageAuthority,
): boolean =>
  authority.authorityBoundary === 'package_review_projection_only' &&
  authority.noDeployment &&
  authority.noExternalRunnerLaunch &&
  authority.noMarketplaceListing &&
  authority.noPaymentMutation &&
  authority.noRuntimePromotion

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeWorkroomTemplatePackageRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new WorkroomTemplatePackageUnsafe({
      reason: `${label} contains private package source, raw prompts, provider payloads, private repo refs, wallet/payment material, secrets, raw logs, or raw timestamps.`,
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

const versionRefs = (
  version: WorkroomTemplatePackageVersion,
): ReadonlyArray<string> => [
  version.id,
  version.templateVersionRef,
  ...version.approvalPolicyRefs,
  ...version.caveatRefs,
  ...version.evidenceRequirementRefs,
  ...version.outcomeTemplateRefs,
  ...version.proofRuleRefs,
  ...version.requiredArtifactRefs,
  ...version.runnerNeedRefs,
  ...version.sourceRefs,
  ...version.uiBindingRefs,
]

const packageRefs = (
  record: WorkroomTemplatePackageRecord,
): ReadonlyArray<string> => [
  record.id,
  record.packageRef,
  record.versionRef,
  ...record.approvalPolicyRefs,
  ...record.blockerRefs,
  ...record.caveatRefs,
  ...record.evidenceRequirementRefs,
  ...record.operatorDiagnosticRefs,
  ...record.orgPrivateEnablementRefs,
  ...record.outcomeTemplateRefs,
  ...record.proofRuleRefs,
  ...record.promotionRefs,
  ...record.publicProjectionRefs,
  ...record.requiredArtifactRefs,
  ...record.reviewRefs,
  ...record.runnerNeedRefs,
  ...record.sourceRefs,
  ...record.templateVersionRefs,
  ...record.uiBindingRefs,
  ...record.validationRefs,
]

const assertVersionSafe = (
  version: WorkroomTemplatePackageVersion,
): void => {
  assertSafeRefs('Workroom template package version refs', versionRefs(version))
}

const assertPackageSafe = (
  record: WorkroomTemplatePackageRecord,
): void => {
  assertSafeRefs('Workroom template package refs', packageRefs(record))

  if (record.displayName.trim() === '') {
    throw new WorkroomTemplatePackageUnsafe({
      reason: 'Workroom template packages require a display name.',
    })
  }

  if (!workroomTemplatePackageAuthorityIsReviewOnly(record.authority)) {
    throw new WorkroomTemplatePackageUnsafe({
      reason: 'Workroom template packages must remain review/projection-only and cannot carry runtime promotion, marketplace listing, external runner launch, deployment, or payment mutation authority.',
    })
  }

  if (record.state === 'blocked' && record.blockerRefs.length === 0) {
    throw new WorkroomTemplatePackageUnsafe({
      reason: 'Blocked workroom template packages require blocker refs.',
    })
  }

  if (
    workroomTemplatePackageStateAtLeast(record.state, 'validation_recorded') &&
    record.validationRefs.length === 0
  ) {
    throw new WorkroomTemplatePackageUnsafe({
      reason: 'Validation state requires validation refs.',
    })
  }

  if (
    workroomTemplatePackageStateAtLeast(record.state, 'review_recorded') &&
    record.reviewRefs.length === 0
  ) {
    throw new WorkroomTemplatePackageUnsafe({
      reason: 'Review state requires review refs.',
    })
  }

  if (
    workroomTemplatePackageStateAtLeast(record.state, 'org_private_enabled') &&
    record.orgPrivateEnablementRefs.length === 0
  ) {
    throw new WorkroomTemplatePackageUnsafe({
      reason: 'Org-private enablement state requires enablement refs.',
    })
  }

  if (
    workroomTemplatePackageStateAtLeast(
      record.state,
      'public_projection_ready',
    ) &&
    record.publicProjectionRefs.length === 0
  ) {
    throw new WorkroomTemplatePackageUnsafe({
      reason: 'Public projection state requires public projection refs.',
    })
  }

  if (
    workroomTemplatePackageStateAtLeast(
      record.state,
      'runtime_promotion_requested',
    ) &&
    record.promotionRefs.length === 0
  ) {
    throw new WorkroomTemplatePackageUnsafe({
      reason: 'Runtime promotion request state requires promotion refs.',
    })
  }
}

export const projectWorkroomTemplatePackageVersion = (
  version: WorkroomTemplatePackageVersion,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): WorkroomTemplatePackageVersionProjection => {
  assertVersionSafe(version)

  return {
    approvalPolicyRefs: safeRefsForAudience(
      'Workroom template package version approval refs',
      version.approvalPolicyRefs,
      audience,
    ),
    audience,
    caveatRefs: safeRefsForAudience(
      'Workroom template package version caveat refs',
      version.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      version.createdAtIso,
      nowIso,
    ),
    evidenceRequirementRefs: safeRefsForAudience(
      'Workroom template package version evidence refs',
      version.evidenceRequirementRefs,
      audience,
    ),
    id: safeRefForAudience(
      'Workroom template package version id',
      version.id,
      audience,
    ),
    outcomeTemplateRefs: safeRefsForAudience(
      'Workroom template package version outcome refs',
      version.outcomeTemplateRefs,
      audience,
    ),
    proofRuleRefs: safeRefsForAudience(
      'Workroom template package version proof refs',
      version.proofRuleRefs,
      audience,
    ),
    requiredArtifactRefs: safeRefsForAudience(
      'Workroom template package version artifact refs',
      version.requiredArtifactRefs,
      audience,
    ),
    runnerNeedRefs: safeRefsForAudience(
      'Workroom template package version runner refs',
      version.runnerNeedRefs,
      audience,
    ),
    sourceRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Workroom template package version source refs',
        version.sourceRefs,
        audience,
      ),
    templateVersionRef: safeRefForAudience(
      'Workroom template package template version ref',
      version.templateVersionRef,
      audience,
    ),
    uiBindingRefs: safeRefsForAudience(
      'Workroom template package version UI refs',
      version.uiBindingRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      version.updatedAtIso,
      nowIso,
    ),
  }
}

export const projectWorkroomTemplatePackage = (
  record: WorkroomTemplatePackageRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): WorkroomTemplatePackageProjection => {
  assertPackageSafe(record)

  return {
    approvalPolicyRefs: safeRefsForAudience(
      'Workroom template package approval refs',
      record.approvalPolicyRefs,
      audience,
    ),
    audience,
    authority: record.authority,
    blockerRefs: safeRefsForAudience(
      'Workroom template package blocker refs',
      record.blockerRefs,
      audience,
    ),
    caveatRefs: safeRefsForAudience(
      'Workroom template package caveat refs',
      record.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    deploymentAllowed: false,
    displayName: record.displayName.trim(),
    evidenceRequirementRefs: safeRefsForAudience(
      'Workroom template package evidence refs',
      record.evidenceRequirementRefs,
      audience,
    ),
    externalRunnerLaunchAllowed: false,
    id: safeRefForAudience('Workroom template package id', record.id, audience),
    marketplaceListingAllowed: false,
    operatorDiagnosticRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'Workroom template package operator diagnostic refs',
        record.operatorDiagnosticRefs,
        audience,
      )
      : [],
    orgPrivateEnablementRecorded:
      workroomTemplatePackageStateAtLeast(
        record.state,
        'org_private_enabled',
      ) && record.orgPrivateEnablementRefs.length > 0,
    orgPrivateEnablementRefs:
      audience === 'operator' || audience === 'private'
        ? safeRefsForAudience(
          'Workroom template package org private enablement refs',
          record.orgPrivateEnablementRefs,
          audience,
        )
        : [],
    outcomeTemplateRefs: safeRefsForAudience(
      'Workroom template package outcome refs',
      record.outcomeTemplateRefs,
      audience,
    ),
    packageRef: safeRefForAudience(
      'Workroom template package ref',
      record.packageRef,
      audience,
    ),
    paymentMutationAllowed: false,
    proofRuleRefs: safeRefsForAudience(
      'Workroom template package proof refs',
      record.proofRuleRefs,
      audience,
    ),
    promotionRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'Workroom template package promotion refs',
        record.promotionRefs,
        audience,
      )
      : [],
    publicProjectionReady:
      workroomTemplatePackageStateAtLeast(
        record.state,
        'public_projection_ready',
      ) && record.publicProjectionRefs.length > 0,
    publicProjectionRefs: safeRefsForAudience(
      'Workroom template package public projection refs',
      record.publicProjectionRefs,
      audience,
    ),
    requiredArtifactRefs: safeRefsForAudience(
      'Workroom template package artifact refs',
      record.requiredArtifactRefs,
      audience,
    ),
    reviewRecorded:
      workroomTemplatePackageStateAtLeast(record.state, 'review_recorded') &&
      record.reviewRefs.length > 0,
    reviewRefs: audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'Workroom template package review refs',
        record.reviewRefs,
        audience,
      )
      : [],
    runnerNeedRefs: safeRefsForAudience(
      'Workroom template package runner refs',
      record.runnerNeedRefs,
      audience,
    ),
    runtimePromotionAllowed: false,
    runtimePromotionRequested:
      workroomTemplatePackageStateAtLeast(
        record.state,
        'runtime_promotion_requested',
      ) && record.promotionRefs.length > 0,
    sourceRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Workroom template package source refs',
        record.sourceRefs,
        audience,
      ),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    templateVersionRefs: safeRefsForAudience(
      'Workroom template package version refs',
      record.templateVersionRefs,
      audience,
    ),
    uiBindingRefs: safeRefsForAudience(
      'Workroom template package UI refs',
      record.uiBindingRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    validationRecorded:
      workroomTemplatePackageStateAtLeast(
        record.state,
        'validation_recorded',
      ) && record.validationRefs.length > 0,
    validationRefs: safeRefsForAudience(
      'Workroom template package validation refs',
      record.validationRefs,
      audience,
    ),
    versionRef: safeRefForAudience(
      'Workroom template package version ref',
      record.versionRef,
      audience,
    ),
  }
}

const projectionText = (
  projection:
    | WorkroomTemplatePackageProjection
    | WorkroomTemplatePackageVersionProjection,
): string =>
  'packageRef' in projection
    ? [
      projection.id,
      projection.packageRef,
      projection.versionRef,
      ...projection.approvalPolicyRefs,
      ...projection.blockerRefs,
      ...projection.caveatRefs,
      ...projection.evidenceRequirementRefs,
      ...projection.operatorDiagnosticRefs,
      ...projection.orgPrivateEnablementRefs,
      ...projection.outcomeTemplateRefs,
      ...projection.proofRuleRefs,
      ...projection.promotionRefs,
      ...projection.publicProjectionRefs,
      ...projection.requiredArtifactRefs,
      ...projection.reviewRefs,
      ...projection.runnerNeedRefs,
      ...projection.sourceRefs,
      ...projection.templateVersionRefs,
      ...projection.uiBindingRefs,
      ...projection.validationRefs,
    ].join(' ')
    : [
      projection.id,
      projection.templateVersionRef,
      ...projection.approvalPolicyRefs,
      ...projection.caveatRefs,
      ...projection.evidenceRequirementRefs,
      ...projection.outcomeTemplateRefs,
      ...projection.proofRuleRefs,
      ...projection.requiredArtifactRefs,
      ...projection.runnerNeedRefs,
      ...projection.sourceRefs,
      ...projection.uiBindingRefs,
    ].join(' ')

export const workroomTemplatePackageProjectionHasPrivateMaterial = (
  projection:
    | WorkroomTemplatePackageProjection
    | WorkroomTemplatePackageVersionProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeWorkroomTemplatePackageRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE:
  WorkroomTemplatePackageVersion = {
    approvalPolicyRefs: ['approval_policy.template_package.operator_review'],
    caveatRefs: ['caveat.template_package.review_required'],
    createdAtIso: '2026-06-07T09:00:00.000Z',
    evidenceRequirementRefs: ['evidence_requirement.template_package.fixture'],
    id: 'workroom_template_package_version.site_builder.v1',
    outcomeTemplateRefs: ['outcome_template.sites.reviewable_site'],
    proofRuleRefs: ['proof_rule.template_package.public_safe'],
    requiredArtifactRefs: ['artifact_requirement.template_package.schema'],
    runnerNeedRefs: ['runner_need.template_package.none_until_promoted'],
    sourceRefs: ['source.template_package.public_repo'],
    templateVersionRef: 'template_version.site_builder.v1',
    uiBindingRefs: ['ui_binding.template_package.json_render_card'],
    updatedAtIso: '2026-06-07T09:05:00.000Z',
  }

export const WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE:
  WorkroomTemplatePackageRecord = {
    approvalPolicyRefs: ['approval_policy.template_package.operator_review'],
    authority: WORKROOM_TEMPLATE_PACKAGE_REVIEW_ONLY_AUTHORITY,
    blockerRefs: [],
    caveatRefs: ['caveat.template_package.no_runtime_promotion'],
    createdAtIso: '2026-06-07T09:10:00.000Z',
    displayName: 'Site Builder Workroom Template Package',
    evidenceRequirementRefs: ['evidence_requirement.template_package.fixture'],
    id: 'workroom_template_package.site_builder',
    operatorDiagnosticRefs: ['operator.template_package.review_trace'],
    orgPrivateEnablementRefs: ['enablement.org_private.openagents_core'],
    outcomeTemplateRefs: ['outcome_template.sites.reviewable_site'],
    packageRef: 'package.workroom_template.site_builder',
    proofRuleRefs: ['proof_rule.template_package.public_safe'],
    promotionRefs: ['promotion.request.runtime.site_builder'],
    publicProjectionRefs: ['public_projection.template_package.site_builder'],
    requiredArtifactRefs: ['artifact_requirement.template_package.schema'],
    reviewRefs: ['review.template_package.operator_approved'],
    runnerNeedRefs: ['runner_need.template_package.none_until_promoted'],
    sourceRefs: ['source.template_package.public_repo'],
    state: 'runtime_promotion_requested',
    templateVersionRefs: ['template_version.site_builder.v1'],
    uiBindingRefs: ['ui_binding.template_package.json_render_card'],
    updatedAtIso: '2026-06-07T09:35:00.000Z',
    validationRefs: ['validation.template_package.passed'],
    versionRef: 'version.workroom_template_package.site_builder.v1',
  }
