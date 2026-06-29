import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const ArtanisPylonV02ReleaseParityStage = S.Literals([
  'accepted_work',
  'eligibility',
  'package_version',
  'paid_work',
  'platform_smoke',
  'release_assets',
  'runtime_smoke',
  'settlement',
  'source_support',
])
export type ArtanisPylonV02ReleaseParityStage =
  typeof ArtanisPylonV02ReleaseParityStage.Type

export const ArtanisPylonV02ReleaseParityState = S.Literals([
  'blocked',
  'verified',
])
export type ArtanisPylonV02ReleaseParityState =
  typeof ArtanisPylonV02ReleaseParityState.Type

export const ArtanisPylonV02PackageVersionState = S.Literals([
  'matched',
  'mismatched',
  'missing',
])
export type ArtanisPylonV02PackageVersionState =
  typeof ArtanisPylonV02PackageVersionState.Type

export class ArtanisPylonV02ReleaseParityAuthority extends S.Class<ArtanisPylonV02ReleaseParityAuthority>(
  'ArtanisPylonV02ReleaseParityAuthority',
)({
  eligibilityMutationAllowed: S.Boolean,
  packagePublishAllowed: S.Boolean,
  providerMutationAllowed: S.Boolean,
  publicClaimUpgradeAllowed: S.Boolean,
  releasePublicationAllowed: S.Boolean,
  settlementAllowed: S.Boolean,
  walletSpendAllowed: S.Boolean,
}) {}

