import type {
  BlueprintSignatureContributionAuthority,
  BlueprintSignatureContributionDraft,
  BlueprintSignatureContributionProjection,
} from '../schemas/signature-contribution'

export const BLUEPRINT_SIGNATURE_CONTRIBUTION_NO_AUTHORITY: BlueprintSignatureContributionAuthority =
  {
    canChangePublicClaims: false,
    canDeploy: false,
    canExecute: false,
    canMutate: false,
    canSendEmail: false,
    canSpend: false,
    deniedEffectRefs: [
      'effect.execute',
      'effect.mutate',
      'effect.deploy',
      'effect.spend',
      'effect.send_email',
      'effect.change_public_claims',
    ],
  }

const privateContributionRefPattern =
  /(bearer\s+|cookie|customer[_-]?email|customer[_-]?name|email[_-]?body|mnemonic|oauth|oa_agent_|openagents_admin|password|preimage|private[_-]?key|provider[_-]?payload|provider[_-]?token|raw[_-]?email|raw[_-]?runner|raw[_-]?run[_-]?log|runner[_-]?log|secret|sk-[a-z0-9]|token|wallet[_-]?secret|\S+@\S+)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const safeRef = (ref: string | null): string | null =>
  ref !== null &&
  ref.trim() !== '' &&
  !privateContributionRefPattern.test(ref) &&
  !isoTimestampPattern.test(ref)
    ? ref
    : null

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(ref => safeRef(ref) !== null)

export const blueprintSignatureContributionDraftHasRuntimeAuthority = (
  draft: BlueprintSignatureContributionDraft,
): boolean =>
  draft.authority.canChangePublicClaims ||
  draft.authority.canDeploy ||
  draft.authority.canExecute ||
  draft.authority.canMutate ||
  draft.authority.canSendEmail ||
  draft.authority.canSpend

export const blueprintSignatureContributionDraftCanEnterReleaseGate = (
  draft: BlueprintSignatureContributionDraft,
): boolean =>
  !blueprintSignatureContributionDraftHasRuntimeAuthority(draft) &&
  draft.status === 'approved_for_release_gate' &&
  draft.reviewStatus === 'approved' &&
  draft.rejectionRef === null &&
  draft.promotionRef === null &&
  draft.requiredFixtureRefs.length > 0 &&
  draft.releaseGateRefs.length > 0 &&
  (draft.proposedProgramSignatureRef !== null ||
    draft.proposedModuleVersionRef !== null)

export const blueprintSignatureContributionDraftRuntimeEffectDeniedRefs = (
  draft: BlueprintSignatureContributionDraft,
): ReadonlyArray<string> =>
  draft.authority.deniedEffectRefs.length > 0
    ? draft.authority.deniedEffectRefs
    : BLUEPRINT_SIGNATURE_CONTRIBUTION_NO_AUTHORITY.deniedEffectRefs

export const blueprintSignatureContributionDraftBlockerRefs = (
  draft: BlueprintSignatureContributionDraft,
): ReadonlyArray<string> => {
  const blockers: string[] = []

  if (blueprintSignatureContributionDraftHasRuntimeAuthority(draft)) {
    blockers.push('blocker.signature_contribution.runtime_authority_present')
  }

  if (draft.reviewStatus !== 'approved') {
    blockers.push('blocker.signature_contribution.review_not_approved')
  }

  if (draft.status !== 'approved_for_release_gate') {
    blockers.push('blocker.signature_contribution.not_release_gate_ready')
  }

  if (draft.requiredFixtureRefs.length === 0) {
    blockers.push('blocker.signature_contribution.fixture_refs_missing')
  }

  if (draft.releaseGateRefs.length === 0) {
    blockers.push('blocker.signature_contribution.release_gate_refs_missing')
  }

  if (
    draft.proposedProgramSignatureRef === null &&
    draft.proposedModuleVersionRef === null
  ) {
    blockers.push('blocker.signature_contribution.target_ref_missing')
  }

  if (draft.rejectionRef !== null) {
    blockers.push('blocker.signature_contribution.rejected')
  }

  if (draft.promotionRef !== null) {
    blockers.push('blocker.signature_contribution.already_promoted')
  }

  return [...new Set(blockers)]
}

export const projectBlueprintSignatureContributionDraft = (
  draft: BlueprintSignatureContributionDraft,
): BlueprintSignatureContributionProjection => ({
  authority: {
    ...draft.authority,
    deniedEffectRefs: safeRefs(
      blueprintSignatureContributionDraftRuntimeEffectDeniedRefs(draft),
    ),
  },
  capabilitySummaryRef:
    safeRef(draft.capabilitySummaryRef) ??
    'signature_contribution.capability.redacted',
  contributorRefs: safeRefs(draft.contributorRefs),
  id: safeRef(draft.id) ?? 'signature_contribution.redacted',
  intendedFamily: draft.intendedFamily,
  nonAuthoritative:
    !blueprintSignatureContributionDraftHasRuntimeAuthority(draft),
  promotionRef: safeRef(draft.promotionRef),
  proposedModuleVersionRef: safeRef(draft.proposedModuleVersionRef),
  proposedProgramSignatureRef: safeRef(draft.proposedProgramSignatureRef),
  proposedProgramTypeRef: safeRef(draft.proposedProgramTypeRef),
  rejectionRef: safeRef(draft.rejectionRef),
  releaseGateRefs: safeRefs(draft.releaseGateRefs),
  requiredFixtureRefs: safeRefs(draft.requiredFixtureRefs),
  reviewStatus: draft.reviewStatus,
  riskClass: draft.riskClass,
  sourceRefs: safeRefs(draft.sourceRefs),
  status: draft.status,
})

export const blueprintSignatureContributionProjectionHasPrivateMaterial = (
  projection: BlueprintSignatureContributionProjection,
): boolean =>
  privateContributionRefPattern.test(JSON.stringify(projection)) ||
  isoTimestampPattern.test(JSON.stringify(projection))
