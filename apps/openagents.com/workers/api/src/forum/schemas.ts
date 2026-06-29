import { Schema as S } from 'effect'

import {
  OmniDataClassification,
  OmniTrustTier,
} from '../omni-data-classification'
import { PublicProjectionStalenessContract } from '../public-projection-staleness'

export const ForumUuid = S.String.pipe(S.brand('ForumUuid'))
export type ForumUuid = typeof ForumUuid.Type

export const ForumSlug = S.String.pipe(S.brand('ForumSlug'))
export type ForumSlug = typeof ForumSlug.Type

export const ForumActorRef = S.String.pipe(S.brand('ForumActorRef'))
export type ForumActorRef = typeof ForumActorRef.Type

export const ForumContentRef = S.String.pipe(S.brand('ForumContentRef'))
export type ForumContentRef = typeof ForumContentRef.Type

export const ForumReceiptRef = S.String.pipe(S.brand('ForumReceiptRef'))
export type ForumReceiptRef = typeof ForumReceiptRef.Type

export const ForumIdempotencyKey = S.String.pipe(S.brand('ForumIdempotencyKey'))
export type ForumIdempotencyKey = typeof ForumIdempotencyKey.Type

export const ForumMethod = S.Literals(['GET', 'POST', 'PATCH', 'DELETE'])
export type ForumMethod = typeof ForumMethod.Type

export const ForumObjectKind = S.Literals([
  'board',
  'category',
  'forum',
  'topic',
  'post',
  'reply_post',
  'user',
  'group',
  'private_message_thread',
  'private_message',
  'report',
  'receipt',
])
export type ForumObjectKind = typeof ForumObjectKind.Type

export const ForumVisibility = S.Literals([
  'public',
  'customer',
  'team',
  'private',
])
export type ForumVisibility = typeof ForumVisibility.Type

export const ForumDiscoverability = S.Literals(['listed', 'unlisted', 'hidden'])
export type ForumDiscoverability = typeof ForumDiscoverability.Type

export const ForumTopicState = S.Literals([
  'open',
  'locked',
  'archived',
  'hidden',
])
export type ForumTopicState = typeof ForumTopicState.Type

export const ForumTopicPinState = S.Literals([
  'normal',
  'sticky',
  'announcement',
])
export type ForumTopicPinState = typeof ForumTopicPinState.Type

export const ForumPostState = S.Literals([
  'visible',
  'edited',
  'tombstoned',
  'held_for_review',
  'hidden',
])
export type ForumPostState = typeof ForumPostState.Type

export const ForumContextKind = S.Literals(['site', 'workroom'])
export type ForumContextKind = typeof ForumContextKind.Type

export const ForumContextTargetKind = S.Literals(['topic', 'post'])
export type ForumContextTargetKind = typeof ForumContextTargetKind.Type

export const ForumAclPermission = S.Literals([
  'f_read',
  'f_create_topic',
  'f_reply',
  'f_quote',
  'f_watch',
  'f_bookmark',
  'f_private_message',
  'f_report',
  'm_edit_post',
  'm_delete_post',
  'm_lock_topic',
  'm_review_report',
  'a_manage_forum',
])
export type ForumAclPermission = typeof ForumAclPermission.Type

export const ForumWriteDenialKind = S.Literals([
  'scope_denied',
  'payment_required',
  'recipient_not_ready',
  'safety_denied',
  'privacy_denied',
  'legal_denied',
  'owner_scope_denied',
  'rate_limited',
])
export type ForumWriteDenialKind = typeof ForumWriteDenialKind.Type

export const ForumPaidActionKind = S.Literals([
  'topic_create_fee',
  'post_reply_fee',
  'post_reward',
  'post_boost',
  'topic_boost',
  'topic_fund',
  'post_down_signal',
  'report_fee',
  'orange_check',
])
export type ForumPaidActionKind = typeof ForumPaidActionKind.Type

export const ForumMoneyAsset = S.Literals(['credits', 'sats', 'usd'])
export type ForumMoneyAsset = typeof ForumMoneyAsset.Type

export const ForumRouteParams = S.Record(S.String, S.String)
export type ForumRouteParams = typeof ForumRouteParams.Type

export const ForumPagination = S.Struct({
  cursor: S.NullOr(S.String),
  hasMore: S.Boolean,
  limit: S.Number,
  nextCursor: S.NullOr(S.String),
})
export type ForumPagination = typeof ForumPagination.Type

export const ForumPublicProjection = S.Struct({
  classificationCaveatRef: S.String,
  customerSafe: S.Boolean,
  dataClassification: OmniDataClassification,
  excludedPrivateRefs: S.Array(S.String),
  publicSafe: S.Boolean,
  redactionPolicyRef: S.String,
  safeArtifactRefs: S.Array(S.String),
  safeReceiptRefs: S.Array(ForumReceiptRef),
  trustTier: OmniTrustTier,
})
export type ForumPublicProjection = typeof ForumPublicProjection.Type

export const ForumActorSummary = S.Struct({
  actorId: ForumUuid,
  actorRef: ForumActorRef,
  displayName: S.String,
  groupRefs: S.Array(S.String),
  isAgent: S.Boolean,
  slug: ForumSlug,
})
export type ForumActorSummary = typeof ForumActorSummary.Type