export class ArtanisPylonV02ReleaseParityEvidence extends S.Class<ArtanisPylonV02ReleaseParityEvidence>(
  'ArtanisPylonV02ReleaseParityEvidence',
)({
  acceptedWorkProofRefs: S.Array(S.String),
  agentRef: S.String,
  authority: ArtanisPylonV02ReleaseParityAuthority,
  caveatRefs: S.Array(S.String),
  eligibilityTelemetryRefs: S.Array(S.String),
  expectedPackageVersionRef: S.String,
  expectedReleaseTag: S.String,
  packageVersionRefs: S.Array(S.String),
  packageVersionState: ArtanisPylonV02PackageVersionState,
  paidWorkReceiptRefs: S.Array(S.String),
  parityRef: S.String,
  paymentTargetRegistrationRefs: S.Array(S.String),
  platformSmokeRefs: S.Array(S.String),
  privateEvidenceRefs: S.Array(S.String),
  releaseAssetRefs: S.Array(S.String),
  releaseTag: S.NullOr(S.String),
  runtimeSmokeRefs: S.Array(S.String),
  settlementReceiptRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  sourceSupportRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class ArtanisPylonV02ReleaseParityStageProjection extends S.Class<ArtanisPylonV02ReleaseParityStageProjection>(
  'ArtanisPylonV02ReleaseParityStageProjection',
)({
  blockerRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  stage: ArtanisPylonV02ReleaseParityStage,
  state: ArtanisPylonV02ReleaseParityState,
}) {}

export class ArtanisPylonV02ReleaseParityProjection extends S.Class<ArtanisPylonV02ReleaseParityProjection>(
  'ArtanisPylonV02ReleaseParityProjection',
)({
  acceptedWorkClaimAllowed: S.Boolean,
  acceptedWorkProofRefs: S.Array(S.String),
  agentRef: S.String,
  audience: OmniProjectionAudience,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  eligibilityMutationAllowed: S.Boolean,
  eligibilityReady: S.Boolean,
  eligibilityTelemetryRefs: S.Array(S.String),
  expectedPackageVersionRef: S.String,
  expectedReleaseTag: S.String,
  generalAvailabilityClaimAllowed: S.Boolean,
  packagePublishAllowed: S.Boolean,
  packageVersionMatched: S.Boolean,
  packageVersionRefs: S.Array(S.String),
  packageVersionState: ArtanisPylonV02PackageVersionState,
  paidClaimAllowed: S.Boolean,
  paidWorkReceiptRefs: S.Array(S.String),
  parityRef: S.String,
  paymentTargetRegistrationRefs: S.Array(S.String),
  platformReady: S.Boolean,
  platformSmokeRefs: S.Array(S.String),
  privateEvidenceRefs: S.Array(S.String),
  providerMutationAllowed: S.Boolean,
  publicClaimSummary: S.String,
  publicClaimUpgradeAllowed: S.Boolean,
  releaseAssetRefs: S.Array(S.String),
  releasePublicationAllowed: S.Boolean,
  releaseReady: S.Boolean,
  releaseTagRef: S.NullOr(S.String),
  runtimeSmokeRefs: S.Array(S.String),
  settledClaimAllowed: S.Boolean,
  settlementAllowed: S.Boolean,
  settlementReceiptRefs: S.Array(S.String),
  shippedClaimAllowed: S.Boolean,
  sourceLevelSupportVisible: S.Boolean,
  sourceRefs: S.Array(S.String),
  sourceSupportRefs: S.Array(S.String),
  stageSummaryRefs: S.Array(S.String),
  stages: S.Array(ArtanisPylonV02ReleaseParityStageProjection),
  state: ArtanisPylonV02ReleaseParityState,
  stateLabel: S.String,
  updatedAtDisplay: S.String,
  walletSpendAllowed: S.Boolean,
}) {}

export class ArtanisPylonV02ReleaseParityUnsafe extends S.TaggedErrorClass<ArtanisPylonV02ReleaseParityUnsafe>()(
  'ArtanisPylonV02ReleaseParityUnsafe',
  {
    reason: S.String,
  },
) {}

export const ARTANIS_PYLON_V02_RELEASE_PARITY_NO_AUTHORITY:
  ArtanisPylonV02ReleaseParityAuthority =
    new ArtanisPylonV02ReleaseParityAuthority({
      eligibilityMutationAllowed: false,
      packagePublishAllowed: false,
      providerMutationAllowed: false,
      publicClaimUpgradeAllowed: false,
      releasePublicationAllowed: false,
      settlementAllowed: false,
      walletSpendAllowed: false,
    })

const requiredReleaseAssetRefs = [
  'asset.public.openagents.pylon_v0_2_0.darwin_arm64',
  'asset.public.openagents.pylon_v0_2_0.linux_x64',
  'asset.public.openagents.pylon_v0_2_0.wsl_ubuntu',
  'asset.public.openagents.pylon_v0_2_0.windows_x64',
] as const

const requiredPlatformSmokeRefs = [
  'smoke.public.pylon.v0_2.platform.linux',
  'smoke.public.pylon.v0_2.platform.macos_apple_silicon',
  'smoke.public.pylon.v0_2.platform.native_windows',
  'smoke.public.pylon.v0_2.platform.wsl_ubuntu',
] as const

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,300}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|callback[_-]?token|command[_-]?output[_-]?raw|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|node[_-]?(telemetry|raw|private)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw)|payout[_-]?target[_-]?raw|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|telemetry|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|command|customer|dataset|email|invoice|log|model|node|payment|payload|payout|prompt|provider|record|release|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|raw[_-]?payout[_-]?target|release[_-]?command[_-]?output|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const publicUnsafeRefPattern =
  /(^|[.:/_-])(customer|operator|payment|private|provider|raw|secret|wallet)([.:/_-]|$)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const refsForAudience = (
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  const safe = uniqueRefs(refs)

  if (audience === 'operator' || audience === 'private') {
    return safe
  }

  return safe.filter(ref => !publicUnsafeRefPattern.test(ref))
}

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new ArtanisPylonV02ReleaseParityUnsafe({
      reason:
        `${label} contains raw payout targets, wallet/payment material, provider secrets, raw release command output, private node telemetry, raw timestamps, or credential material.`,
    })
  }
}

