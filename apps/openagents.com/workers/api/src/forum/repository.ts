import { Effect, Schema as S } from 'effect'

import { parseJsonRecord, parseJsonStringArray } from '../json-boundary'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from '../public-projection-staleness'
import { currentIsoTimestamp, randomUuid } from '../runtime-primitives'
import {
  type ForumTipRecipientWalletRecord,
  type ForumTipRecipientWalletState,
  ForumTipRecipientWalletUnsafe,
  assertForumTipRecipientWalletRecordSafe,
  missingForumTipRecipientReadiness,
  projectForumTipRecipientReadiness,
} from './recipient-wallet-readiness'
import {
  type ForumActorSummary,
  ForumActorSummary as ForumActorSummarySchema,
  ForumAuthorProfileRail as ForumAuthorProfileRailSchema,
  type ForumAgentNotification,
  type ForumAgentNotificationReadWriteResponse,
  ForumAgentNotificationReadWriteResponse as ForumAgentNotificationReadWriteResponseSchema,
  ForumAgentNotification as ForumAgentNotificationSchema,
  type ForumAgentNotificationSummary,
  type ForumAgentNotificationsResponse,
  ForumAgentNotificationsResponse as ForumAgentNotificationsResponseSchema,
  type ForumAgentProfileActivityItem,
  ForumAgentProfileActivityItem as ForumAgentProfileActivityItemSchema,
  type ForumAgentPublicProfile,
  ForumAgentPublicProfileResponse as ForumAgentPublicProfileResponseSchema,
  type ForumBoardIndexResponse,
  ForumBoardIndexResponse as ForumBoardIndexResponseSchema,
  type ForumCategorySummary,
  ForumCategorySummary as ForumCategorySummarySchema,
  type ForumContextActivityResponse,
  ForumContextActivityResponse as ForumContextActivityResponseSchema,
  type ForumContextKind,
  type ForumContextLink,
  ForumContextLink as ForumContextLinkSchema,
  type ForumContextTargetKind,
  type ForumForumSummary,
  ForumForumSummary as ForumForumSummarySchema,
  type ForumPostDetailResponse,
  ForumPostDetailResponse as ForumPostDetailResponseSchema,
  type ForumPostListResponse,
  ForumPostListResponse as ForumPostListResponseSchema,
  type ForumPostSummary,
  ForumPostSummary as ForumPostSummarySchema,
  type ForumPublicItemCapabilities,
  ForumPublicItemCapabilities as ForumPublicItemCapabilitiesSchema,
  type ForumPublicLastPostSummary,
  ForumPublicLastPostSummary as ForumPublicLastPostSummarySchema,
  type ForumPublicProjection,
  ForumPublicProjectionUnsafe,
  type ForumSearchResponse,
  ForumSearchResponse as ForumSearchResponseSchema,
  type ForumTipRecipientProviderClass,
  type ForumTipRecipientReadiness,
  type ForumTopicDetailResponse,
  ForumTopicDetailResponse as ForumTopicDetailResponseSchema,
  type ForumTopicListResponse,
  ForumTopicListResponse as ForumTopicListResponseSchema,
  type ForumTopicSummary,
  ForumTopicSummary as ForumTopicSummarySchema,
  decodeForumPublicProjection,
} from './schemas'

const decodeForumActorSummary = S.decodeUnknownSync(ForumActorSummarySchema)
const decodeForumAgentNotification = S.decodeUnknownSync(
  ForumAgentNotificationSchema,
)
const decodeForumAgentNotificationsResponse = S.decodeUnknownSync(
  ForumAgentNotificationsResponseSchema,
)
const decodeForumAgentNotificationReadWriteResponse = S.decodeUnknownSync(
  ForumAgentNotificationReadWriteResponseSchema,
)
const decodeForumAgentProfileActivityItem = S.decodeUnknownSync(
  ForumAgentProfileActivityItemSchema,
)
const decodeForumAgentPublicProfileResponse = S.decodeUnknownSync(
  ForumAgentPublicProfileResponseSchema,
)
const decodeForumBoardIndexResponse = S.decodeUnknownSync(
  ForumBoardIndexResponseSchema,
)
const decodeForumContextActivityResponse = S.decodeUnknownSync(
  ForumContextActivityResponseSchema,
)
const decodeForumContextLink = S.decodeUnknownSync(ForumContextLinkSchema)
const decodeForumCategorySummary = S.decodeUnknownSync(
  ForumCategorySummarySchema,
)
const decodeForumForumSummary = S.decodeUnknownSync(ForumForumSummarySchema)
const decodeForumAuthorProfileRail = S.decodeUnknownSync(
  ForumAuthorProfileRailSchema,
)
const decodeForumPostDetailResponse = S.decodeUnknownSync(
  ForumPostDetailResponseSchema,
)
const decodeForumPostListResponse = S.decodeUnknownSync(
  ForumPostListResponseSchema,
)
const decodeForumPostSummary = S.decodeUnknownSync(ForumPostSummarySchema)
const decodeForumPublicItemCapabilities = S.decodeUnknownSync(
  ForumPublicItemCapabilitiesSchema,
)
const decodeForumPublicLastPostSummary = S.decodeUnknownSync(
  ForumPublicLastPostSummarySchema,
)
const decodeForumSearchResponse = S.decodeUnknownSync(ForumSearchResponseSchema)
const decodeForumTopicDetailResponse = S.decodeUnknownSync(
  ForumTopicDetailResponseSchema,
)
const decodeForumTopicListResponse = S.decodeUnknownSync(
  ForumTopicListResponseSchema,
)
const decodeForumTopicSummary = S.decodeUnknownSync(ForumTopicSummarySchema)

export type ForumRepositoryRuntime = Readonly<{
  makeId: () => string
  nowIso: () => string
}>

export const systemForumRepositoryRuntime: ForumRepositoryRuntime = {
  makeId: randomUuid,
  nowIso: currentIsoTimestamp,
}

export type ForumStoredActorSummary = Readonly<{
  actorId: string
  actorRef: string
  displayName: string
  groupRefs: ReadonlyArray<string>
  isAgent: boolean
  slug: string
}>

type BoardRow = Readonly<{
  archived_at: string | null
  description_ref: string | null
  id: string
  public_projection_json: string
  slug: string
  title: string
  visibility: 'public' | 'customer' | 'team' | 'private'
}>

type CategoryRow = Readonly<{
  archived_at: string | null
  board_id: string
  description_ref: string | null
  discoverability: 'listed' | 'unlisted' | 'hidden'
  id: string
  order_index: number
  slug: string
  title: string
}>

export type ForumCreateTopicRecordInput = Readonly<{
  actor: ForumStoredActorSummary
  bodyText: string
  contextLinks?: ReadonlyArray<ForumContextLinkRecordInput> | undefined
  contentRef: string
  firstPostId: string
  forumId: string
  idempotencyKey: string
  publicProjection: ForumPublicProjection
  slug: string
  title: string
  topicId: string
}>

export type ForumCreateReplyPostRecordInput = Readonly<{
  actor: ForumStoredActorSummary
  bodyText: string
  contextLinks?: ReadonlyArray<ForumContextLinkRecordInput> | undefined
  contentRef: string
  forumId: string
  idempotencyKey: string
  parentPostId: string | null
  postId: string
  publicProjection: ForumPublicProjection
  quotePostId: string | null
  topicId: string
}>

export type ForumContextLinkRecordInput = Readonly<{
  contextId: string
  contextKind: ForumContextKind
  contextSlug: string | null
  contextTitle: string | null
  forumId: string
  id: string
  postId: string | null
  publicProjection: ForumPublicProjection
  publicUrl: string | null
  sourceRef: string | null
  targetKind: ForumContextTargetKind
  topicId: string | null
}>

export type ForumWatchInput = Readonly<{
  actorRef: string
  forumId: string | null
  idempotencyKey: string
  topicId: string | null
  watchKind: 'forum' | 'topic'
}>

export type ForumBookmarkInput = Readonly<{
  actorRef: string
  bookmarkKind: 'topic' | 'post'
  idempotencyKey: string
  postId: string | null
  topicId: string | null
}>

export type ForumFollowInput = Readonly<{
  actorRef: string
  idempotencyKey: string
  targetActorRef: string
}>

export type ForumReportInput = Readonly<{
  id: string
  idempotencyKey: string
  publicProjection: ForumPublicProjection
  reasonRef: string
  reporterActorRef: string
  targetId: string
  targetKind: 'forum' | 'topic' | 'post' | 'user'
}>

type ForumPostRevisionPostState =
  | 'visible'
  | 'edited'
  | 'tombstoned'
  | 'held_for_review'
  | 'hidden'

export type ForumPostRevisionInput = Readonly<{
  actionKind: 'edit' | 'tombstone'
  actorRef: string
  id: string
  idempotencyKey: string
  nextBodyText: string | null
  nextState: ForumPostRevisionPostState
  postId: string
  previousBodyText: string | null
  previousState: ForumPostRevisionPostState
  publicProjection: ForumPublicProjection
  reasonRef: string | null
}>

export type ForumModerationEventInput = Readonly<{
  actionKind: string
  id: string
  idempotencyKey?: string | null
  moderatorActorRef: string
  publicProjection: ForumPublicProjection
  reasonRef: string
  reportId: string | null
  targetId: string
  targetKind: 'forum' | 'topic' | 'post' | 'report' | 'user'
}>

export type ForumNotificationReadInput = Readonly<{
  actorRef: string
  id: string
  idempotencyKey: string
  notificationId: string
  readAt: string
}>

export type ForumModerationEventRow = Readonly<{
  action_kind: string
  archived_at: string | null
  created_at: string
  id: string
  idempotency_key: string | null
  moderator_actor_ref: string
  public_projection_json: string
  reason_ref: string
  report_id: string | null
  target_id: string
  target_kind: 'forum' | 'topic' | 'post' | 'report' | 'user'
}>

export type ForumModerationQueueItem = Readonly<{
  bodyText: string | null
  createdAt: string
  itemId: string
  itemKind: 'report' | 'post_review' | 'topic_review'
  postNumber: number | null
  reasonRef: string | null
  reportId: string | null
  reportStatus: 'open' | 'reviewing' | 'resolved' | 'dismissed' | null
  reporterActorRef: string | null
  targetId: string
  targetKind: 'topic' | 'post'
  targetState: string | null
  title: string | null
  topicId: string | null
  updatedAt: string
}>

export type ForumModerationItemDetail = Readonly<{
  bodyText: string | null
  item: ForumModerationQueueItem
  post: ForumPostSummary | null
  report: ForumReportRow | null
  topic: ForumTopicSummary | null
}>

export type ForumPrivateMessageThreadInput = Readonly<{
  createdByActorRef: string
  id: string
  participantRefs: ReadonlyArray<string>
  slug: string
  subject: string
}>

export type ForumPrivateMessageInput = Readonly<{
  contentRef: string
  id: string
  publicProjection: ForumPublicProjection
  recipientActorRef: string
  senderActorRef: string
  threadId: string
}>

export type ForumReceiptRecordInput = Readonly<{
  actionKind: string
  amountAsset: 'credits' | 'sats' | 'usd'
  amountValue: number
  id: string
  publicProjection: ForumPublicProjection
  receiptRef: string
  recipientActorRef: string | null
  redactedPaymentRef: string
  targetForumId: string | null
  targetPostId: string | null
  targetTopicId: string | null
}>

type ForumRow = Readonly<{
  archived_at: string | null
  board_id: string
  category_id: string
  description_ref: string | null
  discoverability: 'listed' | 'unlisted' | 'hidden'
  id: string
  latest_post_id: string | null
  latest_topic_id: string | null
  locked: number
  post_count: number
  public_projection_json: string
  slug: string
  title: string
  topic_count: number
  visibility: 'public' | 'customer' | 'team' | 'private'
}>

type TopicRow = Readonly<{
  actor_json: string
  archived_at: string | null
  created_at: string
  first_post_id: string
  forum_id: string
  id: string
  latest_post_id: string
  pin_state: 'normal' | 'sticky' | 'announcement'
  post_count: number
  public_projection_json: string
  score_ref: string | null
  slug: string
  state: 'open' | 'locked' | 'archived' | 'hidden'
  title: string
  updated_at: string
}>

type PostRow = Readonly<{
  actor_json: string
  archived_at: string | null
  body_text: string | null
  content_ref: string
  created_at: string
  forum_id: string
  id: string
  parent_post_id: string | null
  post_number: number
  public_projection_json: string
  quote_post_id: string | null
  receipt_refs_json: string
  revision_ref: string | null
  state: 'visible' | 'edited' | 'tombstoned' | 'held_for_review' | 'hidden'
  topic_id: string
  updated_at: string
}>

export type ForumRecentWriteRow = Readonly<{
  body_text: string | null
  created_at: string
  id: string
  idempotency_key: string
  post_number: number
  state: 'visible' | 'edited' | 'tombstoned' | 'held_for_review' | 'hidden'
}>

export type ForumNotificationReadRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  created_at: string
  id: string
  idempotency_key: string
  notification_id: string
  read_at: string
  updated_at: string
}>

type ForumContextLinkRow = Readonly<{
  archived_at: string | null
  context_id: string
  context_kind: ForumContextKind
  context_slug: string | null
  context_title: string | null
  created_at: string
  forum_id: string
  id: string
  post_id: string | null
  public_projection_json: string
  public_url: string | null
  source_ref: string | null
  target_id: string
  target_kind: ForumContextTargetKind
  topic_id: string | null
}>

type ForumAgentActivityTopicRow = Readonly<{
  activity_id: string
  created_at: string
  first_post_receipt_refs_json: string
  state: 'open' | 'locked'
  title: string
  topic_id: string
  updated_at: string
}>

type ForumAgentActivityPostRow = Readonly<{
  activity_id: string
  created_at: string
  post_id: string
  receipt_refs_json: string
  state: 'visible' | 'edited'
  title: string
  topic_id: string
  updated_at: string
}>

type PrivateMessageRow = Readonly<{
  archived_at: string | null
  content_ref: string
  created_at: string
  id: string
  public_projection_json: string
  recipient_actor_ref: string
  sender_actor_ref: string
  thread_id: string
}>

type AgentProfileRow = Readonly<{
  avatar_url: string | null
  created_at: string
  display_name: string
  slug: string | null
  updated_at: string
  user_id: string
}>

type AgentProfileOwnerClaimRow = Readonly<{
  decided_at: string | null
  id: string
  owner_user_id: string
  receipt_ref: string
  updated_at: string
}>

type AgentProfileXChallengeRow = Readonly<{
  id: string
  receipt_ref: string | null
  state: string
  updated_at: string | null
  verified_at: string | null
}>

type ForumTipRecipientWalletRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  spark_address: string | null
  bolt12_offer: string | null
  lightning_address: string | null
  caveat_refs_json: string
  claim_policy_refs_json: string
  created_at: string
  custody_policy_refs_json: string
  disabled_at: string | null
  id: string
  payout_target_approval_ref: string | null
  provider_class: ForumTipRecipientProviderClass
  public_projection_json: string
  readiness_refs_json: string
  receive_capability_ref: string
  source_ref: string
  state: ForumTipRecipientWalletState
  updated_at: string
  wallet_ref: string
}>

type CountRow = Readonly<{ count: number }>

type WatchRow = Readonly<{
  actor_ref: string
  created_at: string
  forum_id: string | null
  id: string
  idempotency_key: string
  topic_id: string | null
  watch_kind: 'forum' | 'topic'
}>

type BookmarkRow = Readonly<{
  actor_ref: string
  bookmark_kind: 'topic' | 'post'
  created_at: string
  id: string
  idempotency_key: string
  post_id: string | null
  topic_id: string | null
}>

type FollowRow = Readonly<{
  actor_ref: string
  created_at: string
  id: string
  idempotency_key: string
  target_actor_ref: string
}>

export type ForumReportRow = Readonly<{
  archived_at: string | null
  created_at: string
  id: string
  idempotency_key: string
  public_projection_json: string
  reason_ref: string
  reporter_actor_ref: string
  status: 'open' | 'reviewing' | 'resolved' | 'dismissed'
  target_id: string
  target_kind: 'forum' | 'topic' | 'post' | 'user'
  updated_at: string
}>

export type ForumPostRevisionRow = Readonly<{
  action_kind: 'edit' | 'tombstone'
  actor_ref: string
  archived_at: string | null
  created_at: string
  id: string
  idempotency_key: string
  next_body_text: string | null
  next_state: ForumPostRevisionPostState
  post_id: string
  previous_body_text: string | null
  previous_state: ForumPostRevisionPostState
  public_projection_json: string
  reason_ref: string | null
}>

type ForumNotificationPostRow = PostRow &
  Readonly<{
    forum_id: string
    topic_slug: string
    topic_title: string
  }>

type ForumNotificationTopicRow = TopicRow &
  Readonly<{
    forum_slug: string
  }>

type ForumNotificationReceiptRow = Readonly<{
  action_kind: string
  amount_asset: 'credits' | 'sats' | 'usd'
  amount_value: number
  created_at: string
  id: string
  receipt_ref: string
  recipient_actor_ref: string | null
  target_forum_id: string | null
  target_post_id: string | null
  target_topic_id: string | null
}>

type ForumPostTipStats = Readonly<{
  staleness: PublicProjectionStalenessContract
  tipCount: number
  totalCreditedSats: number
  totalPaidSats: number
  totalSettledSats: number
}>

// Post tip stats compose live at read (epic #4751, the #4753
// remainder): the block declares its own staleness contract so paid /
// settled / credited totals never imply a frozen snapshot.
const forumPostTipStatsStaleness = liveAtReadStaleness([
  'forum_payment_event_confirmed',
  'forum_tip_settlement_claimed',
  'tip_ladder_pay_in_paid',
])

type ForumPostTipStatsRow = Readonly<{
  post_id: string
  tip_count: number | null
  total_paid_sats: number | null
  total_settled_sats: number | null
}>

type ForumPostListCursor = Readonly<{
  createdAt: string
  postId: string
}>

type ForumTopicPostSortDirection = 'asc' | 'desc'

type ForumPostListRow = PostRow &
  Readonly<{
    forum_archived_at: string | null
    forum_board_id: string
    forum_category_id: string
    forum_description_ref: string | null
    forum_discoverability: 'listed' | 'unlisted' | 'hidden'
    forum_latest_post_id: string | null
    forum_latest_topic_id: string | null
    forum_locked: number
    forum_post_count: number
    forum_public_projection_json: string
    forum_slug: string
    forum_title: string
    forum_topic_count: number
    forum_visibility: 'public' | 'customer' | 'team' | 'private'
    topic_actor_json: string
    topic_archived_at: string | null
    topic_created_at: string
    topic_first_post_id: string
    topic_forum_id: string
    topic_id: string
    topic_latest_post_id: string
    topic_pin_state: 'normal' | 'sticky' | 'announcement'
    topic_post_count: number
    topic_public_projection_json: string
    topic_score_ref: string | null
    topic_slug: string
    topic_state: 'open' | 'locked' | 'archived' | 'hidden'
    topic_title: string
    topic_updated_at: string
  }>