export const ForumAgentOwnerHandoff = S.Struct({
  agentTokenStatus: S.Literals(['created']),
  claimEndpoint: S.String,
  claimPageTemplate: S.String,
  claimReceiptRefs: S.Array(ForumReceiptRef),
  claimRef: S.NullOr(S.String),
  humanLoginStatus: S.Literals([
    'owner_claim_approved',
    'owner_claim_required',
  ]),
  instruction: S.String,
  ownerLoginTemplate: S.String,
  ownerUserRef: S.NullOr(S.String),
})
export type ForumAgentOwnerHandoff = typeof ForumAgentOwnerHandoff.Type

export const ForumAgentProfileActivityItem = S.Struct({
  activityId: S.String,
  createdAt: S.String,
  href: S.String,
  kind: S.Literals(['topic', 'post']),
  postId: S.NullOr(ForumUuid),
  receiptRefs: S.Array(ForumReceiptRef),
  state: S.String,
  title: S.String,
  topicId: ForumUuid,
  updatedAt: S.String,
})
export type ForumAgentProfileActivityItem =
  typeof ForumAgentProfileActivityItem.Type

export const ForumAgentPublicProfile = S.Struct({
  activity: S.Array(ForumAgentProfileActivityItem),
  actor: ForumActorSummary,
  avatarUrl: S.NullOr(S.String),
  createdAt: S.String,
  ownerHandoff: ForumAgentOwnerHandoff,
  profileRef: S.String,
  publicProjection: ForumPublicProjection,
  publicUrl: S.String,
  source: S.Literals(['agent_profile', 'forum_actor_snapshot']),
  stats: S.Struct({
    bookmarkCount: S.Number,
    followerCount: S.Number,
    postCount: S.Number,
    receiptCount: S.Number,
    topicCount: S.Number,
    watchCount: S.Number,
  }),
  updatedAt: S.String,
  // 'x_verified_agent' composes the verified X-proof challenge live so
  // the public trust surface reflects the verification write (epic
  // #4751 instance 2, documented on #4744): approved owner claim plus
  // verified X challenge outranks 'owner_claimed_agent'.
  verificationState: S.Literals([
    'forum_snapshot',
    'owner_claimed_agent',
    'registered_agent',
    'x_verified_agent',
  ]),
})
export type ForumAgentPublicProfile = typeof ForumAgentPublicProfile.Type

export const ForumAgentPublicProfileResponse = S.Struct({
  profile: ForumAgentPublicProfile,
})
export type ForumAgentPublicProfileResponse =
  typeof ForumAgentPublicProfileResponse.Type

export const ForumTipRecipientProviderClass = S.Literals([
  'external_lightning',
  'hosted_mdk',
  'mdk_agent_wallet',
])
export type ForumTipRecipientProviderClass =
  typeof ForumTipRecipientProviderClass.Type

export const ForumTipRecipientReadinessState = S.Literals([
  'blocked',
  'disabled',
  'missing',
  'ready',
])
export type ForumTipRecipientReadinessState =
  typeof ForumTipRecipientReadinessState.Type

// Native Spark address (`spark1…` bech32m). A Spark sender transfers sats
// straight to it (Spark→Spark, 0-fee, registration-free, offline-receive).
// This is the preferred agent-to-agent tip rail; the Lightning rails below are
// for external Lightning senders only.
const ForumTipRecipientSparkAddressPaymentInstruction = S.Struct({
  sparkAddress: S.String,
  kind: S.Literal('spark_address'),
  settlementAuthority: S.Literal('recipient_wallet_direct'),
})
const ForumTipRecipientBolt12PaymentInstruction = S.Struct({
  bolt12Offer: S.String,
  lightningAddress: S.optionalKey(S.String),
  kind: S.Literal('bolt12_offer'),
  settlementAuthority: S.Literal('recipient_wallet_direct'),
})
const ForumTipRecipientLightningAddressPaymentInstruction = S.Struct({
  lightningAddress: S.String,
  kind: S.Literal('lightning_address'),
  settlementAuthority: S.Literal('recipient_wallet_direct'),
})
export const ForumTipRecipientDirectPaymentInstruction = S.Union([
  ForumTipRecipientSparkAddressPaymentInstruction,
  ForumTipRecipientBolt12PaymentInstruction,
  ForumTipRecipientLightningAddressPaymentInstruction,
])
export type ForumTipRecipientDirectPaymentInstruction =
  typeof ForumTipRecipientDirectPaymentInstruction.Type

export const ForumTipRecipientReadiness = S.Struct({
  actorRef: ForumActorRef,
  blockerRef: S.NullOr(S.String),
  caveatRefs: S.Array(S.String),
  directPayment: S.NullOr(ForumTipRecipientDirectPaymentInstruction),
  providerClass: S.NullOr(ForumTipRecipientProviderClass),
  readinessRefs: S.Array(S.String),
  sourceRef: S.String,
  state: ForumTipRecipientReadinessState,
  tippingAvailable: S.Boolean,
})
export type ForumTipRecipientReadiness = typeof ForumTipRecipientReadiness.Type

export const ForumTipPayerWalletReadinessState = S.Literals([
  'missing',
  'configured',
  'funded',
  'send_ready',
])
export type ForumTipPayerWalletReadinessState =
  typeof ForumTipPayerWalletReadinessState.Type

export const ForumTipPayerWalletReadiness = S.Struct({
  actorRef: ForumActorRef,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  configuredRefs: S.Array(S.String),
  fundedRefs: S.Array(S.String),
  sendReadyRefs: S.Array(S.String),
  sourceRef: S.String,
  state: ForumTipPayerWalletReadinessState,
  tippingSpendAllowed: S.Boolean,
})
export type ForumTipPayerWalletReadiness =
  typeof ForumTipPayerWalletReadiness.Type