const assertAuthority = (
  authority: ArtanisPylonV02ReleaseParityAuthority,
): void => {
  if (
    authority.eligibilityMutationAllowed !== false ||
    authority.packagePublishAllowed !== false ||
    authority.providerMutationAllowed !== false ||
    authority.publicClaimUpgradeAllowed !== false ||
    authority.releasePublicationAllowed !== false ||
    authority.settlementAllowed !== false ||
    authority.walletSpendAllowed !== false
  ) {
    throw new ArtanisPylonV02ReleaseParityUnsafe({
      reason:
        'Pylon v0.2 release parity evidence cannot publish releases or packages, mutate eligibility/providers, spend wallet funds, settle payouts, or upgrade public claims.',
    })
  }
}

const assertRecordSafe = (
  record: ArtanisPylonV02ReleaseParityEvidence,
): void => {
  assertAuthority(record.authority)

  if (record.agentRef !== 'agent_artanis') {
    throw new ArtanisPylonV02ReleaseParityUnsafe({
      reason: 'Pylon v0.2 release parity evidence must be owned by agent_artanis.',
    })
  }

  assertSafeRefs('Pylon v0.2 release parity refs', [
    record.agentRef,
    record.expectedPackageVersionRef,
    record.expectedReleaseTag,
    record.packageVersionState,
    record.parityRef,
    ...(record.releaseTag === null ? [] : [record.releaseTag]),
    ...record.acceptedWorkProofRefs,
    ...record.caveatRefs,
    ...record.eligibilityTelemetryRefs,
    ...record.packageVersionRefs,
    ...record.paidWorkReceiptRefs,
    ...record.paymentTargetRegistrationRefs,
    ...record.platformSmokeRefs,
    ...record.privateEvidenceRefs,
    ...record.releaseAssetRefs,
    ...record.runtimeSmokeRefs,
    ...record.settlementReceiptRefs,
    ...record.sourceRefs,
    ...record.sourceSupportRefs,
  ])

  if (
    rawTimestampPattern.test(JSON.stringify({
      ...record,
      updatedAtIso: 'redacted',
    }))
  ) {
    throw new ArtanisPylonV02ReleaseParityUnsafe({
      reason:
        'Pylon v0.2 release parity evidence cannot expose raw timestamps outside timestamp fields.',
    })
  }
}

const hasAll = (
  observed: ReadonlyArray<string>,
  required: ReadonlyArray<string>,
): boolean => {
  const observedSet = new Set(observed)

  return required.every(ref => observedSet.has(ref))
}

const missingRefs = (
  prefix: string,
  required: ReadonlyArray<string>,
  observed: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const observedSet = new Set(observed)

  return required
    .filter(ref => !observedSet.has(ref))
    .map(ref => `${prefix}.${ref.replace(/[^A-Za-z0-9]+/g, '_')}`)
}

const releaseTagRef = (
  releaseTag: string | null,
): string | null => releaseTag === null
  ? null
  : `release.public.openagents.${releaseTag.replace(/[^A-Za-z0-9]+/g, '_')}`

const projectionStrings = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(projectionStrings)
  }

  if (typeof value === 'object' && value !== null) {
    return Object.values(value).flatMap(projectionStrings)
  }

  return []
}

const stageProjection = (
  stage: ArtanisPylonV02ReleaseParityStage,
  verified: boolean,
  evidenceRefs: ReadonlyArray<string>,
  blockerRefs: ReadonlyArray<string>,
): ArtanisPylonV02ReleaseParityStageProjection =>
  new ArtanisPylonV02ReleaseParityStageProjection({
    blockerRefs: verified ? [] : uniqueRefs(blockerRefs),
    evidenceRefs: uniqueRefs(evidenceRefs),
    stage,
    state: verified ? 'verified' : 'blocked',
  })

const assertProjectionSafe = (
  projection: ArtanisPylonV02ReleaseParityProjection,
): void => {
  const unsafe = projectionStrings(projection).find(value =>
    containsProviderSecretMaterial(value) ||
    unsafeRefPattern.test(value) ||
    rawTimestampPattern.test(value)
  )

  if (unsafe !== undefined) {
    throw new ArtanisPylonV02ReleaseParityUnsafe({
      reason:
        'Pylon v0.2 release parity projection contains raw payout targets, wallet/payment material, provider secrets, raw release command output, private node telemetry, or raw timestamps.',
    })
  }
}

