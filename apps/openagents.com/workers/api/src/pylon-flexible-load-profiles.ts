import { Schema as S } from 'effect'

import type { BlueprintMissionBriefingAudience } from './blueprint/schemas/continuation-mission-briefing'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const PylonFlexibleLoadWorkKind = S.Literals([
  'autopilot_sites',
  'coding_autopilot',
  'forum',
  'omni',
  'pylon_provider',
  'research',
])
export type PylonFlexibleLoadWorkKind = typeof PylonFlexibleLoadWorkKind.Type

export const PylonFlexibleLoadFlexibilityClass = S.Literals([
  'deferrable',
  'fixed',
  'interruptible',
  'opportunistic',
  'preemptible',
])
export type PylonFlexibleLoadFlexibilityClass =
  typeof PylonFlexibleLoadFlexibilityClass.Type

export const PylonFlexibleLoadInterruptionTolerance = S.Literals([
  'checkpoint_required',
  'graceful_stop',
  'none',
  'preemptible',
])
export type PylonFlexibleLoadInterruptionTolerance =
  typeof PylonFlexibleLoadInterruptionTolerance.Type

export const PylonFlexibleLoadCheckpointCadence = S.Literals([
  'continuous',
  'manual',
  'none',
  'per_step',
  'periodic',
])
export type PylonFlexibleLoadCheckpointCadence =
  typeof PylonFlexibleLoadCheckpointCadence.Type

export const PylonFlexibleLoadResumeRequirement = S.Literals([
  'any_eligible_provider',
  'not_resumable',
  'same_provider',
  'same_trust_tier',
])
export type PylonFlexibleLoadResumeRequirement =
  typeof PylonFlexibleLoadResumeRequirement.Type

export const PylonFlexibleLoadDeadlineWindow = S.Literals([
  'days',
  'hours',
  'immediate',
  'minutes',
  'overnight',
])
export type PylonFlexibleLoadDeadlineWindow =
  typeof PylonFlexibleLoadDeadlineWindow.Type

export const PylonFlexibleLoadReplayCost = S.Literals([
  'high',
  'low',
  'medium',
  'prohibitive',
])
export type PylonFlexibleLoadReplayCost =
  typeof PylonFlexibleLoadReplayCost.Type

export const PylonFlexibleLoadPowerEventEligibility = S.Literals([
  'eligible_measured',
  'eligible_modeled',
  'not_eligible',
  'operator_review',
])
export type PylonFlexibleLoadPowerEventEligibility =
  typeof PylonFlexibleLoadPowerEventEligibility.Type

export const PylonFlexibleLoadProfileAuthorityBoundary = S.Literals([
  'read_only_work_class_profile',
])
export type PylonFlexibleLoadProfileAuthorityBoundary =
  typeof PylonFlexibleLoadProfileAuthorityBoundary.Type

export class PylonFlexibleLoadProfileAuthority extends S.Class<PylonFlexibleLoadProfileAuthority>(
  'PylonFlexibleLoadProfileAuthority',
)({
  authorityBoundary: PylonFlexibleLoadProfileAuthorityBoundary,
  noCapacityAssignmentMutation: S.Boolean,
  noPowerEventDispatch: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noRunnerLaunch: S.Boolean,
  noSettlementMutation: S.Boolean,
  noWorkClassMutation: S.Boolean,
}) {}

