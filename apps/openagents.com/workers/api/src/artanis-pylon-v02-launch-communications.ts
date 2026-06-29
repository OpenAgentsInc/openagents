import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  ArtanisForumPublicationIntentRecord,
  ArtanisForumPublicationUnsafe,
  projectArtanisForumPublicationQueue,
} from './artanis-forum-publication'
import {
  ArtanisPylonV02ReadinessProjection,
  exampleArtanisPylonV02Readiness,
  projectArtanisPylonV02Readiness,
} from './artanis-pylon-v02-readiness'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export class ArtanisPylonV02LaunchCommunicationRecord extends S.Class<ArtanisPylonV02LaunchCommunicationRecord>(
  'ArtanisPylonV02LaunchCommunicationRecord',
)({
  agentRef: S.String,
  artanisPageRefs: S.Array(S.String),
  authorityBoundaryRefs: S.Array(S.String),
  briefMarkdown: S.String,
  capabilityRefs: S.Array(S.String),
  docsPageRefs: S.Array(S.String),
  forumIntent: ArtanisForumPublicationIntentRecord,
  forumPostTitle: S.String,
  launchPackageRef: S.String,
  optionalSocialCopy: S.String,
  ownerSetupRefs: S.Array(S.String),
  primaryForumTopicRef: S.String,
  primaryForumTopicUrl: S.String,
  readinessRef: S.String,
  readinessStageRefs: S.Array(S.String),
  resourceModeCaveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class ArtanisPylonV02LaunchCommunicationProjection extends S.Class<ArtanisPylonV02LaunchCommunicationProjection>(
  'ArtanisPylonV02LaunchCommunicationProjection',
)({
  agentRef: S.String,
  artanisPageRefs: S.Array(S.String),
  authorityBoundaryRefs: S.Array(S.String),
  briefMarkdown: S.String,
  capabilityRefs: S.Array(S.String),
  docsPageRefs: S.Array(S.String),
  forumIntentRef: S.String,
  forumIntentReady: S.Boolean,
  forumPostBody: S.String,
  forumPostTitle: S.String,
  launchPackageRef: S.String,
  optionalSocialCopy: S.String,
  ownerSetupRefs: S.Array(S.String),
  primaryForumTopicRef: S.String,
  primaryForumTopicUrl: S.String,
  readinessRef: S.String,
  readinessStageRefs: S.Array(S.String),
  resourceModeCaveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  stageSummaryRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export class ArtanisPylonV02LaunchCommunicationUnsafe extends S.TaggedErrorClass<ArtanisPylonV02LaunchCommunicationUnsafe>()(
  'ArtanisPylonV02LaunchCommunicationUnsafe',
  {
    reason: S.String,
  },
) {}

const requiredCapabilityRefs = [
  'capability.public.pylon.inference',
  'capability.public.pylon.optimization',
  'capability.public.pylon.fine_tuning_training',
  'capability.public.pylon.validation',
  'capability.public.pylon.accepted_work_contribution',
  'capability.public.pylon.marketplace_jobs_planned',
] as const

const requiredStageRefs = [
  'stage.public.pylon_v0_2.source_ready',
  'stage.public.pylon_v0_2.release_ready',
  'stage.public.pylon_v0_2.platform_ready',
  'stage.public.pylon_v0_2.eligible',
  'stage.public.pylon_v0_2.accepted',
  'stage.public.pylon_v0_2.paid',
  'stage.public.pylon_v0_2.settled',
] as const

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const unsafeCopyPattern =
  /(Pylon v0\.2 is publicly released|Pylon v0\.2 is ready for (all users|everyone)|general availability|run Pylon (and|to) (earn|get paid|make money)|earn money|guaranteed|online Pylons? are eligible|accepted work is already paid|paid work is settled|send .*secret|share .*recovery phrase|share .*private key|share .*preimage|wallet seed|wallet secret|bearer [A-Za-z0-9._-]+|[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,})/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUrlPattern =
  /^https:\/\/openagents\.com\/(artanis|docs\/[A-Za-z0-9._~:/-]+|forum\/t\/[A-Za-z0-9-]+)$/

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
    throw new ArtanisPylonV02LaunchCommunicationUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, customer, private repo, or raw timestamp material.`,
    })
  }
}

const assertSafeUrls = (
  label: string,
  urls: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(urls).find(url =>
    !publicUrlPattern.test(url) ||
    unsafeRefPattern.test(url) ||
    rawTimestampPattern.test(url)
  )

  if (unsafe !== undefined) {
    throw new ArtanisPylonV02LaunchCommunicationUnsafe({
      reason: `${label} must be a clean public OpenAgents URL without query strings, fragments, private refs, payment material, or raw timestamps.`,
    })
  }
}

const assertCopySafe = (label: string, copy: string): void => {
  if (
    copy.trim().length < 12 ||
    containsProviderSecretMaterial(copy) ||
    unsafeCopyPattern.test(copy) ||
    unsafeRefPattern.test(copy) ||
    rawTimestampPattern.test(copy)
  ) {
    throw new ArtanisPylonV02LaunchCommunicationUnsafe({
      reason:
        `${label} overclaims launch, earnings, payment, settlement, readiness, or contains private material.`,
    })
  }
}

const assertIncludes = (
  label: string,
  expected: ReadonlyArray<string>,
  actual: ReadonlyArray<string>,
): void => {
  const actualSet = new Set(actual)
  const missing = expected.filter(value => !actualSet.has(value))

  if (missing.length > 0) {
    throw new ArtanisPylonV02LaunchCommunicationUnsafe({
      reason: `${label} missing required refs: ${missing.join(', ')}.`,
    })
  }
}

const validateForumIntent = (
  intent: ArtanisForumPublicationIntentRecord,
  nowIso: string,
): void => {
  try {
    projectArtanisForumPublicationQueue({
      agentId: 'agent_artanis',
      caveatRefs: ['caveat.public.pylon_v0_2_launch_copy_is_gated'],
      createdAtIso: intent.createdAtIso,
      intents: [intent],
      queueRef: 'queue.public.artanis.pylon_v0_2_launch_communication',
      redactionPolicyRef: 'redaction.forum.public.v1',
      updatedAtIso: intent.updatedAtIso,
    }, nowIso)
  } catch (error) {
    if (error instanceof ArtanisForumPublicationUnsafe) {
      throw new ArtanisPylonV02LaunchCommunicationUnsafe({
        reason: error.reason,
      })
    }

    throw error
  }
}

const stageSummaryRefs = (
  readiness: ArtanisPylonV02ReadinessProjection,
): ReadonlyArray<string> =>
  readiness.gates.map(gate =>
    `stage_summary.public.pylon_v0_2.${gate.stage}.${gate.state.state}`
  )

const assertRecordSafe = (
  record: ArtanisPylonV02LaunchCommunicationRecord,
  readiness: ArtanisPylonV02ReadinessProjection,
  nowIso: string,
): void => {
  if (!Number.isFinite(Date.parse(record.updatedAtIso))) {
    throw new ArtanisPylonV02LaunchCommunicationUnsafe({
      reason: 'Artanis Pylon v0.2 launch communication updatedAtIso must be valid.',
    })
  }

  assertSafeRefs('Artanis Pylon v0.2 launch identity refs', [
    record.agentRef,
    record.launchPackageRef,
    record.primaryForumTopicRef,
    record.readinessRef,
  ])
  assertSafeRefs(
    'Artanis Pylon v0.2 authority boundary refs',
    record.authorityBoundaryRefs,
  )
  assertSafeRefs(
    'Artanis Pylon v0.2 capability refs',
    record.capabilityRefs,
  )
  assertSafeRefs('Artanis Pylon v0.2 docs refs', record.docsPageRefs)
  assertSafeRefs(
    'Artanis Pylon v0.2 owner setup refs',
    record.ownerSetupRefs,
  )
  assertSafeRefs(
    'Artanis Pylon v0.2 readiness stage refs',
    record.readinessStageRefs,
  )
  assertSafeRefs(
    'Artanis Pylon v0.2 resource mode caveat refs',
    record.resourceModeCaveatRefs,
  )
  assertSafeRefs('Artanis Pylon v0.2 source refs', record.sourceRefs)
  assertSafeUrls('Artanis page refs', record.artanisPageRefs)
  assertSafeUrls('Artanis primary Forum topic URL', [
    record.primaryForumTopicUrl,
  ])
  assertCopySafe('Pylon v0.2 launch brief', record.briefMarkdown)
  assertCopySafe('Pylon v0.2 Forum post title', record.forumPostTitle)
  assertCopySafe('Pylon v0.2 Forum post body', record.forumIntent.bodyText)
  assertCopySafe('Pylon v0.2 optional social copy', record.optionalSocialCopy)
  validateForumIntent(record.forumIntent, nowIso)
  assertIncludes('Pylon v0.2 launch capabilities', requiredCapabilityRefs, [
    ...record.capabilityRefs,
  ])
  assertIncludes('Pylon v0.2 launch readiness stages', requiredStageRefs, [
    ...record.readinessStageRefs,
  ])

  if (record.agentRef !== 'agent_artanis') {
    throw new ArtanisPylonV02LaunchCommunicationUnsafe({
      reason: 'Pylon v0.2 launch communications must be administered by agent_artanis.',
    })
  }

  if (record.readinessRef !== readiness.readinessRef) {
    throw new ArtanisPylonV02LaunchCommunicationUnsafe({
      reason: 'Pylon v0.2 launch communication must reference the readiness projection it summarizes.',
    })
  }
}

export const projectArtanisPylonV02LaunchCommunication = (
  record: ArtanisPylonV02LaunchCommunicationRecord,
  readiness: ArtanisPylonV02ReadinessProjection,
  nowIso: string,
): ArtanisPylonV02LaunchCommunicationProjection => {
  assertRecordSafe(record, readiness, nowIso)

  return new ArtanisPylonV02LaunchCommunicationProjection({
    agentRef: record.agentRef,
    artanisPageRefs: uniqueRefs(record.artanisPageRefs),
    authorityBoundaryRefs: uniqueRefs(record.authorityBoundaryRefs),
    briefMarkdown: record.briefMarkdown.trim(),
    capabilityRefs: uniqueRefs(record.capabilityRefs),
    docsPageRefs: uniqueRefs(record.docsPageRefs),
    forumIntentReady:
      record.forumIntent.deliveryState === 'ready' &&
      record.forumIntent.targetTopicState === 'open',
    forumIntentRef: record.forumIntent.intentRef,
    forumPostBody: record.forumIntent.bodyText.trim(),
    forumPostTitle: record.forumPostTitle.trim(),
    launchPackageRef: record.launchPackageRef,
    optionalSocialCopy: record.optionalSocialCopy.trim(),
    ownerSetupRefs: uniqueRefs(record.ownerSetupRefs),
    primaryForumTopicRef: record.primaryForumTopicRef,
    primaryForumTopicUrl: record.primaryForumTopicUrl,
    readinessRef: record.readinessRef,
    readinessStageRefs: uniqueRefs(record.readinessStageRefs),
    resourceModeCaveatRefs: uniqueRefs(record.resourceModeCaveatRefs),
    sourceRefs: uniqueRefs(record.sourceRefs),
    stageSummaryRefs: stageSummaryRefs(readiness),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
  })
}

export const exampleArtanisPylonV02LaunchCommunicationRecord =
  (): ArtanisPylonV02LaunchCommunicationRecord =>
  new ArtanisPylonV02LaunchCommunicationRecord({
    agentRef: 'agent_artanis',
    artanisPageRefs: ['https://openagents.com/artanis'],
    authorityBoundaryRefs: [
      'authority.public.no_wallet_self_authorization',
      'authority.public.no_provider_self_authorization',
      'authority.public.no_training_self_authorization',
      'authority.public.no_settlement_self_authorization',
      'authority.public.no_runtime_promotion_self_authorization',
    ],
    briefMarkdown:
      'Artanis is preparing Pylon v0.2 communication for local compute work: inference, optimization, fine-tuning/training, validation, accepted-work contribution, and planned marketplace jobs. Current readiness is gated: source-ready is verified at source-contract level, release-ready and platform-ready are blocked, eligibility is planned, and accepted, paid, and settled claims require future public receipts. Use owner-approved setup only and keep local node material private.',
    capabilityRefs: [...requiredCapabilityRefs],
    docsPageRefs: [
      'docs/artanis/2026-06-06-pylon-v02-launch-readiness.md',
      'docs/sites/2026-06-05-pylon-local-compute-instruction-packet.md',
      'docs/sites/2026-06-05-pylon-v0-2-public-readiness-audit.md',
    ],
    forumIntent: new ArtanisForumPublicationIntentRecord({
      artifactRefs: ['artifact.public.pylon_v0_2.launch_communication'],
      authorAgentId: 'agent_artanis',
      blockerRefs: [
        'blocker.public.no_pylon_v0_2_release_asset',
        'blocker.public.platform_smokes_missing',
      ],
      bodyText:
        'Artanis Pylon update: Pylon is the local compute path for inference, optimization, fine-tuning/training, validation, accepted-work contribution, and planned marketplace jobs. Current readiness is gated. Source-ready is verified at the source-contract level; release-ready and platform-ready are blocked; eligibility is planned; accepted, paid, and settled claims require future public receipts. Use owner-approved setup only, run the readiness commands locally, and keep credentials or local node material out of public posts. Primary updates stay in this Artanis Pylon release work log.',
      caveatRefs: [
        'caveat.public.pylon_v0_2_not_broad_release_ready',
        'caveat.public.no_unconditional_earnings_claim',
        'caveat.public.online_is_not_eligible',
      ],
      createdAtIso: '2026-06-07T03:00:00.000Z',
      deliveredAtIso: null,
      deliveryReceiptRefs: [],
      deliveryState: 'ready',
      goalRefs: ['goal.public.artanis.pylon_v0_2_launch'],
      idempotencyKey: 'artanis:pylon-v0-2-launch-communication:v1',
      intentRef: 'intent.public.artanis.pylon_v0_2_launch_communication',
      modelLabReportRefs: [],
      pageUrls: [
        'https://openagents.com/artanis',
        'https://openagents.com/forum/t/88888888-4004-4004-8004-888888888888',
      ],
      postRef: null,
      pylonNexusPublicRefs: [
        'campaign.public.pylon.v0_2',
        'pylon.public.resource_modes',
        'pylon.public.v0_2_readiness',
      ],
      r10ClaimRefs: ['claim.public.r10.pylon_release'],
      receiptRefs: [],
      redactionPolicyRef: 'redaction.forum.public.v1',
      sourceRefs: [
        'campaign.public.pylon.v0_2',
        'pylon.public.v0_2_readiness',
      ],
      targetForumRef: 'forum.public.artanis',
      targetTopicRef: 'topic.public.forum.artanis.pylon_release_work_log',
      targetTopicState: 'open',
      updatedAtIso: '2026-06-07T03:00:00.000Z',
    }),
    forumPostTitle: 'Artanis Pylon v0.2 launch readiness update',
    launchPackageRef: 'launch.public.artanis.pylon_v0_2.communication',
    optionalSocialCopy:
      'Artanis is coordinating Pylon local-compute readiness for inference, optimization, fine-tuning/training, validation, accepted-work contribution, and planned marketplace jobs. Current status is gated: source-ready is verified, release/platform readiness are blocked, and payment or settlement claims require future receipts.',
    ownerSetupRefs: [
      'setup.public.owner_approved_local_agent',
      'docs/sites/2026-06-05-pylon-local-compute-instruction-packet.md',
    ],
    primaryForumTopicRef: 'topic.public.forum.artanis.pylon_release_work_log',
    primaryForumTopicUrl:
      'https://openagents.com/forum/t/88888888-4004-4004-8004-888888888888',
    readinessRef: 'readiness.public.artanis.pylon_v0_2',
    readinessStageRefs: [...requiredStageRefs],
    resourceModeCaveatRefs: [
      'caveat.public.resource_mode_background_may_not_be_enough',
      'caveat.public.resource_mode_overnight_owner_selected',
      'caveat.public.resource_mode_dedicated_requires_operator_intent',
    ],
    sourceRefs: [
      'docs/artanis/2026-06-06-pylon-v02-launch-readiness.md',
      'docs/sites/2026-06-05-pylon-local-compute-instruction-packet.md',
      'docs/sites/2026-06-05-pylon-v0-2-public-readiness-audit.md',
    ],
    updatedAtIso: '2026-06-07T03:00:00.000Z',
  })

export const exampleArtanisPylonV02LaunchCommunicationProjection = (
  nowIso: string,
): ArtanisPylonV02LaunchCommunicationProjection =>
  projectArtanisPylonV02LaunchCommunication(
    exampleArtanisPylonV02LaunchCommunicationRecord(),
    projectArtanisPylonV02Readiness(
      exampleArtanisPylonV02Readiness(),
      'public',
      nowIso,
    ),
    nowIso,
  )
