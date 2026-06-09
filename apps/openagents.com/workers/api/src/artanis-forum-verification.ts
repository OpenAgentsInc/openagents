import { Schema as S } from 'effect'

import { ARTANIS_FORUM_REF } from './artanis-forum-delivery'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const ARTANIS_STATUS_TOPIC_REF =
  'topic.public.forum.artanis.status'
export const ARTANIS_PYLON_RELEASE_WORK_LOG_TOPIC_REF =
  'topic.public.forum.artanis.pylon_release_work_log'

export const ArtanisForumVerificationDeliveryState = S.Literals([
  'blocked',
  'delivered',
  'duplicate_collapsed',
  'target_blocked',
])
export type ArtanisForumVerificationDeliveryState =
  typeof ArtanisForumVerificationDeliveryState.Type

export const ArtanisForumVerificationListenerState = S.Literals([
  'blocked',
  'no_new_posts',
  'operator_question',
  'reply_draft',
  'work_routing',
])
export type ArtanisForumVerificationListenerState =
  typeof ArtanisForumVerificationListenerState.Type

export const ArtanisForumVerificationTopicState = S.Literals([
  'archived',
  'hidden',
  'locked',
  'open',
])
export type ArtanisForumVerificationTopicState =
  typeof ArtanisForumVerificationTopicState.Type

export class ArtanisForumVerificationAuthority extends S.Class<ArtanisForumVerificationAuthority>(
  'ArtanisForumVerificationAuthority',
)({
  acceptedWorkPayoutAllowed: S.Boolean,
  approvedDeliveryBridgeRequired: S.Boolean,
  directForumPublishAllowed: S.Boolean,
  dispatchAllowed: S.Boolean,
  moderationAllowed: S.Boolean,
  normalAgentPostingAllowed: S.Boolean,
  paymentSpendAllowed: S.Boolean,
  providerMutationAllowed: S.Boolean,
  publicClaimUpgradeAllowed: S.Boolean,
  schedulerEnablementAllowed: S.Boolean,
  walletSpendAllowed: S.Boolean,
}) {}

export class ArtanisForumVerificationRecord extends S.Class<ArtanisForumVerificationRecord>(
  'ArtanisForumVerificationRecord',
)({
  agentRef: S.String,
  authority: ArtanisForumVerificationAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  deliveredPostRefs: S.Array(S.String),
  deliveryReceiptRefs: S.Array(S.String),
  deliveryState: ArtanisForumVerificationDeliveryState,
  idempotencyRefs: S.Array(S.String),
  intendedPostRefs: S.Array(S.String),
  listenerNotificationRefs: S.Array(S.String),
  listenerState: ArtanisForumVerificationListenerState,
  noOpReadRefs: S.Array(S.String),
  operatorQuestionRefs: S.Array(S.String),
  privateEvidenceRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  targetForumRef: S.String,
  targetTopicRefs: S.Array(S.String),
  targetTopicState: ArtanisForumVerificationTopicState,
  triageDraftRefs: S.Array(S.String),
  updatedAtIso: S.String,
  verificationRef: S.String,
  workRoutingProposalRefs: S.Array(S.String),
}) {}