export class PylonFlexibleLoadProfileRecord extends S.Class<PylonFlexibleLoadProfileRecord>(
  'PylonFlexibleLoadProfileRecord',
)({
  acceptedOutcomeRefs: S.Array(S.String),
  authority: PylonFlexibleLoadProfileAuthority,
  caveatRefs: S.Array(S.String),
  checkpointCadence: PylonFlexibleLoadCheckpointCadence,
  checkpointPolicyRefs: S.Array(S.String),
  codingWorkClassRefs: S.Array(S.String),
  deadlineWindow: PylonFlexibleLoadDeadlineWindow,
  evidenceRefs: S.Array(S.String),
  flexibilityClass: PylonFlexibleLoadFlexibilityClass,
  id: S.String,
  interruptionTolerance: PylonFlexibleLoadInterruptionTolerance,
  measuredResponseRefs: S.Array(S.String),
  modeledSuitabilityRefs: S.Array(S.String),
  omniWorkClassRefs: S.Array(S.String),
  powerEventEligibility: PylonFlexibleLoadPowerEventEligibility,
  profileRef: S.String,
  replayCost: PylonFlexibleLoadReplayCost,
  resumePolicyRefs: S.Array(S.String),
  resumeRequirement: PylonFlexibleLoadResumeRequirement,
  revenueRefs: S.Array(S.String),
  settlementRefs: S.Array(S.String),
  siteWorkClassRefs: S.Array(S.String),
  updatedAtIso: S.String,
  verificationAfterResumeRequired: S.Boolean,
  verificationPolicyRefs: S.Array(S.String),
  workClassRef: S.String,
  workKind: PylonFlexibleLoadWorkKind,
}) {}

export class PylonFlexibleLoadProfileProjection extends S.Class<PylonFlexibleLoadProfileProjection>(
  'PylonFlexibleLoadProfileProjection',
)({
  acceptedOutcomeClaimAllowed: S.Boolean,
  acceptedOutcomeRefs: S.Array(S.String),
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  authority: PylonFlexibleLoadProfileAuthority,
  capacityAssignmentMutationAllowed: S.Boolean,
  caveatRefs: S.Array(S.String),
  checkpointCadence: PylonFlexibleLoadCheckpointCadence,
  checkpointCadenceLabel: S.String,
  checkpointPolicyRefs: S.Array(S.String),
  codingWorkClassRefs: S.Array(S.String),
  deadlineWindow: PylonFlexibleLoadDeadlineWindow,
  deadlineWindowLabel: S.String,
  evidenceRefs: S.Array(S.String),
  flexibilityClass: PylonFlexibleLoadFlexibilityClass,
  flexibilityClassLabel: S.String,
  id: S.String,
  interruptionTolerance: PylonFlexibleLoadInterruptionTolerance,
  interruptionToleranceLabel: S.String,
  measuredResponseRefs: S.Array(S.String),
  measuredSuitabilityClaimAllowed: S.Boolean,
  modeledSuitabilityClaimAllowed: S.Boolean,
  modeledSuitabilityRefs: S.Array(S.String),
  omniWorkClassRefs: S.Array(S.String),
  powerEventEligibility: PylonFlexibleLoadPowerEventEligibility,
  powerEventEligibilityLabel: S.String,
  powerEventDispatchAllowed: S.Boolean,
  profileRef: S.String,
  replayCost: PylonFlexibleLoadReplayCost,
  replayCostLabel: S.String,
  resumePolicyRefs: S.Array(S.String),
  resumeRequirement: PylonFlexibleLoadResumeRequirement,
  resumeRequirementLabel: S.String,
  revenueClaimAllowed: S.Boolean,
  revenueRefs: S.Array(S.String),
  settlementClaimAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  settlementRefs: S.Array(S.String),
  siteWorkClassRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  verificationAfterResumeRequired: S.Boolean,
  verificationPolicyRefs: S.Array(S.String),
  workClassRef: S.String,
  workClassMutationAllowed: S.Boolean,
  workKind: PylonFlexibleLoadWorkKind,
  workKindLabel: S.String,
}) {}

export class PylonFlexibleLoadProfileUnsafe extends S.TaggedErrorClass<PylonFlexibleLoadProfileUnsafe>()(
  'PylonFlexibleLoadProfileUnsafe',
  {
    reason: S.String,
  },
) {}

const workKindLabelByKind: Record<PylonFlexibleLoadWorkKind, string> = {
  autopilot_sites: 'Autopilot Sites',
  coding_autopilot: 'Coding Autopilot',
  forum: 'Forum',
  omni: 'Omni',
  pylon_provider: 'Pylon provider',
  research: 'Research',
}

const flexibilityClassLabelByValue: Record<
  PylonFlexibleLoadFlexibilityClass,
  string