export const projectArtanisPylonV02ReleaseParity = (
  record: ArtanisPylonV02ReleaseParityEvidence,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): ArtanisPylonV02ReleaseParityProjection => {
  assertRecordSafe(record)

  const releaseTagMatches = record.releaseTag === record.expectedReleaseTag
  const releaseAssetsReady = hasAll(
    record.releaseAssetRefs,
    requiredReleaseAssetRefs,
  )
  const packageVersionMatched =
    record.packageVersionState === 'matched' &&
    record.packageVersionRefs.includes(record.expectedPackageVersionRef)
  const runtimeReady = record.runtimeSmokeRefs.length > 0
  const platformReady = hasAll(
    record.platformSmokeRefs,
    requiredPlatformSmokeRefs,
  )
  const eligibilityReady =
    record.eligibilityTelemetryRefs.length > 0 &&
    record.paymentTargetRegistrationRefs.length > 0
  const acceptedReady = record.acceptedWorkProofRefs.length > 0
  const paidReady = record.paidWorkReceiptRefs.length > 0
  const settledReady = record.settlementReceiptRefs.length > 0
  const releaseReady = releaseTagMatches &&
    releaseAssetsReady &&
    packageVersionMatched &&
    runtimeReady
  const sourceLevelSupportVisible = record.sourceSupportRefs.length > 0
  const stages = [
    stageProjection(
      'source_support',
      sourceLevelSupportVisible,
      record.sourceSupportRefs,
      ['blocker.public.pylon_v0_2.source_support_missing'],
    ),
    stageProjection(
      'release_assets',
      releaseTagMatches && releaseAssetsReady,
      [
        ...(record.releaseTag === null ? [] : [releaseTagRef(record.releaseTag)!]),
        ...record.releaseAssetRefs,
      ],
      [
        ...(record.releaseTag === null
          ? ['blocker.public.pylon_v0_2.release_tag_missing']
          : releaseTagMatches
            ? []
            : ['blocker.public.pylon_v0_2.release_tag_mismatch']),
        ...missingRefs(
          'missing.public.pylon_v0_2.release_asset',
          requiredReleaseAssetRefs,
          record.releaseAssetRefs,
        ),
      ],
    ),
    stageProjection(
      'package_version',
      packageVersionMatched,
      record.packageVersionRefs,
      [record.packageVersionState === 'missing'
        ? 'blocker.public.pylon_v0_2.package_version_missing'
        : 'blocker.public.pylon_v0_2.package_version_mismatch'],
    ),
    stageProjection(
      'runtime_smoke',
      runtimeReady,
      record.runtimeSmokeRefs,
      ['blocker.public.pylon_v0_2.runtime_smoke_missing'],
    ),
    stageProjection(
      'platform_smoke',
      platformReady,
      record.platformSmokeRefs,
      missingRefs(
        'missing.public.pylon_v0_2.platform_smoke',
        requiredPlatformSmokeRefs,
        record.platformSmokeRefs,
      ),
    ),
    stageProjection(
      'eligibility',
      eligibilityReady,
      [
        ...record.eligibilityTelemetryRefs,
        ...record.paymentTargetRegistrationRefs,
      ],
      [
        ...(record.eligibilityTelemetryRefs.length === 0
          ? ['blocker.public.pylon_v0_2.eligibility_telemetry_missing']
          : []),
        ...(record.paymentTargetRegistrationRefs.length === 0
          ? ['blocker.public.pylon_v0_2.payment_target_registration_missing']
          : []),
      ],
    ),
    stageProjection(
      'accepted_work',
      acceptedReady,
      record.acceptedWorkProofRefs,
      ['blocker.public.pylon_v0_2.accepted_work_proof_missing'],
    ),
    stageProjection(
      'paid_work',
      paidReady,
      record.paidWorkReceiptRefs,
      ['blocker.public.pylon_v0_2.paid_work_receipt_missing'],
    ),
    stageProjection(
      'settlement',
      settledReady,
      record.settlementReceiptRefs,
      ['blocker.public.pylon_v0_2.settlement_receipt_missing'],
    ),
  ]
  const blockerRefs = uniqueRefs(stages.flatMap(stage => stage.blockerRefs))
  const state = blockerRefs.length === 0 ? 'verified' : 'blocked'
  const projection = new ArtanisPylonV02ReleaseParityProjection({
    acceptedWorkClaimAllowed: acceptedReady,
    acceptedWorkProofRefs:
      refsForAudience(record.acceptedWorkProofRefs, audience),
    agentRef: record.agentRef,
    audience,
    blockerRefs,
    caveatRefs: refsForAudience(record.caveatRefs, audience),
    eligibilityMutationAllowed: record.authority.eligibilityMutationAllowed,
    eligibilityReady,
    eligibilityTelemetryRefs:
      refsForAudience(record.eligibilityTelemetryRefs, audience),
    expectedPackageVersionRef: record.expectedPackageVersionRef,
    expectedReleaseTag: record.expectedReleaseTag,
    generalAvailabilityClaimAllowed: releaseReady && platformReady &&
      eligibilityReady,
    packagePublishAllowed: record.authority.packagePublishAllowed,
    packageVersionMatched,
    packageVersionRefs: refsForAudience(record.packageVersionRefs, audience),
    packageVersionState: record.packageVersionState,
    paidClaimAllowed: paidReady,
    paidWorkReceiptRefs:
      refsForAudience(record.paidWorkReceiptRefs, audience),
    parityRef: record.parityRef,
    paymentTargetRegistrationRefs:
      refsForAudience(record.paymentTargetRegistrationRefs, audience),
    platformReady,
    platformSmokeRefs: refsForAudience(record.platformSmokeRefs, audience),
    privateEvidenceRefs: audience === 'operator' || audience === 'private'
      ? refsForAudience(record.privateEvidenceRefs, audience)
      : [],
    providerMutationAllowed: record.authority.providerMutationAllowed,
    publicClaimSummary: state === 'verified'
      ? 'Pylon v0.2 release parity evidence is complete in this modeled packet; public claims must still cite the listed release, platform, eligibility, accepted-work, paid-work, and settlement refs.'
      : 'Pylon v0.2 source-level support is visible; shipped, general-availability, accepted-work, paid-work, and settlement claims remain blocked until release parity evidence is complete.',
    publicClaimUpgradeAllowed: record.authority.publicClaimUpgradeAllowed,
    releaseAssetRefs: refsForAudience(record.releaseAssetRefs, audience),
    releasePublicationAllowed: record.authority.releasePublicationAllowed,
    releaseReady,
    releaseTagRef: releaseTagRef(record.releaseTag),
    runtimeSmokeRefs: refsForAudience(record.runtimeSmokeRefs, audience),
    settledClaimAllowed: settledReady,
    settlementAllowed: record.authority.settlementAllowed,
    settlementReceiptRefs:
      refsForAudience(record.settlementReceiptRefs, audience),
    shippedClaimAllowed: releaseReady,
    sourceLevelSupportVisible,
    sourceRefs: refsForAudience(record.sourceRefs, audience),
    sourceSupportRefs: refsForAudience(record.sourceSupportRefs, audience),
    stageSummaryRefs: stages.map(stage =>
      `stage_summary.public.pylon_v0_2.release_parity.${stage.stage}.${stage.state}`
    ),
    stages,
    state,
    stateLabel: state === 'verified'
      ? 'Pylon v0.2 release parity modeled complete'
      : 'Pylon v0.2 release parity blocked',
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    walletSpendAllowed: record.authority.walletSpendAllowed,
  })

  assertProjectionSafe(projection)

  return projection
}