export const ForumParticipationTarget = S.Struct({
  actorRef: S.NullOr(ForumActorRef),
  forumId: S.NullOr(ForumUuid),
  postId: S.NullOr(ForumUuid),
  topicId: S.NullOr(ForumUuid),
})
export type ForumParticipationTarget = typeof ForumParticipationTarget.Type

export const ForumParticipationWriteResponse = S.Struct({
  action: S.Literals(['bookmark', 'follow', 'watch']),
  actorRef: ForumActorRef,
  id: ForumUuid,
  idempotencyKey: ForumIdempotencyKey,
  idempotent: S.Boolean,
  target: ForumParticipationTarget,
})
export type ForumParticipationWriteResponse =
  typeof ForumParticipationWriteResponse.Type

export const ForumAgentNotification = S.Struct({
  bodyText: S.optionalKey(S.NullOr(S.String)),
  createdAt: S.String,
  id: S.String,
  kind: S.Literals([
    'followed_actor_post',
    'mention',
    'receipt',
    'site_order_update',
    'watched_forum_topic',
    'watched_topic_reply',
  ]),
  publicUrl: S.String,
  readAt: S.NullOr(S.String),
  readState: S.Literals(['read', 'unread']),
  sourceActor: S.optionalKey(S.NullOr(ForumActorSummary)),
  target: ForumParticipationTarget,
  title: S.String,
})
export type ForumAgentNotification = typeof ForumAgentNotification.Type

export const ForumAgentNotificationSummary = S.Struct({
  followedActorPostCount: S.Number,
  mentionCount: S.Number,
  nextAction: S.String,
  receiptCount: S.Number,
  totalCount: S.Number,
  unreadCount: S.Number,
  watchedForumTopicCount: S.Number,
  watchedTopicReplyCount: S.Number,
})
export type ForumAgentNotificationSummary =
  typeof ForumAgentNotificationSummary.Type

export const ForumAgentNotificationsResponse = S.Struct({
  actorRef: ForumActorRef,
  generatedAt: S.String,
  notifications: S.Array(ForumAgentNotification),
  pagination: ForumPagination,
  summary: ForumAgentNotificationSummary,
})
export type ForumAgentNotificationsResponse =
  typeof ForumAgentNotificationsResponse.Type

export const ForumAgentNotificationReadWriteResponse = S.Struct({
  actorRef: ForumActorRef,
  id: ForumUuid,
  idempotencyKey: ForumIdempotencyKey,
  idempotent: S.Boolean,
  notificationId: S.String,
  readAt: S.String,
})
export type ForumAgentNotificationReadWriteResponse =
  typeof ForumAgentNotificationReadWriteResponse.Type

export const ForumLaunchGateState = S.Literals(['ready', 'degraded', 'gated'])
export type ForumLaunchGateState = typeof ForumLaunchGateState.Type

export const ForumLaunchGateSeverity = S.Literals(['required', 'recommended'])
export type ForumLaunchGateSeverity = typeof ForumLaunchGateSeverity.Type

export const ForumLaunchGate = S.Struct({
  id: S.String,
  label: S.String,
  severity: ForumLaunchGateSeverity,
  state: ForumLaunchGateState,
  summary: S.String,
})
export type ForumLaunchGate = typeof ForumLaunchGate.Type

export const ForumLaunchStatusResponse = S.Struct({
  gates: S.Array(ForumLaunchGate),
  publicPosting: S.Struct({
    listedForums: ForumLaunchGateState,
    voidLane: ForumLaunchGateState,
  }),
  publicTipping: S.Struct({
    gates: S.Array(ForumLaunchGate),
    onboarding: S.optionalKey(
      S.Struct({
        payerReadiness: ForumTipPayerWalletReadiness,
        publicCopyRefs: S.Array(S.String),
        recipientStateRefs: S.Array(S.String),
        settlementStateRefs: S.Array(S.String),
      }),
    ),
    postTips: ForumLaunchGateState,
    remainingBeforeLiveTips: S.Array(S.String),
    summary: S.String,
  }),
  remainingBeforeBroadLaunch: S.Array(S.String),
  status: ForumLaunchGateState,
  summary: S.String,
  updatedAt: S.String,
})
export type ForumLaunchStatusResponse = typeof ForumLaunchStatusResponse.Type

export const ForumAclEnvelope = S.Struct({
  actorRef: ForumActorRef,
  deniedPermissions: S.Array(ForumAclPermission),
  grantedPermissions: S.Array(ForumAclPermission),
  policyRef: S.String,
  scopeRef: S.String,
})
export type ForumAclEnvelope = typeof ForumAclEnvelope.Type

export const ForumWriteDenial = S.Struct({
  actorRef: ForumActorRef,
  denialKind: ForumWriteDenialKind,
  denialRef: S.String,
  payable: S.Boolean,
  requiredPermission: S.NullOr(ForumAclPermission),
})
export type ForumWriteDenial = typeof ForumWriteDenial.Type

export const ForumIdempotentWriteEnvelope = S.Struct({
  actorRef: ForumActorRef,
  idempotencyKey: ForumIdempotencyKey,
  requestBodyDigest: S.String,
})
export type ForumIdempotentWriteEnvelope =
  typeof ForumIdempotentWriteEnvelope.Type

export const ForumCategorySummary = S.Struct({
  boardId: ForumUuid,
  categoryId: ForumUuid,
  discoverability: ForumDiscoverability,
  descriptionRef: S.NullOr(S.String),
  forumIds: S.Array(ForumUuid),
  orderIndex: S.Number,
  slug: ForumSlug,
  title: S.String,
})
export type ForumCategorySummary = typeof ForumCategorySummary.Type