> = {
  deferrable: 'Deferrable',
  fixed: 'Fixed',
  interruptible: 'Interruptible',
  opportunistic: 'Opportunistic',
  preemptible: 'Preemptible',
}

const interruptionToleranceLabelByValue: Record<
  PylonFlexibleLoadInterruptionTolerance,
  string
> = {
  checkpoint_required: 'Checkpoint required',
  graceful_stop: 'Graceful stop',
  none: 'None',
  preemptible: 'Preemptible',
}

const checkpointCadenceLabelByValue: Record<
  PylonFlexibleLoadCheckpointCadence,
  string
> = {
  continuous: 'Continuous',
  manual: 'Manual',
  none: 'None',
  per_step: 'Per step',
  periodic: 'Periodic',
}

const resumeRequirementLabelByValue: Record<
  PylonFlexibleLoadResumeRequirement,
  string
> = {
  any_eligible_provider: 'Any eligible provider',
  not_resumable: 'Not resumable',
  same_provider: 'Same provider',
  same_trust_tier: 'Same trust tier',
}

const deadlineWindowLabelByValue: Record<
  PylonFlexibleLoadDeadlineWindow,
  string
> = {
  days: 'Days',
  hours: 'Hours',
  immediate: 'Immediate',
  minutes: 'Minutes',
  overnight: 'Overnight',
}

const replayCostLabelByValue: Record<PylonFlexibleLoadReplayCost, string> = {
  high: 'High',
  low: 'Low',
  medium: 'Medium',
  prohibitive: 'Prohibitive',
}

const powerEventEligibilityLabelByValue: Record<
  PylonFlexibleLoadPowerEventEligibility,
  string
> = {
  eligible_measured: 'Eligible with measured evidence',
  eligible_modeled: 'Eligible by model',
  not_eligible: 'Not eligible',
  operator_review: 'Operator review',
}

export const PYLON_FLEXIBLE_LOAD_PROFILE_READ_ONLY_AUTHORITY:
  PylonFlexibleLoadProfileAuthority = {
    authorityBoundary: 'read_only_work_class_profile',
    noCapacityAssignmentMutation: true,
    noPowerEventDispatch: true,
    noPublicClaimUpgrade: true,
    noRunnerLaunch: true,
    noSettlementMutation: true,
    noWorkClassMutation: true,
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const universallyUnsafeRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hardware[_-]?telemetry|hostname|invoice|lnbc|lntb|lnbcrt|lno1|mac[_-]?address|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage)|payout[_-]?(address|destination|target)|preimage|private[_-]?(hardware|key)|provider[_-]?(account|grant|payload|telemetry|token)|raw[_-]?(host|invoice|payment|payload|power|prompt|runner|run[_-]?log|source[_-]?archive|telemetry|webhook)|runner[_-]?log|secret|serial[_-]?number|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(accepted\.private|measured\.private|provider\.private|revenue\.private|settlement\.private)/i
const customerUnsafeRefPattern =
  /(accepted\.private|measured\.private|provider\.private|revenue\.private|settlement\.private)/i
