import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  PublicClaimCopyRule,
  PublicClaimKind,
  PublicClaimState,
  PublicClaimStateProjection,
} from './public-claim-state'
import {
  PublicClaimProjectionAudience,
  PublicClaimProjectionUnsafe,
  projectPublicClaimRecord,
  publicClaimProjectionHasPrivateMaterial,
} from './public-claim-projections'

export const ArtanisPylonV02ReadinessStage = S.Literals([
  'accepted',
  'eligible',
  'paid',
  'platform_ready',
  'release_ready',
  'settled',
  'source_ready',
])
export type ArtanisPylonV02ReadinessStage =
  typeof ArtanisPylonV02ReadinessStage.Type

export const ArtanisPylonV02Platform = S.Literals([
  'linux',
  'macos_apple_silicon',
  'native_windows',
  'wsl_ubuntu',
])
export type ArtanisPylonV02Platform = typeof ArtanisPylonV02Platform.Type

export class ArtanisPylonV02ReadinessGate extends S.Class<ArtanisPylonV02ReadinessGate>(
  'ArtanisPylonV02ReadinessGate',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  claimId: S.String,
  claimKind: PublicClaimKind,
  claimRef: S.String,
  desiredState: PublicClaimState,
  evidenceRefs: S.Array(S.String),
  nextActionRefs: S.Array(S.String),
  releaseAssetEvidenceRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  stage: ArtanisPylonV02ReadinessStage,
  titleRef: S.String,
  updatedAtIso: S.String,
}) {}

export class ArtanisPylonV02PlatformGuidance extends S.Class<ArtanisPylonV02PlatformGuidance>(
  'ArtanisPylonV02PlatformGuidance',
)({
  assetEvidenceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  platform: ArtanisPylonV02Platform,
  recommendedPathRef: S.String,
  smokeEvidenceRefs: S.Array(S.String),
  state: PublicClaimState,
}) {}

export class ArtanisPylonV02ForumLaunchTemplate extends S.Class<ArtanisPylonV02ForumLaunchTemplate>(
  'ArtanisPylonV02ForumLaunchTemplate',
)({
  bodyText: S.String,
  caveatRefs: S.Array(S.String),
  disallowedClaimRefs: S.Array(S.String),
  readinessCommandRefs: S.Array(S.String),
  resourceModeCaveatRefs: S.Array(S.String),
  setupRefs: S.Array(S.String),
  title: S.String,
}) {}