export class ArtanisForumVerificationProjection extends S.Class<ArtanisForumVerificationProjection>(
  'ArtanisForumVerificationProjection',
)({
  acceptedWorkPayoutAllowed: S.Boolean,
  agentRef: S.String,
  approvedDeliveryBridgeRequired: S.Boolean,
  audience: OmniProjectionAudience,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  deliveredPostCount: S.Number,
  deliveredPostRefs: S.Array(S.String),
  deliveryReceiptRefs: S.Array(S.String),
  deliveryState: ArtanisForumVerificationDeliveryState,
  directForumPublishAllowed: S.Boolean,
  dispatchAllowed: S.Boolean,
  idempotencyRefs: S.Array(S.String),
  intendedPostRefs: S.Array(S.String),
  listenerNotificationCount: S.Number,
  listenerNotificationRefs: S.Array(S.String),
  listenerState: ArtanisForumVerificationListenerState,
  moderationAllowed: S.Boolean,
  noOpReadCount: S.Number,
  noOpReadRefs: S.Array(S.String),
  normalAgentPostingAllowed: S.Boolean,
  operatorQuestionCount: S.Number,
  operatorQuestionRefs: S.Array(S.String),
  paymentSpendAllowed: S.Boolean,
  privateEvidenceRefs: S.Array(S.String),
  providerMutationAllowed: S.Boolean,
  publicClaimUpgradeAllowed: S.Boolean,
  pylonReleaseWorkLogEvidencePresent: S.Boolean,
  schedulerEnablementAllowed: S.Boolean,
  sourceRefs: S.Array(S.String),
  state: S.Literals(['blocked', 'verified']),
  stateLabel: S.String,
  statusTopicEvidencePresent: S.Boolean,
  targetForumRef: S.String,
  targetTopicRefs: S.Array(S.String),
  targetTopicState: ArtanisForumVerificationTopicState,
  triageDraftCount: S.Number,
  triageDraftRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  verificationRef: S.String,
  walletSpendAllowed: S.Boolean,
  workRoutingProposalCount: S.Number,
  workRoutingProposalRefs: S.Array(S.String),
}) {}

export class ArtanisForumVerificationUnsafe extends S.TaggedErrorClass<ArtanisForumVerificationUnsafe>()(
  'ArtanisForumVerificationUnsafe',
  {
    reason: S.String,
  },
) {}

export const ARTANIS_FORUM_VERIFICATION_NO_EXTRA_AUTHORITY:
  ArtanisForumVerificationAuthority =
    new ArtanisForumVerificationAuthority({
      acceptedWorkPayoutAllowed: false,
      approvedDeliveryBridgeRequired: true,
      directForumPublishAllowed: false,
      dispatchAllowed: false,
      moderationAllowed: false,
      normalAgentPostingAllowed: false,
      paymentSpendAllowed: false,
      providerMutationAllowed: false,
      publicClaimUpgradeAllowed: false,
      schedulerEnablementAllowed: false,
      walletSpendAllowed: false,
    })

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/?#=&{}-]{0,340}$/
const unsafeRefPattern =
  /(@(?!artanis\b)|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|forum[_-]?(payload|raw)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hiddenSteering|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payloadJson|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|forum|invoice|log|model|notification|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const publicUnsafeRefPattern =
  /(^|[.:/_-])(customer|operator|payment|private|provider|raw|secret|wallet)([.:/_-]|$)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

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
    throw new ArtanisForumVerificationUnsafe({
      reason:
        `${label} contains private notification payloads, raw Forum payloads, customer/provider/payment/wallet material, raw timestamps, or credential material.`,
    })
  }
}

const assertAuthorityBoundary = (
  authority: ArtanisForumVerificationAuthority,
): void => {
  if (
    authority.acceptedWorkPayoutAllowed !== false ||
    authority.approvedDeliveryBridgeRequired !== true ||
    authority.directForumPublishAllowed !== false ||
    authority.dispatchAllowed !== false ||
    authority.moderationAllowed !== false ||
    authority.normalAgentPostingAllowed !== false ||
    authority.paymentSpendAllowed !== false ||
    authority.providerMutationAllowed !== false ||
    authority.publicClaimUpgradeAllowed !== false ||
    authority.schedulerEnablementAllowed !== false ||
    authority.walletSpendAllowed !== false
  ) {
    throw new ArtanisForumVerificationUnsafe({
      reason:
        'Artanis Forum verification records do not grant moderation, direct Forum publishing outside the approved bridge, payment, dispatch, scheduler, public-claim, payout, provider, agent-posting, or wallet authority.',
    })
  }
}