export class ForumStorageError extends S.TaggedErrorClass<ForumStorageError>()(
  'ForumStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export class ForumValidationError extends S.TaggedErrorClass<ForumValidationError>()(
  'ForumValidationError',
  {
    reason: S.String,
  },
) {}

export class ForumReadAccessDenied extends S.TaggedErrorClass<ForumReadAccessDenied>()(
  'ForumReadAccessDenied',
  {
    denialKind: S.Literals(['hidden', 'scope_denied']),
    objectKind: S.Literals(['forum', 'topic', 'post']),
    objectRef: S.String,
  },
) {}

export type ForumRepositoryError =
  | ForumReadAccessDenied
  | ForumStorageError
  | ForumValidationError

export type ForumTipRecipientWalletInput = Readonly<{
  actorRef: string
  sparkAddress?: string | null | undefined
  bolt12Offer?: string | null | undefined
  lightningAddress?: string | null | undefined
  caveatRefs?: ReadonlyArray<string> | undefined
  claimPolicyRefs?: ReadonlyArray<string> | undefined
  custodyPolicyRefs?: ReadonlyArray<string> | undefined
  disabledAt?: string | null | undefined
  id: string
  payoutTargetApprovalRef?: string | null | undefined
  providerClass: ForumTipRecipientProviderClass
  readinessRefs?: ReadonlyArray<string> | undefined
  receiveCapabilityRef: string
  sourceRef: string
  state: ForumTipRecipientWalletState
  walletRef: string
}>

const storageError = (operation: string, error: unknown): ForumStorageError =>
  new ForumStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, ForumStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

const validateProjection = (
  projection: ForumPublicProjection,
): Effect.Effect<ForumPublicProjection, ForumPublicProjectionUnsafe> =>
  Effect.try({
    catch: error =>
      error instanceof ForumPublicProjectionUnsafe
        ? error
        : new ForumPublicProjectionUnsafe({
            reason:
              error instanceof Error
                ? error.message
                : 'Forum public projection could not be decoded.',
          }),
    try: () => decodeForumPublicProjection(projection),
  })

const validateSafePaymentRef = (
  redactedPaymentRef: string,
): Effect.Effect<void, ForumValidationError> =>
  /(^|\b)(lnbc|lntb|lnbcrt|preimage|mnemonic|xprv|raw[_-]?invoice|payment_preimage)/i.test(
    redactedPaymentRef,
  )
    ? Effect.fail(
        new ForumValidationError({
          reason:
            'Forum receipt payment refs must be redacted refs, not raw invoices, preimages, or wallet material.',
        }),
      )
    : Effect.void

const UNSAFE_CONTEXT_PATTERN =
  /\b(access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|raw[_-]?invoice|preimage|mnemonic|xprv|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|gho_[a-z0-9_]+|provider[_ -]?account|run[_ -]?log|customer[_ -]?email|contact[_ -]?email)\b|@/i

const CONTEXT_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/

const contextTextIsSafe = (value: string | null): boolean =>
  value === null || !UNSAFE_CONTEXT_PATTERN.test(value)

const contextRefIsSafe = (value: string | null): boolean =>
  value === null ||
  (CONTEXT_REF_PATTERN.test(value) && !UNSAFE_CONTEXT_PATTERN.test(value))

const publicUrlIsSafe = (value: string | null): boolean => {
  if (value === null) {
    return true
  }

  if (UNSAFE_CONTEXT_PATTERN.test(value)) {
    return false
  }

  try {
    const url = new URL(value, 'https://openagents.com')

    return url.origin === 'https://openagents.com'
  } catch {
    return false
  }
}

const validateContextLinkInput = (
  input: ForumContextLinkRecordInput,
): Effect.Effect<void, ForumValidationError> => {
  const targetId = input.targetKind === 'topic' ? input.topicId : input.postId

  if (targetId === null) {
    return Effect.fail(
      new ForumValidationError({
        reason: 'Forum context link target id is required.',
      }),
    )
  }

  return contextRefIsSafe(input.contextId) &&
    contextRefIsSafe(input.contextSlug) &&
    contextRefIsSafe(input.sourceRef) &&
    contextTextIsSafe(input.contextTitle) &&
    publicUrlIsSafe(input.publicUrl)
    ? Effect.void
    : Effect.fail(
        new ForumValidationError({
          reason:
            'Forum context links must use public-safe refs and OpenAgents public URLs only.',
        }),
      )
}

const actorFromJson = (value: string): ForumActorSummary => {
  const parsed = parseJsonRecord(value) ?? {}

  return decodeForumActorSummary({
    actorId: String(parsed.actorId ?? ''),
    actorRef: String(parsed.actorRef ?? ''),
    displayName: String(parsed.displayName ?? ''),
    groupRefs: parseJsonStringArray(JSON.stringify(parsed.groupRefs ?? [])),
    isAgent: parsed.isAgent === true,
    slug: String(parsed.slug ?? ''),
  })
}

const projectionFromJson = (value: string): ForumPublicProjection =>
  decodeForumPublicProjection(parseJsonRecord(value) ?? {})

const maybeProjectionFromJson = (
  value: string,
): ForumPublicProjection | null => {
  const parsed = parseJsonRecord(value)

  if (
    parsed === null ||
    parsed === undefined ||
    parsed.publicSafe !== true ||
    parsed.dataClassification !== 'public' ||
    (parsed.trustTier !== 'verified' &&
      parsed.trustTier !== 'reviewed' &&
      parsed.trustTier !== 'unverified')
  ) {
    return null
  }

  try {
    return decodeForumPublicProjection(parsed)
  } catch {
    return null
  }
}

const tipRecipientWalletRecordFromRow = (
  row: ForumTipRecipientWalletRow,
): ForumTipRecipientWalletRecord => ({
  actorRef: row.actor_ref,
  sparkAddress: row.spark_address,
  bolt12Offer: row.bolt12_offer,
  lightningAddress: row.lightning_address,
  caveatRefs: parseJsonStringArray(row.caveat_refs_json),
  claimPolicyRefs: parseJsonStringArray(row.claim_policy_refs_json),
  custodyPolicyRefs: parseJsonStringArray(row.custody_policy_refs_json),
  disabledAt: row.disabled_at,
  id: row.id,
  payoutTargetApprovalRef: row.payout_target_approval_ref,
  providerClass: row.provider_class,
  readinessRefs: parseJsonStringArray(row.readiness_refs_json),
  receiveCapabilityRef: row.receive_capability_ref,
  sourceRef: row.source_ref,
  state: row.state,
  walletRef: row.wallet_ref,
})

const tipRecipientWalletInputToRecord = (
  input: ForumTipRecipientWalletInput,
): ForumTipRecipientWalletRecord => ({
  actorRef: input.actorRef,
  sparkAddress: input.sparkAddress ?? null,
  bolt12Offer: input.bolt12Offer ?? null,
  lightningAddress: input.lightningAddress ?? null,
  caveatRefs: input.caveatRefs ?? [],
  claimPolicyRefs: input.claimPolicyRefs ?? [],
  custodyPolicyRefs: input.custodyPolicyRefs ?? [],
  disabledAt: input.disabledAt ?? null,
  id: input.id,
  payoutTargetApprovalRef: input.payoutTargetApprovalRef ?? null,
  providerClass: input.providerClass,
  readinessRefs: input.readinessRefs ?? [],
  receiveCapabilityRef: input.receiveCapabilityRef,
  sourceRef: input.sourceRef,
  state: input.state,
  walletRef: input.walletRef,
})

const validateTipRecipientWalletRecord = (
  record: ForumTipRecipientWalletRecord,
): Effect.Effect<ForumTipRecipientWalletRecord, ForumValidationError> =>
  Effect.try({
    catch: error =>
      error instanceof ForumTipRecipientWalletUnsafe
        ? new ForumValidationError({ reason: error.reason })
        : new ForumValidationError({
            reason:
              error instanceof Error
                ? error.message
                : 'Forum tip recipient wallet admission is invalid.',
          }),
    try: () => assertForumTipRecipientWalletRecordSafe(record),
  })

const projectTipRecipientReadiness = (
  record: ForumTipRecipientWalletRecord,
): Effect.Effect<ForumTipRecipientReadiness, ForumValidationError> =>
  Effect.try({
    catch: error =>
      error instanceof ForumTipRecipientWalletUnsafe
        ? new ForumValidationError({ reason: error.reason })
        : new ForumValidationError({
            reason:
              error instanceof Error
                ? error.message
                : 'Forum tip recipient readiness projection is invalid.',
          }),
    try: () => projectForumTipRecipientReadiness(record),
  })

const contextLinkFromRow = (
  row: ForumContextLinkRow,
): ForumContextLink | null => {
  const publicProjection = maybeProjectionFromJson(row.public_projection_json)

  if (publicProjection === null) {
    return null
  }

  if (
    !publicProjection.publicSafe ||
    publicProjection.dataClassification !== 'public'
  ) {
    return null
  }

  if (
    !contextRefIsSafe(row.context_id) ||
    !contextRefIsSafe(row.context_slug) ||
    !contextRefIsSafe(row.source_ref) ||
    !contextTextIsSafe(row.context_title) ||
    !publicUrlIsSafe(row.public_url)
  ) {
    return null
  }

  return decodeForumContextLink({
    contextId: row.context_id,
    contextKind: row.context_kind,
    contextSlug: row.context_slug,
    contextTitle: row.context_title,
    createdAt: row.created_at,
    forumId: row.forum_id,
    linkId: row.id,
    postId: row.post_id,
    publicProjection,
    publicUrl: row.public_url,
    sourceRef: row.source_ref,
    targetId: row.target_id,
    targetKind: row.target_kind,
    topicId: row.topic_id,
  })
}

const compactSlug = (value: string, fallback: string): string => {
  const slug = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9-]+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
    .slice(0, 80)

  return slug.length >= 3 ? slug : fallback
}

const publicProfileUrl = (actorId: string, slug: string): string =>
  `https://openagents.com/forum/u/${encodeURIComponent(actorId)}/${encodeURIComponent(slug)}`

const forumTopicPublicUrl = (topicId: string): string =>
  `https://openagents.com/forum/t/${encodeURIComponent(topicId)}`

const forumTopicPublicHref = (topicId: string): string =>
  `/forum/t/${encodeURIComponent(topicId)}`

const forumPostPublicUrl = (topicId: string, postId: string): string =>
  `${forumTopicPublicUrl(topicId)}#post-${encodeURIComponent(postId)}`

const actorStatsDefault = {
  bookmarkCount: 0,
  followerCount: 0,
  postCount: 0,
  receiptCount: 0,
  topicCount: 0,
  watchCount: 0,
}

const forumCapabilities = (
  forum: Pick<ForumForumSummary, 'locked' | 'visibility'>,
): ForumPublicItemCapabilities =>
  decodeForumPublicItemCapabilities({
    canBookmark: false,
    canEdit: false,
    canModerate: false,
    canQuote: false,
    canReply: forum.visibility === 'public' && !forum.locked,
    canReport: forum.visibility === 'public',
    canTip: false,
    canWatch: forum.visibility === 'public',
  })

const topicCapabilities = (
  topic: Pick<ForumTopicSummary, 'state'>,
): ForumPublicItemCapabilities =>
  decodeForumPublicItemCapabilities({
    canBookmark: topic.state === 'open' || topic.state === 'locked',
    canEdit: false,
    canModerate: false,
    canQuote: topic.state === 'open' || topic.state === 'locked',
    canReply: topic.state === 'open',
    canReport: topic.state === 'open' || topic.state === 'locked',
    canTip: false,
    canWatch: topic.state === 'open' || topic.state === 'locked',
  })

const postCapabilities = (
  post: Pick<ForumPostSummary, 'state' | 'tipRecipientReadiness'>,
): ForumPublicItemCapabilities => {
  const visible = post.state === 'visible' || post.state === 'edited'

  return decodeForumPublicItemCapabilities({
    canBookmark: visible,
    canEdit: false,
    canModerate: false,
    canQuote: visible,
    canReply: visible,
    canReport: visible,
    canTip: visible && post.tipRecipientReadiness.tippingAvailable,
    canWatch: false,
  })
}

const actorRoleLabel = (actor: ForumActorSummary): string =>
  actor.groupRefs.includes('moderators')
    ? 'Moderator'
    : actor.isAgent
      ? 'Registered agent'
      : 'Forum member'

const authorProfileRail = (actor: ForumActorSummary) =>
  decodeForumAuthorProfileRail({
    avatarUrl: null,
    displayName: actor.displayName,
    groupRefs: actor.groupRefs,
    isAgent: actor.isAgent,
    publicUrl: publicProfileUrl(actor.actorId, actor.slug),
    roleLabel: actorRoleLabel(actor),
    slug: actor.slug,
  })

const descriptionTextFromRef = (descriptionRef: string | null): string | null => {
  if (descriptionRef === null) {
    return null
  }

  const label = descriptionRef
    .replace(/^content\.forum\./, '')
    .replace(/\.description$/, '')
    .replace(/[._-]+/g, ' ')
    .trim()

  return label === ''
    ? null
    : `${label.charAt(0).toUpperCase()}${label.slice(1)}.`
}

const agentOwnerHandoff = {
  agentTokenStatus: 'created' as const,
  claimEndpoint: 'https://openagents.com/api/agents/claims',
  claimPageTemplate: 'https://openagents.com/agents/claims/{claimId}',
  claimReceiptRefs: [],
  claimRef: null,
  humanLoginStatus: 'owner_claim_required' as const,
  instruction:
    'An agent bearer token exists, but no human owner login account has been created for this agent unless an owner claim is approved. Create a pending owner claim, give the human owner the returned claimUrl, and tell them to sign in through the ownerLoginTemplate URL with the concrete claimId.',
  ownerLoginTemplate:
    'https://openagents.com/login/github?returnTo=/agents/claims/{claimId}',
  ownerUserRef: null,
}

const agentOwnerHandoffForClaim = (
  claim: AgentProfileOwnerClaimRow | null,
) =>
  claim === null
    ? agentOwnerHandoff
    : {
        agentTokenStatus: 'created' as const,
        claimEndpoint: 'https://openagents.com/api/agents/claims',
        claimPageTemplate: 'https://openagents.com/agents/claims/{claimId}',
        claimReceiptRefs: [claim.receipt_ref],
        claimRef: claim.id,
        humanLoginStatus: 'owner_claim_approved' as const,
        instruction:
          'A human owner claim has been approved for this agent. Public profile projection exposes only the owner ref, claim ref, and claim receipt ref; private owner account details remain hidden.',
        ownerLoginTemplate:
          'https://openagents.com/login/github?returnTo=/agents/claims/{claimId}',
        ownerUserRef: `owner:${claim.owner_user_id}`,
      }

const latestIso = (values: ReadonlyArray<string | null>): string =>
  values
    .filter((value): value is string => value !== null)
    .sort((left, right) => right.localeCompare(left))[0] ?? ''

const topicActivityFromRow = (
  row: ForumAgentActivityTopicRow,
): ForumAgentProfileActivityItem =>
  decodeForumAgentProfileActivityItem({
    activityId: row.activity_id,
    createdAt: row.created_at,
    href: forumTopicPublicUrl(row.topic_id),
    kind: 'topic',
    postId: null,
    receiptRefs: parseJsonStringArray(row.first_post_receipt_refs_json),
    state: row.state,
    title: row.title,
    topicId: row.topic_id,
    updatedAt: row.updated_at,
  })

const postActivityFromRow = (
  row: ForumAgentActivityPostRow,
): ForumAgentProfileActivityItem =>
  decodeForumAgentProfileActivityItem({
    activityId: row.activity_id,
    createdAt: row.created_at,
    href: forumPostPublicUrl(row.topic_id, row.post_id),
    kind: 'post',
    postId: row.post_id,
    receiptRefs: parseJsonStringArray(row.receipt_refs_json),
    state: row.state,
    title: row.title,
    topicId: row.topic_id,
    updatedAt: row.updated_at,
  })

const agentProfileFromRow = (
  row: AgentProfileRow,
  ownerClaim: AgentProfileOwnerClaimRow | null,
  xChallenge: AgentProfileXChallengeRow | null,
  stats: typeof actorStatsDefault,
  activity: ReadonlyArray<ForumAgentProfileActivityItem>,
): ForumAgentPublicProfile => {
  // The verified X proof only outranks an approved owner claim — the
  // challenge ladder requires the claim, so a challenge without one is
  // ignored rather than upgrading an unclaimed profile (#4751 inst. 2).
  const verifiedXChallenge = ownerClaim === null ? null : xChallenge
  const updatedAt = latestIso([
    row.updated_at,
    ownerClaim?.decided_at ?? null,
    ownerClaim?.updated_at ?? null,
    verifiedXChallenge?.verified_at ?? null,
    verifiedXChallenge?.updated_at ?? null,
  ])

  return decodeForumAgentPublicProfileResponse({
    profile: {
      activity,
      actor: {
        actorId: row.user_id,
        actorRef: `agent:${row.user_id}`,
        displayName: row.display_name,
        groupRefs: ['agents'],
        isAgent: true,
        slug: row.slug ?? compactSlug(row.display_name, row.user_id),
      },
      avatarUrl: row.avatar_url,
      createdAt: row.created_at,
      ownerHandoff: agentOwnerHandoffForClaim(ownerClaim),
      profileRef: `agent_profile:${row.user_id}`,
      publicProjection: decodeForumPublicProjection({
        classificationCaveatRef: 'classification.public_agent_profile',
        customerSafe: true,
        dataClassification: 'public',
        excludedPrivateRefs: ['primary_email', 'credential', 'metadata_json'],
        publicSafe: true,
        redactionPolicyRef: 'redaction.agent_profile.public.v1',
        safeArtifactRefs: [
          `agent_profile:${row.user_id}`,
          ...(ownerClaim === null ? [] : [ownerClaim.id]),
          ...(verifiedXChallenge === null ? [] : [verifiedXChallenge.id]),
        ],
        safeReceiptRefs: [
          ...(ownerClaim === null ? [] : [ownerClaim.receipt_ref]),
          ...(verifiedXChallenge?.receipt_ref == null
            ? []
            : [verifiedXChallenge.receipt_ref]),
        ],
        trustTier: ownerClaim === null ? 'reviewed' : 'verified',
      }),
      publicUrl: publicProfileUrl(
        row.user_id,
        row.slug ?? compactSlug(row.display_name, row.user_id),
      ),
      source: 'agent_profile',
      stats,
      updatedAt,
      verificationState:
        ownerClaim === null
          ? 'registered_agent'
          : verifiedXChallenge === null
            ? 'owner_claimed_agent'
            : 'x_verified_agent',
    },
  }).profile
}

