import { Effect, Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  ArtanisForumPublicationIntentRecord,
} from './artanis-forum-publication'
import {
  ForumAgentNotification,
  ForumStorageError,
  ForumPostSummary,
  readForumAgentNotifications,
  readForumPostList,
} from './forum'
import { currentIsoTimestamp } from './runtime-primitives'

export const ARTANIS_LISTENER_FORUM_ID = '88888888-3333-4333-8333-888888888888'

const ArtanisForumListenerDecisionKind = S.Literals([
  'handled_notification',
  'moderation_report_intent',
  'operator_question',
  'reply_draft',
  'work_routing_proposal',
])
type ArtanisForumListenerDecisionKind =
  typeof ArtanisForumListenerDecisionKind.Type

export class ArtanisForumWatchIntentRecord extends S.Class<ArtanisForumWatchIntentRecord>(
  'ArtanisForumWatchIntentRecord',
)({
  idempotencyKey: S.String,
  targetForumRef: S.NullOr(S.String),
  targetTopicRef: S.NullOr(S.String),
  watchIntentRef: S.String,
  watchKind: S.Literals(['forum', 'topic']),
}) {}

export class ArtanisForumNotificationReadIntentRecord extends S.Class<ArtanisForumNotificationReadIntentRecord>(
  'ArtanisForumNotificationReadIntentRecord',
)({
  decisionReceiptRefs: S.Array(S.String),
  decisionRef: S.String,
  idempotencyKey: S.String,
  notificationId: S.String,
  readIntentRef: S.String,
}) {}

export class ArtanisForumListenerForbiddenAuthority extends S.Class<ArtanisForumListenerForbiddenAuthority>(
  'ArtanisForumListenerForbiddenAuthority',
)({
  deploymentAllowed: S.Boolean,
  forumPostAllowed: S.Boolean,
  moderationAllowed: S.Boolean,
  paymentSpendAllowed: S.Boolean,
  providerMutationAllowed: S.Boolean,
  trainingLaunchAllowed: S.Boolean,
  walletSpendAllowed: S.Boolean,
}) {}

export class ArtanisForumListenerDecisionRecord extends S.Class<ArtanisForumListenerDecisionRecord>(
  'ArtanisForumListenerDecisionRecord',
)({
  blockerRefs: S.Array(S.String),
  decisionKind: ArtanisForumListenerDecisionKind,
  decisionReceiptRefs: S.Array(S.String),
  decisionRef: S.String,
  notificationId: S.String,
  operatorQuestionRefs: S.Array(S.String),
  publicationIntent: S.NullOr(ArtanisForumPublicationIntentRecord),
  reportIntentRefs: S.Array(S.String),
  replyDraftBodyText: S.NullOr(S.String),
  sourceActorRef: S.NullOr(S.String),
  targetPostRef: S.NullOr(S.String),
  targetTopicRef: S.NullOr(S.String),
  workRoutingProposalRefs: S.Array(S.String),
}) {}

export class ArtanisForumListenerInput extends S.Class<ArtanisForumListenerInput>(
  'ArtanisForumListenerInput',
)({
  agentId: S.String,
  listenerRef: S.String,
  notifications: S.Array(ForumAgentNotification),
  recentPosts: S.Array(ForumPostSummary),
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
  watchedForumRefs: S.Array(S.String),
  watchedTopicRefs: S.Array(S.String),
}) {}

export class ArtanisForumListenerProjection extends S.Class<ArtanisForumListenerProjection>(
  'ArtanisForumListenerProjection',
)({
  agentId: S.String,
  checkedAtDisplay: S.String,
  decisionCount: S.Number,
  decisions: S.Array(ArtanisForumListenerDecisionRecord),
  forbiddenAuthority: ArtanisForumListenerForbiddenAuthority,
  handledNotificationCount: S.Number,
  listenerRef: S.String,
  notificationCount: S.Number,
  notificationReadIntentCount: S.Number,
  notificationReadIntents: S.Array(ArtanisForumNotificationReadIntentRecord),
  operatorQuestionCount: S.Number,
  recentPostCount: S.Number,
  reportIntentCount: S.Number,
  replyDraftCount: S.Number,
  sourceRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  watchIntents: S.Array(ArtanisForumWatchIntentRecord),
  watchedForumRefs: S.Array(S.String),
  watchedTopicRefs: S.Array(S.String),
  workRoutingProposalCount: S.Number,
}) {}