export const exampleArtanisPylonV02ReleaseParityEvidence = ():
  ArtanisPylonV02ReleaseParityEvidence =>
    new ArtanisPylonV02ReleaseParityEvidence({
      acceptedWorkProofRefs: [],
      agentRef: 'agent_artanis',
      authority: ARTANIS_PYLON_V02_RELEASE_PARITY_NO_AUTHORITY,
      caveatRefs: [
        'caveat.public.source_support_is_not_release_parity',
        'caveat.public.release_parity_requires_platform_and_receipt_evidence',
      ],
      eligibilityTelemetryRefs: [],
      expectedPackageVersionRef: 'version.public.pylon.package.0_2_0',
      expectedReleaseTag: 'pylon-v0.2.0',
      packageVersionRefs: ['version.public.pylon.package.0_1_23'],
      packageVersionState: 'mismatched',
      paidWorkReceiptRefs: [],
      parityRef: 'parity.public.artanis.pylon_v0_2.release',
      paymentTargetRegistrationRefs: [],
      platformSmokeRefs: [
        'smoke.public.pylon.v0_2.platform.macos_apple_silicon',
      ],
      privateEvidenceRefs: [
        'evidence.operator.pylon_v0_2.release_command_redacted',
      ],
      releaseAssetRefs: ['asset.public.openagents.pylon_v0_1_23.darwin_arm64'],
      releaseTag: null,
      runtimeSmokeRefs: [],
      settlementReceiptRefs: [],
      sourceRefs: [
        'docs/artanis/2026-06-06-pylon-v02-release-parity-evidence.md',
        'source.public.openagents.pylon_release_audit',
      ],
      sourceSupportRefs: [
        'source.public.pylon_v0_2_ldk_target_contract',
        'docs/pylon/2026-06-06-payout-target-admission-projection.md',
      ],
      updatedAtIso: '2026-06-07T08:00:00.000Z',
    })

