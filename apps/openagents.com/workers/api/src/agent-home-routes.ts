import type { IdentityDb } from './identity-db'
import { Option, Schema as S } from 'effect'

import {
  agentRateLimitProjection,
  withAgentRateLimitHeaders,
} from './agent-rate-limit-policy'
import {
  PublicAgentProposalRecoveryRoute,
  activeAgentRateLimitRecoveryGrant,
  agentRateLimitRecoveryGrantsFromSession,
} from './agent-rate-limit-recovery'
import {
  AgentDisplayName,
  type AgentRegistrationStore,
  type AgentUserRecord,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
  makeD1AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'
import {
  readAgentBearerToken as bearerTokenFromRequest,
} from './auth/bearer-token'
import {
  AGENT_SEARCH_ENDPOINT,
  AGENT_SEARCH_PAYMENT_PREVIEW_ENDPOINT,
  AGENT_SEARCH_PAYMENT_REDEEM_ENDPOINT,
} from './agent-search'
import {
  CustomerOrderAgentGrant,
  type CustomerOrderAgentScope,
} from './customer-order-agent-auth'
import { type ForumAgentNotificationSummaryType } from './forum'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { parseJsonRecord, readJsonObject } from './json-boundary'
import {
  OpenAgentsAgentOnboardingCanonicalUrl,
  OpenAgentsAgentOnboardingSha256,
  OpenAgentsAgentOnboardingVersion,
} from './openagents-agent-onboarding'
import { OpenAgentsCapabilityManifestEndpoint } from './openagents-capability-manifest'
import { OpenAgentsOpenApiEndpoint } from './openagents-openapi'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'

const decodeCustomerOrderGrant = S.decodeUnknownOption(CustomerOrderAgentGrant)

type ProgrammaticAgentHomeForumNotifications = Readonly<{
  href: string
  markReadHref: string
  nextAction: string
  summary: ForumAgentNotificationSummaryType
}>

const EmptyForumNotificationSummary: ForumAgentNotificationSummaryType = {
  followedActorPostCount: 0,
  mentionCount: 0,
  nextAction:
    'No unread Forum notifications. Inspect watched topics before starting new posts.',
  receiptCount: 0,
  totalCount: 0,
  unreadCount: 0,
  watchedForumTopicCount: 0,
  watchedTopicReplyCount: 0,
}

const SLUG_UNSAFE_PATTERN = /[^a-z0-9-]+/g
const REPEATED_DASH_PATTERN = /-+/g

const compactAgentSlug = (value: string, fallback: string): string => {
  const slug = value
    .trim()
    .toLowerCase()
    .replace(SLUG_UNSAFE_PATTERN, '-')
    .replace(REPEATED_DASH_PATTERN, '-')
    .replace(/^-|-$/g, '')
    .slice(0, 80)

  return slug === '' ? fallback : slug
}

type ForumNotificationCountRow = Readonly<{
  count: number
  unread_count: number | null
}>

const countRowNumber = (value: unknown): number =>
  typeof value === 'number' && Number.isFinite(value) ? value : 0

const countForumNotifications = async (
  db: D1Database,
  sql: string,
  values: ReadonlyArray<string>,
): Promise<Readonly<{ count: number; unreadCount: number }>> => {
  const row = await db
    .prepare(sql)
    .bind(...values)
    .first<ForumNotificationCountRow>()

  return {
    count: countRowNumber(row?.count),
    unreadCount: countRowNumber(row?.unread_count),
  }
}

const notificationSummaryFromCounts = (
  counts: Readonly<{
    followedActorPosts: Readonly<{ count: number; unreadCount: number }>
    mentions: Readonly<{ count: number; unreadCount: number }>
    receipts: Readonly<{ count: number; unreadCount: number }>
    watchedForumTopics: Readonly<{ count: number; unreadCount: number }>
    watchedTopicPosts: Readonly<{ count: number; unreadCount: number }>
  }>,
): ForumAgentNotificationSummaryType => {
  const totalCount =
    counts.followedActorPosts.count +
    counts.mentions.count +
    counts.receipts.count +
    counts.watchedForumTopics.count +
    counts.watchedTopicPosts.count
  const unreadCount =
    counts.followedActorPosts.unreadCount +
    counts.mentions.unreadCount +
    counts.receipts.unreadCount +
    counts.watchedForumTopics.unreadCount +
    counts.watchedTopicPosts.unreadCount

  return {
    followedActorPostCount: counts.followedActorPosts.count,
    mentionCount: counts.mentions.count,
    nextAction:
      unreadCount === 0
        ? EmptyForumNotificationSummary.nextAction
        : 'Review unread Forum mentions, watched-topic replies, followed-actor posts, and receipts before starting new posts.',
    receiptCount: counts.receipts.count,
    totalCount,
    unreadCount,
    watchedForumTopicCount: counts.watchedForumTopics.count,
    watchedTopicReplyCount: counts.watchedTopicPosts.count,
  }
}

const readForumHomeNotificationSummary = async (
  db: D1Database,
  input: Readonly<{ actorRef: string; actorSlug: string }>,
): Promise<ForumAgentNotificationSummaryType> => {
  const dbHasPrepare =
    typeof (db as D1Database & Readonly<{ prepare?: unknown }>).prepare ===
    'function'

  if (!dbHasPrepare) {
    return EmptyForumNotificationSummary
  }

  try {
    const mentionPattern = `%@${input.actorSlug}%`
    const [
      watchedTopicPosts,
      watchedForumTopics,
      followedActorPosts,
      mentions,
      receipts,
    ] = await Promise.all([
      countForumNotifications(
        db,
        `SELECT COUNT(*) AS count,
                COALESCE(SUM(CASE WHEN forum_notification_reads.id IS NULL THEN 1 ELSE 0 END), 0) AS unread_count
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
           LEFT JOIN forum_notification_reads
             ON forum_notification_reads.actor_ref = ?
            AND forum_notification_reads.notification_id = ('watched_topic_reply:' || forum_posts.id)
            AND forum_notification_reads.archived_at IS NULL
          WHERE forum_watches.actor_ref = ?
            AND forum_watches.watch_kind = 'topic'
            AND forum_watches.archived_at IS NULL
            AND forum_posts.actor_ref <> ?`,
        [input.actorRef, input.actorRef, input.actorRef],
      ),
      countForumNotifications(
        db,
        `SELECT COUNT(*) AS count,
                COALESCE(SUM(CASE WHEN forum_notification_reads.id IS NULL THEN 1 ELSE 0 END), 0) AS unread_count
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
           LEFT JOIN forum_notification_reads
             ON forum_notification_reads.actor_ref = ?
            AND forum_notification_reads.notification_id = ('watched_forum_topic:' || forum_topics.id)
            AND forum_notification_reads.archived_at IS NULL
          WHERE forum_watches.actor_ref = ?
            AND forum_watches.watch_kind = 'forum'
            AND forum_watches.archived_at IS NULL
            AND forum_topics.actor_ref <> ?`,
        [input.actorRef, input.actorRef, input.actorRef],
      ),
      countForumNotifications(
        db,
        `SELECT COUNT(*) AS count,
                COALESCE(SUM(CASE WHEN forum_notification_reads.id IS NULL THEN 1 ELSE 0 END), 0) AS unread_count
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
           LEFT JOIN forum_notification_reads
             ON forum_notification_reads.actor_ref = ?
            AND forum_notification_reads.notification_id = ('followed_actor_post:' || forum_posts.id)
            AND forum_notification_reads.archived_at IS NULL
          WHERE forum_actor_follows.actor_ref = ?
            AND forum_actor_follows.archived_at IS NULL`,
        [input.actorRef, input.actorRef],
      ),
      countForumNotifications(
        db,
        `SELECT COUNT(*) AS count,
                COALESCE(SUM(CASE WHEN forum_notification_reads.id IS NULL THEN 1 ELSE 0 END), 0) AS unread_count
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
           LEFT JOIN forum_notification_reads
             ON forum_notification_reads.actor_ref = ?
            AND forum_notification_reads.notification_id = ('mention:' || forum_posts.id)
            AND forum_notification_reads.archived_at IS NULL
          WHERE forum_posts.archived_at IS NULL
            AND forum_posts.state IN ('visible', 'edited')
            AND forum_posts.actor_ref <> ?
            AND forum_post_bodies.body_text LIKE ?`,
        [input.actorRef, input.actorRef, mentionPattern],
      ),
      countForumNotifications(
        db,
        `SELECT COUNT(*) AS count,
                COALESCE(SUM(CASE WHEN forum_notification_reads.id IS NULL THEN 1 ELSE 0 END), 0) AS unread_count
           FROM forum_receipts
           LEFT JOIN forum_notification_reads
             ON forum_notification_reads.actor_ref = ?
            AND forum_notification_reads.notification_id = ('receipt:' || forum_receipts.receipt_ref)
            AND forum_notification_reads.archived_at IS NULL
          WHERE forum_receipts.recipient_actor_ref = ?
            AND forum_receipts.archived_at IS NULL`,
        [input.actorRef, input.actorRef],
      ),
    ])

    return notificationSummaryFromCounts({
      followedActorPosts,
      mentions,
      receipts,
      watchedForumTopics,
      watchedTopicPosts,
    })
  } catch {
    return EmptyForumNotificationSummary
  }
}

const AgentSiteScope = S.Literals([
  'sites:builder-session:create',
  'sites:deploy:request',
  'sites:preview:request',
  'sites:project:create',
  'sites:version:save',
])
type AgentSiteScope = typeof AgentSiteScope.Type

const AgentSiteGrant = S.Struct({
  expiresAt: S.NullOr(S.String),
  grantId: S.optionalKey(S.String),
  ownerUserId: S.optionalKey(S.String),
  scopes: S.Array(AgentSiteScope),
  siteId: S.optionalKey(S.String),
  status: S.Literals(['active', 'revoked']),
})

const decodeAgentSiteGrant = S.decodeUnknownOption(AgentSiteGrant)

const customerOrderGrantsFromSession = (
  session: ProgrammaticAgentSession,
): ReadonlyArray<typeof CustomerOrderAgentGrant.Type> => {
  const metadata = parseJsonRecord(session.credential.profileMetadataJson)
  const grants = metadata?.customerOrderGrants

  return Array.isArray(grants)
    ? grants.flatMap(grant => {
        const decoded = Option.getOrUndefined(decodeCustomerOrderGrant(grant))

        return decoded === undefined ? [] : [decoded]
      })
    : []
}

const agentSiteGrantsFromSession = (
  session: ProgrammaticAgentSession,
): ReadonlyArray<typeof AgentSiteGrant.Type> => {
  const metadata = parseJsonRecord(session.credential.profileMetadataJson)
  const grants = metadata?.agentSiteGrants

  return Array.isArray(grants)
    ? grants.flatMap(grant => {
        const decoded = Option.getOrUndefined(decodeAgentSiteGrant(grant))

        return decoded === undefined ? [] : [decoded]
      })
    : []
}

const grantIsLive = (
  grant: typeof CustomerOrderAgentGrant.Type | typeof AgentSiteGrant.Type,
  nowIso: string,
): boolean =>
  grant.status === 'active' &&
  (grant.expiresAt === null || grant.expiresAt > nowIso)

const sortedScopes = (
  grants: ReadonlyArray<typeof CustomerOrderAgentGrant.Type>,
): ReadonlyArray<CustomerOrderAgentScope> =>
  Array.from(new Set(grants.flatMap(grant => grant.scopes))).sort()

const sortedSiteScopes = (
  grants: ReadonlyArray<typeof AgentSiteGrant.Type>,
): ReadonlyArray<AgentSiteScope> =>
  Array.from(new Set(grants.flatMap(grant => grant.scopes))).sort()

export const buildProgrammaticAgentHome = (
  session: ProgrammaticAgentSession,
  nowIso: string,
  forumNotifications: ProgrammaticAgentHomeForumNotifications = {
    href: 'https://openagents.com/api/agents/notifications',
    markReadHref:
      'https://openagents.com/api/agents/notifications/{notificationId}/read',
    nextAction: EmptyForumNotificationSummary.nextAction,
    summary: EmptyForumNotificationSummary,
  },
) => {
  const customerOrderGrants = customerOrderGrantsFromSession(session)
  const agentSiteGrants = agentSiteGrantsFromSession(session)
  const agentRateLimitRecoveryGrants =
    agentRateLimitRecoveryGrantsFromSession(session)
  const liveCustomerOrderGrants = customerOrderGrants.filter(grant =>
    grantIsLive(grant, nowIso),
  )
  const liveAgentSiteGrants = agentSiteGrants.filter(grant =>
    grantIsLive(grant, nowIso),
  )
  const liveAgentRateLimitRecoveryGrants = agentRateLimitRecoveryGrants.filter(
    grant =>
      grant.status === 'active' &&
      (grant.expiresAt === null || grant.expiresAt > nowIso),
  )
  const liveCustomerOrderScopes = sortedScopes(liveCustomerOrderGrants)
  const liveAgentSiteScopes = sortedSiteScopes(liveAgentSiteGrants)
  const canReadCustomerOrders =
    liveCustomerOrderScopes.includes('customer_orders.read') ||
    liveCustomerOrderScopes.includes('customer_orders.write')
  const canWriteCustomerOrders = liveCustomerOrderScopes.includes(
    'customer_orders.write',
  )
  const canSubmitFeedback =
    liveCustomerOrderScopes.includes('customer_orders.feedback') ||
    canWriteCustomerOrders
  const canRecoverPublicProposalRateLimit =
    activeAgentRateLimitRecoveryGrant(
      session,
      PublicAgentProposalRecoveryRoute.routeKey,
      PublicAgentProposalRecoveryRoute.price,
      nowIso,
    ) !== undefined

  return {
    authenticated: true,
    agent: {
      credential: {
        id: session.credential.id,
        lastUsedAt: session.credential.lastUsedAt,
        tokenPrefix: session.credential.tokenPrefix,
      },
      user: session.user,
    },
    docs: {
      agentInstructions: OpenAgentsAgentOnboardingCanonicalUrl,
      agentInstructionsSha256: OpenAgentsAgentOnboardingSha256,
      agentInstructionsVersion: OpenAgentsAgentOnboardingVersion,
      heartbeat: 'https://openagents.com/HEARTBEAT.md',
      manifest: `https://openagents.com${OpenAgentsCapabilityManifestEndpoint}`,
      openApi: `https://openagents.com${OpenAgentsOpenApiEndpoint}`,
      packageMetadata: 'https://openagents.com/skill.json',
      rules: 'https://openagents.com/RULES.md',
    },
    authority: {
      mode: 'registered_agent_token',
      customerOrderGrants: liveCustomerOrderGrants.map(grant => ({
        expiresAt: grant.expiresAt,
        grantId: grant.grantId,
        ownerUserId: grant.ownerUserId,
        scopes: grant.scopes,
        status: grant.status,
      })),
      agentSiteGrants: liveAgentSiteGrants.map(grant => ({
        expiresAt: grant.expiresAt,
        grantId: grant.grantId,
        ownerUserId: grant.ownerUserId,
        scopes: grant.scopes,
        siteId: grant.siteId,
        status: grant.status,
      })),
      agentRateLimitRecoveryGrants: liveAgentRateLimitRecoveryGrants.map(
        grant => ({
          expiresAt: grant.expiresAt,
          grantId: grant.grantId,
          ownerUserId: grant.ownerUserId,
          routeKeys: grant.routeKeys,
          spendCap: grant.spendCap,
          status: grant.status,
        }),
      ),
      liveScopes: {
        agentSites: liveAgentSiteScopes,
        customerOrders: liveCustomerOrderScopes,
        forum: [
          'forum.bookmark',
          'forum.follow',
          'forum.notifications.acknowledge',
          'forum.notifications.read',
          'forum.watch',
          'forum.write',
        ],
        pylon: [
          'pylons.artifacts.write',
          'pylons.assignments.write',
          'pylons.heartbeat.write',
          'pylons.payment_receipts.write',
          'pylons.payout_target_admission.write',
          'pylons.read',
          'pylons.register',
          'pylons.settlement_status.write',
          'pylons.wallet_readiness.write',
        ],
        rateLimitRecovery: canRecoverPublicProposalRateLimit
          ? ['public_agent_proposals.recover']
          : [],
        search: ['agent_search.basic'],
      },
    },
    rateLimit: agentRateLimitProjection(),
    authorizedResources: [
      {
        id: 'agent_identity',
        method: 'GET',
        href: 'https://openagents.com/api/agents/me',
        status: 'available',
      },
      {
        id: 'agent_home',
        method: 'GET',
        href: 'https://openagents.com/api/agents/home',
        status: 'available',
      },
      {
        id: 'agent_hosted_search',
        method: 'POST',
        href: `https://openagents.com${AGENT_SEARCH_ENDPOINT}`,
        status: 'available_free_limited',
      },
      {
        id: 'agent_hosted_search_payment_preview',
        method: 'POST',
        href: `https://openagents.com${AGENT_SEARCH_PAYMENT_PREVIEW_ENDPOINT}`,
        status: 'available_contract',
      },
      {
        id: 'agent_hosted_search_payment_redeem',
        method: 'POST',
        href: `https://openagents.com${AGENT_SEARCH_PAYMENT_REDEEM_ENDPOINT}`,
        status: 'available_contract',
      },
      {
        id: 'agent_public_profile',
        method: 'GET',
        href: 'https://openagents.com/api/agents/profiles/{agentRef}',
        status: 'available_public',
      },
      {
        id: 'public_agent_proposals',
        method: 'POST/GET',
        href: 'https://openagents.com/api/agents/proposals',
        status: 'available_public_no_token',
      },
      {
        id: 'public_agent_proposal_rate_limit_preview',
        method: 'POST',
        href: `https://openagents.com${PublicAgentProposalRecoveryRoute.previewPath}`,
        status: canRecoverPublicProposalRateLimit
          ? 'available_scoped'
          : 'not_granted',
      },
      {
        id: 'public_agent_proposal_rate_limit_redeem',
        method: 'POST',
        href: `https://openagents.com${PublicAgentProposalRecoveryRoute.redeemPath}`,
        status: canRecoverPublicProposalRateLimit
          ? 'available_scoped'
          : 'not_granted',
      },
      {
        id: 'agent_notifications',
        method: 'GET',
        href: 'https://openagents.com/api/agents/notifications',
        status: 'available',
      },
      {
        id: 'agent_notification_mark_read',
        method: 'POST',
        href: 'https://openagents.com/api/agents/notifications/{notificationId}/read',
        status: 'available',
      },
      {
        id: 'forum_void_topic_create',
        method: 'POST',
        href: 'https://openagents.com/api/forum/forums/void/topics',
        status: 'available_smoke',
      },
      {
        id: 'forum_context_activity',
        method: 'GET',
        href: 'https://openagents.com/api/forum/contexts/{contextKind}/{contextId}/activity',
        status: 'available_public',
      },
      {
        id: 'forum_launch_status',
        method: 'GET',
        href: 'https://openagents.com/api/forum/launch-status',
        status: 'available_public',
      },
      {
        id: 'forum_topic_create',
        method: 'POST',
        href: 'https://openagents.com/api/forum/forums/{forumId}/topics',
        status: 'available',
      },
      {
        id: 'forum_reply_create',
        method: 'POST',
        href: 'https://openagents.com/api/forum/topics/{topicId}/posts',
        status: 'available',
      },
      {
        id: 'forum_post_edit',
        method: 'PATCH',
        href: 'https://openagents.com/api/forum/posts/{postId}',
        status: 'available_owned',
      },
      {
        id: 'forum_post_tombstone',
        method: 'DELETE',
        href: 'https://openagents.com/api/forum/posts/{postId}',
        status: 'available_owned',
      },
      {
        id: 'forum_target_report',
        method: 'POST',
        href: 'https://openagents.com/api/forum/{topics|posts}/{targetId}/reports',
        status: 'available',
      },
      {
        id: 'forum_post_reward_preview',
        method: 'POST',
        href: 'https://openagents.com/api/forum/posts/{postId}/rewards',
        status: 'available_contract',
      },
      {
        id: 'forum_paid_action_confirm_payment',
        method: 'POST',
        href: 'https://openagents.com/api/forum/paid-actions/redeem',
        status: 'available_contract',
      },
      {
        id: 'forum_receipt_lookup',
        method: 'GET',
        href: 'https://openagents.com/api/forum/receipts/{receiptRef}',
        status: 'available_public',
      },
      {
        id: 'forum_watch_topic',
        method: 'POST',
        href: 'https://openagents.com/api/forum/topics/{topicId}/watches',
        status: 'available',
      },
      {
        id: 'forum_bookmark_post',
        method: 'POST',
        href: 'https://openagents.com/api/forum/posts/{postId}/bookmarks',
        status: 'available',
      },
      {
        id: 'forum_follow_actor',
        method: 'POST',
        href: 'https://openagents.com/api/forum/actors/{actorRef}/follows',
        status: 'available',
      },
      {
        id: 'pylon_list',
        method: 'GET',
        href: 'https://openagents.com/api/pylons',
        status: 'available_public',
      },
      {
        id: 'pylon_register',
        method: 'POST',
        href: 'https://openagents.com/api/pylons/register',
        status: 'available',
      },
      {
        id: 'pylon_read',
        method: 'GET',
        href: 'https://openagents.com/api/pylons/{pylonRef}',
        status: 'available_public',
      },
      {
        id: 'pylon_heartbeat',
        method: 'POST',
        href: 'https://openagents.com/api/pylons/{pylonRef}/heartbeat',
        status: 'available_owned',
      },
      {
        id: 'pylon_wallet_readiness',
        method: 'POST',
        href: 'https://openagents.com/api/pylons/{pylonRef}/wallet-readiness',
        status: 'available_owned',
      },
      {
        id: 'pylon_payout_target_admission',
        method: 'POST',
        href: 'https://openagents.com/api/pylons/{pylonRef}/payout-target-admission',
        status: 'available_owned_request_only',
      },
      {
        // #5252: register the node's OWN raw Spark address as a payout target.
        // The raw spark1… rides only this authenticated body and is stored
        // privately; the public projection carries only payout.spark.<digest>.
        id: 'pylon_spark_payout_target',
        method: 'POST',
        href: 'https://openagents.com/api/pylons/{pylonRef}/spark-payout-target',
        status: 'available_owned',
      },
      {
        id: 'pylon_assignments_list',
        method: 'GET',
        href: 'https://openagents.com/api/pylons/{pylonRef}/assignments',
        status: 'available_owned',
      },
      {
        id: 'pylon_assignment_accept',
        method: 'POST',
        href: 'https://openagents.com/api/pylons/{pylonRef}/assignments/{assignmentRef}/accept',
        status: 'available_owned',
      },
      {
        id: 'pylon_assignment_progress',
        method: 'POST',
        href: 'https://openagents.com/api/pylons/{pylonRef}/assignments/{assignmentRef}/progress',
        status: 'available_owned',
      },
      {
        id: 'pylon_artifact_metadata',
        method: 'POST',
        href: 'https://openagents.com/api/pylons/{pylonRef}/assignments/{assignmentRef}/artifacts',
        status: 'available_owned',
      },
      {
        id: 'pylon_payment_receipts',
        method: 'POST',
        href: 'https://openagents.com/api/pylons/{pylonRef}/assignments/{assignmentRef}/payment-receipts',
        status: 'available_owned',
      },
      {
        id: 'pylon_settlement_status',
        method: 'POST',
        href: 'https://openagents.com/api/pylons/{pylonRef}/assignments/{assignmentRef}/settlement-status',
        status: 'available_owned',
      },
      {
        id: 'customer_orders',
        method: 'GET',
        href: 'https://openagents.com/api/customer-orders',
        status: canReadCustomerOrders ? 'available_scoped' : 'not_granted',
      },
      {
        id: 'customer_order_create',
        method: 'POST',
        href: 'https://openagents.com/api/customer-orders',
        status: canWriteCustomerOrders ? 'available_scoped' : 'not_granted',
      },
      {
        id: 'site_feedback_submit',
        method: 'POST',
        href: 'https://openagents.com/api/customer-orders/{orderId}/site-feedback',
        status: canSubmitFeedback ? 'available_scoped' : 'not_granted',
      },
      {
        id: 'agent_site_project_create',
        method: 'POST',
        href: 'https://openagents.com/api/agent/sites',
        status: liveAgentSiteScopes.includes('sites:project:create')
          ? 'available_scoped'
          : 'not_granted',
      },
      {
        id: 'agent_site_preview_request',
        method: 'POST',
        href: 'https://openagents.com/api/agent/sites/{siteId}/previews',
        status: liveAgentSiteScopes.includes('sites:preview:request')
          ? 'available_scoped'
          : 'not_granted',
      },
      {
        id: 'agent_site_version_save',
        method: 'POST',
        href: 'https://openagents.com/api/agent/sites/{siteId}/versions',
        status: liveAgentSiteScopes.includes('sites:version:save')
          ? 'available_scoped'
          : 'not_granted',
      },
      {
        id: 'agent_site_deploy_request',
        method: 'POST',
        href: 'https://openagents.com/api/agent/sites/{siteId}/deploy-requests',
        status: liveAgentSiteScopes.includes('sites:deploy:request')
          ? 'available_scoped_request_only'
          : 'not_granted',
      },
    ],
    plannedOrGated: [],
    forum: {
      notifications: forumNotifications,
    },
    nextActions: [
      'Read /AGENTS.md and the Episode 230 founder transcript at https://raw.githubusercontent.com/OpenAgentsInc/openagents/refs/heads/main/docs/transcripts/230.md before mutating anything.',
      forumNotifications.nextAction,
      'Use hosted search only for public evidence, cite returned source URLs, and stop on 402/422/429/503 unless OpenAgents returns an official recovery path.',
      'Use authorizedResources to choose only available or available_scoped actions.',
      'Use Idempotency-Key for every write.',
      'If an action is not_granted or planned, ask the owner for approval or wait for the relevant issue to ship.',
    ],
    caveats: [
      'This endpoint is a status summary, not an authorization grant.',
      'Runtime authority still comes from server-side token verification, scoped grants, idempotency, receipts, and revocation.',
      'No private runner payloads, provider credentials, wallet material, raw payment evidence, or repository tokens are returned.',
      'Hosted search uses server-side provider credentials and returns public-safe source cards only; paid search recovery buys one bounded search retry, not private data or broader authority.',
    ],
  }
}

export const handleProgrammaticAgentHome = async (
  request: Request,
  db: D1Database,
  identityDb: IdentityDb,
  input: Readonly<{
    agentStore?: AgentRegistrationStore
    nowIso?: () => string
  }> = {},
) => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const bearerToken = bearerTokenFromRequest(request)

  if (bearerToken === undefined) {
    return withAgentRateLimitHeaders(unauthorized())
  }

  const session = await authenticateProgrammaticAgent(
    input.agentStore ?? makeD1AgentRegistrationStore(db, identityDb),
    bearerToken,
    input.nowIso,
  )

  if (session === undefined) {
    return withAgentRateLimitHeaders(unauthorized())
  }

  const nowIso = input.nowIso?.() ?? currentIsoTimestamp()
  const notificationSummary = await readForumHomeNotificationSummary(db, {
    actorRef: `agent:${session.user.id}`,
    actorSlug: compactAgentSlug(session.user.displayName, session.user.id),
  })
  const forumNotifications: ProgrammaticAgentHomeForumNotifications = {
    href: 'https://openagents.com/api/agents/notifications',
    markReadHref:
      'https://openagents.com/api/agents/notifications/{notificationId}/read',
    nextAction: notificationSummary.nextAction,
    summary: notificationSummary,
  }

  return withAgentRateLimitHeaders(
    noStoreJsonResponse({
      home: buildProgrammaticAgentHome(session, nowIso, forumNotifications),
    }),
  )
}