const teamUnsafeRefPattern =
  /(provider\.private|settlement\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    universallyUnsafeRefPattern.test(ref) ||
    isoTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PylonFlexibleLoadProfileUnsafe({
      reason: `${label} contains private hardware, provider telemetry, runner logs, wallet, payment, payout target, customer, private repo, secret, raw telemetry, or raw timestamp material.`,
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
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const hasRefs = (refs: ReadonlyArray<string>): boolean =>
  refs.length > 0

const assertRecordSafe = (
  record: PylonFlexibleLoadProfileRecord,
): void => {
  assertSafeRefs('flexible-load identity refs', [
    record.id,
    record.profileRef,
    record.workClassRef,
  ])
  assertSafeRefs('flexible-load coding work refs', record.codingWorkClassRefs)
  assertSafeRefs('flexible-load site work refs', record.siteWorkClassRefs)
  assertSafeRefs('flexible-load omni work refs', record.omniWorkClassRefs)
  assertSafeRefs(
    'flexible-load checkpoint policy refs',
    record.checkpointPolicyRefs,
  )
  assertSafeRefs('flexible-load resume policy refs', record.resumePolicyRefs)
  assertSafeRefs(
    'flexible-load verification policy refs',
    record.verificationPolicyRefs,
  )
  assertSafeRefs(
    'flexible-load modeled suitability refs',
    record.modeledSuitabilityRefs,
  )
  assertSafeRefs(
    'flexible-load measured response refs',
    record.measuredResponseRefs,
  )
  assertSafeRefs(
    'flexible-load accepted outcome refs',
    record.acceptedOutcomeRefs,
  )
  assertSafeRefs('flexible-load revenue refs', record.revenueRefs)
  assertSafeRefs('flexible-load settlement refs', record.settlementRefs)
  assertSafeRefs('flexible-load caveat refs', record.caveatRefs)
  assertSafeRefs('flexible-load evidence refs', record.evidenceRefs)

  if (
    record.authority.noCapacityAssignmentMutation !== true ||
    record.authority.noPowerEventDispatch !== true ||
    record.authority.noPublicClaimUpgrade !== true ||
    record.authority.noRunnerLaunch !== true ||
    record.authority.noSettlementMutation !== true ||
    record.authority.noWorkClassMutation !== true
  ) {
    throw new PylonFlexibleLoadProfileUnsafe({
      reason:
        'Flexible-load profiles must remain read-only and cannot assign capacity, dispatch power events, launch runners, mutate settlements, mutate work classes, or upgrade public claims.',
    })
  }

  if (
    record.flexibilityClass === 'fixed' &&
    (record.interruptionTolerance !== 'none' ||
      record.checkpointCadence !== 'none' ||
      record.powerEventEligibility !== 'not_eligible')
  ) {
    throw new PylonFlexibleLoadProfileUnsafe({
      reason:
        'Fixed work-class profiles cannot claim interruption, checkpoint, or power-event eligibility.',
    })
  }

  if (
    (record.flexibilityClass === 'interruptible' ||
      record.flexibilityClass === 'preemptible') &&
    record.interruptionTolerance === 'none'
  ) {
    throw new PylonFlexibleLoadProfileUnsafe({
      reason:
        'Interruptible and preemptible work-class profiles require interruption tolerance.',
    })
  }

  if (
    record.flexibilityClass === 'deferrable' &&
    record.deadlineWindow === 'immediate'
  ) {
    throw new PylonFlexibleLoadProfileUnsafe({
      reason: 'Deferrable work-class profiles cannot use an immediate deadline.',
    })
  }

  if (
    record.interruptionTolerance === 'checkpoint_required' &&
    record.checkpointCadence === 'none'
  ) {
    throw new PylonFlexibleLoadProfileUnsafe({
      reason: 'Checkpoint-required flexible-load profiles require a non-none checkpoint cadence.',
    })
  }

  if (
    record.checkpointCadence !== 'none' &&
    !hasRefs(record.checkpointPolicyRefs)
  ) {
    throw new PylonFlexibleLoadProfileUnsafe({
      reason: 'Non-none checkpoint cadence requires checkpoint policy refs.',
    })
  }

  if (
    record.resumeRequirement !== 'not_resumable' &&
    !hasRefs(record.resumePolicyRefs)
  ) {
    throw new PylonFlexibleLoadProfileUnsafe({
      reason: 'Resumable flexible-load profiles require resume policy refs.',
    })
  }

  if (
    record.verificationAfterResumeRequired &&
    !hasRefs(record.verificationPolicyRefs)
  ) {
    throw new PylonFlexibleLoadProfileUnsafe({
      reason: 'Verification-after-resume requires verification policy refs.',
    })
  }

  if (
    record.powerEventEligibility === 'eligible_modeled' &&
    !hasRefs(record.modeledSuitabilityRefs)
  ) {
    throw new PylonFlexibleLoadProfileUnsafe({
      reason: 'Modeled power-event eligibility requires modeled suitability refs.',
    })
  }

  if (
    record.powerEventEligibility === 'eligible_measured' &&
    (!hasRefs(record.modeledSuitabilityRefs) ||
      !hasRefs(record.measuredResponseRefs))
  ) {
    throw new PylonFlexibleLoadProfileUnsafe({
      reason: 'Measured power-event eligibility requires modeled suitability refs and measured response refs.',
    })
  }

  if (hasRefs(record.revenueRefs) && !hasRefs(record.acceptedOutcomeRefs)) {
    throw new PylonFlexibleLoadProfileUnsafe({
      reason: 'Flexible-load revenue refs require accepted outcome refs.',
    })
  }

  if (
    hasRefs(record.settlementRefs) &&
    (!hasRefs(record.acceptedOutcomeRefs) || !hasRefs(record.revenueRefs))
  ) {
    throw new PylonFlexibleLoadProfileUnsafe({
      reason: 'Flexible-load settlement refs require accepted outcome and revenue refs.',
    })
  }
}

const projectionText = (
  projection: PylonFlexibleLoadProfileProjection,
): string =>
  [
    projection.id,
    projection.profileRef,
    projection.workClassRef,
    ...projection.codingWorkClassRefs,
    ...projection.siteWorkClassRefs,
    ...projection.omniWorkClassRefs,
    ...projection.checkpointPolicyRefs,
    ...projection.resumePolicyRefs,
    ...projection.verificationPolicyRefs,
    ...projection.modeledSuitabilityRefs,
    ...projection.measuredResponseRefs,
    ...projection.acceptedOutcomeRefs,
    ...projection.revenueRefs,
    ...projection.settlementRefs,
    ...projection.caveatRefs,
    ...projection.evidenceRefs,
  ].join(' ')

export const pylonFlexibleLoadProjectionHasPrivateMaterial = (
  projection: PylonFlexibleLoadProfileProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return universallyUnsafeRefPattern.test(text) ||
    isoTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectPylonFlexibleLoadProfile = (
  record: PylonFlexibleLoadProfileRecord,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): PylonFlexibleLoadProfileProjection => {
  assertRecordSafe(record)

  const projection: PylonFlexibleLoadProfileProjection = {
    acceptedOutcomeClaimAllowed: hasRefs(record.acceptedOutcomeRefs),
    acceptedOutcomeRefs: safeRefsForAudience(
      'flexible-load accepted outcome refs',
      record.acceptedOutcomeRefs,
      audience,
    ),
    audience,
    authority: record.authority,
    capacityAssignmentMutationAllowed: false,
    caveatRefs: safeRefsForAudience(
      'flexible-load caveat refs',
      record.caveatRefs,
      audience,
    ),
    checkpointCadence: record.checkpointCadence,
    checkpointCadenceLabel:
      checkpointCadenceLabelByValue[record.checkpointCadence],
    checkpointPolicyRefs: safeRefsForAudience(
      'flexible-load checkpoint policy refs',
      record.checkpointPolicyRefs,
      audience,
    ),
    codingWorkClassRefs: safeRefsForAudience(
      'flexible-load coding work refs',
      record.codingWorkClassRefs,
      audience,
    ),
    deadlineWindow: record.deadlineWindow,
    deadlineWindowLabel: deadlineWindowLabelByValue[record.deadlineWindow],
    evidenceRefs: safeRefsForAudience(
      'flexible-load evidence refs',
      record.evidenceRefs,
      audience,
    ),
    flexibilityClass: record.flexibilityClass,
    flexibilityClassLabel:
      flexibilityClassLabelByValue[record.flexibilityClass],
    id: record.id,
    interruptionTolerance: record.interruptionTolerance,
    interruptionToleranceLabel:
      interruptionToleranceLabelByValue[record.interruptionTolerance],
    measuredResponseRefs: safeRefsForAudience(
      'flexible-load measured response refs',
      record.measuredResponseRefs,
      audience,
    ),
    measuredSuitabilityClaimAllowed:
      record.powerEventEligibility === 'eligible_measured' &&
      hasRefs(record.measuredResponseRefs),
    modeledSuitabilityClaimAllowed:
      (record.powerEventEligibility === 'eligible_modeled' ||
        record.powerEventEligibility === 'eligible_measured') &&
      hasRefs(record.modeledSuitabilityRefs),
    modeledSuitabilityRefs: safeRefsForAudience(
      'flexible-load modeled suitability refs',
      record.modeledSuitabilityRefs,
      audience,
    ),
    omniWorkClassRefs: safeRefsForAudience(
      'flexible-load omni work refs',
      record.omniWorkClassRefs,
      audience,
    ),
    powerEventEligibility: record.powerEventEligibility,
    powerEventEligibilityLabel:
      powerEventEligibilityLabelByValue[record.powerEventEligibility],
    powerEventDispatchAllowed: false,
    profileRef: record.profileRef,
    replayCost: record.replayCost,
    replayCostLabel: replayCostLabelByValue[record.replayCost],
    resumePolicyRefs: safeRefsForAudience(
      'flexible-load resume policy refs',
      record.resumePolicyRefs,
      audience,
    ),
    resumeRequirement: record.resumeRequirement,
    resumeRequirementLabel:
      resumeRequirementLabelByValue[record.resumeRequirement],
    revenueClaimAllowed:
      hasRefs(record.revenueRefs) && hasRefs(record.acceptedOutcomeRefs),
    revenueRefs: safeRefsForAudience(
      'flexible-load revenue refs',
      record.revenueRefs,
      audience,
    ),
    settlementClaimAllowed:
      hasRefs(record.settlementRefs) &&
      hasRefs(record.revenueRefs) &&
      hasRefs(record.acceptedOutcomeRefs),
    settlementMutationAllowed: false,
    settlementRefs: safeRefsForAudience(
      'flexible-load settlement refs',
      record.settlementRefs,
      audience,
    ),
    siteWorkClassRefs: safeRefsForAudience(
      'flexible-load site work refs',
      record.siteWorkClassRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    verificationAfterResumeRequired: record.verificationAfterResumeRequired,
    verificationPolicyRefs: safeRefsForAudience(
      'flexible-load verification policy refs',
      record.verificationPolicyRefs,
      audience,
    ),
    workClassRef: record.workClassRef,
    workClassMutationAllowed: false,
    workKind: record.workKind,
    workKindLabel: workKindLabelByKind[record.workKind],
  }

  if (pylonFlexibleLoadProjectionHasPrivateMaterial(projection)) {
    throw new PylonFlexibleLoadProfileUnsafe({
      reason: 'Flexible-load projection contains material unsafe for the target audience.',
    })
  }

  return projection
}

export const examplePylonFlexibleLoadProfile = ():
  PylonFlexibleLoadProfileRecord => ({
    acceptedOutcomeRefs: [],
    authority: PYLON_FLEXIBLE_LOAD_PROFILE_READ_ONLY_AUTHORITY,
    caveatRefs: ['caveat.flex_profile_model_only'],
    checkpointCadence: 'per_step',
    checkpointPolicyRefs: ['checkpoint.policy.site_build_step_v1'],
    codingWorkClassRefs: ['coding.work_class.patch_queue'],
    deadlineWindow: 'overnight',
    evidenceRefs: ['evidence.flex_profile.contract_v1'],
    flexibilityClass: 'interruptible',
    id: 'flex_profile.autopilot_sites_review_build',
    interruptionTolerance: 'checkpoint_required',
    measuredResponseRefs: [],
    modeledSuitabilityRefs: ['model.flex.autopilot_sites_review_build_v1'],
    omniWorkClassRefs: ['omni.work_class.site_commerce_agent'],
    powerEventEligibility: 'eligible_modeled',
    profileRef: 'profile.flex.autopilot_sites_review_build',
    replayCost: 'low',
    resumePolicyRefs: ['resume.policy.same_trust_or_any_eligible_v1'],
    resumeRequirement: 'any_eligible_provider',
    revenueRefs: [],
    settlementRefs: [],
    siteWorkClassRefs: ['sites.work_class.review_revision'],
    updatedAtIso: '2026-06-06T22:05:00.000Z',
    verificationAfterResumeRequired: true,
    verificationPolicyRefs: ['verify.policy.after_resume_artifact_v1'],
    workClassRef: 'work_class.autopilot_sites_review_build',
    workKind: 'autopilot_sites',
  })