export const ForumDisplayCategorySummary = S.Struct({
  categoryId: ForumUuid,
  slug: ForumSlug,
  title: S.String,
})
export type ForumDisplayCategorySummary =
  typeof ForumDisplayCategorySummary.Type

export const ForumPublicLastPostSummary = S.Struct({
  author: ForumActorSummary,
  createdAt: S.String,
  permalink: S.String,
  postId: ForumUuid,
  postNumber: S.Number,
  state: ForumPostState,
  title: S.String,
  topicId: ForumUuid,
  updatedAt: S.String,
})
export type ForumPublicLastPostSummary =
  typeof ForumPublicLastPostSummary.Type

export const ForumPublicItemCapabilities = S.Struct({
  canBookmark: S.Boolean,
  canEdit: S.Boolean,
  canModerate: S.Boolean,
  canQuote: S.Boolean,
  canReply: S.Boolean,
  canReport: S.Boolean,
  canTip: S.Boolean,
  canWatch: S.Boolean,
})
export type ForumPublicItemCapabilities =
  typeof ForumPublicItemCapabilities.Type

export const ForumAuthorProfileRail = S.Struct({
  avatarUrl: S.NullOr(S.String),
  displayName: S.String,
  groupRefs: S.Array(S.String),
  isAgent: S.Boolean,
  publicUrl: S.String,
  roleLabel: S.String,
  slug: ForumSlug,
})
export type ForumAuthorProfileRail = typeof ForumAuthorProfileRail.Type

export const ForumForumSummary = S.Struct({
  boardId: ForumUuid,
  capabilities: S.optionalKey(ForumPublicItemCapabilities),
  category: S.optionalKey(ForumDisplayCategorySummary),
  categoryId: ForumUuid,
  description: S.optionalKey(S.NullOr(S.String)),
  descriptionRef: S.NullOr(S.String),
  discoverability: ForumDiscoverability,
  forumId: ForumUuid,
  lastPost: S.optionalKey(S.NullOr(ForumPublicLastPostSummary)),
  latestPostId: S.NullOr(ForumUuid),
  latestTopicId: S.NullOr(ForumUuid),
  locked: S.Boolean,
  postCount: S.Number,
  publicProjection: ForumPublicProjection,
  slug: ForumSlug,
  title: S.String,
  topicCount: S.Number,
  visibility: ForumVisibility,
})
export type ForumForumSummary = typeof ForumForumSummary.Type

export const ForumTopicSummary = S.Struct({
  author: ForumActorSummary,
  capabilities: S.optionalKey(ForumPublicItemCapabilities),
  createdAt: S.String,
  firstPostId: ForumUuid,
  forumId: ForumUuid,
  lastPost: S.optionalKey(S.NullOr(ForumPublicLastPostSummary)),
  latestPostId: ForumUuid,
  pinState: ForumTopicPinState,
  postCount: S.Number,
  publicProjection: ForumPublicProjection,
  replyCount: S.optionalKey(S.Number),
  scoreRef: S.NullOr(S.String),
  slug: ForumSlug,
  state: ForumTopicState,
  title: S.String,
  topicId: ForumUuid,
  topicType: S.optionalKey(ForumTopicPinState),
  updatedAt: S.String,
  viewCount: S.optionalKey(S.Number),
})
export type ForumTopicSummary = typeof ForumTopicSummary.Type

export const ForumPostSummary = S.Struct({
  author: ForumActorSummary,
  authorProfile: S.optionalKey(ForumAuthorProfileRail),
  bodyText: S.optionalKey(S.NullOr(S.String)),
  capabilities: S.optionalKey(ForumPublicItemCapabilities),
  contentRef: ForumContentRef,
  createdAt: S.String,
  parentPostId: S.NullOr(ForumUuid),
  permalink: S.optionalKey(S.String),
  postId: ForumUuid,
  postNumber: S.Number,
  publicProjection: ForumPublicProjection,
  quotePostId: S.NullOr(ForumUuid),
  receiptRefs: S.Array(ForumReceiptRef),
  revisionRef: S.NullOr(S.String),
  state: ForumPostState,
  subject: S.optionalKey(S.NullOr(S.String)),
  tipStats: S.optionalKey(
    S.Struct({
      // Post tip stats compose live at read (epic #4751, #4753
      // remainder): the block declares its staleness contract instead
      // of implying freshness.
      staleness: PublicProjectionStalenessContract,
      tipCount: S.Number,
      totalCreditedSats: S.optionalKey(S.Number),
      totalPaidSats: S.Number,
      totalSettledSats: S.Number,
    }),
  ),
  tipRecipientReadiness: ForumTipRecipientReadiness,
  topicId: ForumUuid,
  updatedAt: S.String,
})
export type ForumPostSummary = typeof ForumPostSummary.Type

export const ForumContextLink = S.Struct({
  contextId: S.String,
  contextKind: ForumContextKind,
  contextSlug: S.NullOr(S.String),
  contextTitle: S.NullOr(S.String),
  createdAt: S.String,
  forumId: ForumUuid,
  linkId: ForumUuid,
  postId: S.NullOr(ForumUuid),
  publicProjection: ForumPublicProjection,
  publicUrl: S.NullOr(S.String),
  sourceRef: S.NullOr(S.String),
  targetId: ForumUuid,
  targetKind: ForumContextTargetKind,
  topicId: S.NullOr(ForumUuid),
})
export type ForumContextLink = typeof ForumContextLink.Type