export class ArtanisPylonV02ReadinessInput extends S.Class<ArtanisPylonV02ReadinessInput>(
  'ArtanisPylonV02ReadinessInput',
)({
  agentRef: S.String,
  forumTemplate: ArtanisPylonV02ForumLaunchTemplate,
  gates: S.Array(ArtanisPylonV02ReadinessGate),
  platformGuidance: S.Array(ArtanisPylonV02PlatformGuidance),
  readinessRef: S.String,
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class ArtanisPylonV02ReadinessGateProjection extends S.Class<ArtanisPylonV02ReadinessGateProjection>(
  'ArtanisPylonV02ReadinessGateProjection',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  claimRef: S.String,
  copyRule: PublicClaimCopyRule,
  evidenceRefs: S.Array(S.String),
  nextActionRefs: S.Array(S.String),
  releaseAssetEvidenceRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  stage: ArtanisPylonV02ReadinessStage,
  state: PublicClaimStateProjection,
  titleRef: S.String,
  updatedAtDisplay: S.String,
}) {}

export class ArtanisPylonV02ReadinessProjection extends S.Class<ArtanisPylonV02ReadinessProjection>(
  'ArtanisPylonV02ReadinessProjection',
)({
  agentRef: S.String,
  audience: PublicClaimProjectionAudience,
  forumTemplate: ArtanisPylonV02ForumLaunchTemplate,
  gates: S.Array(ArtanisPylonV02ReadinessGateProjection),
  platformGuidance: S.Array(ArtanisPylonV02PlatformGuidance),
  readinessRef: S.String,
  sourceRefs: S.Array(S.String),
  stateCounts: S.Array(S.Struct({
    count: S.Number,
    state: PublicClaimState,
  })),
  updatedAtDisplay: S.String,
}) {}

export class ArtanisPylonV02ReadinessUnsafe extends S.TaggedErrorClass<ArtanisPylonV02ReadinessUnsafe>()(
  'ArtanisPylonV02ReadinessUnsafe',
  {
    reason: S.String,
  },
) {}

const requiredStages: ReadonlyArray<ArtanisPylonV02ReadinessStage> = [
  'accepted',
  'eligible',
  'paid',
  'platform_ready',
  'release_ready',
  'settled',
  'source_ready',
]
const requiredPlatforms: ReadonlyArray<ArtanisPylonV02Platform> = [
  'linux',
  'macos_apple_silicon',
  'native_windows',
  'wsl_ubuntu',
]
const readinessCommandRefs: ReadonlySet<string> = new Set([
  'command.public.pylon.version',
  'command.public.pylon.status_json',
  'command.public.pylon.training_status_json',
  'command.public.pylon.balance_json',
  'command.public.pylon.history_json',
])
const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const forbiddenLaunchCopyPatterns = [
  /Pylon v0\.2 is publicly released/i,
  /Pylon v0\.2 is ready for (all users|everyone)/i,
  /run Pylon (and|to) (earn|get paid|make money)/i,
  /earn money/i,
  /guaranteed/i,
  /online Pylons? are eligible/i,
  /accepted work is already paid/i,
  /paid work is settled/i,
  /send .*wallet.*secret/i,
  /share .*recovery phrase/i,
  /share .*private key/i,
  /share .*preimage/i,
]

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
    unsafeRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new ArtanisPylonV02ReadinessUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, customer, private repo, or raw timestamp material.`,
    })
  }
}

const assertCopySafe = (copy: string): void => {
  if (
    containsProviderSecretMaterial(copy) ||
    rawTimestampPattern.test(copy) ||
    forbiddenLaunchCopyPatterns.some(pattern => pattern.test(copy))
  ) {
    throw new ArtanisPylonV02ReadinessUnsafe({
      reason:
        'Pylon v0.2 launch copy overclaims readiness, earnings, payment, settlement, or asks for secret material.',
    })
  }
}

const assertAllPresent = <A extends string>(
  label: string,
  required: ReadonlyArray<A>,
  present: ReadonlyArray<A>,
): void => {
  const presentSet = new Set(present)
  const missing = required.filter(item => !presentSet.has(item))

  if (missing.length > 0) {
    throw new ArtanisPylonV02ReadinessUnsafe({
      reason: `${label} missing required entries: ${missing.join(', ')}.`,
    })
  }
}

const assertForumTemplate = (
  template: ArtanisPylonV02ForumLaunchTemplate,
): void => {
  assertCopySafe(template.title)
  assertCopySafe(template.bodyText)
  assertSafeRefs('Pylon v0.2 setup refs', template.setupRefs)
  assertSafeRefs(
    'Pylon v0.2 readiness command refs',
    template.readinessCommandRefs,
  )
  assertSafeRefs(
    'Pylon v0.2 resource mode caveat refs',
    template.resourceModeCaveatRefs,
  )
  assertSafeRefs('Pylon v0.2 caveat refs', template.caveatRefs)
  assertSafeRefs(
    'Pylon v0.2 disallowed claim refs',
    template.disallowedClaimRefs,
  )

  const templateCommandRefs = new Set(template.readinessCommandRefs)

  for (const commandRef of readinessCommandRefs) {
    if (!templateCommandRefs.has(commandRef)) {
      throw new ArtanisPylonV02ReadinessUnsafe({
        reason:
          'Pylon v0.2 Forum template is missing required readiness command refs.',
      })
    }
  }

  for (const commandRef of template.readinessCommandRefs) {
    if (!readinessCommandRefs.has(commandRef)) {
      throw new ArtanisPylonV02ReadinessUnsafe({
        reason:
          'Pylon v0.2 Forum template includes an unsupported readiness command ref.',
      })
    }
  }

  if (
    !template.setupRefs.includes(
      'docs/sites/2026-06-05-pylon-local-compute-instruction-packet.md',
    ) ||
    !template.setupRefs.includes(
      'docs/sites/2026-06-05-pylon-v0-2-public-readiness-audit.md',
    )
  ) {
    throw new ArtanisPylonV02ReadinessUnsafe({
      reason: 'Pylon v0.2 Forum template must link the setup packet and readiness audit.',
    })
  }
}

const assertGate = (gate: ArtanisPylonV02ReadinessGate): void => {
  assertSafeRefs('Pylon v0.2 gate refs', [
    gate.claimId,
    gate.claimRef,
    gate.titleRef,
  ])
  assertSafeRefs('Pylon v0.2 gate blocker refs', gate.blockerRefs)
  assertSafeRefs('Pylon v0.2 gate caveat refs', gate.caveatRefs)
  assertSafeRefs('Pylon v0.2 gate evidence refs', gate.evidenceRefs)
  assertSafeRefs('Pylon v0.2 gate next action refs', gate.nextActionRefs)
  assertSafeRefs(
    'Pylon v0.2 release asset evidence refs',
    gate.releaseAssetEvidenceRefs,
  )
  assertSafeRefs('Pylon v0.2 gate source refs', gate.sourceRefs)

  if (gate.stage !== 'source_ready' && gate.desiredState === 'verified') {
    throw new ArtanisPylonV02ReadinessUnsafe({
      reason:
        'Only source-ready Pylon v0.2 claims can be verified in the current readiness packet.',
    })
  }

  if (
    (gate.stage === 'paid' || gate.stage === 'settled') &&
    gate.desiredState !== 'prohibited'
  ) {
    throw new ArtanisPylonV02ReadinessUnsafe({
      reason:
        'Pylon v0.2 paid and settled claims are prohibited until public receipt chains exist.',
    })
  }
}

const assertPlatformGuidance = (
  guidance: ArtanisPylonV02PlatformGuidance,
): void => {
  assertSafeRefs('Pylon v0.2 platform path ref', [
    guidance.recommendedPathRef,
  ])
  assertSafeRefs(
    'Pylon v0.2 platform asset refs',
    guidance.assetEvidenceRefs,
  )
  assertSafeRefs('Pylon v0.2 platform caveat refs', guidance.caveatRefs)
  assertSafeRefs(
    'Pylon v0.2 platform smoke refs',
    guidance.smokeEvidenceRefs,
  )

  if (
    guidance.platform !== 'macos_apple_silicon' &&
    guidance.state === 'verified'
  ) {
    throw new ArtanisPylonV02ReadinessUnsafe({
      reason:
        'Only Apple Silicon macOS has the currently verified public binary path in the readiness packet.',
    })
  }
}

const stateCounts = (
  gates: ReadonlyArray<ArtanisPylonV02ReadinessGateProjection>,
): ReadonlyArray<{ count: number; state: PublicClaimState }> =>
  [...gates.reduce((counts, gate) => {
    counts.set(gate.state.state, (counts.get(gate.state.state) ?? 0) + 1)

    return counts
  }, new Map<PublicClaimState, number>())]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([state, count]) => ({ count, state }))

const projectGate = (
  input: ArtanisPylonV02ReadinessInput,
  gate: ArtanisPylonV02ReadinessGate,
  audience: PublicClaimProjectionAudience,
  nowIso: string,
): ArtanisPylonV02ReadinessGateProjection => {
  assertGate(gate)

  const claim = projectPublicClaimRecord({
    caveatRefs: uniqueRefs(gate.caveatRefs),
    claimId: gate.claimId,
    claimKind: gate.claimKind,
    claimRef: gate.claimRef,
    customerRefs: [],
    desiredState: gate.desiredState,
    evidenceRefs: uniqueRefs(gate.evidenceRefs),
    operatorRefs: [],
    sourceRefs: uniqueRefs([...input.sourceRefs, ...gate.sourceRefs]),
    subjectRef: input.readinessRef,
    surface: 'pylon',
    teamRefs: [],
    titleRef: gate.titleRef,
    updatedAt: gate.updatedAtIso,
  }, audience)

  return {
    blockerRefs: uniqueRefs(gate.blockerRefs),
    caveatRefs: claim.caveatRefs,
    claimRef: claim.claimRef,
    copyRule: claim.copyRule,
    evidenceRefs: claim.evidenceRefs,
    nextActionRefs: uniqueRefs(gate.nextActionRefs),
    releaseAssetEvidenceRefs: uniqueRefs(gate.releaseAssetEvidenceRefs),
    sourceRefs: claim.sourceRefs,
    stage: gate.stage,
    state: claim.state,
    titleRef: claim.titleRef,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      gate.updatedAtIso,
      nowIso,
    ),
  }
}

export const projectArtanisPylonV02Readiness = (
  input: ArtanisPylonV02ReadinessInput,
  audience: PublicClaimProjectionAudience,
  nowIso: string,
): ArtanisPylonV02ReadinessProjection => {
  assertSafeRefs('Pylon v0.2 readiness identity refs', [
    input.agentRef,
    input.readinessRef,
  ])
  assertSafeRefs('Pylon v0.2 readiness source refs', input.sourceRefs)
  assertForumTemplate(input.forumTemplate)
  input.gates.forEach(assertGate)
  input.platformGuidance.forEach(assertPlatformGuidance)
  assertAllPresent(
    'Pylon v0.2 readiness stages',
    requiredStages,
    input.gates.map(gate => gate.stage),
  )
  assertAllPresent(
    'Pylon v0.2 platform guidance',
    requiredPlatforms,
    input.platformGuidance.map(guidance => guidance.platform),
  )

  if (input.agentRef !== 'agent_artanis') {
    throw new ArtanisPylonV02ReadinessUnsafe({
      reason: 'Pylon v0.2 readiness must be administered by agent_artanis.',
    })
  }

  const gates = input.gates.map(gate =>
    projectGate(input, gate, audience, nowIso),
  )
  const projection: ArtanisPylonV02ReadinessProjection = {
    agentRef: input.agentRef,
    audience,
    forumTemplate: input.forumTemplate,
    gates,
    platformGuidance: [...input.platformGuidance],
    readinessRef: input.readinessRef,
    sourceRefs: uniqueRefs(input.sourceRefs),
    stateCounts: stateCounts(gates),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      input.updatedAtIso,
      nowIso,
    ),
  }

  const serialized = JSON.stringify(projection)

  if (
    containsProviderSecretMaterial(serialized) ||
    publicClaimProjectionHasPrivateMaterial(serialized) ||
    unsafeRefPattern.test(serialized) ||
    rawTimestampPattern.test(serialized)
  ) {
    throw new PublicClaimProjectionUnsafe({
      reason: 'Pylon v0.2 readiness projection contains private material.',
    })
  }

  return projection
}

export const exampleArtanisPylonV02Readiness = ():
  ArtanisPylonV02ReadinessInput =>
    new ArtanisPylonV02ReadinessInput({
      agentRef: 'agent_artanis',
      forumTemplate: {
        bodyText:
          'Episode 232 introduces Pylon v0.2 source-level LDK target readiness and Artanis coordination. Use the setup packet, run the readiness commands locally, prefer WSL Ubuntu on Windows, and treat online, eligible, assigned, accepted, paid, and settled as separate states. Do not paste credentials or local node material into public posts.',
        caveatRefs: [
          'caveat.public.pylon_v0_2_not_broad_release_ready',
          'caveat.public.no_unconditional_earnings_claim',
          'caveat.public.no_private_material_requests',
        ],
        disallowedClaimRefs: [
          'claim.disallowed.pylon_v0_2_ready_for_everyone',
          'claim.disallowed.run_pylon_and_earn_money',
          'claim.disallowed.online_means_eligible',
          'claim.disallowed.accepted_means_paid',
          'claim.disallowed.paid_means_settled',
        ],
        readinessCommandRefs: [
          'command.public.pylon.version',
          'command.public.pylon.status_json',
          'command.public.pylon.training_status_json',
          'command.public.pylon.balance_json',
          'command.public.pylon.history_json',
        ],
        resourceModeCaveatRefs: [
          'caveat.public.resource_mode_background_may_not_be_enough',
          'caveat.public.resource_mode_overnight_owner_selected',
          'caveat.public.resource_mode_dedicated_requires_operator_intent',
        ],
        setupRefs: [
          'docs/sites/2026-06-05-pylon-local-compute-instruction-packet.md',
          'docs/sites/2026-06-05-pylon-v0-2-public-readiness-audit.md',
        ],
        title: 'Artanis Pylon v0.2 launch readiness update',
      },
      gates: [
        {
          blockerRefs: [],
          caveatRefs: ['caveat.public.source_ready_not_release_ready'],
          claimId: 'claim_artanis_pylon_v0_2_source_ready',
          claimKind: 'research',
          claimRef: 'claim.artanis.pylon_v0_2.source_ready',
          desiredState: 'verified',
          evidenceRefs: [
            'docs/sites/2026-06-05-pylon-v0-2-public-readiness-audit.md',
            'docs/pylon/2026-06-06-payout-target-admission-projection.md',
          ],
          nextActionRefs: ['next_action.retain_release_assets'],
          releaseAssetEvidenceRefs: [
            'release.public.pylon_v0_1_23.darwin_arm64',
          ],
          sourceRefs: ['source.public.pylon_v0_2_ldk_target_contract'],
          stage: 'source_ready',
          titleRef: 'title.artanis.pylon_v0_2.source_ready',
          updatedAtIso: '2026-06-06T23:45:00.000Z',
        },
        {
          blockerRefs: ['blocker.public.no_pylon_v0_2_release_asset'],
          caveatRefs: ['caveat.public.current_release_line_is_v0_1'],
          claimId: 'claim_artanis_pylon_v0_2_release_ready',
          claimKind: 'deployment',
          claimRef: 'claim.artanis.pylon_v0_2.release_ready',
          desiredState: 'blocked',
          evidenceRefs: [],
          nextActionRefs: ['next_action.publish_v0_2_or_document_v0_1_line'],
          releaseAssetEvidenceRefs: [
            'missing.public.pylon_v0_2.release_tag',
            'missing.public.pylon_v0_2.linux_assets',
            'missing.public.pylon_v0_2.windows_asset',
          ],
          sourceRefs: ['source.public.pylon_release_audit'],
          stage: 'release_ready',
          titleRef: 'title.artanis.pylon_v0_2.release_ready',
          updatedAtIso: '2026-06-06T23:45:00.000Z',
        },
        {
          blockerRefs: ['blocker.public.platform_smokes_missing'],
          caveatRefs: ['caveat.public.apple_silicon_strongest_path'],
          claimId: 'claim_artanis_pylon_v0_2_platform_ready',
          claimKind: 'deployment',
          claimRef: 'claim.artanis.pylon_v0_2.platform_ready',
          desiredState: 'blocked',
          evidenceRefs: [],
          nextActionRefs: ['next_action.retain_linux_wsl_windows_smokes'],
          releaseAssetEvidenceRefs: [
            'asset.public.current.darwin_arm64_only',
          ],
          sourceRefs: ['source.public.pylon_platform_posture'],
          stage: 'platform_ready',
          titleRef: 'title.artanis.pylon_v0_2.platform_ready',
          updatedAtIso: '2026-06-06T23:45:00.000Z',
        },
        {
          blockerRefs: ['blocker.public.ldk_target_required_for_eligibility'],
          caveatRefs: ['caveat.public.online_is_not_eligible'],
          claimId: 'claim_artanis_pylon_v0_2_eligible',
          claimKind: 'provider_settlement',
          claimRef: 'claim.artanis.pylon_v0_2.eligible',
          desiredState: 'planned',
          evidenceRefs: [],
          nextActionRefs: ['next_action.verify_ldk_target_registration'],
          releaseAssetEvidenceRefs: [],
          sourceRefs: ['source.public.pylon_payout_target_admission'],
          stage: 'eligible',
          titleRef: 'title.artanis.pylon_v0_2.eligible',
          updatedAtIso: '2026-06-06T23:45:00.000Z',
        },
        {
          blockerRefs: ['blocker.public.no_accepted_work_receipt'],
          caveatRefs: ['caveat.public.assigned_is_not_accepted'],
          claimId: 'claim_artanis_pylon_v0_2_accepted',
          claimKind: 'fulfillment_receipt',
          claimRef: 'claim.artanis.pylon_v0_2.accepted',
          desiredState: 'prohibited',
          evidenceRefs: [],
          nextActionRefs: ['next_action.wait_for_accepted_work_receipt'],
          releaseAssetEvidenceRefs: [],
          sourceRefs: ['source.public.accepted_work_boundary'],
          stage: 'accepted',
          titleRef: 'title.artanis.pylon_v0_2.accepted',
          updatedAtIso: '2026-06-06T23:45:00.000Z',
        },
        {
          blockerRefs: ['blocker.public.no_paid_work_receipt'],
          caveatRefs: ['caveat.public.accepted_is_not_paid'],
          claimId: 'claim_artanis_pylon_v0_2_paid',
          claimKind: 'provider_settlement',
          claimRef: 'claim.artanis.pylon_v0_2.paid',
          desiredState: 'prohibited',
          evidenceRefs: [],
          nextActionRefs: ['next_action.wait_for_public_paid_work_receipt'],
          releaseAssetEvidenceRefs: [],
          sourceRefs: ['source.public.paid_work_boundary'],
          stage: 'paid',
          titleRef: 'title.artanis.pylon_v0_2.paid',
          updatedAtIso: '2026-06-06T23:45:00.000Z',
        },
        {
          blockerRefs: ['blocker.public.no_settlement_receipt'],
          caveatRefs: ['caveat.public.paid_is_not_settled'],
          claimId: 'claim_artanis_pylon_v0_2_settled',
          claimKind: 'provider_settlement',
          claimRef: 'claim.artanis.pylon_v0_2.settled',
          desiredState: 'prohibited',
          evidenceRefs: [],
          nextActionRefs: ['next_action.wait_for_public_settlement_receipt'],
          releaseAssetEvidenceRefs: [],
          sourceRefs: ['source.public.settlement_boundary'],
          stage: 'settled',
          titleRef: 'title.artanis.pylon_v0_2.settled',
          updatedAtIso: '2026-06-06T23:45:00.000Z',
        },
      ],
      platformGuidance: [
        {
          assetEvidenceRefs: ['asset.public.current.darwin_arm64'],
          caveatRefs: ['caveat.public.macos_apple_silicon_binary_path'],
          platform: 'macos_apple_silicon',
          recommendedPathRef: 'path.public.pylon_launcher.macos_darwin_arm64',
          smokeEvidenceRefs: ['smoke.public.pylon.macos_darwin_arm64_current'],
          state: 'verified',
        },
        {
          assetEvidenceRefs: ['missing.public.current.linux_assets'],
          caveatRefs: ['caveat.public.linux_may_source_build'],
          platform: 'linux',
          recommendedPathRef: 'path.public.pylon_launcher.linux_source_build',
          smokeEvidenceRefs: [],
          state: 'blocked',
        },
        {
          assetEvidenceRefs: ['missing.public.current.linux_assets'],
          caveatRefs: ['caveat.public.wsl_ubuntu_preferred_on_windows'],
          platform: 'wsl_ubuntu',
          recommendedPathRef: 'path.public.pylon_launcher.wsl_ubuntu',
          smokeEvidenceRefs: [],
          state: 'blocked',
        },
        {
          assetEvidenceRefs: ['missing.public.current.windows_asset'],
          caveatRefs: ['caveat.public.native_windows_experimental'],
          platform: 'native_windows',
          recommendedPathRef: 'path.public.pylon_launcher.native_windows_exp',
          smokeEvidenceRefs: [],
          state: 'blocked',
        },
      ],
      readinessRef: 'readiness.public.artanis.pylon_v0_2',
      sourceRefs: [
        'docs/sites/2026-06-05-pylon-local-compute-instruction-packet.md',
        'docs/sites/2026-06-05-pylon-v0-2-public-readiness-audit.md',
        'docs/pylon/2026-06-06-payout-target-admission-projection.md',
        'docs/pylon/2026-06-06-ldk-readiness-projections.md',
      ],
      updatedAtIso: '2026-06-06T23:45:00.000Z',
    })
