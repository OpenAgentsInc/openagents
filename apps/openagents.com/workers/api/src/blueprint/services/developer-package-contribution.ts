import type {
  BlueprintDeveloperPackageContributionAuthority,
  BlueprintDeveloperPackageContributionProjection,
  BlueprintDeveloperPackageContributionRecord,
} from '../schemas/developer-package-contribution'

export const BLUEPRINT_DEVELOPER_PACKAGE_CONTRIBUTION_NO_AUTHORITY: BlueprintDeveloperPackageContributionAuthority =
  {
    canChangePublicClaims: false,
    canCreateSite: false,
    canDeploy: false,
    canDispatchRuntime: false,
    canExecute: false,
    canMutateRepository: false,
    canPostPublicly: false,
    canSendEmail: false,
    canSpend: false,
    deniedEffectRefs: [
      'effect.execute',
      'effect.dispatch_runtime',
      'effect.deploy',
      'effect.spend',
      'effect.send_email',
      'effect.mutate_repository',
      'effect.post_publicly',
      'effect.create_site',
      'effect.change_public_claims',
    ],
  }

const privatePackageContributionRefPattern =
  /(bearer\s+|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|oa_agent_|openagents_admin|password|payment[_-]?(hash|id|preimage)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|provider[_-]?(grant|payload|token)|raw[_-]?(email|payload|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet|\S+@\S+)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const safeRef = (ref: string | null): string | null =>
  ref !== null &&
  ref.trim() !== '' &&
  !privatePackageContributionRefPattern.test(ref) &&
  !isoTimestampPattern.test(ref)
    ? ref
    : null

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  uniqueRefs(refs).filter(ref => safeRef(ref) !== null)

const contributionTargetRefs = (
  contribution: BlueprintDeveloperPackageContributionRecord,
): ReadonlyArray<string> => [
  ...contribution.backendProjectionAdapterRefs,
  ...contribution.proposedProgramTypeRefs,
  ...contribution.proposedProgramSignatureRefs,
  ...contribution.proposedModuleVersionRefs,
  ...contribution.contextPackageRefs,
  ...contribution.outcomeTemplateRefs,
  ...contribution.toolPackageRefs,
  ...contribution.uiBindingRefs,
]

export const blueprintDeveloperPackageContributionHasRuntimeAuthority = (
  contribution: BlueprintDeveloperPackageContributionRecord,
): boolean =>
  contribution.authority.canChangePublicClaims ||
  contribution.authority.canCreateSite ||
  contribution.authority.canDeploy ||
  contribution.authority.canDispatchRuntime ||
  contribution.authority.canExecute ||
  contribution.authority.canMutateRepository ||
  contribution.authority.canPostPublicly ||
  contribution.authority.canSendEmail ||
  contribution.authority.canSpend

export const blueprintDeveloperPackageContributionRuntimeEffectDeniedRefs = (
  contribution: BlueprintDeveloperPackageContributionRecord,
): ReadonlyArray<string> =>
  contribution.authority.deniedEffectRefs.length > 0
    ? contribution.authority.deniedEffectRefs
    : BLUEPRINT_DEVELOPER_PACKAGE_CONTRIBUTION_NO_AUTHORITY.deniedEffectRefs

export const blueprintDeveloperPackageContributionCanEnterReleaseGate = (
  contribution: BlueprintDeveloperPackageContributionRecord,
): boolean =>
  !blueprintDeveloperPackageContributionHasRuntimeAuthority(contribution) &&
  !contribution.selfPromotionAttempt &&
  contribution.noProductionRuntimeAuthority &&
  contribution.status === 'approved_for_release_gate' &&
  contribution.reviewStatus === 'approved' &&
  contribution.rejectionRef === null &&
  contribution.promotionRef === null &&
  contribution.requiredFixtureRefs.length > 0 &&
  contribution.releaseGateRefs.length > 0 &&
  contributionTargetRefs(contribution).length > 0

export const blueprintDeveloperPackageContributionBlockerRefs = (
  contribution: BlueprintDeveloperPackageContributionRecord,
): ReadonlyArray<string> =>
  uniqueRefs([
    ...(blueprintDeveloperPackageContributionHasRuntimeAuthority(contribution)
      ? ['blocker.developer_package_contribution.runtime_authority_present']
      : []),
    ...(contribution.selfPromotionAttempt
      ? ['blocker.developer_package_contribution.self_promotion_attempt']
      : []),
    ...(!contribution.noProductionRuntimeAuthority
      ? [
          'blocker.developer_package_contribution.production_runtime_authority_present',
        ]
      : []),
    ...(contribution.reviewStatus !== 'approved'
      ? ['blocker.developer_package_contribution.review_not_approved']
      : []),
    ...(contribution.status !== 'approved_for_release_gate'
      ? ['blocker.developer_package_contribution.not_release_gate_ready']
      : []),
    ...(contribution.requiredFixtureRefs.length === 0
      ? ['blocker.developer_package_contribution.fixture_refs_missing']
      : []),
    ...(contribution.releaseGateRefs.length === 0
      ? ['blocker.developer_package_contribution.release_gate_refs_missing']
      : []),
    ...(contributionTargetRefs(contribution).length === 0
      ? ['blocker.developer_package_contribution.target_ref_missing']
      : []),
    ...(contribution.rejectionRef !== null
      ? ['blocker.developer_package_contribution.rejected']
      : []),
    ...(contribution.promotionRef !== null
      ? ['blocker.developer_package_contribution.already_promoted']
      : []),
  ])

export const projectBlueprintDeveloperPackageContribution = (
  contribution: BlueprintDeveloperPackageContributionRecord,
): BlueprintDeveloperPackageContributionProjection => ({
  authority: {
    ...contribution.authority,
    deniedEffectRefs: safeRefs(
      blueprintDeveloperPackageContributionRuntimeEffectDeniedRefs(
        contribution,
      ),
    ),
  },
  backendProjectionAdapterRefs: safeRefs(
    contribution.backendProjectionAdapterRefs,
  ),
  capabilityFamily: contribution.capabilityFamily,
  capabilitySummaryRef:
    safeRef(contribution.capabilitySummaryRef) ??
    'developer_package_contribution.capability.redacted',
  contextPackageRefs: safeRefs(contribution.contextPackageRefs),
  contributorRefs: safeRefs(contribution.contributorRefs),
  dogfoodScopeRef: safeRef(contribution.dogfoodScopeRef),
  id: safeRef(contribution.id) ?? 'developer_package_contribution.redacted',
  intendedProgramFamily: contribution.intendedProgramFamily,
  nonAuthoritative:
    !blueprintDeveloperPackageContributionHasRuntimeAuthority(contribution),
  noProductionRuntimeAuthority: contribution.noProductionRuntimeAuthority,
  outcomeTemplateRefs: safeRefs(contribution.outcomeTemplateRefs),
  paymentAttributionRefs: safeRefs(contribution.paymentAttributionRefs),
  promotionRef: safeRef(contribution.promotionRef),
  proposedModuleVersionRefs: safeRefs(contribution.proposedModuleVersionRefs),
  proposedProgramSignatureRefs: safeRefs(
    contribution.proposedProgramSignatureRefs,
  ),
  proposedProgramTypeRefs: safeRefs(contribution.proposedProgramTypeRefs),
  rejectionRef: safeRef(contribution.rejectionRef),
  releaseGateReady:
    blueprintDeveloperPackageContributionCanEnterReleaseGate(contribution),
  releaseGateRefs: safeRefs(contribution.releaseGateRefs),
  requiredFixtureRefs: safeRefs(contribution.requiredFixtureRefs),
  retainedFailureRefs: safeRefs(contribution.retainedFailureRefs),
  reviewStatus: contribution.reviewStatus,
  riskClass: contribution.riskClass,
  selfPromotionAttempt: contribution.selfPromotionAttempt,
  sourceRefs: safeRefs(contribution.sourceRefs),
  status: contribution.status,
  toolPackageRefs: safeRefs(contribution.toolPackageRefs),
  uiBindingRefs: safeRefs(contribution.uiBindingRefs),
})

export const blueprintDeveloperPackageContributionProjectionHasPrivateMaterial =
  (projection: BlueprintDeveloperPackageContributionProjection): boolean =>
    privatePackageContributionRefPattern.test(JSON.stringify(projection)) ||
    isoTimestampPattern.test(JSON.stringify(projection))