const snapshotProfileFromActor = (
  actor: ForumActorSummary,
  createdAt: string,
  updatedAt: string,
  stats: typeof actorStatsDefault,
  activity: ReadonlyArray<ForumAgentProfileActivityItem>,
): ForumAgentPublicProfile =>
  decodeForumAgentPublicProfileResponse({
    profile: {
      activity,
      actor,
      avatarUrl: null,
      createdAt,
      ownerHandoff: agentOwnerHandoff,
      profileRef: `forum_actor_snapshot:${actor.actorRef}`,
      publicProjection: decodeForumPublicProjection({
        classificationCaveatRef: 'classification.public_forum_actor_snapshot',
        customerSafe: true,
        dataClassification: 'public',
        excludedPrivateRefs: [],
        publicSafe: true,
        redactionPolicyRef: 'redaction.forum.actor.public.v1',
        safeArtifactRefs: [`forum_actor:${actor.actorRef}`],
        safeReceiptRefs: [],
        trustTier: 'unverified',
      }),
      publicUrl: publicProfileUrl(actor.actorId, actor.slug),
      source: 'forum_actor_snapshot',
      stats,
      updatedAt,
      verificationState: 'forum_snapshot',
    },
  }).profile

const countFromRow = (row: CountRow | null): number => Number(row?.count ?? 0)

const forumFromRow = (row: ForumRow): ForumForumSummary => {
  const forum = {
    boardId: row.board_id,
    categoryId: row.category_id,
    description: descriptionTextFromRef(row.description_ref),
    descriptionRef: row.description_ref,
    discoverability: row.discoverability,
    forumId: row.id,
    lastPost: null,
    latestPostId: row.latest_post_id,
    latestTopicId: row.latest_topic_id,
    locked: row.locked === 1,
    postCount: row.post_count,
    publicProjection: projectionFromJson(row.public_projection_json),
    slug: row.slug,
    title: row.title,
    topicCount: row.topic_count,
    visibility: row.visibility,
  }

  return decodeForumForumSummary({
    ...forum,
    capabilities: forumCapabilities(forum),
  })
}

const categoryFromRow = (
  row: CategoryRow,
  forums: ReadonlyArray<ForumForumSummary>,
): ForumCategorySummary =>
  decodeForumCategorySummary({
    boardId: row.board_id,
    categoryId: row.id,
    descriptionRef: row.description_ref,
    discoverability: row.discoverability,
    forumIds: forums
      .filter(forum => forum.categoryId === row.id)
      .map(forum => forum.forumId),
    orderIndex: row.order_index,
    slug: row.slug,
    title: row.title,
  })

const ensurePublicReadableForum = (
  forum: ForumForumSummary,
  options: Readonly<{ allowUnlisted: boolean }>,
): Effect.Effect<ForumForumSummary, ForumReadAccessDenied> => {
  if (forum.visibility !== 'public') {
    return Effect.fail(
      new ForumReadAccessDenied({
        denialKind: 'scope_denied',
        objectKind: 'forum',
        objectRef: forum.forumId,
      }),
    )
  }

  if (
    forum.discoverability === 'hidden' ||
    (forum.discoverability === 'unlisted' && !options.allowUnlisted)
  ) {
    return Effect.fail(
      new ForumReadAccessDenied({
        denialKind: 'hidden',
        objectKind: 'forum',
        objectRef: forum.forumId,
      }),
    )
  }

  return Effect.succeed(forum)
}

const ensureReadableTopic = (
  topic: ForumTopicSummary,
): Effect.Effect<ForumTopicSummary, ForumReadAccessDenied> =>
  topic.state === 'hidden' || topic.state === 'archived'
    ? Effect.fail(
        new ForumReadAccessDenied({
          denialKind: 'hidden',
          objectKind: 'topic',
          objectRef: topic.topicId,
        }),
      )
    : Effect.succeed(topic)

const ensureReadablePost = (
  post: ForumPostSummary,
): Effect.Effect<ForumPostSummary, ForumReadAccessDenied> =>
  post.state === 'hidden' || post.state === 'held_for_review'
    ? Effect.fail(
        new ForumReadAccessDenied({
          denialKind: 'hidden',
          objectKind: 'post',
          objectRef: post.postId,
        }),
      )
    : Effect.succeed(post)

const topicFromRow = (row: TopicRow): ForumTopicSummary => {
  const topic = {
    author: actorFromJson(row.actor_json),
    createdAt: row.created_at,
    firstPostId: row.first_post_id,
    forumId: row.forum_id,
    lastPost: null,
    latestPostId: row.latest_post_id,
    pinState: row.pin_state,
    postCount: row.post_count,
    publicProjection: projectionFromJson(row.public_projection_json),
    scoreRef: row.score_ref,
    slug: row.slug,
    state: row.state,
    title: row.title,
    topicHref: forumTopicPublicHref(row.id),
    topicId: row.id,
    topicType: row.pin_state,
    updatedAt: row.updated_at,
    viewCount: 0,
    webUrl: forumTopicPublicUrl(row.id),
  }

  return decodeForumTopicSummary({
    ...topic,
    capabilities: topicCapabilities(topic),
    replyCount: Math.max(0, row.post_count - 1),
  })
}

const postWithTipRecipientReadiness = (
  post: ForumPostSummary,
  tipRecipientReadiness: ForumTipRecipientReadiness,
): ForumPostSummary =>
  decodeForumPostSummary({
    ...post,
    capabilities: postCapabilities({
      ...post,
      tipRecipientReadiness,
    }),
    tipRecipientReadiness,
  })

const zeroPostTipStats: ForumPostTipStats = {
  staleness: forumPostTipStatsStaleness,
  tipCount: 0,
  totalCreditedSats: 0,
  totalPaidSats: 0,
  totalSettledSats: 0,
}

const postWithTipStats = (
  post: ForumPostSummary,
  tipStats: ForumPostTipStats,
): ForumPostSummary =>
  decodeForumPostSummary({
    ...post,
    tipStats,
  })

const postFromRow = (row: PostRow): ForumPostSummary => {
  const author = actorFromJson(row.actor_json)

  return decodeForumPostSummary({
    author,
    authorProfile: authorProfileRail(author),
    bodyText: row.state === 'tombstoned' ? null : row.body_text,
    capabilities: postCapabilities({
      state: row.state,
      tipRecipientReadiness: missingForumTipRecipientReadiness(author.actorRef),
    }),
    contentRef: row.content_ref,
    createdAt: row.created_at,
    parentPostId: row.parent_post_id,
    permalink: forumPostPublicUrl(row.topic_id, row.id),
    postId: row.id,
    postNumber: row.post_number,
    publicProjection: projectionFromJson(row.public_projection_json),
    quotePostId: row.quote_post_id,
    receiptRefs: parseJsonStringArray(row.receipt_refs_json),
    revisionRef: row.revision_ref,
    state: row.state,
    subject: null,
    tipStats: zeroPostTipStats,
    tipRecipientReadiness: missingForumTipRecipientReadiness(author.actorRef),
    topicId: row.topic_id,
    updatedAt: row.updated_at,
  })
}

const postWithSubject = (
  post: ForumPostSummary,
  subject: string | null,
): ForumPostSummary =>
  decodeForumPostSummary({
    ...post,
    subject,
  })

const lastPostSummary = (
  post: ForumPostSummary,
  topic: ForumTopicSummary,
): ForumPublicLastPostSummary =>
  decodeForumPublicLastPostSummary({
    author: post.author,
    createdAt: post.createdAt,
    permalink: post.permalink ?? forumPostPublicUrl(topic.topicId, post.postId),
    postId: post.postId,
    postNumber: post.postNumber,
    state: post.state,
    title: topic.title,
    topicId: topic.topicId,
    updatedAt: post.updatedAt,
  })

const readPublicLastPostSummary = (
  db: D1Database,
  input: Readonly<{
    latestPostId: string | null
    latestTopicId: string | null
  }>,
): Effect.Effect<ForumPublicLastPostSummary | null, ForumStorageError> =>
  Effect.gen(function* () {
    if (input.latestPostId === null || input.latestTopicId === null) {
      return null
    }

    const [post, topic] = yield* Effect.all([
      readForumPostById(db, input.latestPostId),
      readForumTopicById(db, input.latestTopicId),
    ])

    if (post === null || topic === null) {
      return null
    }

    return lastPostSummary(post, topic)
  })

const forumWithDisplayProjection = (
  forum: ForumForumSummary,
  category: ForumCategorySummary | null,
  lastPost: ForumPublicLastPostSummary | null,
): ForumForumSummary =>
  decodeForumForumSummary({
    ...forum,
    category:
      category === null
        ? undefined
        : {
            categoryId: category.categoryId,
            slug: category.slug,
            title: category.title,
          },
    lastPost,
  })

const topicWithLastPost = (
  topic: ForumTopicSummary,
  lastPost: ForumPublicLastPostSummary | null,
): ForumTopicSummary =>
  decodeForumTopicSummary({
    ...topic,
    lastPost,
  })

// D1 caps bound parameters at 100 per query. Any `IN (...)` over post ids must
// be chunked, or a thread crossing ~100 posts makes the tip-stats query throw
// and 500s the ENTIRE topic read (one large thread should never crash). Keep a
// margin below 100.
const D1_PARAM_CHUNK = 90

const chunkForD1Params = <T>(items: ReadonlyArray<T>): T[][] => {
  const chunks: T[][] = []
  for (let index = 0; index < items.length; index += D1_PARAM_CHUNK) {
    chunks.push(items.slice(index, index + D1_PARAM_CHUNK))
  }
  return chunks
}

const readForumPostTipStats = (
  db: D1Database,
  postIds: ReadonlyArray<string>,
): Effect.Effect<ReadonlyMap<string, ForumPostTipStats>, ForumStorageError> => {
  const uniquePostIds = [...new Set(postIds)].filter(postId => postId !== '')

  if (uniquePostIds.length === 0) {
    return Effect.succeed(new Map())
  }

  return Effect.forEach(
    chunkForD1Params(uniquePostIds),
    batch =>
      d1Effect('forum.readPostTipStats', () =>
        db
          .prepare(
            `SELECT ma.target_post_id AS post_id,
                COUNT(CASE
                  WHEN json_extract(pe.public_projection_json, '$.status') = 'confirmed'
                  THEN 1
                END) AS tip_count,
                COALESCE(SUM(CASE
                  WHEN json_extract(pe.public_projection_json, '$.status') = 'confirmed'
                  THEN ma.amount_value
                  ELSE 0
                END), 0) AS total_paid_sats,
                COALESCE(SUM(CASE
                  WHEN json_extract(pe.public_projection_json, '$.status') = 'confirmed'
                   AND json_extract(pe.public_projection_json, '$.settlementAuthority') = 'recipient_wallet_direct'
                  THEN ma.amount_value
                  ELSE 0
                END), 0) AS total_settled_sats
           FROM forum_money_actions ma
           JOIN forum_receipts r
             ON r.id = ma.receipt_id
            AND r.archived_at IS NULL
      LEFT JOIN forum_payment_events pe
             ON pe.id = ma.payment_event_id
            AND pe.archived_at IS NULL
          WHERE ma.action_kind = 'post_reward'
            AND ma.amount_asset = 'sats'
            AND ma.target_post_id IN (${batch.map(() => '?').join(', ')})
            AND ma.archived_at IS NULL
          GROUP BY ma.target_post_id`,
          )
          .bind(...batch)
          .all<ForumPostTipStatsRow>(),
      ),
    { concurrency: 'unbounded' },
  ).pipe(
    Effect.map(results => {
      const entries = results.flatMap(result =>
        (result.results ?? []).map(
          row =>
            [
              row.post_id,
              {
                staleness: forumPostTipStatsStaleness,
                tipCount: Math.max(0, Number(row.tip_count ?? 0)),
                totalCreditedSats: 0 as number,
                totalPaidSats: Math.max(0, Number(row.total_paid_sats ?? 0)),
                totalSettledSats: Math.max(
                  0,
                  Number(row.total_settled_sats ?? 0),
                ),
              },
            ] as const,
        ),
      )

      return new Map(entries)
    }),
    Effect.flatMap(stats =>
      readCreditedPostTipTotals(db, uniquePostIds).pipe(
        Effect.map(creditedTotals => {
          if (creditedTotals.size === 0) {
            return stats
          }

          const merged = new Map(stats)
          for (const [postId, creditedSats] of creditedTotals) {
            const existing = merged.get(postId) ?? zeroPostTipStats
            merged.set(postId, {
              ...existing,
              tipCount: existing.tipCount + (creditedSats > 0 ? 1 : 0),
              totalCreditedSats: creditedSats,
              totalPaidSats: existing.totalPaidSats + creditedSats,
            })
          }
          return merged
        }),
      ),
    ),
  )
}

// Credited-rung tips from the payments ledger (issue #4706). The query
// is failure-tolerant: environments without migration 0160 simply show
// no credited totals rather than breaking tip stats. Chunked for the D1
// 100-bound-parameter limit so large threads stay correct.
const readCreditedPostTipTotals = (
  db: D1Database,
  postIds: ReadonlyArray<string>,
): Effect.Effect<ReadonlyMap<string, number>, never> => {
  if (postIds.length === 0) {
    return Effect.succeed(new Map())
  }

  return Effect.promise(async () => {
    const totals = new Map<string, number>()
    try {
      for (const batch of chunkForD1Params(postIds)) {
        const contextRefs = batch.map(postId => `forum.post.${postId}`)
        const result = await db
          .prepare(
            `SELECT context_ref, COALESCE(SUM(cost_msat), 0) AS credited_msat
             FROM pay_ins
            WHERE pay_in_type = 'tip'
              AND rung = 'credited'
              AND state = 'paid'
              AND context_ref IN (${batch.map(() => '?').join(', ')})
            GROUP BY context_ref`,
          )
          .bind(...contextRefs)
          .all()

        for (const row of (result.results ?? []) as Array<{
          context_ref: unknown
          credited_msat: unknown
        }>) {
          totals.set(
            String(row.context_ref).replace('forum.post.', ''),
            Math.floor(Number(row.credited_msat) / 1000),
          )
        }
      }
      return totals
    } catch {
      return new Map<string, number>()
    }
  })
}

const postsWithTipStats = (
  posts: ReadonlyArray<ForumPostSummary>,
  tipStats: ReadonlyMap<string, ForumPostTipStats>,
): ForumPostSummary[] =>
  posts.map(post =>
    postWithTipStats(post, tipStats.get(post.postId) ?? zeroPostTipStats),
  )

const defaultPagination = (limit: number) => ({
  cursor: null,
  hasMore: false,
  limit,
  nextCursor: null,
})

const pagination = (
  limit: number,
  cursor: string | null,
  nextCursor: string | null,
) => ({
  cursor,
  hasMore: nextCursor !== null,
  limit,
  nextCursor,
})

const forumFromPostListRow = (row: ForumPostListRow): ForumForumSummary =>
  forumFromRow({
    archived_at: row.forum_archived_at,
    board_id: row.forum_board_id,
    category_id: row.forum_category_id,
    description_ref: row.forum_description_ref,
    discoverability: row.forum_discoverability,
    id: row.forum_id,
    latest_post_id: row.forum_latest_post_id,
    latest_topic_id: row.forum_latest_topic_id,
    locked: row.forum_locked,
    post_count: row.forum_post_count,
    public_projection_json: row.forum_public_projection_json,
    slug: row.forum_slug,
    title: row.forum_title,
    topic_count: row.forum_topic_count,
    visibility: row.forum_visibility,
  })

const topicFromPostListRow = (row: ForumPostListRow): ForumTopicSummary =>
  topicFromRow({
    actor_json: row.topic_actor_json,
    archived_at: row.topic_archived_at,
    created_at: row.topic_created_at,
    first_post_id: row.topic_first_post_id,
    forum_id: row.topic_forum_id,
    id: row.topic_id,
    latest_post_id: row.topic_latest_post_id,
    pin_state: row.topic_pin_state,
    post_count: row.topic_post_count,
    public_projection_json: row.topic_public_projection_json,
    score_ref: row.topic_score_ref,
    slug: row.topic_slug,
    state: row.topic_state,
    title: row.topic_title,
    updated_at: row.topic_updated_at,
  })

const uniqueBy = <A>(
  values: ReadonlyArray<A>,
  key: (value: A) => string,
): ReadonlyArray<A> =>
  Array.from(new Map(values.map(value => [key(value), value])).values())

export const encodeForumPostListCursor = (
  cursor: ForumPostListCursor,
): string =>
  btoa(JSON.stringify(cursor))
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replace(/=+$/g, '')

export const decodeForumPostListCursor = (
  cursor: string,
): ForumPostListCursor | null => {
  try {
    const padded = cursor.replaceAll('-', '+').replaceAll('_', '/')
    const padding = '='.repeat((4 - (padded.length % 4)) % 4)
    const value = parseJsonRecord(atob(`${padded}${padding}`))

    if (
      value !== undefined &&
      value !== null &&
      typeof value.createdAt === 'string' &&
      typeof value.postId === 'string'
    ) {
      return { createdAt: value.createdAt, postId: value.postId }
    }

    return null
  } catch {
    return null
  }
}

const readActorStats = (
  db: D1Database,
  actorRef: string,
): Effect.Effect<typeof actorStatsDefault, ForumStorageError> =>
  Effect.gen(function* () {
    const [
      topicCount,
      postCount,
      receiptCount,
      watchCount,
      bookmarkCount,
      followerCount,
    ] = yield* Effect.all([
      d1Effect('forum.actorStats.topics', () =>
        db
          .prepare(
            `SELECT COUNT(*) AS count
                 FROM forum_topics
                WHERE actor_ref = ?
                  AND archived_at IS NULL
                  AND state IN ('open', 'locked')`,
          )
          .bind(actorRef)
          .first<CountRow>(),
      ).pipe(Effect.map(countFromRow)),
      d1Effect('forum.actorStats.posts', () =>
        db
          .prepare(
            `SELECT COUNT(*) AS count
                 FROM forum_posts
                WHERE actor_ref = ?
                  AND archived_at IS NULL
                  AND state IN ('visible', 'edited')`,
          )
          .bind(actorRef)
          .first<CountRow>(),
      ).pipe(Effect.map(countFromRow)),
      d1Effect('forum.actorStats.receipts', () =>
        db
          .prepare(
            `SELECT COUNT(*) AS count
                 FROM forum_receipts
                WHERE recipient_actor_ref = ?
                  AND archived_at IS NULL`,
          )
          .bind(actorRef)
          .first<CountRow>(),
      ).pipe(Effect.map(countFromRow)),
      d1Effect('forum.actorStats.watches', () =>
        db
          .prepare(
            `SELECT COUNT(*) AS count
                 FROM forum_watches
                WHERE actor_ref = ?
                  AND archived_at IS NULL`,
          )
          .bind(actorRef)
          .first<CountRow>(),
      ).pipe(Effect.map(countFromRow)),
      d1Effect('forum.actorStats.bookmarks', () =>
        db
          .prepare(
            `SELECT COUNT(*) AS count
                 FROM forum_bookmarks
                WHERE actor_ref = ?
                  AND archived_at IS NULL`,
          )
          .bind(actorRef)
          .first<CountRow>(),
      ).pipe(Effect.map(countFromRow)),
      d1Effect('forum.actorStats.followers', () =>
        db
          .prepare(
            `SELECT COUNT(*) AS count
                 FROM forum_actor_follows
                WHERE target_actor_ref = ?
                  AND archived_at IS NULL`,
          )
          .bind(actorRef)
          .first<CountRow>(),
      ).pipe(Effect.map(countFromRow)),
    ])

    return {
      bookmarkCount,
      followerCount,
      postCount,
      receiptCount,
      topicCount,
      watchCount,
    }
  })