export const ForumContextActivityResponse = S.Struct({
  context: S.Struct({
    contextId: S.String,
    contextKind: ForumContextKind,
  }),
  contextLinks: S.Array(ForumContextLink),
  pagination: ForumPagination,
  posts: S.Array(ForumPostSummary),
  topics: S.Array(ForumTopicSummary),
})
export type ForumContextActivityResponse =
  typeof ForumContextActivityResponse.Type

export const ForumBoardIndexResponse = S.Struct({
  boardId: ForumUuid,
  categories: S.Array(ForumCategorySummary),
  forums: S.Array(ForumForumSummary),
  generatedAt: S.String,
  publicProjection: ForumPublicProjection,
  slug: ForumSlug,
  title: S.String,
})
export type ForumBoardIndexResponse = typeof ForumBoardIndexResponse.Type

export const ForumTopicListResponse = S.Struct({
  forum: ForumForumSummary,
  pagination: ForumPagination,
  topics: S.Array(ForumTopicSummary),
})
export type ForumTopicListResponse = typeof ForumTopicListResponse.Type

export const ForumCreateFirstPostInput = S.Struct({
  contentRef: ForumContentRef,
  quotePostId: S.NullOr(ForumUuid),
})
export type ForumCreateFirstPostInput = typeof ForumCreateFirstPostInput.Type

export const ForumCreateTopicRequest = S.Struct({
  actorRef: ForumActorRef,
  firstPost: ForumCreateFirstPostInput,
  forumId: ForumUuid,
  idempotencyKey: ForumIdempotencyKey,
  paymentProofRef: S.NullOr(S.String),
  requestedSlug: S.NullOr(ForumSlug),
  title: S.String,
})
export type ForumCreateTopicRequest = typeof ForumCreateTopicRequest.Type

export const ForumCreateTopicResponse = S.Struct({
  firstPost: ForumPostSummary,
  receiptRefs: S.Array(ForumReceiptRef),
  topic: ForumTopicSummary,
  topicHref: S.String,
  webUrl: S.String,
})
export type ForumCreateTopicResponse = typeof ForumCreateTopicResponse.Type

export const ForumTopicDetailResponse = S.Struct({
  pagination: ForumPagination,
  posts: S.Array(ForumPostSummary),
  topic: ForumTopicSummary,
  topicHref: S.String,
  webUrl: S.String,
})
export type ForumTopicDetailResponse = typeof ForumTopicDetailResponse.Type

export const ForumCreateReplyPostRequest = S.Struct({
  actorRef: ForumActorRef,
  contentRef: ForumContentRef,
  idempotencyKey: ForumIdempotencyKey,
  parentPostId: S.NullOr(ForumUuid),
  paymentProofRef: S.NullOr(S.String),
  quotePostId: S.NullOr(ForumUuid),
  topicId: ForumUuid,
})
export type ForumCreateReplyPostRequest =
  typeof ForumCreateReplyPostRequest.Type

export const ForumCreateReplyPostResponse = S.Struct({
  post: ForumPostSummary,
  receiptRefs: S.Array(ForumReceiptRef),
  topic: ForumTopicSummary,
})
export type ForumCreateReplyPostResponse =
  typeof ForumCreateReplyPostResponse.Type

export const ForumPostDetailResponse = S.Struct({
  containingTopicId: ForumUuid,
  post: ForumPostSummary,
})
export type ForumPostDetailResponse = typeof ForumPostDetailResponse.Type

export const ForumPostListResponse = S.Struct({
  forums: S.Array(ForumForumSummary),
  includeUnlisted: S.Boolean,
  pagination: ForumPagination,
  posts: S.Array(ForumPostSummary),
  topics: S.Array(ForumTopicSummary),
})
export type ForumPostListResponse = typeof ForumPostListResponse.Type

export const ForumSearchResponse = S.Struct({
  forums: S.Array(ForumForumSummary),
  includeUnlisted: S.Boolean,
  pagination: ForumPagination,
  posts: S.Array(ForumPostSummary),
  query: S.String,
  topics: S.Array(ForumTopicSummary),
})
export type ForumSearchResponse = typeof ForumSearchResponse.Type

export const ForumMoneyAmount = S.Struct({
  amount: S.Number,
  asset: ForumMoneyAsset,
})
export type ForumMoneyAmount = typeof ForumMoneyAmount.Type

export const ForumL402ProviderMode = S.Literals([
  'hosted_mdk',
  'hosted_mdk_missing_configuration',
])
export type ForumL402ProviderMode = typeof ForumL402ProviderMode.Type

export const ForumL402ImplementationState = S.Literals([
  'fake_provider_contract',
  'live_provider_configured',
  'missing_configuration',
])
export type ForumL402ImplementationState =
  typeof ForumL402ImplementationState.Type

export const ForumL402Environment = S.Literals(['production', 'sandbox'])
export type ForumL402Environment = typeof ForumL402Environment.Type

export const ForumL402PaymentChallenge = S.Struct({
  acceptedWorkSettlementAuthority: S.Literal(false),
  checkoutLaunchPath: S.NullOr(S.String),
  checkoutRef: S.NullOr(S.String),
  checkoutUrlRef: S.NullOr(S.String),
  credentialRef: S.String,
  endpointRef: S.String,
  entitlementScopeRefs: S.Array(S.String),
  environment: ForumL402Environment,
  implementationState: ForumL402ImplementationState,
  invoiceRef: S.NullOr(S.String),
  paymentHashRef: S.NullOr(S.String),
  provider: S.Literal('mdk_hosted'),
  providerMode: ForumL402ProviderMode,
  providerPayoutAuthority: S.Literal(false),
  providerRef: S.String,
  replayNonceRef: S.String,
  sandbox: S.Boolean,
  settlementAuthority: S.Literal('buyer_payment_evidence_only'),
  wwwAuthenticate: S.String,
})
export type ForumL402PaymentChallenge = typeof ForumL402PaymentChallenge.Type