export const releaseReadyArtanisPylonV02ReleaseParityEvidence = ():
  ArtanisPylonV02ReleaseParityEvidence =>
    new ArtanisPylonV02ReleaseParityEvidence({
      acceptedWorkProofRefs: [
        'proof.public.pylon_v0_2.accepted_work.first_job',
      ],
      agentRef: 'agent_artanis',
      authority: ARTANIS_PYLON_V02_RELEASE_PARITY_NO_AUTHORITY,
      caveatRefs: [
        'caveat.public.release_parity_packet_is_modeled_evidence',
      ],
      eligibilityTelemetryRefs: [
        'telemetry.public.pylon_v0_2.eligibility_snapshot',
      ],
      expectedPackageVersionRef: 'version.public.pylon.package.0_2_0',
      expectedReleaseTag: 'pylon-v0.2.0',
      packageVersionRefs: ['version.public.pylon.package.0_2_0'],
      packageVersionState: 'matched',
      paidWorkReceiptRefs: [
        'receipt.public.pylon_v0_2.paid_work.first_job',
      ],
      parityRef: 'parity.public.artanis.pylon_v0_2.release',
      paymentTargetRegistrationRefs: [
        'target.public.pylon_v0_2.ldk_payment_target_registered',
      ],
      platformSmokeRefs: [...requiredPlatformSmokeRefs],
      privateEvidenceRefs: [
        'evidence.operator.pylon_v0_2.release_command_redacted',
      ],
      releaseAssetRefs: [...requiredReleaseAssetRefs],
      releaseTag: 'pylon-v0.2.0',
      runtimeSmokeRefs: ['smoke.public.pylon_v0_2.runtime.first_boot'],
      settlementReceiptRefs: [
        'receipt.public.pylon_v0_2.settlement.first_job',
      ],
      sourceRefs: [
        'docs/artanis/2026-06-06-pylon-v02-release-parity-evidence.md',
        'source.public.openagents.pylon_release_audit',
      ],
      sourceSupportRefs: [
        'source.public.pylon_v0_2_ldk_target_contract',
        'docs/pylon/2026-06-06-payout-target-admission-projection.md',
      ],
      updatedAtIso: '2026-06-07T08:00:00.000Z',
    })