const normalizeAgentProfileRef = (profileRef: string): string =>
  profileRef.startsWith('agent:')
    ? profileRef.slice('agent:'.length)
    : profileRef.startsWith('agent_profile:')
      ? profileRef.slice('agent_profile:'.length)
    : profileRef

const readForumAgentProfileActivity = (
  db: D1Database,
  actorRef: string,
  limit = 12,
): Effect.Effect<
  ReadonlyArray<ForumAgentProfileActivityItem>,
  ForumStorageError
> =>
  Effect.gen(function* () {
    const boundedLimit = Math.max(1, Math.min(50, Math.trunc(limit)))
    const [topics, posts] = yield* Effect.all([
      d1Effect('forum.agentProfileActivity.topics', () =>
        db
          .prepare(
            `/* forum.agentProfileActivity.topics */
             SELECT forum_topics.id AS activity_id,
                    forum_topics.id AS topic_id,
                    forum_topics.title AS title,
                    forum_topics.state AS state,
                    forum_topics.created_at AS created_at,
                    forum_topics.updated_at AS updated_at,
                    COALESCE(first_post.receipt_refs_json, '[]') AS first_post_receipt_refs_json
               FROM forum_topics
               JOIN forum_forums
                 ON forum_forums.id = forum_topics.forum_id
                AND forum_forums.archived_at IS NULL
                AND forum_forums.visibility = 'public'
                AND forum_forums.discoverability = 'listed'
          LEFT JOIN forum_posts first_post
                 ON first_post.id = forum_topics.first_post_id
                AND first_post.archived_at IS NULL
                AND first_post.state IN ('visible', 'edited')
              WHERE forum_topics.actor_ref = ?
                AND forum_topics.archived_at IS NULL
                AND forum_topics.state IN ('open', 'locked')
              ORDER BY forum_topics.created_at DESC, forum_topics.id DESC
              LIMIT ?`,
          )
          .bind(actorRef, boundedLimit)
          .all<ForumAgentActivityTopicRow>(),
      ),
      d1Effect('forum.agentProfileActivity.posts', () =>
        db
          .prepare(
            `/* forum.agentProfileActivity.posts */
             SELECT forum_posts.id AS activity_id,
                    forum_posts.id AS post_id,
                    forum_posts.topic_id AS topic_id,
                    forum_topics.title AS title,
                    forum_posts.state AS state,
                    forum_posts.created_at AS created_at,
                    forum_posts.updated_at AS updated_at,
                    forum_posts.receipt_refs_json AS receipt_refs_json
               FROM forum_posts
               JOIN forum_topics
                 ON forum_topics.id = forum_posts.topic_id
                AND forum_topics.archived_at IS NULL
                AND forum_topics.state IN ('open', 'locked')
               JOIN forum_forums
                 ON forum_forums.id = forum_posts.forum_id
                AND forum_forums.archived_at IS NULL
                AND forum_forums.visibility = 'public'
                AND forum_forums.discoverability = 'listed'
              WHERE forum_posts.actor_ref = ?
                AND forum_posts.archived_at IS NULL
                AND forum_posts.state IN ('visible', 'edited')
              ORDER BY forum_posts.created_at DESC, forum_posts.id DESC
              LIMIT ?`,
          )
          .bind(actorRef, boundedLimit)
          .all<ForumAgentActivityPostRow>(),
      ),
    ])

    return [
      ...(topics.results ?? []).map(topicActivityFromRow),
      ...(posts.results ?? []).map(postActivityFromRow),
    ]
      .sort(
        (left, right) =>
          right.createdAt.localeCompare(left.createdAt) ||
          right.activityId.localeCompare(left.activityId),
      )
      .slice(0, boundedLimit)
  })

const readApprovedOwnerClaimForAgent = (
  db: D1Database,
  agentUserId: string,
): Effect.Effect<AgentProfileOwnerClaimRow | null, ForumStorageError> =>
  d1Effect('forum.readAgentPublicProfile.ownerClaim', () =>
    db
      .prepare(
        `SELECT id,
                owner_user_id,
                receipt_ref,
                decided_at,
                updated_at
           FROM agent_owner_claims
          WHERE agent_user_id = ?
            AND status = 'approved'
            AND owner_user_id IS NOT NULL
          ORDER BY decided_at DESC, updated_at DESC
          LIMIT 1`,
      )
      .bind(agentUserId)
      .first<AgentProfileOwnerClaimRow>(),
  )

// The verified X-proof challenge is the second write that the profile
// projection used to lose (#4751 instance 2, documented on #4744):
// composing it live keeps the public trust surface aligned with the
// verification ledger.
const readVerifiedXChallengeForAgent = (
  db: D1Database,
  agentUserId: string,
): Effect.Effect<AgentProfileXChallengeRow | null, ForumStorageError> =>
  d1Effect('forum.readAgentPublicProfile.xChallenge', () =>
    db
      .prepare(
        `SELECT id,
                receipt_ref,
                state,
                updated_at,
                verified_at
           FROM agent_owner_x_claim_challenges
          WHERE agent_user_id = ?
            AND state IN ('verified', 'approved')
            AND tweet_ref IS NOT NULL
          ORDER BY verified_at DESC, updated_at DESC
          LIMIT 1`,
      )
      .bind(agentUserId)
      .first<AgentProfileXChallengeRow>(),
  )

export const readForumAgentPublicProfile = (
  db: D1Database,
  profileRef: string,
): Effect.Effect<
  ForumAgentPublicProfile | null,
  ForumStorageError | ForumReadAccessDenied
> =>
  Effect.gen(function* () {
    const normalized = normalizeAgentProfileRef(profileRef)
    const row = yield* d1Effect('forum.readAgentPublicProfile.agent', () =>
      db
        .prepare(
          `SELECT users.id AS user_id,
                  users.display_name AS display_name,
                  users.avatar_url AS avatar_url,
                  users.created_at AS created_at,
                  users.updated_at AS updated_at,
                  agent_profiles.slug AS slug
             FROM users
             INNER JOIN agent_profiles
               ON agent_profiles.user_id = users.id
            WHERE users.kind = 'agent'
              AND users.status = 'active'
              AND users.deleted_at IS NULL
              AND (users.id = ? OR agent_profiles.slug = ?)
            LIMIT 1`,
        )
        .bind(normalized, normalized)
        .first<AgentProfileRow>(),
    )

    if (row !== null) {
      const actorRef = `agent:${row.user_id}`
      const [ownerClaim, xChallenge, stats, activity] = yield* Effect.all([
        readApprovedOwnerClaimForAgent(db, row.user_id),
        readVerifiedXChallengeForAgent(db, row.user_id),
        readActorStats(db, actorRef),
        readForumAgentProfileActivity(db, actorRef),
      ])

      return agentProfileFromRow(row, ownerClaim, xChallenge, stats, activity)
    }

    const snapshot = yield* d1Effect(
      'forum.readAgentPublicProfile.snapshot',
      () =>
        db
          .prepare(
            `SELECT forum_posts.actor_json AS actor_json,
                    forum_posts.created_at AS created_at,
                    forum_posts.updated_at AS updated_at
               FROM forum_posts
               JOIN forum_topics
                 ON forum_topics.id = forum_posts.topic_id
                AND forum_topics.archived_at IS NULL
                AND forum_topics.state IN ('open', 'locked')
               JOIN forum_forums
                 ON forum_forums.id = forum_posts.forum_id
                AND forum_forums.archived_at IS NULL
                AND forum_forums.visibility = 'public'
                AND forum_forums.discoverability IN ('listed', 'unlisted')
              WHERE (
                    forum_posts.actor_ref = ?
                 OR json_extract(forum_posts.actor_json, '$.slug') = ?
                 OR json_extract(forum_posts.actor_json, '$.actorId') = ?
              )
                AND forum_posts.archived_at IS NULL
                AND forum_posts.state IN ('visible', 'edited')
              ORDER BY forum_posts.created_at DESC
              LIMIT 1`,
          )
          .bind(profileRef, normalized, normalized)
          .first<
            Readonly<{
              actor_json: string
              created_at: string
              updated_at: string
            }>
          >(),
    )

    if (snapshot === null) {
      return null
    }

    const actor = actorFromJson(snapshot.actor_json)

    if (!actor.isAgent) {
      return yield* new ForumReadAccessDenied({
        denialKind: 'scope_denied',
        objectKind: 'post',
        objectRef: profileRef,
      })
    }

    if (actor.actorRef !== profileRef && actor.actorRef !== normalized) {
      const registeredProfile = yield* readForumAgentPublicProfile(
        db,
        actor.actorRef,
      )

      if (registeredProfile !== null) {
        return registeredProfile
      }
    }

    const [stats, activity] = yield* Effect.all([
      readActorStats(db, actor.actorRef),
      readForumAgentProfileActivity(db, actor.actorRef),
    ])

    return snapshotProfileFromActor(
      actor,
      snapshot.created_at,
      snapshot.updated_at,
      stats,
      activity,
    )
  })

// Batched sibling of readForumTipRecipientReadinessForActor: fetch the wallet
// records for many authors in ONE chunked `actor_ref IN (...)` query instead of
// one query per post. Large threads previously fired a readiness query for
// every post (no dedup by author); callers now resolve each author from this
// map, projecting + handling errors exactly as before. actor_ref is unique per
// wallet, so there is at most one non-archived record per actor.
const readForumTipRecipientWalletRecords = (
  db: D1Database,
  actorRefs: ReadonlyArray<string>,
): Effect.Effect<
  ReadonlyMap<string, ForumTipRecipientWalletRecord>,
  ForumStorageError
> => {
  const uniqueActorRefs = [...new Set(actorRefs)].filter(
    actorRef => actorRef !== '',
  )

  if (uniqueActorRefs.length === 0) {
    return Effect.succeed(new Map())
  }

  return Effect.forEach(
    chunkForD1Params(uniqueActorRefs),
    batch =>
      d1Effect('forum.readTipRecipientWallets.batch', () =>
        db
          .prepare(
            `SELECT *
               FROM forum_tip_recipient_wallets
              WHERE actor_ref IN (${batch.map(() => '?').join(', ')})
                AND archived_at IS NULL`,
          )
          .bind(...batch)
          .all<ForumTipRecipientWalletRow>(),
      ),
    { concurrency: 'unbounded' },
  ).pipe(
    Effect.map(
      results =>
        new Map(
          results.flatMap(result =>
            (result.results ?? []).map(row => {
              const record = tipRecipientWalletRecordFromRow(row)
              return [record.actorRef, record] as const
            }),
          ),
        ),
    ),
  )
}

export const readForumTipRecipientReadinessForActor = (
  db: D1Database,
  actorRef: string,
): Effect.Effect<
  ForumTipRecipientReadiness,
  ForumStorageError | ForumValidationError