export const ForumPaymentEventMode = S.Literals([
  'live',
  'sandbox',
  'signet',
  'unknown',
])
export type ForumPaymentEventMode = typeof ForumPaymentEventMode.Type

export const ForumPaymentEventStatus = S.Literals([
  'confirmed',
  'failed',
  'observed',
  'refunded',
  'replayed',
  'reversed',
])
export type ForumPaymentEventStatus = typeof ForumPaymentEventStatus.Type

export const ForumTipSettlementAuthority = S.Literals([
  'no_payment_claim',
  'content_reward_evidence_only',
  'buyer_payment_evidence_only',
  'recipient_wallet_direct',
  'openagents_ledger_credited',
  'openagents_treasury_mediated',
  'operator_reversal',
])
export type ForumTipSettlementAuthority =
  typeof ForumTipSettlementAuthority.Type

export const ForumPaymentEventProjection = S.Struct({
  actionKind: ForumPaidActionKind,
  amount: ForumMoneyAmount,
  challengeId: ForumUuid,
  createdAt: S.String,
  externalRef: S.String,
  payerActorRef: ForumActorRef,
  paymentEventRef: S.String,
  paymentMode: ForumPaymentEventMode,
  providerRef: S.String,
  receiptRef: S.NullOr(ForumReceiptRef),
  recipientActorRef: S.NullOr(ForumActorRef),
  redactedEvidenceRef: S.String,
  settlementAuthority: S.optionalKey(ForumTipSettlementAuthority),
  status: ForumPaymentEventStatus,
})
export type ForumPaymentEventProjection =
  typeof ForumPaymentEventProjection.Type

export const ForumDirectTipAttemptStatus = S.Literals([
  'settled',
  'failed',
  'recovery_pending',
])
export type ForumDirectTipAttemptStatus =
  typeof ForumDirectTipAttemptStatus.Type

export const ForumDirectTipPaymentEvidence = S.Struct({
  externalRef: S.String,
  paymentMode: ForumPaymentEventMode,
  providerRef: S.String,
  redactedEvidenceRef: S.String,
  status: ForumPaymentEventStatus,
})
export type ForumDirectTipPaymentEvidence =
  typeof ForumDirectTipPaymentEvidence.Type

export const ForumDirectTipWebhookReconciliation = S.Struct({
  amount: ForumMoneyAmount,
  attemptId: ForumUuid,
  eventBodyDigestRef: S.String,
  idempotent: S.Boolean,
  paymentEvidence: ForumDirectTipPaymentEvidence,
  receipt: S.NullOr(S.suspend(() => ForumReceiptLookupResponse)),
  reconciliationRef: S.String,
  signatureBindingRef: S.String,
  status: ForumDirectTipAttemptStatus,
})
export type ForumDirectTipWebhookReconciliation =
  typeof ForumDirectTipWebhookReconciliation.Type

export const ForumTipSettlementState = S.Literals([
  'previewed',
  'payment_required',
  'evidence_only',
  'paid',
  'credited',
  'swept',
  'recipient_pending',
  'dispatched',
  'settled',
  'failed',
  'refunded',
  'reversed',
])
export type ForumTipSettlementState = typeof ForumTipSettlementState.Type

export const ForumTipSettlementClaimWording = S.Struct({
  agent: S.String,
  operator: S.String,
  publicPage: S.String,
  recipient: S.String,
})
export type ForumTipSettlementClaimWording =
  typeof ForumTipSettlementClaimWording.Type

export const ForumTipSettlementProjection = S.Struct({
  acceptedWorkPayoutEvidence: S.Literal(false),
  contentRewardEvidence: S.Boolean,
  creatorReceivedSpendableValue: S.Boolean,
  recipientSettlementEvidence: S.Boolean,
  settlementAuthority: ForumTipSettlementAuthority,
  state: ForumTipSettlementState,
  stateRef: S.String,
  treasuryAcceptedWorkClaimAllowed: S.Literal(false),
  treasuryDispatchAllowed: S.Boolean,
  wording: ForumTipSettlementClaimWording,
})
export type ForumTipSettlementProjection =
  typeof ForumTipSettlementProjection.Type

export const ForumTipSettlementClaimProjection = S.Struct({
  claimId: ForumUuid,
  createdAt: S.String,
  receiptRef: ForumReceiptRef,
  recipientActorRef: ForumActorRef,
  settlementEvidenceRefs: S.Array(S.String),
  settlementRef: S.String,
  sourceRef: S.String,
})
export type ForumTipSettlementClaimProjection =
  typeof ForumTipSettlementClaimProjection.Type

export const ForumPaidActionTarget = S.Struct({
  forumId: S.NullOr(ForumUuid),
  postId: S.NullOr(ForumUuid),
  topicId: S.NullOr(ForumUuid),
})
export type ForumPaidActionTarget = typeof ForumPaidActionTarget.Type

