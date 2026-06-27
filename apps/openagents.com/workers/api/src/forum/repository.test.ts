import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type ForumPublicProjection,
  ForumPublicProjection as ForumPublicProjectionSchema,
  ForumPublicProjectionUnsafe,
  ForumReadAccessDenied,
  type ForumRepositoryRuntime,
  type ForumStoredActorSummary,
  ForumValidationError,
  addForumPrivateMessage,
  bookmarkForumTarget,
  createForumPrivateMessageThread,
  createForumReplyPost,
  createForumTopicWithFirstPost,
  listForumPrivateMessagesForActor,
  readForumBoardIndex,
  readForumContextActivity,
  readForumPostDetail,
  readForumSummaryById,
  readForumSummaryByRef,
  readForumTipRecipientReadinessForActor,
  readForumTopicById,
  readForumTopicDetail,
  readForumTopicList,
  recordForumModerationEvent,
  recordForumReceipt,
  recordForumReport,
  searchForumPublicContent,
  upsertForumTipRecipientWallet,
  watchForumTarget,
} from './index'

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
  actor_ref: string
  archived_at: string | null
  created_at: string
  first_post_id: string
  forum_id: string
  id: string
  idempotency_key: string
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
  actor_ref: string
  archived_at: string | null
  body_text: string | null
  content_ref: string
  created_at: string
  forum_id: string
  id: string
  idempotency_key: string
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

type WatchRow = Readonly<{
  actor_ref: string
  forum_id: string | null
  id: string
  idempotency_key: string
  topic_id: string | null
  watch_kind: 'forum' | 'topic'
}>

type BookmarkRow = Readonly<{
  actor_ref: string
  bookmark_kind: 'topic' | 'post'
  id: string
  idempotency_key: string
  post_id: string | null
  topic_id: string | null
}>

type ReportRow = Readonly<{
  id: string
  reason_ref: string
  reporter_actor_ref: string
  status: 'open' | 'reviewing' | 'resolved' | 'dismissed'
  target_id: string
  target_kind: 'forum' | 'topic' | 'post' | 'user'
}>

type ModerationEventRow = Readonly<{
  action_kind: string
  id: string
  idempotency_key: string | null
  moderator_actor_ref: string
  public_projection_json: string
  reason_ref: string
  report_id: string | null
  target_id: string
  target_kind: 'forum' | 'topic' | 'post' | 'report' | 'user'
}>