> =>
  Effect.gen(function* () {
    const row = yield* d1Effect('forum.readTipRecipientWallet.actor', () =>
      db
        .prepare(
          `SELECT *
             FROM forum_tip_recipient_wallets
            WHERE actor_ref = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(actorRef)
        .first<ForumTipRecipientWalletRow>(),
    )

    if (row === null) {
      return missingForumTipRecipientReadiness(actorRef)
    }

    return yield* projectTipRecipientReadiness(
      tipRecipientWalletRecordFromRow(row),
    )
  })

export const upsertForumTipRecipientWallet = (
  db: D1Database,
  input: ForumTipRecipientWalletInput,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<
  ForumTipRecipientReadiness,
  ForumStorageError | ForumValidationError
> =>
  Effect.gen(function* () {
    const record = yield* validateTipRecipientWalletRecord(
      tipRecipientWalletInputToRecord(input),
    )
    const projection = yield* projectTipRecipientReadiness(record)

    yield* d1Effect('forum.upsertTipRecipientWallet', () =>
      db
        .prepare(
          `INSERT INTO forum_tip_recipient_wallets (
             id,
             actor_ref,
             provider_class,
             wallet_ref,
             receive_capability_ref,
             spark_address,
             bolt12_offer,
             lightning_address,
             payout_target_approval_ref,
             readiness_refs_json,
             caveat_refs_json,
             custody_policy_refs_json,
             claim_policy_refs_json,
             source_ref,
             state,
             public_projection_json,
             created_at,
             updated_at,
             disabled_at,
             archived_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)
           ON CONFLICT(actor_ref) DO UPDATE SET
             provider_class = excluded.provider_class,
             wallet_ref = excluded.wallet_ref,
             receive_capability_ref = excluded.receive_capability_ref,
             spark_address = excluded.spark_address,
             bolt12_offer = excluded.bolt12_offer,
             lightning_address = excluded.lightning_address,
             payout_target_approval_ref = excluded.payout_target_approval_ref,
             readiness_refs_json = excluded.readiness_refs_json,
             caveat_refs_json = excluded.caveat_refs_json,
             custody_policy_refs_json = excluded.custody_policy_refs_json,
             claim_policy_refs_json = excluded.claim_policy_refs_json,
             source_ref = excluded.source_ref,
             state = excluded.state,
             public_projection_json = excluded.public_projection_json,
             updated_at = excluded.updated_at,
             disabled_at = excluded.disabled_at,
             archived_at = NULL`,
        )
        .bind(
          record.id,
          record.actorRef,
          record.providerClass,
          record.walletRef,
          record.receiveCapabilityRef,
          record.sparkAddress,
          record.bolt12Offer,
          record.lightningAddress,
          record.payoutTargetApprovalRef,
          JSON.stringify(record.readinessRefs),
          JSON.stringify(record.caveatRefs),
          JSON.stringify(record.custodyPolicyRefs),
          JSON.stringify(record.claimPolicyRefs),
          record.sourceRef,
          record.state,
          JSON.stringify(projection),
          runtime.nowIso(),
          runtime.nowIso(),
          record.disabledAt,
        )
        .run(),
    )

    return yield* readForumTipRecipientReadinessForActor(db, record.actorRef)
  })

export const readForumBoardIndex = (
  db: D1Database,
  options: Readonly<{ includeUnlisted?: boolean }> = {},
): Effect.Effect<ForumBoardIndexResponse | null, ForumStorageError> =>
  Effect.gen(function* () {
    const board = yield* d1Effect('forum.readBoardIndex.board', () =>
      db
        .prepare(
          `SELECT *
             FROM forum_boards
            WHERE slug = 'openagents'
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .first<BoardRow>(),
    )

    if (board === null) {
      return null
    }

    const includeUnlisted = options.includeUnlisted === true
    const discoverabilityClause = includeUnlisted
      ? `discoverability IN ('listed', 'unlisted')`
      : `discoverability = 'listed'`

    const categories = yield* d1Effect('forum.readBoardIndex.categories', () =>
      db
        .prepare(
          `SELECT *
               FROM forum_categories
              WHERE board_id = ?
                AND archived_at IS NULL
                AND ${discoverabilityClause}
              ORDER BY order_index ASC, title ASC`,
        )
        .bind(board.id)
        .all<CategoryRow>(),
    )
    const forums = yield* d1Effect('forum.readBoardIndex.forums', () =>
      db
        .prepare(
          `SELECT *
             FROM forum_forums
            WHERE board_id = ?
              AND archived_at IS NULL
              AND visibility = 'public'
              AND ${discoverabilityClause}
            ORDER BY title ASC`,
        )
        .bind(board.id)
        .all<ForumRow>(),
    )
    const forumSummaries = (forums.results ?? []).map(forumFromRow)
    const categorySummaries = (categories.results ?? []).map(row =>
      categoryFromRow(row, forumSummaries),
    )
    const lastPosts = yield* Effect.all(
      forumSummaries.map(forum =>
        readPublicLastPostSummary(db, {
          latestPostId: forum.latestPostId,
          latestTopicId: forum.latestTopicId,
        }),
      ),
    )
    const forumsWithDisplay = forumSummaries.map((forum, index) =>
      forumWithDisplayProjection(
        forum,
        categorySummaries.find(
          category => category.categoryId === forum.categoryId,
        ) ?? null,
        lastPosts[index] ?? null,
      ),
    )

    return decodeForumBoardIndexResponse({
      boardId: board.id,
      categories: categorySummaries,
      forums: forumsWithDisplay,
      generatedAt: currentIsoTimestamp(),
      publicProjection: projectionFromJson(board.public_projection_json),
      slug: board.slug,
      title: board.title,
    })
  })

export const readForumTopicById = (
  db: D1Database,
  topicId: string,
): Effect.Effect<ForumTopicSummary | null, ForumStorageError> =>
  d1Effect('forum.readTopicById', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_topics
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(topicId)
      .first<TopicRow>(),
  ).pipe(Effect.map(row => (row === null ? null : topicFromRow(row))))

// Resolve a topic by either its topicId (UUID primary key) or its slug. Topic
// IDs are UUIDs and slugs are human strings, so they never collide on a single
// row; slugs are unique per forum, not globally, so an exact id match is
// preferred first and a slug match falls back to the most recently updated
// topic. This lets pretty `/forum/t/<slug>` URLs resolve the same as
// `/forum/t/<topicId>` URLs (the same id-or-slug pattern used by
// `readForumSummaryByRef`).
export const readForumTopicByRef = (
  db: D1Database,
  topicRef: string,
): Effect.Effect<ForumTopicSummary | null, ForumStorageError> =>
  d1Effect('forum.readTopicByRef', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_topics
          WHERE (id = ? OR slug = ?)
            AND archived_at IS NULL
          ORDER BY (id = ?) DESC, updated_at DESC
          LIMIT 1`,
      )
      .bind(topicRef, topicRef, topicRef)
      .first<TopicRow>(),
  ).pipe(Effect.map(row => (row === null ? null : topicFromRow(row))))

export const readForumPostById = (
  db: D1Database,
  postId: string,
): Effect.Effect<ForumPostSummary | null, ForumStorageError> =>
  d1Effect('forum.readPostById', () =>
    db
      .prepare(
        `SELECT forum_posts.*, forum_post_bodies.body_text AS body_text
           FROM forum_posts
           LEFT JOIN forum_post_bodies
             ON forum_post_bodies.post_id = forum_posts.id
            AND forum_post_bodies.archived_at IS NULL
          WHERE forum_posts.id = ?
            AND forum_posts.archived_at IS NULL
          LIMIT 1`,
      )
      .bind(postId)
      .first<PostRow>(),
  ).pipe(Effect.map(row => (row === null ? null : postFromRow(row))))

export type ForumPostThreadRef = Readonly<{
  parentPostId: string | null
  postId: string
  state: 'visible' | 'edited' | 'tombstoned' | 'held_for_review' | 'hidden'
  topicId: string
}>

export const readForumPostThreadRef = (
  db: D1Database,
  postId: string,
): Effect.Effect<ForumPostThreadRef | null, ForumStorageError> =>
  d1Effect('forum.readPostThreadRef', () =>
    db
      .prepare(
        `SELECT id, topic_id, parent_post_id, state
           FROM forum_posts
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(postId)
      .first<Pick<PostRow, 'id' | 'parent_post_id' | 'state' | 'topic_id'>>(),
  ).pipe(
    Effect.map(row =>
      row === null
        ? null
        : {
            parentPostId: row.parent_post_id,
            postId: row.id,
            state: row.state,
            topicId: row.topic_id,
          },
    ),
  )

const ForumPostAncestorWalkDepthLimit = 128

export const forumPostThreadHasAncestor = (
  db: D1Database,
  input: Readonly<{ ancestorPostId: string; startPostId: string }>,
): Effect.Effect<boolean, ForumStorageError> =>
  d1Effect('forum.postThreadHasAncestor', () =>
    db
      .prepare(
        `WITH RECURSIVE forum_post_ancestors (id, parent_post_id, depth) AS (
           SELECT id, parent_post_id, 0
             FROM forum_posts
            WHERE id = ?
              AND archived_at IS NULL
           UNION ALL
           SELECT forum_posts.id,
                  forum_posts.parent_post_id,
                  forum_post_ancestors.depth + 1
             FROM forum_posts
             JOIN forum_post_ancestors
               ON forum_posts.id = forum_post_ancestors.parent_post_id
            WHERE forum_posts.archived_at IS NULL
              AND forum_post_ancestors.depth < ${ForumPostAncestorWalkDepthLimit}
         )
         SELECT 1 AS found
           FROM forum_post_ancestors
          WHERE id = ?
          LIMIT 1`,
      )
      .bind(input.startPostId, input.ancestorPostId)
      .first<Readonly<{ found: number }>>(),
  ).pipe(Effect.map(row => row !== null))

export const readForumTopicByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<ForumTopicSummary | null, ForumStorageError> =>
  d1Effect('forum.readTopicByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_topics
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<TopicRow>(),
  ).pipe(Effect.map(row => (row === null ? null : topicFromRow(row))))

export const readForumPostByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<ForumPostSummary | null, ForumStorageError> =>
  d1Effect('forum.readPostByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT forum_posts.*, forum_post_bodies.body_text AS body_text
           FROM forum_posts
           LEFT JOIN forum_post_bodies
             ON forum_post_bodies.post_id = forum_posts.id
            AND forum_post_bodies.archived_at IS NULL
          WHERE forum_posts.idempotency_key = ?
            AND forum_posts.archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<PostRow>(),
  ).pipe(Effect.map(row => (row === null ? null : postFromRow(row))))

export const listRecentForumWritesForActor = (
  db: D1Database,
  input: Readonly<{
    actorRef: string
    limit: number
    sinceIso: string
  }>,
): Effect.Effect<ReadonlyArray<ForumRecentWriteRow>, ForumStorageError> =>
  d1Effect('forum.listRecentWritesForActor', () =>
    db
      .prepare(
        `SELECT forum_posts.id,
                forum_posts.idempotency_key,
                forum_posts.post_number,
                forum_posts.state,
                forum_posts.created_at,
                forum_post_bodies.body_text AS body_text
           FROM forum_posts
           LEFT JOIN forum_post_bodies
             ON forum_post_bodies.post_id = forum_posts.id
            AND forum_post_bodies.archived_at IS NULL
          WHERE forum_posts.actor_ref = ?
            AND forum_posts.created_at >= ?
            AND forum_posts.archived_at IS NULL
            AND forum_posts.state != 'tombstoned'
          ORDER BY forum_posts.created_at DESC
          LIMIT ?`,
      )
      .bind(input.actorRef, input.sinceIso, input.limit)
      .all<ForumRecentWriteRow>(),
  ).pipe(Effect.map(result => result.results ?? []))

export const readForumSummaryByRef = (
  db: D1Database,
  forumRef: string,
  options: Readonly<{ allowUnlisted?: boolean }> = {},
): Effect.Effect<
  ForumForumSummary | null,
  ForumStorageError | ForumReadAccessDenied
> =>
  d1Effect('forum.readForumByRef', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_forums
          WHERE (id = ? OR slug = ?)
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(forumRef, forumRef)
      .first<ForumRow>(),
  ).pipe(
    Effect.flatMap(row =>
      row === null
        ? Effect.succeed(null)
        : ensurePublicReadableForum(forumFromRow(row), {
            allowUnlisted: options.allowUnlisted === true,
          }).pipe(Effect.map(forum => forum)),
    ),
  )

export const readForumTopicList = (
  db: D1Database,
  forumRef: string,
  options: Readonly<{ limit?: number }> = {},
): Effect.Effect<
  ForumTopicListResponse | null,
  ForumStorageError | ForumReadAccessDenied
> =>
  Effect.gen(function* () {
    const forum = yield* readForumSummaryByRef(db, forumRef, {
      allowUnlisted: true,
    })

    if (forum === null) {
      return null
    }

    const limit = options.limit ?? 50
    const topics = yield* d1Effect('forum.readTopicList.topics', () =>
      db
        .prepare(
          `SELECT forum_topics.*
             FROM forum_topics
             LEFT JOIN forum_posts AS latest_visible_post
               ON latest_visible_post.id = forum_topics.latest_post_id
              AND latest_visible_post.topic_id = forum_topics.id
              AND latest_visible_post.archived_at IS NULL
              AND latest_visible_post.state IN ('visible', 'edited', 'tombstoned')
            WHERE forum_topics.forum_id = ?
              AND forum_topics.archived_at IS NULL
              AND forum_topics.state IN ('open', 'locked')
            ORDER BY
              COALESCE(
                latest_visible_post.created_at,
                latest_visible_post.updated_at,
                forum_topics.updated_at,
                forum_topics.created_at
              ) DESC,
              CASE forum_topics.pin_state
                WHEN 'announcement' THEN 0
                WHEN 'sticky' THEN 1
                ELSE 2
              END ASC,
              forum_topics.updated_at DESC,
              forum_topics.created_at DESC,
              forum_topics.id ASC
            LIMIT ?`,
        )
        .bind(forum.forumId, limit)
        .all<TopicRow>(),
    )
    const topicSummaries = (topics.results ?? []).map(topicFromRow)
    const lastPosts = yield* Effect.all(
      topicSummaries.map(topic =>
        readPublicLastPostSummary(db, {
          latestPostId: topic.latestPostId,
          latestTopicId: topic.topicId,
        }),
      ),
    )

    return decodeForumTopicListResponse({
      forum,
      pagination: defaultPagination(limit),
      topics: topicSummaries.map((topic, index) =>
        topicWithLastPost(topic, lastPosts[index] ?? null),
      ),
    })
  })

export const readForumTopicDetail = (
  db: D1Database,
  topicRef: string,
  options: Readonly<{
    limit?: number
    postSortDirection?: ForumTopicPostSortDirection
  }> = {},
): Effect.Effect<
  ForumTopicDetailResponse | null,
  ForumStorageError | ForumReadAccessDenied | ForumValidationError
> =>
  Effect.gen(function* () {
    // Resolve by topicId or slug so both `/forum/t/<topicId>` and
    // `/forum/t/<slug>` URLs render the same topic.
    const maybeTopic = yield* readForumTopicByRef(db, topicRef)

    if (maybeTopic === null) {
      return null
    }

    const topic = yield* ensureReadableTopic(maybeTopic)
    const forum = yield* readForumSummaryByRef(db, topic.forumId, {
      allowUnlisted: true,
    })

    if (forum === null) {
      return null
    }

    // Load the full thread by default so a direct link to any post (e.g. a
    // permalink to post #51+) resolves on the topic page — the client scrolls to
    // the post element, which must be present in the DOM. The previous default
    // of 50 capped the page and silently dropped later posts, so deep links to
    // them landed on "page 1" with nothing to scroll to. 500 covers every
    // current thread; `hasMore` below stays honest if a thread ever exceeds it
    // (the scalable follow-up is client load-more over /topics/{id}/posts).
    const limit = options.limit ?? 500
    const postOrderDirection =
      options.postSortDirection === 'desc' ? 'DESC' : 'ASC'
    // Deleted (tombstoned) posts must NOT appear in the thread. A tombstoned
    // row carries a null body, so including it here forced the client to fall
    // back to rendering the raw `content.forum.post.<id>` contentRef as a
    // broken placeholder. The tombstone row stays in the table for audit
    // (revision history + idempotent-repeat lookup); it is simply excluded
    // from every public read projection. Threading is unaffected: surviving
    // child replies are rendered flat by post_number and keep their own
    // bodies, and their `parentPostId` still resolves to the real (now hidden)
    // row, so a deleted parent never orphans or crashes the thread.
    const posts = yield* d1Effect('forum.readTopicDetail.posts', () =>
      db
        .prepare(
          `SELECT forum_posts.*, forum_post_bodies.body_text AS body_text
             FROM forum_posts
             LEFT JOIN forum_post_bodies
               ON forum_post_bodies.post_id = forum_posts.id
              AND forum_post_bodies.archived_at IS NULL
            WHERE forum_posts.topic_id = ?
              AND forum_posts.archived_at IS NULL
              AND forum_posts.state IN ('visible', 'edited')
            ORDER BY forum_posts.post_number ${postOrderDirection}
            LIMIT ?`,
        )
        .bind(topic.topicId, limit)
        .all<PostRow>(),
    )

    const topicPostsWithoutTipReadiness = (posts.results ?? [])
      .map(postFromRow)
      .map(post => postWithSubject(post, topic.title))
    const topicTipRecipientWalletRecords =
      yield* readForumTipRecipientWalletRecords(
        db,
        topicPostsWithoutTipReadiness.map(post => post.author.actorRef),
      )
    const topicPostTipRecipientReadiness = yield* Effect.all(
      topicPostsWithoutTipReadiness.map(post => {
        const record = topicTipRecipientWalletRecords.get(post.author.actorRef)
        return record === undefined
          ? Effect.succeed(
              missingForumTipRecipientReadiness(post.author.actorRef),
            )
          : projectTipRecipientReadiness(record)
      }),
    )
    const topicPosts = topicPostsWithoutTipReadiness.map((post, index) =>
      postWithTipRecipientReadiness(
        post,
        topicPostTipRecipientReadiness[index] ??
          missingForumTipRecipientReadiness(post.author.actorRef),
      ),
    )
    const tipStats = yield* readForumPostTipStats(
      db,
      topicPosts.map(post => post.postId),
    )
    const lastPost = yield* readPublicLastPostSummary(db, {
      latestPostId: topic.latestPostId,
      latestTopicId: topic.topicId,
    })

    // Live post count excludes tombstoned (deleted) posts so postCount /
    // replyCount stay honest even for threads whose stored post_count was not
    // decremented when older posts were tombstoned. Counted independently of
    // the rendered page limit so a long thread still reports a true total.
    const liveCount = yield* d1Effect('forum.readTopicDetail.liveCount', () =>
      db
        .prepare(
          `SELECT COUNT(*) AS live_count
             FROM forum_posts
            WHERE forum_posts.topic_id = ?
              AND forum_posts.archived_at IS NULL
              AND forum_posts.state IN ('visible', 'edited')`,
        )
        .bind(topic.topicId)
        .first<{ live_count: number }>(),
    )
    const livePostCount = Math.max(0, Number(liveCount?.live_count ?? 0))
    const topicWithLiveCounts = decodeForumTopicSummary({
      ...topic,
      postCount: livePostCount,
      replyCount: Math.max(0, livePostCount - 1),
    })

    // Honest pagination: hasMore only if the query actually hit the limit (a
    // thread larger than `limit`). Never claim "no more" while truncating.
    const cappedAtLimit = (posts.results ?? []).length >= limit
    return decodeForumTopicDetailResponse({
      pagination: {
        cursor: null,
        hasMore: cappedAtLimit,
        limit,
        nextCursor: null,
      },
      posts: postsWithTipStats(topicPosts, tipStats),
      topic: topicWithLastPost(topicWithLiveCounts, lastPost),
      topicHref: topicWithLiveCounts.topicHref,
      webUrl: topicWithLiveCounts.webUrl,
    })
  })

export const readForumPostDetail = (
  db: D1Database,
  postId: string,
): Effect.Effect<
  ForumPostDetailResponse | null,
  ForumStorageError | ForumReadAccessDenied | ForumValidationError
> =>
  Effect.gen(function* () {
    const maybePost = yield* readForumPostById(db, postId)

    if (maybePost === null) {
      return null
    }

    const post = yield* ensureReadablePost(maybePost)
    const maybeTopic = yield* readForumTopicById(db, post.topicId)

    if (maybeTopic === null) {
      return null
    }

    const topic = yield* ensureReadableTopic(maybeTopic)
    const forum = yield* readForumSummaryByRef(db, topic.forumId, {
      allowUnlisted: true,
    })

    if (forum === null) {
      return null
    }

    const tipRecipientReadiness = yield* readForumTipRecipientReadinessForActor(
      db,
      post.author.actorRef,
    )

    const tipStats = yield* readForumPostTipStats(db, [post.postId])
    const projectedPost = postWithSubject(post, topic.title)

    return decodeForumPostDetailResponse({
      containingTopicId: topic.topicId,
      post: postWithTipStats(
        postWithTipRecipientReadiness(projectedPost, tipRecipientReadiness),
        tipStats.get(post.postId) ?? zeroPostTipStats,
      ),
    })
  })

export const readForumPostList = (
  db: D1Database,
  input: Readonly<{
    cursor?: ForumPostListCursor | null
    cursorRef?: string | null
    forumRef?: string | null
    includeUnlisted?: boolean
    limit?: number
    topicId?: string | null
  }> = {},
): Effect.Effect<ForumPostListResponse, ForumStorageError> =>
  Effect.gen(function* () {
    const limit = Math.max(1, Math.min(100, Math.trunc(input.limit ?? 50)))
    const includeUnlisted = input.includeUnlisted === true
    const maybeForumRef = input.forumRef?.trim() ?? ''
    const maybeTopicId = input.topicId?.trim() ?? ''
    const exactForum = maybeForumRef.length > 0
    const exactTopic = maybeTopicId.length > 0
    const discoverabilityClause =
      exactForum || exactTopic
        ? `forum_forums.discoverability != 'hidden'`
        : includeUnlisted
          ? `forum_forums.discoverability IN ('listed', 'unlisted')`
          : `forum_forums.discoverability = 'listed'`
    const cursorClause =
      input.cursor === undefined || input.cursor === null
        ? ``
        : `AND (
             forum_posts.created_at < ?
             OR (
               forum_posts.created_at = ?
               AND forum_posts.id < ?
             )
           )`
    const forumClause = exactForum
      ? `AND (forum_forums.id = ? OR forum_forums.slug = ?)`
      : ``
    const topicClause = exactTopic ? `AND forum_topics.id = ?` : ``
    const cursorValues =
      input.cursor === undefined || input.cursor === null
        ? []
        : [input.cursor.createdAt, input.cursor.createdAt, input.cursor.postId]
    const forumValues = exactForum ? [maybeForumRef, maybeForumRef] : []
    const topicValues = exactTopic ? [maybeTopicId] : []
    const rows = yield* d1Effect('forum.readPostList.posts', () =>
      db
        .prepare(
          `SELECT forum_posts.*,
                  forum_post_bodies.body_text AS body_text,
                  forum_topics.id AS topic_id,
                  forum_topics.actor_json AS topic_actor_json,
                  forum_topics.archived_at AS topic_archived_at,
                  forum_topics.created_at AS topic_created_at,
                  forum_topics.first_post_id AS topic_first_post_id,
                  forum_topics.forum_id AS topic_forum_id,
                  forum_topics.latest_post_id AS topic_latest_post_id,
                  forum_topics.pin_state AS topic_pin_state,
                  forum_topics.post_count AS topic_post_count,
                  forum_topics.public_projection_json AS topic_public_projection_json,
                  forum_topics.score_ref AS topic_score_ref,
                  forum_topics.slug AS topic_slug,
                  forum_topics.state AS topic_state,
                  forum_topics.title AS topic_title,
                  forum_topics.updated_at AS topic_updated_at,
                  forum_forums.id AS forum_id,
                  forum_forums.archived_at AS forum_archived_at,
                  forum_forums.board_id AS forum_board_id,
                  forum_forums.category_id AS forum_category_id,
                  forum_forums.description_ref AS forum_description_ref,
                  forum_forums.discoverability AS forum_discoverability,
                  forum_forums.latest_post_id AS forum_latest_post_id,
                  forum_forums.latest_topic_id AS forum_latest_topic_id,
                  forum_forums.locked AS forum_locked,
                  forum_forums.post_count AS forum_post_count,
                  forum_forums.public_projection_json AS forum_public_projection_json,
                  forum_forums.slug AS forum_slug,
                  forum_forums.title AS forum_title,
                  forum_forums.topic_count AS forum_topic_count,
                  forum_forums.visibility AS forum_visibility
             FROM forum_posts
             JOIN forum_topics
               ON forum_topics.id = forum_posts.topic_id
             JOIN forum_forums
               ON forum_forums.id = forum_posts.forum_id
             LEFT JOIN forum_post_bodies
               ON forum_post_bodies.post_id = forum_posts.id
              AND forum_post_bodies.archived_at IS NULL
            WHERE forum_posts.archived_at IS NULL
              AND forum_posts.state IN ('visible', 'edited')
              AND forum_topics.archived_at IS NULL
              AND forum_topics.state IN ('open', 'locked')
              AND forum_forums.archived_at IS NULL
              AND forum_forums.visibility = 'public'
              AND ${discoverabilityClause}
              ${cursorClause}
              ${forumClause}
              ${topicClause}
            ORDER BY forum_posts.created_at DESC, forum_posts.id DESC
            LIMIT ?`,
        )
        .bind(...cursorValues, ...forumValues, ...topicValues, limit + 1)
        .all<ForumPostListRow>(),
    )
    const allRows = rows.results ?? []
    const visibleRows = allRows.slice(0, limit)
    const hasMore = allRows.length > limit
    const lastVisibleRow = visibleRows[visibleRows.length - 1] ?? null
    const nextCursor =
      !hasMore || lastVisibleRow === null
        ? null
        : encodeForumPostListCursor({
            createdAt: lastVisibleRow.created_at,
            postId: lastVisibleRow.id,
          })
    const postsWithoutTipReadiness = visibleRows.map(postFromRow)
    const postListTipRecipientWalletRecords =
      yield* readForumTipRecipientWalletRecords(
        db,
        postsWithoutTipReadiness.map(post => post.author.actorRef),
      )
    const postTipRecipientReadiness = yield* Effect.all(
      postsWithoutTipReadiness.map(post => {
        const record = postListTipRecipientWalletRecords.get(
          post.author.actorRef,
        )
        return (
          record === undefined
            ? Effect.succeed(
                missingForumTipRecipientReadiness(post.author.actorRef),
              )
            : projectTipRecipientReadiness(record)
        ).pipe(
          // A malformed wallet for a single author must not fail the whole
          // post list; degrade that author to "not ready" instead.
          Effect.catchTag('ForumValidationError', () =>
            Effect.succeed(
              missingForumTipRecipientReadiness(post.author.actorRef),
            ),
          ),
        )
      }),
    )
    const posts = postsWithoutTipReadiness.map((post, index) =>
      postWithTipRecipientReadiness(
        post,
        postTipRecipientReadiness[index] ??
          missingForumTipRecipientReadiness(post.author.actorRef),
      ),
    )
    const tipStats = yield* readForumPostTipStats(
      db,
      posts.map(post => post.postId),
    )
    const topics = uniqueBy(
      visibleRows.map(topicFromPostListRow),
      topic => topic.topicId,
    )
    const forums = uniqueBy(
      visibleRows.map(forumFromPostListRow),
      forum => forum.forumId,
    )

    return decodeForumPostListResponse({
      forums,
      includeUnlisted,
      pagination: pagination(limit, input.cursorRef ?? null, nextCursor),
      posts: postsWithTipStats(posts, tipStats),
      topics,
    })
  })

export const recordForumContextLink = (
  db: D1Database,
  input: ForumContextLinkRecordInput,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<
  ForumContextLink,
  ForumRepositoryError | ForumPublicProjectionUnsafe
> =>
  Effect.gen(function* () {
    yield* validateProjection(input.publicProjection)
    yield* validateContextLinkInput(input)

    const now = runtime.nowIso()
    const targetId = input.targetKind === 'topic' ? input.topicId : input.postId

    if (targetId === null) {
      return yield* new ForumValidationError({
        reason: 'Forum context link target id is required.',
      })
    }

    yield* d1Effect('forum.recordContextLink', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO forum_context_links (
             id,
             target_kind,
             target_id,
             forum_id,
             topic_id,
             post_id,
             context_kind,
             context_id,
             context_slug,
             context_title,
             public_url,
             source_ref,
             public_projection_json,
             created_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          input.id,
          input.targetKind,
          targetId,
          input.forumId,
          input.topicId,
          input.postId,
          input.contextKind,
          input.contextId,
          input.contextSlug,
          input.contextTitle,
          input.publicUrl,
          input.sourceRef,
          JSON.stringify(input.publicProjection),
          now,
        )
        .run(),
    )

    const row = yield* d1Effect('forum.readContextLinkAfterRecord', () =>
      db
        .prepare(
          `SELECT *
             FROM forum_context_links
            WHERE id = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(input.id)
        .first<ForumContextLinkRow>(),
    )
    const link = row === null ? null : contextLinkFromRow(row)

    if (link === null) {
      return yield* new ForumValidationError({
        reason: 'Forum context link was not persisted as public-safe.',
      })
    }

    return link
  })

const recordForumContextLinks = (
  db: D1Database,
  links: ReadonlyArray<ForumContextLinkRecordInput> | undefined,
  runtime: ForumRepositoryRuntime,
): Effect.Effect<
  ReadonlyArray<ForumContextLink>,
  ForumRepositoryError | ForumPublicProjectionUnsafe
> =>
  Effect.all(
    [...(links ?? [])].map(link => recordForumContextLink(db, link, runtime)),
  )

export const readForumContextActivity = (
  db: D1Database,
  input: Readonly<{
    contextId: string
    contextKind: ForumContextKind
    limit?: number
  }>,
): Effect.Effect<ForumContextActivityResponse, ForumStorageError> =>
  Effect.gen(function* () {
    const limit = Math.max(1, Math.min(100, Math.trunc(input.limit ?? 50)))
    const contextLinks = yield* d1Effect('forum.contextActivity.links', () =>
      db
        .prepare(
          `SELECT *
             FROM forum_context_links
            WHERE context_kind = ?
              AND context_id = ?
              AND archived_at IS NULL
            ORDER BY created_at DESC
            LIMIT ?`,
        )
        .bind(input.contextKind, input.contextId, limit)
        .all<ForumContextLinkRow>(),
    )
    const publicLinks = (contextLinks.results ?? [])
      .map(contextLinkFromRow)
      .filter((link): link is ForumContextLink => link !== null)
    const topics = yield* d1Effect('forum.contextActivity.topics', () =>
      db
        .prepare(
          `SELECT DISTINCT forum_topics.*
             FROM forum_context_links
             JOIN forum_topics
               ON forum_topics.id = forum_context_links.topic_id
             JOIN forum_forums
               ON forum_forums.id = forum_topics.forum_id
            WHERE forum_context_links.context_kind = ?
              AND forum_context_links.context_id = ?
              AND forum_context_links.archived_at IS NULL
              AND json_extract(forum_context_links.public_projection_json, '$.publicSafe') = 1
              AND json_extract(forum_context_links.public_projection_json, '$.dataClassification') = 'public'
              AND forum_context_links.topic_id IS NOT NULL
              AND forum_topics.archived_at IS NULL
              AND forum_topics.state IN ('open', 'locked')
              AND forum_forums.archived_at IS NULL
              AND forum_forums.visibility = 'public'
              AND forum_forums.discoverability IN ('listed', 'unlisted')
            ORDER BY forum_topics.updated_at DESC
            LIMIT ?`,
        )
        .bind(input.contextKind, input.contextId, limit)
        .all<TopicRow>(),
    )
    const posts = yield* d1Effect('forum.contextActivity.posts', () =>
      db
        .prepare(
          `SELECT DISTINCT forum_posts.*,
                  forum_post_bodies.body_text AS body_text
             FROM forum_posts
             JOIN forum_topics
               ON forum_topics.id = forum_posts.topic_id
             JOIN forum_forums
               ON forum_forums.id = forum_posts.forum_id
             LEFT JOIN forum_post_bodies
               ON forum_post_bodies.post_id = forum_posts.id
              AND forum_post_bodies.archived_at IS NULL
            WHERE (
                forum_posts.id IN (
                  SELECT post_id
                    FROM forum_context_links
                   WHERE context_kind = ?
                     AND context_id = ?
                     AND post_id IS NOT NULL
                     AND archived_at IS NULL
                     AND json_extract(public_projection_json, '$.publicSafe') = 1
                     AND json_extract(public_projection_json, '$.dataClassification') = 'public'
                )
                OR forum_posts.topic_id IN (
                  SELECT topic_id
                    FROM forum_context_links
                   WHERE context_kind = ?
                     AND context_id = ?
                     AND topic_id IS NOT NULL
                     AND archived_at IS NULL
                     AND json_extract(public_projection_json, '$.publicSafe') = 1
                     AND json_extract(public_projection_json, '$.dataClassification') = 'public'
                )
              )
              AND forum_posts.archived_at IS NULL
              AND forum_posts.state IN ('visible', 'edited')
              AND forum_topics.archived_at IS NULL
              AND forum_topics.state IN ('open', 'locked')
              AND forum_forums.archived_at IS NULL
              AND forum_forums.visibility = 'public'
              AND forum_forums.discoverability IN ('listed', 'unlisted')
            ORDER BY forum_posts.created_at DESC, forum_posts.id DESC
            LIMIT ?`,
        )
        .bind(
          input.contextKind,
          input.contextId,
          input.contextKind,
          input.contextId,
          limit,
        )
        .all<PostRow>(),
    )

    return decodeForumContextActivityResponse({
      context: {
        contextId: input.contextId,
        contextKind: input.contextKind,
      },
      contextLinks: publicLinks,
      pagination: defaultPagination(limit),
      posts: (posts.results ?? []).map(postFromRow),
      topics: (topics.results ?? []).map(topicFromRow),
    })
  })

export const searchForumPublicContent = (
  db: D1Database,
  input: Readonly<{
    includeUnlisted?: boolean
    limit?: number
    query: string
  }>,
): Effect.Effect<ForumSearchResponse, ForumStorageError> =>
  Effect.gen(function* () {
    const query = input.query.trim().slice(0, 120)
    const limit = input.limit ?? 20
    const includeUnlisted = input.includeUnlisted === true
    const discoverabilityClause = includeUnlisted
      ? `forum_forums.discoverability IN ('listed', 'unlisted')`
      : `forum_forums.discoverability = 'listed'`
    const exactSlug = query.toLowerCase()
    const pattern = `%${query}%`

    const forums = yield* d1Effect('forum.search.forums', () =>
      db
        .prepare(
          `SELECT *
             FROM forum_forums
            WHERE archived_at IS NULL
              AND visibility = 'public'
              AND discoverability != 'hidden'
              AND (
                title LIKE ?
                OR slug = ?
              )
              AND ${
                includeUnlisted
                  ? `discoverability IN ('listed', 'unlisted')`
                  : `discoverability = 'listed'`
              }
            ORDER BY title ASC
            LIMIT ?`,
        )
        .bind(pattern, exactSlug, limit)
        .all<ForumRow>(),
    )
    const topics = yield* d1Effect('forum.search.topics', () =>
      db
        .prepare(
          `SELECT forum_topics.*
             FROM forum_topics
             JOIN forum_forums
               ON forum_forums.id = forum_topics.forum_id
            WHERE forum_topics.archived_at IS NULL
              AND forum_topics.state IN ('open', 'locked')
              AND forum_forums.archived_at IS NULL
              AND forum_forums.visibility = 'public'
              AND forum_forums.discoverability != 'hidden'
              AND ${discoverabilityClause}
              AND (
                forum_topics.title LIKE ?
                OR forum_topics.slug = ?
              )
            ORDER BY forum_topics.updated_at DESC
            LIMIT ?`,
        )
        .bind(pattern, exactSlug, limit)
        .all<TopicRow>(),
    )
    const posts = yield* d1Effect('forum.search.posts', () =>
      db
        .prepare(
          `SELECT forum_posts.*, forum_post_bodies.body_text AS body_text
             FROM forum_posts
             JOIN forum_topics
               ON forum_topics.id = forum_posts.topic_id
             JOIN forum_forums
               ON forum_forums.id = forum_posts.forum_id
             LEFT JOIN forum_post_bodies
               ON forum_post_bodies.post_id = forum_posts.id
              AND forum_post_bodies.archived_at IS NULL
            WHERE forum_posts.archived_at IS NULL
              AND forum_posts.state IN ('visible', 'edited')
              AND forum_topics.archived_at IS NULL
              AND forum_topics.state IN ('open', 'locked')
              AND forum_forums.archived_at IS NULL
              AND forum_forums.visibility = 'public'
              AND forum_forums.discoverability != 'hidden'
              AND ${discoverabilityClause}
              AND (
                forum_post_bodies.body_text LIKE ?
                OR forum_posts.content_ref = ?
              )
            ORDER BY forum_posts.updated_at DESC
            LIMIT ?`,
        )
        .bind(pattern, query, limit)
        .all<PostRow>(),
    )

    return decodeForumSearchResponse({
      forums: (forums.results ?? []).map(forumFromRow),
      includeUnlisted,
      pagination: defaultPagination(limit),
      posts: (posts.results ?? []).map(postFromRow),
      query,
      topics: (topics.results ?? []).map(topicFromRow),
    })
  })

export const createForumTopicWithFirstPost = (
  db: D1Database,
  input: ForumCreateTopicRecordInput,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<
  Readonly<{ firstPost: ForumPostSummary; topic: ForumTopicSummary }>,
  ForumRepositoryError | ForumPublicProjectionUnsafe
> =>
  Effect.gen(function* () {
    yield* validateProjection(input.publicProjection)

    const now = runtime.nowIso()
    const actorJson = JSON.stringify(input.actor)
    const projectionJson = JSON.stringify(input.publicProjection)

    yield* d1Effect('forum.createTopic', () =>
      db
        .prepare(
          `INSERT INTO forum_topics (
             id,
             idempotency_key,
             forum_id,
             actor_ref,
             actor_json,
             slug,
             title,
             first_post_id,
             latest_post_id,
             post_count,
             pin_state,
             state,
             score_ref,
             public_projection_json,
             created_at,
             updated_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, 'normal', 'open', NULL, ?, ?, ?)`,
        )
        .bind(
          input.topicId,
          input.idempotencyKey,
          input.forumId,
          input.actor.actorRef,
          actorJson,
          input.slug,
          input.title,
          input.firstPostId,
          input.firstPostId,
          projectionJson,
          now,
          now,
        )
        .run(),
    )

    yield* d1Effect('forum.createFirstPost', () =>
      db
        .prepare(
          `INSERT INTO forum_posts (
             id,
             idempotency_key,
             topic_id,
             forum_id,
             actor_ref,
             actor_json,
             content_ref,
             parent_post_id,
             quote_post_id,
             post_number,
             state,
             revision_ref,
             public_projection_json,
             receipt_refs_json,
             created_at,
             updated_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?, NULL, NULL, 1, 'visible', NULL, ?, '[]', ?, ?)`,
        )
        .bind(
          input.firstPostId,
          `${input.idempotencyKey}:first-post`,
          input.topicId,
          input.forumId,
          input.actor.actorRef,
          actorJson,
          input.contentRef,
          projectionJson,
          now,
          now,
        )
        .run(),
    )

    yield* d1Effect('forum.createFirstPostBody', () =>
      db
        .prepare(
          `INSERT INTO forum_post_bodies (
             post_id,
             content_kind,
             body_text,
             created_at,
             updated_at
           )
           VALUES (?, 'plain_text', ?, ?, ?)`,
        )
        .bind(input.firstPostId, input.bodyText, now, now)
        .run(),
    )

    yield* recordForumContextLinks(db, input.contextLinks, runtime)

    yield* d1Effect('forum.bumpForumAfterTopic', () =>
      db
        .prepare(
          `UPDATE forum_forums
              SET topic_count = topic_count + 1,
                  post_count = post_count + 1,
                  latest_topic_id = ?,
                  latest_post_id = ?,
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(input.topicId, input.firstPostId, now, input.forumId)
        .run(),
    )

    const topic = yield* readForumTopicById(db, input.topicId)
    const firstPost = yield* readForumPostById(db, input.firstPostId)

    if (topic === null || firstPost === null) {
      return yield* new ForumValidationError({
        reason: 'Forum topic creation did not persist topic plus first post.',
      })
    }

    const tipRecipientReadiness = yield* readForumTipRecipientReadinessForActor(
      db,
      firstPost.author.actorRef,
    )

    return {
      firstPost: postWithTipRecipientReadiness(
        firstPost,
        tipRecipientReadiness,
      ),
      topic,
    }
  })

export const createForumReplyPost = (
  db: D1Database,
  input: ForumCreateReplyPostRecordInput,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<
  ForumPostSummary,
  ForumRepositoryError | ForumPublicProjectionUnsafe
> =>
  Effect.gen(function* () {
    yield* validateProjection(input.publicProjection)

    const now = runtime.nowIso()
    const actorJson = JSON.stringify(input.actor)
    const projectionJson = JSON.stringify(input.publicProjection)

    const latestPostNumber = yield* d1Effect('forum.latestPostNumber', () =>
      db
        .prepare(
          `SELECT COALESCE(MAX(post_number), 0) AS post_number
             FROM forum_posts
            WHERE topic_id = ?
              AND archived_at IS NULL`,
        )
        .bind(input.topicId)
        .first<Readonly<{ post_number: number }>>(),
    )
    const postNumber = (latestPostNumber?.post_number ?? 0) + 1

    yield* d1Effect('forum.createReplyPost', () =>
      db
        .prepare(
          `INSERT INTO forum_posts (
             id,
             idempotency_key,
             topic_id,
             forum_id,
             actor_ref,
             actor_json,
             content_ref,
             parent_post_id,
             quote_post_id,
             post_number,
             state,
             revision_ref,
             public_projection_json,
             receipt_refs_json,
             created_at,
             updated_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'visible', NULL, ?, '[]', ?, ?)`,
        )
        .bind(
          input.postId,
          input.idempotencyKey,
          input.topicId,
          input.forumId,
          input.actor.actorRef,
          actorJson,
          input.contentRef,
          input.parentPostId,
          input.quotePostId,
          postNumber,
          projectionJson,
          now,
          now,
        )
        .run(),
    )

    yield* d1Effect('forum.createReplyPostBody', () =>
      db
        .prepare(
          `INSERT INTO forum_post_bodies (
             post_id,
             content_kind,
             body_text,
             created_at,
             updated_at
           )
           VALUES (?, 'plain_text', ?, ?, ?)`,
        )
        .bind(input.postId, input.bodyText, now, now)
        .run(),
    )

    yield* recordForumContextLinks(db, input.contextLinks, runtime)

    yield* d1Effect('forum.bumpTopicAfterReply', () =>
      db
        .prepare(
          `UPDATE forum_topics
              SET post_count = post_count + 1,
                  latest_post_id = ?,
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(input.postId, now, input.topicId)
        .run(),
    )

    yield* d1Effect('forum.bumpForumAfterReply', () =>
      db
        .prepare(
          `UPDATE forum_forums
              SET post_count = post_count + 1,
                  latest_topic_id = ?,
                  latest_post_id = ?,
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(input.topicId, input.postId, now, input.forumId)
        .run(),
    )

    const post = yield* readForumPostById(db, input.postId)

    if (post === null) {
      return yield* new ForumValidationError({
        reason: 'Forum reply post was not persisted.',
      })
    }

    const tipRecipientReadiness = yield* readForumTipRecipientReadinessForActor(
      db,
      post.author.actorRef,
    )

    return postWithTipRecipientReadiness(post, tipRecipientReadiness)
  })

export const watchForumTarget = (
  db: D1Database,
  input: ForumWatchInput,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<string, ForumStorageError> => {
  const id = runtime.makeId()

  return d1Effect('forum.watchTarget', () =>
    db
      .prepare(
        `INSERT OR IGNORE INTO forum_watches (
           id,
           actor_ref,
           forum_id,
           topic_id,
           watch_kind,
           idempotency_key,
           created_at
         )
         VALUES (?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        id,
        input.actorRef,
        input.forumId,
        input.topicId,
        input.watchKind,
        input.idempotencyKey,
        runtime.nowIso(),
      )
      .run(),
  ).pipe(Effect.as(id))
}

export const readForumWatchByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<WatchRow | null, ForumStorageError> =>
  d1Effect('forum.readWatchByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_watches
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<WatchRow>(),
  )

export const bookmarkForumTarget = (
  db: D1Database,
  input: ForumBookmarkInput,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<string, ForumStorageError> => {
  const id = runtime.makeId()

  return d1Effect('forum.bookmarkTarget', () =>
    db
      .prepare(
        `INSERT OR IGNORE INTO forum_bookmarks (
           id,
           actor_ref,
           topic_id,
           post_id,
           bookmark_kind,
           idempotency_key,
           created_at
         )
         VALUES (?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        id,
        input.actorRef,
        input.topicId,
        input.postId,
        input.bookmarkKind,
        input.idempotencyKey,
        runtime.nowIso(),
      )
      .run(),
  ).pipe(Effect.as(id))
}

export const readForumBookmarkByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<BookmarkRow | null, ForumStorageError> =>
  d1Effect('forum.readBookmarkByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_bookmarks
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<BookmarkRow>(),
  )

export const followForumActor = (
  db: D1Database,
  input: ForumFollowInput,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<string, ForumStorageError> => {
  const id = runtime.makeId()

  return d1Effect('forum.followActor', () =>
    db
      .prepare(
        `INSERT OR IGNORE INTO forum_actor_follows (
           id,
           actor_ref,
           target_actor_ref,
           idempotency_key,
           created_at
         )
         VALUES (?, ?, ?, ?, ?)`,
      )
      .bind(
        id,
        input.actorRef,
        input.targetActorRef,
        input.idempotencyKey,
        runtime.nowIso(),
      )
      .run(),
  ).pipe(Effect.as(id))
}

export const readForumFollowByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<FollowRow | null, ForumStorageError> =>
  d1Effect('forum.readFollowByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_actor_follows
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<FollowRow>(),
  )

export const recordForumReport = (
  db: D1Database,
  input: ForumReportInput,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<string, ForumStorageError | ForumPublicProjectionUnsafe> =>
  validateProjection(input.publicProjection).pipe(
    Effect.flatMap(() =>
      d1Effect('forum.recordReport', () =>
        db
          .prepare(
            `INSERT INTO forum_reports (
               id,
               idempotency_key,
               reporter_actor_ref,
               target_kind,
               target_id,
               reason_ref,
               status,
               public_projection_json,
               created_at,
               updated_at
             )
             VALUES (?, ?, ?, ?, ?, ?, 'open', ?, ?, ?)`,
          )
          .bind(
            input.id,
            input.idempotencyKey,
            input.reporterActorRef,
            input.targetKind,
            input.targetId,
            input.reasonRef,
            JSON.stringify(input.publicProjection),
            runtime.nowIso(),
            runtime.nowIso(),
          )
          .run(),
      ),
    ),
    Effect.as(input.id),
  )

export const readForumReportByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<ForumReportRow | null, ForumStorageError> =>
  d1Effect('forum.readReportByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_reports
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<ForumReportRow>(),
  )

export const readForumPostRevisionByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<ForumPostRevisionRow | null, ForumStorageError> =>
  d1Effect('forum.readPostRevisionByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_post_revisions
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<ForumPostRevisionRow>(),
  )

const recordForumPostRevision = (
  db: D1Database,
  input: ForumPostRevisionInput,
  runtime: ForumRepositoryRuntime,
): Effect.Effect<string, ForumStorageError | ForumPublicProjectionUnsafe> =>
  validateProjection(input.publicProjection).pipe(
    Effect.flatMap(() =>
      d1Effect('forum.recordPostRevision', () =>
        db
          .prepare(
            `INSERT INTO forum_post_revisions (
               id,
               idempotency_key,
               post_id,
               actor_ref,
               action_kind,
               previous_body_text,
               next_body_text,
               previous_state,
               next_state,
               reason_ref,
               public_projection_json,
               created_at
             )
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            input.id,
            input.idempotencyKey,
            input.postId,
            input.actorRef,
            input.actionKind,
            input.previousBodyText,
            input.nextBodyText,
            input.previousState,
            input.nextState,
            input.reasonRef,
            JSON.stringify(input.publicProjection),
            runtime.nowIso(),
          )
          .run(),
      ),
    ),
    Effect.as(input.id),
  )

export const editForumPostBody = (
  db: D1Database,
  input: Omit<
    ForumPostRevisionInput,
    'actionKind' | 'nextState' | 'previousBodyText' | 'previousState'
  > &
    Readonly<{ nextParentPostId?: string | null | undefined }>,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<
  ForumPostSummary,
  ForumRepositoryError | ForumPublicProjectionUnsafe
> =>
  Effect.gen(function* () {
    const existing = yield* readForumPostById(db, input.postId)

    if (
      existing === null ||
      existing.state === 'hidden' ||
      existing.state === 'held_for_review' ||
      existing.state === 'tombstoned'
    ) {
      return yield* new ForumValidationError({
        reason: 'Forum post cannot be edited.',
      })
    }

    const { nextParentPostId, ...revisionBase } = input
    const revisionInput: ForumPostRevisionInput = {
      ...revisionBase,
      actionKind: 'edit',
      previousBodyText: existing.bodyText ?? null,
      previousState: existing.state,
      nextState: 'edited',
    }
    const now = runtime.nowIso()
    yield* recordForumPostRevision(db, revisionInput, {
      ...runtime,
      nowIso: () => now,
    })

    yield* d1Effect('forum.editPostBody', () =>
      db
        .prepare(
          `UPDATE forum_post_bodies
              SET body_text = ?,
                  updated_at = ?,
                  archived_at = NULL
            WHERE post_id = ?`,
        )
        .bind(input.nextBodyText, now, input.postId)
        .run(),
    )

    yield* d1Effect('forum.markPostEdited', () =>
      db
        .prepare(
          `UPDATE forum_posts
              SET state = 'edited',
                  revision_ref = ?,
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(input.id, now, input.postId)
        .run(),
    )

    if (nextParentPostId !== undefined) {
      yield* d1Effect('forum.reparentPost', () =>
        db
          .prepare(
            `UPDATE forum_posts
                SET parent_post_id = ?,
                    updated_at = ?
              WHERE id = ?
                AND archived_at IS NULL`,
          )
          .bind(nextParentPostId, now, input.postId)
          .run(),
      )
    }

    const updated = yield* readForumPostById(db, input.postId)

    if (updated === null) {
      return yield* new ForumValidationError({
        reason: 'Forum post edit did not persist.',
      })
    }

    return updated
  })

export const tombstoneForumPost = (
  db: D1Database,
  input: Omit<
    ForumPostRevisionInput,
    | 'actionKind'
    | 'nextBodyText'
    | 'nextState'
    | 'previousBodyText'
    | 'previousState'
  >,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<
  ForumPostSummary,
  ForumRepositoryError | ForumPublicProjectionUnsafe
> =>
  Effect.gen(function* () {
    const existing = yield* readForumPostById(db, input.postId)

    if (
      existing === null ||
      existing.state === 'hidden' ||
      existing.state === 'held_for_review' ||
      existing.state === 'tombstoned'
    ) {
      return yield* new ForumValidationError({
        reason: 'Forum post cannot be tombstoned.',
      })
    }

    const revisionInput: ForumPostRevisionInput = {
      ...input,
      actionKind: 'tombstone',
      nextBodyText: null,
      nextState: 'tombstoned',
      previousBodyText: existing.bodyText ?? null,
      previousState: existing.state,
    }
    const now = runtime.nowIso()
    yield* recordForumPostRevision(db, revisionInput, {
      ...runtime,
      nowIso: () => now,
    })

    yield* d1Effect('forum.archivePostBodyAfterTombstone', () =>
      db
        .prepare(
          `UPDATE forum_post_bodies
              SET updated_at = ?,
                  archived_at = ?
            WHERE post_id = ?
              AND archived_at IS NULL`,
        )
        .bind(now, now, input.postId)
        .run(),
    )

    yield* d1Effect('forum.markPostTombstoned', () =>
      db
        .prepare(
          `UPDATE forum_posts
              SET state = 'tombstoned',
                  revision_ref = ?,
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(input.id, now, input.postId)
        .run(),
    )

    // A tombstoned post no longer counts toward the public thread. Decrement
    // the topic and forum post counts so topic-list replyCount (derived as
    // post_count - 1) and forum totals stay honest. Clamped at zero so a count
    // can never go negative. The topic-detail read derives its own live count
    // independently, which also covers older tombstoned rows that predate this
    // decrement.
    const containingTopic = yield* readForumTopicById(db, existing.topicId)
    yield* d1Effect('forum.decrementTopicPostCountAfterTombstone', () =>
      db
        .prepare(
          `UPDATE forum_topics
              SET post_count = MAX(0, post_count - 1),
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(now, existing.topicId)
        .run(),
    )
    if (containingTopic !== null) {
      yield* d1Effect('forum.decrementForumPostCountAfterTombstone', () =>
        db
          .prepare(
            `UPDATE forum_forums
                SET post_count = MAX(0, post_count - 1)
              WHERE id = ?
                AND archived_at IS NULL`,
          )
          .bind(containingTopic.forumId)
          .run(),
      )
    }

    const updated = yield* readForumPostById(db, input.postId)

    if (updated === null) {
      return yield* new ForumValidationError({
        reason: 'Forum post tombstone did not persist.',
      })
    }

    return updated
  })

export const recordForumModerationEvent = (
  db: D1Database,
  input: ForumModerationEventInput,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<string, ForumStorageError | ForumPublicProjectionUnsafe> =>
  validateProjection(input.publicProjection).pipe(
    Effect.flatMap(() =>
      d1Effect('forum.recordModerationEvent', () =>
        db
          .prepare(
            `INSERT INTO forum_moderation_events (
               id,
               idempotency_key,
               moderator_actor_ref,
               action_kind,
               target_kind,
               target_id,
               reason_ref,
               report_id,
               public_projection_json,
               created_at
             )
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            input.id,
            input.idempotencyKey ?? null,
            input.moderatorActorRef,
            input.actionKind,
            input.targetKind,
            input.targetId,
            input.reasonRef,
            input.reportId,
            JSON.stringify(input.publicProjection),
            runtime.nowIso(),
          )
          .run(),
      ),
    ),
    Effect.as(input.id),
  )

export const readForumModerationEventByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<ForumModerationEventRow | null, ForumStorageError> =>
  d1Effect('forum.readModerationEventByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_moderation_events
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<ForumModerationEventRow>(),
  )

export const readForumReportById = (
  db: D1Database,
  reportId: string,
): Effect.Effect<ForumReportRow | null, ForumStorageError> =>
  d1Effect('forum.readReportById', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_reports
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(reportId)
      .first<ForumReportRow>(),
  )

const moderationQueueItemFromReport = (
  report: ForumReportRow,
  target: ForumTopicSummary | ForumPostSummary | null,
): ForumModerationQueueItem => ({
  bodyText:
    target !== null && 'bodyText' in target ? (target.bodyText ?? null) : null,
  createdAt: report.created_at,
  itemId: `report:${report.id}`,
  itemKind: 'report',
  postNumber:
    target !== null && 'postNumber' in target ? target.postNumber : null,
  reasonRef: report.reason_ref,
  reportId: report.id,
  reportStatus: report.status,
  reporterActorRef: report.reporter_actor_ref,
  targetId: report.target_id,
  targetKind: report.target_kind === 'topic' ? 'topic' : 'post',
  targetState: target === null ? null : target.state,
  title: target !== null && 'title' in target ? target.title : null,
  topicId:
    target !== null && 'topicId' in target
      ? target.topicId
      : report.target_kind === 'topic'
        ? report.target_id
        : null,
  updatedAt: report.updated_at,
})

const moderationQueueItemFromPost = (
  post: ForumPostSummary,
): ForumModerationQueueItem => ({
  bodyText: post.bodyText ?? null,
  createdAt: post.createdAt,
  itemId: `post_review:${post.postId}`,
  itemKind: 'post_review',
  postNumber: post.postNumber,
  reasonRef: null,
  reportId: null,
  reportStatus: null,
  reporterActorRef: null,
  targetId: post.postId,
  targetKind: 'post',
  targetState: post.state,
  title: null,
  topicId: post.topicId,
  updatedAt: post.updatedAt,
})

const moderationQueueItemFromTopic = (
  topic: ForumTopicSummary,
): ForumModerationQueueItem => ({
  bodyText: null,
  createdAt: topic.createdAt,
  itemId: `topic_review:${topic.topicId}`,
  itemKind: 'topic_review',
  postNumber: null,
  reasonRef: null,
  reportId: null,
  reportStatus: null,
  reporterActorRef: null,
  targetId: topic.topicId,
  targetKind: 'topic',
  targetState: topic.state,
  title: topic.title,
  topicId: topic.topicId,
  updatedAt: topic.updatedAt,
})

export const listForumModerationQueue = (
  db: D1Database,
  input: Readonly<{ limit?: number }> = {},
): Effect.Effect<ReadonlyArray<ForumModerationQueueItem>, ForumStorageError> =>
  Effect.gen(function* () {
    const limit = input.limit ?? 50
    const reports = yield* d1Effect('forum.listModerationQueue.reports', () =>
      db
        .prepare(
          `SELECT *
             FROM forum_reports
            WHERE status IN ('open', 'reviewing')
              AND archived_at IS NULL
            ORDER BY created_at ASC
            LIMIT ?`,
        )
        .bind(limit)
        .all<ForumReportRow>(),
    )
    const reportItems: Array<ForumModerationQueueItem> = []

    for (const report of reports.results ?? []) {
      const target =
        report.target_kind === 'post'
          ? yield* readForumPostById(db, report.target_id)
          : report.target_kind === 'topic'
            ? yield* readForumTopicById(db, report.target_id)
            : null

      if (report.target_kind === 'post' || report.target_kind === 'topic') {
        reportItems.push(moderationQueueItemFromReport(report, target))
      }
    }

    const posts = yield* d1Effect('forum.listModerationQueue.posts', () =>
      db
        .prepare(
          `SELECT forum_posts.*, forum_post_bodies.body_text AS body_text
             FROM forum_posts
             LEFT JOIN forum_post_bodies
               ON forum_post_bodies.post_id = forum_posts.id
              AND forum_post_bodies.archived_at IS NULL
            WHERE forum_posts.state IN ('held_for_review', 'hidden')
              AND forum_posts.archived_at IS NULL
            ORDER BY forum_posts.updated_at ASC
            LIMIT ?`,
        )
        .bind(limit)
        .all<PostRow>(),
    )
    const topics = yield* d1Effect('forum.listModerationQueue.topics', () =>
      db
        .prepare(
          `SELECT *
             FROM forum_topics
            WHERE state = 'hidden'
              AND archived_at IS NULL
            ORDER BY updated_at ASC
            LIMIT ?`,
        )
        .bind(limit)
        .all<TopicRow>(),
    )

    return [
      ...reportItems,
      ...(posts.results ?? [])
        .map(postFromRow)
        .map(moderationQueueItemFromPost),
      ...(topics.results ?? [])
        .map(topicFromRow)
        .map(moderationQueueItemFromTopic),
    ].slice(0, limit)
  })

export const readForumModerationItem = (
  db: D1Database,
  input: Readonly<{
    itemId: string
    itemKind: 'report' | 'post_review' | 'topic_review'
  }>,
): Effect.Effect<ForumModerationItemDetail | null, ForumStorageError> =>
  Effect.gen(function* () {
    if (input.itemKind === 'report') {
      const report = yield* readForumReportById(db, input.itemId)

      if (report === null) {
        return null
      }

      const target =
        report.target_kind === 'post'
          ? yield* readForumPostById(db, report.target_id)
          : report.target_kind === 'topic'
            ? yield* readForumTopicById(db, report.target_id)
            : null

      if (report.target_kind !== 'post' && report.target_kind !== 'topic') {
        return null
      }

      const item = moderationQueueItemFromReport(report, target)

      return {
        bodyText: item.bodyText,
        item,
        post: target !== null && 'postNumber' in target ? target : null,
        report,
        topic: target !== null && 'title' in target ? target : null,
      }
    }

    if (input.itemKind === 'post_review') {
      const post = yield* readForumPostById(db, input.itemId)

      return post === null
        ? null
        : {
            bodyText: post.bodyText ?? null,
            item: moderationQueueItemFromPost(post),
            post,
            report: null,
            topic: null,
          }
    }

    const topic = yield* readForumTopicById(db, input.itemId)

    return topic === null
      ? null
      : {
          bodyText: null,
          item: moderationQueueItemFromTopic(topic),
          post: null,
          report: null,
          topic,
        }
  })

export const updateForumReportStatus = (
  db: D1Database,
  input: Readonly<{
    reportId: string
    status: 'open' | 'reviewing' | 'resolved' | 'dismissed'
  }>,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<ForumReportRow | null, ForumStorageError> =>
  d1Effect('forum.updateReportStatus', () =>
    db
      .prepare(
        `UPDATE forum_reports
            SET status = ?,
                updated_at = ?
          WHERE id = ?
            AND archived_at IS NULL`,
      )
      .bind(input.status, runtime.nowIso(), input.reportId)
      .run(),
  ).pipe(Effect.flatMap(() => readForumReportById(db, input.reportId)))

export const updateForumPostModerationState = (
  db: D1Database,
  input: Readonly<{
    postId: string
    state: 'visible' | 'edited' | 'tombstoned' | 'held_for_review' | 'hidden'
  }>,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<ForumPostSummary | null, ForumStorageError> =>
  d1Effect('forum.updatePostModerationState', () =>
    db
      .prepare(
        `UPDATE forum_posts
            SET state = ?,
                updated_at = ?
          WHERE id = ?
            AND archived_at IS NULL`,
      )
      .bind(input.state, runtime.nowIso(), input.postId)
      .run(),
  ).pipe(Effect.flatMap(() => readForumPostById(db, input.postId)))

export const updateForumTopicModerationState = (
  db: D1Database,
  input: Readonly<{
    state: 'open' | 'locked' | 'archived' | 'hidden'
    topicId: string
  }>,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<ForumTopicSummary | null, ForumStorageError> =>
  d1Effect('forum.updateTopicModerationState', () =>
    db
      .prepare(
        `UPDATE forum_topics
            SET state = ?,
                updated_at = ?
          WHERE id = ?
            AND archived_at IS NULL`,
      )
      .bind(input.state, runtime.nowIso(), input.topicId)
      .run(),
  ).pipe(Effect.flatMap(() => readForumTopicById(db, input.topicId)))

export const updateForumTopicPinState = (
  db: D1Database,
  input: Readonly<{
    pinState: 'normal' | 'sticky' | 'announcement'
    topicId: string
  }>,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<ForumTopicSummary | null, ForumStorageError> =>
  d1Effect('forum.updateTopicPinState', () =>
    db
      .prepare(
        `UPDATE forum_topics
            SET pin_state = ?,
                updated_at = ?
          WHERE id = ?
            AND archived_at IS NULL`,
      )
      .bind(input.pinState, runtime.nowIso(), input.topicId)
      .run(),
  ).pipe(Effect.flatMap(() => readForumTopicById(db, input.topicId)))

export const updateForumTopicTitle = (
  db: D1Database,
  input: Readonly<{
    title: string
    topicId: string
  }>,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<ForumTopicSummary | null, ForumStorageError> =>
  d1Effect('forum.updateTopicTitle', () =>
    db
      .prepare(
        `UPDATE forum_topics
            SET title = ?,
                updated_at = ?
          WHERE id = ?
            AND archived_at IS NULL`,
      )
      .bind(input.title, runtime.nowIso(), input.topicId)
      .run(),
  ).pipe(Effect.flatMap(() => readForumTopicById(db, input.topicId)))

export const createForumPrivateMessageThread = (
  db: D1Database,
  input: ForumPrivateMessageThreadInput,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<string, ForumStorageError> =>
  d1Effect('forum.createPrivateMessageThread', () =>
    db
      .prepare(
        `INSERT INTO forum_private_message_threads (
           id,
           subject,
           slug,
           created_by_actor_ref,
           participant_refs_json,
           latest_message_id,
           message_count,
           created_at,
           updated_at
         )
         VALUES (?, ?, ?, ?, ?, NULL, 0, ?, ?)`,
      )
      .bind(
        input.id,
        input.subject,
        input.slug,
        input.createdByActorRef,
        JSON.stringify(input.participantRefs),
        runtime.nowIso(),
        runtime.nowIso(),
      )
      .run(),
  ).pipe(Effect.as(input.id))

export const addForumPrivateMessage = (
  db: D1Database,
  input: ForumPrivateMessageInput,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<string, ForumStorageError | ForumPublicProjectionUnsafe> =>
  validateProjection(input.publicProjection).pipe(
    Effect.flatMap(() =>
      d1Effect('forum.addPrivateMessage', () =>
        db
          .prepare(
            `INSERT INTO forum_private_messages (
               id,
               thread_id,
               sender_actor_ref,
               recipient_actor_ref,
               content_ref,
               public_projection_json,
               created_at
             )
             VALUES (?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            input.id,
            input.threadId,
            input.senderActorRef,
            input.recipientActorRef,
            input.contentRef,
            JSON.stringify(input.publicProjection),
            runtime.nowIso(),
          )
          .run(),
      ),
    ),
    Effect.flatMap(() =>
      d1Effect('forum.bumpPrivateMessageThread', () =>
        db
          .prepare(
            `UPDATE forum_private_message_threads
                SET latest_message_id = ?,
                    message_count = message_count + 1,
                    updated_at = ?
              WHERE id = ?
                AND archived_at IS NULL`,
          )
          .bind(input.id, runtime.nowIso(), input.threadId)
          .run(),
      ),
    ),
    Effect.as(input.id),
  )

export const listForumPrivateMessagesForActor = (
  db: D1Database,
  actorRef: string,
): Effect.Effect<ReadonlyArray<PrivateMessageRow>, ForumStorageError> =>
  d1Effect('forum.listPrivateMessagesForActor', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_private_messages
          WHERE archived_at IS NULL
            AND (sender_actor_ref = ? OR recipient_actor_ref = ?)
          ORDER BY created_at ASC`,
      )
      .bind(actorRef, actorRef)
      .all<PrivateMessageRow>(),
  ).pipe(Effect.map(result => result.results ?? []))

export const recordForumReceipt = (
  db: D1Database,
  input: ForumReceiptRecordInput,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<string, ForumRepositoryError | ForumPublicProjectionUnsafe> =>
  Effect.gen(function* () {
    yield* validateProjection(input.publicProjection)
    yield* validateSafePaymentRef(input.redactedPaymentRef)

    yield* d1Effect('forum.recordReceipt', () =>
      db
        .prepare(
          `INSERT INTO forum_receipts (
             id,
             receipt_ref,
             action_kind,
             target_forum_id,
             target_topic_id,
             target_post_id,
             amount_asset,
             amount_value,
             recipient_actor_ref,
             redacted_payment_ref,
             public_projection_json,
             created_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          input.id,
          input.receiptRef,
          input.actionKind,
          input.targetForumId,
          input.targetTopicId,
          input.targetPostId,
          input.amountAsset,
          input.amountValue,
          input.recipientActorRef,
          input.redactedPaymentRef,
          JSON.stringify(input.publicProjection),
          runtime.nowIso(),
        )
        .run(),
    )

    return input.receiptRef
  })

const notificationReadWriteResponse = (
  row: ForumNotificationReadRow,
  idempotent: boolean,
): ForumAgentNotificationReadWriteResponse =>
  decodeForumAgentNotificationReadWriteResponse({
    actorRef: row.actor_ref,
    id: row.id,
    idempotencyKey: row.idempotency_key,
    idempotent,
    notificationId: row.notification_id,
    readAt: row.read_at,
  })

export const readForumNotificationReadByIdempotencyKey = (
  db: D1Database,
  input: Readonly<{
    actorRef: string
    idempotencyKey: string
  }>,
): Effect.Effect<ForumNotificationReadRow | null, ForumStorageError> =>
  d1Effect('forum.notificationReads.byIdempotency', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_notification_reads
          WHERE actor_ref = ?
            AND idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(input.actorRef, input.idempotencyKey)
      .first<ForumNotificationReadRow>(),
  )

export const readForumNotificationReadByNotificationId = (
  db: D1Database,
  input: Readonly<{
    actorRef: string
    notificationId: string
  }>,
): Effect.Effect<ForumNotificationReadRow | null, ForumStorageError> =>
  d1Effect('forum.notificationReads.byNotification', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_notification_reads
          WHERE actor_ref = ?
            AND notification_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(input.actorRef, input.notificationId)
      .first<ForumNotificationReadRow>(),
  )

export const listForumNotificationReadsForActor = (
  db: D1Database,
  actorRef: string,
): Effect.Effect<ReadonlyArray<ForumNotificationReadRow>, ForumStorageError> =>
  d1Effect('forum.notificationReads.forActor', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_notification_reads
          WHERE actor_ref = ?
            AND archived_at IS NULL
          ORDER BY read_at DESC
          LIMIT 1000`,
      )
      .bind(actorRef)
      .all<ForumNotificationReadRow>(),
  ).pipe(Effect.map(result => result.results ?? []))

export const recordForumNotificationRead = (
  db: D1Database,
  input: ForumNotificationReadInput,
): Effect.Effect<
  ForumAgentNotificationReadWriteResponse,
  ForumStorageError | ForumValidationError
> =>
  Effect.gen(function* () {
    yield* d1Effect('forum.notificationReads.insert', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO forum_notification_reads (
             id,
             actor_ref,
             notification_id,
             idempotency_key,
             read_at,
             created_at,
             updated_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          input.id,
          input.actorRef,
          input.notificationId,
          input.idempotencyKey,
          input.readAt,
          input.readAt,
          input.readAt,
        )
        .run(),
    )

    const row = yield* readForumNotificationReadByNotificationId(db, {
      actorRef: input.actorRef,
      notificationId: input.notificationId,
    })

    if (row === null) {
      return yield* new ForumValidationError({
        reason: 'Forum notification read state was not persisted.',
      })
    }

    return notificationReadWriteResponse(row, false)
  })

const postNotification = (
  kind: 'followed_actor_post' | 'mention' | 'watched_topic_reply',
  row: ForumNotificationPostRow,
): ForumAgentNotification =>
  decodeForumAgentNotification({
    bodyText: row.state === 'tombstoned' ? null : row.body_text,
    createdAt: row.created_at,
    id: `${kind}:${row.id}`,
    kind,
    publicUrl: forumPostPublicUrl(row.topic_id, row.id),
    readAt: null,
    readState: 'unread',
    sourceActor: actorFromJson(row.actor_json),
    target: {
      actorRef: actorFromJson(row.actor_json).actorRef,
      forumId: row.forum_id,
      postId: row.id,
      topicId: row.topic_id,
    },
    title: row.topic_title,
  })

const topicNotification = (
  row: ForumNotificationTopicRow,
): ForumAgentNotification =>
  decodeForumAgentNotification({
    createdAt: row.created_at,
    id: `watched_forum_topic:${row.id}`,
    kind: 'watched_forum_topic',
    publicUrl: forumTopicPublicUrl(row.id),
    readAt: null,
    readState: 'unread',
    sourceActor: actorFromJson(row.actor_json),
    target: {
      actorRef: actorFromJson(row.actor_json).actorRef,
      forumId: row.forum_id,
      postId: row.first_post_id,
      topicId: row.id,
    },
    title: row.title,
  })

const receiptNotification = (
  row: ForumNotificationReceiptRow,
): ForumAgentNotification =>
  decodeForumAgentNotification({
    createdAt: row.created_at,
    id: `receipt:${row.receipt_ref}`,
    kind: 'receipt',
    publicUrl: `https://openagents.com/forum/receipts/${encodeURIComponent(row.receipt_ref)}`,
    readAt: null,
    readState: 'unread',
    target: {
      actorRef: row.recipient_actor_ref,
      forumId: row.target_forum_id,
      postId: row.target_post_id,
      topicId: row.target_topic_id,
    },
    title: `${row.action_kind.replaceAll('_', ' ')} receipt`,
  })

const uniqueNotifications = (
  notifications: ReadonlyArray<ForumAgentNotification>,
): ReadonlyArray<ForumAgentNotification> => {
  const seen = new Set<string>()

  return notifications.filter(notification => {
    if (seen.has(notification.id)) {
      return false
    }

    seen.add(notification.id)
    return true
  })
}

const notificationSummary = (
  notifications: ReadonlyArray<ForumAgentNotification>,
): ForumAgentNotificationSummary => {
  const countKind = (kind: ForumAgentNotification['kind']): number =>
    notifications.filter(notification => notification.kind === kind).length
  const unreadCount = notifications.filter(
    notification => notification.readState === 'unread',
  ).length

  return {
    followedActorPostCount: countKind('followed_actor_post'),
    mentionCount: countKind('mention'),
    nextAction:
      unreadCount === 0
        ? 'No unread Forum notifications. Inspect watched topics before starting new posts.'
        : 'Review unread Forum mentions, watched-topic replies, followed-actor posts, and receipts before starting new posts.',
    receiptCount: countKind('receipt'),
    totalCount: notifications.length,
    unreadCount,
    watchedForumTopicCount: countKind('watched_forum_topic'),
    watchedTopicReplyCount: countKind('watched_topic_reply'),
  }
}

const withReadState = (
  notifications: ReadonlyArray<ForumAgentNotification>,
  reads: ReadonlyArray<ForumNotificationReadRow>,
): ReadonlyArray<ForumAgentNotification> => {
  const readByNotificationId = new Map(
    reads.map(read => [read.notification_id, read.read_at]),
  )

  return notifications.map(notification => {
    const readAt = readByNotificationId.get(notification.id)

    return readAt === undefined
      ? notification
      : {
          ...notification,
          readAt,
          readState: 'read' as const,
        }
  })
}

export const readForumAgentNotifications = (
  db: D1Database,
  input: Readonly<{
    actorRef: string
    actorSlug: string
    generatedAt: string
    limit?: number
  }>,
): Effect.Effect<ForumAgentNotificationsResponse, ForumStorageError> =>
  Effect.gen(function* () {
    const limit = Math.min(Math.max(input.limit ?? 50, 1), 100)
    const mentionPattern = `%@${input.actorSlug}%`
    const [
      watchedTopicPosts,
      watchedForumTopics,
      followedActorPosts,
      mentions,
      receipts,
      reads,
    ] = yield* Effect.all([
      d1Effect('forum.notifications.watchedTopicPosts', () =>
        db
          .prepare(
            `SELECT forum_posts.*,
                      forum_post_bodies.body_text AS body_text,
                      forum_topics.title AS topic_title,
                      forum_topics.slug AS topic_slug
                 FROM forum_watches
                 JOIN forum_posts
                   ON forum_posts.topic_id = forum_watches.topic_id
                  AND forum_posts.archived_at IS NULL
                  AND forum_posts.state IN ('visible', 'edited')
                 JOIN forum_post_bodies
                   ON forum_post_bodies.post_id = forum_posts.id
                  AND forum_post_bodies.archived_at IS NULL
                 JOIN forum_topics
                   ON forum_topics.id = forum_posts.topic_id
                  AND forum_topics.archived_at IS NULL
                  AND forum_topics.state IN ('open', 'locked')
                 JOIN forum_forums
                   ON forum_forums.id = forum_posts.forum_id
                  AND forum_forums.archived_at IS NULL
                  AND forum_forums.visibility = 'public'
                  AND forum_forums.discoverability IN ('listed', 'unlisted')
                WHERE forum_watches.actor_ref = ?
                  AND forum_watches.watch_kind = 'topic'
                  AND forum_watches.archived_at IS NULL
                  AND forum_posts.actor_ref <> ?
                ORDER BY forum_posts.created_at DESC
                LIMIT ?`,
          )
          .bind(input.actorRef, input.actorRef, limit)
          .all<ForumNotificationPostRow>(),
      ).pipe(Effect.map(result => result.results ?? [])),
      d1Effect('forum.notifications.watchedForumTopics', () =>
        db
          .prepare(
            `SELECT forum_topics.*,
                      forum_forums.slug AS forum_slug
                 FROM forum_watches
                 JOIN forum_topics
                   ON forum_topics.forum_id = forum_watches.forum_id
                  AND forum_topics.archived_at IS NULL
                  AND forum_topics.state IN ('open', 'locked')
                 JOIN forum_forums
                   ON forum_forums.id = forum_topics.forum_id
                  AND forum_forums.archived_at IS NULL
                  AND forum_forums.visibility = 'public'
                  AND forum_forums.discoverability IN ('listed', 'unlisted')
                WHERE forum_watches.actor_ref = ?
                  AND forum_watches.watch_kind = 'forum'
                  AND forum_watches.archived_at IS NULL
                  AND forum_topics.actor_ref <> ?
                ORDER BY forum_topics.created_at DESC
                LIMIT ?`,
          )
          .bind(input.actorRef, input.actorRef, limit)
          .all<ForumNotificationTopicRow>(),
      ).pipe(Effect.map(result => result.results ?? [])),
      d1Effect('forum.notifications.followedActorPosts', () =>
        db
          .prepare(
            `SELECT forum_posts.*,
                      forum_post_bodies.body_text AS body_text,
                      forum_topics.title AS topic_title,
                      forum_topics.slug AS topic_slug
                 FROM forum_actor_follows
                 JOIN forum_posts
                   ON forum_posts.actor_ref = forum_actor_follows.target_actor_ref
                  AND forum_posts.archived_at IS NULL
                  AND forum_posts.state IN ('visible', 'edited')
                 JOIN forum_post_bodies
                   ON forum_post_bodies.post_id = forum_posts.id
                  AND forum_post_bodies.archived_at IS NULL
                 JOIN forum_topics
                   ON forum_topics.id = forum_posts.topic_id
                  AND forum_topics.archived_at IS NULL
                  AND forum_topics.state IN ('open', 'locked')
                 JOIN forum_forums
                   ON forum_forums.id = forum_posts.forum_id
                  AND forum_forums.archived_at IS NULL
                  AND forum_forums.visibility = 'public'
                  AND forum_forums.discoverability IN ('listed', 'unlisted')
                WHERE forum_actor_follows.actor_ref = ?
                  AND forum_actor_follows.archived_at IS NULL
                ORDER BY forum_posts.created_at DESC
                LIMIT ?`,
          )
          .bind(input.actorRef, limit)
          .all<ForumNotificationPostRow>(),
      ).pipe(Effect.map(result => result.results ?? [])),
      d1Effect('forum.notifications.mentions', () =>
        db
          .prepare(
            `SELECT forum_posts.*,
                      forum_post_bodies.body_text AS body_text,
                      forum_topics.title AS topic_title,
                      forum_topics.slug AS topic_slug
                 FROM forum_posts
                 JOIN forum_post_bodies
                   ON forum_post_bodies.post_id = forum_posts.id
                  AND forum_post_bodies.archived_at IS NULL
                 JOIN forum_topics
                   ON forum_topics.id = forum_posts.topic_id
                  AND forum_topics.archived_at IS NULL
                  AND forum_topics.state IN ('open', 'locked')
                 JOIN forum_forums
                   ON forum_forums.id = forum_posts.forum_id
                  AND forum_forums.archived_at IS NULL
                  AND forum_forums.visibility = 'public'
                  AND forum_forums.discoverability IN ('listed', 'unlisted')
                WHERE forum_posts.archived_at IS NULL
                  AND forum_posts.state IN ('visible', 'edited')
                  AND forum_posts.actor_ref <> ?
                  AND forum_post_bodies.body_text LIKE ?
                ORDER BY forum_posts.created_at DESC
                LIMIT ?`,
          )
          .bind(input.actorRef, mentionPattern, limit)
          .all<ForumNotificationPostRow>(),
      ).pipe(Effect.map(result => result.results ?? [])),
      d1Effect('forum.notifications.receipts', () =>
        db
          .prepare(
            `SELECT id,
                      receipt_ref,
                      action_kind,
                      target_forum_id,
                      target_topic_id,
                      target_post_id,
                      amount_asset,
                      amount_value,
                      recipient_actor_ref,
                      created_at
                 FROM forum_receipts
                WHERE recipient_actor_ref = ?
                  AND archived_at IS NULL
                ORDER BY created_at DESC
                LIMIT ?`,
          )
          .bind(input.actorRef, limit)
          .all<ForumNotificationReceiptRow>(),
      ).pipe(Effect.map(result => result.results ?? [])),
      listForumNotificationReadsForActor(db, input.actorRef),
    ])
    const allForumNotifications = Array.from(
      withReadState(
        uniqueNotifications([
          ...watchedTopicPosts.map(row =>
            postNotification('watched_topic_reply', row),
          ),
          ...watchedForumTopics.map(topicNotification),
          ...followedActorPosts.map(row =>
            postNotification('followed_actor_post', row),
          ),
          ...mentions.map(row => postNotification('mention', row)),
          ...receipts.map(receiptNotification),
        ]),
        reads,
      ),
    ).sort((left: ForumAgentNotification, right: ForumAgentNotification) =>
      right.createdAt.localeCompare(left.createdAt),
    )
    const forumNotifications = allForumNotifications.slice(0, limit)

    const response = {
      actorRef: input.actorRef,
      generatedAt: input.generatedAt,
      notifications: forumNotifications,
      pagination: defaultPagination(limit),
      summary: notificationSummary(allForumNotifications),
    }
    const decoded = decodeForumAgentNotificationsResponse({
      ...response,
      notifications: [...response.notifications],
    })
    return { ...decoded, notifications: forumNotifications }
  })

export const readForumSummaryById = (
  db: D1Database,
  forumId: string,
): Effect.Effect<ReturnType<typeof forumFromRow> | null, ForumStorageError> =>
  d1Effect('forum.readForumById', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_forums
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(forumId)
      .first<ForumRow>(),
  ).pipe(Effect.map(row => (row === null ? null : forumFromRow(row))))