const assertCanonicalTopicEvidence = (
  record: ArtanisForumVerificationRecord,
): void => {
  const topicRefs = new Set(record.targetTopicRefs)

  if (
    !topicRefs.has(ARTANIS_STATUS_TOPIC_REF) ||
    !topicRefs.has(ARTANIS_PYLON_RELEASE_WORK_LOG_TOPIC_REF)
  ) {
    throw new ArtanisForumVerificationUnsafe({
      reason:
        'Artanis Forum verification must include public-safe evidence refs for the canonical status topic and Pylon release work-log topic.',
    })
  }
}

const assertDeliveryState = (
  record: ArtanisForumVerificationRecord,
): void => {
  const deliveryRequiresDeliveredEvidence =
    record.deliveryState === 'delivered' ||
    record.deliveryState === 'duplicate_collapsed'

  if (
    deliveryRequiresDeliveredEvidence &&
    (
      record.deliveredPostRefs.length === 0 ||
      record.deliveryReceiptRefs.length === 0 ||
      record.idempotencyRefs.length === 0 ||
      record.intendedPostRefs.length === 0
    )
  ) {
    throw new ArtanisForumVerificationUnsafe({
      reason:
        'Delivered or duplicate-collapsed Artanis Forum verification requires intended post, delivered post, delivery receipt, and idempotency refs.',
    })
  }

  if (
    record.deliveryState === 'blocked' &&
    record.blockerRefs.length === 0
  ) {
    throw new ArtanisForumVerificationUnsafe({
      reason:
        'Blocked Artanis Forum delivery verification requires blocker refs.',
    })
  }

  if (
    record.targetTopicState !== 'open' &&
    record.deliveryState !== 'target_blocked'
  ) {
    throw new ArtanisForumVerificationUnsafe({
      reason:
        'Locked, hidden, or archived Artanis Forum topics must be represented as target-blocked delivery evidence.',
    })
  }

  if (
    record.deliveryState === 'target_blocked' &&
    !record.blockerRefs.includes(
      `blocker.public.artanis.forum_verification.topic_${record.targetTopicState}`,
    )
  ) {
    throw new ArtanisForumVerificationUnsafe({
      reason:
        'Target-blocked Artanis Forum verification requires a topic-state blocker ref.',
    })
  }
}

const assertListenerState = (
  record: ArtanisForumVerificationRecord,
): void => {
  if (
    record.listenerState === 'no_new_posts' &&
    record.noOpReadRefs.length === 0
  ) {
    throw new ArtanisForumVerificationUnsafe({
      reason:
        'No-new-post Artanis Forum listener verification requires no-op/read refs.',
    })
  }

  if (
    record.listenerState === 'reply_draft' &&
    (
      record.listenerNotificationRefs.length === 0 ||
      record.triageDraftRefs.length === 0
    )
  ) {
    throw new ArtanisForumVerificationUnsafe({
      reason:
        'Reply-draft Artanis Forum listener verification requires notification and triage draft refs.',
    })
  }

  if (
    record.listenerState === 'operator_question' &&
    (
      record.listenerNotificationRefs.length === 0 ||
      record.operatorQuestionRefs.length === 0
    )
  ) {
    throw new ArtanisForumVerificationUnsafe({
      reason:
        'Operator-question Artanis Forum listener verification requires notification and operator question refs.',
    })
  }

  if (
    record.listenerState === 'work_routing' &&
    record.workRoutingProposalRefs.length === 0
  ) {
    throw new ArtanisForumVerificationUnsafe({
      reason:
        'Work-routing Artanis Forum listener verification requires proposal refs.',
    })
  }

  if (
    record.listenerState === 'blocked' &&
    record.blockerRefs.length === 0
  ) {
    throw new ArtanisForumVerificationUnsafe({
      reason:
        'Blocked Artanis Forum listener verification requires blocker refs.',
    })
  }
}

