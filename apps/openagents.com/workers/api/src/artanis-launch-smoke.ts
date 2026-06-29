import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  ArtanisForumPublicationQueueRecord,
  exampleArtanisForumPublicationQueue,
  projectArtanisForumPublicationQueue,
} from './artanis-forum-publication'
import {
  ArtanisLoopLedgerRecord,
  exampleArtanisLoopLedger,
  projectArtanisLoopLedger,
} from './artanis-loop'
import {
  ArtanisOperatorSteeringWorkspaceRecord,
  exampleArtanisOperatorSteeringWorkspace,
  projectArtanisOperatorSteeringWorkspace,
} from './artanis-operator-steering'
import {
  ArtanisPublicReport,
  artanisPublicReportSnapshot,
} from './artanis-public-report'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { publicPylonStatsFromNexusPayload } from './public-pylon-stats'
import { currentIsoTimestamp } from './runtime-primitives'

export const ArtanisLaunchSmokeStage = S.Literals([
  'operator_goal',
  'loop_claim',
  'safe_result',
  'forum_post',
  'public_summary',
])
export type ArtanisLaunchSmokeStage = typeof ArtanisLaunchSmokeStage.Type

export class ArtanisLaunchSmokeStageProjection extends S.Class<ArtanisLaunchSmokeStageProjection>(
  'ArtanisLaunchSmokeStageProjection',
)({
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  label: S.String,
  receiptRefs: S.Array(S.String),
  ref: S.String,
  stage: ArtanisLaunchSmokeStage,
}) {}

export class ArtanisLaunchSmokeInput extends S.Class<ArtanisLaunchSmokeInput>(
  'ArtanisLaunchSmokeInput',
)({
  forumQueue: ArtanisForumPublicationQueueRecord,
  loopLedger: ArtanisLoopLedgerRecord,
  operatorWorkspace: ArtanisOperatorSteeringWorkspaceRecord,
  publicReport: ArtanisPublicReport,
  smokeRef: S.String,
  updatedAtIso: S.String,
}) {}

export class ArtanisLaunchSmokeProjection extends S.Class<ArtanisLaunchSmokeProjection>(
  'ArtanisLaunchSmokeProjection',
)({
  agentId: S.String,
  artifactRefs: S.Array(S.String),
  blockedBeforeAuthorityRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  forumIntentRef: S.String,
  forumPostRef: S.String,
  forumTopicRef: S.String,
  goalRef: S.String,
  loopRef: S.String,
  publicSummaryRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  safeActionRef: S.String,
  smokeRef: S.String,
  stages: S.Array(ArtanisLaunchSmokeStageProjection),
  tickRef: S.String,
  updatedAtDisplay: S.String,
}) {}