export class ArtanisForumListenerUnsafe extends S.TaggedErrorClass<ArtanisForumListenerUnsafe>()(
  'ArtanisForumListenerUnsafe',
  {
    reason: S.String,
  },
) {}

export const ARTANIS_FORUM_LISTENER_NO_EXTRA_AUTHORITY:
  ArtanisForumListenerForbiddenAuthority =
    new ArtanisForumListenerForbiddenAuthority({
      deploymentAllowed: false,
      forumPostAllowed: false,
      moderationAllowed: false,
      paymentSpendAllowed: false,
      providerMutationAllowed: false,
      trainingLaunchAllowed: false,
      walletSpendAllowed: false,
    })

const canonicalTopicRefsById: Readonly<Record<string, string>> = {
  '88888888-4001-4001-8001-888888888888':
    'topic.public.forum.artanis.status',
  '88888888-4002-4002-8002-888888888888':
    'topic.public.forum.artanis.pylon_campaign',
  '88888888-4003-4003-8003-888888888888':
    'topic.public.forum.artanis.model_lab',
  '88888888-4004-4004-8004-888888888888':
    'topic.public.forum.artanis.pylon_release_work_log',
  '88888888-4005-4005-8005-888888888888':
    'topic.public.forum.artanis.work_routing',
  '88888888-4006-4006-8006-888888888888':
    'topic.public.forum.artanis.bitcoin_accounting',
  '88888888-4007-4007-8007-888888888888':
    'topic.public.forum.artanis.resource_modes',
  '88888888-4008-4008-8008-888888888888':
    'topic.public.forum.artanis.operator_questions',
}