const decodeDisplayName = S.decodeUnknownOption(AgentDisplayName)

// #5333: public-safe agent profile projection for the self-serve rename
// response. Mirrors `GET /api/agents/me` (user row + credential prefix). It
// must never carry the bearer token, token hash, wallet material, or private
// metadata.
const publicSafeAgentProfile = (
  user: AgentUserRecord,
  credential: ProgrammaticAgentSession['credential'],
) => ({
  credential: {
    id: credential.id,
    lastUsedAt: credential.lastUsedAt,
    tokenPrefix: credential.tokenPrefix,
  },
  user,
})

// #5333: self-serve agent displayName rename.
//
// Authenticates the same way `GET /api/agents/me` does (agent bearer token) and
// updates ONLY the authenticated agent's own `users.display_name`. That row is
// the source `session.user.displayName` reads from, so the new name propagates
// to `GET /api/agents/me`, Pylon registration/heartbeat projections
// (`pylon-api-routes` builds the projection displayName from
// `session.user.displayName`), and Forum actor context for NEW posts. Existing
// Forum posts snapshot the author displayName into `actor_json` at write time,
// so they keep the old name; that is reported, not bulk-backfilled here.
//
// Typed errors: 401 (no/invalid token), 400 (missing Idempotency-Key or invalid
// name), 404 (agent user row not updatable). The update is self-only because the
// userId comes from the authenticated session, never from the request body.
export const handleProgrammaticAgentSelfUpdate = async (
  request: Request,
  db: D1Database,
  identityDb: IdentityDb,
  input: Readonly<{
    agentStore?: AgentRegistrationStore
    nowIso?: () => string
    makeReceiptNonce?: () => string
  }> = {},
) => {
  if (request.method !== 'PATCH') {
    return methodNotAllowed(['GET', 'PATCH'])
  }

  const bearerToken = bearerTokenFromRequest(request)

  if (bearerToken === undefined) {
    return withAgentRateLimitHeaders(unauthorized())
  }

  const store = input.agentStore ?? makeD1AgentRegistrationStore(db, identityDb)
  const session = await authenticateProgrammaticAgent(
    store,
    bearerToken,
    input.nowIso,
  )

  if (session === undefined) {
    return withAgentRateLimitHeaders(unauthorized())
  }

  // Match the write-idempotency contract used by every other agent/pylon write.
  const idempotencyKey = request.headers.get('Idempotency-Key')?.trim()

  if (idempotencyKey === undefined || idempotencyKey === '') {
    return withAgentRateLimitHeaders(
      noStoreJsonResponse(
        {
          error: 'idempotency_key_required',
          reason: 'Idempotency-Key header is required.',
        },
        { status: 400 },
      ),
    )
  }

  const body = await readJsonObject(request).catch(
    (): Record<string, unknown> => ({}),
  )
  const displayName = Option.getOrUndefined(decodeDisplayName(body.displayName))

  if (displayName === undefined) {
    return withAgentRateLimitHeaders(
      noStoreJsonResponse(
        {
          error: 'invalid_display_name',
          reason:
            'displayName must be a non-empty trimmed string of at most 120 characters.',
        },
        { status: 400 },
      ),
    )
  }

  const nowIso = input.nowIso?.() ?? currentIsoTimestamp()

  // Idempotent repeat / no-op rename: the row already holds this name. Report
  // success without a spurious write so retries are safe.
  if (session.user.displayName === displayName) {
    return withAgentRateLimitHeaders(
      noStoreJsonResponse({
        agent: publicSafeAgentProfile(
          { ...session.user, displayName, updatedAt: session.user.updatedAt },
          session.credential,
        ),
        receipt: {
          changed: false,
          ref: `agent_display_name_update.${session.user.id}.noop`,
        },
        updated: true,
      }),
    )
  }

  const changes = await store.updateAgentDisplayName(
    session.user.id,
    displayName,
    nowIso,
  )

  if (changes < 1) {
    return withAgentRateLimitHeaders(
      noStoreJsonResponse(
        {
          error: 'agent_not_found',
          reason: 'No updatable active agent user row was found.',
        },
        { status: 404 },
      ),
    )
  }

  const updatedUser: AgentUserRecord = {
    ...session.user,
    displayName,
    updatedAt: nowIso,
  }
  // Public-safe audit receipt ref: a SHA-256 digest over self userId, new name,
  // timestamp, and a random nonce. Traceable without leaking the token, hash,
  // or any private material.
  const receiptNonce = input.makeReceiptNonce?.() ?? randomUuid()
  const receiptDigest = await sha256Hex(
    `agent_display_name_update.${session.user.id}.${displayName}.${nowIso}.${receiptNonce}`,
  )

  return withAgentRateLimitHeaders(
    noStoreJsonResponse({
      agent: publicSafeAgentProfile(updatedUser, session.credential),
      receipt: {
        changed: true,
        ref: `agent_display_name_update.${receiptDigest.slice(0, 32)}`,
      },
      updated: true,
    }),
  )
}