export class ArtanisLaunchSmokeUnsafe extends S.TaggedErrorClass<ArtanisLaunchSmokeUnsafe>()(
  'ArtanisLaunchSmokeUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

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
    throw new ArtanisLaunchSmokeUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, customer, private repo, or raw timestamp material.`,
    })
  }
}

const assertProjectionSafe = (
  projection: ArtanisLaunchSmokeProjection,
): void => {
  const serialized = JSON.stringify(projection)

  if (
    containsProviderSecretMaterial(serialized) ||
    unsafeRefPattern.test(serialized) ||
    rawTimestampPattern.test(serialized)
  ) {
    throw new ArtanisLaunchSmokeUnsafe({
      reason: 'Artanis launch smoke projection contains private or raw material.',
    })
  }
}

const firstOrThrow = <A>(
  label: string,
  value: A | undefined,
): A => {
  if (value === undefined) {
    throw new ArtanisLaunchSmokeUnsafe({ reason: label })
  }

  return value
}

export const projectArtanisLaunchSmoke = (
  input: ArtanisLaunchSmokeInput,
  nowIso = currentIsoTimestamp(),
): ArtanisLaunchSmokeProjection => {
  assertSafeRefs('Artanis launch smoke refs', [
    input.smokeRef,
  ])

  const operator = projectArtanisOperatorSteeringWorkspace(
    input.operatorWorkspace,
    'public_artanis',
    nowIso,
  )
  const loopLedger = projectArtanisLoopLedger(
    input.loopLedger,
    'public',
    nowIso,
  )
  const forum = projectArtanisForumPublicationQueue(input.forumQueue, nowIso)
  const publicReport = input.publicReport
  const createGoalCommand = firstOrThrow(
    'Artanis launch smoke requires an accepted or completed operator create-goal command.',
    operator.goalCommands.find(command =>
      command.kind === 'create_goal' &&
      (command.state === 'accepted' || command.state === 'completed')
    ),
  )
  const loop = firstOrThrow(
    'Artanis launch smoke requires an active Artanis loop for the operator goal.',
    loopLedger.loops.find(candidate =>
      candidate.active &&
      candidate.goalRefs.includes(createGoalCommand.goalRef)
    ),
  )
  const tick = firstOrThrow(
    'Artanis launch smoke requires a completed loop tick for the operator goal.',
    loop.ticks.find(candidate =>
      candidate.goalRef === createGoalCommand.goalRef &&
      candidate.state === 'completed'
    ),
  )
  const safeAction = firstOrThrow(
    'Artanis launch smoke requires a safe status-projection action result.',
    tick.actionProposals.find(action =>
      action.kind === 'status_projection' && action.risk === 'safe'
    ),
  )
  const forumIntent = firstOrThrow(
    'Artanis launch smoke requires a delivered public Artanis Forum post.',
    forum.intents.find(intent =>
      intent.deliveryState === 'delivered' &&
      intent.goalRefs.includes(createGoalCommand.goalRef) &&
      tick.forumPublicationIntentRefs.includes(intent.intentRef) &&
      intent.postRef !== null
    ),
  )

  if (!publicReport.publicUrls.includes('https://openagents.com/artanis')) {
    throw new ArtanisLaunchSmokeUnsafe({
      reason: 'Artanis launch smoke requires the public report to reference /artanis.',
    })
  }

  if (
    !publicReport.forumLinks.some(link => link.href.startsWith('/forum/f/artanis'))
  ) {
    throw new ArtanisLaunchSmokeUnsafe({
      reason: 'Artanis launch smoke requires /artanis to link the Artanis Forum.',
    })
  }

  if (
    !publicReport.forumLinks.some(link =>
      link.label === 'Latest status post' &&
      link.topicRef === forumIntent.targetTopicRef
    )
  ) {
    throw new ArtanisLaunchSmokeUnsafe({
      reason:
        'Artanis launch smoke requires /artanis to link the delivered canonical status post.',
    })
  }

  const receiptRefs = uniqueRefs([
    ...createGoalCommand.operatorReceiptRefs,
    ...tick.receiptRefs,
    ...tick.closeoutReceiptRefs,
    ...forumIntent.receiptRefs,
    ...forumIntent.deliveryReceiptRefs,
    ...publicReport.receiptRefs,
  ])
  const artifactRefs = uniqueRefs([
    ...tick.artifactRefs,
    ...forumIntent.artifactRefs,
    ...publicReport.artifactRefs,
  ])
  const blockedBeforeAuthorityRefs = uniqueRefs([
    ...publicReport.publicBlockerRefs,
    'blocker.artanis.live_spend_requires_operator_gate',
    'blocker.artanis.provider_mutation_requires_authority',
    'blocker.artanis.runtime_promotion_requires_release_gate',
    'blocker.artanis.settlement_requires_public_receipts',
  ])
  const publicSummaryRefs = uniqueRefs([
    'route:/api/public/artanis/report',
    'https://openagents.com/artanis',
    ...publicReport.forumLinks.map(link => link.href),
    ...publicReport.publicUrls,
  ])
  const stages: ReadonlyArray<ArtanisLaunchSmokeStageProjection> = [
    {
      artifactRefs: [],
      blockerRefs: createGoalCommand.blockerRefs,
      caveatRefs: createGoalCommand.caveatRefs,
      label: 'Operator goal accepted',
      receiptRefs: createGoalCommand.operatorReceiptRefs,
      ref: createGoalCommand.commandRef,
      stage: 'operator_goal',
    },
    {
      artifactRefs: [],
      blockerRefs: loop.blockerRefs,
      caveatRefs: loop.caveatRefs,
      label: 'Loop claimed goal',
      receiptRefs: tick.receiptRefs,
      ref: loop.loopRef,
      stage: 'loop_claim',
    },
    {
      artifactRefs: safeAction.artifactRefs,
      blockerRefs: [],
      caveatRefs: safeAction.caveatRefs,
      label: 'Safe status result recorded',
      receiptRefs: tick.closeoutReceiptRefs,
      ref: safeAction.actionRef,
      stage: 'safe_result',
    },
    {
      artifactRefs: forumIntent.artifactRefs,
      blockerRefs: forumIntent.blockerRefs,
      caveatRefs: forumIntent.caveatRefs,
      label: 'Forum update delivered',
      receiptRefs: forumIntent.deliveryReceiptRefs,
      ref: forumIntent.postRef ?? 'post.public.artanis.missing',
      stage: 'forum_post',
    },
    {
      artifactRefs: publicReport.artifactRefs,
      blockerRefs: publicReport.publicBlockerRefs,
      caveatRefs: publicReport.publicCaveatRefs,
      label: 'Public Artanis summary linked',
      receiptRefs: publicReport.receiptRefs,
      ref: publicReport.reportRef,
      stage: 'public_summary',
    },
  ]
  const projection: ArtanisLaunchSmokeProjection = {
    agentId: 'agent_artanis',
    artifactRefs,
    blockedBeforeAuthorityRefs,
    caveatRefs: uniqueRefs([
      ...operator.caveatRefs,
      ...loop.caveatRefs,
      ...forum.caveatRefs,
      ...publicReport.publicCaveatRefs,
    ]),
    forumIntentRef: forumIntent.intentRef,
    forumPostRef: forumIntent.postRef ?? 'post.public.artanis.missing',
    forumTopicRef: forumIntent.targetTopicRef,
    goalRef: createGoalCommand.goalRef,
    loopRef: loop.loopRef,
    publicSummaryRefs,
    receiptRefs,
    safeActionRef: safeAction.actionRef,
    smokeRef: input.smokeRef,
    stages: [...stages],
    tickRef: tick.tickRef,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      input.updatedAtIso,
      nowIso,
    ),
  }

  assertProjectionSafe(projection)

  return projection
}

const deliveredForumQueue = (): ArtanisForumPublicationQueueRecord => {
  const queue = exampleArtanisForumPublicationQueue()

  return {
    ...queue,
    intents: queue.intents.map(intent => ({
      ...intent,
      deliveredAtIso: '2026-06-07T01:24:00.000Z',
      deliveryReceiptRefs: ['receipt.public.artanis.forum_status_delivered'],
      deliveryState: 'delivered',
      intentRef: 'forum.public.artanis.status_intent',
      postRef: 'post.public.forum.artanis.status.20260607T0124',
      updatedAtIso: '2026-06-07T01:24:00.000Z',
    })),
    updatedAtIso: '2026-06-07T01:24:00.000Z',
  }
}

export const exampleArtanisLaunchSmokeInput = (): ArtanisLaunchSmokeInput =>
  new ArtanisLaunchSmokeInput({
    forumQueue: deliveredForumQueue(),
    loopLedger: exampleArtanisLoopLedger(),
    operatorWorkspace: exampleArtanisOperatorSteeringWorkspace,
    publicReport: artanisPublicReportSnapshot({
      forumPublicationQueue: deliveredForumQueue(),
      nowIso: '2026-06-07T01:30:00.000Z',
      pylonStats: publicPylonStatsFromNexusPayload({
        hosted_nexus_relay_url: 'wss://nexus.openagents.com/',
        nexus_accepted_work_payout_sats_paid_24h: 0,
        nexus_accepted_work_payout_sats_paid_total: 0,
        pylon_sessions_online_now: 4,
        pylons_online_now: 3,
        recent_pylons: [],
        sellable_pylons_online_now: 2,
        training_accepted_contributors: 1,
        training_assigned_contributors: 2,
      }),
    }),
    smokeRef: 'smoke.public.artanis.launch_e2e',
    updatedAtIso: '2026-06-07T01:30:00.000Z',
  })