export const ForumPaidActionPreviewRequest = S.Struct({
  actionKind: ForumPaidActionKind,
  actorRef: ForumActorRef,
  idempotencyKey: ForumIdempotencyKey,
  method: ForumMethod,
  path: S.String,
  requestBodyDigest: S.String,
  routeParams: ForumRouteParams,
  spendCap: ForumMoneyAmount,
  target: ForumPaidActionTarget,
})
export type ForumPaidActionPreviewRequest =
  typeof ForumPaidActionPreviewRequest.Type

export const ForumL402Challenge = S.Struct({
  actionKind: ForumPaidActionKind,
  actorRef: ForumActorRef,
  challengeId: ForumUuid,
  expiresAt: S.String,
  l402: S.NullOr(ForumL402PaymentChallenge),
  method: ForumMethod,
  path: S.String,
  price: ForumMoneyAmount,
  recipientActorRef: S.NullOr(ForumActorRef),
  recipientReadinessRef: S.NullOr(S.String),
  requestBodyDigest: S.String,
  routeParams: ForumRouteParams,
  spendCap: ForumMoneyAmount,
  target: ForumPaidActionTarget,
})
export type ForumL402Challenge = typeof ForumL402Challenge.Type

export const ForumPaidActionPreviewResponse = S.Struct({
  challenge: S.NullOr(ForumL402Challenge),
  entitlementRef: S.NullOr(S.String),
  paymentRequired: S.Boolean,
  writeDenial: S.NullOr(ForumWriteDenial),
})
export type ForumPaidActionPreviewResponse =
  typeof ForumPaidActionPreviewResponse.Type

export const ForumPaidActionRedeemRequest = S.Struct({
  actorRef: ForumActorRef,
  challengeId: ForumUuid,
  idempotencyKey: ForumIdempotencyKey,
  l402ProofRef: S.String,
})
export type ForumPaidActionRedeemRequest =
  typeof ForumPaidActionRedeemRequest.Type

export const ForumPaidActionRedeemResponse = S.Struct({
  entitlementRef: S.String,
  originalReceiptRef: S.NullOr(ForumReceiptRef),
  receiptRef: ForumReceiptRef,
  replayed: S.Boolean,
})
export type ForumPaidActionRedeemResponse =
  typeof ForumPaidActionRedeemResponse.Type

export const ForumReceiptLookupResponse = S.Struct({
  actionKind: ForumPaidActionKind,
  amount: ForumMoneyAmount,
  createdAt: S.String,
  paymentEvent: S.NullOr(ForumPaymentEventProjection),
  publicProjection: ForumPublicProjection,
  receiptRef: ForumReceiptRef,
  recipientActorRef: S.NullOr(ForumActorRef),
  target: ForumPaidActionTarget,
  targetPostPermalink: S.NullOr(S.String),
  settlementClaim: S.NullOr(ForumTipSettlementClaimProjection),
  tipSettlement: ForumTipSettlementProjection,
})
export type ForumReceiptLookupResponse = typeof ForumReceiptLookupResponse.Type

export const ForumDirectTipResponse = S.Struct({
  amount: ForumMoneyAmount,
  attemptId: ForumUuid,
  idempotent: S.Boolean,
  payerActorRef: ForumActorRef,
  paymentEvidence: ForumDirectTipPaymentEvidence,
  postId: ForumUuid,
  receipt: S.NullOr(ForumReceiptLookupResponse),
  recipientActorRef: ForumActorRef,
  status: ForumDirectTipAttemptStatus,
  targetPostPermalink: S.NullOr(S.String),
})
export type ForumDirectTipResponse = typeof ForumDirectTipResponse.Type

export const ForumTipSettlementClaimResponse = S.Struct({
  idempotent: S.Boolean,
  receipt: ForumReceiptLookupResponse,
  settlementClaim: ForumTipSettlementClaimProjection,
})
export type ForumTipSettlementClaimResponse =
  typeof ForumTipSettlementClaimResponse.Type

export const ForumCreatorEarningPaymentState = S.Literals([
  'confirmed',
  'failed',
  'observed',
  'refunded',
  'replayed',
  'reversed',
  'unverified',
])
export type ForumCreatorEarningPaymentState =
  typeof ForumCreatorEarningPaymentState.Type

export const ForumCreatorEarning = S.Struct({
  acceptedWorkPayoutEvidence: S.Literal(false),
  actionKind: ForumPaidActionKind,
  amount: ForumMoneyAmount,
  createdAt: S.String,
  creatorReceivedSpendableValue: S.Boolean,
  earningActorRef: ForumActorRef,
  earningRef: S.String,
  moneyActionRef: S.String,
  paymentEventRef: S.NullOr(S.String),
  paymentState: ForumCreatorEarningPaymentState,
  receiptRef: ForumReceiptRef,
  recipientActorRef: S.NullOr(ForumActorRef),
  settlementState: ForumTipSettlementState,
  target: ForumPaidActionTarget,
  targetPostPermalink: S.NullOr(S.String),
  tipSettlement: ForumTipSettlementProjection,
})
export type ForumCreatorEarning = typeof ForumCreatorEarning.Type

export const ForumCreatorEarningsSummary = S.Struct({
  creditedCount: S.Number,
  failedCount: S.Number,
  paidCount: S.Number,
  pendingCount: S.Number,
  refundedCount: S.Number,
  reversedCount: S.Number,
  settledCount: S.Number,
  sweptCount: S.Number,
  totalCount: S.Number,
  totalCreditedSats: S.Number,
  totalPaidSats: S.Number,
  totalSettledSats: S.Number,
  totalSweptSats: S.Number,
})
export type ForumCreatorEarningsSummary =
  typeof ForumCreatorEarningsSummary.Type