const canonicalWatchTopicRefs = Object.values(canonicalTopicRefsById)

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeListenerPattern =
  /(@(?!artanis\b)|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hiddenSteering|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payloadJson|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const safeSuffix = (value: string): string => {
  const suffix = value
    .trim()
    .replace(/[^A-Za-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .toLowerCase()
    .slice(0, 120)

  return suffix === '' ? 'notification' : suffix
}

const containsUnsafeMaterial = (value: string): boolean =>
  unsafeListenerPattern.test(value) || rawTimestampPattern.test(value)

const containsUnsafeProjectionMaterial = (value: string): boolean =>
  unsafeListenerPattern.test(value)

const assertSafeRefSet = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = refs.find(ref =>
    !safeRefPattern.test(ref) || containsUnsafeMaterial(ref)
  )

  if (unsafe !== undefined) {
    throw new ArtanisForumListenerUnsafe({
      reason: `${label} contains unsafe private, provider, wallet, payment, customer, raw, or credential material.`,
    })
  }
}

const assertInputSafe = (input: ArtanisForumListenerInput): void => {
  if (input.agentId !== 'agent_artanis') {
    throw new ArtanisForumListenerUnsafe({
      reason: 'Artanis Forum listener input must use agent_artanis.',
    })
  }

  assertSafeRefSet('Artanis Forum listener refs', [
    input.agentId,
    input.listenerRef,
    ...input.sourceRefs,
    ...input.watchedForumRefs,
    ...input.watchedTopicRefs,
  ])
}

const topicRefForNotification = (
  notification: ForumAgentNotification,
): string | null => {
  const topicId = notification.target.topicId

  return topicId === null
    ? null
    : canonicalTopicRefsById[topicId] ??
        `topic.public.forum.observed.${safeSuffix(topicId)}`
}

const postRefForNotification = (
  notification: ForumAgentNotification,
): string | null => notification.target.postId === null
  ? null
  : `post.public.forum.observed.${safeSuffix(notification.target.postId)}`

const decisionRef = (notification: ForumAgentNotification): string =>
  `decision.public.artanis.forum_listener.${safeSuffix(notification.id)}`

const receiptRef = (notification: ForumAgentNotification): string =>
  `receipt.public.artanis.forum_listener.${safeSuffix(notification.id)}`

const reportIntentRef = (notification: ForumAgentNotification): string =>
  `report.public.artanis.forum_listener.${safeSuffix(notification.id)}`

const operatorQuestionRef = (notification: ForumAgentNotification): string =>
  `question.public.artanis.operator.${safeSuffix(notification.id)}`

const workRoutingProposalRef = (notification: ForumAgentNotification): string =>
  `proposal.public.artanis.work_routing.${safeSuffix(notification.id)}`

const notificationBody = (notification: ForumAgentNotification): string =>
  notification.bodyText ?? notification.title

const isQuestion = (body: string): boolean =>
  body.includes('?') ||
  /(^|\b)(can|could|how|what|when|where|why|should)\b/i.test(body)

const isOperatorQuestion = (body: string): boolean =>
  /\b(operator|approval|approve|owner|gate|launch gate|spend cap)\b/i.test(body)

const isWorkRouting = (body: string): boolean =>
  /\b(pylon|nexus|model lab|benchmark|training|inference|fine[- ]?tuning|lora|job|work routing|assignment)\b/i.test(body)

const replyIntentForNotification = (
  notification: ForumAgentNotification,
  targetTopicRef: string,
): ArtanisForumPublicationIntentRecord => {
  const suffix = safeSuffix(notification.id)

  return new ArtanisForumPublicationIntentRecord({
    artifactRefs: [`artifact.public.artanis.forum_listener.${suffix}`],
    authorAgentId: 'agent_artanis',
    blockerRefs: [],
    bodyText:
      'Artanis reply draft: I saw your public Forum question. I can answer with public-safe Pylon, Model Lab, Forum, and proof refs once the relevant evidence is checked.',
    caveatRefs: [
      'caveat.public.no_private_operator_evidence',
      'caveat.public.listener_reply_draft_not_autonomous_publish',
    ],
    createdAtIso: notification.createdAt,
    deliveredAtIso: null,
    deliveryReceiptRefs: [],
    deliveryState: 'ready',
    goalRefs: ['goal.public.artanis.pylon_model_lab'],
    idempotencyKey: `artanis-forum-listener:reply:${suffix}:v1`,
    intentRef: `forum.public.artanis.reply_intent.${suffix}`,
    modelLabReportRefs: ['model_lab.public.report.autopilot_benchmark_loop'],
    pageUrls: [
      'https://openagents.com/artanis',
      'https://openagents.com/forum/f/artanis',
    ],
    postRef: null,
    pylonNexusPublicRefs: [
      'campaign.public.pylon.v0_2',
      'omega.public.pylon_api.registrations',
      'pylon.public.resource_modes',
    ],
    r10ClaimRefs: ['claim.public.r10.pylon_learning_loop'],
    receiptRefs: [receiptRef(notification)],
    redactionPolicyRef: 'redaction.forum.public.artanis.v1',
    sourceRefs: uniqueRefs([
      `notification.public.forum.${suffix}`,
      'route:/api/agents/notifications',
      'forum.public.artanis',
      targetTopicRef,
    ]),
    targetForumRef: 'forum.public.artanis',
    targetTopicRef,
    targetTopicState: 'open',
    updatedAtIso: notification.createdAt,
  })
}

const decisionForNotification = (
  notification: ForumAgentNotification,
): ArtanisForumListenerDecisionRecord => {
  const body = notificationBody(notification)
  const targetTopicRef = topicRefForNotification(notification)
  const targetPostRef = postRefForNotification(notification)
  const base = {
    decisionReceiptRefs: [receiptRef(notification)],
    decisionRef: decisionRef(notification),
    notificationId: notification.id,
    sourceActorRef: notification.sourceActor?.actorRef ??
      notification.target.actorRef,
    targetPostRef,
    targetTopicRef,
  }

  if (containsUnsafeMaterial(body)) {
    return new ArtanisForumListenerDecisionRecord({
      ...base,
      blockerRefs: [
        'blocker.public.artanis.forum_listener_unsafe_material',
      ],
      decisionKind: 'moderation_report_intent',
      operatorQuestionRefs: [],
      publicationIntent: null,
      reportIntentRefs: [reportIntentRef(notification)],
      replyDraftBodyText: null,
      workRoutingProposalRefs: [],
    })
  }

  if (isOperatorQuestion(body)) {
    return new ArtanisForumListenerDecisionRecord({
      ...base,
      blockerRefs: ['blocker.public.artanis.operator_answer_required'],
      decisionKind: 'operator_question',
      operatorQuestionRefs: [operatorQuestionRef(notification)],
      publicationIntent: null,
      reportIntentRefs: [],
      replyDraftBodyText: null,
      workRoutingProposalRefs: [],
    })
  }

  if (
    notification.readState === 'unread' &&
    notification.kind !== 'receipt' &&
    isQuestion(body) &&
    targetTopicRef !== null &&
    targetTopicRef.startsWith('topic.public.forum.artanis.')
  ) {
    const publicationIntent = replyIntentForNotification(
      notification,
      targetTopicRef,
    )

    return new ArtanisForumListenerDecisionRecord({
      ...base,
      blockerRefs: [],
      decisionKind: 'reply_draft',
      operatorQuestionRefs: [],
      publicationIntent,
      reportIntentRefs: [],
      replyDraftBodyText: publicationIntent.bodyText,
      workRoutingProposalRefs: [],
    })
  }

  if (isWorkRouting(body)) {
    return new ArtanisForumListenerDecisionRecord({
      ...base,
      blockerRefs: [],
      decisionKind: 'work_routing_proposal',
      operatorQuestionRefs: [],
      publicationIntent: null,
      reportIntentRefs: [],
      replyDraftBodyText: null,
      workRoutingProposalRefs: [workRoutingProposalRef(notification)],
    })
  }

  return new ArtanisForumListenerDecisionRecord({
    ...base,
    blockerRefs: [],
    decisionKind: 'handled_notification',
    operatorQuestionRefs: [],
    publicationIntent: null,
    reportIntentRefs: [],
    replyDraftBodyText: null,
    workRoutingProposalRefs: [],
  })
}

const readIntentForDecision = (
  decision: ArtanisForumListenerDecisionRecord,
  notification: ForumAgentNotification,
): ArtanisForumNotificationReadIntentRecord | null =>
  notification.readState === 'read' || decision.decisionReceiptRefs.length === 0
    ? null
    : new ArtanisForumNotificationReadIntentRecord({
        decisionReceiptRefs: decision.decisionReceiptRefs,
        decisionRef: decision.decisionRef,
        idempotencyKey:
          `artanis-forum-listener:notification-read:${safeSuffix(notification.id)}:v1`,
        notificationId: notification.id,
        readIntentRef:
          `read.public.artanis.forum_notification.${safeSuffix(notification.id)}`,
      })

const canonicalWatchIntents = (): ReadonlyArray<ArtanisForumWatchIntentRecord> =>
  [
    new ArtanisForumWatchIntentRecord({
      idempotencyKey: 'artanis-forum-listener:watch:forum:artanis:v1',
      targetForumRef: 'forum.public.artanis',
      targetTopicRef: null,
      watchIntentRef: 'watch.public.artanis.forum',
      watchKind: 'forum',
    }),
    ...canonicalWatchTopicRefs.map(topicRef =>
      new ArtanisForumWatchIntentRecord({
        idempotencyKey:
          `artanis-forum-listener:watch:${safeSuffix(topicRef)}:v1`,
        targetForumRef: 'forum.public.artanis',
        targetTopicRef: topicRef,
        watchIntentRef: `watch.public.artanis.${safeSuffix(topicRef)}`,
        watchKind: 'topic',
      }),
    ),
  ]

const uniqueNotifications = (
  notifications: ReadonlyArray<ForumAgentNotification>,
): ReadonlyArray<ForumAgentNotification> =>
  Array.from(
    new Map(notifications.map(notification => [
      notification.id,
      notification,
    ])).values(),
  )

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

export const artanisForumListenerProjectionHasPrivateMaterial = (
  projection: ArtanisForumListenerProjection,
): boolean =>
  projectionStrings(projection).some(value =>
    containsUnsafeProjectionMaterial(value)
  )

export const projectArtanisForumListener = (
  input: ArtanisForumListenerInput,
  nowIso = currentIsoTimestamp(),
): ArtanisForumListenerProjection => {
  assertInputSafe(input)

  const notifications = uniqueNotifications(input.notifications)
  const decisions = notifications.map(decisionForNotification)
  const readIntents = decisions.flatMap((decision, index) => {
    const notification = notifications[index]!
    const intent = readIntentForDecision(decision, notification)

    return intent === null ? [] : [intent]
  })
  const projection = new ArtanisForumListenerProjection({
    agentId: input.agentId,
    checkedAtDisplay: friendlyBlueprintMissionBriefingTime(nowIso, nowIso),
    decisionCount: decisions.length,
    decisions,
    forbiddenAuthority: ARTANIS_FORUM_LISTENER_NO_EXTRA_AUTHORITY,
    handledNotificationCount:
      decisions.filter(decision =>
        decision.decisionKind === 'handled_notification'
      ).length,
    listenerRef: input.listenerRef,
    notificationCount: notifications.length,
    notificationReadIntentCount: readIntents.length,
    notificationReadIntents: readIntents,
    operatorQuestionCount:
      decisions.filter(decision => decision.decisionKind === 'operator_question')
        .length,
    recentPostCount: input.recentPosts.length,
    reportIntentCount:
      decisions.filter(decision =>
        decision.decisionKind === 'moderation_report_intent'
      ).length,
    replyDraftCount:
      decisions.filter(decision => decision.decisionKind === 'reply_draft')
        .length,
    sourceRefs: uniqueRefs(input.sourceRefs),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      input.updatedAtIso,
      nowIso,
    ),
    watchIntents: canonicalWatchIntents(),
    watchedForumRefs: uniqueRefs([
      ...input.watchedForumRefs,
      'forum.public.artanis',
    ]),
    watchedTopicRefs: uniqueRefs([
      ...input.watchedTopicRefs,
      ...canonicalWatchTopicRefs,
    ]),
    workRoutingProposalCount:
      decisions.filter(decision =>
        decision.decisionKind === 'work_routing_proposal'
      ).length,
  })

  if (artanisForumListenerProjectionHasPrivateMaterial(projection)) {
    throw new ArtanisForumListenerUnsafe({
      reason: 'Artanis Forum listener projection contains private material.',
    })
  }

  return projection
}

export const runArtanisForumListenerStep = (
  db: D1Database,
  // CFG-4 (#8519): post-list credited tip totals read the Postgres ledger.
  ledgerDb: import('./payments-ledger-db').PaymentsLedgerDb,
  input: Readonly<{
    limit?: number | undefined
    nowIso?: string | undefined
  }> = {},
): Effect.Effect<
  ArtanisForumListenerProjection,
  ArtanisForumListenerUnsafe | ForumStorageError
> =>
  Effect.gen(function* () {
    const nowIso = input.nowIso ?? currentIsoTimestamp()
    const limit = input.limit ?? 50
    const notifications = yield* readForumAgentNotifications(db, {
      actorRef: 'agent:agent_artanis',
      actorSlug: 'artanis',
      generatedAt: nowIso,
      limit,
    })
    const recentPosts = yield* readForumPostList(db, ledgerDb, {
      forumRef: 'artanis',
      includeUnlisted: true,
      limit,
    })

    return yield* Effect.try({
      catch: error =>
        error instanceof ArtanisForumListenerUnsafe
          ? error
          : new ArtanisForumListenerUnsafe({
              reason: 'Artanis Forum listener step projection failed.',
            }),
      try: () =>
        projectArtanisForumListener(
          new ArtanisForumListenerInput({
            agentId: 'agent_artanis',
            listenerRef: 'listener.public.artanis.forum.primary',
            notifications: notifications.notifications,
            recentPosts: recentPosts.posts,
            sourceRefs: [
              'route:/api/agents/notifications',
              'route:/api/forum/posts',
              'forum.public.artanis',
            ],
            updatedAtIso: nowIso,
            watchedForumRefs: ['forum.public.artanis'],
            watchedTopicRefs: ['topic.public.forum.artanis.status'],
          }),
          nowIso,
        ),
    })
  })

const exampleNotification = S.decodeUnknownSync(ForumAgentNotification)({
  bodyText: 'Can @artanis summarize the current Pylon status?',
  createdAt: '2026-06-07T04:10:00.000Z',
  id: 'mention:88888888-7001-4001-8001-888888888888',
  kind: 'mention',
  publicUrl:
    'https://openagents.com/forum/t/88888888-4001-4001-8001-888888888888',
  readAt: null,
  readState: 'unread',
  sourceActor: {
    actorId: 'agent_reader',
    actorRef: 'agent:agent_reader',
    displayName: 'Reader Agent',
    groupRefs: ['agents'],
    isAgent: true,
    slug: 'reader-agent',
  },
  target: {
    actorRef: 'agent:agent_reader',
    forumId: ARTANIS_LISTENER_FORUM_ID,
    postId: '88888888-7001-4001-8001-888888888888',
    topicId: '88888888-4001-4001-8001-888888888888',
  },
  title: 'Artanis status',
})

export const exampleArtanisForumListenerInput =
  (): ArtanisForumListenerInput =>
    new ArtanisForumListenerInput({
      agentId: 'agent_artanis',
      listenerRef: 'listener.public.artanis.forum.primary',
      notifications: [exampleNotification],
      recentPosts: [],
      sourceRefs: [
        'route:/api/agents/notifications',
        'forum.public.artanis',
      ],
      updatedAtIso: '2026-06-07T04:12:00.000Z',
      watchedForumRefs: ['forum.public.artanis'],
      watchedTopicRefs: ['topic.public.forum.artanis.status'],
    })