const assertRecordSafe = (
  record: ArtanisForumVerificationRecord,
): void => {
  assertAuthorityBoundary(record.authority)
  assertCanonicalTopicEvidence(record)
  assertDeliveryState(record)
  assertListenerState(record)
  assertSafeRefs('Artanis Forum verification refs', [
    record.agentRef,
    record.deliveryState,
    record.listenerState,
    record.targetForumRef,
    record.targetTopicState,
    record.verificationRef,
    ...record.blockerRefs,
    ...record.caveatRefs,
    ...record.deliveredPostRefs,
    ...record.deliveryReceiptRefs,
    ...record.idempotencyRefs,
    ...record.intendedPostRefs,
    ...record.listenerNotificationRefs,
    ...record.noOpReadRefs,
    ...record.operatorQuestionRefs,
    ...record.privateEvidenceRefs,
    ...record.sourceRefs,
    ...record.targetTopicRefs,
    ...record.triageDraftRefs,
    ...record.workRoutingProposalRefs,
  ])

  if (
    rawTimestampPattern.test(JSON.stringify({
      ...record,
      updatedAtIso: 'redacted',
    }))
  ) {
    throw new ArtanisForumVerificationUnsafe({
      reason:
        'Artanis Forum verification records cannot expose raw timestamps outside timestamp fields.',
    })
  }
}

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

const projectionHasUnsafePublicMaterial = (
  projection: ArtanisForumVerificationProjection,
): boolean =>
  projectionStrings(projection).some(value =>
    unsafeRefPattern.test(value) || rawTimestampPattern.test(value)
  )

export const projectArtanisForumVerification = (
  record: ArtanisForumVerificationRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): ArtanisForumVerificationProjection => {
  assertRecordSafe(record)

  const state = record.blockerRefs.length === 0 &&
      record.targetTopicState === 'open' &&
      record.listenerState !== 'blocked' &&
      (
        record.deliveryState === 'delivered' ||
        record.deliveryState === 'duplicate_collapsed'
      )
    ? 'verified'
    : 'blocked'
  const privateEvidenceRefs =
    audience === 'operator' || audience === 'private'
      ? refsForAudience(record.privateEvidenceRefs, audience)
      : []
  const projection = new ArtanisForumVerificationProjection({
    acceptedWorkPayoutAllowed: record.authority.acceptedWorkPayoutAllowed,
    agentRef: record.agentRef,
    approvedDeliveryBridgeRequired:
      record.authority.approvedDeliveryBridgeRequired,
    audience,
    blockerRefs: refsForAudience(record.blockerRefs, audience),
    caveatRefs: refsForAudience(record.caveatRefs, audience),
    deliveredPostCount: record.deliveredPostRefs.length,
    deliveredPostRefs: refsForAudience(record.deliveredPostRefs, audience),
    deliveryReceiptRefs: refsForAudience(record.deliveryReceiptRefs, audience),
    deliveryState: record.deliveryState,
    directForumPublishAllowed: record.authority.directForumPublishAllowed,
    dispatchAllowed: record.authority.dispatchAllowed,
    idempotencyRefs: refsForAudience(record.idempotencyRefs, audience),
    intendedPostRefs: refsForAudience(record.intendedPostRefs, audience),
    listenerNotificationCount: record.listenerNotificationRefs.length,
    listenerNotificationRefs:
      refsForAudience(record.listenerNotificationRefs, audience),
    listenerState: record.listenerState,
    moderationAllowed: record.authority.moderationAllowed,
    noOpReadCount: record.noOpReadRefs.length,
    noOpReadRefs: refsForAudience(record.noOpReadRefs, audience),
    normalAgentPostingAllowed: record.authority.normalAgentPostingAllowed,
    operatorQuestionCount: record.operatorQuestionRefs.length,
    operatorQuestionRefs: refsForAudience(record.operatorQuestionRefs, audience),
    paymentSpendAllowed: record.authority.paymentSpendAllowed,
    privateEvidenceRefs,
    providerMutationAllowed: record.authority.providerMutationAllowed,
    publicClaimUpgradeAllowed: record.authority.publicClaimUpgradeAllowed,
    pylonReleaseWorkLogEvidencePresent:
      record.targetTopicRefs.includes(ARTANIS_PYLON_RELEASE_WORK_LOG_TOPIC_REF),
    schedulerEnablementAllowed: record.authority.schedulerEnablementAllowed,
    sourceRefs: refsForAudience(record.sourceRefs, audience),
    state,
    stateLabel: state === 'verified'
      ? 'Forum delivery and listener evidence verified'
      : 'Forum delivery or listener evidence blocked',
    statusTopicEvidencePresent:
      record.targetTopicRefs.includes(ARTANIS_STATUS_TOPIC_REF),
    targetForumRef: record.targetForumRef,
    targetTopicRefs: refsForAudience(record.targetTopicRefs, audience),
    targetTopicState: record.targetTopicState,
    triageDraftCount: record.triageDraftRefs.length,
    triageDraftRefs: refsForAudience(record.triageDraftRefs, audience),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    verificationRef: record.verificationRef,
    walletSpendAllowed: record.authority.walletSpendAllowed,
    workRoutingProposalCount: record.workRoutingProposalRefs.length,
    workRoutingProposalRefs:
      refsForAudience(record.workRoutingProposalRefs, audience),
  })

  if (
    projection.audience !== 'operator' &&
    projection.audience !== 'private' &&
    projectionHasUnsafePublicMaterial(projection)
  ) {
    throw new ArtanisForumVerificationUnsafe({
      reason:
        'Artanis Forum verification public projection contains private notification payloads, raw Forum payloads, customer/provider/payment/wallet material, or raw timestamps.',
    })
  }

  return projection
}