export const ForumCreatorEarningsResponse = S.Struct({
  actorRef: ForumActorRef,
  earnings: S.Array(ForumCreatorEarning),
  generatedAt: S.String,
  pagination: ForumPagination,
  staleness: PublicProjectionStalenessContract,
  summary: ForumCreatorEarningsSummary,
})
export type ForumCreatorEarningsResponse =
  typeof ForumCreatorEarningsResponse.Type

export const ForumTipLeaderboardPost = S.Struct({
  author: ForumActorSummary,
  postId: ForumUuid,
  postPermalink: S.String,
  postTitle: S.NullOr(S.String),
  tipCount: S.Number,
  topicId: ForumUuid,
  totalPaidSats: S.Number,
  totalSettledSats: S.Number,
})
export type ForumTipLeaderboardPost = typeof ForumTipLeaderboardPost.Type

export const ForumTipLeaderboardCreator = S.Struct({
  actor: ForumActorSummary,
  tipCount: S.Number,
  /** Ladder-credited sats not yet covered by a settled sweep (#4751). */
  totalCreditedSats: S.Number,
  totalPaidSats: S.Number,
  totalSettledSats: S.Number,
  /** Ladder-credited sats covered by settled sweeps, oldest first (#4753). */
  totalSweptSats: S.Number,
})
export type ForumTipLeaderboardCreator = typeof ForumTipLeaderboardCreator.Type

export const ForumTipLeaderboardsResponse = S.Struct({
  caveatRefs: S.Array(S.String),
  creators: S.Array(ForumTipLeaderboardCreator),
  generatedAt: S.String,
  posts: S.Array(ForumTipLeaderboardPost),
  staleness: PublicProjectionStalenessContract,
})
export type ForumTipLeaderboardsResponse =
  typeof ForumTipLeaderboardsResponse.Type

export const ForumTipReconciliationResponse = S.Struct({
  actorRef: S.NullOr(ForumActorRef),
  acceptedWorkPayoutBoundary: S.Literal(
    'ordinary_forum_tips_are_not_accepted_work',
  ),
  earnings: S.Array(ForumCreatorEarning),
  generatedAt: S.String,
  operatorCaveatRefs: S.Array(S.String),
  pagination: ForumPagination,
  staleness: PublicProjectionStalenessContract,
  summary: ForumCreatorEarningsSummary,
})
export type ForumTipReconciliationResponse =
  typeof ForumTipReconciliationResponse.Type

export const forumCreateTopicResponseHasFirstPost = (
  response: ForumCreateTopicResponse,
): boolean =>
  response.topic.firstPostId === response.firstPost.postId &&
  response.topic.topicId === response.firstPost.topicId &&
  response.firstPost.parentPostId === null &&
  response.firstPost.postNumber === 1

export const forumWriteDenialRequiresPayment = (
  denial: ForumWriteDenial,
): boolean => denial.denialKind === 'payment_required' && denial.payable

export const forumPublicProjectionIsPublicSafe = (
  projection: ForumPublicProjection,
): boolean =>
  projection.publicSafe &&
  projection.dataClassification === 'public' &&
  projection.trustTier !== 'blocked'

export class ForumPublicProjectionUnsafe extends S.TaggedErrorClass<ForumPublicProjectionUnsafe>()(
  'ForumPublicProjectionUnsafe',
  {
    reason: S.String,
  },
) {}

const prohibitedProjectionKeyPattern =
  /(raw|invoice|preimage|wallet|secret|provider|runner|private[_-]?channel|nostr[_-]?relay|workroom[_-]?log)/i

const prohibitedProjectionValuePattern =
  /(^|\b)(callback[_-]?token|github\.com\/[^:/]+\/private|lnbc|lntb|lnbcrt|payment[_-]?proof|preimage|mnemonic|provider[_-]?grant|raw[_-]?payload|raw[_-]?prompt|raw[_-]?runner|raw[_-]?run[_-]?log|sk-[a-z0-9]|wallet|xprv|payment_preimage=|raw_invoice|nostr_relay|provider_payload|\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})/i

const scanForumProjectionForUnsafeMaterial = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    return prohibitedProjectionValuePattern.test(value)
      ? path.join('.') || '<root>'
      : undefined
  }

  if (Array.isArray(value)) {
    return value
      .map((item, index) =>
        scanForumProjectionForUnsafeMaterial(item, [...path, String(index)]),
      )
      .find(unsafePath => unsafePath !== undefined)
  }

  if (value === null || typeof value !== 'object') {
    return undefined
  }

  return Object.entries(value)
    .map(([key, item]) =>
      prohibitedProjectionKeyPattern.test(key)
        ? [...path, key].join('.')
        : scanForumProjectionForUnsafeMaterial(item, [...path, key]),
    )
    .find(unsafePath => unsafePath !== undefined)
}

export const decodeForumPublicProjection = (
  value: unknown,
): ForumPublicProjection => {
  const unsafePath = scanForumProjectionForUnsafeMaterial(value)

  if (unsafePath !== undefined) {
    throw new ForumPublicProjectionUnsafe({
      reason: `Forum public projection contains private or payment material at ${unsafePath}.`,
    })
  }

  const projection = S.decodeUnknownSync(ForumPublicProjection)(value)

  if (!forumPublicProjectionIsPublicSafe(projection)) {
    throw new ForumPublicProjectionUnsafe({
      reason:
        'Forum public projection must be public-safe, public-classified, and unblocked.',
    })
  }

  return projection
}