type PrivateMessageThreadRow = Readonly<{
  id: string
  latest_message_id: string | null
  message_count: number
  participant_refs_json: string
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

type ReceiptRow = Readonly<{
  action_kind: string
  amount_asset: 'credits' | 'sats' | 'usd'
  amount_value: number
  id: string
  receipt_ref: string
  redacted_payment_ref: string
}>

type TipRecipientWalletRow = Readonly<{
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
  provider_class: 'external_lightning' | 'hosted_mdk' | 'mdk_agent_wallet'
  public_projection_json: string
  readiness_refs_json: string
  receive_capability_ref: string
  source_ref: string
  state: 'ready' | 'disabled' | 'blocked'
  updated_at: string
  wallet_ref: string
}>

type ContextLinkRow = Readonly<{
  archived_at: string | null
  context_id: string
  context_kind: 'site' | 'workroom'
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
  target_kind: 'topic' | 'post'
  topic_id: string | null
}>

const publicProjectionFixture = {
  classificationCaveatRef: 'classification.public_forum_projection',
  customerSafe: true,
  dataClassification: 'public',
  excludedPrivateRefs: ['payment_private.invoice_redacted'],
  publicSafe: true,
  redactionPolicyRef: 'redaction.forum.public.v1',
  safeArtifactRefs: ['artifact.public_forum_post'],
  safeReceiptRefs: ['receipt.forum.reward.public_1'],
  trustTier: 'reviewed',
}

const publicProjectionJson = JSON.stringify(publicProjectionFixture)
const privateProjectionJson = JSON.stringify({
  ...publicProjectionFixture,
  dataClassification: 'private',
  publicSafe: false,
})

class ForumRepositoryStore {
  boards: Array<BoardRow> = [
    {
      archived_at: null,
      description_ref: 'content.forum.board.openagents.description',
      id: '11111111-1111-4111-8111-111111111111',
      public_projection_json: publicProjectionJson,
      slug: 'openagents',
      title: 'OpenAgents',
      visibility: 'public',
    },
  ]
  categories: Array<CategoryRow> = [
    {
      archived_at: null,
      board_id: '11111111-1111-4111-8111-111111111111',
      description_ref: 'content.forum.category.sites.description',
      discoverability: 'listed',
      id: '22222222-2222-4222-8222-222222222222',
      order_index: 10,
      slug: 'sites',
      title: 'Sites',
    },
    {
      archived_at: null,
      board_id: '11111111-1111-4111-8111-111111111111',
      description_ref: 'content.forum.category.void.description',
      discoverability: 'unlisted',
      id: '44444444-1111-4111-8111-444444444444',
      order_index: 900,
      slug: 'void',
      title: 'Void',
    },
  ]
  forums: Array<ForumRow> = [
    {
      archived_at: null,
      board_id: '11111111-1111-4111-8111-111111111111',
      category_id: '22222222-2222-4222-8222-222222222222',
      description_ref: 'content.forum.site_builder_help.description',
      discoverability: 'listed',
      id: '33333333-3333-4333-8333-333333333333',
      latest_post_id: null,
      latest_topic_id: null,
      locked: 0,
      post_count: 0,
      public_projection_json: publicProjectionJson,
      slug: 'site-builder-help',
      title: 'Site Builder Help',
      topic_count: 0,
      visibility: 'public',
    },
    {
      archived_at: null,
      board_id: '11111111-1111-4111-8111-111111111111',
      category_id: '44444444-1111-4111-8111-444444444444',
      description_ref: 'content.forum.void.description',
      discoverability: 'unlisted',
      id: '55555555-1111-4111-8111-555555555555',
      latest_post_id: null,
      latest_topic_id: null,
      locked: 0,
      post_count: 0,
      public_projection_json: publicProjectionJson,
      slug: 'void',
      title: 'Void',
      topic_count: 0,
      visibility: 'public',
    },
  ]
  topics: Array<TopicRow> = []
  posts: Array<PostRow> = []
  watches: Array<WatchRow> = []
  bookmarks: Array<BookmarkRow> = []
  reports: Array<ReportRow> = []
  moderationEvents: Array<ModerationEventRow> = []
  privateMessageThreads: Array<PrivateMessageThreadRow> = []
  privateMessages: Array<PrivateMessageRow> = []
  receipts: Array<ReceiptRow> = []
  tipRecipientWallets: Array<TipRecipientWalletRow> = []
  contextLinks: Array<ContextLinkRow> = []
}

const forumTopicPinRank = (pinState: TopicRow['pin_state']): number =>
  pinState === 'announcement' ? 0 : pinState === 'sticky' ? 1 : 2

const forumTopicActivityIso = (
  store: ForumRepositoryStore,
  topic: TopicRow,
): string => {
  const latestPost = store.posts.find(
    post =>
      post.id === topic.latest_post_id &&
      post.topic_id === topic.id &&
      post.archived_at === null &&
      (post.state === 'visible' ||
        post.state === 'edited' ||
        post.state === 'tombstoned'),
  )

  return latestPost?.created_at ?? latestPost?.updated_at ?? topic.updated_at
}

const sortForumTopicListRows = (
  store: ForumRepositoryStore,
  rows: ReadonlyArray<TopicRow>,
): Array<TopicRow> =>
  [...rows].sort(
    (left, right) =>
      forumTopicActivityIso(store, right).localeCompare(
        forumTopicActivityIso(store, left),
      ) ||
      forumTopicPinRank(left.pin_state) - forumTopicPinRank(right.pin_state) ||
      right.updated_at.localeCompare(left.updated_at) ||
      right.created_at.localeCompare(left.created_at) ||
      left.id.localeCompare(right.id),
  )

class ForumRepositoryStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: ForumRepositoryStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM forum_boards')) {
      const row =
        this.store.boards.find(
          item => item.slug === 'openagents' && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('COALESCE(MAX(post_number)')) {
      const topicId = String(this.values[0])
      const postNumbers = this.store.posts
        .filter(item => item.topic_id === topicId && item.archived_at === null)
        .map(item => item.post_number)
      const postNumber = postNumbers.length === 0 ? 0 : Math.max(...postNumbers)

      return Promise.resolve({ post_number: postNumber } as T)
    }

    if (this.query.includes('FROM forum_context_links')) {
      const id = String(this.values[0])
      const row =
        this.store.contextLinks.find(
          item => item.id === id && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_tip_recipient_wallets')) {
      const actorRef = String(this.values[0])
      const row =
        this.store.tipRecipientWallets.find(
          item => item.actor_ref === actorRef && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_topics')) {
      const topicId = String(this.values[0])
      const row =
        this.store.topics.find(
          item => item.id === topicId && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('COUNT(*) AS live_count')) {
      const topicId = String(this.values[0])
      const count = this.store.posts.filter(
        item =>
          item.topic_id === topicId &&
          item.archived_at === null &&
          (item.state === 'visible' || item.state === 'edited'),
      ).length

      return Promise.resolve({ live_count: count } as T)
    }

    if (this.query.includes('FROM forum_posts')) {
      const postId = String(this.values[0])
      const row =
        this.store.posts.find(
          item => item.id === postId && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_forums')) {
      const forumRef = String(this.values[0])
      const slugRef = String(this.values[1] ?? this.values[0])
      const row =
        this.store.forums.find(
          item =>
            (item.id === forumRef || item.slug === slugRef) &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT INTO forum_tip_recipient_wallets')) {
      const actorRef = String(this.values[1])
      const row: TipRecipientWalletRow = {
        actor_ref: actorRef,
        archived_at: null,
        spark_address: this.values[5] === null ? null : String(this.values[5]),
        bolt12_offer: this.values[6] === null ? null : String(this.values[6]),
        lightning_address:
          this.values[7] === null ? null : String(this.values[7]),
        caveat_refs_json: String(this.values[10]),
        claim_policy_refs_json: String(this.values[12]),
        created_at: String(this.values[16]),
        custody_policy_refs_json: String(this.values[11]),
        disabled_at: this.values[18] === null ? null : String(this.values[18]),
        id: String(this.values[0]),
        payout_target_approval_ref:
          this.values[8] === null ? null : String(this.values[8]),
        provider_class: this.values[2] as
          | 'external_lightning'
          | 'hosted_mdk'
          | 'mdk_agent_wallet',
        public_projection_json: String(this.values[15]),
        readiness_refs_json: String(this.values[9]),
        receive_capability_ref: String(this.values[4]),
        source_ref: String(this.values[13]),
        state: this.values[14] as 'ready' | 'disabled' | 'blocked',
        updated_at: String(this.values[17]),
        wallet_ref: String(this.values[3]),
      }
      const existingIndex = this.store.tipRecipientWallets.findIndex(
        item => item.actor_ref === actorRef,
      )

      if (existingIndex === -1) {
        this.store.tipRecipientWallets.push(row)
      } else {
        this.store.tipRecipientWallets[existingIndex] = {
          ...row,
          created_at: this.store.tipRecipientWallets[existingIndex]!.created_at,
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_topics')) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.topics.every(item => item.idempotency_key !== idempotencyKey)
      ) {
        this.store.topics.push({
          actor_json: String(this.values[4]),
          actor_ref: String(this.values[3]),
          archived_at: null,
          created_at: String(this.values[10]),
          first_post_id: String(this.values[7]),
          forum_id: String(this.values[2]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          latest_post_id: String(this.values[8]),
          pin_state: 'normal',
          post_count: 1,
          public_projection_json: String(this.values[9]),
          score_ref: null,
          slug: String(this.values[5]),
          state: 'open',
          title: String(this.values[6]),
          updated_at: String(this.values[11]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_posts')) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.posts.every(item => item.idempotency_key !== idempotencyKey)
      ) {
        const firstPost = this.values.length === 10

        this.store.posts.push({
          actor_json: String(this.values[5]),
          actor_ref: String(this.values[4]),
          archived_at: null,
          body_text: null,
          content_ref: String(this.values[6]),
          created_at: String(firstPost ? this.values[8] : this.values[11]),
          forum_id: String(this.values[3]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          parent_post_id: firstPost ? null : String(this.values[7]),
          post_number: firstPost ? 1 : Number(this.values[9]),
          public_projection_json: String(
            firstPost ? this.values[7] : this.values[10],
          ),
          quote_post_id: firstPost ? null : String(this.values[8]),
          receipt_refs_json: '[]',
          revision_ref: null,
          state: 'visible',
          topic_id: String(this.values[2]),
          updated_at: String(firstPost ? this.values[9] : this.values[12]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_post_bodies')) {
      const postId = String(this.values[0])
      const existing = this.store.posts.find(item => item.id === postId)

      if (existing !== undefined) {
        const index = this.store.posts.findIndex(item => item.id === postId)

        this.store.posts[index] = {
          ...existing,
          body_text: String(this.values[1]),
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_forums')) {
      const isDecrement = this.query.includes(
        'post_count = MAX(0, post_count - 1)',
      )
      const forumId = isDecrement
        ? String(this.values[0])
        : String(this.values[3])
      const existing = this.store.forums.find(item => item.id === forumId)

      if (existing !== undefined) {
        const index = this.store.forums.findIndex(item => item.id === forumId)
        this.store.forums[index] = isDecrement
          ? {
              ...existing,
              post_count: Math.max(0, existing.post_count - 1),
            }
          : {
              ...existing,
              latest_post_id: String(this.values[1]),
              latest_topic_id: String(this.values[0]),
              post_count: existing.post_count + 1,
              topic_count: this.query.includes('topic_count = topic_count + 1')
                ? existing.topic_count + 1
                : existing.topic_count,
            }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_topics')) {
      const isDecrement = this.query.includes(
        'post_count = MAX(0, post_count - 1)',
      )
      const topicId = String(this.values[isDecrement ? 1 : 2])
      const existing = this.store.topics.find(item => item.id === topicId)

      if (existing !== undefined) {
        const index = this.store.topics.findIndex(item => item.id === topicId)

        this.store.topics[index] = isDecrement
          ? {
              ...existing,
              post_count: Math.max(0, existing.post_count - 1),
              updated_at: String(this.values[0]),
            }
          : {
              ...existing,
              latest_post_id: String(this.values[0]),
              post_count: existing.post_count + 1,
              updated_at: String(this.values[1]),
            }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO forum_watches')) {
      const idempotencyKey = String(this.values[5])

      if (
        this.store.watches.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.watches.push({
          actor_ref: String(this.values[1]),
          forum_id: this.values[2] === null ? null : String(this.values[2]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          topic_id: this.values[3] === null ? null : String(this.values[3]),
          watch_kind: this.values[4] as 'forum' | 'topic',
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO forum_bookmarks')) {
      const idempotencyKey = String(this.values[5])

      if (
        this.store.bookmarks.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.bookmarks.push({
          actor_ref: String(this.values[1]),
          bookmark_kind: this.values[4] as 'topic' | 'post',
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          post_id: this.values[3] === null ? null : String(this.values[3]),
          topic_id: this.values[2] === null ? null : String(this.values[2]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_reports')) {
      this.store.reports.push({
        id: String(this.values[0]),
        reason_ref: String(this.values[5]),
        reporter_actor_ref: String(this.values[2]),
        status: 'open',
        target_id: String(this.values[4]),
        target_kind: this.values[3] as 'forum' | 'topic' | 'post' | 'user',
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_moderation_events')) {
      this.store.moderationEvents.push({
        action_kind: String(this.values[3]),
        id: String(this.values[0]),
        idempotency_key:
          this.values[1] === null ? null : String(this.values[1]),
        moderator_actor_ref: String(this.values[2]),
        public_projection_json: String(this.values[8]),
        reason_ref: String(this.values[6]),
        report_id: this.values[7] === null ? null : String(this.values[7]),
        target_id: String(this.values[5]),
        target_kind: this.values[4] as
          | 'forum'
          | 'topic'
          | 'post'
          | 'report'
          | 'user',
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_private_message_threads')) {
      this.store.privateMessageThreads.push({
        id: String(this.values[0]),
        latest_message_id: null,
        message_count: 0,
        participant_refs_json: String(this.values[4]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_private_messages')) {
      this.store.privateMessages.push({
        archived_at: null,
        content_ref: String(this.values[4]),
        created_at: String(this.values[6]),
        id: String(this.values[0]),
        public_projection_json: String(this.values[5]),
        recipient_actor_ref: String(this.values[3]),
        sender_actor_ref: String(this.values[2]),
        thread_id: String(this.values[1]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_private_message_threads')) {
      const threadId = String(this.values[2])
      const existing = this.store.privateMessageThreads.find(
        item => item.id === threadId,
      )

      if (existing !== undefined) {
        const index = this.store.privateMessageThreads.findIndex(
          item => item.id === threadId,
        )

        this.store.privateMessageThreads[index] = {
          ...existing,
          latest_message_id: String(this.values[0]),
          message_count: existing.message_count + 1,
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_receipts')) {
      this.store.receipts.push({
        action_kind: String(this.values[2]),
        amount_asset: this.values[6] as 'credits' | 'sats' | 'usd',
        amount_value: Number(this.values[7]),
        id: String(this.values[0]),
        receipt_ref: String(this.values[1]),
        redacted_payment_ref: String(this.values[9]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO forum_context_links')) {
      const targetKind = this.values[1] as 'topic' | 'post'
      const targetId = String(this.values[2])
      const contextKind = this.values[6] as 'site' | 'workroom'
      const contextId = String(this.values[7])

      if (
        this.store.contextLinks.every(
          item =>
            item.target_kind !== targetKind ||
            item.target_id !== targetId ||
            item.context_kind !== contextKind ||
            item.context_id !== contextId,
        )
      ) {
        this.store.contextLinks.push({
          archived_at: null,
          context_id: contextId,
          context_kind: contextKind,
          context_slug: this.values[8] === null ? null : String(this.values[8]),
          context_title:
            this.values[9] === null ? null : String(this.values[9]),
          created_at: String(this.values[13]),
          forum_id: String(this.values[3]),
          id: String(this.values[0]),
          post_id: this.values[5] === null ? null : String(this.values[5]),
          public_projection_json: String(this.values[12]),
          public_url: this.values[10] === null ? null : String(this.values[10]),
          source_ref: this.values[11] === null ? null : String(this.values[11]),
          target_id: targetId,
          target_kind: targetKind,
          topic_id: this.values[4] === null ? null : String(this.values[4]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM forum_money_actions')) {
      return Promise.resolve({ results: [] } as unknown as D1Result<T>)
    }

    if (this.query.includes('FROM forum_categories')) {
      const boardId = String(this.values[0])
      const listedOnly = this.query.includes("discoverability = 'listed'")
      const rows = this.store.categories.filter(
        item =>
          item.board_id === boardId &&
          item.archived_at === null &&
          (listedOnly
            ? item.discoverability === 'listed'
            : item.discoverability !== 'hidden'),
      )

      return Promise.resolve({ results: rows } as unknown as D1Result<T>)
    }

    if (this.query.includes('FROM forum_forums')) {
      if (this.query.includes('title LIKE')) {
        const pattern = String(this.values[0]).replaceAll('%', '').toLowerCase()
        const exactSlug = String(this.values[1])
        const listedOnly = this.query.includes("discoverability = 'listed'")
        const rows = this.store.forums.filter(
          item =>
            item.archived_at === null &&
            item.visibility === 'public' &&
            item.discoverability !== 'hidden' &&
            (!listedOnly || item.discoverability === 'listed') &&
            (item.title.toLowerCase().includes(pattern) ||
              item.slug === exactSlug),
        )

        return Promise.resolve({ results: rows } as unknown as D1Result<T>)
      }

      const boardId = String(this.values[0])
      const listedOnly = this.query.includes("discoverability = 'listed'")
      const rows = this.store.forums.filter(
        item =>
          item.board_id === boardId &&
          item.archived_at === null &&
          item.visibility === 'public' &&
          (listedOnly
            ? item.discoverability === 'listed'
            : item.discoverability !== 'hidden'),
      )

      return Promise.resolve({ results: rows } as unknown as D1Result<T>)
    }

    if (
      this.query.includes('FROM forum_context_links') &&
      this.query.includes('JOIN forum_topics') &&
      !this.query.includes('FROM forum_posts')
    ) {
      const contextKind = this.values[0] as 'site' | 'workroom'
      const contextId = String(this.values[1])
      const topicIds = this.store.contextLinks
        .filter(
          item =>
            item.context_kind === contextKind &&
            item.context_id === contextId &&
            item.archived_at === null &&
            item.topic_id !== null &&
            item.public_projection_json.includes('"publicSafe":true') &&
            item.public_projection_json.includes(
              '"dataClassification":"public"',
            ),
        )
        .map(item => item.topic_id)
      const rows = this.store.topics.filter(item => {
        const forum = this.store.forums.find(f => f.id === item.forum_id)

        return (
          topicIds.includes(item.id) &&
          forum !== undefined &&
          item.archived_at === null &&
          (item.state === 'open' || item.state === 'locked') &&
          forum.archived_at === null &&
          forum.visibility === 'public' &&
          forum.discoverability !== 'hidden'
        )
      })

      return Promise.resolve({ results: rows } as unknown as D1Result<T>)
    }

    if (
      this.query.includes('FROM forum_context_links') &&
      !this.query.includes('FROM forum_posts')
    ) {
      const contextKind = this.values[0] as 'site' | 'workroom'
      const contextId = String(this.values[1])
      const rows = this.store.contextLinks.filter(
        item =>
          item.context_kind === contextKind &&
          item.context_id === contextId &&
          item.archived_at === null,
      )

      return Promise.resolve({ results: rows } as unknown as D1Result<T>)
    }

    if (this.query.includes('FROM forum_topics')) {
      if (this.query.includes('JOIN forum_forums')) {
        const pattern = String(this.values[0]).replaceAll('%', '').toLowerCase()
        const exactSlug = String(this.values[1])
        const listedOnly = this.query.includes("discoverability = 'listed'")
        const rows = this.store.topics.filter(item => {
          const forum = this.store.forums.find(f => f.id === item.forum_id)

          return (
            forum !== undefined &&
            item.archived_at === null &&
            (item.state === 'open' || item.state === 'locked') &&
            forum.archived_at === null &&
            forum.visibility === 'public' &&
            forum.discoverability !== 'hidden' &&
            (!listedOnly || forum.discoverability === 'listed') &&
            (item.title.toLowerCase().includes(pattern) ||
              item.slug === exactSlug)
          )
        })

        return Promise.resolve({ results: rows } as unknown as D1Result<T>)
      }

      const forumId = String(this.values[0])
      const limit = Number(this.values[1] ?? 50)
      const rows = sortForumTopicListRows(
        this.store,
        this.store.topics.filter(
          item =>
            item.forum_id === forumId &&
            item.archived_at === null &&
            (item.state === 'open' || item.state === 'locked'),
        ),
      ).slice(0, limit)

      return Promise.resolve({ results: rows } as unknown as D1Result<T>)
    }

    if (this.query.includes('FROM forum_posts')) {
      if (this.query.includes('forum_posts.id IN')) {
        const contextKind = this.values[0] as 'site' | 'workroom'
        const contextId = String(this.values[1])
        const publicLinks = this.store.contextLinks.filter(
          item =>
            item.context_kind === contextKind &&
            item.context_id === contextId &&
            item.archived_at === null &&
            item.public_projection_json.includes('"publicSafe":true') &&
            item.public_projection_json.includes(
              '"dataClassification":"public"',
            ),
        )
        const postIds = publicLinks
          .filter(item => item.post_id !== null)
          .map(item => item.post_id)
        const topicIds = publicLinks
          .filter(item => item.topic_id !== null)
          .map(item => item.topic_id)
        const rows = this.store.posts.filter(item => {
          const topic = this.store.topics.find(t => t.id === item.topic_id)
          const forum = this.store.forums.find(f => f.id === item.forum_id)

          return (
            (postIds.includes(item.id) || topicIds.includes(item.topic_id)) &&
            topic !== undefined &&
            forum !== undefined &&
            item.archived_at === null &&
            (item.state === 'visible' || item.state === 'edited') &&
            topic.archived_at === null &&
            (topic.state === 'open' || topic.state === 'locked') &&
            forum.archived_at === null &&
            forum.visibility === 'public' &&
            forum.discoverability !== 'hidden'
          )
        })

        return Promise.resolve({ results: rows } as unknown as D1Result<T>)
      }

      if (this.query.includes('JOIN forum_topics')) {
        const pattern = String(this.values[0]).replaceAll('%', '').toLowerCase()
        const exactContentRef = String(this.values[1])
        const listedOnly = this.query.includes("discoverability = 'listed'")
        const rows = this.store.posts.filter(item => {
          const topic = this.store.topics.find(t => t.id === item.topic_id)
          const forum = this.store.forums.find(f => f.id === item.forum_id)

          return (
            topic !== undefined &&
            forum !== undefined &&
            item.archived_at === null &&
            (item.state === 'visible' || item.state === 'edited') &&
            topic.archived_at === null &&
            (topic.state === 'open' || topic.state === 'locked') &&
            forum.archived_at === null &&
            forum.visibility === 'public' &&
            forum.discoverability !== 'hidden' &&
            (!listedOnly || forum.discoverability === 'listed') &&
            ((item.body_text ?? '').toLowerCase().includes(pattern) ||
              item.content_ref === exactContentRef)
          )
        })

        return Promise.resolve({ results: rows } as unknown as D1Result<T>)
      }

      const topicId = String(this.values[0])
      const descending = this.query.includes(
        'ORDER BY forum_posts.post_number DESC',
      )
      // Mirror production: the topic-detail projection excludes tombstoned
      // (deleted) posts so a deleted post never renders in the thread.
      const includeTombstoned = this.query.includes("'tombstoned'")
      const rows = this.store.posts
        .filter(
          item =>
            item.topic_id === topicId &&
            item.archived_at === null &&
            (item.state === 'visible' ||
              item.state === 'edited' ||
              (includeTombstoned && item.state === 'tombstoned')),
        )
        .sort((left, right) =>
          descending
            ? right.post_number - left.post_number
            : left.post_number - right.post_number,
        )

      return Promise.resolve({ results: rows } as unknown as D1Result<T>)
    }

    if (this.query.includes('FROM forum_tip_recipient_wallets')) {
      const actorRefs = new Set(this.values.map(value => String(value)))
      const rows = this.store.tipRecipientWallets.filter(
        item => actorRefs.has(item.actor_ref) && item.archived_at === null,
      )

      return Promise.resolve({ results: rows } as unknown as D1Result<T>)
    }

    if (this.query.includes('FROM forum_private_messages')) {
      const actorRef = String(this.values[0])
      const rows = this.store.privateMessages.filter(
        item =>
          item.archived_at === null &&
          (item.sender_actor_ref === actorRef ||
            item.recipient_actor_ref === actorRef),
      )

      return Promise.resolve({ results: rows } as unknown as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true
      ? Promise.resolve([[]])
      : Promise.resolve([])
  }
}

const forumRepositoryDb = (store: ForumRepositoryStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new ForumRepositoryStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const runtime: ForumRepositoryRuntime = {
  makeId: () => 'generated-forum-row-id',
  nowIso: () => '2026-06-05T20:00:00.000Z',
}

const actor: ForumStoredActorSummary = {
  actorId: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
  actorRef: 'actor.ben',
  displayName: 'Ben',
  groupRefs: ['group.customers'],
  isAgent: false,
  slug: 'ben-silone',
}

const publicProjection = S.decodeUnknownSync(ForumPublicProjectionSchema)(
  publicProjectionFixture,
)

const createTopic = (
  store: ForumRepositoryStore,
  overrides: Partial<Parameters<typeof createForumTopicWithFirstPost>[1]> = {},
) =>
  Effect.runPromise(
    createForumTopicWithFirstPost(
      forumRepositoryDb(store),
      {
        actor,
        bodyText: 'First OTEC forum post body.',
        contentRef: 'content.forum.topic.first',
        firstPostId: '55555555-5555-4555-8555-555555555555',
        forumId: '33333333-3333-4333-8333-333333333333',
        idempotencyKey:
          'forum:topic:33333333-3333-4333-8333-333333333333:actor.ben:1',
        publicProjection,
        slug: 'otc-floating-datacenter',
        title: 'OTEC Floating Datacenter',
        topicId: '44444444-4444-4444-8444-444444444444',
        ...overrides,
      },
      runtime,
    ),
  )

const readyTipRecipientWalletInput = (
  overrides: Partial<Parameters<typeof upsertForumTipRecipientWallet>[1]> = {},
): Parameters<typeof upsertForumTipRecipientWallet>[1] => ({
  actorRef: actor.actorRef,
  bolt12Offer:
    'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
  caveatRefs: ['caveat.public.forum_tip_recipient.claim_required'],
  claimPolicyRefs: ['policy.public.forum_tip_recipient.agent_claimed'],
  custodyPolicyRefs: ['policy.public.forum_tip_recipient.self_custody'],
  id: 'forum_tip_recipient_wallet_ben',
  payoutTargetApprovalRef: 'approval.public.forum_tip_recipient.ben',
  providerClass: 'mdk_agent_wallet',
  readinessRefs: ['readiness.public.forum_tip_recipient.receive_ready'],
  receiveCapabilityRef: 'receive_capability.public.forum_tip_recipient.ben',
  sourceRef: 'source.public.pylon_api_registration.ben',
  state: 'ready',
  walletRef: 'wallet.public.forum_tip_recipient.ben',
  ...overrides,
})

describe('Forum repository foundation', () => {
  test('reads the default board index without listing the void test lane', async () => {
    const store = new ForumRepositoryStore()
    const board = await Effect.runPromise(
      readForumBoardIndex(forumRepositoryDb(store)),
    )
    const testBoard = await Effect.runPromise(
      readForumBoardIndex(forumRepositoryDb(store), { includeUnlisted: true }),
    )

    expect(board?.slug).toBe('openagents')
    expect(board?.forums.map(forum => forum.slug)).toStrictEqual([
      'site-builder-help',
    ])
    expect(board?.categories.map(category => category.slug)).toStrictEqual([
      'sites',
    ])
    expect(testBoard?.forums.map(forum => forum.slug).sort()).toStrictEqual([
      'site-builder-help',
      'void',
    ])
    expect(
      testBoard?.categories.map(category => category.slug).sort(),
    ).toStrictEqual(['sites', 'void'])
  })

  test('reads the unlisted void forum by exact slug with a public-safe projection', async () => {
    const store = new ForumRepositoryStore()
    const forum = await Effect.runPromise(
      readForumSummaryByRef(forumRepositoryDb(store), 'void', {
        allowUnlisted: true,
      }),
    )

    expect(forum).toMatchObject({
      discoverability: 'unlisted',
      publicProjection: {
        publicSafe: true,
      },
      slug: 'void',
      visibility: 'public',
    })
    await expect(
      Effect.runPromise(
        readForumSummaryByRef(forumRepositoryDb(store), 'void', {
          allowUnlisted: false,
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'ForumReadAccessDenied',
      denialKind: 'hidden',
    })
  })

  test('reads forum topic list, topic detail, and post detail', async () => {
    const store = new ForumRepositoryStore()
    const { firstPost, topic } = await createTopic(store)
    const reply = await Effect.runPromise(
      createForumReplyPost(
        forumRepositoryDb(store),
        {
          actor,
          bodyText: 'First reply body.',
          contentRef: 'content.forum.topic.reply_1',
          forumId: topic.forumId,
          idempotencyKey:
            'forum:reply:44444444-4444-4444-8444-444444444444:actor.ben:1',
          parentPostId: firstPost.postId,
          postId: '66666666-6666-4666-8666-666666666666',
          publicProjection,
          quotePostId: firstPost.postId,
          topicId: topic.topicId,
        },
        runtime,
      ),
    )
    const topicList = await Effect.runPromise(
      readForumTopicList(forumRepositoryDb(store), topic.forumId),
    )
    const topicDetail = await Effect.runPromise(
      readForumTopicDetail(forumRepositoryDb(store), topic.topicId),
    )
    const newestFirstTopicDetail = await Effect.runPromise(
      readForumTopicDetail(forumRepositoryDb(store), topic.topicId, {
        postSortDirection: 'desc',
      }),
    )
    const postDetail = await Effect.runPromise(
      readForumPostDetail(forumRepositoryDb(store), reply.postId),
    )

    expect(topicList?.topics).toHaveLength(1)
    expect(topicList?.topics[0]).toMatchObject({
      postCount: 2,
      topicId: topic.topicId,
    })
    expect(topicDetail?.posts.map(post => post.postId)).toStrictEqual([
      firstPost.postId,
      reply.postId,
    ])
    expect(topicDetail).toMatchObject({
      topicHref: `/forum/t/${topic.topicId}`,
      webUrl: `https://openagents.com/forum/t/${topic.topicId}`,
    })
    expect(
      newestFirstTopicDetail?.posts.map(post => post.postId),
    ).toStrictEqual([reply.postId, firstPost.postId])
    expect(postDetail).toMatchObject({
      containingTopicId: topic.topicId,
      post: {
        postId: reply.postId,
        tipRecipientReadiness: {
          blockerRef: 'blocker.public.forum_tip_recipient.wallet_missing',
          state: 'missing',
          tippingAvailable: false,
        },
      },
    })
  })

  test('excludes tombstoned posts from the topic-detail projection and reports live counts', async () => {
    const store = new ForumRepositoryStore()
    const { firstPost, topic } = await createTopic(store)
    const reply = await Effect.runPromise(
      createForumReplyPost(
        forumRepositoryDb(store),
        {
          actor,
          bodyText: 'Surviving child reply.',
          contentRef: 'content.forum.topic.reply_survivor',
          forumId: topic.forumId,
          idempotencyKey:
            'forum:reply:44444444-4444-4444-8444-444444444444:actor.ben:survivor',
          parentPostId: firstPost.postId,
          postId: '66666666-6666-4666-8666-666666666666',
          publicProjection,
          quotePostId: null,
          topicId: topic.topicId,
        },
        runtime,
      ),
    )

    // Simulate a deleted (tombstoned) PARENT row that survives for audit.
    const parentIndex = store.posts.findIndex(
      item => item.id === firstPost.postId,
    )
    store.posts[parentIndex] = {
      ...store.posts[parentIndex]!,
      body_text: null,
      state: 'tombstoned',
    }

    const topicDetail = await Effect.runPromise(
      readForumTopicDetail(forumRepositoryDb(store), topic.topicId),
    )

    // The deleted parent is absent from the thread; only the live child shows.
    expect(topicDetail?.posts.map(post => post.postId)).toStrictEqual([
      reply.postId,
    ])
    expect(
      topicDetail?.posts.some(post => post.state === 'tombstoned'),
    ).toBe(false)
    // No surviving post carries a null body (which would force the broken
    // `content.forum.post.<id>` placeholder on the client).
    expect(topicDetail?.posts.some(post => post.bodyText === null)).toBe(false)
    // The surviving child still references its (now hidden) parent: no orphan.
    expect(topicDetail?.posts[0]?.parentPostId).toBe(firstPost.postId)
    // Counts reflect exactly one live post.
    expect(topicDetail?.topic.postCount).toBe(1)
    expect(topicDetail?.topic.replyCount).toBe(0)
  })

  test('orders forum topic lists by newest visible post activity before pin state', async () => {
    const store = new ForumRepositoryStore()
    const announcement = await createTopic(store, {
      firstPostId: 'aaaaaaaa-1000-4000-8000-aaaaaaaa1000',
      idempotencyKey: 'forum:topic:activity-order:announcement',
      slug: 'older-announcement',
      title: 'Older announcement',
      topicId: 'aaaaaaaa-2000-4000-8000-aaaaaaaa2000',
    })
    const sticky = await createTopic(store, {
      firstPostId: 'bbbbbbbb-1000-4000-8000-bbbbbbbb1000',
      idempotencyKey: 'forum:topic:activity-order:sticky',
      slug: 'newer-sticky',
      title: 'Newer sticky',
      topicId: 'bbbbbbbb-2000-4000-8000-bbbbbbbb2000',
    })
    const announcementIndex = store.topics.findIndex(
      item => item.id === announcement.topic.topicId,
    )
    const stickyIndex = store.topics.findIndex(
      item => item.id === sticky.topic.topicId,
    )

    store.topics[announcementIndex] = {
      ...store.topics[announcementIndex]!,
      pin_state: 'announcement',
      updated_at: '2026-06-15T12:00:00.000Z',
    }
    store.posts.push({
      actor_json: JSON.stringify(actor),
      actor_ref: actor.actorRef,
      archived_at: null,
      body_text: 'Reply that should control recency.',
      content_ref: 'content.forum.topic.activity_order.reply',
      created_at: '2026-06-16T12:00:00.000Z',
      forum_id: sticky.topic.forumId,
      id: 'bbbbbbbb-3000-4000-8000-bbbbbbbb3000',
      idempotency_key: 'forum:post:activity-order:sticky-reply',
      parent_post_id: sticky.firstPost.postId,
      post_number: 2,
      public_projection_json: publicProjectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible',
      topic_id: sticky.topic.topicId,
      updated_at: '2026-06-16T12:00:00.000Z',
    })
    store.topics[stickyIndex] = {
      ...store.topics[stickyIndex]!,
      latest_post_id: 'bbbbbbbb-3000-4000-8000-bbbbbbbb3000',
      pin_state: 'sticky',
      post_count: 2,
      updated_at: '2026-06-10T12:00:00.000Z',
    }

    const topicList = await Effect.runPromise(
      readForumTopicList(forumRepositoryDb(store), sticky.topic.forumId),
    )

    expect(topicList?.topics.map(topic => topic.title)).toStrictEqual([
      'Newer sticky',
      'Older announcement',
    ])
    expect(topicList?.topics[0]?.lastPost).toMatchObject({
      createdAt: '2026-06-16T12:00:00.000Z',
      postId: 'bbbbbbbb-3000-4000-8000-bbbbbbbb3000',
    })
  })

  test('projects Forum tip recipient wallet readiness for post authors', async () => {
    const store = new ForumRepositoryStore()
    const { firstPost, topic } = await createTopic(store)
    const missing = await Effect.runPromise(
      readForumTipRecipientReadinessForActor(
        forumRepositoryDb(store),
        actor.actorRef,
      ),
    )
    const ready = await Effect.runPromise(
      upsertForumTipRecipientWallet(
        forumRepositoryDb(store),
        readyTipRecipientWalletInput(),
        runtime,
      ),
    )
    const topicDetail = await Effect.runPromise(
      readForumTopicDetail(forumRepositoryDb(store), firstPost.topicId),
    )
    const postDetail = await Effect.runPromise(
      readForumPostDetail(forumRepositoryDb(store), firstPost.postId),
    )
    const replyAfterReadinessClaim = await Effect.runPromise(
      createForumReplyPost(
        forumRepositoryDb(store),
        {
          actor,
          bodyText: 'Reply after readiness claim.',
          contentRef: 'content.forum.topic.reply_after_readiness',
          forumId: topic.forumId,
          idempotencyKey:
            'forum:reply:77777777-7777-4777-8777-777777777777:actor.ben:ready',
          parentPostId: firstPost.postId,
          postId: '77777777-7777-4777-8777-777777777777',
          publicProjection,
          quotePostId: null,
          topicId: firstPost.topicId,
        },
        runtime,
      ),
    )
    const topicPost = topicDetail?.posts[0]

    expect(missing).toMatchObject({
      blockerRef: 'blocker.public.forum_tip_recipient.wallet_missing',
      providerClass: null,
      state: 'missing',
      tippingAvailable: false,
    })
    expect(ready).toMatchObject({
      blockerRef: null,
      directPayment: {
        bolt12Offer:
          'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
        kind: 'bolt12_offer',
        settlementAuthority: 'recipient_wallet_direct',
      },
      providerClass: 'mdk_agent_wallet',
      readinessRefs: ['readiness.public.forum_tip_recipient.receive_ready'],
      state: 'ready',
      tippingAvailable: true,
    })
    expect(postDetail?.post.tipRecipientReadiness).toMatchObject({
      blockerRef: null,
      directPayment: {
        kind: 'bolt12_offer',
        settlementAuthority: 'recipient_wallet_direct',
      },
      state: 'ready',
      tippingAvailable: true,
    })
    expect(topicPost).toBeDefined()
    expect(topicPost!.tipRecipientReadiness).toMatchObject({
      blockerRef: null,
      directPayment: {
        kind: 'bolt12_offer',
        settlementAuthority: 'recipient_wallet_direct',
      },
      state: 'ready',
      tippingAvailable: true,
    })
    expect(topicPost!.capabilities).toMatchObject({ canTip: true })
    expect(replyAfterReadinessClaim.tipRecipientReadiness).toMatchObject({
      blockerRef: null,
      directPayment: {
        kind: 'bolt12_offer',
        settlementAuthority: 'recipient_wallet_direct',
      },
      state: 'ready',
      tippingAvailable: true,
    })
    expect(replyAfterReadinessClaim.capabilities).toMatchObject({
      canTip: true,
    })
    expect(JSON.stringify(postDetail)).not.toContain(
      'wallet.public.forum_tip_recipient.ben',
    )
    expect(JSON.stringify(topicDetail)).not.toContain(
      'wallet.public.forum_tip_recipient.ben',
    )
    expect(JSON.stringify(postDetail)).not.toContain(
      'receive_capability.public.forum_tip_recipient.ben',
    )
    expect(JSON.stringify(topicDetail)).not.toContain(
      'receive_capability.public.forum_tip_recipient.ben',
    )
  })

  test('keeps ready recipient rows without payment destinations visible but not tip-payable', async () => {
    const store = new ForumRepositoryStore()
    const readiness = await Effect.runPromise(
      upsertForumTipRecipientWallet(
        forumRepositoryDb(store),
        readyTipRecipientWalletInput({ bolt12Offer: null }),
        runtime,
      ),
    )

    expect(readiness).toMatchObject({
      blockerRef:
        'blocker.public.forum_tip_recipient.payment_instruction_missing',
      caveatRefs: expect.arrayContaining([
        'caveat.public.forum_tip_recipient.payment_instruction_missing',
      ]),
      directPayment: null,
      state: 'ready',
      tippingAvailable: false,
    })
  })

  test('projects disabled and blocked recipient wallets as unavailable', async () => {
    const store = new ForumRepositoryStore()
    const disabled = await Effect.runPromise(
      upsertForumTipRecipientWallet(
        forumRepositoryDb(store),
        readyTipRecipientWalletInput({
          caveatRefs: ['caveat.public.forum_tip_recipient.owner_disabled'],
          disabledAt: '2026-06-07T10:00:00.000Z',
          state: 'disabled',
        }),
        runtime,
      ),
    )
    const blocked = await Effect.runPromise(
      upsertForumTipRecipientWallet(
        forumRepositoryDb(store),
        readyTipRecipientWalletInput({
          caveatRefs: ['caveat.public.forum_tip_recipient.actor_blocked'],
          state: 'blocked',
        }),
        runtime,
      ),
    )

    expect(disabled).toMatchObject({
      blockerRef: 'blocker.public.forum_tip_recipient.wallet_disabled',
      readinessRefs: [],
      state: 'disabled',
      tippingAvailable: false,
    })
    expect(blocked).toMatchObject({
      blockerRef: 'blocker.public.forum_tip_recipient.actor_blocked',
      readinessRefs: [],
      state: 'blocked',
      tippingAvailable: false,
    })
  })

  test('rejects unsafe Forum tip recipient wallet admission material', async () => {
    const store = new ForumRepositoryStore()

    await expect(
      Effect.runPromise(
        upsertForumTipRecipientWallet(
          forumRepositoryDb(store),
          readyTipRecipientWalletInput({
            readinessRefs: ['payment_hash=abc123'],
          }),
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(ForumValidationError)

    await expect(
      Effect.runPromise(
        upsertForumTipRecipientWallet(
          forumRepositoryDb(store),
          readyTipRecipientWalletInput({
            bolt12Offer: 'lnbc1rawinvoice',
          }),
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(ForumValidationError)
  })

  test('searches listed public content while excluding unlisted and hidden content by default', async () => {
    const store = new ForumRepositoryStore()
    const listedTopic = await createTopic(store)
    const voidTopic = await createTopic(store, {
      bodyText: 'Hello world from the void lane.',
      firstPostId: '77777777-7777-4777-8777-777777777777',
      forumId: '55555555-1111-4111-8111-555555555555',
      idempotencyKey:
        'forum:topic:55555555-1111-4111-8111-555555555555:actor.ben:1',
      slug: 'hello-world',
      title: 'Hello world',
      topicId: '88888888-8888-4888-8888-888888888888',
    })
    const defaultSearch = await Effect.runPromise(
      searchForumPublicContent(forumRepositoryDb(store), {
        query: 'Hello',
      }),
    )
    const unlistedSearch = await Effect.runPromise(
      searchForumPublicContent(forumRepositoryDb(store), {
        includeUnlisted: true,
        query: 'Hello',
      }),
    )

    expect(defaultSearch).toMatchObject({
      forums: [],
      posts: [],
      topics: [],
    })
    expect(unlistedSearch).toMatchObject({
      includeUnlisted: true,
      posts: [{ bodyText: 'Hello world from the void lane.' }],
      query: 'Hello',
      topics: [{ topicId: voidTopic.topic.topicId }],
    })

    const listedTopicIndex = store.topics.findIndex(
      item => item.id === listedTopic.topic.topicId,
    )
    const listedPostIndex = store.posts.findIndex(
      item => item.id === listedTopic.firstPost.postId,
    )

    store.forums[0] = { ...store.forums[0]!, discoverability: 'hidden' }
    await expect(
      Effect.runPromise(
        searchForumPublicContent(forumRepositoryDb(store), {
          includeUnlisted: true,
          query: 'OTEC',
        }),
      ),
    ).resolves.toMatchObject({ forums: [], posts: [], topics: [] })

    store.forums[0] = { ...store.forums[0]!, discoverability: 'listed' }
    store.topics[listedTopicIndex] = {
      ...store.topics[listedTopicIndex]!,
      state: 'hidden',
    }
    await expect(
      Effect.runPromise(
        searchForumPublicContent(forumRepositoryDb(store), {
          query: 'OTEC',
        }),
      ),
    ).resolves.toMatchObject({ topics: [] })

    store.topics[listedTopicIndex] = {
      ...store.topics[listedTopicIndex]!,
      state: 'open',
    }
    store.posts[listedPostIndex] = {
      ...store.posts[listedPostIndex]!,
      state: 'hidden',
    }
    await expect(
      Effect.runPromise(
        searchForumPublicContent(forumRepositoryDb(store), {
          query: 'First OTEC',
        }),
      ),
    ).resolves.toMatchObject({ posts: [] })
  })

  test('denies archived, hidden, and non-public forum read paths', async () => {
    const store = new ForumRepositoryStore()
    const [forum] = store.forums

    if (forum === undefined) {
      throw new Error('Expected forum fixture.')
    }

    store.forums[0] = { ...forum, archived_at: '2026-06-05T20:00:00.000Z' }
    await expect(
      Effect.runPromise(
        readForumSummaryByRef(forumRepositoryDb(store), forum.id, {
          allowUnlisted: true,
        }),
      ),
    ).resolves.toBeNull()

    store.forums[0] = { ...forum, discoverability: 'hidden' }
    await expect(
      Effect.runPromise(
        readForumSummaryByRef(forumRepositoryDb(store), forum.id, {
          allowUnlisted: true,
        }),
      ),
    ).rejects.toBeInstanceOf(ForumReadAccessDenied)

    store.forums[0] = { ...forum, visibility: 'team' }
    await expect(
      Effect.runPromise(
        readForumSummaryByRef(forumRepositoryDb(store), forum.id, {
          allowUnlisted: true,
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'ForumReadAccessDenied',
      denialKind: 'scope_denied',
    })
  })

  test('denies hidden topic and hidden post detail reads', async () => {
    const store = new ForumRepositoryStore()
    const { firstPost, topic } = await createTopic(store)
    const topicIndex = store.topics.findIndex(item => item.id === topic.topicId)
    const postIndex = store.posts.findIndex(
      item => item.id === firstPost.postId,
    )
    const topicRow = store.topics[topicIndex]
    const postRow = store.posts[postIndex]

    if (topicRow === undefined || postRow === undefined) {
      throw new Error('Expected persisted topic and post fixtures.')
    }

    store.topics[topicIndex] = { ...topicRow, state: 'hidden' }
    await expect(
      Effect.runPromise(
        readForumTopicDetail(forumRepositoryDb(store), topic.topicId),
      ),
    ).rejects.toMatchObject({
      _tag: 'ForumReadAccessDenied',
      objectKind: 'topic',
    })

    store.topics[topicIndex] = topicRow
    store.posts[postIndex] = { ...postRow, state: 'hidden' }
    await expect(
      Effect.runPromise(
        readForumPostDetail(forumRepositoryDb(store), firstPost.postId),
      ),
    ).rejects.toMatchObject({
      _tag: 'ForumReadAccessDenied',
      objectKind: 'post',
    })
  })

  test('persists topic creation as one topic plus one first post', async () => {
    const store = new ForumRepositoryStore()
    const result = await createTopic(store)
    const forum = await Effect.runPromise(
      readForumSummaryById(
        forumRepositoryDb(store),
        '33333333-3333-4333-8333-333333333333',
      ),
    )

    expect(result.topic.firstPostId).toBe(result.firstPost.postId)
    expect(result.firstPost.postNumber).toBe(1)
    expect(store.topics).toHaveLength(1)
    expect(store.posts).toHaveLength(1)
    expect(forum).toMatchObject({
      latestPostId: result.firstPost.postId,
      latestTopicId: result.topic.topicId,
      postCount: 1,
      topicCount: 1,
    })
  })

  test('links public-safe Site context activity and excludes private context links', async () => {
    const store = new ForumRepositoryStore()
    const { firstPost, topic } = await createTopic(store, {
      contextLinks: [
        {
          contextId: 'site_project_otec',
          contextKind: 'site',
          contextSlug: 'otec',
          contextTitle: 'OTEC Site',
          forumId: '33333333-3333-4333-8333-333333333333',
          id: '99999999-1111-4111-8111-999999999999',
          postId: null,
          publicProjection,
          publicUrl: 'https://openagents.com/sites/otec',
          sourceRef: 'site_project:site_project_otec',
          targetKind: 'topic',
          topicId: '44444444-4444-4444-8444-444444444444',
        },
      ],
    })

    store.contextLinks.push({
      archived_at: null,
      context_id: 'site_project_private',
      context_kind: 'site',
      context_slug: 'private',
      context_title: 'private@example.com',
      created_at: '2026-06-05T20:00:00.000Z',
      forum_id: topic.forumId,
      id: '99999999-2222-4222-8222-999999999999',
      post_id: null,
      public_projection_json: privateProjectionJson,
      public_url: 'https://openagents.com/sites/private?access_token=secret',
      source_ref: 'private_key:secret',
      target_id: topic.topicId,
      target_kind: 'topic',
      topic_id: topic.topicId,
    })

    const publicActivity = await Effect.runPromise(
      readForumContextActivity(forumRepositoryDb(store), {
        contextId: 'site_project_otec',
        contextKind: 'site',
      }),
    )
    const privateActivity = await Effect.runPromise(
      readForumContextActivity(forumRepositoryDb(store), {
        contextId: 'site_project_private',
        contextKind: 'site',
      }),
    )

    expect(publicActivity).toMatchObject({
      context: { contextId: 'site_project_otec', contextKind: 'site' },
      contextLinks: [
        {
          contextTitle: 'OTEC Site',
          publicUrl: 'https://openagents.com/sites/otec',
          targetKind: 'topic',
          topicId: topic.topicId,
        },
      ],
      posts: [{ postId: firstPost.postId }],
      topics: [{ topicId: topic.topicId }],
    })
    expect(JSON.stringify(publicActivity)).not.toContain('access_token')
    expect(JSON.stringify(publicActivity)).not.toContain('private@example.com')
    expect(privateActivity).toMatchObject({
      contextLinks: [],
      posts: [],
      topics: [],
    })
  })

  test('reply posts update topic and forum last-post references', async () => {
    const store = new ForumRepositoryStore()
    const { firstPost, topic } = await createTopic(store)
    const reply = await Effect.runPromise(
      createForumReplyPost(
        forumRepositoryDb(store),
        {
          actor,
          bodyText: 'First reply body.',
          contentRef: 'content.forum.topic.reply_1',
          forumId: topic.forumId,
          idempotencyKey:
            'forum:reply:44444444-4444-4444-8444-444444444444:actor.ben:1',
          parentPostId: firstPost.postId,
          postId: '66666666-6666-4666-8666-666666666666',
          publicProjection,
          quotePostId: firstPost.postId,
          topicId: topic.topicId,
        },
        runtime,
      ),
    )
    const readTopic = await Effect.runPromise(
      readForumTopicById(forumRepositoryDb(store), topic.topicId),
    )
    const readForum = await Effect.runPromise(
      readForumSummaryById(forumRepositoryDb(store), topic.forumId),
    )

    expect(reply).toMatchObject({
      parentPostId: firstPost.postId,
      postNumber: 2,
      quotePostId: firstPost.postId,
    })
    expect(readTopic).toMatchObject({
      latestPostId: reply.postId,
      postCount: 2,
    })
    expect(readForum).toMatchObject({
      latestPostId: reply.postId,
      latestTopicId: topic.topicId,
      postCount: 2,
      topicCount: 1,
    })
  })

  test('reads sticky and locked topic state from persisted rows', async () => {
    const store = new ForumRepositoryStore()
    const { topic } = await createTopic(store)
    const index = store.topics.findIndex(item => item.id === topic.topicId)
    const topicRow = store.topics[index]

    if (topicRow === undefined) {
      throw new Error('Expected persisted forum topic fixture.')
    }

    store.topics[index] = {
      ...topicRow,
      pin_state: 'sticky',
      state: 'locked',
    }

    await expect(
      Effect.runPromise(
        readForumTopicById(forumRepositoryDb(store), topic.topicId),
      ),
    ).resolves.toMatchObject({
      pinState: 'sticky',
      state: 'locked',
    })
  })

  test('records watches and bookmarks idempotently', async () => {
    const store = new ForumRepositoryStore()
    const { firstPost, topic } = await createTopic(store)

    await Effect.runPromise(
      watchForumTarget(
        forumRepositoryDb(store),
        {
          actorRef: actor.actorRef,
          forumId: null,
          idempotencyKey: 'forum:watch:actor.ben:topic:1',
          topicId: topic.topicId,
          watchKind: 'topic',
        },
        runtime,
      ),
    )
    await Effect.runPromise(
      watchForumTarget(
        forumRepositoryDb(store),
        {
          actorRef: actor.actorRef,
          forumId: null,
          idempotencyKey: 'forum:watch:actor.ben:topic:1',
          topicId: topic.topicId,
          watchKind: 'topic',
        },
        runtime,
      ),
    )
    await Effect.runPromise(
      bookmarkForumTarget(
        forumRepositoryDb(store),
        {
          actorRef: actor.actorRef,
          bookmarkKind: 'post',
          idempotencyKey: 'forum:bookmark:actor.ben:post:1',
          postId: firstPost.postId,
          topicId: topic.topicId,
        },
        runtime,
      ),
    )
    await Effect.runPromise(
      bookmarkForumTarget(
        forumRepositoryDb(store),
        {
          actorRef: actor.actorRef,
          bookmarkKind: 'post',
          idempotencyKey: 'forum:bookmark:actor.ben:post:1',
          postId: firstPost.postId,
          topicId: topic.topicId,
        },
        runtime,
      ),
    )

    expect(store.watches).toHaveLength(1)
    expect(store.bookmarks).toHaveLength(1)
  })

  test('records reports and linked moderation events', async () => {
    const store = new ForumRepositoryStore()
    const { firstPost } = await createTopic(store)
    const reportId = await Effect.runPromise(
      recordForumReport(
        forumRepositoryDb(store),
        {
          id: '77777777-7777-4777-8777-777777777777',
          idempotencyKey: 'forum:report:post:1',
          publicProjection,
          reasonRef: 'reason.forum.spam',
          reporterActorRef: actor.actorRef,
          targetId: firstPost.postId,
          targetKind: 'post',
        },
        runtime,
      ),
    )

    await Effect.runPromise(
      recordForumModerationEvent(
        forumRepositoryDb(store),
        {
          actionKind: 'hide_post',
          id: '88888888-8888-4888-8888-888888888888',
          moderatorActorRef: 'actor.moderator',
          publicProjection,
          reasonRef: 'reason.forum.report_reviewed',
          reportId,
          targetId: firstPost.postId,
          targetKind: 'post',
        },
        runtime,
      ),
    )

    expect(store.reports).toStrictEqual([
      {
        id: reportId,
        reason_ref: 'reason.forum.spam',
        reporter_actor_ref: actor.actorRef,
        status: 'open',
        target_id: firstPost.postId,
        target_kind: 'post',
      },
    ])
    expect(store.moderationEvents).toHaveLength(1)
    expect(store.moderationEvents[0]).toMatchObject({ report_id: reportId })
  })

  test('lists private messages only for the sender or recipient actor', async () => {
    const store = new ForumRepositoryStore()

    await Effect.runPromise(
      createForumPrivateMessageThread(
        forumRepositoryDb(store),
        {
          createdByActorRef: 'actor.alice',
          id: '99999999-9999-4999-8999-999999999999',
          participantRefs: ['actor.alice', 'actor.bob'],
          slug: 'alice-bob',
          subject: 'Site review',
        },
        runtime,
      ),
    )
    await Effect.runPromise(
      addForumPrivateMessage(
        forumRepositoryDb(store),
        {
          contentRef: 'content.private.alice_bob.1',
          id: 'aaaaaaaa-aaaa-4ccc-8ddd-eeeeeeeeeeee',
          publicProjection,
          recipientActorRef: 'actor.bob',
          senderActorRef: 'actor.alice',
          threadId: '99999999-9999-4999-8999-999999999999',
        },
        runtime,
      ),
    )
    await Effect.runPromise(
      addForumPrivateMessage(
        forumRepositoryDb(store),
        {
          contentRef: 'content.private.alice_carol.1',
          id: 'bbbbbbbb-aaaa-4ccc-8ddd-eeeeeeeeeeee',
          publicProjection,
          recipientActorRef: 'actor.carol',
          senderActorRef: 'actor.alice',
          threadId: '99999999-9999-4999-8999-999999999999',
        },
        runtime,
      ),
    )

    const bobMessages = await Effect.runPromise(
      listForumPrivateMessagesForActor(forumRepositoryDb(store), 'actor.bob'),
    )
    const carolMessages = await Effect.runPromise(
      listForumPrivateMessagesForActor(forumRepositoryDb(store), 'actor.carol'),
    )

    expect(bobMessages.map(item => item.content_ref)).toStrictEqual([
      'content.private.alice_bob.1',
    ])
    expect(carolMessages.map(item => item.content_ref)).toStrictEqual([
      'content.private.alice_carol.1',
    ])
    expect(store.privateMessageThreads[0]?.message_count).toBe(2)
  })

  test('records only public-safe redacted receipts', async () => {
    const store = new ForumRepositoryStore()
    const { firstPost, topic } = await createTopic(store)
    const receiptRef = await Effect.runPromise(
      recordForumReceipt(
        forumRepositoryDb(store),
        {
          actionKind: 'post_reward',
          amountAsset: 'sats',
          amountValue: 1000,
          id: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
          publicProjection,
          receiptRef: 'receipt.forum.post_reward.1',
          recipientActorRef: actor.actorRef,
          redactedPaymentRef: 'payment.redacted.forum.post_reward.1',
          targetForumId: topic.forumId,
          targetPostId: firstPost.postId,
          targetTopicId: topic.topicId,
        },
        runtime,
      ),
    )

    expect(receiptRef).toBe('receipt.forum.post_reward.1')
    expect(store.receipts).toStrictEqual([
      {
        action_kind: 'post_reward',
        amount_asset: 'sats',
        amount_value: 1000,
        id: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
        receipt_ref: 'receipt.forum.post_reward.1',
        redacted_payment_ref: 'payment.redacted.forum.post_reward.1',
      },
    ])

    await expect(
      Effect.runPromise(
        recordForumReceipt(
          forumRepositoryDb(store),
          {
            actionKind: 'post_reward',
            amountAsset: 'sats',
            amountValue: 1000,
            id: 'dddddddd-dddd-4ddd-8ddd-dddddddddddd',
            publicProjection,
            receiptRef: 'receipt.forum.post_reward.2',
            recipientActorRef: actor.actorRef,
            redactedPaymentRef: 'lnbc1rawinvoice',
            targetForumId: topic.forumId,
            targetPostId: firstPost.postId,
            targetTopicId: topic.topicId,
          },
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(ForumValidationError)

    await expect(
      Effect.runPromise(
        recordForumReceipt(
          forumRepositoryDb(store),
          {
            actionKind: 'post_reward',
            amountAsset: 'sats',
            amountValue: 1000,
            id: 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee',
            publicProjection: {
              ...publicProjection,
              safeArtifactRefs: ['provider_payload.raw'],
            } satisfies ForumPublicProjection,
            receiptRef: 'receipt.forum.post_reward.3',
            recipientActorRef: actor.actorRef,
            redactedPaymentRef: 'payment.redacted.forum.post_reward.3',
            targetForumId: topic.forumId,
            targetPostId: firstPost.postId,
            targetTopicId: topic.topicId,
          },
          runtime,
        ),
      ),
    ).rejects.toBeInstanceOf(ForumPublicProjectionUnsafe)
  })
})