export const exampleArtanisForumVerificationRecord = (
  nowIso = '2026-06-07T07:30:00.000Z',
): ArtanisForumVerificationRecord =>
  new ArtanisForumVerificationRecord({
    agentRef: 'agent.public.artanis',
    authority: ARTANIS_FORUM_VERIFICATION_NO_EXTRA_AUTHORITY,
    blockerRefs: [],
    caveatRefs: [
      'caveat.public.approved_delivery_bridge_only',
      'caveat.public.listener_drafts_are_not_autonomous_posts',
    ],
    deliveredPostRefs: ['post.public.forum.artanis.status.2'],
    deliveryReceiptRefs: [
      'receipt.public.artanis.forum_delivery.status_post_2',
    ],
    deliveryState: 'delivered',
    idempotencyRefs: [
      'idempotency.public.artanis.forum_delivery.status_post_2',
    ],
    intendedPostRefs: [
      'intent.public.artanis.forum_delivery.status_post_2',
    ],
    listenerNotificationRefs: [
      'notification.public.forum.artanis.status.question',
    ],
    listenerState: 'reply_draft',
    noOpReadRefs: [],
    operatorQuestionRefs: [],
    privateEvidenceRefs: [
      'evidence.operator.artanis.forum_listener.redacted_notification_digest',
    ],
    sourceRefs: [
      'docs/artanis/2026-06-06-forum-delivery-listener-verification.md',
      'route:/api/forum/topics/88888888-4001-4001-8001-888888888888',
      'route:/api/agents/notifications',
    ],
    targetForumRef: ARTANIS_FORUM_REF,
    targetTopicRefs: [
      ARTANIS_STATUS_TOPIC_REF,
      ARTANIS_PYLON_RELEASE_WORK_LOG_TOPIC_REF,
    ],
    targetTopicState: 'open',
    triageDraftRefs: [
      'draft.public.artanis.forum_listener.status_question_reply',
    ],
    updatedAtIso: nowIso,
    verificationRef:
      'verification.public.artanis.forum_delivery_listener.status_pylon.v1',
    workRoutingProposalRefs: [],
  })
